pub mod chunk;
pub mod worldgen;

use chunk::{is_solid, ChunkSection, CHUNK_SIZE};
use std::collections::HashMap;

/// World storage — holds raw ChunkSection data.
pub struct World {
    chunks: HashMap<(i32, i32), ChunkSection>,
    pub seed: u32,
    pub chunks_x: i32,
    pub chunks_z: i32,
}

impl World {
    pub fn generate(chunks_x: i32, chunks_z: i32, seed: u32) -> Self {
        let mut chunks = HashMap::new();
        for cx in 0..chunks_x {
            for cz in 0..chunks_z {
                chunks.insert((cx, cz), worldgen::generate_chunk(cx, cz, seed));
            }
        }
        Self {
            chunks,
            seed,
            chunks_x,
            chunks_z,
        }
    }

    pub fn iter_chunks(&self) -> impl Iterator<Item = (&(i32, i32), &ChunkSection)> {
        self.chunks.iter()
    }

    pub fn is_solid_at(&self, world_x: i32, world_y: i32, world_z: i32) -> bool {
        if world_y < 0 || world_y >= CHUNK_SIZE as i32 {
            return false;
        }
        let cx = world_x.div_euclid(CHUNK_SIZE as i32);
        let cz = world_z.div_euclid(CHUNK_SIZE as i32);
        let lx = world_x.rem_euclid(CHUNK_SIZE as i32) as usize;
        let lz = world_z.rem_euclid(CHUNK_SIZE as i32) as usize;

        match self.chunks.get(&(cx, cz)) {
            Some(section) => is_solid(section.get_block(lx, world_y as usize, lz)),
            None => false,
        }
    }
}
