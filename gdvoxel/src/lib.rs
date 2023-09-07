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

// isosurface
use isosurface::linear_hashed_marching_cubes::LinearHashedMarchingCubes;
use isosurface::source::Source;

struct GDVoxel;

#[gdextension]
unsafe impl ExtensionLibrary for GDVoxel {}

struct VoxelData {
    noise: Gd<FastNoiseLite>,
    resolution: i32,
    voxel_size: f32,
}

#[derive(GodotClass)]
#[class(init, base=MeshInstance3D)]
struct VoxelChunk {
    #[export]
    #[var]
    #[init(default = 32)]
    data_resolution: i32,
    #[export]
    #[var]
    #[init(default = 8)]
    voxel_resolution: i32,
    #[export]
    #[var]
    #[init(default = 1.0)]
    voxel_size: f32,

    #[init(default = VoxelData{
        noise: FastNoiseLite::new(),
        resolution: 32,
        voxel_size: 1.0,
    })]
    voxel_data: VoxelData,

    #[var]
    mesh_positions: PackedVector3Array,
    #[var]
    mesh_indices: PackedInt32Array,
    #[var]
    mesh_normals: PackedVector3Array,

    #[base]
    base: Base<MeshInstance3D>,
}

#[godot_api]
impl VoxelChunk {
    #[func]
    fn generate(&mut self) {
        // initialize voxel data
        self.voxel_data.resolution = self.data_resolution;
        self.voxel_data.voxel_size = self.voxel_size;

        // clear mesh data
        self.mesh_positions.clear();
        self.mesh_indices.clear();
        self.mesh_normals.clear();

        // set up noise
        let mut noise = FastNoiseLite::new();
        // get a 3d noise texture with voxel resolution * 3
        noise.set_noise_type(NoiseType::TYPE_SIMPLEX);
        noise.set_frequency(1.0);
        noise.set_fractal_type(FractalType::FRACTAL_FBM);
        noise.set_fractal_octaves(4);
        noise.set_fractal_gain(0.5);
        noise.set_fractal_lacunarity(2.0);

        self.voxel_data.noise = noise;

        // start a timer to check performance of isosurface generation
        let start = std::time::Instant::now();

        // get mesh data from isosurface
        let mut lhmc = LinearHashedMarchingCubes::new(self.voxel_resolution.try_into().unwrap());

        let mut positions: Vec<f32> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();

        lhmc.extract(&self.voxel_data, &mut positions, &mut indices);

        // stop timer
        let duration = start.elapsed();

        // print time
        godot_print!(
            "[GDVoxel] Isosurface generation took {}us",
            duration.as_micros()
        );

        // print
        godot_print!(
            "[GDVoxel] Generated {} vertices and {} indices",
            positions.len() / 3,
            indices.len()
        );

        // convert mesh data to godot arrays
        for i in 0..positions.len() / 3 {
            self.mesh_positions.push(Vector3::new(
                positions[i * 3] * self.voxel_size,
                positions[i * 3 + 1] * self.voxel_size,
                positions[i * 3 + 2] * self.voxel_size,
            ));
        }

        for i in 0..indices.len() / 3 {
            self.mesh_indices.push(indices[i * 3] as i32);
            self.mesh_indices.push(indices[i * 3 + 1] as i32);
            self.mesh_indices.push(indices[i * 3 + 2] as i32);
        }

        // calculate smooth normals
        let mut normals: Vec<Vector3> = vec![Vector3::ZERO; positions.len() / 3];
        for i in 0..indices.len() / 3 {
            let i0 = indices[i * 3] as usize;
            let i1 = indices[i * 3 + 1] as usize;
            let i2 = indices[i * 3 + 2] as usize;

            let v0 = Vector3::new(
                positions[i0 * 3],
                positions[i0 * 3 + 1],
                positions[i0 * 3 + 2],
            );
            let v1 = Vector3::new(
                positions[i1 * 3],
                positions[i1 * 3 + 1],
                positions[i1 * 3 + 2],
            );
            let v2 = Vector3::new(
                positions[i2 * 3],
                positions[i2 * 3 + 1],
                positions[i2 * 3 + 2],
            );

            let normal = (v1 - v0).cross(v2 - v0).normalized();

            normals[i0] += normal;
            normals[i1] += normal;
            normals[i2] += normal;
        }

        for i in 0..normals.len() {
            self.mesh_normals.push(normals[i].normalized());
        }

        godot_print!("[GDVoxel] Generation complete!");
    }
}

impl Source for VoxelData {
    fn sample(&self, x: f32, y: f32, z: f32) -> f32 {
        // sample noise
        let val = self.noise.get_noise_3d(x, y, z);
        //godot_print!("Sampled noise: {}", val);
        // return value
        return val;
    }
}

// #[godot_api]
// impl MeshInstance3DVirtual for VoxelChunk {
//     fn ready(&mut self) {}
// }
