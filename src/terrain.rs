//! Terrain voxel des planètes et des lunes, utilisé pour les atterrissages.
//!
//! Chaque corps est une sphère-cube : 6 faces, chacune découpée en un quadtree de tuiles de
//! 32 × 32 colonnes. Plus la caméra est proche d'une tuile, plus elle est subdivisée, jusqu'au
//! voxel le plus fin (une colonne d'environ 5 à 11 unités). La hauteur d'une colonne ne dépend que
//! de sa direction : le maillage affiché et le sol sous les pieds du joueur sont donc identiques.
//!
//! Tout ce fichier est du calcul pur (aucune ressource Bevy), ce qui permet de le tester.

use bevy::math::Vec3;
use bevy::render::mesh::{Indices, Mesh, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use noise::{Fbm, NoiseFn, Perlin};
use std::f32::consts::{FRAC_PI_2, FRAC_PI_4};

use crate::planet::VoxelType;
use crate::settings::{MoonConfig, PlanetConfig};

/// Colonnes par côté d'une tuile.
pub const TILE_CELLS: usize = 32;

/// Taille maximale d'un voxel au niveau le plus fin.
const MAX_VOXEL: f32 = 24.0;

/// Une tuile est subdivisée tant que la caméra est plus proche que ce multiple de sa taille.
pub const SPLIT_FACTOR: f32 = 1.8;

/// Longueurs d'onde maximales (unités) des collines moyennes et du relief fin ajoutés au relief du
/// corps ; elles rétrécissent avec le rayon pour les petites lunes.
const MID_WAVE: f32 = 1800.0;
const FINE_WAVE: f32 = 200.0;

// ─────────────────────────────────────────────────────────────────────────
//  Paramètres d'un corps
// ─────────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug)]
pub struct BodyParams {
    /// Lune : pas d'océan ni d'atmosphère, sol gris.
    pub airless: bool,
    pub atmosphere: bool,
    pub radius: f32,
    pub sea_level: f32,
    pub terrain_height: f32,
    pub seed: u32,
    pub noise_scale: f32,
    pub detail_scale: f32,
    pub temperature: f32,
}

impl BodyParams {
    pub fn planet(p: &PlanetConfig) -> Self {
        Self {
            airless: false,
            atmosphere: p.atmosphere,
            radius: p.radius,
            sea_level: p.sea_level,
            terrain_height: p.terrain_height,
            seed: p.seed,
            noise_scale: p.noise_scale,
            detail_scale: p.detail_scale,
            temperature: p.temperature(),
        }
    }

    pub fn moon(m: &MoonConfig, parent: &PlanetConfig) -> Self {
        Self {
            airless: true,
            atmosphere: false,
            radius: m.radius,
            sea_level: 0.5,
            terrain_height: m.radius * 0.045,
            seed: m.seed,
            noise_scale: 2.0,
            detail_scale: 4.0,
            temperature: parent.temperature(),
        }
    }

    pub fn layout(&self) -> Layout {
        layout_for(self.radius)
    }
}

/// Finesse du quadtree d'un corps.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    /// Profondeur du niveau le plus fin (les tuiles racines sont à la profondeur 0).
    pub max_depth: u32,
    /// Taille d'un voxel au niveau le plus fin (unités).
    pub voxel: f32,
}

pub fn layout_for(radius: f32) -> Layout {
    let arc = FRAC_PI_2 * radius;
    let mut depth = 0u32;
    while depth < 14 && arc / ((TILE_CELLS << depth) as f32) > MAX_VOXEL {
        depth += 1;
    }
    Layout { max_depth: depth, voxel: arc / (TILE_CELLS << depth) as f32 }
}

// ─────────────────────────────────────────────────────────────────────────
//  Faces de la sphère-cube (projection équiangulaire : cellules presque uniformes)
// ─────────────────────────────────────────────────────────────────────────

/// Direction (unitaire) du point (s, t) ∈ [-1, 1]² de la face `face` (0..6).
/// Accepte des valeurs un peu hors de [-1, 1] (colonnes voisines au bord d'une face).
pub fn face_dir(face: u8, s: f32, t: f32) -> Vec3 {
    let a = (s * FRAC_PI_4).tan();
    let b = (t * FRAC_PI_4).tan();
    let v = match face {
        0 => Vec3::new(1.0, b, -a),
        1 => Vec3::new(-1.0, b, a),
        2 => Vec3::new(a, 1.0, -b),
        3 => Vec3::new(a, -1.0, b),
        4 => Vec3::new(a, b, 1.0),
        _ => Vec3::new(-a, b, -1.0),
    };
    v.normalize()
}

/// Inverse de `face_dir` : (face, s, t).
pub fn dir_to_face(dir: Vec3) -> (u8, f32, f32) {
    let (ax, ay, az) = (dir.x.abs(), dir.y.abs(), dir.z.abs());
    let (face, a, b) = if ax >= ay && ax >= az {
        if dir.x > 0.0 { (0u8, -dir.z / ax, dir.y / ax) } else { (1, dir.z / ax, dir.y / ax) }
    } else if ay >= az {
        if dir.y > 0.0 { (2, dir.x / ay, -dir.z / ay) } else { (3, dir.x / ay, dir.z / ay) }
    } else if dir.z > 0.0 {
        (4, dir.x / az, dir.y / az)
    } else {
        (5, -dir.x / az, dir.y / az)
    };
    (face, a.atan() / FRAC_PI_4, b.atan() / FRAC_PI_4)
}

// ─────────────────────────────────────────────────────────────────────────
//  Tuiles du quadtree
// ─────────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TileKey {
    pub face: u8,
    pub depth: u8,
    pub x: u32,
    pub y: u32,
}

impl TileKey {
    pub fn root(face: u8) -> Self {
        Self { face, depth: 0, x: 0, y: 0 }
    }

    pub fn parent(self) -> Option<Self> {
        (self.depth > 0).then(|| Self { face: self.face, depth: self.depth - 1, x: self.x >> 1, y: self.y >> 1 })
    }

    pub fn children(self) -> [Self; 4] {
        let d = self.depth + 1;
        let (x, y) = (self.x << 1, self.y << 1);
        [
            Self { face: self.face, depth: d, x, y },
            Self { face: self.face, depth: d, x: x + 1, y },
            Self { face: self.face, depth: d, x, y: y + 1 },
            Self { face: self.face, depth: d, x: x + 1, y: y + 1 },
        ]
    }

    /// `self` contient strictement `other`.
    #[cfg(test)]
    pub fn is_ancestor_of(self, other: Self) -> bool {
        if self.face != other.face || other.depth <= self.depth {
            return false;
        }
        let shift = other.depth - self.depth;
        (other.x >> shift) == self.x && (other.y >> shift) == self.y
    }

    pub fn center_dir(self) -> Vec3 {
        let n = (1u32 << self.depth) as f32;
        face_dir(self.face, -1.0 + 2.0 * (self.x as f32 + 0.5) / n, -1.0 + 2.0 * (self.y as f32 + 0.5) / n)
    }

    /// Longueur d'arc d'un côté de la tuile.
    pub fn arc(self, radius: f32) -> f32 {
        FRAC_PI_2 * radius / (1u32 << self.depth) as f32
    }
}

/// Tuiles (feuilles du quadtree) à afficher pour une caméra à `cam_local` (repère du corps).
pub fn select_tiles(layout: Layout, radius: f32, cam_local: Vec3, out: &mut Vec<TileKey>) {
    fn visit(key: TileKey, layout: Layout, radius: f32, cam: Vec3, out: &mut Vec<TileKey>) {
        if (key.depth as u32) < layout.max_depth {
            let dist = (cam - key.center_dir() * radius).length();
            if dist < SPLIT_FACTOR * key.arc(radius) {
                for child in key.children() {
                    visit(child, layout, radius, cam, out);
                }
                return;
            }
        }
        out.push(key);
    }
    for face in 0..6 {
        visit(TileKey::root(face), layout, radius, cam_local, out);
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  Champ de hauteur
// ─────────────────────────────────────────────────────────────────────────

/// Une colonne de voxels : hauteur de sa face supérieure et apparence.
#[derive(Clone, Copy, Debug)]
pub struct Column {
    pub dir: Vec3,
    /// Rayon de la face supérieure.
    pub top: f32,
    pub kind: VoxelType,
    pub color: [f32; 4],
}

pub struct Terrain {
    pub params: BodyParams,
    pub layout: Layout,
    continent: Fbm<Perlin>,
    detail: Fbm<Perlin>,
    mid: Fbm<Perlin>,
    fine: Fbm<Perlin>,
    color: Perlin,
}

impl Terrain {
    pub fn new(params: BodyParams) -> Self {
        let mut continent: Fbm<Perlin> = Fbm::new(params.seed);
        continent.octaves = 6;
        let mut detail: Fbm<Perlin> = Fbm::new(params.seed.wrapping_add(81));
        detail.octaves = 4;
        let mut mid: Fbm<Perlin> = Fbm::new(params.seed.wrapping_add(131));
        mid.octaves = 3;
        let mut fine: Fbm<Perlin> = Fbm::new(params.seed.wrapping_add(171));
        fine.octaves = 4;
        Self {
            params,
            layout: params.layout(),
            continent,
            detail,
            mid,
            fine,
            color: Perlin::new(params.seed.wrapping_add(200)),
        }
    }

    pub fn voxel(&self) -> f32 {
        self.layout.voxel
    }

    /// Rayon brut (non quantifié) du sol dans la direction `dir`, et sa « hauteur relative » 0..1.
    fn raw_height(&self, dir: Vec3) -> (f32, f32) {
        let p = &self.params;
        let s = dir * p.noise_scale;
        let continent = self.continent.get([s.x as f64, s.y as f64, s.z as f64]) as f32;
        let ds = p.detail_scale as f64;
        let det = self.detail.get([s.x as f64 * ds, s.y as f64 * ds, s.z as f64 * ds]) as f32 * 0.15;
        let hv = ((continent + det + 1.0) * 0.5).clamp(0.0, 1.0);

        let rugged = if p.airless { 1.5 } else { 1.0 };
        let mid_amp = (p.terrain_height * 0.5).clamp(self.layout.voxel * 3.0, self.layout.voxel * 36.0) * rugged;
        let fine_amp = self.layout.voxel * 2.5 * rugged;
        let mf = (p.radius / (p.radius * 0.3).clamp(250.0, MID_WAVE)) as f64;
        let ff = (p.radius / (p.radius * 0.1).clamp(80.0, FINE_WAVE)) as f64;
        let mid = self.mid.get([dir.x as f64 * mf, dir.y as f64 * mf, dir.z as f64 * mf]) as f32 * mid_amp;
        let fine = self.fine.get([dir.x as f64 * ff, dir.y as f64 * ff, dir.z as f64 * ff]) as f32 * fine_amp;

        let h = p.radius + (hv - p.sea_level) * p.terrain_height + mid + fine;
        (h, hv)
    }

    fn surface_type(&self, rh: f32, lat: f32) -> VoxelType {
        let t = self.params.temperature;
        let snow_lat = if t > 200.0 {
            2.0
        } else if t > 60.0 {
            0.95
        } else if t > 20.0 {
            0.78
        } else if t > -20.0 {
            0.50
        } else {
            0.15
        };
        if self.params.airless {
            return VoxelType::Stone;
        }
        if t > 300.0 {
            if rh < 0.0 { VoxelType::Stone } else { VoxelType::Sand }
        } else if t > 100.0 {
            if rh < 0.30 { VoxelType::Sand } else { VoxelType::Stone }
        } else if rh < 0.02 {
            VoxelType::Sand
        } else if rh < 0.12 {
            if lat > snow_lat { VoxelType::Snow } else { VoxelType::Grass }
        } else if rh < 0.28 {
            if lat > snow_lat - 0.04 { VoxelType::Snow } else { VoxelType::Grass }
        } else if rh < 0.42 {
            if lat > snow_lat { VoxelType::Snow } else { VoxelType::Stone }
        } else {
            VoxelType::Snow
        }
    }

    /// Colonne dans la direction `dir`, hauteur arrondie au multiple de `quantum` au-dessus du niveau de la mer.
    pub fn column(&self, dir: Vec3, quantum: f32) -> Column {
        let p = &self.params;
        let (h, hv) = self.raw_height(dir);
        let rel = ((h - p.radius) / quantum).round();
        let water = !p.airless && rel < 0.0;
        let frozen = p.temperature < -50.0;

        let var = self.color.get([dir.x as f64 * 12.0, dir.y as f64 * 12.0, dir.z as f64 * 12.0]) as f32 * 0.10;
        let jitter = ((dir.x * 127.1 + dir.y * 311.7 + dir.z * 74.7).sin() * 43758.547).fract().abs() * 0.05 - 0.025;

        let (top, kind, color) = if water {
            let kind = if frozen { VoxelType::Snow } else { VoxelType::Water };
            let base = kind.color();
            let depth = ((p.radius - h) / (p.terrain_height * 0.5)).clamp(0.0, 0.45);
            let color = [
                (base[0] * (1.0 - depth) + var * 0.7 + jitter).clamp(0.03, 1.0),
                (base[1] * (1.0 - depth * 0.4) + var * 0.9 + jitter).clamp(0.03, 1.0),
                (base[2] * (1.0 - depth * 0.2) + var * 0.4 + jitter).clamp(0.03, 1.0),
                1.0,
            ];
            (p.radius, kind, color)
        } else {
            let kind = self.surface_type(hv - p.sea_level, dir.y.abs());
            let color = if p.airless {
                let (lo, hi) = ([0.45, 0.44, 0.42], [0.70, 0.68, 0.65]);
                [
                    (lo[0] + (hi[0] - lo[0]) * hv + var + jitter).clamp(0.03, 1.0),
                    (lo[1] + (hi[1] - lo[1]) * hv + var * 0.8 + jitter).clamp(0.03, 1.0),
                    (lo[2] + (hi[2] - lo[2]) * hv + var * 0.4 + jitter).clamp(0.03, 1.0),
                    1.0,
                ]
            } else {
                let base = kind.color();
                [
                    (base[0] + var * 0.7 + jitter).clamp(0.03, 1.0),
                    (base[1] + var * 0.9 + jitter).clamp(0.03, 1.0),
                    (base[2] + var * 0.4 + jitter).clamp(0.03, 1.0),
                    1.0,
                ]
            };
            (p.radius + rel * quantum, kind, color)
        };
        Column { dir, top, kind, color }
    }

    /// Colonne du niveau le plus fin qui contient la direction `dir` (sol marchable).
    pub fn ground(&self, dir: Vec3) -> Column {
        let n = (TILE_CELLS << self.layout.max_depth) as f32;
        let (face, s, t) = dir_to_face(dir);
        let i = (((s + 1.0) * 0.5 * n).floor()).clamp(0.0, n - 1.0);
        let j = (((t + 1.0) * 0.5 * n).floor()).clamp(0.0, n - 1.0);
        let center = face_dir(face, -1.0 + 2.0 * (i + 0.5) / n, -1.0 + 2.0 * (j + 0.5) / n);
        self.column(center, self.layout.voxel)
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  Maillage d'une tuile
// ─────────────────────────────────────────────────────────────────────────

#[derive(Default)]
struct MeshBuf {
    pos: Vec<[f32; 3]>,
    nor: Vec<[f32; 3]>,
    col: Vec<[f32; 4]>,
    idx: Vec<u32>,
}

impl MeshBuf {
    /// Quad plat dont la face avant regarde du côté de `normal`.
    fn quad(&mut self, c: [Vec3; 4], normal: Vec3, color: [f32; 4]) {
        let base = self.pos.len() as u32;
        for p in &c {
            self.pos.push(p.to_array());
            self.nor.push(normal.to_array());
            self.col.push(color);
        }
        let geo = (c[1] - c[0]).cross(c[2] - c[0]);
        if geo.dot(normal) >= 0.0 {
            self.idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        } else {
            self.idx.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
        }
    }

    fn into_mesh(self) -> Mesh {
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.pos);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.nor);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.col);
        mesh.insert_indices(Indices::U32(self.idx));
        mesh
    }
}

/// Quantum vertical (pas de hauteur) d'une tuile de profondeur `depth` : les cellules grossissent avec
/// la distance, mais pas les marches, sinon le lointain serait fait de falaises de plusieurs
/// centaines d'unités.
pub fn tile_quantum(layout: Layout, depth: u32) -> f32 {
    let steps = layout.max_depth - depth.min(layout.max_depth);
    layout.voxel * (1u32 << steps.min(MAX_STEP_DOUBLINGS)) as f32
}

/// Les marches ne dépassent pas 4 voxels (2 doublements).
const MAX_STEP_DOUBLINGS: u32 = 2;

/// Construit le maillage d'une tuile, dans le repère du corps (centre à l'origine).
///
/// Chaque colonne donne une face supérieure ; entre deux colonnes de hauteurs différentes, la plus
/// haute dessine la paroi qui les sépare. Une « jupe » descend le long des bords de la tuile pour
/// cacher les fentes avec les tuiles voisines de profondeur différente.
pub fn build_tile_mesh(params: &BodyParams, key: TileKey) -> Mesh {
    let terrain = Terrain::new(*params);
    let layout = terrain.layout;
    let depth = (key.depth as u32).min(layout.max_depth);
    let lattice = (TILE_CELLS as u32) << depth;
    let quantum = tile_quantum(layout, depth);
    let skirt = quantum * 8.0;
    let (i0, j0) = (key.x as i64 * TILE_CELLS as i64, key.y as i64 * TILE_CELLS as i64);

    let line = |i: i64| -> f32 { -1.0 + 2.0 * i as f32 / lattice as f32 };
    let mid = |i: i64| -> f32 { -1.0 + 2.0 * (i as f32 + 0.5) / lattice as f32 };

    let n1 = TILE_CELLS + 1;
    let mut corners = Vec::with_capacity(n1 * n1);
    for cj in 0..n1 as i64 {
        for ci in 0..n1 as i64 {
            corners.push(face_dir(key.face, line(i0 + ci), line(j0 + cj)));
        }
    }
    let corner = |ci: usize, cj: usize| corners[cj * n1 + ci];

    // Colonnes de la tuile et une rangée de colonnes voisines tout autour
    let nc = TILE_CELLS + 2;
    let mut cols = Vec::with_capacity(nc * nc);
    for cj in -1..=TILE_CELLS as i64 {
        for ci in -1..=TILE_CELLS as i64 {
            cols.push(terrain.column(face_dir(key.face, mid(i0 + ci), mid(j0 + cj)), quantum));
        }
    }
    let col = |ci: i32, cj: i32| &cols[(cj + 1) as usize * nc + (ci + 1) as usize];

    let mut buf = MeshBuf::default();
    buf.pos.reserve(TILE_CELLS * TILE_CELLS * 6);
    let last = TILE_CELLS as i32 - 1;

    for cj in 0..TILE_CELLS as i32 {
        for ci in 0..TILE_CELLS as i32 {
            let c = col(ci, cj);
            let (u, v) = (ci as usize, cj as usize);

            buf.quad(
                [
                    corner(u, v) * c.top,
                    corner(u + 1, v) * c.top,
                    corner(u + 1, v + 1) * c.top,
                    corner(u, v + 1) * c.top,
                ],
                c.dir,
                c.color,
            );

            // (voisin, extrémités de l'arête partagée)
            let sides: [((i32, i32), (usize, usize), (usize, usize)); 4] = [
                ((1, 0), (u + 1, v), (u + 1, v + 1)),
                ((-1, 0), (u, v), (u, v + 1)),
                ((0, 1), (u, v + 1), (u + 1, v + 1)),
                ((0, -1), (u, v), (u + 1, v)),
            ];
            for ((dx, dy), ea, eb) in sides {
                let (ni, nj) = (ci + dx, cj + dy);
                let nb = col(ni, nj);
                let boundary = ni < 0 || ni > last || nj < 0 || nj > last;
                let hi = c.top;
                let lo = if boundary {
                    c.top.min(nb.top) - skirt
                } else if c.top > nb.top + 1e-3 {
                    nb.top
                } else {
                    continue;
                };
                let (a, b) = (corner(ea.0, ea.1), corner(eb.0, eb.1));
                let mut n = (b - a).cross(c.dir).normalize_or_zero();
                if n.dot(nb.dir - c.dir) < 0.0 {
                    n = -n;
                }
                let shade = [c.color[0] * 0.82, c.color[1] * 0.82, c.color[2] * 0.82, 1.0];
                buf.quad([a * lo, b * lo, b * hi, a * hi], n, shade);
            }
        }
    }
    buf.into_mesh()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::render::mesh::VertexAttributeValues;

    fn earth_like() -> BodyParams {
        BodyParams {
            airless: false,
            atmosphere: true,
            radius: 9000.0,
            sea_level: 0.4,
            terrain_height: 360.0,
            seed: 1234,
            noise_scale: 2.0,
            detail_scale: 4.0,
            temperature: 15.0,
        }
    }

    #[test]
    fn faces_round_trip() {
        for face in 0..6u8 {
            for &(s, t) in &[(0.0, 0.0), (0.7, -0.3), (-0.95, 0.95), (0.2, 0.9)] {
                let d = face_dir(face, s, t);
                let (f2, s2, t2) = dir_to_face(d);
                assert_eq!(face, f2, "face {face} ({s},{t})");
                assert!((s - s2).abs() < 1e-4 && (t - t2).abs() < 1e-4, "face {face}: ({s},{t}) -> ({s2},{t2})");
            }
        }
    }

    #[test]
    fn voxel_size_stays_in_range() {
        for r in [120.0, 300.0, 500.0, 1_000.0, 1_500.0, 3_000.0, 6_000.0, 13_000.0] {
            let l = layout_for(r);
            assert!(l.voxel > MAX_VOXEL * 0.2 && l.voxel <= MAX_VOXEL, "rayon {r} : voxel {}", l.voxel);
        }
    }

    #[test]
    fn selection_refines_around_the_camera() {
        let p = earth_like();
        let t = Terrain::new(p);
        let dir = Vec3::new(0.3, 0.8, 0.5).normalize();
        let cam = dir * (t.ground(dir).top + 15.0);
        let mut tiles = Vec::new();
        select_tiles(t.layout, p.radius, cam, &mut tiles);
        assert!(tiles.len() < 600, "{} tuiles", tiles.len());
        // Le niveau le plus fin existe sous la caméra...
        let (face, s, tt) = dir_to_face(dir);
        let finest = tiles.iter().filter(|k| k.depth as u32 == t.layout.max_depth && k.face == face).count();
        assert!(finest > 0);
        let n = (1u32 << t.layout.max_depth) as f32;
        let under = TileKey {
            face,
            depth: t.layout.max_depth as u8,
            x: (((s + 1.0) * 0.5 * n) as u32).min(n as u32 - 1),
            y: (((tt + 1.0) * 0.5 * n) as u32).min(n as u32 - 1),
        };
        assert!(tiles.contains(&under));
        // ...et les tuiles ne se chevauchent pas
        for a in &tiles {
            for b in &tiles {
                assert!(!a.is_ancestor_of(*b));
            }
        }
    }

    #[test]
    fn far_camera_uses_only_coarse_tiles() {
        let p = earth_like();
        let l = p.layout();
        let mut tiles = Vec::new();
        select_tiles(l, p.radius, Vec3::new(0.0, 4.0 * p.radius, 0.0), &mut tiles);
        assert!(tiles.len() <= 24, "{} tuiles", tiles.len());
    }

    #[test]
    fn tile_mesh_is_well_formed() {
        let p = earth_like();
        let t = Terrain::new(p);
        let dir = Vec3::new(0.1, 0.9, 0.2).normalize();
        let (face, s, tt) = dir_to_face(dir);
        for depth in [0u32, 2, t.layout.max_depth] {
            let n = (1u32 << depth) as f32;
            let key = TileKey {
                face,
                depth: depth as u8,
                x: (((s + 1.0) * 0.5 * n) as u32).min(n as u32 - 1),
                y: (((tt + 1.0) * 0.5 * n) as u32).min(n as u32 - 1),
            };
            let mesh = build_tile_mesh(&p, key);
            let Some(VertexAttributeValues::Float32x3(pos)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else { panic!() };
            let Some(VertexAttributeValues::Float32x3(nor)) = mesh.attribute(Mesh::ATTRIBUTE_NORMAL) else { panic!() };
            let Some(Indices::U32(idx)) = mesh.indices() else { panic!() };
            assert!(pos.len() >= TILE_CELLS * TILE_CELLS * 4);
            assert_eq!(idx.len() % 3, 0);
            let mut up_facing = 0;
            for tri in idx.chunks(3) {
                let v = |i: u32| Vec3::from_array(pos[i as usize]);
                let n = Vec3::from_array(nor[tri[0] as usize]);
                let geo = (v(tri[1]) - v(tri[0])).cross(v(tri[2]) - v(tri[0]));
                assert!(geo.is_finite() && geo.length() > 0.0, "triangle dégénéré");
                // Face avant du côté de la normale : sinon la surface serait invisible
                assert!(geo.dot(n) > 0.0, "triangle à l'envers (profondeur {depth})");
                if n.dot(v(tri[0]).normalize()) > 0.99 {
                    up_facing += 1;
                }
            }
            assert!(up_facing >= TILE_CELLS * TILE_CELLS * 2, "faces supérieures : {up_facing}");
            for v in pos {
                let r = Vec3::from_array(*v).length();
                assert!(r > p.radius * 0.5 && r < p.radius + 2000.0, "sommet hors de la planète : {r}");
            }
        }
    }

    #[test]
    fn ground_matches_the_finest_mesh() {
        let p = earth_like();
        let t = Terrain::new(p);
        let dir = Vec3::new(-0.4, 0.3, 0.86).normalize();
        let (face, s, tt) = dir_to_face(dir);
        let n = (1u32 << t.layout.max_depth) as f32;
        let key = TileKey {
            face,
            depth: t.layout.max_depth as u8,
            x: (((s + 1.0) * 0.5 * n) as u32).min(n as u32 - 1),
            y: (((tt + 1.0) * 0.5 * n) as u32).min(n as u32 - 1),
        };
        let mesh = build_tile_mesh(&p, key);
        let Some(VertexAttributeValues::Float32x3(pos)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else { panic!() };
        // Le sommet de face supérieure le plus proche de `dir` est à la hauteur de `ground`
        let ground = t.ground(dir).top;
        let best = pos
            .iter()
            .map(|p| Vec3::from_array(*p))
            .min_by(|a, b| a.normalize().distance(dir).total_cmp(&b.normalize().distance(dir)))
            .unwrap();
        assert!((best.length() - ground).abs() <= t.voxel() * 8.0 + 1.0, "{} vs {}", best.length(), ground);
        assert!(ground > p.radius - 1.0 && ground < p.radius + p.terrain_height * 3.0);
    }

    #[test]
    fn moons_are_gray_and_dry() {
        let mut p = earth_like();
        p.airless = true;
        p.atmosphere = false;
        p.radius = 3000.0;
        p.terrain_height = 135.0;
        let t = Terrain::new(p);
        for k in 0..50 {
            let a = k as f32 * 0.37;
            let c = t.ground(Vec3::new(a.cos(), (a * 0.7).sin(), a.sin()).normalize());
            assert_ne!(c.kind, VoxelType::Water);
            assert!((c.color[0] - c.color[2]).abs() < 0.2, "pas gris : {:?}", c.color);
        }
    }
}
