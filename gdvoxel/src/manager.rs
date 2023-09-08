pub use crate::chunk::*;
use crate::{chunk, threaded_generator};
use godot::{engine::MeshInstance3D, prelude::*};

use std::collections::HashMap;

#[derive(GodotClass)]
#[class(init, base=Node3D)]
pub struct VoxelChunkManager {
    #[export(file)]
    #[var]
    #[init]
    chunk_scene: GodotString,

    #[export]
    #[var]
    #[init]
    player_pos: Vector3,

    #[export]
    #[var]
    #[init(default = 32)]
    data_resolution: u32,

    #[export]
    #[var]
    #[init(default = 1.0)]
    voxel_size: f32,

    #[export]
    #[var]
    #[init(default = 0)]
    seed: i32,

    #[export]
    #[var]
    #[init(default = 3)]
    render_distance: i32,

    #[init(default = HashMap::new())]
    previous_chunk_positions: HashMap<String, Vector3>,

    #[base]
    base: Base<Node3D>,
}

#[godot_api]
impl VoxelChunkManager {
    #[func]
    pub fn update_player_position(&mut self, player_pos: Vector3) {
        // if player moved more than 0 units, update chunks
        if self.player_pos.distance_to(player_pos) > 10.0 {
            self.player_pos = player_pos;
            self.base.call_deferred("update_chunks".into(), &[]);
        }
    }

    /// quantize player position to chunk position, chunk position is a grid
    /// with cell size data_resolution * voxel_size
    #[func]
    pub fn quantize_player_pos(&self) -> Vector3 {
        return Vector3::new(
            (self.player_pos.x / (self.data_resolution as f32 * self.voxel_size)).floor(),
            (self.player_pos.y / (self.data_resolution as f32 * self.voxel_size)).floor(),
            (self.player_pos.z / (self.data_resolution as f32 * self.voxel_size)).floor(),
        );
    }

    /// spirals outward by index amount from the current quantized player position
    /// this is used to generate chunks in a spiral pattern
    #[func]
    pub fn spiral(&self, index: i32) -> Vector3 {
        // get the current quantized player position
        let cur_center_chunk_coord = self.quantize_player_pos();

        let r = ((f64::sqrt(index as f64 + 1.0) - 1.0) / 2.0).floor() as i32 + 1;
        let p = (8 * r * (r - 1)) / 2;
        let en = r * 2;
        let a = (1 + index - p) % (r * 8);
        let mut pos: Vector3 = Vector3::new(0.0, 0.0, 0.0);

        match a / (r * 2) {
            0 => {
                pos.x = (a - r) as f32;
                pos.z = (-r) as f32;
            }
            1 => {
                pos.x = r as f32;
                pos.z = (a % en - r) as f32;
            }
            2 => {
                pos.x = (r - a % en) as f32;
                pos.z = r as f32;
            }
            3 => {
                pos.x = (-r) as f32;
                pos.z = (r - a % en) as f32;
            }
            _ => {}
        }

        // return the current spiral offset + the current quantized player position
        return cur_center_chunk_coord + pos;
    }

    fn hash_chunk_pos(&self, chunk_pos: Vector3) -> String {
        let hash_string = format!("chunk_{}x_{}y_{}z", chunk_pos.x, chunk_pos.y, chunk_pos.z);
        return hash_string;
    }

    #[func]
    fn update_chunks(&mut self) {
        // we have several things to set up in the ready function, one of which is
        // the thread that will spawn the chunks and run generation on them

        let mut cur_center_chunk_coord = self.quantize_player_pos();

        let chunk_scene: Gd<PackedScene> = load(self.chunk_scene.clone());

        let mut chunk_positions = Vec::new();

        // generate chunk positions
        for i in 0..self.render_distance {
            // init chunk position
            let new_pos = Vector3::new(
                cur_center_chunk_coord.x * (self.data_resolution - 2) as f32 * self.voxel_size,
                cur_center_chunk_coord.y * (self.data_resolution - 2) as f32 * self.voxel_size,
                cur_center_chunk_coord.z * (self.data_resolution - 2) as f32 * self.voxel_size,
            );

            chunk_positions.push(new_pos);

            // increment chunk coordinate
            cur_center_chunk_coord = self.spiral(i);
        }

        godot_print!("[GDVoxel - Data Generator] Generating data...");
        // generate chunk data
        let buffers = &threaded_generator::generate_chunk_data(
            self.voxel_size,
            self.data_resolution,
            self.seed,
            chunk_positions.clone(),
        );
        godot_print!("[GDVoxel - Data generator] Done!");

        // spawn chunks
        for i in 0..self.render_distance {
            let hashed_chunk_coord = self.hash_chunk_pos(chunk_positions[i as usize]);

            // make sure hash doesn't already exist in the scene as a child
            match self
                .base
                .find_child(format!("{}", hashed_chunk_coord).into())
            {
                Some(child) => {
                    // if it does, skip this chunk and make it visible
                    // cast to voxel chunk
                    match child.try_cast::<VoxelChunk>() {
                        Some(mut chunk) => {
                            chunk.set_visible(true);
                        }
                        None => {
                            // child not a voxel chunk, skip
                        }
                    }
                    continue;
                }
                None => {
                    // we haven't generated this chunk yet
                }
            }

            // generate chunk
            let mut chunk: Gd<VoxelChunk> = chunk_scene.instantiate_as::<VoxelChunk>();

            // set position
            chunk.set_position(chunk_positions[i as usize]);

            // set name
            chunk.set_name(hashed_chunk_coord.to_string().into());

            // print out name
            godot_print!("[GDVoxel] Chunk name: {}", chunk.get_name());

            // load from buffer
            chunk.bind_mut().load_from_buffer(&buffers[i as usize]);

            // add child
            self.base.add_child(chunk.upcast::<Node>());
        }

        // loop through all children and hide ones that aren't present in the chunk positions
        for child in self.base.get_children().iter_shared() {
            // cast to voxel chunk
            match child.try_cast::<VoxelChunk>() {
                Some(mut chunk) => {
                    // check if chunk is in chunk positions
                    if !chunk_positions.contains(&chunk.get_position()) {
                        // if not, hide it
                        chunk.set_visible(false);
                    }
                }
                None => {
                    // child not a voxel chunk, skip
                }
            }
        }
    }
}

#[godot_api]
impl Node3DVirtual for VoxelChunkManager {
    fn ready(&mut self) {
        self.update_chunks();
    }
}
