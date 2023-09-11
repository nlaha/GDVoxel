// godot imports
use godot::engine::mesh::ArrayType;
use godot::engine::mesh::PrimitiveType;
use godot::engine::ArrayMesh;
use godot::prelude::*;

use async_recursion::async_recursion;

// iso surface mesh generation
use fast_surface_nets::{surface_nets, SurfaceNetsBuffer};
use ndshape::RuntimeShape;
use ndshape::Shape;

use tokio::task;

use std::sync::Arc;

use crate::execute_gpu;
use crate::manager;
use crate::VoxelChunk;

pub struct ChunkData {
    pub buffer: SurfaceNetsBuffer,
}

impl Clone for ChunkData {
    fn clone(&self) -> Self {
        return ChunkData {
            buffer: SurfaceNetsBuffer {
                positions: self.buffer.positions.clone(),
                normals: self.buffer.normals.clone(),
                indices: self.buffer.indices.clone(),

                surface_points: self.buffer.surface_points.clone(),
                surface_strides: self.buffer.surface_strides.clone(),
                stride_to_index: self.buffer.stride_to_index.clone(),
            },
        };
    }
}

pub struct ThreadedGenerator {
    voxel_size: f32,
    data_resolution: u32,
    seed: i32,
    tokio_runtime: tokio::runtime::Runtime,
    root_id: InstanceId,
}

impl ThreadedGenerator {
    pub fn initialize(
        voxel_size: f32,
        data_resolution: u32,
        seed: i32,
        root_id: InstanceId,
    ) -> ThreadedGenerator {
        let mut self_ = ThreadedGenerator {
            voxel_size: voxel_size,
            data_resolution: data_resolution,
            seed: seed,
            tokio_runtime: tokio::runtime::Runtime::new().unwrap(),
            root_id: root_id,
        };

        self_.tokio_runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(16)
            .enable_all()
            .build()
            .unwrap();

        return self_;
    }

    #[async_recursion]
    async fn generation_future(
        chunk_pos: Vector3,
        voxel_size: f32,
        data_resolution: u32,
        seed: i32,
        hash: String,
        root_id: InstanceId,
        tries: u32,
    ) {
        let dimensions = [data_resolution, data_resolution * 2, data_resolution];
        let shape_3d: RuntimeShape<u32, 3> = RuntimeShape::<u32, 3>::new(dimensions);
        let shape_3d_gpu: RuntimeShape<u32, 3> = RuntimeShape::<u32, 3>::new([256, 256, 256]);

        // // Continents
        // let mut cont = Fbm::<SuperSimplex>::new(seed as u32);
        // cont.octaves = 5;
        // cont.frequency = 0.005 as f64;

        // // start a timer to check performance of isosurface generation
        // let start = std::time::Instant::now();

        // // define generation splines for noise
        // let mut cont_spline = Spline::from_vec(vec![
        //     Key::new(-1.0, 10.0, Interpolation::Linear),
        //     Key::new(0.3, 30.0, Interpolation::Cosine),
        //     Key::new(0.4, 35.0, Interpolation::Cosine),
        //     Key::new(1.0, 50.0, Interpolation::Linear),
        // ]);

        let compute_result = execute_gpu(shape_3d_gpu.clone(), chunk_pos, shape_3d.clone())
            .await
            .unwrap();

        // print out length of resulting buffer
        godot_print!(
            "[GDVoxel - Compute] Generated size: {:?}",
            compute_result.len()
        );

        // compute sdf
        let mut sdf = vec![0.0; shape_3d.usize()];
        for i in 0u32..shape_3d.size() {
            // linearize for compute_result which is larger than shape_3d
            let [x, y, z] = shape_3d.delinearize(i);

            let compute_idx = shape_3d_gpu.linearize([x, y, z]);

            //sdf[i as usize] = (cont.get([x, y, z]) + density_offset).clamp(-1.0, 1.0) as f32;
            sdf[i as usize] = compute_result[compute_idx as usize];
        }

        let mut buffer: SurfaceNetsBuffer = SurfaceNetsBuffer::default();
        surface_nets(
            &sdf,
            &shape_3d,
            [0; 3],
            [
                shape_3d.as_array()[0] - 1,
                shape_3d.as_array()[1] - 1,
                shape_3d.as_array()[2] - 1,
            ],
            &mut buffer,
        );

        // if any of the buffers are empty, try again
        // TODO: this is scary and dangerous, I don't like it
        if buffer.positions.len() == 0 || buffer.indices.len() == 0 {
            if tries < 2 {
                Self::generation_future(
                    chunk_pos,
                    voxel_size,
                    data_resolution,
                    seed,
                    hash.clone(),
                    root_id,
                    tries + 1,
                )
                .await;
                return;
            } else {
                return;
            }
        }

        // load into godot arrays
        let mut gd_positions = PackedVector3Array::new();
        let mut gd_normals = PackedVector3Array::new();
        let mut gd_indices = PackedInt32Array::new();

        // resize
        gd_positions.resize(buffer.positions.len() as usize);
        gd_normals.resize(buffer.normals.len() as usize);
        gd_indices.resize(buffer.indices.len() as usize);

        // copy
        for i in 0..buffer.positions.len() {
            gd_positions.set(
                i as usize,
                Vector3 {
                    x: buffer.positions[i][0],
                    y: buffer.positions[i][1],
                    z: buffer.positions[i][2],
                },
            );
        }

        for i in 0..buffer.normals.len() {
            gd_normals.set(
                i as usize,
                Vector3 {
                    x: buffer.normals[i][0] * -1.0,
                    y: buffer.normals[i][1] * -1.0,
                    z: buffer.normals[i][2] * -1.0,
                },
            );
        }

        for i in 0..buffer.indices.len() {
            gd_indices.set(i as usize, buffer.indices[i] as i32);
        }

        // put into mesh data array
        let mut arrays = VariantArray::new();
        arrays.resize(ArrayType::ARRAY_MAX.ord() as usize);

        // positions
        arrays.set(
            ArrayType::ARRAY_VERTEX.ord() as usize,
            gd_positions.to_variant(),
        );

        // normals
        arrays.set(
            ArrayType::ARRAY_NORMAL.ord() as usize,
            gd_normals.to_variant(),
        );

        // indices
        arrays.set(
            ArrayType::ARRAY_INDEX.ord() as usize,
            gd_indices.to_variant(),
        );

        // a chunk has been requested for this position, so we might as well spawn one in right now
        let chunk_scene: Gd<PackedScene> = load("res://voxel_chunk.tscn");

        // create a new chunk
        let mut chunk: Gd<VoxelChunk> = chunk_scene.instantiate_as();

        // set arrays
        let mut mesh = ArrayMesh::new();
        mesh.add_surface_from_arrays(PrimitiveType::PRIMITIVE_TRIANGLES, arrays);

        // set the name
        chunk.set_name(hash.clone().into());

        chunk.set_position(chunk_pos);
        chunk.set_scale(Vector3::new(voxel_size, voxel_size, voxel_size));

        chunk.set_mesh(mesh.upcast());

        chunk.call_deferred("add_to_tree".into(), &[root_id.to_variant()]);
    }

    pub fn generate_chunk_data(&mut self, positions: &Vec<Vector3>) {
        // install all chunk positions into the thread pool

        positions.iter().for_each(|chunk_pos| {
            let hash: String = manager::VoxelChunkManager::hash_chunk_pos(*chunk_pos);

            self.tokio_runtime
                .spawn(ThreadedGenerator::generation_future(
                    *chunk_pos,
                    self.voxel_size,
                    self.data_resolution,
                    self.seed,
                    hash,
                    self.root_id,
                    1,
                ));
        });
    }
}
