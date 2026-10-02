//! Arches et surplombs naturels (0.11, phase B3), posés sur le terrain en voxels 3D.
//!
//! Chaque face de la sphère-cube est découpée en cellules d'environ 60 voxels ; chaque cellule
//! est générée à la demande depuis sa graine (règle 12) : rien, une arche (l'érosion a percé une
//! paroi) ou une cheminée de fée coiffée d'un chapeau en auvent. Plus fréquentes dans les
//! montagnes, sur les falaises et les mesas, seulement là où le vent et l'eau sculptent (air).

use bevy::math::Vec3;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::caves::{Piece, Shape};
use crate::terrain::{dir_to_face, face_dir};

/// Côté d'une cellule, en voxels.
const CELL_VOXELS: f32 = 60.0;

/// Ce que le terrain dit d'une direction : rayon du sol (sans formes 3D), terre ferme, et à quel
/// point le relief y est sculpté (montagnes, falaises : 0 à 1).
pub type Ground<'a> = dyn Fn(Vec3) -> (f32, bool, f32) + 'a;

pub struct Rocks {
    seed: u32,
    v: f32,
    /// Cellules par côté de face.
    n: i64,
    cache: Mutex<HashMap<(u8, i64, i64), Arc<Vec<Shape>>>>,
    near: Mutex<Option<((u8, i64, i64), Arc<Vec<Piece>>)>>,
}

fn hash(seed: u32, face: u8, x: i64, y: i64) -> u64 {
    let mut h = (seed as u64) ^ ((face as u64) << 56);
    for v in [x, y] {
        h ^= (v as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        h ^= h >> 31;
        h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
        h ^= h >> 29;
    }
    h
}

struct Rng(u64);

impl Rng {
    fn unit(&mut self) -> f32 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        ((z ^ (z >> 31)) >> 40) as f32 / (1u64 << 24) as f32
    }

    fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.unit()
    }
}

impl Rocks {
    pub fn new(seed: u32, radius: f32, voxel: f32) -> Self {
        let n = ((std::f32::consts::FRAC_PI_2 * radius) / (CELL_VOXELS * voxel)).round().max(1.0) as i64;
        Self { seed: seed ^ 0xA2C4, v: voxel, n, cache: Mutex::new(HashMap::new()), near: Mutex::new(None) }
    }

    fn key_of(&self, dir: Vec3) -> (u8, i64, i64) {
        let (face, s, t) = dir_to_face(dir);
        let n = self.n as f32;
        let x = (((s + 1.0) * 0.5 * n).floor() as i64).clamp(0, self.n - 1);
        let y = (((t + 1.0) * 0.5 * n).floor() as i64).clamp(0, self.n - 1);
        (face, x, y)
    }

    /// Formes de la cellule (générées une fois, puis gardées en cache).
    fn cell(&self, key: (u8, i64, i64), ground: &Ground) -> Arc<Vec<Shape>> {
        if let Some(c) = self.cache.lock().ok().and_then(|c| c.get(&key).cloned()) {
            return c;
        }
        let shapes = Arc::new(self.build(key, ground));
        if let Ok(mut c) = self.cache.lock() {
            if c.len() > 8192 {
                c.clear();
            }
            c.insert(key, shapes.clone());
        }
        shapes
    }

    fn build(&self, (face, x, y): (u8, i64, i64), ground: &Ground) -> Vec<Shape> {
        let mut rng = Rng(hash(self.seed, face, x, y));
        let n = self.n as f32;
        // Un point au hasard dans la cellule, loin de ses bords
        let s = -1.0 + 2.0 * (x as f32 + rng.range(0.3, 0.7)) / n;
        let t = -1.0 + 2.0 * (y as f32 + rng.range(0.3, 0.7)) / n;
        let up = face_dir(face, s, t);
        let (g, land, sculpted) = ground(up);
        if !land || rng.unit() > 0.03 + 0.3 * sculpted {
            return Vec::new();
        }
        let v = self.v;
        let east = Vec3::Y.cross(up).normalize_or(Vec3::X);
        let north = up.cross(east);
        let a = rng.range(0.0, std::f32::consts::TAU);
        let along = east * a.cos() + north * a.sin();
        let mut out = Vec::new();
        if rng.unit() < 0.6 {
            // Arche : un demi-anneau de capsules, pieds enfoncés dans le sol
            let span = rng.range(14.0, 28.0) * v;
            let height = span * rng.range(0.5, 0.8);
            let thick = rng.range(1.6, 2.6) * v;
            let foot_a = (up * g + along * span * 0.5).normalize();
            let foot_b = (up * g - along * span * 0.5).normalize();
            let base = g.min(ground(foot_a).0).min(ground(foot_b).0) - 2.0 * v;
            let mid = up * base;
            let steps = 8;
            let point = |k: usize| {
                let ang = std::f32::consts::PI * k as f32 / steps as f32;
                mid + along * ang.cos() * span * 0.5 + up * (ang.sin() * (height + 2.0 * v))
            };
            for k in 0..steps {
                out.push(Shape::Capsule(point(k), point(k + 1), thick));
            }
        } else {
            // Cheminée de fée : un pilier coiffé d'un chapeau qui déborde en auvent
            let h = rng.range(6.0, 12.0) * v;
            let pillar = rng.range(1.5, 2.4) * v;
            let foot = up * (g - 2.0 * v);
            let top = up * (g + h);
            out.push(Shape::Capsule(foot, top, pillar));
            let reach = rng.range(6.0, 12.0) * v;
            out.push(Shape::Capsule(top - along * reach * 0.3, top + along * reach, rng.range(2.0, 3.0) * v));
        }
        out
    }

    /// Formes autour de `dir` (sa cellule et les 8 voisines de la même face).
    pub fn around(&self, dir: Vec3, ground: &Ground) -> Vec<Shape> {
        let (face, x, y) = self.key_of(dir);
        let mut out = Vec::new();
        for dy in -1..=1 {
            for dx in -1..=1 {
                let (cx, cy) = (x + dx, y + dy);
                if (0..self.n).contains(&cx) && (0..self.n).contains(&cy) {
                    out.extend(self.cell((face, cx, cy), ground).iter().copied());
                }
            }
        }
        out
    }

    /// Pièces autour de `dir`, gardées tant qu'on reste dans la même cellule (collisions).
    pub fn pieces_near(&self, dir: Vec3, ground: &Ground) -> Arc<Vec<Piece>> {
        let key = self.key_of(dir);
        if let Some((k, list)) = self.near.lock().ok().and_then(|n| n.clone()) {
            if k == key {
                return list;
            }
        }
        let list: Arc<Vec<Piece>> = Arc::new(self.around(dir, ground).into_iter().map(Piece::Add).collect());
        if let Ok(mut n) = self.near.lock() {
            *n = Some((key, list.clone()));
        }
        list
    }

    /// Pièces qui peuvent toucher une tuile (directions prises dans la tuile).
    pub fn for_tile(&self, dirs: &[Vec3], ground: &Ground) -> Vec<Piece> {
        let mut keys = std::collections::HashSet::new();
        for &d in dirs {
            let (face, x, y) = self.key_of(d);
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let (cx, cy) = (x + dx, y + dy);
                    if (0..self.n).contains(&cx) && (0..self.n).contains(&cy) {
                        keys.insert((face, cx, cy));
                    }
                }
            }
        }
        keys.into_iter().flat_map(|k| self.cell(k, ground).iter().copied().collect::<Vec<_>>()).map(Piece::Add).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Règle 12 : mêmes formes pour la même graine ; davantage là où le relief est sculpté ;
    /// rien sur la mer.
    #[test]
    fn arches_and_hoodoos_are_hashed_and_follow_the_relief() {
        let radius = 9_000.0;
        let flat = |_: Vec3| (radius, true, 0.0);
        let rugged = |_: Vec3| (radius, true, 1.0);
        let sea = |_: Vec3| (radius, false, 1.0);
        let a = Rocks::new(5, radius, 7.0);
        let b = Rocks::new(5, radius, 7.0);
        let count = |r: &Rocks, g: &Ground| -> usize { (0..r.n).map(|x| r.build((2, x, r.n / 2), g).len().min(1)).sum() };
        let (n_flat, n_rugged) = (count(&a, &flat), count(&Rocks::new(5, radius, 7.0), &rugged));
        assert!(n_rugged > n_flat * 3, "{n_flat} {n_rugged}");
        assert_eq!(count(&a, &sea), 0);
        for x in 0..a.n {
            assert_eq!(a.build((4, x, 3), &rugged).len(), b.build((4, x, 3), &rugged).len());
        }
    }
}
