//! Géologie active (0.13 T5) : geysers, fumerolles, cryovolcans, coulées de lave lumineuses et
//! petits séismes, d'après l'activité de `planetgen::geology` (volcanisme, séismes) et l'eau.
//!
//! Tout est f(graine, horloge) (règle 12) : les évents sont hachés par cellule d'environ 400
//! voxels sur la sphère-cube, leur force à un instant ne dépend que de leur graine et de
//! `WorldClock::secs` ; deux joueurs voient le même geyser jaillir au même moment.

use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::view::NoFrustumCulling;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

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

/// Évents dans un carré de (2 `reach` + 1)² cellules autour de `dir` (même face).
pub fn vents_near(p: &BodyParams, dir: Vec3, reach: i64, ground: &dyn Fn(Vec3) -> VoxelType) -> Vec<Vent> {
    let n = cells(p);
    let (face, x, y) = cell_of(p, dir);
    let mut out = Vec::new();
    for cy in (y - reach).max(0)..=(y + reach).min(n - 1) {
        for cx in (x - reach).max(0)..=(x + reach).min(n - 1) {
            out.extend(cell_vent(p, (face, cx, cy), ground));
        }
    }
    out
}

/// Force de l'évent (0 à 1) à l'instant `t` (secondes de jeu) : le geyser jaillit quelques
/// dizaines de secondes toutes les quelques minutes, la fumerolle fume toujours, le cryovolcan
/// enfle et faiblit avec les heures, la lave coule pendant des éruptions de quelques heures.
pub fn strength(v: &Vent, t: f64) -> f32 {
    let r = |k: u64| unit(splitmix64(v.seed ^ k)) as f64;
    match v.kind {
        VentKind::Geyser => {
            let period = 90.0 + 420.0 * r(1);
            let length = 8.0 + 22.0 * r(2);
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
                // Refroidissement : la coulée s'assombrit en une demi-heure
                (1.0 - (x - length) / 1800.0).max(0.0) as f32 * 0.8
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
fn vent_particles(v: &Vent, base: Vec3, s: f32, t: f64, voxel: f32, out: &mut Vec<Particle>) {
    let up = base.normalize();
    let e1 = up.any_orthonormal_vector();
    let e2 = up.cross(e1);
    // (hauteur, nombre, durée de vie, rayon de base, évasement, taille, couleur)
    let (height, count, life, r0, spread, size, color) = match v.kind {
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

/// Lit d'une coulée de lave : le chemin de la plus grande pente depuis l'évent (pas de 2,5 voxels).
fn lava_path(t: &Terrain, v: &Vent, steps: usize) -> Vec<Vec3> {
    let voxel = t.voxel();
    let mut dir = v.dir;
    let mut out = vec![dir * t.ground(dir).top];
    for _ in 0..steps {
        let up = dir;
        let e1 = up.any_orthonormal_vector();
        let e2 = up.cross(e1);
        let mut best: Option<(f32, Vec3)> = None;
        for k in 0..8 {
            let a = k as f32 * std::f32::consts::FRAC_PI_4;
            let d = (up * t.params.radius + (e1 * a.cos() + e2 * a.sin()) * 2.5 * voxel).normalize();
            let g = t.ground(d).top;
            if best.is_none_or(|b| g < b.0) {
                best = Some((g, d));
            }
        }
        let Some((g, d)) = best else { break };
        dir = d;
        out.push(d * g);
    }
    out
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
    cells: HashMap<(u8, i64, i64), Option<Vent>>,
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
            let v = *state.cells.entry(key).or_insert_with(|| cell_vent(p, key, &|d| t.base_column(d, voxel).kind));
            if let Some(v) = v {
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
    for (v, base) in vents.iter().take(8) {
        let s = strength(v, now);
        if s > 0.01 {
            vent_particles(v, *base, s, now, voxel, &mut parts);
        }
    }
    if shake > 0.05 && p.atmosphere {
        let ground = here * t.ground(here).top;
        let dust = Vent { kind: VentKind::Fumarole, dir: here, seed: 0xD057 };
        let mut d = Vec::new();
        vent_particles(&dust, ground, shake, now * 0.5, voxel * 1.5, &mut d);
        parts.extend(d.into_iter().map(|(q, sz, c)| (q, sz, [0.62, 0.52, 0.4, c[3] * 0.8])));
    }
    parts.truncate(MAX_PARTICLES);
    // Quads face à la caméra
    let cam_rot = root_tf.rotation.inverse() * cam.rotation;
    let (right, upv) = (cam_rot * Vec3::X, cam_rot * Vec3::Y);

    // Coulée de lave de l'évent de lave le plus proche
    let lava_vent = vents.iter().find(|(v, _)| v.kind == VentKind::Lava && strength(v, now) > 0.01).map(|(v, _)| *v);
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
                let s = strength(&v, now);
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
        let s = lava_vent.map_or(0.0, |v| strength(&v, now));
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
    let voxel = t.voxel();
    let all: Vec<Vent> = vents_near(&t.params, dir, reach, &|d| t.base_column(d, voxel).kind).into_iter().filter(|v| v.kind == kind).collect();
    let active: Vec<Vent> = all.iter().copied().filter(|v| kind == VentKind::Geyser || strength(v, now) > 0.3).collect();
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
        p.geo = GeoActivity { volcanism: 0.9, quakes: 0.5, cryo: false };
        let lava = count(&p, VentKind::Lava);
        let geysers = count(&p, VentKind::Geyser);
        assert!(lava > 10 && geysers > 10, "{lava} {geysers}");
        assert_eq!(count(&p, VentKind::Cryovolcano), 0);
        let again = count(&p, VentKind::Lava);
        assert_eq!(lava, again);
        // Encelade : froid, sans air, océan sous la glace
        let mut moon = p;
        moon.geo = GeoActivity { volcanism: 0.0, quakes: 0.2, cryo: true };
        moon.temperature = -190.0;
        moon.atmosphere = false;
        moon.airless = true;
        assert!(count(&moon, VentKind::Cryovolcano) > 10);
        assert_eq!(count(&moon, VentKind::Geyser) + count(&moon, VentKind::Fumarole), 0);
    }
}
