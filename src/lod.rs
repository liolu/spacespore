use bevy::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LodLevel {
    Lod0,
    Lod1,
    Lod2,
    Lod3,
    Lod4,
    Lod5,
}

impl LodLevel {
    pub fn resolution(self) -> usize {
        match self {
            LodLevel::Lod0 => 20,
            LodLevel::Lod1 => 14,
            LodLevel::Lod2 => 10,
            LodLevel::Lod3 => 6,
            LodLevel::Lod4 => 4,
            LodLevel::Lod5 => 2,
        }
    }
}

#[derive(Component)]
pub struct LodChunk;

pub fn compute_lod_level(camera_pos: Vec3, chunk_center: Vec3, planet_radius: f32) -> LodLevel {
    let dist = camera_pos.distance(chunk_center);
    let ratio = dist / planet_radius;

    if ratio < 1.5 {
        LodLevel::Lod0
    } else if ratio < 2.5 {
        LodLevel::Lod1
    } else if ratio < 3.5 {
        LodLevel::Lod2
    } else if ratio < 5.0 {
        LodLevel::Lod3
    } else if ratio < 7.0 {
        LodLevel::Lod4
    } else {
        LodLevel::Lod5
    }
}
