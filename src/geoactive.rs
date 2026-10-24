//! Géologie active (0.13 T5, 0.14 D2) : geysers, fumerolles, cryovolcans, coulées de lave lumineuses et
//! petits séismes, d'après l'activité de `planetgen::geology` (volcanisme, séismes) et l'eau.
//!
//! Tout est f(graine, horloge) (règle 12) : les évents sont hachés par cellule d'environ 400
//! voxels sur la sphère-cube (un principal, et jusqu'à trois secondaires pour les champs de geysers et de
//! fumerolles), leur force à un instant ne dépend que de leur graine et de `WorldClock::secs` ; deux joueurs
//! voient le même geyser jaillir au même moment.
//!
//! 0.14 D2 : les évents **façonnent le sol** (`GeoForms`, par les colonnes de `terrain.rs`, règle 16) : cône
//! de sinter blanc à bouche pour un geyser (géant avec le trait « geysers géants ») entouré de bassins d'eau
//! chaude turquoise, tache de soufre pour une fumerolle, cône de glace pour un cryovolcan, cône de cendres
//! à cratère pour un évent de lave avec sa coulée figée en basalte (lac de lave permanent avec le trait
//! « lacs de lave actifs »). La lave luit la nuit (sommets marqués `terrain::GLOW_ALPHA`, sol et orbite).
//! Les séismes font tomber des éboulis sur les pentes.

use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::view::NoFrustumCulling;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::planet::{MoonId, MoonRoot, PlanetId, PlanetRoot, VoxelType};
use crate::planetgen::seeds::splitmix64;
use crate::surface::{Surface, SurfaceControl};
use crate::terrain::{dir_to_face, face_dir, BodyParams, Terrain};
use crate::ui::TargetKind;
use crate::world_clock::WorldClock;

/// Ce que la géologie de l'astre permet (copié dans `BodyParams`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GeoActivity {
    /// Volcanisme (0 à 1).
    pub volcanism: f32,
    /// Séismes (0 à 1).
    pub quakes: f32,
    /// Océan sous la glace chauffé par les marées (Encelade) : cryovolcans.
    pub cryo: bool,
    /// Trait « geysers géants » (0.14 D2) : geysers quatre fois plus hauts, cônes plus larges.
    pub giant_geysers: bool,
    /// Trait « lacs de lave actifs » : un lac de lave permanent dans chaque cratère d'évent de lave.
    pub lava_lakes: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VentKind {
    Geyser,
    Fumarole,
    Cryovolcano,
    Lava,
}

impl VentKind {
    pub const ALL: [VentKind; 4] = [VentKind::Geyser, VentKind::Fumarole, VentKind::Cryovolcano, VentKind::Lava];

    pub fn name(self) -> &'static str {
        match self {
            VentKind::Geyser => "geyser",
            VentKind::Fumarole => "fumerolle",
            VentKind::Cryovolcano => "cryovolcan",
            VentKind::Lava => "lave",
        }
    }

    pub fn parse(s: &str) -> Option<VentKind> {
        let s = s.trim().to_lowercase();
        Self::ALL.into_iter().find(|k| !s.is_empty() && k.name().starts_with(&s))
    }
}

/// Un évent : sa sorte, sa direction (repère fixe de l'astre), sa graine.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vent {
    pub kind: VentKind,
    pub dir: Vec3,
    pub seed: u64,
}

/// Côté d'une cellule d'évents, en voxels.
const CELL_VOXELS: f32 = 400.0;

fn unit(h: u64) -> f32 {
    (h >> 40) as f32 / (1u64 << 24) as f32
}

fn cell_hash(seed: u32, face: u8, x: i64, y: i64) -> u64 {
    splitmix64(splitmix64((seed as u64) ^ ((face as u64) << 56) ^ 0x7E57) ^ (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F))
}

/// Cellules par côté de face.
pub fn cells(p: &BodyParams) -> i64 {
    ((std::f32::consts::FRAC_PI_2 * p.radius) / (CELL_VOXELS * p.layout().voxel)).round().max(1.0) as i64
}

/// Cellule (face, x, y) qui contient `dir`.
pub fn cell_of(p: &BodyParams, dir: Vec3) -> (u8, i64, i64) {
    let n = cells(p);
    let (face, s, t) = dir_to_face(dir);
    let x = (((s + 1.0) * 0.5 * n as f32).floor() as i64).clamp(0, n - 1);
    let y = (((t + 1.0) * 0.5 * n as f32).floor() as i64).clamp(0, n - 1);
    (face, x, y)
}

/// L'évent de la cellule, s'il y en a un. `ground` : matière du sol dans une direction.
pub fn cell_vent(p: &BodyParams, (face, x, y): (u8, i64, i64), ground: &dyn Fn(Vec3) -> VoxelType) -> Option<Vent> {
    let g = p.geo;
    if p.gaseous || p.asteroid.is_some() {
        return None;
    }
    let n = cells(p) as f32;
    let mut h = cell_hash(p.seed, face, x, y);
    let mut next = || {
        h = splitmix64(h);
        unit(h)
    };
    let s = -1.0 + 2.0 * (x as f32 + 0.2 + 0.6 * next()) / n;
    let t = -1.0 + 2.0 * (y as f32 + 0.2 + 0.6 * next()) / n;
    let dir = face_dir(face, s, t);
    let roll = next();
    let air = p.atmosphere && !p.airless && p.pressure > 0.01;
    let water = air && p.hydro.liquid == crate::planetgen::hydrology::Liquid::Water && p.temperature > -15.0;
    // Une seule sorte par cellule, tirée dans l'ordre
    let mut acc = 0.0;
    let mut pick = |chance: f32| {
        acc += chance.max(0.0);
        roll < acc
    };
    let kind = if pick((g.volcanism - 0.45) * 0.3) {
        VentKind::Lava
    } else if water && pick(0.02 + g.volcanism * 0.12) {
        VentKind::Geyser
    } else if air && pick(g.volcanism * 0.15) {
        VentKind::Fumarole
    } else if g.cryo && p.temperature < -40.0 && pick(0.2) {
        VentKind::Cryovolcano
    } else {
        return None;
    };
    // Jamais sous la mer
    let ground = ground(dir);
    if ground.is_liquid() {
        return None;
    }
    Some(Vent { kind, dir, seed: splitmix64(h) })
}

/// Tous les évents de la cellule (0.14 D2) : le principal, et pour un geyser ou une fumerolle jusqu'à trois
/// secondaires de la même sorte à moins de 60 voxels (champ de geysers), jamais dans l'eau.
pub fn cell_vents(p: &BodyParams, cell: (u8, i64, i64), ground: &dyn Fn(Vec3) -> VoxelType) -> Vec<Vent> {
    let Some(main) = cell_vent(p, cell, ground) else { return Vec::new() };
    let mut out = vec![main];
    if !matches!(main.kind, VentKind::Geyser | VentKind::Fumarole) {
        return out;
    }
    let voxel = p.layout().voxel;
    let mut h = main.seed ^ 0xF1E1D;
    let mut next = || {
        h = splitmix64(h);
        unit(h)
    };
    let extra = (next() * 4.0) as usize;
    let up = main.dir;
    let e1 = up.any_orthonormal_vector();
    let e2 = up.cross(e1);
    for k in 0..extra {
        let az = next() * std::f32::consts::TAU;
        let dist = (18.0 + 42.0 * next()) * voxel / p.radius.max(1.0);
        let dir = (up * dist.cos() + (e1 * az.cos() + e2 * az.sin()) * dist.sin()).normalize();
        if ground(dir).is_liquid() {
            continue;
        }
        out.push(Vent { kind: main.kind, dir, seed: splitmix64(main.seed ^ (k as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15)) });
    }
    out
}

/// Évents dans un carré de (2 `reach` + 1)² cellules autour de `dir` (même face).
pub fn vents_near(p: &BodyParams, dir: Vec3, reach: i64, ground: &dyn Fn(Vec3) -> VoxelType) -> Vec<Vent> {
    let n = cells(p);
    let (face, x, y) = cell_of(p, dir);
    let mut out = Vec::new();
    for cy in (y - reach).max(0)..=(y + reach).min(n - 1) {
        for cx in (x - reach).max(0)..=(x + reach).min(n - 1) {
            out.extend(cell_vents(p, (face, cx, cy), ground));
        }
    }
    out
}

/// Force de l'évent (0 à 1) à l'instant `t` (secondes de jeu) : le geyser jaillit quelques
/// dizaines de secondes toutes les quelques minutes, la fumerolle fume toujours, le cryovolcan
/// enfle et faiblit avec les heures, la lave coule pendant des éruptions de quelques heures.
pub fn strength(v: &Vent, t: f64) -> f32 {
    strength_in(v, t, &GeoActivity::default())
}

/// Comme `strength`, avec les traits de l'astre : un geyser géant jaillit plus longtemps et moins souvent, un lac
/// de lave actif ne refroidit jamais tout à fait.
pub fn strength_in(v: &Vent, t: f64, g: &GeoActivity) -> f32 {
    let r = |k: u64| unit(splitmix64(v.seed ^ k)) as f64;
    match v.kind {
        VentKind::Geyser => {
            let big = if g.giant_geysers { 2.0 } else { 1.0 };
            let period = (90.0 + 420.0 * r(1)) * big;
            let length = (8.0 + 22.0 * r(2)) * big;
            let x = (t + period * r(3)).rem_euclid(period);
            if x < length {
                let u = x / length;
                // Montée brusque, retombée lente
                ((u * 8.0).min(1.0) * (1.0 - u).powf(0.6)) as f32
            } else {
                0.06
            }
        }
        VentKind::Fumarole => (0.55 + 0.25 * (t / (40.0 + 60.0 * r(1)) + 6.28 * r(2)).sin() + 0.15 * (t / 7.0 + 6.28 * r(3)).sin()) as f32,
        VentKind::Cryovolcano => (0.5 + 0.4 * (t / (3600.0 * (1.0 + 3.0 * r(1))) * std::f64::consts::TAU + 6.28 * r(2)).sin()) as f32,
        VentKind::Lava => {
            let period = 3600.0 * (2.0 + 4.0 * r(1));
            let length = period * (0.3 + 0.3 * r(2));
            let x = (t + period * r(3)).rem_euclid(period);
            if x < length {
                1.0
            } else {
                // Refroidissement : la coulée s'assombrit en une demi-heure (jamais tout à fait sur un lac de lave actif)
                ((1.0 - (x - length) / 1800.0).max(0.0) as f32 * 0.8).max(if g.lava_lakes { 0.5 } else { 0.0 })
            }
        }
    }
}

/// Séisme ressenti à l'instant `t` (0 à 1) : des tranches de 4 minutes, dans certaines un séisme de
/// quelques secondes, plus fréquents et plus forts sur un monde actif.
pub fn quake(seed: u32, quakes: f32, t: f64) -> f32 {
    if quakes <= 0.0 {
        return 0.0;
    }
    const BIN: f64 = 240.0;
    let bin = (t / BIN).floor();
    let mut best: f32 = 0.0;
    // Le séisme de la tranche précédente peut déborder sur celle-ci
    for b in [bin - 1.0, bin] {
        let h = splitmix64((seed as u64) ^ 0x5E15_0000 ^ (b as i64 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
        if unit(h) > quakes * 0.3 {
            continue;
        }
        let start = b * BIN + BIN * unit(splitmix64(h ^ 1)) as f64;
        let length = 3.0 + 6.0 * unit(splitmix64(h ^ 2)) as f64;
        let mag = (0.3 + 0.7 * unit(splitmix64(h ^ 3))) * quakes.sqrt();
        let x = t - start;
        if (0.0..length).contains(&x) {
            let u = (x / length) as f32;
            best = best.max(mag * (u * 10.0).min(1.0) * (1.0 - u));
        }
    }
    best
}

/// Séisme forcé (`/geologie seisme`, tests) : instant de départ (bits d'un f64), 0 = aucun.
static FORCED_QUAKE: AtomicU64 = AtomicU64::new(0);

pub fn force_quake(t: f64) {
    FORCED_QUAKE.store(t.to_bits(), Ordering::Relaxed);
}

fn forced_quake(t: f64) -> f32 {
    let start = f64::from_bits(FORCED_QUAKE.load(Ordering::Relaxed));
    let x = t - start;
    if start != 0.0 && (0.0..6.0).contains(&x) {
        let u = (x / 6.0) as f32;
        0.8 * (u * 10.0).min(1.0) * (1.0 - u)
    } else {
        0.0
    }
}

/// Une particule : position (repère de l'astre), demi-taille, couleur (alpha compris).
type Particle = (Vec3, f32, [f32; 4]);

/// Particules d'un évent de force `s` dont la bouche est en `base` (sur le sol).
fn vent_particles(v: &Vent, base: Vec3, s: f32, t: f64, voxel: f32, giant: bool, out: &mut Vec<Particle>) {
    let up = base.normalize();
    let e1 = up.any_orthonormal_vector();
    let e2 = up.cross(e1);
    // (hauteur, nombre, durée de vie, rayon de base, évasement, taille, couleur)
    let (height, count, life, r0, spread, size, color) = match v.kind {
        VentKind::Geyser if giant => (180.0, 300, 5.0, 2.5, 0.1, 1.6, [0.95, 0.97, 1.0, 0.75]),
        VentKind::Geyser => (45.0, 160, 3.0, 0.8, 0.12, 0.7, [0.95, 0.97, 1.0, 0.75]),
        VentKind::Fumarole => (16.0, 50, 6.0, 1.5, 0.35, 1.0, [0.86, 0.84, 0.72, 0.35]),
        VentKind::Cryovolcano => (320.0, 240, 12.0, 3.0, 0.25, 3.0, [0.86, 0.93, 1.0, 0.5]),
        VentKind::Lava => (14.0, 90, 1.6, 1.5, 0.4, 0.14, [2.4, 0.8, 0.12, 0.95]),
    };
    let n = ((count as f32) * s.clamp(0.0, 1.0)).ceil() as usize;
    for i in 0..n {
        let h = splitmix64(v.seed ^ (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let ph = ((t / life + unit(h) as f64).fract()) as f32;
        let a = unit(splitmix64(h ^ 1)) * std::f32::consts::TAU;
        let rr = unit(splitmix64(h ^ 2)).sqrt();
        let z = ph * height * (0.6 + 0.4 * s) * (0.7 + 0.3 * unit(splitmix64(h ^ 3)));
        // Les gerbes montent puis s'évasent ; la lave retombe en arc
        let lift = if v.kind == VentKind::Lava { z * (1.0 - ph) * 2.0 } else { z };
        let radial = (r0 + spread * z) * rr;
        let p = base + (up * lift + (e1 * a.cos() + e2 * a.sin()) * radial) * voxel;
        let fade = (1.0 - ph).powf(0.7) * (ph * 6.0).min(1.0);
        out.push((p, size * voxel * (0.6 + 1.2 * ph), [color[0], color[1], color[2], color[3] * fade]));
    }
}

/// Lit d'une coulée de lave : le chemin de la plus grande pente depuis l'évent (pas de `step` voxels), sur le
/// relief naturel (le même que les formes du sol, règle 16) ; directions unitaires.
fn lava_path_dirs(t: &Terrain, v: &Vent, steps: usize, step: f32) -> Vec<Vec3> {
    let voxel = t.voxel();
    let mut dir = v.dir;
    let mut h = t.ground_radius(dir);
    let mut out = vec![dir];
    for _ in 0..steps {
        let up = dir;
        let e1 = up.any_orthonormal_vector();
        let e2 = up.cross(e1);
        let mut best: Option<(f32, Vec3)> = None;
        for k in 0..8 {
            let a = k as f32 * std::f32::consts::FRAC_PI_4;
            let d = (up * t.params.radius + (e1 * a.cos() + e2 * a.sin()) * step * voxel).normalize();
            let g = t.ground_radius(d);
            if best.is_none_or(|b| g < b.0) {
                best = Some((g, d));
            }
        }
        let Some((g, d)) = best else { break };
        if g >= h {
            break;
        }
        dir = d;
        h = g;
        out.push(d);
    }
    out
}

/// Lit d'une coulée de lave en points (rayon du sol réel, formes comprises), pour la lave qui luit.
fn lava_path(t: &Terrain, v: &Vent, steps: usize) -> Vec<Vec3> {
    lava_path_dirs(t, v, steps, 2.5).into_iter().map(|d| d * t.ground(d).top).collect()
}

// ─────────────────────────────────────────────────────────────────────────
//  Formes au sol (0.14 D2)
// ─────────────────────────────────────────────────────────────────────────

/// Un évent et la forme qu'il donne au sol (unités).
#[derive(Clone, Debug)]
pub struct Shaped {
    pub vent: Vent,
    /// Cône : rayon et hauteur ; bouche : rayon et profondeur.
    cone_r: f32,
    cone_h: f32,
    hole_r: f32,
    hole_d: f32,
    /// Matière du cône.
    kind: VoxelType,
    /// Bassins d'eau chaude : direction, rayon, rayon du sol naturel au centre.
    pools: Vec<(Vec3, f32, f32)>,
    /// Coulée figée : directions des points du lit.
    flow: Vec<Vec3>,
    /// Portée angulaire de tout cela.
    reach: f32,
}

/// Ce qu'un évent fait à une colonne : nouveau rayon du sol, matière, liquide (surface, matière, couleur).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Form {
    pub ground: f32,
    pub kind: Option<VoxelType>,
    pub water: Option<(f32, VoxelType, [f32; 4])>,
}

/// Formes des évents d'un astre, par cellule : partagées par toutes les tuiles et tous les `Terrain` du même astre
/// (`of`, cache global comme `rivers.rs`), générées une fois par cellule.
pub struct GeoForms {
    /// Les évents d'une cellule, façonnés.
    own: Mutex<HashMap<(u8, i64, i64), Arc<Vec<Shaped>>>>,
    /// Ceux dont la forme touche une cellule (les siens et ceux des voisines).
    near: Mutex<HashMap<(u8, i64, i64), Arc<Vec<Shaped>>>>,
}

type GeoKey = (u32, u32, u32, u32, u32, u32, u32, bool, bool);

static FORMS: Mutex<Vec<(GeoKey, Arc<GeoForms>)>> = Mutex::new(Vec::new());

/// Couleur de l'eau chaude des bassins (turquoise laiteux).
pub const HOT_SPRING_COLOR: [f32; 4] = [0.36, 0.78, 0.76, 1.0];

impl GeoForms {
    pub fn wanted(p: &BodyParams) -> bool {
        !p.gaseous && p.asteroid.is_none()
    }

    pub fn new() -> Self {
        Self { own: Mutex::new(HashMap::new()), near: Mutex::new(HashMap::new()) }
    }

    /// Les formes de cet astre, partagées (8 astres gardés).
    pub fn of(p: &BodyParams, voxel: f32) -> Option<Arc<GeoForms>> {
        // (`SPACESPORE_NO_GEO=1` : mesures avant / après, `bench_tiles`)
        if !Self::wanted(p) || std::env::var_os("SPACESPORE_NO_GEO").is_some() {
            return None;
        }
        let key: GeoKey = (p.seed, p.radius.to_bits(), voxel.to_bits(), p.terrain_height.to_bits(), p.sea_level.to_bits(), p.geo.volcanism.to_bits(), p.temperature.to_bits(), p.geo.giant_geysers, p.geo.lava_lakes);
        let mut all = FORMS.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((_, g)) = all.iter().find(|(k, _)| *k == key) {
            return Some(g.clone());
        }
        let g = Arc::new(Self::new());
        all.push((key, g.clone()));
        if all.len() > 8 {
            all.remove(0);
        }
        Some(g)
    }

    /// La forme d'un évent, d'après le relief naturel.
    fn shaped(t: &Terrain, v: Vent) -> Shaped {
        let p = &t.params;
        let vx = t.voxel();
        let mut h = v.seed ^ 0x5A4D;
        let mut next = || {
            h = splitmix64(h);
            unit(h)
        };
        let (cone_r, cone_h, hole_r, hole_d, kind) = match v.kind {
            VentKind::Geyser if p.geo.giant_geysers => (30.0, 10.0, 4.0, 8.0, VoxelType::Salt),
            VentKind::Geyser => (10.0, 3.5, 1.6, 3.0, VoxelType::Salt),
            VentKind::Fumarole => (5.0, 1.0, 1.0, 2.0, VoxelType::Sulfur),
            VentKind::Cryovolcano => (36.0, 14.0, 3.0, 6.0, VoxelType::Ice),
            VentKind::Lava => (45.0, 20.0, 9.0, 7.0, VoxelType::Basalt),
        };
        let mut reach = cone_r + 2.0;
        let mut pools = Vec::new();
        let up = v.dir;
        let e1 = up.any_orthonormal_vector();
        let e2 = up.cross(e1);
        if v.kind == VentKind::Geyser {
            let big = if p.geo.giant_geysers { 2.0 } else { 1.0 };
            let n = 2 + (next() * 2.0) as usize;
            for _ in 0..n {
                let az = next() * std::f32::consts::TAU;
                let dist = (cone_r + 2.0 + 14.0 * next()) * big;
                let r = (3.0 + 2.5 * next()) * big;
                let ang = dist * vx / p.radius.max(1.0);
                let dir = (up * ang.cos() + (e1 * az.cos() + e2 * az.sin()) * ang.sin()).normalize();
                let g = t.ground_radius(dir);
                if g < p.radius + 2.0 * vx {
                    continue;
                }
                pools.push((dir, r * vx, g));
                reach = reach.max(dist + r + 3.0);
            }
        }
        let mut flow = Vec::new();
        if v.kind == VentKind::Lava {
            flow = lava_path_dirs(t, &v, 60, 3.0);
            for d in &flow {
                reach = reach.max((*d - up).length() * p.radius / vx + 4.0);
            }
        }
        Shaped { vent: v, cone_r: cone_r * vx, cone_h: cone_h * vx, hole_r: hole_r * vx, hole_d: hole_d * vx, kind, pools, flow, reach: reach * vx / p.radius.max(1.0) }
    }

    /// Les évents de la cellule `key`, façonnés (une fois).
    fn own(&self, t: &Terrain, key: (u8, i64, i64)) -> Arc<Vec<Shaped>> {
        if let Some(c) = self.own.lock().ok().and_then(|c| c.get(&key).cloned()) {
            return c;
        }
        let ground = |d: Vec3| t.vent_ground(d);
        let out = Arc::new(cell_vents(&t.params, key, &ground).into_iter().map(|v| Self::shaped(t, v)).collect::<Vec<_>>());
        if let Ok(mut c) = self.own.lock() {
            if c.len() > 4096 {
                c.clear();
            }
            c.insert(key, out.clone());
        }
        out
    }

    /// Les évents dont la forme touche la cellule `key` (les siens et ceux des voisines), générés une fois.
    fn cell(&self, t: &Terrain, key: (u8, i64, i64)) -> Arc<Vec<Shaped>> {
        if let Some(c) = self.near.lock().ok().and_then(|c| c.get(&key).cloned()) {
            return c;
        }
        let p = &t.params;
        let n = cells(p);
        let (face, x, y) = key;
        let center = face_dir(face, -1.0 + 2.0 * (x as f32 + 0.5) / n as f32, -1.0 + 2.0 * (y as f32 + 0.5) / n as f32);
        let half = std::f32::consts::FRAC_PI_2 / n as f32 * 0.75;
        let mut out = Vec::new();
        for cy in (y - 1).max(0)..=(y + 1).min(n - 1) {
            for cx in (x - 1).max(0)..=(x + 1).min(n - 1) {
                for s in self.own(t, (face, cx, cy)).iter() {
                    if (s.vent.dir - center).length() < half + s.reach {
                        out.push(s.clone());
                    }
                }
            }
        }
        let out = Arc::new(out);
        if let Ok(mut c) = self.near.lock() {
            if c.len() > 4096 {
                c.clear();
            }
            c.insert(key, out.clone());
        }
        out
    }

    /// Les bassins d'eau chaude des cellules autour de `dir` (0.14 D6 : lieux).
    pub fn pools_near(&self, t: &Terrain, dir: Vec3, reach: i64) -> Vec<Vec3> {
        let p = &t.params;
        let n = cells(p);
        let (face, x, y) = cell_of(p, dir);
        let mut out = Vec::new();
        for cy in (y - reach).max(0)..=(y + reach).min(n - 1) {
            for cx in (x - reach).max(0)..=(x + reach).min(n - 1) {
                for s in self.own(t, (face, cx, cy)).iter() {
                    out.extend(s.pools.iter().map(|(d, _, _)| *d));
                }
            }
        }
        out
    }

    /// Ce que les évents font à la colonne `dir` dont le sol naturel est au rayon `h`.
    pub fn at(&self, t: &Terrain, dir: Vec3, h: f32) -> Option<Form> {
        let p = &t.params;
        let list = self.cell(t, cell_of(p, dir));
        if list.is_empty() {
            return None;
        }
        let vx = t.voxel();
        let radius = p.radius.max(1.0);
        let mut f = Form { ground: h, kind: None, water: None };
        let mut hit = false;
        for s in list.iter() {
            let ang = (dir - s.vent.dir).length();
            if ang > s.reach {
                continue;
            }
            let d = ang * radius;
            let cone = if d < s.cone_r { s.cone_h * (1.0 - (d / s.cone_r).powi(2)).powf(1.5) } else { 0.0 };
            let hole = if d < s.hole_r { s.hole_d * (1.0 - (d / s.hole_r).powi(2)) } else { 0.0 };
            if cone > 0.0 || hole > 0.0 {
                hit = true;
                f.ground = f.ground.max(h + cone - hole);
                if d < s.cone_r * 0.8 {
                    f.kind = Some(s.kind);
                }
                // Lac de lave permanent au fond du cratère
                if s.vent.kind == VentKind::Lava && p.geo.lava_lakes && d < s.hole_r * 0.85 {
                    f.water = Some((h + s.cone_h - s.hole_d * 0.4, VoxelType::Lava, VoxelType::Lava.color()));
                }
            }
            for (pd, pr, pg) in &s.pools {
                let dd = (dir - *pd).length() * radius;
                if dd < pr + 2.0 * vx {
                    hit = true;
                    f.kind = Some(VoxelType::Salt);
                    if dd < *pr {
                        f.ground = f.ground.min(pg - 1.5 * vx - vx * (1.0 - (dd / pr).powi(2)));
                        f.water = Some((pg - 0.3 * vx, VoxelType::Water, HOT_SPRING_COLOR));
                    }
                }
            }
            if s.flow.len() > 1 && d >= s.cone_r * 0.8 {
                let mut best = f32::INFINITY;
                for w in s.flow.windows(2) {
                    let ab = w[1] - w[0];
                    let len2 = ab.length_squared().max(1e-12);
                    let tt = ((dir - w[0]).dot(ab) / len2).clamp(0.0, 1.0);
                    best = best.min((dir - (w[0] + ab * tt)).length() * radius);
                }
                if best < 2.5 * vx {
                    hit = true;
                    f.ground = f.ground.max(h + 0.8 * vx * (1.0 - best / (2.5 * vx)));
                    f.kind = Some(VoxelType::Basalt);
                }
            }
        }
        hit.then_some(f)
    }
}

impl Default for GeoForms {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  Affichage
// ─────────────────────────────────────────────────────────────────────────

pub struct GeoActivePlugin;

impl Plugin for GeoActivePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GeoState>().add_systems(Update, geo_fx.after(SurfaceControl));
    }
}

#[derive(Component)]
struct GeoParticles;

#[derive(Component)]
struct GeoLava;

#[derive(Component)]
struct GeoLight;

/// Évents connus de l'astre où l'on est (cache par cellule) et dernier séisme annoncé.
#[derive(Resource, Default)]
pub struct GeoState {
    body: Option<TargetKind>,
    cells: HashMap<(u8, i64, i64), Vec<Vent>>,
    /// Coulée calculée (graine de l'évent, chemin).
    lava: Option<(u64, Vec<Vec3>)>,
    announced: f64,
    /// Séisme en cours (0 à 1), pour le HUD et les tests.
    pub quake: f32,
}

/// Particules au plus.
const MAX_PARTICLES: usize = 1200;
/// Distance (voxels) jusqu'où les évents sont affichés.
const SHOW_VOXELS: f32 = 2500.0;

fn quad_mesh(n: usize) -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0f32; 3]; n * 4]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[1.0f32; 4]; n * 4]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 1.0, 0.0]; n * 4]);
    mesh.insert_indices(Indices::U32((0..n as u32).flat_map(|q| [q * 4, q * 4 + 1, q * 4 + 2, q * 4, q * 4 + 2, q * 4 + 3]).collect()));
    mesh
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn geo_fx(
    mut commands: Commands,
    clock: Res<WorldClock>,
    time: Res<Time>,
    surface: Res<Surface>,
    mut state: ResMut<GeoState>,
    mut net: ResMut<crate::net::Net>,
    planets: Query<(Entity, &Transform, &PlanetId), With<PlanetRoot>>,
    moons: Query<(Entity, &Transform, &MoonId), With<MoonRoot>>,
    mut cam_q: Query<&mut Transform, (With<Camera3d>, Without<PlanetRoot>, Without<MoonRoot>, Without<PointLight>)>,
    mut fx: Query<(Entity, &Mesh3d, &mut Visibility, Option<&Parent>, Has<GeoLava>), Or<(With<GeoParticles>, With<GeoLava>)>>,
    mut light_q: Query<(Entity, &mut Transform, &mut PointLight, &mut Visibility, Option<&Parent>), (With<GeoLight>, Without<GeoParticles>, Without<GeoLava>, Without<Camera3d>, Without<PlanetRoot>, Without<MoonRoot>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let body = surface.body().filter(|_| surface.active());
    if state.body != body {
        state.body = body;
        state.cells.clear();
        state.lava = None;
    }
    let root = match body {
        Some(TargetKind::Planet(id)) => planets.iter().find(|(_, _, p)| p.0 == id).map(|(e, t, _)| (e, *t)),
        Some(TargetKind::Moon(pid, mi)) => moons.iter().find(|(_, _, m)| m.planet_idx == pid && m.moon_idx == mi).map(|(e, t, _)| (e, *t)),
        _ => None,
    };
    let hide = |fx: &mut Query<(Entity, &Mesh3d, &mut Visibility, Option<&Parent>, Has<GeoLava>), Or<(With<GeoParticles>, With<GeoLava>)>>| {
        for (_, _, mut v, _, _) in fx.iter_mut() {
            if *v != Visibility::Hidden {
                *v = Visibility::Hidden;
            }
        }
    };
    let (Some((root_e, root_tf)), Some(t), Ok(mut cam)) = (root, surface.terrain(), cam_q.get_single_mut()) else {
        hide(&mut fx);
        for (_, _, _, mut v, _) in &mut light_q {
            *v = Visibility::Hidden;
        }
        state.quake = 0.0;
        return;
    };
    // Créés une fois, rattachés à l'astre où l'on est
    if fx.is_empty() {
        let mat = materials.add(StandardMaterial { base_color: Color::WHITE, unlit: true, alpha_mode: AlphaMode::Blend, cull_mode: None, double_sided: true, ..default() });
        commands.spawn((Mesh3d(meshes.add(quad_mesh(MAX_PARTICLES))), MeshMaterial3d(mat), Transform::IDENTITY, Visibility::Hidden, NotShadowCaster, NoFrustumCulling, GeoParticles));
        // (sommets réécrits à chaque image : la boîte englobante calculée au départ serait fausse)
        // Lave : sans ombre ni lumière reçue, plus claire que blanc (visible la nuit)
        let lava = materials.add(StandardMaterial { base_color: Color::WHITE, unlit: true, cull_mode: None, double_sided: true, ..default() });
        commands.spawn((Mesh3d(meshes.add(quad_mesh(64))), MeshMaterial3d(lava), Transform::IDENTITY, Visibility::Hidden, NotShadowCaster, NoFrustumCulling, GeoLava));
        commands.spawn((PointLight { color: Color::srgb(1.0, 0.45, 0.12), intensity: 0.0, shadows_enabled: false, ..default() }, Transform::IDENTITY, Visibility::Hidden, GeoLight));
        return;
    }
    let p = &t.params;
    let voxel = t.voxel();
    let now = clock.secs;
    let cam_local = root_tf.rotation.inverse() * (cam.translation - root_tf.translation);
    let here = cam_local.normalize_or(Vec3::Y);

    // Évents autour de la caméra (cellules gardées en cache)
    let reach = (SHOW_VOXELS / CELL_VOXELS).ceil() as i64;
    let (face, x, y) = cell_of(p, here);
    let n = cells(p);
    let mut vents = Vec::new();
    for cy in (y - reach).max(0)..=(y + reach).min(n - 1) {
        for cx in (x - reach).max(0)..=(x + reach).min(n - 1) {
            let key = (face, cx, cy);
            let list = state.cells.entry(key).or_insert_with(|| cell_vents(p, key, &|d| t.vent_ground(d))).clone();
            for v in list {
                let base = v.dir * t.ground(v.dir).top;
                if base.distance(cam_local) < SHOW_VOXELS * voxel {
                    vents.push((v, base));
                }
            }
        }
    }
    vents.sort_by(|a, b| a.1.distance_squared(cam_local).total_cmp(&b.1.distance_squared(cam_local)));
    if state.cells.len() > 4096 {
        state.cells.clear();
    }

    // Séisme : la caméra tremble, de la poussière se lève
    let shake = quake(p.seed, p.geo.quakes, now).max(forced_quake(now));
    if shake > 0.05 && state.quake <= 0.05 && now - state.announced > 30.0 {
        state.announced = now;
        net.notify(&format!("Seisme ! (intensite {:.0} %)", shake * 100.0), time.elapsed_secs_f64());
    }
    state.quake = shake;
    if shake > 0.0 {
        let h = splitmix64((time.elapsed_secs_f64() * 60.0) as u64);
        let j = Vec3::new(unit(h) - 0.5, unit(splitmix64(h)) - 0.5, unit(splitmix64(h ^ 7)) - 0.5);
        cam.translation += j * shake * voxel * 0.5;
    }

    // Particules
    let mut parts: Vec<Particle> = Vec::new();
    for (v, base) in vents.iter().take(12) {
        let s = strength_in(v, now, &p.geo);
        if s > 0.01 {
            vent_particles(v, *base, s, now, voxel, p.geo.giant_geysers, &mut parts);
        }
    }
    // Éboulis (0.14 D2) : pendant un séisme, des pierres dévalent les pentes autour du joueur
    if shake > 0.2 {
        let bin = (now / 8.0).floor() as u64;
        let up = here;
        let e1 = up.any_orthonormal_vector();
        let e2 = up.cross(e1);
        for i in 0..40u64 {
            let h = splitmix64((p.seed as u64) ^ 0xEB0_0115 ^ bin.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ i.wrapping_mul(0xC2B2_AE3D_27D4_EB4F));
            let az = unit(h) * std::f32::consts::TAU;
            let dist = (8.0 + 50.0 * unit(splitmix64(h ^ 1))) * voxel / p.radius.max(1.0);
            let d = (up * dist.cos() + (e1 * az.cos() + e2 * az.sin()) * dist.sin()).normalize();
            let g = t.ground(d).top;
            // Seulement sur une pente : le sol descend d'au moins un demi-voxel en 3 voxels
            let step = 3.0 * voxel / p.radius.max(1.0);
            let (ga, gb) = (t.ground((d + e1 * step).normalize()).top, t.ground((d + e2 * step).normalize()).top);
            let grad = Vec3::new(ga - g, gb - g, 0.0);
            if grad.length() < 0.5 * voxel {
                continue;
            }
            let down = -(e1 * grad.x + e2 * grad.y).normalize_or_zero();
            let start = bin as f64 * 8.0 + 2.0 * unit(splitmix64(h ^ 2)) as f64;
            let age = (now - start) as f32;
            if !(0.0..3.0).contains(&age) {
                continue;
            }
            let bounce = (age * 7.0).sin().abs() * (1.0 - age / 3.0) * 2.5 * voxel;
            let slide = down * age * 2.0 * voxel;
            let pos = d * (g + 0.6 * voxel + bounce) + slide;
            let grey = 0.28 + 0.12 * unit(splitmix64(h ^ 3));
            parts.push((pos, (0.35 + 0.3 * unit(splitmix64(h ^ 4))) * voxel, [grey, grey * 0.95, grey * 0.9, 1.0]));
        }
    }
    if shake > 0.05 && p.atmosphere {
        let ground = here * t.ground(here).top;
        let dust = Vent { kind: VentKind::Fumarole, dir: here, seed: 0xD057 };
        let mut d = Vec::new();
        vent_particles(&dust, ground, shake, now * 0.5, voxel * 1.5, false, &mut d);
        parts.extend(d.into_iter().map(|(q, sz, c)| (q, sz, [0.62, 0.52, 0.4, c[3] * 0.8])));
    }
    parts.truncate(MAX_PARTICLES);
    // Quads face à la caméra
    let cam_rot = root_tf.rotation.inverse() * cam.rotation;
    let (right, upv) = (cam_rot * Vec3::X, cam_rot * Vec3::Y);

    // Coulée de lave de l'évent de lave le plus proche
    let lava_vent = vents.iter().find(|(v, _)| v.kind == VentKind::Lava && strength_in(v, now, &p.geo) > 0.01).map(|(v, _)| *v);
    if let Some(v) = lava_vent {
        if state.lava.as_ref().is_none_or(|(s, _)| *s != v.seed) {
            state.lava = Some((v.seed, lava_path(t, &v, 40)));
        }
    }
    for (e, mesh_h, mut vis, parent, is_lava) in &mut fx {
        if parent.map(|p| p.get()) != Some(root_e) {
            commands.entity(root_e).add_child(e);
        }
        let Some(mesh) = meshes.get_mut(&mesh_h.0) else { continue };
        let (mut pos, mut col) = (Vec::new(), Vec::new());
        if is_lava {
            if let (Some(v), Some((_, path))) = (lava_vent, state.lava.as_ref()) {
                // La coulée s'allonge et rougeoie avec l'éruption, puis s'assombrit
                let s = strength_in(&v, now, &p.geo);
                let len = ((path.len() as f32 * s.min(1.0)).ceil() as usize).clamp(2, path.len());
                let glow = [2.2 * s, 0.75 * s * s, 0.12 * s, 1.0];
                // Bassin à la bouche de l'évent
                let mouth = path[0] + path[0].normalize() * 0.2 * voxel;
                let (e1, e2) = {
                    let up = path[0].normalize();
                    let e = up.any_orthonormal_vector();
                    (e * 3.5 * voxel, up.cross(e) * 3.5 * voxel)
                };
                pos.extend_from_slice(&[(mouth - e1 - e2).to_array(), (mouth + e1 - e2).to_array(), (mouth + e1 + e2).to_array(), (mouth - e1 + e2).to_array()]);
                col.extend_from_slice(&[glow; 4]);
                for w in path[..len].windows(2) {
                    let (a, b) = (w[0] + w[0].normalize() * 0.15 * voxel, w[1] + w[1].normalize() * 0.15 * voxel);
                    let side = (b - a).cross(a.normalize()).normalize_or(Vec3::X) * 2.0 * voxel;
                    pos.extend_from_slice(&[(a - side).to_array(), (a + side).to_array(), (b + side).to_array(), (b - side).to_array()]);
                    col.extend_from_slice(&[glow; 4]);
                }
            }
            pos.resize(64 * 4, [0.0; 3]);
            col.resize(64 * 4, [0.0; 4]);
        } else {
            for (q, sz, c) in &parts {
                let (r, u) = (right * *sz, upv * *sz);
                pos.extend_from_slice(&[(*q - r - u).to_array(), (*q + r - u).to_array(), (*q + r + u).to_array(), (*q - r + u).to_array()]);
                col.extend_from_slice(&[*c; 4]);
            }
            pos.resize(MAX_PARTICLES * 4, [0.0; 3]);
            col.resize(MAX_PARTICLES * 4, [0.0; 4]);
        }
        let any = col.iter().any(|c| c[3] > 0.0);
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, VertexAttributeValues::Float32x3(pos));
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, VertexAttributeValues::Float32x4(col));
        let wanted = if any { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != wanted {
            *vis = wanted;
        }
    }
    // Lueur de la lave sur le sol alentour
    for (e, mut tf, mut light, mut vis, parent) in &mut light_q {
        if parent.map(|p| p.get()) != Some(root_e) {
            commands.entity(root_e).add_child(e);
        }
        let s = lava_vent.map_or(0.0, |v| strength_in(&v, now, &p.geo));
        let wanted = if s > 0.01 { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != wanted {
            *vis = wanted;
        }
        if let (Some(v), true) = (lava_vent, s > 0.01) {
            tf.translation = v.dir * (t.ground(v.dir).top + 6.0 * voxel);
            light.intensity = 4.0e6 * s * voxel * voxel;
            light.range = 120.0 * voxel;
        }
    }
}

/// L'évent de cette sorte le plus proche de `dir` (dans un carré de `reach` cellules), en activité
/// à l'instant `now` si possible (une coulée en éruption plutôt qu'une coulée froide).
pub fn nearest_vent(t: &Terrain, dir: Vec3, kind: VentKind, reach: i64, now: f64) -> Option<Vent> {
    let all: Vec<Vent> = vents_near(&t.params, dir, reach, &|d| t.vent_ground(d)).into_iter().filter(|v| v.kind == kind).collect();
    let active: Vec<Vent> = all.iter().copied().filter(|v| kind == VentKind::Geyser || strength_in(v, now, &t.params.geo) > 0.3).collect();
    if active.is_empty() { all } else { active }
        .into_iter()
        .min_by(|a, b| a.dir.distance_squared(dir).total_cmp(&b.dir.distance_squared(dir)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vent(kind: VentKind) -> Vent {
        Vent { kind, dir: Vec3::Y, seed: 99 }
    }

    /// Règle 12 : la force ne dépend que de la graine et de l'horloge ; un geyser jaillit par
    /// moments (et reste calme le reste du temps), une fumerolle fume toujours.
    #[test]
    fn vents_follow_the_clock() {
        let g = vent(VentKind::Geyser);
        let samples: Vec<f32> = (0..2000).map(|k| strength(&g, k as f64 * 0.5)).collect();
        let bursts = samples.iter().filter(|s| **s > 0.5).count();
        let calm = samples.iter().filter(|s| **s < 0.1).count();
        assert!(bursts > 10 && calm > 1000, "{bursts} {calm}");
        assert_eq!(strength(&g, 1234.5), strength(&g, 1234.5));
        let f = vent(VentKind::Fumarole);
        assert!((0..500).all(|k| strength(&f, k as f64 * 3.0) > 0.1));
        let l = vent(VentKind::Lava);
        let active = (0..200).filter(|k| strength(&l, *k as f64 * 360.0) >= 1.0).count();
        assert!(active > 20 && active < 190, "{active}");
    }

    /// Séismes : rares et courts, plus fréquents sur un monde actif, aucun sur un monde mort.
    #[test]
    fn quakes_depend_on_activity() {
        let felt = |q: f32| (0..200_000).filter(|k| quake(7, q, *k as f64 * 0.5) > 0.05).count();
        let (calm, active) = (felt(0.1), felt(0.9));
        assert_eq!(felt(0.0), 0);
        assert!(active > calm * 3 && calm > 0, "{calm} {active}");
        // Moins d'un dixième du temps, même sur un monde très actif
        assert!(active < 20_000, "{active}");
    }

    /// Évents : hachés (mêmes pour la même graine), seulement là où la géologie le permet.
    #[test]
    fn vents_follow_geology() {
        let mut p = crate::terrain::tests::earth_like();
        let count = |p: &BodyParams, kind: VentKind| -> usize {
            let n = cells(p);
            (0..n.min(60)).flat_map(|x| (0..n.min(60)).map(move |y| (x, y))).filter(|&(x, y)| cell_vent(p, (2, x, y), &|_| VoxelType::Grass).is_some_and(|v| v.kind == kind)).count()
        };
        assert_eq!(count(&p, VentKind::Lava), 0);
        p.geo = GeoActivity { volcanism: 0.9, quakes: 0.5, cryo: false, giant_geysers: false, lava_lakes: false };
        let lava = count(&p, VentKind::Lava);
        let geysers = count(&p, VentKind::Geyser);
        assert!(lava > 10 && geysers > 10, "{lava} {geysers}");
        assert_eq!(count(&p, VentKind::Cryovolcano), 0);
        let again = count(&p, VentKind::Lava);
        assert_eq!(lava, again);
        // Encelade : froid, sans air, océan sous la glace
        let mut moon = p;
        moon.geo = GeoActivity { volcanism: 0.0, quakes: 0.2, cryo: true, giant_geysers: false, lava_lakes: false };
        moon.temperature = -190.0;
        moon.atmosphere = false;
        moon.airless = true;
        assert!(count(&moon, VentKind::Cryovolcano) > 10);
        assert_eq!(count(&moon, VentKind::Geyser) + count(&moon, VentKind::Fumarole), 0);
    }

    /// 0.14 D2 : un évent de lave fait un cône de basalte à cratère ; un geyser, un cône de sinter avec des
    /// bassins d'eau chaude ; le sol d'une colonne ordinaire ne change pas.
    #[test]
    fn vents_shape_the_ground() {
        let mut p = crate::terrain::tests::earth_like();
        p.geo = GeoActivity { volcanism: 0.9, quakes: 0.5, cryo: false, giant_geysers: false, lava_lakes: true };
        let t = Terrain::new(p);
        let v = t.voxel();
        let n = cells(&p);
        let mut lava = None;
        let mut geyser = None;
        'outer: for x in 0..n {
            for y in 0..n {
                for vent in cell_vents(&p, (2, x, y), &|d| t.vent_ground(d)) {
                    match vent.kind {
                        VentKind::Lava if lava.is_none() => lava = Some(vent),
                        VentKind::Geyser if geyser.is_none() => geyser = Some(vent),
                        _ => {}
                    }
                    if lava.is_some() && geyser.is_some() {
                        break 'outer;
                    }
                }
            }
        }
        let (lava, geyser) = (lava.expect("un évent de lave"), geyser.expect("un geyser"));
        // Flanc du cône de lave : plus haut que le relief naturel, en basalte
        let up = lava.dir;
        let e = up.any_orthonormal_vector();
        let flank = (up + e * (25.0 * v / p.radius)).normalize();
        let c = t.base_column(flank, v);
        assert!(c.top > t.ground_radius(flank) + 3.0 * v, "pas de cône : {} vs {}", c.top, t.ground_radius(flank));
        assert_eq!(c.kind, VoxelType::Basalt);
        // Le cratère : un lac de lave (trait « lacs de lave actifs ») sous le bord du cratère (à 10 voxels)
        let crater = t.base_column(lava.dir, v);
        assert_eq!(crater.kind, VoxelType::Lava);
        let rim = t.base_column((up + e * (10.0 * v / p.radius)).normalize(), v);
        assert!(crater.top < rim.top, "le lac ({}) est sous le bord du cratère ({})", crater.top, rim.top);
        // Geyser : sinter blanc, bassins d'eau chaude autour
        let rim = (geyser.dir + geyser.dir.any_orthonormal_vector() * (4.0 * v / p.radius)).normalize();
        assert_eq!(t.base_column(rim, v).kind, VoxelType::Salt);
        let forms = t.geo.as_ref().unwrap();
        let shaped = forms.cell(&t, cell_of(&p, geyser.dir));
        let s = shaped.iter().find(|s| s.vent.seed == geyser.seed).unwrap();
        assert!(!s.pools.is_empty());
        let pool = t.base_column(s.pools[0].0, v);
        assert_eq!(pool.kind, VoxelType::Water);
        assert!((0..3).all(|k| (pool.color[k] - HOT_SPRING_COLOR[k]).abs() < 0.12), "{:?}", pool.color);
        // Loin de tout évent : rien ne change
        let far = (geyser.dir + e * (150.0 * v / p.radius)).normalize();
        assert!(forms.at(&t, far, t.ground_radius(far)).is_none_or(|f| f.ground == t.ground_radius(far)));
    }
}
