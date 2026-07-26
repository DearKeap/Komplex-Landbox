use super::chunk::{ChunkSection, CHUNK_SIZE, DIRT, GRASS, LEAF, PLANK, STONE};

/// Hash integer -> [0,1). Deterministic murni dari (x, z, seed) —
/// konsisten sama prinsip determinism yang dipegang di desain penuh
/// (input sama = output sama, gak ada state global).
fn hash2d(x: i32, z: i32, seed: u32) -> f32 {
    let mut h = (x as u32)
        .wrapping_mul(374761393)
        .wrapping_add((z as u32).wrapping_mul(668265263))
        .wrapping_add(seed.wrapping_mul(2654435761).wrapping_add(1));
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    h ^= h >> 16;
    (h & 0x00FF_FFFF) as f32 / 0x0100_0000 as f32
}

fn smoothstep(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// Value noise 2D — bukan OpenSimplex/Perlin, cuma interpolasi hash antar titik grid.
/// Cukup buat "jalan dulu"; multi-noise climate-based penuh nyusul kalau udah stabil.
fn value_noise_2d(x: f32, z: f32, seed: u32) -> f32 {
    let x0 = x.floor() as i32;
    let z0 = z.floor() as i32;
    let tx = smoothstep(x - x0 as f32);
    let tz = smoothstep(z - z0 as f32);

    let v00 = hash2d(x0, z0, seed);
    let v10 = hash2d(x0 + 1, z0, seed);
    let v01 = hash2d(x0, z0 + 1, seed);
    let v11 = hash2d(x0 + 1, z0 + 1, seed);

    let a = v00 + (v10 - v00) * tx;
    let b = v01 + (v11 - v01) * tx;
    a + (b - a) * tz
}

/// Tinggi terrain di kolom (world_x, world_z), dijaga dalam batas
/// CHUNK_SIZE (16) biar semua muat di satu section vertikal.
pub fn column_height(world_x: i32, world_z: i32, seed: u32) -> i32 {
    let base = value_noise_2d(world_x as f32 * 0.08, world_z as f32 * 0.08, seed);
    let detail = value_noise_2d(world_x as f32 * 0.25, world_z as f32 * 0.25, seed.wrapping_add(101));
    let h = base * 7.0 + detail * 2.0 + 4.0;
    h.clamp(2.0, (CHUNK_SIZE - 6) as f32) as i32
}

fn should_place_tree(world_x: i32, world_z: i32, seed: u32) -> bool {
    hash2d(world_x, world_z, seed.wrapping_add(777)) > 0.93
}

/// Generate satu ChunkSection di koordinat chunk (chunk_x, chunk_z).
/// Layering: stone di dalam, dirt 3 lapis di bawah permukaan, grass di atas.
pub fn generate_chunk(chunk_x: i32, chunk_z: i32, seed: u32) -> ChunkSection {
    let mut section = ChunkSection::new_empty();

    for local_x in 0..CHUNK_SIZE {
        for local_z in 0..CHUNK_SIZE {
            let world_x = chunk_x * CHUNK_SIZE as i32 + local_x as i32;
            let world_z = chunk_z * CHUNK_SIZE as i32 + local_z as i32;
            let height = column_height(world_x, world_z, seed);

            for y in 0..=height {
                let block = if y == height {
                    GRASS
                } else if y >= height - 3 {
                    DIRT
                } else {
                    STONE
                };
                section.set_block(local_x, y as usize, local_z, block);
            }

            if should_place_tree(world_x, world_z, seed) {
                place_tree(&mut section, local_x, (height + 1) as usize, local_z);
            }
        }
    }

    section
}

fn place_tree(section: &mut ChunkSection, x: usize, base_y: usize, z: usize) {
    for i in 0..3 {
        let y = base_y + i;
        if y < CHUNK_SIZE {
            section.set_block(x, y, z, PLANK);
        }
    }

    let top = base_y + 2;
    for dy in 0..2usize {
        for dx in -1i32..=1 {
            for dz in -1i32..=1 {
                if dx.abs() == 1 && dz.abs() == 1 && dy == 1 {
                    continue; // biar bentuknya gak kotak sempurna
                }
                let lx = x as i32 + dx;
                let lz = z as i32 + dz;
                let ly = top + dy;
                if lx < 0 || lz < 0 || lx >= CHUNK_SIZE as i32 || lz >= CHUNK_SIZE as i32 || ly >= CHUNK_SIZE {
                    continue;
                }
                section.set_block(lx as usize, ly, lz as usize, LEAF);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_same_seed_same_result() {
        let a = generate_chunk(0, 0, 42);
        let b = generate_chunk(0, 0, 42);
        for y in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    assert_eq!(a.get_block(x, y, z), b.get_block(x, y, z));
                }
            }
        }
    }

    #[test]
    fn different_seed_can_differ() {
        let a = generate_chunk(0, 0, 1);
        let b = generate_chunk(0, 0, 2);
        // gak assert HARUS beda di tiap voxel (bisa kebetulan sama),
        // cukup buktiin function-nya jalan tanpa panic buat dua seed beda
        let _ = (a.palette_len(), b.palette_len());
    }
}
