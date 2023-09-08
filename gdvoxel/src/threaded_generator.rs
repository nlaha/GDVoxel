// godot imports
use godot::builtin::Array;
use godot::engine::fast_noise_lite::FractalType;
use godot::engine::fast_noise_lite::NoiseType;
use godot::engine::mesh::ArrayType;
use godot::engine::mesh::PrimitiveType;
use godot::engine::ArrayMesh;
use godot::engine::FastNoiseLite;
use godot::engine::Mesh;
use godot::engine::MeshInstance3D;
use godot::engine::MeshInstance3DVirtual;
use godot::engine::Node3D;
use godot::prelude::*;

use rand::prelude::*;

// iso surface mesh generation
use fast_surface_nets::{surface_nets, SurfaceNetsBuffer};
use ndshape::RuntimeShape;
use ndshape::Shape;
use ndshape::{ConstShape, ConstShape3u32};

use tokio::task;

use std::collections::HashMap;
use std::sync::Arc;
use std::thread::JoinHandle;

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
    cache: Arc<moka::sync::Cache<String, ChunkData>>,
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
            cache: Arc::new(moka::sync::Cache::new(100)),
            voxel_size: voxel_size,
            data_resolution: data_resolution,
            seed: seed,
            tokio_runtime: tokio::runtime::Runtime::new().unwrap(),
            root_id: root_id,
        };

        self_.tokio_runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(8)
            .enable_all()
            .build()
            .unwrap();

        return self_;
    }

    async fn generation_future(
        chunk_pos: Vector3,
        cache: Arc<moka::sync::Cache<String, ChunkData>>,
        voxel_size: f32,
        data_resolution: u32,
        seed: i32,
        hash: String,
        root_id: InstanceId,
    ) {
        // add blank (lock) entry to cache
        cache.insert(
            hash.clone(),
            ChunkData {
                buffer: SurfaceNetsBuffer::default(),
            },
        );

        // set up noise
        let mut noise = FastNoiseLite::new();
        // get a 3d noise texture with voxel resolution * 3
        noise.set_noise_type(NoiseType::TYPE_SIMPLEX);
        noise.set_frequency(0.005);
        noise.set_fractal_type(FractalType::FRACTAL_FBM);
        noise.set_fractal_octaves(4);
        noise.set_fractal_gain(0.5);
        noise.set_fractal_lacunarity(2.0);
        noise.set_seed(seed);

        // start a timer to check performance of isosurface generation
        let start = std::time::Instant::now();

        // A 16^3 chunk with 1-voxel boundary padding.
        let dimensions = [data_resolution; 3];
        let shape = RuntimeShape::<u32, 3>::new(dimensions);

        // compute sdf
        let mut sdf = vec![0.0; shape.usize()];
        for i in 0u32..shape.size() {
            let [x, y, z] = shape.delinearize(i);

            // add offsets
            let x = x as f32 + chunk_pos.x;
            let y = y as f32 + chunk_pos.y;
            let z = z as f32 + chunk_pos.z;

            sdf[i as usize] = noise.get_noise_3d(x as f32, y as f32, z as f32);
        }

        let mut buffer: SurfaceNetsBuffer = SurfaceNetsBuffer::default();
        surface_nets(&sdf, &shape, [0; 3], [data_resolution - 1; 3], &mut buffer);

        // stop timer
        let duration = start.elapsed();

        // print time
        godot_print!(
            "[GDVoxel] Isosurface generation took {}ms",
            duration.as_millis()
        );

        // print number of normals, verts, and indices
        godot_print!(
            "[GDVoxel] Generated {} normals, {} vertices, and {} indices",
            buffer.normals.len(),
            buffer.positions.len(),
            buffer.indices.len()
        );

        // if any of the buffers are empty, return
        if buffer.positions.len() == 0 || buffer.indices.len() == 0 {
            return;
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
                    x: buffer.positions[i][0] * voxel_size,
                    y: buffer.positions[i][1] * voxel_size,
                    z: buffer.positions[i][2] * voxel_size,
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

        // set arrays
        let mut mesh = ArrayMesh::new();
        mesh.call_deferred(
            "add_surface_from_arrays".into(),
            &[
                PrimitiveType::PRIMITIVE_TRIANGLES.to_variant(),
                arrays.to_variant(),
            ],
        );

        // a chunk has been requested for this position, so we might as well spawn one in right now
        let chunk_scene: Gd<PackedScene> = load("res://voxel_chunk.tscn");

        // create a new chunk
        let mut chunk: Gd<VoxelChunk> = chunk_scene.instantiate_as();

        // set the name
        chunk.set_name(hash.clone().into());

        chunk.set_position(chunk_pos);

        chunk.set_mesh(mesh.upcast());

        chunk.call_deferred("add_to_tree".into(), &[root_id.to_variant()]);

        cache.insert(hash, ChunkData { buffer: buffer });
    }

    pub fn generate_chunk_data(&mut self, positions: &Vec<Vector3>) -> Vec<Option<ChunkData>> {
        // install all chunk positions into the thread pool

        positions.iter().for_each(|chunk_pos| {
            let hash: String = manager::VoxelChunkManager::hash_chunk_pos(*chunk_pos);

            // skip if already in cache
            if self.cache.contains_key(&hash) {
                return;
            }

            let thread = {
                let cache: Arc<moka::sync::Cache<String, ChunkData>> = Arc::clone(&self.cache);

                self.tokio_runtime
                    .spawn(ThreadedGenerator::generation_future(
                        *chunk_pos,
                        cache,
                        self.voxel_size,
                        self.data_resolution,
                        self.seed,
                        hash,
                        self.root_id,
                    ))
            };
        });

        // get data from cache
        // first, hash the chunk positions
        return positions
            .iter()
            .map(|pos| {
                let hash = manager::VoxelChunkManager::hash_chunk_pos(*pos);

                match self.cache.get(&hash) {
                    Some(data) => {
                        // return the data
                        return Some(data);
                    }
                    None => {
                        // return none
                        return None;
                    }
                }
            })
            .collect();
    }
}
