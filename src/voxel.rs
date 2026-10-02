//! Voxels 3D (0.11, phase B1) : coordonnées des cellules et des blocs, et format des deltas
//! (règle 11 : voxel = densité(graine, point fixe de l'astre) + delta).
//!
//! Une cellule est repérée au niveau le plus fin du quadtree de `terrain.rs` : face de la
//! sphère-cube, colonne (`i`, `j`) et couche radiale `k` (épaisseur d'un voxel, 0 = niveau de la
//! mer). Un bloc regroupe 32 × 32 colonnes × 32 couches. Rien n'est encore creusé (minage en 0.14),
//! mais les deltas ont déjà leur format, leur place dans `world.json` et leur message réseau.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::planet::VoxelType;

/// Côté d'un bloc (cellules).
pub const BLOCK: i64 = 32;

/// Une cellule voxel (niveau le plus fin).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Cell {
    pub face: u8,
    pub i: i64,
    pub j: i64,
    pub k: i32,
}

/// Un bloc de 32³ cellules.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlockKey {
    pub face: u8,
    pub x: i64,
    pub y: i64,
    pub z: i32,
}

impl Cell {
    /// Bloc de la cellule et son rang dans le bloc (i + 32 j + 1024 k).
    pub fn block(self) -> (BlockKey, u16) {
        let (bx, lx) = (self.i.div_euclid(BLOCK), self.i.rem_euclid(BLOCK));
        let (by, ly) = (self.j.div_euclid(BLOCK), self.j.rem_euclid(BLOCK));
        let (bz, lz) = ((self.k as i64).div_euclid(BLOCK), (self.k as i64).rem_euclid(BLOCK));
        (BlockKey { face: self.face, x: bx, y: by, z: bz as i32 }, (lx + BLOCK * ly + BLOCK * BLOCK * lz) as u16)
    }
}

impl BlockKey {
    /// Clé texte (JSON) : « face.x.y.z ».
    pub fn text(self) -> String {
        format!("{}.{}.{}.{}", self.face, self.x, self.y, self.z)
    }

    pub fn parse(s: &str) -> Option<Self> {
        let mut it = s.split('.');
        let key = Self { face: it.next()?.parse().ok()?, x: it.next()?.parse().ok()?, y: it.next()?.parse().ok()?, z: it.next()?.parse().ok()? };
        (it.next().is_none() && key.face < 6).then_some(key)
    }

    /// Cellule de rang `index` dans le bloc.
    pub fn cell(self, index: u16) -> Cell {
        let n = index as i64;
        Cell {
            face: self.face,
            i: self.x * BLOCK + n % BLOCK,
            j: self.y * BLOCK + (n / BLOCK) % BLOCK,
            k: (self.z as i64 * BLOCK + n / (BLOCK * BLOCK)) as i32,
        }
    }
}

/// Matières, dans un ordre figé (codes des deltas : ne jamais réordonner, seulement ajouter).
const CODES: [VoxelType; 25] = [
    VoxelType::Air,
    VoxelType::Water,
    VoxelType::Sand,
    VoxelType::Grass,
    VoxelType::Stone,
    VoxelType::Snow,
    VoxelType::Ice,
    VoxelType::Methane,
    VoxelType::Ammonia,
    VoxelType::Lava,
    VoxelType::Tundra,
    VoxelType::Taiga,
    VoxelType::Forest,
    VoxelType::Steppe,
    VoxelType::Savanna,
    VoxelType::Jungle,
    VoxelType::Swamp,
    VoxelType::Basalt,
    VoxelType::Salt,
    VoxelType::Rust,
    VoxelType::Crystal,
    VoxelType::Spore,
    VoxelType::Fungus,
    VoxelType::Glass,
    VoxelType::Sulfur,
];

pub fn voxel_code(v: VoxelType) -> u8 {
    CODES.iter().position(|c| *c == v).unwrap_or(0) as u8
}

pub fn voxel_from_code(code: u8) -> VoxelType {
    CODES.get(code as usize).copied().unwrap_or(VoxelType::Air)
}

/// Cellules modifiées d'un bloc : rang → matière posée (code ; 0 = air, cellule creusée).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct BlockDelta {
    pub cells: BTreeMap<u16, u8>,
}

/// Deltas voxel d'un astre, par bloc (« face.x.y.z »).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct BodyVoxels {
    pub blocks: BTreeMap<String, BlockDelta>,
}

impl BodyVoxels {
    /// Matière posée (ou air creusé) dans cette cellule, si elle a été modifiée.
    pub fn get(&self, cell: Cell) -> Option<VoxelType> {
        let (block, index) = cell.block();
        self.blocks.get(&block.text())?.cells.get(&index).map(|c| voxel_from_code(*c))
    }

    pub fn set(&mut self, cell: Cell, v: VoxelType) {
        let (block, index) = cell.block();
        self.blocks.entry(block.text()).or_default().cells.insert(index, voxel_code(v));
    }

    /// Le bloc contient des modifications.
    pub fn touches(&self, block: BlockKey) -> bool {
        self.blocks.get(&block.text()).is_some_and(|b| !b.cells.is_empty())
    }

    /// Couches (k) modifiées dans les blocs de la colonne de blocs (face, x, y).
    pub fn layers_in(&self, face: u8, x: i64, y: i64) -> Option<(i32, i32)> {
        let mut range: Option<(i32, i32)> = None;
        for (text, delta) in &self.blocks {
            let Some(key) = BlockKey::parse(text) else { continue };
            if key.face != face || key.x != x || key.y != y {
                continue;
            }
            for &index in delta.cells.keys() {
                let k = key.cell(index).k;
                range = Some(range.map_or((k, k), |(a, b)| (a.min(k), b.max(k))));
            }
        }
        range
    }
}

/// Deltas voxel de tous les astres, par clé d'astre (`BodyId::key`) : sauvés dans `world.json`.
pub type VoxelDeltas = BTreeMap<String, BodyVoxels>;

/// Deltas voxel de l'astre `kind` (planète ou lune), s'il en a.
pub fn body_voxels(settings: &crate::settings::GameSettings, kind: &crate::ui::TargetKind) -> Option<std::sync::Arc<BodyVoxels>> {
    let key = crate::chat_cmd::target_body(kind, false)?.key();
    settings.voxel_deltas.get(&key).filter(|v| !v.blocks.is_empty()).cloned().map(std::sync::Arc::new)
}

/// Message réseau des modifications d'un bloc (minage, 0.14) : l'astre, le bloc et ses cellules.
// Envoyé et appliqué par le minage (0.14) : seul le format existe pour l'instant.
#[allow(dead_code)]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct VoxelEdit {
    pub body: String,
    pub block: String,
    pub cells: Vec<(u16, u8)>,
}

#[allow(dead_code)]
impl VoxelEdit {
    /// Applique le message aux deltas du monde.
    pub fn apply(&self, deltas: &mut VoxelDeltas) -> bool {
        if BlockKey::parse(&self.block).is_none() {
            return false;
        }
        let block = deltas.entry(self.body.clone()).or_default().blocks.entry(self.block.clone()).or_default();
        for &(index, code) in &self.cells {
            if (index as i64) < BLOCK * BLOCK * BLOCK {
                block.cells.insert(index, code);
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_and_blocks_round_trip() {
        for cell in [
            Cell { face: 0, i: 0, j: 0, k: 0 },
            Cell { face: 3, i: 4095, j: 77, k: -1 },
            Cell { face: 5, i: 31, j: 32, k: -200 },
            Cell { face: 2, i: 1000, j: 2000, k: 63 },
        ] {
            let (block, index) = cell.block();
            assert_eq!(block.cell(index), cell);
            assert_eq!(BlockKey::parse(&block.text()), Some(block));
        }
        assert_eq!(BlockKey::parse("7.0.0.0"), None);
        assert_eq!(BlockKey::parse("1.2.3"), None);
    }

    #[test]
    fn deltas_survive_the_save_and_the_network() {
        let mut body = BodyVoxels::default();
        let dug = Cell { face: 2, i: 1234, j: 567, k: 3 };
        let placed = Cell { face: 2, i: 1235, j: 567, k: 4 };
        body.set(dug, VoxelType::Air);
        body.set(placed, VoxelType::Crystal);
        assert_eq!(body.get(dug), Some(VoxelType::Air));
        assert_eq!(body.get(placed), Some(VoxelType::Crystal));
        assert_eq!(body.get(Cell { k: 5, ..placed }), None);
        assert_eq!(body.layers_in(2, 38, 17), Some((3, 4)));

        // world.json
        let mut world = VoxelDeltas::new();
        world.insert("p:12:3".into(), body.clone());
        let json = serde_json::to_string(&world).unwrap();
        let back: VoxelDeltas = serde_json::from_str(&json).unwrap();
        assert_eq!(back, world);

        // Réseau : un message par bloc, petit
        let (block, index) = dug.block();
        let msg = VoxelEdit { body: "p:12:3".into(), block: block.text(), cells: vec![(index, voxel_code(VoxelType::Air))] };
        let text = serde_json::to_string(&msg).unwrap();
        assert!(text.len() < 80, "{text}");
        let mut other = VoxelDeltas::new();
        assert!(serde_json::from_str::<VoxelEdit>(&text).unwrap().apply(&mut other));
        assert_eq!(other["p:12:3"].get(dug), Some(VoxelType::Air));
    }

    #[test]
    fn material_codes_are_stable() {
        assert_eq!(voxel_code(VoxelType::Air), 0);
        assert_eq!(voxel_code(VoxelType::Stone), 4);
        assert_eq!(voxel_code(VoxelType::Sulfur), 24);
        for code in 0..25u8 {
            assert_eq!(voxel_code(voxel_from_code(code)), code);
        }
    }
}
