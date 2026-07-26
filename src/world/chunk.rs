/// ID tipe block — nanti jadi registry penuh, buat sekarang cukup u16.
pub type BlockTypeId = u16;

pub const CHUNK_SIZE: usize = 16;
pub const VOXELS_PER_SECTION: usize = CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE; // 4096

/// Satu section 16^3. Palette-compressed:
/// - `palette` nyimpen daftar tipe block UNIK yang ada di section ini
/// - `indices` nyimpen INDEX ke palette per-voxel, bukan BlockTypeId langsung
///
/// Section kosong (cuma udara) cuma butuh palette 1 entry, indices semua 0 —
/// jauh lebih hemat dibanding array u16 mentah 4096 element.
pub struct ChunkSection {
    palette: Vec<BlockTypeId>,
    indices: Vec<u16>, // nanti bisa di-pack ke bits_per_index sesuai palette.len(),
                        // buat skeleton ini dulu pakai u16 flat biar simpel
}

pub const AIR: BlockTypeId = 0;
pub const GRASS: BlockTypeId = 1;
pub const DIRT: BlockTypeId = 2;
pub const STONE: BlockTypeId = 3;
pub const PLANK: BlockTypeId = 4;
pub const LEAF: BlockTypeId = 5;

/// Warna placeholder per block — belum ada texture, jadi tiap block
/// digambar warna flat dulu. Nanti ini diganti UV lookup ke texture array
/// sesuai desain (Axel nambahin texture asli belakangan).
pub fn block_color(block: BlockTypeId) -> [f32; 3] {
    match block {
        GRASS => [0.36, 0.68, 0.24],
        DIRT => [0.50, 0.36, 0.22],
        STONE => [0.55, 0.55, 0.57],
        PLANK => [0.66, 0.49, 0.28],
        LEAF => [0.16, 0.47, 0.16],
        _ => [1.0, 0.0, 1.0], // magenta — gampang keliatan kalau ada block gak dikenal
    }
}

pub fn is_solid(block: BlockTypeId) -> bool {
    block != AIR
}

impl ChunkSection {
    pub fn new_empty() -> Self {
        Self {
            palette: vec![AIR],
            indices: vec![0; VOXELS_PER_SECTION],
        }
    }

    fn voxel_index(x: usize, y: usize, z: usize) -> usize {
        debug_assert!(x < CHUNK_SIZE && y < CHUNK_SIZE && z < CHUNK_SIZE);
        (y * CHUNK_SIZE + z) * CHUNK_SIZE + x
    }

    pub fn get_block(&self, x: usize, y: usize, z: usize) -> BlockTypeId {
        let palette_idx = self.indices[Self::voxel_index(x, y, z)] as usize;
        self.palette[palette_idx]
    }

    pub fn set_block(&mut self, x: usize, y: usize, z: usize, block: BlockTypeId) {
        // Cari block di palette; kalau belum ada, tambahin (palette tumbuh dinamis)
        let palette_idx = match self.palette.iter().position(|&b| b == block) {
            Some(idx) => idx,
            None => {
                self.palette.push(block);
                self.palette.len() - 1
            }
        };
        let voxel_idx = Self::voxel_index(x, y, z);
        self.indices[voxel_idx] = palette_idx as u16;
    }

    pub fn palette_len(&self) -> usize {
        self.palette.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_section_is_all_air() {
        let section = ChunkSection::new_empty();
        assert_eq!(section.get_block(0, 0, 0), AIR);
        assert_eq!(section.palette_len(), 1);
    }

    #[test]
    fn set_and_get_block() {
        let mut section = ChunkSection::new_empty();
        section.set_block(1, 2, 3, STONE);
        assert_eq!(section.get_block(1, 2, 3), STONE);
        assert_eq!(section.get_block(0, 0, 0), AIR); // sisanya tetap udara
        assert_eq!(section.palette_len(), 2); // AIR + stone
    }

    #[test]
    fn unknown_block_has_magenta_fallback() {
        assert_eq!(block_color(99), [1.0, 0.0, 1.0]);
    }
}
