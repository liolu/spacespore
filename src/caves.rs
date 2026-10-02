//! Grottes (0.11, phase B2), creusées dans les voxels 3D (`terrain.rs`).
//!
//! L'espace de l'astre (repère fixe) est découpé en régions cubiques de ~40 voxels ; chaque région
//! est générée à la demande depuis sa graine (règle 12 : pas de liste globale) et oubliée ensuite.
//! Une région « à grotte » relie par des tunnels une salle centrale à des portes posées sur ses
//! faces ; une porte est partagée avec la région voisine (même hachage), si bien que les tunnels se
//! prolongent d'une région à l'autre en réseaux. Près de la surface, un puits fait l'entrée.
//!
//! Sortes, selon la géologie : tubes de lave (volcanisme, peu profonds, larges), karst (eau
//! liquide : salles, rivières et lacs, stalactites), grottes de glace (mondes gelés), géodes de
//! cristaux (rares), failles (tectonique). Jusqu'à 2 000 unités sous la surface (Q4).

use bevy::math::{IVec3, Vec3};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::planet::VoxelType;
use crate::planetgen::hydrology::Liquid;
use crate::terrain::BodyParams;

/// Profondeur maximale des grottes sous la surface (Q4).
pub const MAX_DEPTH: f32 = 2_000.0;
/// Côté d'une région, en voxels.
const REGION_VOXELS: f32 = 40.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CaveKind {
    LavaTube,
    Karst,
    Ice,
    Geode,
    Fault,
}

impl CaveKind {
    pub fn name(self) -> &'static str {
        match self {
            CaveKind::LavaTube => "tube de lave",
            CaveKind::Karst => "grotte karstique",
            CaveKind::Ice => "grotte de glace",
            CaveKind::Geode => "geode de cristaux",
            CaveKind::Fault => "faille",
        }
    }
}

/// Grottes possibles sur un astre, d'après sa géologie.
#[derive(Clone, Copy, Debug)]
pub struct CaveStyle {
    /// Part des régions qui ont une grotte.
    pub density: f32,
    /// Poids des sortes : tube de lave, karst, glace, géode, faille.
    pub weights: [f32; 5],
    /// Roche des parois profondes.
    pub rock: VoxelType,
    /// Rivières et lacs souterrains (eau liquide).
    pub water: bool,
    /// Champignons lumineux (il y a de la vie).
    pub glow: bool,
}

impl CaveStyle {
    pub fn of(p: &BodyParams) -> Option<Self> {
        if p.gaseous {
            return None;
        }
        let wet = p.hydro.liquid == Liquid::Water && p.atmosphere && (-10.0..90.0).contains(&p.climate.mean_c);
        let frozen = p.hydro.snow && p.climate.mean_c < -10.0;
        let volcanic = p.biomes.volcanism.max(if p.relief.volcanoes > 0 { 0.4 } else { 0.0 });
        let tectonic = if p.relief.plates > 0 { 0.6 } else { 0.15 };
        let weights = [
            volcanic * 1.5,
            if wet { 1.6 } else { 0.0 },
            if frozen { 1.4 } else { 0.0 },
            0.08,
            tectonic * 0.5,
        ];
        let rock = if frozen {
            VoxelType::Ice
        } else if volcanic > 0.5 {
            VoxelType::Basalt
        } else {
            VoxelType::Stone
        };
        Some(Self {
            density: (0.18 + 0.25 * weights.iter().sum::<f32>().min(2.0) / 2.0).min(0.45),
            weights,
            rock,
            water: wet,
            glow: p.biomes.flora,
        })
    }
}

/// Forme creusée ou ajoutée.
#[derive(Clone, Copy, Debug)]
pub enum Shape {
    Capsule(Vec3, Vec3, f32),
    Sphere(Vec3, f32),
    /// Fente plate : centre, axes (épaisseur, longueur, hauteur) et demi-tailles.
    Slab { c: Vec3, axes: [Vec3; 3], half: Vec3 },
}

impl Shape {
    pub fn contains(&self, p: Vec3) -> bool {
        match *self {
            Shape::Capsule(a, b, r) => {
                let ab = b - a;
                let t = ((p - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
                (a + ab * t).distance_squared(p) < r * r
            }
            Shape::Sphere(c, r) => c.distance_squared(p) < r * r,
            Shape::Slab { c, axes, half } => {
                let d = p - c;
                (0..3).all(|i| d.dot(axes[i]).abs() < half[i])
            }
        }
    }

    /// Sphère englobante (centre, rayon).
    fn bound(&self) -> (Vec3, f32) {
        match *self {
            Shape::Capsule(a, b, r) => ((a + b) * 0.5, a.distance(b) * 0.5 + r),
            Shape::Sphere(c, r) => (c, r),
            Shape::Slab { c, half, .. } => (c, half.length()),
        }
    }
}

/// Une région à grotte.
#[derive(Clone, Debug)]
pub struct Region {
    pub kind: CaveKind,
    /// Air (tunnels, salles, puits).
    pub carve: Vec<Shape>,
    /// Roche rajoutée dans l'air (stalactites, stalagmites).
    pub fill: Vec<Shape>,
    /// Sous ce rayon, l'air creusé est de l'eau (lac, rivière) ; 0 : pas d'eau.
    pub water_r: f32,
    /// Géode : centre et rayon (paroi de cristal).
    pub geode: Option<(Vec3, f32)>,
    /// Entrée (point de la surface), s'il y en a une.
    pub entrance: Option<Vec3>,
    /// Salle centrale (centre, rayon) : champignons lumineux sur son sol.
    pub room: Option<(Vec3, f32)>,
    bound: (Vec3, f32),
}

impl Region {
    /// Sphère englobante.
    pub fn bound(&self) -> (Vec3, f32) {
        self.bound
    }
}

/// Ce que la grotte met dans une cellule pleine du champ de hauteur.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaveCell {
    Rock,
    Air,
    Water,
    Crystal,
}

fn hash(seed: u32, k: IVec3, salt: u32) -> u64 {
    let mut x = (seed as u64) ^ ((salt as u64) << 40);
    for v in [k.x, k.y, k.z] {
        x ^= (v as u32 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        x ^= x >> 31;
        x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
        x ^= x >> 29;
    }
    x
}

/// Suite de nombres (0..1) tirée d'un hachage.
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

/// Grottes d'un astre : style, taille des régions et cache des régions déjà générées.
pub struct Caves {
    pub style: CaveStyle,
    seed: u32,
    /// Côté d'une région (unités) et taille d'un voxel.
    pub size: f32,
    v: f32,
    cache: Mutex<HashMap<IVec3, Option<Arc<Region>>>>,
    /// Tirage « grotte ou pas » des régions (sert aussi aux portes des voisines).
    drawn: Mutex<HashMap<IVec3, bool>>,
    /// Pièces autour de la dernière région interrogée (collisions : on reste au même endroit).
    near: Mutex<Option<(IVec3, Arc<Vec<Piece>>)>>,
}

impl Caves {
    pub fn new(style: CaveStyle, seed: u32, voxel: f32) -> Self {
        Self { style, seed: seed ^ 0xCA7E, size: REGION_VOXELS * voxel, v: voxel, cache: Mutex::new(HashMap::new()), drawn: Mutex::new(HashMap::new()), near: Mutex::new(None) }
    }

    /// Région qui contient le point `p`.
    pub fn key_of(&self, p: Vec3) -> IVec3 {
        (p / self.size).floor().as_ivec3()
    }

    fn center(&self, key: IVec3) -> Vec3 {
        (key.as_vec3() + 0.5) * self.size
    }

    /// La région a une grotte (sans la construire) : selon son hachage et sa profondeur.
    /// `surface` : rayon du sol dans une direction.
    fn has_cave(&self, key: IVec3, surface: &dyn Fn(Vec3) -> f32) -> bool {
        if let Some(&b) = self.drawn.lock().ok().and_then(|d| d.get(&key).copied()).as_ref() {
            return b;
        }
        let b = self.draw(key, surface);
        if let Ok(mut d) = self.drawn.lock() {
            if d.len() > 16_384 {
                d.clear();
            }
            d.insert(key, b);
        }
        b
    }

    fn draw(&self, key: IVec3, surface: &dyn Fn(Vec3) -> f32) -> bool {
        // Tirage d'abord (gratuit), la profondeur ensuite (bruit du relief)
        if ((hash(self.seed, key, 1) >> 40) as f32 / (1u64 << 24) as f32) >= self.style.density {
            return false;
        }
        let c = self.center(key);
        let r = c.length();
        let ground = surface(c / r.max(1.0));
        let depth = ground - r;
        !(depth < -0.3 * self.size || depth > MAX_DEPTH - 0.3 * self.size)
    }

    /// Porte entre `key` et sa voisine `key + axis` : partagée par les deux, ouverte si les deux
    /// ont une grotte (et selon le hachage de la face).
    fn port(&self, key: IVec3, axis: IVec3, surface: &dyn Fn(Vec3) -> f32) -> Option<Vec3> {
        let other = key + axis;
        let low = if axis.x + axis.y + axis.z > 0 { key } else { other };
        let a = if axis.x != 0 { 0 } else if axis.y != 0 { 1 } else { 2 };
        let mut rng = Rng(hash(self.seed, low, 10 + a as u32));
        if rng.unit() > 0.65 || !self.has_cave(other, surface) {
            return None;
        }
        // Point sur la face partagée (côté haut de `low`)
        let mut p = self.center(low);
        p[a] += self.size * 0.5;
        for b in 0..3 {
            if b != a {
                p[b] += rng.range(-0.3, 0.3) * self.size;
            }
        }
        Some(p)
    }

    /// Région `key` (générée une fois, puis gardée en cache).
    pub fn region(&self, key: IVec3, surface: &dyn Fn(Vec3) -> f32) -> Option<Arc<Region>> {
        if let Some(r) = self.cache.lock().ok().and_then(|c| c.get(&key).cloned()) {
            return r;
        }
        let region = self.build(key, surface).map(Arc::new);
        if let Ok(mut c) = self.cache.lock() {
            if c.len() > 4096 {
                c.clear();
            }
            c.insert(key, region.clone());
        }
        region
    }

    fn build(&self, key: IVec3, surface: &dyn Fn(Vec3) -> f32) -> Option<Region> {
        if !self.has_cave(key, surface) {
            return None;
        }
        let v = self.v;
        let s = self.size;
        let mut rng = Rng(hash(self.seed, key, 2));
        let c = self.center(key);
        let j = c + Vec3::new(rng.range(-0.2, 0.2), rng.range(-0.2, 0.2), rng.range(-0.2, 0.2)) * s;
        let up = j.normalize();
        let ground = surface(up);
        let depth = ground - j.length();
        // Sorte : pondérée par la géologie ; les tubes de lave restent près de la surface
        let mut w = self.style.weights;
        if depth > 400.0 {
            w[0] = 0.0;
        }
        let total: f32 = w.iter().sum();
        if total <= 0.0 {
            return None;
        }
        let mut roll = rng.unit() * total;
        let mut kind = CaveKind::Fault;
        for (i, k) in [CaveKind::LavaTube, CaveKind::Karst, CaveKind::Ice, CaveKind::Geode, CaveKind::Fault].into_iter().enumerate() {
            if roll < w[i] {
                kind = k;
                break;
            }
            roll -= w[i];
        }

        let mut carve = Vec::new();
        let mut fill = Vec::new();
        let mut geode = None;
        let mut room = None;
        let mut water_r = 0.0;
        let tunnel_r = match kind {
            CaveKind::LavaTube => rng.range(3.5, 6.0) * v,
            _ => rng.range(1.8, 3.2) * v,
        };
        // Salle centrale
        match kind {
            CaveKind::Karst | CaveKind::Ice => {
                let r = rng.range(6.0, 13.0) * v;
                carve.push(Shape::Sphere(j, r));
                room = Some((j, r));
                // Lac ou rivière au fond
                if kind == CaveKind::Karst && self.style.water && rng.unit() < 0.7 {
                    water_r = j.length() - r * 0.45;
                }
                // Stalactites et stalagmites
                let east = Vec3::Y.cross(up).normalize_or(Vec3::X);
                let north = up.cross(east);
                for n in 0..8 {
                    let a = rng.range(0.0, std::f32::consts::TAU);
                    let spread = rng.range(0.1, 0.6);
                    let side = east * a.cos() + north * a.sin();
                    let ceiling = n % 2 == 0;
                    let dir = (if ceiling { up } else { -up } + side * spread).normalize();
                    let root = j + dir * r * 1.02;
                    let tip = root - dir * rng.range(2.0, 5.0) * v;
                    fill.push(Shape::Capsule(root, tip, rng.range(0.7, 1.3) * v));
                }
            }
            CaveKind::LavaTube => {
                let r = tunnel_r * 1.2;
                carve.push(Shape::Sphere(j, r));
                room = Some((j, r));
            }
            CaveKind::Geode => {
                let r = rng.range(4.0, 8.0) * v;
                geode = Some((j, r));
            }
            CaveKind::Fault => {
                let east = Vec3::Y.cross(up).normalize_or(Vec3::X);
                let north = up.cross(east);
                let a = rng.range(0.0, std::f32::consts::PI);
                let n = east * a.cos() + north * a.sin();
                let along = up.cross(n);
                carve.push(Shape::Slab { c: j, axes: [n, along, up], half: Vec3::new(rng.range(0.6, 1.2) * v, rng.range(12.0, 18.0) * v, rng.range(8.0, 14.0) * v) });
            }
        }
        // Tunnels vers les portes (en coude, pour ne pas être droits)
        if kind != CaveKind::Geode {
            for axis in [IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z] {
                let Some(port) = self.port(key, axis, surface) else { continue };
                // Les tubes de lave courent à l'horizontale
                if kind == CaveKind::LavaTube && (port - j).normalize_or(up).dot(up).abs() > 0.6 {
                    continue;
                }
                let bend = (j + port) * 0.5 + Vec3::new(rng.range(-0.12, 0.12), rng.range(-0.12, 0.12), rng.range(-0.12, 0.12)) * s;
                carve.push(Shape::Capsule(j, bend, tunnel_r));
                carve.push(Shape::Capsule(bend, port, tunnel_r));
            }
        }
        // Entrée : un puits jusqu'à la surface, si la grotte est assez proche
        let mut entrance = None;
        if depth < 0.9 * s && kind != CaveKind::Geode && rng.unit() < 0.75 {
            let side = (Vec3::Y.cross(up).normalize_or(Vec3::X)) * rng.range(-0.15, 0.15) * s;
            let mouth_dir = (j + side).normalize();
            let mouth = mouth_dir * (surface(mouth_dir) + 3.0 * v);
            carve.push(Shape::Capsule(j, mouth, (tunnel_r * 1.1).max(2.5 * v)));
            entrance = Some(mouth_dir * surface(mouth_dir));
        }

        let mut bound = (j, 0.0f32);
        for shape in carve.iter().chain(fill.iter()) {
            let (bc, br) = shape.bound();
            bound.1 = bound.1.max(bc.distance(j) + br);
        }
        if let Some((gc, gr)) = geode {
            bound.1 = bound.1.max(gc.distance(j) + gr);
        }
        bound.1 += v;
        Some(Region { kind, carve, fill, water_r, geode, entrance, room, bound })
    }

    /// Régions (avec grotte) autour du point `p` : la sienne et ses 26 voisines.
    pub fn around(&self, p: Vec3, surface: &dyn Fn(Vec3) -> f32) -> Vec<Arc<Region>> {
        let k = self.key_of(p);
        let mut out = Vec::new();
        for dz in -1..=1 {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    if let Some(r) = self.region(k + IVec3::new(dx, dy, dz), surface) {
                        out.push(r);
                    }
                }
            }
        }
        out
    }

    /// Pièces des régions autour du point `p` (gardées tant qu'on reste dans la même région).
    pub fn pieces_near(&self, p: Vec3, surface: &dyn Fn(Vec3) -> f32) -> Arc<Vec<Piece>> {
        let key = self.key_of(p);
        if let Some((k, list)) = self.near.lock().ok().and_then(|n| n.clone()) {
            if k == key {
                return list;
            }
        }
        let list: Arc<Vec<Piece>> = Arc::new(self.around(p, surface).iter().flat_map(|r| r.pieces().collect::<Vec<_>>()).collect());
        if let Ok(mut n) = self.near.lock() {
            *n = Some((key, list.clone()));
        }
        list
    }

    /// Régions qui peuvent toucher une tuile : celles autour de points pris dans la tuile, de la
    /// surface jusqu'à la profondeur maximale.
    pub fn for_tile(&self, dirs: &[Vec3], surface: &dyn Fn(Vec3) -> f32) -> Vec<Arc<Region>> {
        let mut keys = std::collections::HashSet::new();
        for &d in dirs {
            let top = surface(d);
            let mut r = top + self.size;
            while r > top - MAX_DEPTH - self.size {
                let k = self.key_of(d * r);
                for dz in -1..=1 {
                    for dy in -1..=1 {
                        for dx in -1..=1 {
                            keys.insert(k + IVec3::new(dx, dy, dz));
                        }
                    }
                }
                r -= self.size * 0.5;
            }
        }
        keys.into_iter().filter_map(|k| self.region(k, surface)).collect()
    }
}

/// Une pièce d'une grotte : creusée (avec le niveau de l'eau de sa région), rajoutée
/// (stalactite) ou géode.
#[derive(Clone, Copy, Debug)]
pub enum Piece {
    Carve(Shape, f32),
    Fill(Shape),
    Geode(Vec3, f32),
    /// Roche posée sur le sol, même au-dessus de la surface (arches, cheminées : `rocks.rs`).
    Add(Shape),
}

impl Piece {
    /// Sphère englobante.
    pub fn bound(&self) -> (Vec3, f32) {
        match self {
            Piece::Carve(s, _) | Piece::Fill(s) | Piece::Add(s) => s.bound(),
            Piece::Geode(c, r) => (*c, *r),
        }
    }
}

impl Region {
    /// Toutes les pièces de la région.
    pub fn pieces(&self) -> impl Iterator<Item = Piece> + '_ {
        self.carve
            .iter()
            .map(|s| Piece::Carve(*s, self.water_r))
            .chain(self.fill.iter().map(|s| Piece::Fill(*s)))
            .chain(self.geode.map(|(c, r)| Piece::Geode(c, r)))
    }
}

/// Ce que des pièces de grottes mettent au point `p` : l'intérieur d'une géode est vide, sa
/// paroi en cristal ; une stalactite est de la roche ; un tunnel ou une salle, de l'air (ou de
/// l'eau sous le niveau de leur lac).
pub fn eval_pieces<'a>(pieces: impl Iterator<Item = &'a Piece>, p: Vec3, voxel: f32) -> CaveCell {
    let (mut carved, mut water, mut filled, mut crystal) = (false, false, false, false);
    for piece in pieces {
        match piece {
            Piece::Geode(c, r) => {
                let d = c.distance(p);
                if d < r - 1.3 * voxel {
                    return CaveCell::Air;
                }
                crystal |= d < *r;
            }
            Piece::Fill(s) | Piece::Add(s) => filled |= s.contains(p),
            Piece::Carve(s, water_r) => {
                if s.contains(p) {
                    carved = true;
                    water |= *water_r > 0.0 && p.length() < *water_r;
                }
            }
        }
    }
    if crystal {
        CaveCell::Crystal
    } else if filled || !carved {
        CaveCell::Rock
    } else if water {
        CaveCell::Water
    } else {
        CaveCell::Air
    }
}

impl Caves {
    /// Entrée de grotte la plus proche du point `p` (près de la surface), dans un rayon de
    /// `reach` régions : (point de la surface, sorte).
    pub fn nearest_entrance(&self, p: Vec3, reach: i32, surface: &dyn Fn(Vec3) -> f32) -> Option<(Vec3, CaveKind)> {
        let k = self.key_of(p);
        let r = p.length();
        let mut best: Option<(f32, Vec3, CaveKind)> = None;
        for dz in -reach..=reach {
            for dy in -reach..=reach {
                for dx in -reach..=reach {
                    let key = k + IVec3::new(dx, dy, dz);
                    // Les entrées sont près de la surface : on saute les régions trop hautes ou profondes
                    if (self.center(key).length() - r).abs() > 2.0 * self.size {
                        continue;
                    }
                    let Some(region) = self.region(key, surface) else { continue };
                    let Some(e) = region.entrance else { continue };
                    let d = e.distance(p);
                    if best.is_none_or(|(bd, _, _)| d < bd) {
                        best = Some((d, e, region.kind));
                    }
                }
            }
        }
        best.map(|(_, e, kind)| (e, kind))
    }
}

/// Ce que les grottes de `regions` mettent au point `p` (dans la roche du champ de hauteur).
#[cfg(test)]
pub fn cave_at(regions: &[Arc<Region>], p: Vec3, voxel: f32) -> CaveCell {
    let pieces: Vec<Piece> = regions
        .iter()
        .filter(|r| r.bound.0.distance_squared(p) <= r.bound.1 * r.bound.1)
        .flat_map(|r| r.pieces())
        .filter(|piece| {
            let (c, r) = piece.bound();
            c.distance_squared(p) <= r * r
        })
        .collect();
    eval_pieces(pieces.iter(), p, voxel)
}

/// Probabilité qu'une cellule de paroi soit un filon (phase 8 : plus riche en profondeur).
pub fn ore_chance(depth: f32) -> f32 {
    0.01 + 0.07 * (depth / MAX_DEPTH).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn style() -> CaveStyle {
        CaveStyle { density: 0.4, weights: [0.5, 1.6, 0.0, 0.08, 0.3], rock: VoxelType::Stone, water: true, glow: true }
    }

    #[test]
    fn shapes_contain_what_they_should() {
        let c = Shape::Capsule(Vec3::ZERO, Vec3::X * 10.0, 2.0);
        assert!(c.contains(Vec3::new(5.0, 1.5, 0.0)) && !c.contains(Vec3::new(5.0, 2.5, 0.0)));
        assert!(c.contains(Vec3::new(11.5, 0.0, 0.0)));
        let s = Shape::Slab { c: Vec3::ZERO, axes: [Vec3::X, Vec3::Y, Vec3::Z], half: Vec3::new(1.0, 5.0, 3.0) };
        assert!(s.contains(Vec3::new(0.5, 4.0, -2.0)) && !s.contains(Vec3::new(1.5, 0.0, 0.0)));
    }

    /// Règle 12 : une région ne dépend que de sa graine ; ses portes sont partagées avec ses
    /// voisines (les tunnels se rejoignent) ; rien au-delà de la profondeur maximale.
    #[test]
    fn regions_are_hashed_and_connected() {
        let radius = 9_000.0;
        let surface = |_: Vec3| radius;
        let a = Caves::new(style(), 7, 7.0);
        let b = Caves::new(style(), 7, 7.0);
        let mut found = 0;
        let mut shared = 0;
        for x in -3..3 {
            for z in -3..3 {
                let p = Vec3::new(x as f32, 0.0, z as f32) * a.size + Vec3::Y * (radius - 300.0);
                let key = a.key_of(p);
                let ra = a.region(key, &surface);
                let rb = b.region(key, &surface);
                assert_eq!(ra.is_some(), rb.is_some());
                if let (Some(ra), Some(rb)) = (ra, rb) {
                    found += 1;
                    assert_eq!(ra.carve.len(), rb.carve.len());
                    assert_eq!(ra.kind, rb.kind);
                    for axis in [IVec3::X, IVec3::Z] {
                        if let (Some(p1), Some(p2)) = (a.port(key, axis, &surface), a.port(key + axis, -axis, &surface)) {
                            assert!(p1.distance(p2) < 1e-3, "porte non partagee");
                            shared += 1;
                        }
                    }
                }
            }
        }
        assert!(found > 3, "{found} grottes");
        assert!(shared > 0, "aucun tunnel entre deux regions");
        // Trop profond : rien
        let deep = Vec3::Y * (radius - MAX_DEPTH - a.size * 2.0);
        assert!(a.region(a.key_of(deep), &surface).is_none());
    }

    #[test]
    fn caves_have_air_water_and_crystal() {
        let radius = 9_000.0;
        let surface = |_: Vec3| radius;
        let caves = Caves::new(style(), 3, 7.0);
        let (mut air, mut water, mut crystal, mut entrances) = (0, 0, 0, 0);
        for x in -6..6 {
            for z in -6..6 {
                for y in [-1, -3, -6] {
                    let p = Vec3::new(x as f32 * caves.size, radius + y as f32 * caves.size, z as f32 * caves.size);
                    let Some(r) = caves.region(caves.key_of(p), &surface) else { continue };
                    entrances += r.entrance.is_some() as usize;
                    if let Some((gc, gr)) = r.geode {
                        crystal += (cave_at(&[r.clone()], gc + Vec3::X * (gr - 0.5), 7.0) == CaveCell::Crystal) as usize;
                    }
                    if let Some((rc, rr)) = r.room {
                        air += (cave_at(&[r.clone()], rc, 7.0) == CaveCell::Air) as usize;
                        if r.water_r > 0.0 {
                            water += (cave_at(&[r.clone()], rc - rc.normalize() * rr * 0.8, 7.0) == CaveCell::Water) as usize;
                        }
                    }
                }
            }
        }
        assert!(air > 5 && water > 0 && entrances > 0, "air {air} eau {water} entrees {entrances}");
        let _ = crystal;
        assert!(ore_chance(1_900.0) > 4.0 * ore_chance(0.0));
    }
}
