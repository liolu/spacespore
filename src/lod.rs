use bevy::prelude::*;
use std::sync::atomic::{AtomicU32, Ordering};

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

/// Multiplicateur de détail des planètes (réglage « Détail planètes »), stocké en bits f32.
/// > 1 : les niveaux fins restent actifs plus loin ; < 1 : maillages plus grossiers.
static LOD_QUALITY: AtomicU32 = AtomicU32::new(0x3F80_0000); // 1.0

pub fn set_lod_quality(q: f32) {
    LOD_QUALITY.store(q.clamp(0.25, 4.0).to_bits(), Ordering::Relaxed);
}

pub fn compute_lod_level(camera_pos: Vec3, chunk_center: Vec3, planet_radius: f32) -> LodLevel {
    let dist = camera_pos.distance(chunk_center);
    let quality = f32::from_bits(LOD_QUALITY.load(Ordering::Relaxed));
    let ratio = dist / planet_radius / quality;

    if ratio < 3.0 {
        LodLevel::Lod0
    } else if ratio < 5.0 {
        LodLevel::Lod1
    } else if ratio < 7.0 {
        LodLevel::Lod2
    } else if ratio < 10.0 {
        LodLevel::Lod3
    } else if ratio < 14.0 {
        LodLevel::Lod4
    } else {
        LodLevel::Lod5
    }
}
