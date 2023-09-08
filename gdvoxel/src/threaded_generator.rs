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
use rayon::prelude::IntoParallelIterator;
use rayon::prelude::ParallelIterator;

pub fn generate_chunk_data(
    voxel_size: f32,
    data_resolution: u32,
    seed: i32,
    positions: Vec<Vector3>,
) -> Vec<SurfaceNetsBuffer> {
    // parallel for loop
    return positions
        .into_par_iter()
        .map(|chunk_pos| {
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
                "[GDVoxel] Isosurface generation took {}us",
                duration.as_micros()
            );

            // print number of normals, verts, and indices
            godot_print!(
                "[GDVoxel] Generated {} normals, {} vertices, and {} indices",
                buffer.normals.len(),
                buffer.positions.len(),
                buffer.indices.len()
            );

            return buffer;
        })
        .collect();
}
