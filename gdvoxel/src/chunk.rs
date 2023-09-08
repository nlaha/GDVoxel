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
    }
    /// Generate a mesh for this chunk. Takes a chunk position, this is simply
    /// the floating point position of the chunk in world space. This is used to
    /// offset the mesh noise generation
    pub async fn load_from_buffer(&mut self, buffer: &SurfaceNetsBuffer) {
        // if buffer is empty, return
        if buffer.positions.len() == 0 || buffer.indices.len() == 0 {
            return;
        }

        godot_print!(
            "[GDVoxel - Post-Process - {}] Starting post process...",
            self.base.get_name()
        );
        // load into godot arrays
        // positions
        // allocate
        self.mesh_positions.resize(buffer.positions.len() as usize);
        // copy
        for i in 0..buffer.positions.len() {
            self.mesh_positions.set(
                i as usize,
                Vector3 {
                    x: buffer.positions[i][0],
                    y: buffer.positions[i][1],
                    z: buffer.positions[i][2],
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
                    x: buffer.normals[i][0],
                    y: buffer.normals[i][1],
                    z: buffer.normals[i][2],
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

        godot_print!("[GDVoxel - Post-process - {}] Done!", self.base.get_name());

        // create array mesh
        let mut mesh = ArrayMesh::new();

        // create arrays
        let mut arrays = VariantArray::new();
        arrays.resize(ArrayType::ARRAY_MAX.ord() as usize);

        // positions
        arrays.set(
            ArrayType::ARRAY_VERTEX.ord() as usize,
            self.mesh_positions.to_variant(),
        );

        // normals
        arrays.set(
            ArrayType::ARRAY_NORMAL.ord() as usize,
            self.mesh_normals.to_variant(),
        );

        // indices
        arrays.set(
            ArrayType::ARRAY_INDEX.ord() as usize,
            self.mesh_indices.to_variant(),
        );

        // set arrays
        mesh.add_surface_from_arrays(PrimitiveType::PRIMITIVE_TRIANGLES, arrays);

        self.base.set_mesh(mesh.upcast());

        self.generated = true;
    }
}
