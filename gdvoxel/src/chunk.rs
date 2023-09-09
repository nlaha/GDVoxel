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

use godot::prelude::utilities::instance_from_id;
use rand::prelude::*;

// iso surface mesh generation
use fast_surface_nets::{surface_nets, SurfaceNetsBuffer};
use ndshape::RuntimeShape;
use ndshape::Shape;
use ndshape::{ConstShape, ConstShape3u32};

use crate::threaded_generator;
use crate::VoxelChunkManager;

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

    pub generated: bool,

    #[base]
    pub base: Base<MeshInstance3D>,
}

#[godot_api]
impl VoxelChunk {
    #[func]
    pub fn add_to_tree(&mut self, root_id: InstanceId) {
        // get the root node in the scene tree without accessing any members
        let mut root: Gd<Node> = instance_from_id(root_id.to_i64()).unwrap().cast();

        // add this node to the root node
        root.call_deferred("add_child".into(), &[self.base.to_variant()]);

        // hash position
        let hash = VoxelChunkManager::hash_chunk_pos(self.base.get_position());

        // cast root to VoxelChunkManager
        let mut root: Gd<VoxelChunkManager> = root.cast();
        // call on_chunk_added on the chunk manager
        root.bind_mut().on_chunk_added(hash, &self);
    }
}
