//! Astéroïdes des ceintures (C1 de `ROADMAP-0.11.md`).
//!
//! - Pas de liste (règle 12) : chaque ceinture (`planetgen::belts`) est découpée en cellules dans
//!   le repère qui tourne avec elle (anneau, secteur à l'instant 0, couche). La graine d'une cellule
//!   donne ses astéroïdes : nombre selon la densité (champs denses), taille (loi de puissance),
//!   type C / S / M / glace (lien avec les minerais de la phase 8), forme et rotation propre. Trois
//!   niveaux de cellules : cailloux, rochers, gros astéroïdes (où l'on peut se poser).
//! - Tout bouge d'après l'horloge du monde (règle 9) : chaque cellule suit l'orbite de Kepler de son
//!   anneau (les anneaux intérieurs vont plus vite), chaque astéroïde y ajoute un petit épicycle et
//!   tourne sur son plus petit axe.
//! - Formes irrégulières (d'après `astre/planete/meteoroid.rs`) : tas de gravats, binaires de
//!   contact, allongés, métalliques, fragments à facettes, avec leurs cratères. Une seule fonction
//!   (`AsteroidShape::radius_at`) donne le maillage, les collisions et le terrain en voxels 3D quand
//!   on se pose (règle 11).
//! - Autour de la caméra seulement : les cellules proches sont créées, les autres oubliées. Les
//!   maillages des gros sont construits hors du fil principal (règle 13) ; de loin, la ceinture est
//!   une bande de poussière.
//! - Collisions (Q6) : en vol, un choc retire de la coque selon la vitesse ; les petits cailloux
//!   sont repoussés sans dégât. Le pilote automatique contourne les astéroïdes.

use bevy::math::DVec3;
use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::tasks::{block_on, futures_lite::future, AsyncComputeTaskPool, Task};
use std::collections::HashMap;
use std::f64::consts::TAU;

use crate::net::{Net, MAX_HP};
use crate::planet::SpawnedSystems;
use crate::planetgen::belts::{AsteroidClass, Belt, Swarm};
use crate::planetgen::comets::Comet;
use crate::planetgen::system::Ring;
use crate::planetgen::seeds::splitmix64;
use crate::settings::{origin, to_abs, GameSettings, PlanetConfig, StarSystemConfig};
use crate::ship::Ship;
use crate::surface::{FarMesh, Surface};
use crate::ui::{CameraTarget, TargetKind};
use crate::world_clock::{Spin, WorldClock};
use crate::CameraController;

pub struct AsteroidsPlugin;

impl Plugin for AsteroidsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AsteroidField>()
            .add_event::<AsteroidHit>()
            .add_event::<BeltCommand>()
            .add_systems(Startup, setup_assets)
            // Après le recentrage de l'origine (First) : tout le reste de l'image voit les poses
            .add_systems(PreUpdate, place_asteroids)
            .init_resource::<CometFx>()
            .add_event::<CometCommand>()
            .add_systems(Update, (go_belt, go_comet, stream_asteroids, update_bands, update_comet_tails).chain().before(crate::surface::SurfaceControl))
            .add_systems(Update, draw_trails.after(crate::surface::SurfaceControl))
            .add_systems(PostUpdate, ship_collisions.before(bevy::transform::TransformSystem::TransformPropagate))
            .add_systems(PostUpdate, comet_log.after(bevy::transform::TransformSystem::TransformPropagate));
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  Hachage et bruit
// ─────────────────────────────────────────────────────────────────────────

fn mix(a: u64, b: u64) -> u64 {
    splitmix64(a ^ splitmix64(b))
}

/// Tirages déterministes.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        splitmix64(self.0)
    }

    fn f64(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }

    fn f32(&mut self) -> f32 {
        self.f64() as f32
    }

    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.f32()
    }

    /// Direction uniforme sur la sphère.
    fn dir(&mut self) -> Vec3 {
        let z = self.range(-1.0, 1.0);
        let a = self.range(0.0, std::f32::consts::TAU);
        let s = (1.0 - z * z).max(0.0).sqrt();
        Vec3::new(s * a.cos(), z, s * a.sin())
    }
}

fn lattice(x: i32, y: i32, z: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8DA6_B343) ^ (y as u32).wrapping_mul(0xD816_3841) ^ (z as u32).wrapping_mul(0xCB1A_B31F) ^ seed.wrapping_mul(0x9E37_79B9);
    h ^= h >> 16;
    h = h.wrapping_mul(0x85EB_CA6B);
    h ^= h >> 13;
    h = h.wrapping_mul(0xC2B2_AE35);
    h ^= h >> 16;
    (h >> 8) as f32 / (1u32 << 24) as f32
}

/// Bruit de valeur lissé dans [0, 1].
pub(crate) fn vnoise(p: Vec3, seed: u32) -> f32 {
    let f = p.floor();
    let (x, y, z) = (f.x as i32, f.y as i32, f.z as i32);
    let t = p - f;
    let s = t * t * (Vec3::splat(3.0) - 2.0 * t);
    let l = |dx, dy, dz| lattice(x + dx, y + dy, z + dz, seed);
    let x00 = l(0, 0, 0) + (l(1, 0, 0) - l(0, 0, 0)) * s.x;
    let x10 = l(0, 1, 0) + (l(1, 1, 0) - l(0, 1, 0)) * s.x;
    let x01 = l(0, 0, 1) + (l(1, 0, 1) - l(0, 0, 1)) * s.x;
    let x11 = l(0, 1, 1) + (l(1, 1, 1) - l(0, 1, 1)) * s.x;
    let y0 = x00 + (x10 - x00) * s.y;
    let y1 = x01 + (x11 - x01) * s.y;
    y0 + (y1 - y0) * s.z
}

/// Bruit fractal dans [0, 1].
pub(crate) fn fbm(p: Vec3, seed: u32, octaves: u32) -> f32 {
    let (mut sum, mut amp, mut norm, mut q) = (0.0, 1.0, 0.0, p);
    for o in 0..octaves {
        sum += vnoise(q, seed.wrapping_add(o * 101)) * amp;
        norm += amp;
        amp *= 0.5;
        q *= 2.03;
    }
    sum / norm
}

fn smoothstep(e0: f64, e1: f64, x: f64) -> f64 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

// ─────────────────────────────────────────────────────────────────────────
//  Formes
// ─────────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeKind {
    /// Tas de gravats : bosses et blocs, cratères (la plupart des astéroïdes).
    Rubble,
    /// Binaire de contact : deux lobes soudés (comme Arrokoth ou Itokawa).
    Binary,
    /// Allongé (comme Éros).
    Elongated,
    /// Métallique : lisse, à grandes facettes (noyau mis à nu).
    Metallic,
    /// Fragment anguleux d'une collision.
    Fragment,
}

impl ShapeKind {
    pub fn name(self) -> &'static str {
        match self {
            ShapeKind::Rubble => "tas de gravats",
            ShapeKind::Binary => "binaire de contact",
            ShapeKind::Elongated => "allonge",
            ShapeKind::Metallic => "metallique",
            ShapeKind::Fragment => "fragment",
        }
    }
}

const MAX_CRATERS: usize = 14;
const MAX_FACETS: usize = 9;

/// Forme d'un astéroïde : rayon dans chaque direction (repère de l'astéroïde), toujours défini
/// (un rayon depuis le centre ne traverse la surface qu'une fois) : le terrain en colonnes de
/// `terrain.rs` et les collisions s'en servent tels quels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AsteroidShape {
    pub kind: ShapeKind,
    pub class: AsteroidClass,
    /// Rayon moyen (volume équivalent), unités du jeu.
    pub radius: f32,
    pub seed: u32,
    /// Demi-axes relatifs (le plus petit sur z : l'axe de rotation).
    axes: Vec3,
    /// Binaire : rayon du second lobe, écart des centres, position du premier (centre de masse à
    /// l'origine), facteur de volume.
    lobe: f32,
    sep: f32,
    d1: f32,
    norm: f32,
    bumps: f32,
    craters: [(Vec3, f32); MAX_CRATERS],
    n_craters: u8,
    /// Facettes (plans de coupe) : normale et distance relative.
    facets: [(Vec3, f32); MAX_FACETS],
    n_facets: u8,
    /// Rayon maximal / rayon moyen.
    bound: f32,
}

impl AsteroidShape {
    pub fn new(class: AsteroidClass, seed: u32, radius: f32) -> Self {
        let mut r = Rng(mix(seed as u64, 0x5348_4150));
        let roll = r.f32();
        let kind = match class {
            AsteroidClass::M if roll < 0.6 => ShapeKind::Metallic,
            AsteroidClass::M if roll < 0.8 => ShapeKind::Elongated,
            AsteroidClass::M => ShapeKind::Fragment,
            _ if roll < 0.16 => ShapeKind::Binary,
            _ if roll < 0.34 => ShapeKind::Elongated,
            _ if roll < 0.5 => ShapeKind::Fragment,
            _ => ShapeKind::Rubble,
        };
        let mut axes = match kind {
            ShapeKind::Rubble => Vec3::new(1.0, r.range(0.8, 1.0), r.range(0.7, 0.92)),
            ShapeKind::Elongated => Vec3::new(r.range(1.5, 2.3), r.range(0.65, 0.9), r.range(0.5, 0.75)),
            ShapeKind::Metallic => Vec3::new(r.range(1.1, 1.6), r.range(0.8, 1.0), r.range(0.65, 0.85)),
            ShapeKind::Fragment => Vec3::new(r.range(1.0, 1.5), r.range(0.7, 1.0), r.range(0.45, 0.75)),
            ShapeKind::Binary => Vec3::ONE,
        };
        axes /= (axes.x * axes.y * axes.z).cbrt();
        let lobe = r.range(0.45, 0.9);
        let sep = (1.0 + lobe) * 0.78;
        let d1 = sep * lobe.powi(3) / (1.0 + lobe.powi(3));
        let norm = 1.0 / (1.0 + lobe.powi(3)).cbrt();
        let bumps = match kind {
            ShapeKind::Rubble => 0.17,
            ShapeKind::Binary => 0.08,
            ShapeKind::Elongated => 0.1,
            ShapeKind::Metallic => 0.05,
            ShapeKind::Fragment => 0.07,
        };
        let mut craters = [(Vec3::Y, 0.0); MAX_CRATERS];
        let n_craters = (3 + (r.f32() * 11.0) as usize).min(MAX_CRATERS);
        for c in craters.iter_mut().take(n_craters) {
            *c = (r.dir(), r.range(0.12, 0.45) * r.range(0.5, 1.0));
        }
        let mut facets = [(Vec3::Y, 1.0); MAX_FACETS];
        let n_facets = match kind {
            ShapeKind::Fragment => 6 + (r.f32() * 4.0) as usize,
            ShapeKind::Metallic => 4 + (r.f32() * 3.0) as usize,
            _ => 0,
        }
        .min(MAX_FACETS);
        let ellipsoid = |d: Vec3| 1.0 / ((d / axes).length()).max(1e-4);
        for f in facets.iter_mut().take(n_facets) {
            let n = r.dir();
            *f = (n, ellipsoid(n) * r.range(0.72, 0.92));
        }
        let extent = match kind {
            ShapeKind::Binary => (d1 + 1.0).max(sep - d1 + lobe) * norm,
            _ => axes.max_element(),
        };
        let bound = extent * (1.0 + bumps * 1.3) * 1.08;
        Self { kind, class, radius, seed, axes, lobe, sep, d1, norm, bumps, craters, n_craters: n_craters as u8, facets, n_facets: n_facets as u8, bound }
    }

    /// La même forme à un autre rayon.
    pub fn with_radius(mut self, radius: f32) -> Self {
        self.radius = radius;
        self
    }

    /// Plus grand rayon possible (sphère englobante).
    pub fn max_radius(&self) -> f32 {
        self.radius * self.bound
    }

    /// Rayon de la surface dans la direction `dir` (unitaire, repère de l'astéroïde).
    pub fn radius_at(&self, dir: Vec3) -> f32 {
        self.radius * self.unit_at(dir).0
    }

    /// Rayon relatif et éclat du sol (fond des cratères plus sombre, remparts plus clairs).
    fn unit_at(&self, d: Vec3) -> (f32, f32) {
        let mut r = match self.kind {
            ShapeKind::Binary => {
                // Rayon le plus lointain des deux sphères vues depuis le centre de masse
                let far = |c: f32, rr: f32| {
                    let b = d.x * c;
                    let disc = b * b - c * c + rr * rr;
                    if disc >= 0.0 { (b + disc.sqrt()).max(0.0) } else { 0.0 }
                };
                far(-self.d1, 1.0).max(far(self.sep - self.d1, self.lobe)) * self.norm
            }
            _ => 1.0 / (d / self.axes).length().max(1e-4),
        };
        for &(n, o) in self.facets.iter().take(self.n_facets as usize) {
            let c = d.dot(n);
            if c > 0.05 {
                r = r.min(o / c);
            }
        }
        let s = self.seed;
        r *= 1.0 + self.bumps * (fbm(d * 2.3, s, 3) * 2.0 - 1.0) + self.bumps * 0.3 * (vnoise(d * 9.0, s ^ 0x77) * 2.0 - 1.0);
        let mut shade = 0.0;
        for &(c, a) in self.craters.iter().take(self.n_craters as usize) {
            let cos = d.dot(c);
            if cos < (a * 1.5).min(3.0).cos() {
                continue;
            }
            let t = cos.clamp(-1.0, 1.0).acos() / a;
            let depth = 0.22 * a;
            let rim = depth * 0.4 * (-((t - 1.0) / 0.22).powi(2)).exp();
            let bowl = if t < 1.0 { -(1.0 - t * t) * depth } else { 0.0 };
            r *= 1.0 + bowl + rim;
            shade += bowl * 1.5 + rim * 2.0;
        }
        (r.max(0.25), shade)
    }

    /// Couleur du sol dans la direction `dir` (sRGB, comme les tuiles du terrain).
    pub fn color_at(&self, dir: Vec3) -> [f32; 4] {
        let base = self.class.color();
        let (_, shade) = self.unit_at(dir);
        let v = 0.84 + 0.32 * fbm(dir * 4.0, self.seed ^ 0x51, 2) + shade.clamp(-0.25, 0.25);
        let mut c = [base[0] * v, base[1] * v, base[2] * v];
        // Glace : plaques claires ; métal : reflets plus froids
        if self.class == AsteroidClass::Ice && vnoise(dir * 6.0, self.seed ^ 0x1CE) > 0.62 {
            c = [c[0] * 1.15, c[1] * 1.15, c[2] * 1.18];
        }
        [c[0].clamp(0.02, 1.0), c[1].clamp(0.02, 1.0), c[2].clamp(0.02, 1.0), 1.0]
    }
}

/// Maillage d'une forme : sphère-cube de `n` × `n` quads par face, déformée (normales calculées
/// sur la forme elle-même : pas de couture entre les faces).
pub fn build_mesh(shape: &AsteroidShape, n: usize) -> Mesh {
    let n = n.max(2);
    let verts = 6 * (n + 1) * (n + 1);
    let (mut pos, mut nor, mut col) = (Vec::with_capacity(verts), Vec::with_capacity(verts), Vec::with_capacity(verts));
    let mut idx: Vec<u32> = Vec::with_capacity(6 * n * n * 6);
    let point = |face: u8, s: f32, t: f32| {
        let d = crate::terrain::face_dir(face, s, t);
        d * shape.radius_at(d)
    };
    let h = 0.5 / n as f32;
    for face in 0..6u8 {
        let base = pos.len() as u32;
        for j in 0..=n {
            for i in 0..=n {
                let s = -1.0 + 2.0 * i as f32 / n as f32;
                let t = -1.0 + 2.0 * j as f32 / n as f32;
                let d = crate::terrain::face_dir(face, s, t);
                let p = d * shape.radius_at(d);
                let mut normal = (point(face, s + h, t) - point(face, s - h, t)).cross(point(face, s, t + h) - point(face, s, t - h)).normalize_or(d);
                if normal.dot(d) < 0.0 {
                    normal = -normal;
                }
                pos.push(p.to_array());
                nor.push(normal.to_array());
                col.push(shape.color_at(d));
            }
        }
        // Sens des triangles : vers l'extérieur
        let (a, b, c) = (crate::terrain::face_dir(face, -1.0, -1.0), crate::terrain::face_dir(face, 1.0, -1.0), crate::terrain::face_dir(face, -1.0, 1.0));
        let outward = (b - a).cross(c - a).dot(a) > 0.0;
        let row = (n + 1) as u32;
        for j in 0..n as u32 {
            for i in 0..n as u32 {
                let v00 = base + j * row + i;
                let (v10, v01, v11) = (v00 + 1, v00 + row, v00 + row + 1);
                if outward {
                    idx.extend_from_slice(&[v00, v10, v01, v10, v11, v01]);
                } else {
                    idx.extend_from_slice(&[v00, v01, v10, v10, v01, v11]);
                }
            }
        }
    }
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, nor);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
    mesh.insert_indices(Indices::U32(idx));
    mesh
}

// ─────────────────────────────────────────────────────────────────────────
//  Cellules et astéroïdes
// ─────────────────────────────────────────────────────────────────────────

/// Un niveau de cellules : taille, portée autour de la caméra, tailles d'astéroïdes, nombre moyen
/// par cellule (multiplié dans les champs denses), distance de vue (`sight` × rayon + `sight_base`).
pub struct Level {
    pub cell: f64,
    pub reach: f64,
    pub min_r: f32,
    pub max_r: f32,
    pub per_cell: f64,
    pub dense: f64,
    pub sight: f32,
    pub sight_base: f32,
}

/// Cailloux, rochers, gros astéroïdes (on peut se poser sur ces derniers).
pub const LEVELS: [Level; 3] = [
    Level { cell: 400.0, reach: 1_000.0, min_r: 0.4, max_r: 3.0, per_cell: 0.35, dense: 4.0, sight: 300.0, sight_base: 150.0 },
    Level { cell: 4_000.0, reach: 16_000.0, min_r: 3.0, max_r: 30.0, per_cell: 0.25, dense: 4.0, sight: 500.0, sight_base: 2_000.0 },
    Level { cell: 50_000.0, reach: 320_000.0, min_r: 30.0, max_r: 450.0, per_cell: 0.06, dense: 2.5, sight: 1_500.0, sight_base: 20_000.0 },
];

/// Niveau des astéroïdes où l'on peut se poser.
pub const LANDABLE_LEVEL: u8 = 2;

/// Formes partagées des petits astéroïdes (par type).
const VARIANTS: u32 = 10;

/// Échelle des champs denses (amas de la ceinture).
const CLUMP: f64 = 300_000.0;

/// Paramètre gravitationnel des orbites (le même que celui des planètes).
pub fn mu() -> f64 {
    (crate::kepler::DEFAULT_MU * crate::planet::PLANET_MU_SCALE) as f64
}

/// Identifiant d'un astéroïde : sa cellule et sa place dans la cellule.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AsteroidKey {
    pub sys: u32,
    pub belt: u8,
    pub level: u8,
    pub ring: u32,
    pub sector: u32,
    pub layer: i32,
    pub slot: u8,
}

/// Codes de source dans `AsteroidKey::belt` : ceintures (0..), Troyens, anneaux, comètes.
pub const SWARM_BASE: u8 = 100;
pub const RING_BASE: u8 = 200;
pub const COMET_SOURCE: u8 = 250;

/// D'où vient un astéroïde.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Belt,
    Trojan,
    Ring,
    Comet,
}

impl AsteroidKey {
    pub fn source(&self) -> Source {
        match self.belt {
            b if b >= COMET_SOURCE => Source::Comet,
            b if b >= RING_BASE => Source::Ring,
            b if b >= SWARM_BASE => Source::Trojan,
            _ => Source::Belt,
        }
    }
}

/// Orbite de Kepler en f64 (même formule que `kepler::OrbitalElements`, même `mu`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Elements {
    pub a: f64,
    pub e: f64,
    pub i: f64,
    pub node: f64,
    pub peri: f64,
    pub m0: f64,
    /// Planète errante : elle ne tourne pas autour de l'étoile.
    pub frozen: bool,
}

impl Elements {
    pub fn of_planet(p: &PlanetConfig) -> Self {
        Self {
            a: p.orbit_distance as f64,
            e: p.eccentricity as f64,
            i: p.inclination as f64,
            node: p.ascending_node as f64,
            peri: p.arg_periapsis as f64,
            m0: p.mean_anomaly_0 as f64,
            frozen: p.rogue,
        }
    }

    pub fn of_comet(c: &Comet) -> Self {
        Self { a: c.a, e: c.e, i: c.inc, node: c.node, peri: c.peri, m0: c.m0, frozen: false }
    }

    pub fn mean_motion(&self) -> f64 {
        (mu() / self.a.max(1.0).powi(3)).sqrt()
    }

    /// Période (secondes de jeu).
    pub fn period(&self) -> f64 {
        TAU / self.mean_motion()
    }

    /// Position (depuis l'étoile) à l'instant `t`, en avance de `lead` (anomalie moyenne).
    pub fn position(&self, t: f64, lead: f64) -> DVec3 {
        let motion = if self.frozen { 0.0 } else { self.mean_motion() * t };
        let m = (self.m0 + lead + motion).rem_euclid(TAU);
        let e = self.e.clamp(0.0, 0.999);
        // Newton (départ à π pour les orbites très allongées)
        let mut ea = if e > 0.8 { std::f64::consts::PI } else { m };
        for _ in 0..40 {
            let d = (ea - e * ea.sin() - m) / (1.0 - e * ea.cos());
            ea -= d;
            if d.abs() < 1e-12 {
                break;
            }
        }
        let x = self.a * (ea.cos() - e);
        let y = self.a * (1.0 - e * e).sqrt() * ea.sin();
        let (cw, sw) = (self.peri.cos(), self.peri.sin());
        let (co, so) = (self.node.cos(), self.node.sin());
        let (ci, si) = (self.i.cos(), self.i.sin());
        let px = (co * cw - so * sw * ci) * x + (-co * sw - so * cw * ci) * y;
        let py = (so * cw + co * sw * ci) * x + (-so * sw + co * cw * ci) * y;
        let pz = (sw * si) * x + (cw * si) * y;
        DVec3::new(px, pz, py)
    }

    /// Repère du point en avance de `lead` : position, puis axes radial, normal, le long de l'orbite.
    pub fn frame(&self, t: f64, lead: f64) -> (DVec3, DVec3, DVec3, DVec3) {
        let p = self.position(t, lead);
        let r = p.normalize_or(DVec3::X);
        let dt = self.period() * 1e-4;
        let v = self.position(t + dt, lead) - self.position(t - dt, lead);
        let along = (v - r * v.dot(r)).normalize_or(r.any_orthonormal_vector());
        (p, r, r.cross(along), along)
    }
}

/// Anneau d'une planète vu par le champ d'astéroïdes (ses particules).
#[derive(Clone, Copy, Debug)]
pub struct RingSrc {
    pub planet: u8,
    pub ring: Ring,
    pub el: Elements,
    pub spin: Spin,
    pub radius: f32,
}

impl RingSrc {
    /// Centre de la planète et orientation du plan de l'anneau (y = axe de la planète) à `t`.
    pub fn plane(&self, t: f64) -> (DVec3, Quat) {
        let p = self.el.position(t, 0.0);
        let axis = self.spin.rotation(t, -p.as_vec3()) * Vec3::Y;
        (p, Quat::from_rotation_arc(Vec3::Y, axis))
    }

    /// Demi-épaisseur de l'anneau (unités).
    pub fn half_thickness(&self) -> f64 {
        ((self.ring.outer - self.ring.inner) as f64 * 0.004).max(30.0)
    }
}

/// Tout ce qui donne des astéroïdes dans un système : ceintures, Troyens, anneaux, comètes.
#[derive(Clone, Debug, Default)]
pub struct Sources {
    pub belts: Vec<Belt>,
    /// Essaim, orbite de sa géante, distance réelle (UA).
    pub swarms: Vec<(Swarm, Elements, f32)>,
    pub rings: Vec<RingSrc>,
    pub comets: Vec<Comet>,
}

impl Sources {
    /// Seulement la source de `key` (calcul léger, appelé souvent).
    pub fn for_key(sys: &StarSystemConfig, key: &AsteroidKey) -> Self {
        match key.source() {
            Source::Belt => Self { belts: sys.belts(), ..Default::default() },
            Source::Comet => Self { comets: sys.comets(), ..Default::default() },
            _ => Self::of(sys),
        }
    }

    pub fn of(sys: &StarSystemConfig) -> Self {
        let planets = sys.planets_uncached();
        Self {
            belts: sys.belts(),
            swarms: sys
                .swarms()
                .into_iter()
                .filter_map(|s| planets.get(s.planet as usize).map(|p| (s, Elements::of_planet(p), p.semi_major_au)))
                .collect(),
            rings: planets
                .iter()
                .enumerate()
                .filter_map(|(i, p)| p.ring.map(|ring| RingSrc { planet: i as u8, ring, el: Elements::of_planet(p), spin: Spin::planet(p), radius: p.radius }))
                .collect(),
            comets: sys.comets(),
        }
    }
}

/// Trajectoire d'un astéroïde : tout est une fonction de l'horloge (règle 9).
#[derive(Clone, Copy, Debug)]
enum Path {
    /// Ceinture : anneau de Kepler autour de l'étoile, petit épicycle.
    Belt { rho0: f64, theta0: f64, y0: f64, n: f64, epi: f64, epi_y: f64, ph: f64, ph_y: f64 },
    /// Troyen : décalage fixe (radial, normal, le long de l'orbite) autour du point L4 / L5.
    Lagrange { el: Elements, lead: f64, off: DVec3 },
    /// Comète : son orbite.
    Comet { el: Elements, comet: Comet },
    /// Particule d'anneau : orbite circulaire autour de sa planète, dans son plan équatorial.
    Ring { src: RingSrc, rho: f64, theta0: f64, h: f64, n: f64 },
}

/// Un astéroïde, tout entier calculé depuis sa clé.
#[derive(Clone, Copy, Debug)]
pub struct Asteroid {
    pub key: AsteroidKey,
    pub shape: AsteroidShape,
    /// Forme partagée (petits astéroïdes) : `Some(numéro)`.
    pub variant: Option<u32>,
    /// Distance à l'étoile en UA (température).
    pub au: f32,
    path: Path,
    axis: Vec3,
    spin: f64,
    spin0: f64,
}

/// Paramètre gravitationnel des orbites autour d'une planète (le même que celui des lunes).
pub fn planet_mu() -> f64 {
    (crate::kepler::DEFAULT_MU * 0.001) as f64
}

impl Asteroid {
    pub fn class(&self) -> AsteroidClass {
        self.shape.class
    }

    pub fn landable(&self) -> bool {
        self.key.level == LANDABLE_LEVEL
    }

    /// La comète, si c'en est une.
    pub fn comet(&self) -> Option<&Comet> {
        match &self.path {
            Path::Comet { comet, .. } => Some(comet),
            _ => None,
        }
    }

    /// Orbite (comète), pour la période.
    pub fn elements(&self) -> Option<Elements> {
        match self.path {
            Path::Comet { el, .. } | Path::Lagrange { el, .. } => Some(el),
            _ => None,
        }
    }

    /// Position (depuis le centre du système) à l'instant `t`.
    pub fn rel_position(&self, t: f64) -> DVec3 {
        match self.path {
            Path::Belt { rho0, theta0, y0, n, epi, epi_y, ph, ph_y } => {
                let m = n * t;
                let rho = rho0 + epi * (m + ph).cos();
                let theta = theta0 + m + 2.0 * epi / rho0 * (m + ph).sin();
                DVec3::new(rho * theta.cos(), y0 + epi_y * (m + ph_y).sin(), rho * theta.sin())
            }
            Path::Lagrange { el, lead, off } => {
                let (p, r, n, along) = el.frame(t, lead);
                p + r * off.x + n * off.y + along * off.z
            }
            Path::Comet { el, .. } => el.position(t, 0.0),
            Path::Ring { src, rho, theta0, h, n } => {
                let (p, q) = src.plane(t);
                let th = theta0 + n * t;
                p + q.as_dquat() * DVec3::new(rho * th.cos(), h, rho * th.sin())
            }
        }
    }

    /// Orientation à l'instant `t` : rotation propre autour du plus petit axe (z de la forme).
    pub fn rotation(&self, t: f64) -> Quat {
        let angle = (self.spin0 + self.spin * t).rem_euclid(TAU) as f32;
        Quat::from_axis_angle(self.axis, angle) * Quat::from_rotation_arc(Vec3::Z, self.axis)
    }

    /// Période de rotation (secondes de jeu).
    pub fn spin_period(&self) -> f64 {
        TAU / self.spin.abs().max(1e-9)
    }

    /// Masse (tonnes), d'après le rayon réel (km) et la masse volumique du type.
    pub fn mass_t(&self) -> f64 {
        let r_m = crate::planetgen::units::game_to_km(self.shape.radius as f64) * 1000.0;
        4.0 / 3.0 * std::f64::consts::PI * r_m.powi(3) * self.class().density() * 1000.0 / 1000.0
    }

    /// Gravité de surface (g).
    pub fn gravity_g(&self) -> f64 {
        let r_m = crate::planetgen::units::game_to_km(self.shape.radius as f64) * 1000.0;
        6.674e-11 * self.mass_t() * 1000.0 / (r_m * r_m) / 9.807
    }

    /// Minerais (tonnes), du plus abondant au plus rare.
    pub fn ores(&self) -> Vec<(crate::planetgen::resources::Ore, f64)> {
        let m = self.mass_t();
        self.class().ores().iter().map(|&(o, f)| (o, m * f)).collect()
    }

    /// Paramètres de terrain pour se poser dessus (microgravité, voxels 3D, pas d'air).
    pub fn body_params(&self, lum: f64) -> crate::terrain::BodyParams {
        use crate::planetgen::climate::Climate;
        let temp = Belt::temperature_c(self.au as f64, lum) as f32;
        crate::terrain::BodyParams {
            airless: true,
            atmosphere: false,
            radius: self.shape.radius,
            sea_level: 0.5,
            terrain_height: self.shape.radius * 0.3,
            seed: self.shape.seed,
            noise_scale: 2.0,
            detail_scale: 4.0,
            temperature: temp,
            gravity: self.gravity_g() as f32,
            gaseous: false,
            climate: Climate::from_mean(temp, false),
            sky: [0.0; 3],
            sunset: [0.0; 3],
            haze: [0.0; 3],
            pressure: 0.0,
            hydro: crate::planetgen::hydrology::Hydro::DRY,
            relief: Default::default(),
            biomes: Default::default(),
            asteroid: Some(self.shape),
            tide: Default::default(),
        }
    }

    /// Une ligne de description (HUD, scanner).
    pub fn title(&self) -> String {
        let km = crate::planetgen::units::game_to_km(self.shape.radius as f64) * 2.0;
        match self.key.source() {
            Source::Comet => format!("Comete - noyau de {:.0} km", km),
            Source::Trojan => format!("Troyen {} - {}, {:.0} km", self.class().name(), self.shape.kind.name(), km),
            Source::Ring => format!("Bloc de l'anneau, {:.1} km", km),
            Source::Belt => format!("Asteroide {} - {}, {:.0} km", self.class().name(), self.shape.kind.name(), km),
        }
    }
}

/// Masse en tonnes, lisible.
fn tonnes(t: f64) -> String {
    match t {
        t if t >= 1e12 => format!("{:.1e} t", t),
        t if t >= 1e9 => format!("{:.1} Gt", t / 1e9),
        t if t >= 1e6 => format!("{:.1} Mt", t / 1e6),
        t if t >= 1e3 => format!("{:.0} kt", t / 1e3),
        t => format!("{:.0} t", t),
    }
}

/// Texte du scanner (touche I) pour un astéroïde ou une comète.
pub fn scanner_text(a: &Asteroid, src: Option<&Sources>, lum: f64) -> String {
    use crate::planetgen::units::game_to_km;
    let what = match a.key.source() {
        Source::Comet => "Comete",
        Source::Trojan => "Troyen",
        _ => "Asteroide",
    };
    let mut lines = vec![format!("SCANNER  -  {what}")];
    lines.push(format!("Type : {}   Forme : {}", a.class().name(), a.shape.kind.name()));
    lines.push(format!("Diametre {:.0} km   Masse {}   Gravite {:.4} g", game_to_km(a.shape.radius as f64) * 2.0, tonnes(a.mass_t()), a.gravity_g()));
    // 1 h de l'astre = 1 min de jeu (A1)
    lines.push(format!("Rotation {:.1} h   Temperature {:.0} C   Pas d'air", a.spin_period() / 60.0, Belt::temperature_c(a.au as f64, lum)));
    match (a.key.source(), src) {
        (Source::Belt, Some(s)) => {
            if let Some(b) = s.belts.get(a.key.belt as usize) {
                lines.push(format!("{} : {:.2} a {:.2} UA, ~{:.1e} asteroides de plus d'1 km", b.kind.name(), b.au_inner, b.au_outer, b.count_over_km()));
            }
        }
        (Source::Trojan, Some(s)) => {
            if let Some((sw, _, au)) = s.swarms.get((a.key.belt - SWARM_BASE) as usize) {
                lines.push(format!("{} de la planete {} ({:.2} UA)", sw.name(), sw.planet + 1, au));
            }
        }
        (Source::Comet, _) => {
            if let (Some(c), Some(el)) = (a.comet(), a.elements()) {
                lines.push(format!("{} : perihelie {:.2} UA, excentricite {:.2}, un tour en {:.0} h de jeu", c.name(), c.q_au, c.e, el.period() / 3600.0));
            }
        }
        _ => {}
    }
    let mut ores = a.ores();
    ores.sort_by(|x, y| y.1.total_cmp(&x.1));
    let list: Vec<String> = ores.iter().take(4).map(|(o, t)| format!("{} {}", o.name(), tonnes(*t))).collect();
    lines.push(format!("Minerais : {}", list.join(", ")));
    lines.push(if a.landable() { "V : se poser (microgravite)".to_string() } else { "Trop petit pour s'y poser".to_string() });
    lines.push("(I : masquer)".into());
    lines.join("\n")
}

/// Ligne des ceintures d'un système (scanner de l'étoile) : ceintures, Troyens, comètes.
pub fn belts_line(src: &Sources) -> Option<String> {
    let mut list: Vec<String> = src.belts.iter().map(|b| format!("{} {:.2}-{:.2} UA", b.kind.name(), b.au_inner, b.au_outer)).collect();
    let giants: std::collections::BTreeSet<u8> = src.swarms.iter().map(|(s, _, _)| s.planet + 1).collect();
    if !giants.is_empty() {
        list.push(format!("Troyens (planete {})", giants.iter().map(|g| g.to_string()).collect::<Vec<_>>().join(", ")));
    }
    if !src.comets.is_empty() {
        list.push(format!("{} comete(s)", src.comets.len()));
    }
    (!list.is_empty()).then(|| format!("Petits corps : {}", list.join(" ; ")))
}

/// Densité de la ceinture au point (rayon, angle à l'instant 0, hauteur) : (base, champ dense 0..1).
pub fn field_density(belt: &Belt, rho: f64, theta0: f64, y: f64) -> (f64, f64) {
    let width = (belt.outer - belt.inner) as f64;
    let f = (rho - belt.inner as f64) / width.max(1.0);
    if !(0.0..=1.0).contains(&f) {
        return (0.0, 0.0);
    }
    let h = (belt.half_thickness as f64).max(1.0);
    if y.abs() > 3.0 * h {
        return (0.0, 0.0);
    }
    let edge = smoothstep(0.0, 0.12, f) * smoothstep(1.0, 0.88, f);
    let vertical = (-(y / h).powi(2)).exp();
    // Amas (repère qui tourne avec la ceinture, sans couture à l'angle 0)
    let p = Vec3::new((rho * theta0.cos() / CLUMP) as f32, (y / CLUMP * 2.0) as f32, (rho * theta0.sin() / CLUMP) as f32);
    let v = fbm(p, belt.seed, 2) as f64;
    let dense = smoothstep(0.6, 0.76, v);
    (belt.density as f64 * edge * vertical * (0.3 + 0.9 * v), dense)
}

fn sectors(rho: f64, cell: f64) -> u32 {
    ((TAU * rho / cell).floor() as u64).clamp(1, u32::MAX as u64) as u32
}

/// Rayon tiré selon une loi de puissance (beaucoup de petits, peu de gros).
fn power_radius(lo: f32, hi: f32, u: f32) -> f32 {
    let a = 1.6f32;
    let k = 1.0 - (hi / lo).powf(-a);
    (lo * (1.0 - u * k).powf(-1.0 / a)).min(hi)
}

/// Nombre d'astéroïdes d'une cellule de moyenne `lambda` (au plus 12).
fn draw_count(rng: &mut Rng, lambda: f64) -> usize {
    if lambda <= 0.0 {
        return 0;
    }
    (lambda.floor() as usize + (rng.f64() < lambda.fract()) as usize).min(12)
}

/// Forme, rotation propre : la partie commune à toutes les sources.
fn spin_and_shape(r: &mut Rng, level: u8, class: AsteroidClass, radius: f32) -> (AsteroidShape, Option<u32>, f64) {
    let shape_seed = r.next() as u32;
    let (shape, variant) = if level < LANDABLE_LEVEL {
        let v = shape_seed % VARIANTS;
        (variant_shape(class, v).with_radius(radius), Some(v))
    } else {
        (AsteroidShape::new(class, shape_seed, radius), None)
    };
    // Rotation propre : 2 à 20 h pour les gros (1 h = 1 min de jeu), plus vite les petits
    let (p_lo, p_hi) = match level {
        0 => (20.0, 90.0),
        1 => (40.0, 300.0),
        _ => (120.0, 1200.0),
    };
    let period = p_lo + (p_hi - p_lo) * r.f64();
    let spin = TAU / period * if r.f64() < 0.1 { -1.0 } else { 1.0 };
    (shape, variant, spin)
}

/// Les astéroïdes d'une cellule d'une ceinture (même résultat sur toutes les machines).
pub fn cell_asteroids(sys: u32, bi: u8, belt: &Belt, level: u8, ring: u32, sector: u32, layer: i32) -> Vec<Asteroid> {
    let lv = &LEVELS[level as usize];
    let rho_lo = belt.inner as f64 + ring as f64 * lv.cell;
    if rho_lo >= belt.outer as f64 {
        return Vec::new();
    }
    let rho_mid = rho_lo + lv.cell * 0.5;
    let s = sectors(rho_mid, lv.cell);
    if sector >= s {
        return Vec::new();
    }
    let dt = TAU / s as f64;
    let base = mix(mix(mix(belt.seed as u64 ^ ((level as u64) << 40), ring as u64), sector as u64), layer as i64 as u64);
    let (dens, dense) = field_density(belt, rho_mid, (sector as f64 + 0.5) * dt, (layer as f64 + 0.5) * lv.cell);
    let mut rng = Rng(base);
    let count = draw_count(&mut rng, lv.per_cell * dens * (1.0 + lv.dense * dense));
    let n = (mu() / rho_mid.powi(3)).sqrt();
    let width = (belt.outer - belt.inner).max(1.0);
    (0..count)
        .map(|slot| {
            let mut r = Rng(mix(base, slot as u64 + 1));
            let rho0 = rho_lo + r.f64() * lv.cell;
            let theta0 = (sector as f64 + r.f64()) * dt;
            let y0 = (layer as f64 + r.f64()) * lv.cell;
            let radius = power_radius(lv.min_r, lv.max_r, r.f32());
            let frac = ((rho0 - belt.inner as f64) as f32 / width).clamp(0.0, 1.0);
            let class = belt.class_at(frac, r.f32());
            let (shape, variant, spin) = spin_and_shape(&mut r, level, class, radius);
            let path = Path::Belt {
                rho0,
                theta0,
                y0,
                n,
                epi: lv.cell * 0.06 * r.f64(),
                epi_y: lv.cell * 0.06 * r.f64(),
                ph: TAU * r.f64(),
                ph_y: TAU * r.f64(),
            };
            Asteroid {
                key: AsteroidKey { sys, belt: bi, level, ring, sector, layer, slot: slot as u8 },
                shape,
                variant,
                au: belt.au_at(frac),
                path,
                axis: r.dir(),
                spin,
                spin0: TAU * r.f64(),
            }
        })
        .collect()
}

/// Demi-axes (radial, normal, le long de l'orbite) d'un essaim de Troyens.
fn swarm_axes(sw: &Swarm, el: &Elements) -> DVec3 {
    let along = sw.spread as f64 * el.a;
    DVec3::new(along * 0.2, along * 0.2, along)
}

/// Les Troyens d'une cellule (repère du point de Lagrange, cellules cubiques).
fn swarm_cell(sys: u32, k: u8, sw: &Swarm, el: &Elements, au: f32, level: u8, i: i32, j: i32, l: i32) -> Vec<Asteroid> {
    let lv = &LEVELS[level as usize];
    let c = DVec3::new(i as f64 + 0.5, j as f64 + 0.5, l as f64 + 0.5) * lv.cell;
    let ax = swarm_axes(sw, el);
    let d = (c / ax).length_squared();
    if d > 9.0 {
        return Vec::new();
    }
    let base = mix(mix(mix(sw.seed as u64 ^ ((level as u64) << 40), i as i64 as u64), j as i64 as u64), l as i64 as u64);
    let mut rng = Rng(base);
    let count = draw_count(&mut rng, lv.per_cell * 1.5 * sw.density as f64 * (-d).exp());
    let lead = if sw.leading { TAU / 6.0 } else { -TAU / 6.0 };
    (0..count)
        .map(|slot| {
            let mut r = Rng(mix(base, slot as u64 + 1));
            let off = (DVec3::new(i as f64, j as f64, l as f64) + DVec3::new(r.f64(), r.f64(), r.f64())) * lv.cell;
            let radius = power_radius(lv.min_r, lv.max_r, r.f32());
            let class = sw.class_at(r.f32());
            let (shape, variant, spin) = spin_and_shape(&mut r, level, class, radius);
            Asteroid {
                key: AsteroidKey { sys, belt: SWARM_BASE + k, level, ring: i as u32, sector: j as u32, layer: l, slot: slot as u8 },
                shape,
                variant,
                au,
                path: Path::Lagrange { el: *el, lead, off },
                axis: r.dir(),
                spin,
                spin0: TAU * r.f64(),
            }
        })
        .collect()
}

/// Particules d'une cellule d'un anneau (repère qui tourne avec l'anneau, comme une ceinture).
fn ring_cell(sys: u32, k: u8, rs: &RingSrc, ring: u32, sector: u32, layer: i32) -> Vec<Asteroid> {
    let cell = RING_CELL;
    let (inner, outer) = (rs.ring.inner as f64, rs.ring.outer as f64);
    let rho_lo = inner + ring as f64 * cell;
    if rho_lo >= outer {
        return Vec::new();
    }
    let rho_mid = rho_lo + cell * 0.5;
    let s = sectors(rho_mid, cell);
    if sector >= s {
        return Vec::new();
    }
    let dt = TAU / s as f64;
    let f = ((rho_mid - inner) / (outer - inner)) as f32;
    let base = mix(mix(mix(rs.ring.seed as u64 ^ 0x5249, ring as u64), sector as u64), layer as i64 as u64);
    let mut rng = Rng(base);
    let count = draw_count(&mut rng, RING_PER_CELL * rs.ring.profile(f) as f64);
    let n = (planet_mu() / rho_mid.powi(3)).sqrt();
    let h = rs.half_thickness();
    (0..count)
        .map(|slot| {
            let mut r = Rng(mix(base, slot as u64 + 1));
            let rho = rho_lo + r.f64() * cell;
            let theta0 = (sector as f64 + r.f64()) * dt;
            let y = ((layer as f64 + r.f64()) * cell).clamp(-h, h);
            let radius = power_radius(RING_SIZES.0, RING_SIZES.1, r.f32());
            let class = if r.f32() < rs.ring.ice { AsteroidClass::Ice } else if r.f32() < 0.5 { AsteroidClass::C } else { AsteroidClass::S };
            let (shape, variant, spin) = spin_and_shape(&mut r, 0, class, radius);
            Asteroid {
                key: AsteroidKey { sys, belt: RING_BASE + k, level: 0, ring, sector, layer, slot: slot as u8 },
                shape,
                variant,
                au: 0.0,
                path: Path::Ring { src: *rs, rho, theta0, h: y, n },
                axis: r.dir(),
                spin,
                spin0: TAU * r.f64(),
            }
        })
        .collect()
}

/// Cellules des anneaux de particules : taille, nombre moyen là où l'anneau est opaque, tailles.
const RING_CELL: f64 = 400.0;
const RING_PER_CELL: f64 = 2.5;
const RING_SIZES: (f32, f32) = (0.3, 2.5);

/// Le noyau d'une comète.
fn comet_asteroid(sys: u32, idx: usize, c: &Comet) -> Asteroid {
    let mut r = Rng(mix(c.seed as u64, 0xC0E7));
    let shape = AsteroidShape::new(AsteroidClass::Ice, c.seed, c.radius);
    let period = 120.0 + 600.0 * r.f64();
    Asteroid {
        key: AsteroidKey { sys, belt: COMET_SOURCE, level: LANDABLE_LEVEL, ring: idx as u32, sector: 0, layer: 0, slot: 0 },
        shape,
        variant: None,
        au: c.q_au * 2.0,
        path: Path::Comet { el: Elements::of_comet(c), comet: *c },
        axis: r.dir(),
        spin: TAU / period,
        spin0: TAU * r.f64(),
    }
}

/// Forme partagée `v` des petits astéroïdes d'un type (rayon 1).
pub fn variant_shape(class: AsteroidClass, v: u32) -> AsteroidShape {
    AsteroidShape::new(class, 0xA570_0000 ^ ((class as u32) << 8) ^ v, 1.0)
}

/// L'astéroïde `key`, recalculé.
pub fn find(src: &Sources, key: AsteroidKey) -> Option<Asteroid> {
    let list = match key.source() {
        Source::Belt => {
            let belt = src.belts.get(key.belt as usize)?;
            cell_asteroids(key.sys, key.belt, belt, key.level, key.ring, key.sector, key.layer)
        }
        Source::Trojan => {
            let k = key.belt - SWARM_BASE;
            let (sw, el, au) = src.swarms.get(k as usize)?;
            swarm_cell(key.sys, k, sw, el, *au, key.level, key.ring as i32, key.sector as i32, key.layer)
        }
        Source::Ring => {
            let k = key.belt - RING_BASE;
            ring_cell(key.sys, k, src.rings.get(k as usize)?, key.ring, key.sector, key.layer)
        }
        Source::Comet => return src.comets.get(key.ring as usize).map(|c| comet_asteroid(key.sys, key.ring as usize, c)),
    };
    list.into_iter().nth(key.slot as usize)
}

/// Cellules d'un anneau (ceinture ou anneau de planète) autour de `rel` (repère de l'anneau :
/// plan y = 0, centre à l'origine) : (anneau, secteur, couche).
#[allow(clippy::too_many_arguments)]
fn annulus_cells(inner: f64, outer: f64, half: f64, mu: f64, cell: f64, rel: DVec3, t: f64, reach: f64, mut visit: impl FnMut(u32, u32, i32)) {
    let rho_c = (rel.x * rel.x + rel.z * rel.z).sqrt();
    let d_rho = (inner - rho_c).max(rho_c - outer).max(0.0);
    let d_y = (rel.y.abs() - half).max(0.0);
    if d_rho * d_rho + d_y * d_y > reach * reach {
        return;
    }
    let phi = rel.z.atan2(rel.x);
    let rings = (((outer - inner) / cell).ceil() as i64).max(1);
    let r0 = ((rho_c - reach - inner) / cell).floor().max(0.0) as i64;
    let r1 = (((rho_c + reach - inner) / cell).floor() as i64).min(rings - 1);
    let l0 = ((rel.y - reach) / cell).floor().max((-half / cell).floor()) as i64;
    let l1 = ((rel.y + reach) / cell).floor().min((half / cell).floor()) as i64;
    let margin = reach + cell * 1.1;
    for ring in r0..=r1 {
        let rho_mid = inner + (ring as f64 + 0.5) * cell;
        let s = sectors(rho_mid, cell) as i64;
        let dt = TAU / s as f64;
        let n = (mu / rho_mid.powi(3)).sqrt();
        let epoch = (phi - n * t).rem_euclid(TAU);
        let center = (epoch / dt).floor() as i64;
        let span = (reach / (rho_mid * dt)).ceil() as i64 + 1;
        let list: Vec<i64> = if 2 * span + 1 >= s { (0..s).collect() } else { (center - span..=center + span).map(|k| k.rem_euclid(s)).collect() };
        for sector in list {
            for layer in l0..=l1 {
                // La cellule (son centre à l'instant t) est-elle assez près ?
                let th = (sector as f64 + 0.5) * dt + n * t;
                let c = DVec3::new(rho_mid * th.cos(), (layer as f64 + 0.5) * cell, rho_mid * th.sin());
                if c.distance_squared(rel) <= margin * margin {
                    visit(ring as u32, sector as u32, layer as i32);
                }
            }
        }
    }
}

/// Astéroïdes d'un niveau d'une ceinture autour du point `rel` (depuis le centre du système) à
/// l'instant `t`, plus près que `reach`.
#[allow(clippy::too_many_arguments)]
pub fn near_belt(sys: u32, bi: u8, belt: &Belt, level: u8, rel: DVec3, t: f64, reach: f64, out: &mut Vec<Asteroid>) {
    let cell = LEVELS[level as usize].cell;
    annulus_cells(belt.inner as f64, belt.outer as f64, belt.half_thickness as f64 * 3.0, mu(), cell, rel, t, reach, |ring, sector, layer| {
        out.extend(cell_asteroids(sys, bi, belt, level, ring, sector, layer));
    });
}

/// Troyens d'un niveau d'un essaim autour de `rel`.
#[allow(clippy::too_many_arguments)]
fn near_swarm(sys: u32, k: u8, sw: &Swarm, el: &Elements, au: f32, level: u8, rel: DVec3, t: f64, reach: f64, out: &mut Vec<Asteroid>) {
    let cell = LEVELS[level as usize].cell;
    let lead = if sw.leading { TAU / 6.0 } else { -TAU / 6.0 };
    let (p, r, n, along) = el.frame(t, lead);
    let d = rel - p;
    let local = DVec3::new(d.dot(r), d.dot(n), d.dot(along));
    let ax = swarm_axes(sw, el) * 3.0;
    // Assez près de l'essaim (ellipsoïde à 3 écarts) ?
    if (local.abs() - ax).max(DVec3::ZERO).length() > reach {
        return;
    }
    let lo = ((local - DVec3::splat(reach)).max(-ax) / cell).floor();
    let hi = ((local + DVec3::splat(reach)).min(ax) / cell).floor();
    let margin = reach + cell;
    for i in lo.x as i32..=hi.x as i32 {
        for j in lo.y as i32..=hi.y as i32 {
            for l in lo.z as i32..=hi.z as i32 {
                let c = DVec3::new(i as f64 + 0.5, j as f64 + 0.5, l as f64 + 0.5) * cell;
                if c.distance_squared(local) <= margin * margin {
                    out.extend(swarm_cell(sys, k, sw, el, au, level, i, j, l));
                }
            }
        }
    }
}

/// Particules d'un anneau de planète autour de `rel`.
fn near_ring(sys: u32, k: u8, rs: &RingSrc, rel: DVec3, t: f64, reach: f64, out: &mut Vec<Asteroid>) {
    let (p, q) = rs.plane(t);
    let d = rel - p;
    if d.length() > rs.ring.outer as f64 + reach {
        return;
    }
    let local = q.inverse().as_dquat() * d;
    annulus_cells(rs.ring.inner as f64, rs.ring.outer as f64, rs.half_thickness(), planet_mu(), RING_CELL, local, t, reach, |ring, sector, layer| {
        out.extend(ring_cell(sys, k, rs, ring, sector, layer));
    });
}

/// Tous les astéroïdes (ceintures, Troyens, anneaux) d'un système autour de `rel`, chacun avec la
/// portée de son niveau. Les comètes n'y sont pas : elles sont toujours affichées.
pub fn near_all(sys: u32, src: &Sources, rel: DVec3, t: f64, out: &mut Vec<Asteroid>) {
    for (bi, belt) in src.belts.iter().enumerate() {
        for (level, lv) in LEVELS.iter().enumerate() {
            near_belt(sys, bi as u8, belt, level as u8, rel, t, lv.reach, out);
        }
    }
    for (k, (sw, el, au)) in src.swarms.iter().enumerate() {
        for (level, lv) in LEVELS.iter().enumerate() {
            near_swarm(sys, k as u8, sw, el, *au, level as u8, rel, t, lv.reach, out);
        }
    }
    for (k, rs) in src.rings.iter().enumerate() {
        near_ring(sys, k as u8, rs, rel, t, LEVELS[0].reach, out);
    }
}

/// Toutes les comètes d'un système.
pub fn comets_of(sys: u32, src: &Sources) -> impl Iterator<Item = Asteroid> + '_ {
    src.comets.iter().enumerate().map(move |(i, c)| comet_asteroid(sys, i, c))
}

// ─────────────────────────────────────────────────────────────────────────
//  Ressources et composants
// ─────────────────────────────────────────────────────────────────────────

/// Un astéroïde affiché.
pub struct Live {
    pub entity: Entity,
    pub ast: Asteroid,
    pub pose: Transform,
    pub vel: Vec3,
    prev: Option<(Vec3, u32)>,
    /// Caillou poussé par le vaisseau (effet local, il dérive puis revient quand on repasse).
    knock: Vec3,
    knock_vel: Vec3,
    meshed: bool,
}

/// Les astéroïdes affichés autour de la caméra, et les petits corps des systèmes chargés.
#[derive(Resource, Default)]
pub struct AsteroidField {
    live: HashMap<AsteroidKey, Live>,
    sources: HashMap<usize, Sources>,
    tasks: HashMap<AsteroidKey, Task<Mesh>>,
    bands: HashMap<(usize, u8), (Entity, Handle<StandardMaterial>, Belt)>,
    last_refresh: f64,
}

impl AsteroidField {
    /// Pose (monde) d'un astéroïde affiché.
    pub fn pose(&self, key: &AsteroidKey) -> Option<Transform> {
        self.live.get(key).map(|l| l.pose)
    }

    /// Entité racine et pose d'un astéroïde affiché.
    pub fn root(&self, key: &AsteroidKey) -> Option<(Entity, Transform)> {
        self.live.get(key).map(|l| (l.entity, l.pose))
    }

    pub fn get(&self, key: &AsteroidKey) -> Option<&Asteroid> {
        self.live.get(key).map(|l| &l.ast)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Live> {
        self.live.values()
    }

    pub fn len(&self) -> usize {
        self.live.len()
    }

    /// Petits corps d'un système chargé.
    pub fn sources(&self, si: usize) -> Option<&Sources> {
        self.sources.get(&si)
    }

    /// Recalcule à la prochaine image ce qui est autour de la caméra.
    pub fn refresh_now(&mut self) {
        self.last_refresh = f64::NEG_INFINITY;
    }
}

/// Paramètres de terrain d'un astéroïde (on peut se poser sur les gros seulement).
pub fn body_params(settings: &GameSettings, key: &AsteroidKey) -> Option<crate::terrain::BodyParams> {
    if key.level != LANDABLE_LEVEL {
        return None;
    }
    let sys = settings.systems.get(key.sys as usize)?;
    let a = find(&Sources::for_key(sys, key), *key)?;
    let lum = sys.lighting().map_or(1.0, |p| p.luminosity_sun);
    Some(a.body_params(lum))
}

/// Racine d'un astéroïde affiché.
#[derive(Component)]
pub struct AsteroidBody;

/// Choc du vaisseau contre un astéroïde (dégâts de coque).
#[derive(Event)]
pub struct AsteroidHit {
    pub damage: u8,
    pub speed: f32,
}

/// `/ceinture` : aller à un champ dense d'une ceinture du système.
#[derive(Event)]
pub struct BeltCommand;

#[derive(Resource)]
struct AsteroidAssets {
    materials: [Handle<StandardMaterial>; 4],
    /// Formes partagées des petits astéroïdes, par type.
    variants: Vec<Vec<Handle<Mesh>>>,
}

fn setup_assets(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let mat = |metallic: f32, rough: f32, reflect: f32| StandardMaterial { base_color: Color::WHITE, metallic, perceptual_roughness: rough, reflectance: reflect, ..default() };
    let materials = [
        materials.add(mat(0.0, 0.97, 0.1)),
        materials.add(mat(0.0, 0.9, 0.2)),
        materials.add(mat(0.75, 0.42, 0.5)),
        materials.add(mat(0.0, 0.55, 0.45)),
    ];
    let variants = AsteroidClass::ALL.iter().map(|&c| (0..VARIANTS).map(|v| meshes.add(build_mesh(&variant_shape(c, v), 7))).collect()).collect();
    commands.insert_resource(AsteroidAssets { materials, variants });
}

// ─────────────────────────────────────────────────────────────────────────
//  Placement (chaque image), streaming (quatre fois par seconde)
// ─────────────────────────────────────────────────────────────────────────

/// Position monde (origine flottante) d'un point donné depuis le centre d'un système.
fn world_of(center: DVec3, rel: DVec3) -> Vec3 {
    (center + rel - origin()).as_vec3()
}

fn sys_center(settings: &GameSettings, si: usize) -> Option<DVec3> {
    settings.systems.get(si).map(|s| s.abs_center().as_dvec3())
}

fn place_asteroids(
    time: Res<Time>,
    clock: Res<WorldClock>,
    settings: Res<GameSettings>,
    epoch: Res<crate::origin::OriginEpoch>,
    mut field: ResMut<AsteroidField>,
    mut q: Query<&mut Transform, With<AsteroidBody>>,
) {
    let t = clock.secs;
    let dt = time.delta_secs().max(1e-4);
    let mut centers: HashMap<u32, Option<DVec3>> = HashMap::new();
    for live in field.live.values_mut() {
        let sys = live.ast.key.sys;
        let Some(center) = *centers.entry(sys).or_insert_with(|| sys_center(&settings, sys as usize)) else { continue };
        // Caillou poussé : il s'éloigne en ralentissant
        if live.knock_vel != Vec3::ZERO {
            live.knock += live.knock_vel * dt;
            live.knock_vel *= (-0.3 * dt).exp();
            if live.knock_vel.length_squared() < 0.01 {
                live.knock_vel = Vec3::ZERO;
            }
        }
        let pos = world_of(center, live.ast.rel_position(t)) + live.knock;
        live.vel = match live.prev {
            Some((p, e)) if e == epoch.0 => (pos - p) / dt,
            _ => live.vel,
        };
        live.prev = Some((pos, epoch.0));
        live.pose = Transform { translation: pos, rotation: live.ast.rotation(t), scale: Vec3::ONE };
        if let Ok(mut tf) = q.get_mut(live.entity) {
            *tf = live.pose;
        }
    }
}

/// Astéroïdes qui doivent toujours exister : la cible, et celui où l'on séjourne.
fn pinned(target: &CameraTarget, surface: &Surface) -> Vec<AsteroidKey> {
    let mut keys = Vec::new();
    if let TargetKind::Asteroid(k) = target.0 {
        keys.push(k);
    }
    if let Some(TargetKind::Asteroid(k)) = surface.body() {
        if !keys.contains(&k) {
            keys.push(k);
        }
    }
    keys
}

/// Créations au plus par mise à jour (les autres suivent au tour suivant).
const SPAWN_BUDGET: usize = 150;
/// Constructions de maillages en cours au plus.
const MAX_MESH_TASKS: usize = 8;

#[allow(clippy::too_many_arguments)]
fn stream_asteroids(
    mut commands: Commands,
    time: Res<Time>,
    clock: Res<WorldClock>,
    settings: Res<GameSettings>,
    spawned: Res<SpawnedSystems>,
    target: Res<CameraTarget>,
    surface: Res<Surface>,
    assets: Res<AsteroidAssets>,
    cam_q: Query<&Transform, With<Camera3d>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut field: ResMut<AsteroidField>,
) {
    let field = &mut *field;
    // Maillages terminés
    let done: Vec<AsteroidKey> = field.tasks.keys().copied().collect();
    for key in done {
        let Some(task) = field.tasks.get_mut(&key) else { continue };
        let Some(mesh) = block_on(future::poll_once(task)) else { continue };
        field.tasks.remove(&key);
        if let Some(live) = field.live.get_mut(&key) {
            let child = commands
                .spawn((
                    Mesh3d(meshes.add(mesh)),
                    MeshMaterial3d(assets.materials[live.ast.class() as usize].clone()),
                    Transform::IDENTITY,
                    FarMesh,
                    NotShadowCaster,
                ))
                .id();
            commands.entity(live.entity).add_child(child);
            live.meshed = true;
        }
    }

    let now = time.elapsed_secs_f64();
    if now - field.last_refresh < 0.25 {
        return;
    }
    field.last_refresh = now;
    let Ok(cam) = cam_q.get_single() else { return };
    let t = clock.secs;
    let cam_abs = to_abs(cam.translation);

    // Petits corps des systèmes chargés (recalculés : l'éditeur peut changer les planètes)
    field.sources.retain(|si, _| spawned.0.contains(si));
    for &si in &spawned.0 {
        if let Some(sys) = settings.systems.get(si) {
            field.sources.insert(si, Sources::of(sys));
        }
    }

    // Astéroïdes voulus : assez près de la caméra pour être vus (selon leur taille) ; les comètes
    // toujours (leur queue se voit de loin)
    let mut wanted: HashMap<AsteroidKey, (Asteroid, f32)> = HashMap::new();
    let mut found = Vec::new();
    for (&si, src) in &field.sources {
        let Some(center) = sys_center(&settings, si) else { continue };
        let rel = cam_abs - center;
        found.clear();
        near_all(si as u32, src, rel, t, &mut found);
        for a in &found {
            let lv = &LEVELS[a.key.level as usize];
            let d = a.rel_position(t).distance(rel) as f32;
            let sight = (lv.sight * a.shape.radius + lv.sight_base).min(lv.reach as f32);
            if d < sight {
                wanted.insert(a.key, (*a, d));
            }
        }
        for a in comets_of(si as u32, src) {
            wanted.insert(a.key, (a, a.rel_position(t).distance(rel) as f32));
        }
    }
    for key in pinned(&target, &surface) {
        if wanted.contains_key(&key) {
            continue;
        }
        let found = match field.sources.get(&(key.sys as usize)) {
            Some(src) => find(src, key),
            None => settings.systems.get(key.sys as usize).and_then(|s| find(&Sources::for_key(s, &key), key)),
        };
        if let Some(a) = found {
            wanted.insert(key, (a, 0.0));
        }
    }

    // Oubli des astéroïdes trop loin
    let gone: Vec<AsteroidKey> = field.live.keys().filter(|k| !wanted.contains_key(k)).copied().collect();
    for key in gone {
        if let Some(live) = field.live.remove(&key) {
            commands.entity(live.entity).try_despawn_recursive();
        }
        field.tasks.remove(&key);
    }

    // Créations, les plus proches d'abord
    let mut missing: Vec<(f32, Asteroid)> = wanted.into_values().filter(|(a, _)| !field.live.contains_key(&a.key)).map(|(a, d)| (d, a)).collect();
    missing.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (_, a) in missing.into_iter().take(SPAWN_BUDGET) {
        let Some(center) = sys_center(&settings, a.key.sys as usize) else { continue };
        let pose = Transform { translation: world_of(center, a.rel_position(t)), rotation: a.rotation(t), scale: Vec3::ONE };
        let entity = commands.spawn((pose, Visibility::default(), AsteroidBody)).id();
        let meshed = match a.variant {
            Some(v) => {
                let child = commands
                    .spawn((
                        Mesh3d(assets.variants[a.class() as usize][v as usize].clone()),
                        MeshMaterial3d(assets.materials[a.class() as usize].clone()),
                        Transform::from_scale(Vec3::splat(a.shape.radius)),
                        FarMesh,
                        NotShadowCaster,
                    ))
                    .id();
                commands.entity(entity).add_child(child);
                true
            }
            None => false,
        };
        field.live.insert(a.key, Live { entity, ast: a, pose, vel: Vec3::ZERO, prev: None, knock: Vec3::ZERO, knock_vel: Vec3::ZERO, meshed });
    }

    // Maillages des gros (hors du fil principal), les plus proches d'abord
    let mut todo: Vec<(f32, AsteroidKey, AsteroidShape)> = field
        .live
        .values()
        .filter(|l| !l.meshed && !field.tasks.contains_key(&l.ast.key))
        .map(|l| (l.pose.translation.distance(cam.translation), l.ast.key, l.ast.shape))
        .collect();
    todo.sort_by(|a, b| a.0.total_cmp(&b.0));
    let pool = AsyncComputeTaskPool::get();
    for (_, key, shape) in todo {
        if field.tasks.len() >= MAX_MESH_TASKS {
            break;
        }
        // Finesse selon la taille : ~4 unités par côté de quad, 10 à 40 par face
        let n = ((shape.radius / 4.0) as usize).clamp(10, 40);
        field.tasks.insert(key, pool.spawn(async move { build_mesh(&shape, n) }));
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  Vue de loin : bande de poussière
// ─────────────────────────────────────────────────────────────────────────

/// Grains de la bande d'une ceinture.
const BAND_GRAINS: usize = 4_500;
/// La bande s'efface en approchant de la ceinture (les vrais astéroïdes prennent le relais).
const BAND_FADE_NEAR: f32 = 500_000.0;
const BAND_FADE_FAR: f32 = 2_500_000.0;
const BAND_BRIGHTNESS: f32 = 0.9;

#[derive(Component)]
struct BeltBand;

/// Teinte moyenne d'une ceinture (mélange de ses types).
fn belt_tint(belt: &Belt) -> [f32; 3] {
    let mix = belt.class_mix(0.5);
    let mut c = [0.0; 3];
    for (k, w) in AsteroidClass::ALL.iter().zip(mix) {
        let col = k.color();
        for i in 0..3 {
            c[i] += col[i] * w;
        }
    }
    // Un peu plus clair : la poussière diffuse la lumière
    c.map(|x| (x * 1.4).min(1.0))
}

/// Icosaèdre unité (sommets, faces) : la base des cailloux de la bande.
fn icosahedron() -> ([Vec3; 12], [[usize; 3]; 20]) {
    let t = (1.0 + 5f32.sqrt()) / 2.0;
    let v = [
        Vec3::new(-1.0, t, 0.0), Vec3::new(1.0, t, 0.0), Vec3::new(-1.0, -t, 0.0), Vec3::new(1.0, -t, 0.0),
        Vec3::new(0.0, -1.0, t), Vec3::new(0.0, 1.0, t), Vec3::new(0.0, -1.0, -t), Vec3::new(0.0, 1.0, -t),
        Vec3::new(t, 0.0, -1.0), Vec3::new(t, 0.0, 1.0), Vec3::new(-t, 0.0, -1.0), Vec3::new(-t, 0.0, 1.0),
    ]
    .map(|p| p.normalize());
    let f = [
        [0, 11, 5], [0, 5, 1], [0, 1, 7], [0, 7, 10], [0, 10, 11], [1, 5, 9], [5, 11, 4], [11, 10, 2], [10, 7, 6], [7, 1, 8],
        [3, 9, 4], [3, 4, 2], [3, 2, 6], [3, 6, 8], [3, 8, 9], [4, 9, 5], [2, 4, 11], [6, 2, 10], [8, 6, 7], [9, 8, 1],
    ];
    (v, f)
}

/// Maillage de la bande : des cailloux de formes variées (icosaèdres bosselés, allongés, tournés),
/// de tailles en loi de puissance, placés selon la densité de la ceinture ; chaque face est ombrée
/// d'après l'étoile (au centre du maillage) : un côté jour, un côté nuit, pas de points lumineux.
fn band_mesh(belt: &Belt) -> Mesh {
    let mut rng = Rng(mix(belt.seed as u64, 0xBA4D));
    let (mut pos, mut nor, mut col, mut idx) = (Vec::new(), Vec::new(), Vec::new(), Vec::<u32>::new());
    let size = belt.width() * 0.0045;
    let (ico, faces) = icosahedron();
    let mut grains = 0;
    let mut tries = 0;
    while grains < BAND_GRAINS && tries < BAND_GRAINS * 30 {
        tries += 1;
        let rho = belt.inner as f64 + rng.f64() * belt.width() as f64;
        let theta = rng.f64() * TAU;
        let y = (rng.f64() * 2.0 - 1.0) * belt.half_thickness as f64 * 2.0;
        let (d, dense) = field_density(belt, rho, theta, y);
        let p = d / belt.density.max(0.1) as f64 * (1.0 + 1.5 * dense);
        if rng.f64() * 1.6 > p {
            continue;
        }
        let c = Vec3::new((rho * theta.cos()) as f32, y as f32, (rho * theta.sin()) as f32);
        grains += 1;
        // Taille : beaucoup de petits, quelques gros
        let s = size * power_radius(0.35, 3.0, rng.f64() as f32);
        // Forme : allongée, aplatie, bosselée, tournée au hasard
        let stretch = Vec3::new(rng.range(0.55, 1.5) as f32, rng.range(0.45, 1.0) as f32, rng.range(0.6, 1.3) as f32);
        let rot = Quat::from_euler(EulerRot::XYZ, rng.range(0.0, std::f32::consts::TAU), rng.range(0.0, std::f32::consts::TAU), rng.range(0.0, std::f32::consts::TAU));
        let bumps: Vec<f32> = (0..12).map(|_| rng.range(0.72, 1.15) as f32).collect();
        let verts: Vec<Vec3> = ico.iter().zip(&bumps).map(|(v, b)| c + rot * (*v * stretch * *b) * s).collect();
        let albedo = rng.range(0.55, 1.0) as f32;
        let sun = -c.normalize_or(Vec3::X);
        for f in faces {
            let (a, b, d) = (verts[f[0]], verts[f[1]], verts[f[2]]);
            let n = (b - a).cross(d - a).normalize_or(Vec3::Y);
            let light = albedo * (0.12 + 0.88 * n.dot(sun).max(0.0));
            let base = pos.len() as u32;
            for p in [a, b, d] {
                pos.push(p.to_array());
                nor.push(n.to_array());
                col.push([light, light, light, 1.0]);
            }
            idx.extend_from_slice(&[base, base + 1, base + 2]);
        }
    }
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, nor);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
    mesh.insert_indices(Indices::U32(idx));
    mesh
}

/// Distance d'un point (depuis le centre du système) à l'anneau d'une ceinture.
fn distance_to_belt(belt: &Belt, rel: DVec3) -> f64 {
    let rho = (rel.x * rel.x + rel.z * rel.z).sqrt();
    let d_rho = (belt.inner as f64 - rho).max(rho - belt.outer as f64).max(0.0);
    let d_y = (rel.y.abs() - belt.half_thickness as f64).max(0.0);
    (d_rho * d_rho + d_y * d_y).sqrt()
}

#[allow(clippy::too_many_arguments)]
fn update_bands(
    mut commands: Commands,
    clock: Res<WorldClock>,
    settings: Res<GameSettings>,
    cam_q: Query<&Transform, (With<Camera3d>, Without<BeltBand>)>,
    mut bands_q: Query<&mut Transform, With<BeltBand>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut field: ResMut<AsteroidField>,
) {
    let field = &mut *field;
    let Ok(cam) = cam_q.get_single() else { return };
    let cam_abs = to_abs(cam.translation);
    // Bandes des ceintures qui n'existent plus (système déchargé, ceinture changée)
    let stale: Vec<(usize, u8)> = field
        .bands
        .iter()
        .filter(|((si, bi), (_, _, belt))| field.sources.get(si).and_then(|s| s.belts.get(*bi as usize)) != Some(belt))
        .map(|(k, _)| *k)
        .collect();
    for k in stale {
        if let Some((e, _, _)) = field.bands.remove(&k) {
            commands.entity(e).try_despawn_recursive();
        }
    }
    for (&si, src) in &field.sources {
        let Some(center) = sys_center(&settings, si) else { continue };
        let rel = cam_abs - center;
        for (bi, belt) in src.belts.iter().enumerate() {
            let key = (si, bi as u8);
            let fade = ((distance_to_belt(belt, rel) as f32 - BAND_FADE_NEAR) / (BAND_FADE_FAR - BAND_FADE_NEAR)).clamp(0.0, 1.0);
            let fade = fade * fade * (3.0 - 2.0 * fade);
            let tint = belt_tint(belt);
            let color = Color::srgba(tint[0] * BAND_BRIGHTNESS, tint[1] * BAND_BRIGHTNESS, tint[2] * BAND_BRIGHTNESS, fade);
            // Toute la bande tourne à la vitesse du milieu de la ceinture
            let n = (mu() / (belt.mid() as f64).powi(3)).sqrt();
            let pose = Transform {
                translation: world_of(center, DVec3::ZERO),
                rotation: Quat::from_rotation_y(-((n * clock.secs).rem_euclid(TAU)) as f32),
                scale: Vec3::ONE,
            };
            match field.bands.get(&key) {
                Some((e, mat, _)) => {
                    if let Ok(mut tf) = bands_q.get_mut(*e) {
                        *tf = pose;
                    }
                    let (old, new) = (materials.get(mat).map(|m| m.base_color.to_srgba()), color.to_srgba());
                    if old.is_some_and(|o| (o.alpha - new.alpha).abs() > 0.004) {
                        if let Some(m) = materials.get_mut(mat) {
                            m.base_color = color;
                        }
                    }
                }
                None => {
                    let mat = materials.add(StandardMaterial { base_color: color, unlit: true, alpha_mode: AlphaMode::Blend, ..default() });
                    let e = commands
                        .spawn((Mesh3d(meshes.add(band_mesh(belt))), MeshMaterial3d(mat.clone()), pose, Visibility::default(), NotShadowCaster, BeltBand))
                        .id();
                    field.bands.insert(key, (e, mat, *belt));
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  Comètes : chevelure, queue de gaz, queue de poussière
// ─────────────────────────────────────────────────────────────────────────

#[derive(Component)]
struct CometPart;

/// Chevelure et queues d'une comète affichée.
struct Tails {
    parts: [Entity; 3],
    materials: [Handle<StandardMaterial>; 3],
    activity: f32,
    /// Atténuation quand la caméra est dans la chevelure (0,1 à 1).
    near: f32,
}

#[derive(Resource, Default)]
struct CometFx {
    tails: HashMap<AsteroidKey, Tails>,
    /// Chevelure, queue de gaz (droite), queue de poussière (courbée).
    meshes: Option<[Handle<Mesh>; 3]>,
}

/// Couleurs (linéaires, mélange additif) : chevelure, gaz ionisé bleu, poussière jaunâtre.
const COMET_COLORS: [[f32; 3]; 3] = [[0.75, 0.88, 1.0], [0.3, 0.55, 1.0], [1.0, 0.86, 0.62]];

/// Queue : longueur 1 le long de +z depuis la tête, deux rubans croisés qui s'élargissent et
/// s'éteignent ; `curve` la courbe vers +x (poussière en retard sur l'orbite).
fn tail_mesh(curve: f32) -> Mesh {
    const N: usize = 32;
    let (mut pos, mut col, mut idx) = (Vec::new(), Vec::new(), Vec::<u32>::new());
    for plane in 0..2 {
        let base = pos.len() as u32;
        for k in 0..=N {
            let s = k as f32 / N as f32;
            let w = 0.015 + 0.16 * s;
            let c = Vec3::new(curve * s * s, 0.0, s);
            let side = if plane == 0 { Vec3::X } else { Vec3::Y };
            let fade = (1.0 - s).powf(1.6);
            for (o, edge) in [(-1.0f32, 0.0f32), (0.0, 1.0), (1.0, 0.0)] {
                pos.push((c + side * w * o).to_array());
                let v = fade * edge;
                col.push([v, v, v, 1.0]);
            }
        }
        for k in 0..N as u32 {
            let a = base + k * 3;
            let b = a + 3;
            idx.extend_from_slice(&[a, a + 1, b, a + 1, b + 1, b, a + 1, a + 2, b + 1, a + 2, b + 2, b + 1]);
        }
    }
    let n = pos.len();
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; n]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
    mesh.insert_indices(Indices::U32(idx));
    mesh
}

/// Taille de la chevelure (rayon) et longueur de la queue d'une comète d'activité `act`.
pub fn tail_size(c: &Comet, act: f32) -> (f32, f32) {
    (c.radius * 4.0 + act * c.tail * 0.015, act * c.tail)
}

#[allow(clippy::too_many_arguments)]
fn update_comet_tails(
    mut commands: Commands,
    settings: Res<GameSettings>,
    field: Res<AsteroidField>,
    mut fx: ResMut<CometFx>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut parts: Query<(&mut Transform, &mut Visibility), (With<CometPart>, Without<AsteroidBody>)>,
    cam_q: Query<&Transform, (With<Camera3d>, Without<CometPart>, Without<AsteroidBody>)>,
) {
    let fx = &mut *fx;
    let cam = cam_q.get_single().map(|t| t.translation).unwrap_or(Vec3::splat(f32::MAX));
    let mesh = fx
        .meshes
        .get_or_insert_with(|| [meshes.add(Sphere::new(1.0).mesh().ico(3).unwrap_or_else(|_| Mesh::from(Sphere::new(1.0)))), meshes.add(tail_mesh(0.0)), meshes.add(tail_mesh(0.35))])
        .clone();
    // Comètes qui ne sont plus affichées
    let gone: Vec<AsteroidKey> = fx.tails.keys().filter(|k| !field.live.contains_key(k)).copied().collect();
    for k in gone {
        if let Some(t) = fx.tails.remove(&k) {
            for e in t.parts {
                commands.entity(e).try_despawn_recursive();
            }
        }
    }
    for live in field.live.values() {
        let Some(comet) = live.ast.comet() else { continue };
        let Some(center) = sys_center(&settings, live.ast.key.sys as usize) else { continue };
        let star = world_of(center, DVec3::ZERO);
        let pos = live.pose.translation;
        let away = pos - star;
        let act = comet.activity(away.length() as f64);
        let anti = away.normalize_or(Vec3::Z);
        let tails = fx.tails.entry(live.ast.key).or_insert_with(|| {
            let materials: [Handle<StandardMaterial>; 3] = [0, 1, 2].map(|_| materials.add(StandardMaterial { base_color: Color::BLACK, unlit: true, alpha_mode: AlphaMode::Add, cull_mode: None, double_sided: true, ..default() }));
            let parts = [0, 1, 2].map(|i| commands.spawn((Mesh3d(mesh[i].clone()), MeshMaterial3d(materials[i].clone()), Transform::IDENTITY, Visibility::Hidden, NotShadowCaster, CometPart)).id());
            Tails { parts, materials, activity: -1.0, near: 1.0 }
        });
        // De près (dans la chevelure ou la queue), la lueur s'efface : on voit le noyau
        let (coma, _) = tail_size(comet, act);
        let d = cam.distance(pos);
        let near = 0.1 + 0.9 * ((d - coma * 1.5) / (coma * 10.0)).clamp(0.0, 1.0);
        // Couleurs : seulement quand l'activité ou la distance changent vraiment
        if (tails.activity - act).abs() > 0.01 || (tails.near - near).abs() > 0.03 {
            tails.activity = act;
            tails.near = near;
            for (i, m) in tails.materials.iter().enumerate() {
                if let Some(m) = materials.get_mut(m) {
                    let k = if i == 0 { 0.35 + 0.9 * act } else { 1.2 * act } * near;
                    let c = COMET_COLORS[i];
                    m.base_color = Color::linear_rgb(c[0] * k, c[1] * k, c[2] * k);
                }
            }
        }
        let (coma, length) = tail_size(comet, act);
        // Poussière : courbée en retard sur le mouvement (vers l'arrière de l'orbite)
        let back = -live.vel;
        let lag = (back - anti * back.dot(anti)).try_normalize().unwrap_or_else(|| anti.any_orthonormal_vector());
        let dust_rot = Quat::from_mat3(&Mat3::from_cols(lag, anti.cross(lag), anti));
        let poses = [
            Transform::from_translation(pos).with_scale(Vec3::splat(coma)),
            Transform::from_translation(pos).with_rotation(Quat::from_rotation_arc(Vec3::Z, anti)).with_scale(Vec3::splat(length.max(1.0))),
            Transform::from_translation(pos).with_rotation(dust_rot).with_scale(Vec3::splat((length * 0.75).max(1.0))),
        ];
        let shown = [act > 0.005, act > 0.01, act > 0.01];
        for i in 0..3 {
            if let Ok((mut tf, mut v)) = parts.get_mut(tails.parts[i]) {
                *tf = poses[i];
                let wanted = if shown[i] { Visibility::Inherited } else { Visibility::Hidden };
                if *v != wanted {
                    *v = wanted;
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  Traînées : où vont les comètes, les planètes et les gros astéroïdes
// ─────────────────────────────────────────────────────────────────────────

/// Points d'une traînée : le chemin parcouru pendant les `span` dernières secondes, du présent au
/// passé, de plus en plus transparent.
fn trail(n: usize, span: f64, rgb: [f32; 3], alpha: f32, at: impl Fn(f64) -> Vec3) -> Vec<(Vec3, Color)> {
    (0..=n)
        .map(|k| {
            let f = k as f64 / n as f64;
            let a = alpha * (1.0 - f as f32).powf(1.5);
            (at(span * f), Color::srgba(rgb[0], rgb[1], rgb[2], a))
        })
        .collect()
}

/// Derrière chaque comète (5 % de son orbite), planète (4 % de la sienne) et gros astéroïde proche
/// (30 fois sa taille), le chemin qu'il vient de suivre s'efface : on voit où il est et où il va,
/// même quand le mouvement est trop lent pour être vu.
fn draw_trails(clock: Res<WorldClock>, settings: Res<GameSettings>, field: Res<AsteroidField>, surface: Res<Surface>, cam_q: Query<&Transform, With<Camera3d>>, mut g: Gizmos) {
    // À pied ou en vol bas, les traînées du ciel gêneraient
    if surface.active() {
        return;
    }
    let t = clock.secs;
    let cam = cam_q.get_single().map(|c| c.translation).unwrap_or_default();
    for live in field.live.values() {
        let Some(center) = sys_center(&settings, live.ast.key.sys as usize) else { continue };
        if let Some(c) = live.ast.comet() {
            let span = Elements::of_comet(c).period() * 0.05;
            g.linestrip_gradient(trail(48, span, [0.55, 0.78, 1.0], 0.6, |dt| world_of(center, live.ast.rel_position(t - dt))));
            continue;
        }
        if live.ast.key.level != LANDABLE_LEVEL {
            continue;
        }
        let r = live.ast.shape.max_radius();
        let speed = live.vel.length();
        if speed < 1e-3 || live.pose.translation.distance(cam) > r * 300.0 {
            continue;
        }
        let span = (r * 30.0 / speed) as f64;
        g.linestrip_gradient(trail(16, span, [0.8, 0.78, 0.72], 0.35, |dt| world_of(center, live.ast.rel_position(t - dt))));
    }
    for &si in field.sources.keys() {
        let (Some(sys), Some(center)) = (settings.systems.get(si), sys_center(&settings, si)) else { continue };
        for p in sys.planets().iter().filter(|p| !p.rogue) {
            let el = Elements::of_planet(p);
            g.linestrip_gradient(trail(40, el.period() * 0.04, [0.85, 0.9, 1.0], 0.3, |dt| world_of(center, el.position(t - dt, 0.0))));
        }
    }
}

/// `/comete` : aller à la comète la plus active du système.
#[derive(Event)]
pub struct CometCommand;

#[allow(clippy::too_many_arguments)]
fn go_comet(
    time: Res<Time>,
    clock: Res<WorldClock>,
    settings: Res<GameSettings>,
    spawned: Res<SpawnedSystems>,
    surface: Res<Surface>,
    mut events: EventReader<CometCommand>,
    mut target: ResMut<CameraTarget>,
    mut net: ResMut<Net>,
    mut field: ResMut<AsteroidField>,
    mut ship_q: Query<&mut Transform, With<Ship>>,
    mut cam_q: Query<&mut CameraController>,
) {
    for _ in events.read() {
        let now = time.elapsed_secs_f64();
        if surface.active() {
            net.notify("Remontez d'abord en orbite (molette ou V).", now);
            continue;
        }
        let Some(&si) = spawned.0.iter().next() else {
            net.notify("Aucun systeme charge : approchez-vous d'une etoile.", now);
            continue;
        };
        let Some(sys) = settings.systems.get(si) else { continue };
        let src = Sources { comets: sys.comets(), ..Default::default() };
        let t = clock.secs;
        // La plus active, sinon la plus proche de l'étoile
        let best = comets_of(si as u32, &src).min_by(|a, b| {
            let score = |x: &Asteroid| {
                let r = x.rel_position(t).length();
                (-(x.comet().map_or(0.0, |c| c.activity(r)) as f64), r)
            };
            score(a).partial_cmp(&score(b)).unwrap_or(std::cmp::Ordering::Equal)
        });
        let Some(a) = best else {
            net.notify(&format!("{} n'a pas de comete. Essayez un autre systeme.", sys.name), now);
            continue;
        };
        let pos = world_of(sys.abs_center().as_dvec3(), a.rel_position(t));
        target.0 = TargetKind::Asteroid(a.key);
        if let Ok(mut ship) = ship_q.get_single_mut() {
            ship.translation = pos + Vec3::Y * (a.shape.max_radius() * 2.0 + 300.0);
        }
        let act = a.comet().map_or(0.0, |c| c.activity(a.rel_position(t).length()));
        let (_, length) = a.comet().map_or((0.0, 0.0), |c| tail_size(c, act));
        if let Ok(mut ctrl) = cam_q.get_single_mut() {
            ctrl.last_target_pos = pos;
            // Assez loin pour voir la queue entière, sinon près du noyau
            ctrl.zoom_goal = Some(if length > 1.0 { length * 1.5 } else { a.shape.radius * 8.0 + 600.0 });
        }
        field.refresh_now();
        let state = if act > 0.3 { "tres active" } else if act > 0.01 { "active" } else { "endormie, loin de l'etoile" };
        net.notify(&format!("{} : {} ({state}). Molette pour s'approcher du noyau, V pour s'y poser.", sys.name, a.title()), now);
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  Collisions (Q6)
// ─────────────────────────────────────────────────────────────────────────

/// Demi-longueur du vaisseau (unités, avant mise à l'échelle).
const SHIP_HALF: f32 = 3.0;
/// Vitesse de choc (unités/s) sans dégât, et vitesse qui détruit la coque d'un coup.
const SAFE_SPEED: f32 = 60.0;
const KILL_SPEED: f32 = 700.0;

/// Dégâts (points de coque) d'un choc à `closing` unités/s.
pub fn impact_damage(closing: f32) -> u8 {
    (((closing - SAFE_SPEED) / (KILL_SPEED - SAFE_SPEED)).clamp(0.0, 1.0) * MAX_HP as f32).ceil() as u8
}

/// Le vaisseau (centre `ship`, demi-longueur `ship_r`) touche-t-il l'astéroïde ? Direction pour
/// l'en sortir et profondeur.
fn contact(shape: &AsteroidShape, pose: &Transform, ship: Vec3, ship_r: f32) -> Option<(Vec3, f32)> {
    let to = ship - pose.translation;
    let d = to.length();
    if d > shape.max_radius() + ship_r {
        return None;
    }
    let normal = if d < 1e-3 { Vec3::Y } else { to / d };
    let pen = shape.radius_at(pose.rotation.inverse() * normal) + ship_r * 0.6 - d;
    (pen > 0.0).then_some((normal, pen))
}

#[allow(clippy::too_many_arguments)]
fn ship_collisions(
    time: Res<Time>,
    net: Res<Net>,
    epoch: Res<crate::origin::OriginEpoch>,
    mut surface: ResMut<Surface>,
    mut field: ResMut<AsteroidField>,
    mut ship_q: Query<&mut Transform, With<Ship>>,
    mut cam_q: Query<&mut Transform, (With<Camera3d>, Without<Ship>)>,
    target: Res<CameraTarget>,
    mut hits: EventWriter<AsteroidHit>,
    mut prev: Local<Option<(Vec3, u32)>>,
    mut cooldown: Local<f64>,
) {
    let Ok(mut ship) = ship_q.get_single_mut() else { return };
    let dt = time.delta_secs().max(1e-4);
    let now = time.elapsed_secs_f64();
    let v_ship = match *prev {
        Some((p, e)) if e == epoch.0 => (ship.translation - p) / dt,
        _ => Vec3::ZERO,
    };
    *prev = Some((ship.translation, epoch.0));
    // À pied, en descente ou en montée : pas de choc
    let flying = surface.flying();
    if surface.active() && !flying {
        return;
    }
    if net.local.hp == 0 {
        return;
    }
    let here = match surface.body() {
        Some(TargetKind::Asteroid(k)) => Some(k),
        _ => None,
    };
    // Dans l'espace, le vaisseau (à taille d'icône) stationne autour de sa cible : la toucher le
    // ferait sauter à chaque image (comète qui « tremble », C8)
    let parked = match (flying, target.0) {
        (false, TargetKind::Asteroid(k)) => Some(k),
        _ => None,
    };
    let ship_r = SHIP_HALF * ship.scale.x;
    for live in field.live.values_mut() {
        if Some(live.ast.key) == here || Some(live.ast.key) == parked {
            continue;
        }
        let shape = &live.ast.shape;
        let Some((normal, pen)) = contact(shape, &live.pose, ship.translation, ship_r) else { continue };
        let closing = -(v_ship - live.vel).dot(normal);
        // Petit caillou : il est repoussé, sans dégât
        if shape.radius < ship_r * 0.5 {
            live.knock_vel -= normal * closing.max(20.0) * 1.3;
            continue;
        }
        let mut push = normal * pen;
        // Pilote automatique : il glisse sur le côté (même s'il arrivait droit dessus)
        if !flying {
            let heading = v_ship.normalize_or(normal);
            let side = (normal - heading * normal.dot(heading)).try_normalize().unwrap_or_else(|| heading.any_orthonormal_vector());
            push += side * pen.max(ship_r);
        }
        ship.translation += push;
        // Dans l'espace, la caméra suit le vaisseau (déjà placée pour cette image)
        if !flying {
            if let Ok(mut cam) = cam_q.get_single_mut() {
                cam.translation += push;
            }
        }
        if flying {
            surface.bump(push, closing > 0.0);
            if closing > SAFE_SPEED && now > *cooldown {
                *cooldown = now + 0.5;
                hits.send(AsteroidHit { damage: impact_damage(closing), speed: closing });
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  /ceinture : un champ dense et son plus gros astéroïde
// ─────────────────────────────────────────────────────────────────────────

/// Le plus gros astéroïde du champ le plus dense d'une ceinture (cherché à l'instant `t`).
pub fn densest_field(sys: u32, bi: u8, belt: &Belt, t: f64) -> Option<Asteroid> {
    let mid = belt.mid() as f64;
    let n = (mu() / mid.powi(3)).sqrt();
    // Le point le plus dense sur le cercle du milieu (repère de la ceinture)
    let best = (0..720)
        .map(|i| {
            let th = i as f64 / 720.0 * TAU;
            let (d, dense) = field_density(belt, mid, th, 0.0);
            (d * (1.0 + 3.0 * dense), th)
        })
        .max_by(|a, b| a.0.total_cmp(&b.0))?;
    let th = best.1 + n * t;
    let rel = DVec3::new(mid * th.cos(), 0.0, mid * th.sin());
    let mut found = Vec::new();
    near_belt(sys, bi, belt, LANDABLE_LEVEL, rel, t, LEVELS[LANDABLE_LEVEL as usize].reach, &mut found);
    found.into_iter().max_by(|a, b| a.shape.radius.total_cmp(&b.shape.radius))
}

#[allow(clippy::too_many_arguments)]
fn go_belt(
    time: Res<Time>,
    clock: Res<WorldClock>,
    settings: Res<GameSettings>,
    spawned: Res<SpawnedSystems>,
    surface: Res<Surface>,
    mut events: EventReader<BeltCommand>,
    mut target: ResMut<CameraTarget>,
    mut net: ResMut<Net>,
    mut field: ResMut<AsteroidField>,
    mut ship_q: Query<&mut Transform, With<Ship>>,
    mut cam_q: Query<&mut CameraController>,
) {
    for _ in events.read() {
        let now = time.elapsed_secs_f64();
        if surface.active() {
            net.notify("Remontez d'abord en orbite (molette ou V).", now);
            continue;
        }
        let Some(&si) = spawned.0.iter().next() else {
            net.notify("Aucun systeme charge : approchez-vous d'une etoile.", now);
            continue;
        };
        let Some(sys) = settings.systems.get(si) else { continue };
        let belts = sys.belts();
        // La ceinture rocheuse d'abord (la plus proche de l'étoile)
        let Some(a) = belts.iter().enumerate().find_map(|(bi, b)| densest_field(si as u32, bi as u8, b, clock.secs)) else {
            net.notify(&format!("{} n'a pas de ceinture d'asteroides. /aller etoile g, puis reessayez.", sys.name), now);
            continue;
        };
        let pos = world_of(sys.abs_center().as_dvec3(), a.rel_position(clock.secs));
        target.0 = TargetKind::Asteroid(a.key);
        if let Ok(mut ship) = ship_q.get_single_mut() {
            ship.translation = pos + Vec3::Y * (a.shape.max_radius() * 2.0 + 300.0);
        }
        if let Ok(mut ctrl) = cam_q.get_single_mut() {
            ctrl.last_target_pos = pos;
            ctrl.zoom_goal = Some(a.shape.radius * 6.0 + 600.0);
        }
        field.refresh_now();
        let belt = &belts[a.key.belt as usize];
        net.notify(
            &format!("{} : champ dense de la {}. {} (zoom sous 1000 pour voler entre les asteroides, V pour se poser).", sys.name, belt.kind.name(), a.title()),
            now,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planetgen::belts::BeltKind;

    fn belt() -> Belt {
        Belt {
            kind: BeltKind::Main,
            seed: 1234,
            au_inner: 2.1,
            au_outer: 3.3,
            inner: 20_000_000.0,
            outer: 23_000_000.0,
            half_thickness: 1_500_000.0,
            mass_earth: 4.5e-4,
            density: 1.0,
            snow_au: 2.7,
        }
    }

    #[test]
    fn shapes_are_closed_and_bounded() {
        for class in AsteroidClass::ALL {
            for seed in 0..60 {
                let s = AsteroidShape::new(class, seed, 100.0);
                let mut rng = Rng(seed as u64);
                for _ in 0..400 {
                    let r = s.radius_at(rng.dir());
                    assert!(r > 20.0 && r <= s.max_radius(), "{:?} {seed} : {r} > {}", s.kind, s.max_radius());
                }
            }
        }
        // Toutes les sortes de formes existent
        let kinds: std::collections::HashSet<_> = (0..400).map(|s| format!("{:?}", AsteroidShape::new(AsteroidClass::S, s, 1.0).kind)).collect();
        assert_eq!(kinds.len(), 4, "{kinds:?}");
        let metal: std::collections::HashSet<_> = (0..400).map(|s| format!("{:?}", AsteroidShape::new(AsteroidClass::M, s, 1.0).kind)).collect();
        assert!(metal.contains("Metallic"));
    }

    #[test]
    fn cells_are_reproducible_and_keyed() {
        let b = belt();
        for ring in [0, 7, 40] {
            for sector in [0, 3, 999] {
                let a = cell_asteroids(5, 0, &b, 2, ring, sector, 0);
                let again = cell_asteroids(5, 0, &b, 2, ring, sector, 0);
                assert_eq!(a.len(), again.len());
                for (x, y) in a.iter().zip(&again) {
                    assert_eq!(x.key, y.key);
                    assert_eq!(x.shape, y.shape);
                    assert_eq!(x.rel_position(1234.5), y.rel_position(1234.5));
                    assert_eq!(find(&Sources { belts: vec![b], ..Default::default() }, x.key).map(|f| f.shape), Some(x.shape));
                }
            }
        }
    }

    #[test]
    fn asteroids_orbit_with_the_clock_and_stay_near_their_cell() {
        let b = belt();
        let mut found = Vec::new();
        near_belt(0, 0, &b, 1, DVec3::new(21_500_000.0, 0.0, 0.0), 0.0, 16_000.0, &mut found);
        assert!(!found.is_empty(), "un champ de rochers autour du milieu de la ceinture");
        for a in &found {
            let p0 = a.rel_position(0.0);
            let p1 = a.rel_position(3600.0);
            // Une heure de jeu : l'orbite a avancé (sens des planètes), le rayon presque pas
            let (r0, r1) = ((p0.x * p0.x + p0.z * p0.z).sqrt(), (p1.x * p1.x + p1.z * p1.z).sqrt());
            assert!((r0 - r1).abs() < LEVELS[1].cell * 0.2);
            assert!(p0.distance(p1) > 1000.0);
            // Toujours dans la ceinture
            assert!(r0 > b.inner as f64 - 1000.0 && r0 < b.outer as f64 + 1000.0);
        }
        // Plus tard, on retrouve les mêmes astéroïdes autour de leur nouvelle position
        let a = found[0];
        let t = 50_000.0;
        let mut later = Vec::new();
        near_belt(0, 0, &b, 1, a.rel_position(t), t, 5_000.0, &mut later);
        assert!(later.iter().any(|x| x.key == a.key), "l'asteroide suit sa cellule");
    }

    #[test]
    fn dense_fields_exist_and_are_denser() {
        let b = belt();
        let mut counts = Vec::new();
        for i in 0..200 {
            let th = i as f64 / 200.0 * TAU;
            let mut found = Vec::new();
            near_belt(0, 0, &b, 1, DVec3::new(21_500_000.0 * th.cos(), 0.0, 21_500_000.0 * th.sin()), 0.0, 12_000.0, &mut found);
            counts.push(found.len());
        }
        counts.sort();
        let (low, high) = (counts[40], counts[195]);
        assert!(high >= low * 3 + 5, "champs denses {low} .. {high}");
        assert!(high < 2000, "budget : {high}");
        assert!(densest_field(0, 0, &b, 0.0).is_some());
    }

    #[test]
    fn landing_params_are_microgravity() {
        let b = belt();
        let a = densest_field(0, 0, &b, 0.0).unwrap();
        let p = a.body_params(1.0);
        assert!(p.gravity > 0.0 && p.gravity < 0.05, "{}", p.gravity);
        assert!(p.airless && p.pressure == 0.0);
        assert!(a.ores().iter().all(|(_, t)| *t > 0.0));
        // Le terrain reprend la forme exacte de l'astéroïde
        let t = crate::terrain::Terrain::new(p);
        assert!(t.caves.is_none());
        for d in [Vec3::X, Vec3::Y, Vec3::new(0.3, -0.5, 0.8).normalize()] {
            let top = t.ground(d).top;
            assert!((top - a.shape.radius_at(d)).abs() <= t.voxel() * 1.01, "{top} vs {}", a.shape.radius_at(d));
        }
    }

    /// Un système avec une géante à anneaux, des Troyens et des comètes.
    fn rich_system() -> (u32, Sources, Vec<PlanetConfig>) {
        let settings = GameSettings::default();
        for (si, sys) in settings.systems.dense().iter().enumerate().take(3000) {
            let src = Sources::of(sys);
            if !src.swarms.is_empty() && !src.rings.is_empty() && !src.comets.is_empty() {
                return (si as u32, src, sys.planets_uncached().into_owned());
            }
        }
        panic!("aucun systeme avec anneaux, Troyens et cometes");
    }

    #[test]
    fn elements_match_the_planet_orbits() {
        let (_, _, planets) = rich_system();
        for p in planets.iter().filter(|p| !p.rogue) {
            let el = Elements::of_planet(p);
            let k = crate::kepler::OrbitalElements { a: p.orbit_distance, e: p.eccentricity, i: p.inclination, omega_big: p.ascending_node, omega: p.arg_periapsis, m0: p.mean_anomaly_0 };
            for t in [0.0, 1234.5, 98_765.0] {
                let a = el.position(t, 0.0);
                let b = k.position(t, crate::kepler::DEFAULT_MU * crate::planet::PLANET_MU_SCALE).as_dvec3();
                assert!(a.distance(b) < p.orbit_distance as f64 * 1e-4, "{a} vs {b}");
            }
        }
    }

    #[test]
    fn trojans_gather_sixty_degrees_from_their_giant() {
        let (si, src, planets) = rich_system();
        let (k, (sw, el, _)) = src.swarms.iter().enumerate().next().unwrap();
        let t = 5000.0;
        let lead = if sw.leading { TAU / 6.0 } else { -TAU / 6.0 };
        let l = el.position(t, lead);
        let mut found = Vec::new();
        for level in 0..3u8 {
            near_swarm(si, k as u8, sw, el, 1.0, level, l, t, LEVELS[level as usize].reach, &mut found);
        }
        assert!(!found.is_empty(), "des Troyens autour de L4/L5");
        let giant = el.position(t, 0.0);
        for a in &found {
            let p = a.rel_position(t);
            // A 60 degres de la geante, sur son orbite
            let angle = p.normalize().dot(giant.normalize()).clamp(-1.0, 1.0).acos().to_degrees();
            assert!((40.0..80.0).contains(&angle), "{angle}");
            assert_eq!(find(&src, a.key).map(|f| f.shape), Some(a.shape));
        }
        assert!(planets[sw.planet as usize].kind.gaseous());
    }

    #[test]
    fn ring_particles_fill_the_ring_but_not_its_gaps() {
        let (si, src, _) = rich_system();
        let (k, rs) = src.rings.iter().enumerate().next().unwrap();
        let t = 777.0;
        let (p, q) = rs.plane(t);
        let mut counts = Vec::new();
        for i in 1..20 {
            let f = i as f32 / 20.0;
            let rho = rs.ring.inner + (rs.ring.outer - rs.ring.inner) * f;
            let point = p + q.as_dquat() * DVec3::new(rho as f64, 0.0, 0.0);
            let mut found = Vec::new();
            near_ring(si, k as u8, rs, point, t, 600.0, &mut found);
            for a in &found {
                // Dans le plan de l'anneau, entre ses bords
                let local = q.inverse().as_dquat() * (a.rel_position(t) - p);
                assert!(local.y.abs() <= rs.half_thickness() + 1.0);
                let r = (local.x * local.x + local.z * local.z).sqrt();
                assert!(r >= rs.ring.inner as f64 - 1.0 && r <= rs.ring.outer as f64 + 1.0);
                assert!(a.shape.radius <= RING_SIZES.1);
            }
            counts.push((rs.ring.profile(f), found.len()));
        }
        assert!(counts.iter().any(|(_, n)| *n > 3), "{counts:?}");
        // Là où l'anneau est vide (division), presque rien
        for (alpha, n) in &counts {
            if *alpha < 0.01 {
                assert!(*n <= 2, "{counts:?}");
            }
        }
    }

    #[test]
    fn comets_follow_their_orbit_and_grow_tails_near_the_star() {
        let (si, src, _) = rich_system();
        let c = src.comets[0];
        let a = comets_of(si, &src).next().unwrap();
        assert!(a.landable() && a.class() == AsteroidClass::Ice);
        assert_eq!(find(&src, a.key).map(|f| f.shape), Some(a.shape));
        let el = Elements::of_comet(&c);
        // Au périhélie et à l'aphélie : bonnes distances, queue seulement près de l'étoile
        let (mut near_r, mut far_r) = (f64::MAX, 0.0f64);
        for k in 0..2000 {
            let r = a.rel_position(el.period() * k as f64 / 2000.0).length();
            near_r = near_r.min(r);
            far_r = far_r.max(r);
        }
        assert!((near_r / c.perihelion() - 1.0).abs() < 0.05, "{near_r} vs {}", c.perihelion());
        assert!((far_r / c.aphelion() - 1.0).abs() < 0.01);
        let (_, tail_far) = tail_size(&c, c.activity(far_r));
        let (_, tail_near) = tail_size(&c, c.activity(near_r));
        assert_eq!(tail_far, 0.0);
        assert!(tail_near > 100_000.0, "{tail_near}");
    }

    #[test]
    fn damage_grows_with_speed() {
        assert_eq!(impact_damage(30.0), 0);
        assert!(impact_damage(120.0) > 0 && impact_damage(120.0) < 20);
        assert!(impact_damage(480.0) > impact_damage(120.0));
        assert_eq!(impact_damage(2000.0), MAX_HP);
    }

    /// Les vrais systèmes : streaming autour de la caméra, placement, maillages hors du fil
    /// principal, collision.
    #[test]
    fn field_streams_places_meshes_and_pushes_the_ship_out() {
        let settings = GameSettings::default();
        let (si, a) = settings
            .systems
            .dense()
            .iter()
            .enumerate()
            .take(500)
            .find_map(|(si, s)| s.belts().iter().enumerate().find_map(|(bi, b)| densest_field(si as u32, bi as u8, b, 0.0)).map(|a| (si, a)))
            .expect("une ceinture dans les 500 premiers systemes");
        let center = settings.systems[si].abs_center().as_dvec3();
        let pos = world_of(center, a.rel_position(0.0));
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .insert_resource(settings)
            .insert_resource(SpawnedSystems([si].into_iter().collect()))
            .insert_resource(CameraTarget(TargetKind::Asteroid(a.key)))
            .init_resource::<Surface>()
            .init_resource::<WorldClock>()
            .init_resource::<crate::origin::OriginEpoch>()
            .init_resource::<Net>()
            .init_resource::<AsteroidField>()
            .add_event::<AsteroidHit>()
            .add_systems(Startup, setup_assets)
            .add_systems(PreUpdate, place_asteroids)
            .add_systems(Update, stream_asteroids)
            .add_systems(PostUpdate, ship_collisions);
        app.world_mut().spawn((Camera3d::default(), Transform::from_translation(pos + Vec3::Y * 3000.0)));
        let ship = app.world_mut().spawn((Transform::from_translation(pos + Vec3::Y * 9000.0), Ship)).id();
        app.world_mut().resource_mut::<AsteroidField>().refresh_now();
        app.update();
        app.update();
        let field = app.world().resource::<AsteroidField>();
        assert!(field.len() > 10, "champ dense : {} asteroides", field.len());
        let pose = field.pose(&a.key).expect("la cible existe");
        assert!(pose.translation.distance(pos) < 10.0, "{} vs {pos}", pose.translation);
        assert!(field.sources(si).is_some_and(|s| !s.belts.is_empty()));
        // Les maillages des gros arrivent
        let mut meshed = false;
        for _ in 0..300 {
            app.update();
            if app.world().resource::<AsteroidField>().live.get(&a.key).is_some_and(|l| l.meshed) {
                meshed = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(meshed, "maillage du gros asteroide");
        // Le vaisseau a toujours une position valide (collisions passées sur tout le champ)
        assert!(app.world().entity(ship).get::<Transform>().unwrap().translation.is_finite());
    }

    #[test]
    fn contact_pushes_out_along_the_surface() {
        let s = AsteroidShape::new(AsteroidClass::S, 3, 100.0);
        let pose = Transform::from_translation(Vec3::new(10.0, -4.0, 7.0)).with_rotation(Quat::from_rotation_y(0.7));
        // Loin : rien ; dedans : poussé juste au-delà de la surface
        assert!(contact(&s, &pose, pose.translation + Vec3::X * 1000.0, 3.0).is_none());
        for dir in [Vec3::X, Vec3::NEG_Y, Vec3::new(0.4, 0.2, -0.9).normalize()] {
            let ship = pose.translation + dir * 20.0;
            let (normal, pen) = contact(&s, &pose, ship, 3.0).expect("dedans");
            let out = ship + normal * pen;
            let local = pose.rotation.inverse() * (out - pose.translation);
            assert!((local.length() - (s.radius_at(local.normalize()) + 3.0 * 0.6)).abs() < 0.01);
        }
    }

    #[test]
    #[ignore]
    fn bench_asteroids() {
        let b = belt();
        let t0 = std::time::Instant::now();
        let mut total = 0;
        for i in 0..20 {
            let rel = DVec3::new(21_500_000.0, 0.0, i as f64 * 50_000.0);
            for level in 0..3u8 {
                let mut found = Vec::new();
                near_belt(0, 0, &b, level, rel, 0.0, LEVELS[level as usize].reach, &mut found);
                total += found.len();
            }
        }
        println!("cellules autour de la camera : {:.2} ms par mise a jour ({} asteroides en moyenne)", t0.elapsed().as_secs_f64() * 1000.0 / 20.0, total / 20);
        // Troyens, anneau : la caméra dans un essaim, puis dans un anneau
        let (si, src, _) = rich_system();
        let settings = GameSettings::default();
        let sys = &settings.systems[si as usize];
        sys.planets();
        let t0 = std::time::Instant::now();
        let src2 = Sources::of(sys);
        println!("petits corps d'un systeme (Sources::of) : {:.2} ms", t0.elapsed().as_secs_f64() * 1000.0);
        let (sw, el, _) = &src.swarms[0];
        let l = el.position(0.0, if sw.leading { TAU / 6.0 } else { -TAU / 6.0 });
        let rs = &src.rings[0];
        let (p, q) = rs.plane(0.0);
        let in_ring = p + q.as_dquat() * DVec3::new((rs.ring.inner as f64 + rs.ring.outer as f64) * 0.5, 0.0, 0.0);
        for (what, at) in [("essaim de Troyens", l), ("anneau", in_ring)] {
            let t0 = std::time::Instant::now();
            let mut found = Vec::new();
            near_all(si, &src2, at, 0.0, &mut found);
            println!("{what} : {:.2} ms, {} asteroides", t0.elapsed().as_secs_f64() * 1000.0, found.len());
        }
        let s = AsteroidShape::new(AsteroidClass::S, 7, 300.0);
        let t0 = std::time::Instant::now();
        let m = build_mesh(&s, 40);
        println!("maillage d'un gros (40 par face) : {:.1} ms, {} sommets", t0.elapsed().as_secs_f64() * 1000.0, m.count_vertices());
    }
}

/// Mesure du tremblement (C8) : avec `SPACESPORE_COMET_LOG`, écrit à chaque image la position
/// rendue de l'astéroïde ou de la comète ciblé, du vaisseau et de la caméra (après la propagation
/// des transformations : ce que l'on voit).
fn comet_log(
    time: Res<Time>,
    target: Res<CameraTarget>,
    field: Res<AsteroidField>,
    bodies: Query<&GlobalTransform, With<AsteroidBody>>,
    ship_q: Query<&GlobalTransform, With<crate::Ship>>,
    cam_q: Query<&GlobalTransform, With<Camera3d>>,
) {
    if std::env::var("SPACESPORE_COMET_LOG").is_err() {
        return;
    }
    let TargetKind::Asteroid(k) = target.0 else { return };
    let Some(live) = field.live.get(&k) else { return };
    let (Ok(body), Ok(ship), Ok(cam)) = (bodies.get(live.entity), ship_q.get_single(), cam_q.get_single()) else { return };
    let (b, s, c) = (body.translation(), ship.translation(), cam.translation());
    eprintln!("COMET {:.4} {:.3} {:.3} {:.3} {:.3} {:.3} {:.3} {:.3} {:.3} {:.3}", time.delta_secs(), b.x - c.x, b.y - c.y, b.z - c.z, s.x - c.x, s.y - c.y, s.z - c.z, c.x, c.y, c.z);
}
