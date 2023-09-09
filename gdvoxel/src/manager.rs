use std::collections::HashMap;

pub use crate::chunk::*;
use crate::{chunk, threaded_generator};
use godot::{
    engine::MeshInstance3D,
    prelude::{utilities::instance_from_id, *},
};

use priority_queue::PriorityQueue;

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

    threaded_generator: Option<threaded_generator::ThreadedGenerator>,

    queued_chunks: Vec<String>,
    generated_chunks: HashMap<String, i64>,
    to_remove: Vec<String>,

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

    pub fn on_chunk_added(&mut self, hash: String, chunk: &VoxelChunk) {
        // add chunk to chunks hashmap
        self.generated_chunks
            .insert(hash.to_string(), chunk.base.instance_id().to_i64());
        // remove from queued chunks
        self.queued_chunks.retain(|x| x != &hash.to_string());

        godot_print!(
            "[GDVoxel - Chunk Generator] Chunk added, {} chunks queued, {} chunks generated",
            self.queued_chunks.len(),
            self.generated_chunks.len()
        );

        // if there are no queued chunks, print a message
        if self.queued_chunks.len() == 0 {
            godot_print!("[GDVoxel - Chunk Generator] No chunks queued, generation complete");
        }
    }

    pub fn hash_chunk_pos(chunk_pos: Vector3) -> String {
        let hash_string = format!("chunk_{}_{}_{}", chunk_pos.x, chunk_pos.y, chunk_pos.z);
        return hash_string;
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

    #[func]
    fn update_chunks(&mut self) {
        // we have several things to set up in the ready function, one of which is
        // the thread that will spawn the chunks and run generation on them

        let mut cur_center_chunk_coord = self.quantize_player_pos();

        let mut chunk_positions = Vec::new();

        // generate chunk positions
        for i in 0..self.render_distance {
            // init chunk position
            let new_pos = Vector3::new(
                cur_center_chunk_coord.x * (self.data_resolution - 2) as f32 * self.voxel_size,
                0.0,
                cur_center_chunk_coord.z * (self.data_resolution - 2) as f32 * self.voxel_size,
            );

            // create hash
            let hash = VoxelChunkManager::hash_chunk_pos(cur_center_chunk_coord);

            // if chunk already exists, skip
            if self.generated_chunks.contains_key(&hash) {
                cur_center_chunk_coord = self.spiral(i);

                // check to make sure the chunk is_instance_valid
                let chunk_id = self.generated_chunks.get(&hash).unwrap();
                let chunk: Gd<VoxelChunk> = instance_from_id(*chunk_id).unwrap().cast();
                // if chunk is not valid, remove it
                if chunk.is_queued_for_deletion() {
                    self.generated_chunks.remove(&hash);
                } else {
                    continue;
                }
            }

            // if chunk is already queued, skip
            if self.queued_chunks.contains(&hash) {
                cur_center_chunk_coord = self.spiral(i);

                continue;
            }

            chunk_positions.push(new_pos);
            self.queued_chunks.push(hash);

            // increment chunk coordinate
            cur_center_chunk_coord = self.spiral(i);
        }

        godot_print!(
            "[GDVoxel - Chunk Generator] Generating chunks, {} new chunks queued, {} chunks generated",
            chunk_positions.len(),
            self.generated_chunks.len()
        );
        // generate chunk data
        let buffers = self
            .threaded_generator
            .as_mut()
            .unwrap()
            .generate_chunk_data(&chunk_positions);

        // // iterate through generated chunks
        // for chunk_id in self.generated_chunks.clone().values() {
        //     // get chunk
        //     let mut chunk: Gd<VoxelChunk> = match instance_from_id(*chunk_id) {
        //         Some(chunk) => chunk.cast(),
        //         None => {
        //             self.generated_chunks.remove(&chunk_id.to_string());
        //             continue;
        //         }
        //     };
        //     // get chunk pos
        //     let chunk_pos = chunk.get_position();

        //     // if it's too far away from player, queue it for removal
        //     let player_pos_xz = Vector3::new(self.player_pos.x, 0.0, self.player_pos.z);
        //     if chunk_pos.distance_to(player_pos_xz)
        //         > self.spiral(self.render_distance).distance_to(player_pos_xz)
        //     {
        //         // print
        //         godot_print!(
        //             "[GDVoxel - Chunk Generator] Chunk too far away, queueing for removal"
        //         );
        //         chunk.call_deferred("queue_free".into(), &[]);
        //     }
        // }
    }
}

#[godot_api]
impl Node3DVirtual for VoxelChunkManager {
    fn ready(&mut self) {
        self.threaded_generator = Some(threaded_generator::ThreadedGenerator::initialize(
            self.voxel_size,
            self.data_resolution,
            self.seed,
            self.base.instance_id_or_none().unwrap(),
        ));

        self.update_chunks();
    }
}
