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

use crate::threaded_generator;

#[derive(GodotClass)]
#[class(init, base=MeshInstance3D)]
pub struct VoxelChunk {
    #[export]
    #[var]
    #[init(default = 32)]
    pub data_resolution: u32,
    #[export]
    #[var]
    #[init(default = 1.0)]
    pub voxel_size: f32,
    #[export]
    #[var]
    #[init(default = 0)]
    pub seed: i32,

    #[var]
    pub mesh_positions: PackedVector3Array,
    #[var]
    pub mesh_indices: PackedInt32Array,
    #[var]
    pub mesh_normals: PackedVector3Array,

    #[base]
    pub base: Base<MeshInstance3D>,
}

#[godot_api]
impl VoxelChunk {
    /// Generate a mesh for this chunk. Takes a chunk position, this is simply
    /// the floating point position of the chunk in world space. This is used to
    /// offset the mesh noise generation
    pub fn load_from_buffer(&mut self, buffer: &SurfaceNetsBuffer) {
        godot_print!("[GDVoxel - Post-Process] Starting post process...");
        // load into godot arrays
        // positions
        // allocate
        self.mesh_positions.resize(buffer.positions.len() as usize);
        // copy
        for i in 0..buffer.positions.len() {
            self.mesh_positions.set(
                i as usize,
                Vector3 {
                    x: buffer.positions[i][0] * self.voxel_size,
                    y: buffer.positions[i][1] * self.voxel_size,
                    z: buffer.positions[i][2] * self.voxel_size,
                },
            );
        }

        // normals
        // allocate
        self.mesh_normals.resize(buffer.normals.len() as usize);
        // copy
        for i in 0..buffer.normals.len() {
            self.mesh_normals.set(
                i as usize,
                Vector3 {
                    x: buffer.normals[i][0] * -1.0,
                    y: buffer.normals[i][1] * -1.0,
                    z: buffer.normals[i][2] * -1.0,
                },
            );
        }

        // indices
        // allocate
        self.mesh_indices.resize(buffer.indices.len() as usize);
        // copy
        for i in 0..buffer.indices.len() {
            self.mesh_indices.set(i as usize, buffer.indices[i] as i32);
        }

        godot_print!("[GDVoxel - Post-process] Done!");
    }
}
