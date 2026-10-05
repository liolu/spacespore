//! Formes 3D du relief (0.11 B3, 0.13 T2), posées sur le terrain en voxels 3D ou creusées dedans.
//!
//! Chaque face de la sphère-cube est découpée en cellules d'environ 60 voxels ; chaque cellule
//! est générée à la demande depuis sa graine (règle 12). Sur les falaises (pente > 45°) :
//! corniches en surplomb, entrées de grottes, strates creusées ; dans les montagnes : pitons ;
//! au pied des pentes : chaos de blocs. Là où le vent et l'eau sculptent (air) : gorges étroites
//! et ponts naturels, arches, cheminées de fée coiffées d'un chapeau en auvent.

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

/// Les formes du relief (pour `/relief` : aller voir l'une d'elles).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Feature {
    Ledge,
    CliffCave,
    Strata,
    Pinnacle,
    Boulders,
    Gorge,
    Bridge,
    Arch,
    Hoodoo,
}

impl Feature {
    pub const ALL: [Feature; 9] = [Feature::Ledge, Feature::CliffCave, Feature::Strata, Feature::Pinnacle, Feature::Boulders, Feature::Gorge, Feature::Bridge, Feature::Arch, Feature::Hoodoo];

    pub fn name(self) -> &'static str {
        match self {
            Feature::Ledge => "corniche",
            Feature::CliffCave => "grotte",
            Feature::Strata => "strates",
            Feature::Pinnacle => "piton",
            Feature::Boulders => "blocs",
            Feature::Gorge => "gorge",
            Feature::Bridge => "pont",
            Feature::Arch => "arche",
            Feature::Hoodoo => "cheminee",
        }
    }

    pub fn parse(s: &str) -> Option<Feature> {
        let s = s.trim().to_lowercase().replace('é', "e");
        Self::ALL.into_iter().find(|f| s.starts_with(f.name()) || (!s.is_empty() && f.name().starts_with(&s)))
    }
}

/// Une cellule : ses pièces, et ses formes repérées (sorte, point du sol d'où la voir, point visé).
pub struct CellForms {
    pub pieces: Vec<Piece>,
    pub marks: Vec<(Feature, Vec3, Vec3)>,
}

pub struct Rocks {
    seed: u32,
    v: f32,
    /// Rayon de l'astre (niveau de la mer).
    radius: f32,
    /// Vent et eau (arches, gorges, grottes des falaises).
    air: bool,
    /// Eau liquide : les gorges et grottes sous le niveau de la mer sont noyées.
    wet: bool,
    /// Cellules par côté de face.
    n: i64,
    cache: Mutex<HashMap<(u8, i64, i64), Arc<CellForms>>>,
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

/// Deux directions tangentes (est, nord) en `up`.
fn tangents(up: Vec3) -> (Vec3, Vec3) {
    let east = Vec3::Y.cross(up).normalize_or(Vec3::X);
    (east, up.cross(east))
}

impl Rocks {
    pub fn new(seed: u32, radius: f32, voxel: f32, air: bool, wet: bool) -> Self {
        let n = ((std::f32::consts::FRAC_PI_2 * radius) / (CELL_VOXELS * voxel)).round().max(1.0) as i64;
        Self { seed: seed ^ 0xA2C4, v: voxel, radius, air, wet, n, cache: Mutex::new(HashMap::new()), near: Mutex::new(None) }
    }

    fn key_of(&self, dir: Vec3) -> (u8, i64, i64) {
        let (face, s, t) = dir_to_face(dir);
        let n = self.n as f32;
        let x = (((s + 1.0) * 0.5 * n).floor() as i64).clamp(0, self.n - 1);
        let y = (((t + 1.0) * 0.5 * n).floor() as i64).clamp(0, self.n - 1);
        (face, x, y)
    }

    /// Formes de la cellule (générées une fois, puis gardées en cache).
    fn cell(&self, key: (u8, i64, i64), ground: &Ground) -> Arc<CellForms> {
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

    /// Pente du sol (tangente) en `up` et direction de la descente (horizontale).
    fn slope(&self, up: Vec3, g: f32, ground: &Ground) -> (f32, Vec3) {
        let (east, north) = tangents(up);
        let d = 3.0 * self.v;
        let h = |t: Vec3| ground((up * g + t * d).normalize()).0;
        let gx = (h(east) - h(-east)) / (2.0 * d);
        let gy = (h(north) - h(-north)) / (2.0 * d);
        let down = -(east * gx + north * gy);
        (gx.hypot(gy), down.normalize_or(east))
    }

    /// Un point au hasard dans la cellule, loin de ses bords.
    fn pick(&self, rng: &mut Rng, face: u8, x: i64, y: i64) -> Vec3 {
        let n = self.n as f32;
        let s = -1.0 + 2.0 * (x as f32 + rng.range(0.3, 0.7)) / n;
        let t = -1.0 + 2.0 * (y as f32 + rng.range(0.3, 0.7)) / n;
        face_dir(face, s, t)
    }

    fn build(&self, key: (u8, i64, i64), ground: &Ground) -> CellForms {
        let mut cell = CellForms { pieces: Vec::new(), marks: Vec::new() };
        self.build_into(key, ground, &mut cell);
        cell
    }

    fn build_into(&self, (face, x, y): (u8, i64, i64), ground: &Ground, cell: &mut CellForms) {
        let mut rng = Rng(hash(self.seed, face, x, y));
        let v = self.v;
        let CellForms { pieces: out, marks } = cell;
        // La paroi la plus raide parmi trois points de la cellule
        let mut wall: Option<(f32, Vec3, f32, Vec3, f32)> = None;
        for _ in 0..3 {
            let up = self.pick(&mut rng, face, x, y);
            let (g, land, sculpted) = ground(up);
            if !land {
                continue;
            }
            let (s, down) = self.slope(up, g, ground);
            if wall.is_none_or(|w| s > w.0) {
                wall = Some((s, up, g, down, sculpted));
            }
        }
        let Some((slope, up, g, down, sculpted)) = wall else { return };
        let along = up.cross(down).normalize_or(tangents(up).0);
        let water = if self.wet { self.radius } else { 0.0 };

        // Falaises (pente > 45°) : corniche en surplomb, grotte, strates creusées
        if slope > 1.0 {
            let w = up * g;
            let roll = rng.unit();
            if roll < 0.55 {
                // Corniche : une dalle qui déborde de la paroi, la roche creusée dessous
                let reach = rng.range(3.0, 8.0) * v;
                let th = rng.range(1.5, 3.0) * v;
                let len = rng.range(10.0, 30.0) * v;
                out.push(Piece::Add(Shape::Slab { c: w + down * (reach * 0.5 - 0.5 * v) - up * th * 0.5, axes: [down, along, up], half: Vec3::new(reach * 0.5 + 0.5 * v, len * 0.5, th * 0.5) }));
                let nh = rng.range(3.0, 5.0) * v;
                let depth = rng.range(3.0, 6.0) * v;
                let out_x = (th + nh) / slope + v;
                out.push(Piece::Carve(Shape::Slab { c: w + down * (out_x - depth) * 0.5 - up * (th + nh * 0.5), axes: [down, along, up], half: Vec3::new((out_x + depth) * 0.5, len * 0.45, nh * 0.5) }, water));
                marks.push((Feature::Ledge, w + down * (reach + 6.0 * v), w - up * th));
            } else if roll < 0.8 && self.air {
                // Entrée de grotte dans la paroi : un boyau qui finit en petite salle
                let hgt = rng.range(4.0, 8.0) * v;
                let r = rng.range(2.5, 4.0) * v;
                let deep = rng.range(10.0, 25.0) * v;
                let mouth = w - up * hgt + down * (hgt / slope + r + 2.0 * v);
                let end = w - up * hgt - down * deep;
                out.push(Piece::Carve(Shape::Capsule(mouth, end, r), water));
                out.push(Piece::Carve(Shape::Sphere(end - down * r, rng.range(4.0, 7.0) * v), water));
                marks.push((Feature::CliffCave, mouth + down * 4.0 * v, mouth));
            }
            // Strates : rainures horizontales le long de la paroi
            if rng.unit() < 0.5 {
                let count = 2 + (rng.unit() * 3.0) as usize;
                let len = rng.range(20.0, 40.0) * v;
                let mut hgt = rng.range(-10.0, -4.0) * v;
                for _ in 0..count {
                    let x0 = -hgt / slope;
                    let c = w + up * hgt + down * (x0 - 0.25 * v);
                    out.push(Piece::Carve(Shape::Slab { c, axes: [down, along, up], half: Vec3::new(1.75 * v, len * 0.5, 0.55 * v) }, water));
                    hgt += rng.range(3.0, 5.0) * v;
                }
                marks.push((Feature::Strata, w + down * 12.0 * v, w));
            }
        }

        // Pitons : aiguilles de roche dans les montagnes
        if rng.unit() < 0.25 * (sculpted - 0.35).max(0.0) {
            let p = self.pick(&mut rng, face, x, y);
            let (gp, land, _) = ground(p);
            if land {
                let height = rng.range(20.0, 60.0) * v;
                let rb = rng.range(3.0, 6.0) * v;
                let rt = rng.range(0.8, 1.5) * v;
                let a = rng.range(0.0, std::f32::consts::TAU);
                let (e, nn) = tangents(p);
                let lean = (e * a.cos() + nn * a.sin()) * rng.range(0.0, 0.15);
                let steps = 4;
                let at = |k: usize| {
                    let f = k as f32 / steps as f32;
                    p * (gp - 3.0 * v + (height + 3.0 * v) * f) + lean * height * f * f
                };
                for k in 0..steps {
                    let r = rb + (rt - rb) * (k as f32 + 0.5) / steps as f32;
                    out.push(Piece::Add(Shape::Capsule(at(k), at(k + 1), r)));
                }
                marks.push((Feature::Pinnacle, p * gp + e * (rb + 35.0 * v), at(1)));
            }
        }

        // Chaos de blocs : éboulis au pied des pentes, blocs épars sur les mondes sans air
        let boulders = 0.04 + 0.25 * sculpted + if slope > 0.6 { 0.3 } else { 0.0 } + if self.air { 0.0 } else { 0.1 };
        if rng.unit() < boulders {
            let foot = if slope > 0.6 { (up * g + down * 15.0 * v).normalize() } else { up };
            let (ef, nf) = tangents(foot);
            let gf = ground(foot).0;
            let count = 5 + (rng.unit() * 6.0) as usize;
            for _ in 0..count {
                let a = rng.range(0.0, std::f32::consts::TAU);
                let d = rng.range(0.0, 10.0) * v;
                let dir = (foot * gf + (ef * a.cos() + nf * a.sin()) * d).normalize();
                let (gb, land, _) = ground(dir);
                if land {
                    let r = rng.range(1.2, 4.0) * v;
                    out.push(Piece::Add(Shape::Sphere(dir * (gb + 0.2 * r), r)));
                }
            }
            marks.push((Feature::Boulders, foot * gf + ef * 14.0 * v, foot * gf));
        }

        // Le reste demande du vent et de l'eau
        if !self.air {
            return;
        }

        // Gorge étroite (parfois franchie par un pont naturel), là où le sol est peu pentu
        if slope < 0.5 && rng.unit() < (0.03 + 0.12 * sculpted) * if self.wet { 1.0 } else { 0.5 } {
            let width = rng.range(3.0, 6.0) * v;
            let depth = rng.range(15.0, 40.0) * v;
            let seg = rng.range(15.0, 22.0) * v;
            let mut heading = rng.range(0.0, std::f32::consts::TAU);
            let (e, nn) = tangents(up);
            let mut pos = up * g - (e * heading.cos() + nn * heading.sin()) * seg * 2.0;
            let bridge = (rng.unit() < 0.4).then(|| 1 + (rng.unit() * 2.0) as usize);
            for k in 0..4 {
                heading += rng.range(-0.45, 0.45);
                let a_dir = pos.normalize();
                let (e, nn) = tangents(a_dir);
                let fwd = e * heading.cos() + nn * heading.sin();
                let b = pos + fwd * seg;
                let (ga, gb) = (ground(a_dir).0, ground(b.normalize()).0);
                let mid = (a_dir + b.normalize()).normalize();
                let bottom = ga.min(gb) - depth;
                let top = ga.max(gb) + 2.0 * v;
                let fwd = (fwd - mid * fwd.dot(mid)).normalize_or(e);
                let side = mid.cross(fwd);
                out.push(Piece::Carve(Shape::Slab { c: mid * (bottom + top) * 0.5, axes: [side, fwd, mid], half: Vec3::new(width * 0.5, seg * 0.5 + width * 0.5, (top - bottom) * 0.5) }, water));
                if bridge == Some(k) {
                    // Pont naturel : une bande de roche laissée au ras des deux bords
                    let edge = width * 0.5 + 2.0 * v;
                    let gm = ground((mid * ga + side * edge).normalize()).0.min(ground((mid * ga - side * edge).normalize()).0);
                    let th = rng.range(3.0, 5.0) * v;
                    out.push(Piece::Add(Shape::Slab { c: mid * (gm - th * 0.5), axes: [side, fwd, mid], half: Vec3::new(edge + v, rng.range(2.0, 3.0) * v, th * 0.5) }));
                    marks.push((Feature::Bridge, mid * gm + fwd * 8.0 * v, mid * (gm - th * 0.5)));
                }
                if k == 1 {
                    marks.push((Feature::Gorge, mid * ga.max(gb) + side * (width * 0.5 + 3.0 * v), mid * (bottom + 3.0 * v)));
                }
                pos = b;
            }
        }

        // Arches et cheminées de fée (érosion) : plus fréquentes là où le relief est sculpté
        if rng.unit() < 0.03 + 0.3 * sculpted {
            let p = self.pick(&mut rng, face, x, y);
            let (gp, land, _) = ground(p);
            if !land {
                return;
            }
            let (e, nn) = tangents(p);
            let a = rng.range(0.0, std::f32::consts::TAU);
            let along = e * a.cos() + nn * a.sin();
            if rng.unit() < 0.6 {
                // Arche : un demi-anneau de capsules, pieds enfoncés dans le sol
                let span = rng.range(14.0, 40.0) * v;
                let height = span * rng.range(0.5, 0.8);
                let thick = rng.range(1.6, 2.6) * v * (span / (20.0 * v)).max(1.0).sqrt();
                let foot_a = (p * gp + along * span * 0.5).normalize();
                let foot_b = (p * gp - along * span * 0.5).normalize();
                let base = gp.min(ground(foot_a).0).min(ground(foot_b).0) - 2.0 * v;
                let mid = p * base;
                let steps = 8;
                let point = |k: usize| {
                    let ang = std::f32::consts::PI * k as f32 / steps as f32;
                    mid + along * ang.cos() * span * 0.5 + p * (ang.sin() * (height + 2.0 * v))
                };
                for k in 0..steps {
                    out.push(Piece::Add(Shape::Capsule(point(k), point(k + 1), thick)));
                }
                marks.push((Feature::Arch, p * gp + (along.cross(p)) * span * 0.7, point(4)));
            } else {
                // Cheminée de fée : un pilier coiffé d'un chapeau qui déborde en auvent
                let h = rng.range(6.0, 12.0) * v;
                let pillar = rng.range(1.5, 2.4) * v;
                let foot = p * (gp - 2.0 * v);
                let top = p * (gp + h);
                out.push(Piece::Add(Shape::Capsule(foot, top, pillar)));
                let reach = rng.range(6.0, 12.0) * v;
                out.push(Piece::Add(Shape::Capsule(top - along * reach * 0.3, top + along * reach, rng.range(2.0, 3.0) * v)));
                marks.push((Feature::Hoodoo, p * gp - along * 12.0 * v, top));
            }
        }
    }

    /// La forme la plus proche de `dir` (de cette sorte, ou de n'importe laquelle), dans un carré de
    /// `reach` cellules de côté autour de la sienne, sur la même face : (sorte, point du sol d'où
    /// la voir, point visé).
    pub fn nearest(&self, dir: Vec3, want: Option<Feature>, reach: i64, ground: &Ground) -> Option<(Feature, Vec3, Vec3)> {
        let (face, x, y) = self.key_of(dir);
        for ring in 0..=reach {
            let mut best: Option<(f32, (Feature, Vec3, Vec3))> = None;
            for cy in y - ring..=y + ring {
                for cx in x - ring..=x + ring {
                    if (cx - x).abs().max((cy - y).abs()) != ring || !(0..self.n).contains(&cx) || !(0..self.n).contains(&cy) {
                        continue;
                    }
                    for m in self.cell((face, cx, cy), ground).marks.iter() {
                        let d = m.1.normalize().distance_squared(dir);
                        if want.is_none_or(|f| f == m.0) && best.is_none_or(|b| d < b.0) {
                            best = Some((d, *m));
                        }
                    }
                }
            }
            if let Some((_, m)) = best {
                return Some(m);
            }
        }
        None
    }

    /// Formes autour de `dir` (sa cellule et les 8 voisines de la même face).
    pub fn around(&self, dir: Vec3, ground: &Ground) -> Vec<Piece> {
        let (face, x, y) = self.key_of(dir);
        let mut out = Vec::new();
        for dy in -1..=1 {
            for dx in -1..=1 {
                let (cx, cy) = (x + dx, y + dy);
                if (0..self.n).contains(&cx) && (0..self.n).contains(&cy) {
                    out.extend(self.cell((face, cx, cy), ground).pieces.iter().copied());
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
        let list: Arc<Vec<Piece>> = Arc::new(self.around(dir, ground));
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
        keys.into_iter().flat_map(|k| self.cell(k, ground).pieces.clone()).collect()
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
        let a = Rocks::new(5, radius, 7.0, true, true);
        let b = Rocks::new(5, radius, 7.0, true, true);
        let count = |r: &Rocks, g: &Ground| -> usize { (0..r.n).map(|x| r.build((2, x, r.n / 2), g).pieces.len().min(1)).sum() };
        let (n_flat, n_rugged) = (count(&a, &flat), count(&Rocks::new(5, radius, 7.0, true, true), &rugged));
        assert!(n_rugged > n_flat * 3, "{n_flat} {n_rugged}");
        assert_eq!(count(&a, &sea), 0);
        for x in 0..a.n {
            assert_eq!(a.build((4, x, 3), &rugged).pieces.len(), b.build((4, x, 3), &rugged).pieces.len());
        }
    }
}
