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
    pub fn quantize_pos(&self, vec: Vector3) -> Vector3 {
        return Vector3::new(
            (vec.x / (self.data_resolution as f32 * self.voxel_size)).floor(),
            0.0,
            (vec.z / (self.data_resolution as f32 * self.voxel_size)).floor(),
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

    #[func]
    fn update_chunks(&mut self) {
        // we have several things to set up in the ready function, one of which is
        // the thread that will spawn the chunks and run generation on them

        let cur_center_chunk_coord = self.quantize_pos(self.player_pos);

        let mut priority_queue: PriorityQueue<[i32; 3], i32> = PriorityQueue::new();

        for x in -self.render_distance..self.render_distance {
            for z in -self.render_distance..self.render_distance {
                let new_pos = Vector3::new(
                    ((x + cur_center_chunk_coord.x as i32) as f32
                        * (self.data_resolution as i32 - 2) as f32
                        * self.voxel_size),
                    0.0,
                    ((z + cur_center_chunk_coord.z as i32) as f32
                        * (self.data_resolution as i32 - 2) as f32
                        * self.voxel_size),
                );

                let hash = VoxelChunkManager::hash_chunk_pos(new_pos);

                // if chunk already exists, skip
                if self.generated_chunks.contains_key(&hash) {
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
                    continue;
                }

                let priority = new_pos.distance_to(cur_center_chunk_coord);
                priority_queue.push(
                    [new_pos.x as i32, new_pos.y as i32, new_pos.z as i32],
                    priority as i32,
                );
                self.queued_chunks.push(hash);
            }
        }

        let chunk_positions: Vec<Vector3> = priority_queue
            .into_sorted_vec()
            .iter()
            .rev()
            .map(|pos| {
                // get vector from array
                let chunk_pos = Vector3::new(pos[0] as f32, pos[1] as f32, pos[2] as f32);
                return chunk_pos;
            })
            .collect();

        godot_print!(
            "[GDVoxel - Chunk Generator] Generating chunks, {} new chunks queued, {} chunks generated",
            chunk_positions.len(),
            self.generated_chunks.len()
        );
        // generate chunk data
        self.threaded_generator
            .as_mut()
            .unwrap()
            .generate_chunk_data(&chunk_positions);

        // iterate through generated chunks
        for (hash, chunk_id) in self.generated_chunks.clone().iter() {
            // make sure it's not in the queue
            if self.queued_chunks.contains(hash) {
                continue;
            }

            // get chunk
            let mut chunk: Gd<VoxelChunk> = match instance_from_id(*chunk_id) {
                Some(chunk) => chunk.cast(),
                None => {
                    self.generated_chunks.remove(&chunk_id.to_string());
                    continue;
                }
            };
            // get chunk pos
            let mut chunk_pos = chunk.get_position();
            chunk_pos.y = 0.0;

            // if it's too far away from player, queue it for removal
            if chunk_pos
                .distance_to(cur_center_chunk_coord * self.voxel_size * self.data_resolution as f32)
                > (self.render_distance * self.data_resolution as i32) as f32 * 2.0
            {
                // print
                godot_print!(
                    "[GDVoxel - Chunk Generator] Chunk too far away, queueing for removal"
                );
                chunk.call_deferred("queue_free".into(), &[]);
                // remove from generated chunks
                self.generated_chunks.remove(hash);
            }
        }
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
