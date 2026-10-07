//! Phénomènes du ciel (C4 de `roadmaps/fait/ROADMAP-0.11.md`).
//!
//! - **Orages magnétiques** : chaque étoile a des orages tirés par tranches de 15 min de jeu
//!   (graine + horloge, règle 9), plus fréquents si elle est active : éruptions plus hautes, puis
//!   aurores plus vives deux minutes plus tard.
//! - **Éclipses** : une lune devant un soleil assombrit la lumière (au sol : `SunDim`), son ombre
//!   se voit sur la planète (tache sombre) ; une lune dans l'ombre de sa planète devient sombre et
//!   rougeâtre. `/eclipse` (ou `/eclipse lune`) : aller à la prochaine.
//! - **Phases des lunes** (scanner) et **marées** : la mer monte et descend avec les lunes et le
//!   soleil (`terrain::Tide`, de 0 à 3 voxels), recalculée quand le niveau change sous le joueur.
//! - **Aurores** vues du sol, la nuit : rideaux qui ondulent autour de l'ovale auroral.

use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use std::collections::HashMap;

use crate::kepler::{OrbitalElements, DEFAULT_MU};
use crate::net::Net;
use crate::planet::{MoonId, MoonRoot, PlanetId, PlanetRoot, SpawnedSystems, StarId, StarRoot};
use crate::planetgen::seeds::splitmix64;
use crate::settings::{GameSettings, SPACE_STRETCH};
use crate::surface::{Surface, SurfaceControl};
use crate::terrain::Tide;
use crate::ui::{CameraTarget, TargetKind};
use crate::world_clock::{Spin, WorldClock};
use crate::CameraController;

pub struct SkyPlugin;

impl Plugin for SkyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SunDim>()
            .init_resource::<Storms>()
            .init_resource::<MoonPhases>()
            .init_resource::<SkyFx>()
            .add_event::<EclipseCommand>()
            .add_systems(
                Update,
                (update_storms, update_eclipses, update_tides, update_moon_phases, go_eclipse)
                    .chain()
                    .after(crate::planet::orbit_planets)
                    .after(crate::planet::orbit_moons)
                    .before(SurfaceControl),
            )
            .add_systems(Update, (eclipse_visuals, ground_aurora).after(SurfaceControl));
    }
}

fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

// ─────────────────────────────────────────────────────────────────────────
//  Orages magnétiques
// ─────────────────────────────────────────────────────────────────────────

/// Durée d'une tranche d'orage (secondes de jeu) et retard des aurores sur les éruptions.
const STORM_BIN: f64 = 900.0;
const AURORA_LAG: f64 = 120.0;

/// Force de l'orage (0..1) d'une étoile de graine `seed` et d'activité `activity` (0..1) à
/// l'instant `t` : une tranche sur 6 (étoile calme) à une sur 2 (étoile active) a son orage, qui
/// monte puis retombe.
pub fn storm(seed: u64, activity: f32, t: f64) -> f32 {
    let bin = (t / STORM_BIN).floor();
    let h = splitmix64(seed ^ (bin as i64 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
    let u = (h >> 11) as f64 / (1u64 << 53) as f64;
    let chance = 0.15 + 0.35 * activity.clamp(0.0, 1.0) as f64;
    if u >= chance {
        return 0.0;
    }
    let strength = (0.4 + 0.6 * (u / chance)) as f32;
    // Pic au premier tiers de la tranche
    let x = ((t / STORM_BIN).fract()) as f32;
    let shape = if x < 0.3 { smooth(0.0, 0.3, x) } else { 1.0 - smooth(0.3, 1.0, x) };
    strength * shape
}

/// Orages des étoiles du système chargé et aurores de ses planètes (calculés à chaque image).
#[derive(Resource, Default)]
pub struct Storms {
    /// Étoile (identifiant) -> force de l'orage ; planète -> éclat des aurores (0,4 à 2).
    stars: HashMap<usize, f32>,
    planets: HashMap<usize, f32>,
    /// Graine et activité de chaque étoile (calculées une fois).
    known: HashMap<usize, (u64, f32)>,
}

impl Storms {
    pub fn flare(&self, star: usize) -> f32 {
        self.stars.get(&star).copied().unwrap_or(0.0)
    }

    /// Éclat des aurores d'une planète (1 en temps calme).
    pub fn aurora(&self, planet: usize) -> f32 {
        self.planets.get(&planet).copied().unwrap_or(1.0)
    }
}

fn update_storms(clock: Res<WorldClock>, settings: Res<GameSettings>, spawned: Res<SpawnedSystems>, mut storms: ResMut<Storms>) {
    let t = clock.secs;
    let storms = &mut *storms;
    storms.stars.clear();
    storms.planets.clear();
    for &si in &spawned.0 {
        let Some(sys) = settings.systems.get(si) else { continue };
        for i in 0..sys.stars.len() {
            let id = si * 1000 + i;
            let (seed, act) = *storms.known.entry(id).or_insert_with(|| {
                let act = sys.star_physics_of(i).map_or(0.3, |p| p.activity as f32);
                (splitmix64(sys.body_seed() ^ (i as u64 + 1) * 0x5354_4F52), act)
            });
            storms.stars.insert(id, storm(seed, act, t));
            if i == 0 {
                let aurora = 0.4 + 1.6 * storm(seed, act, t - AURORA_LAG);
                for pi in 0..sys.planets().len() {
                    storms.planets.insert(si * 1000 + pi, aurora);
                }
            }
        }
    }
    if storms.known.len() > 64 {
        storms.known.clear();
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  Éclipses
// ─────────────────────────────────────────────────────────────────────────

/// Part (0..1) du disque d'un soleil (direction `s`, rayon angulaire `a`) cachée par un disque
/// (direction `d`, rayon angulaire `b`).
pub fn occultation(s: Vec3, a: f32, d: Vec3, b: f32) -> f32 {
    if a <= 0.0 || b <= 0.0 {
        return 0.0;
    }
    let theta = s.dot(d).clamp(-1.0, 1.0).acos();
    if theta >= a + b {
        return 0.0;
    }
    if theta <= (a - b).abs() {
        return (b.min(a) / a).powi(2);
    }
    // Lentille d'intersection de deux disques (petits angles : géométrie plane)
    let (r, s2, d) = (a, b, theta);
    let p1 = r * r * ((d * d + r * r - s2 * s2) / (2.0 * d * r)).clamp(-1.0, 1.0).acos();
    let p2 = s2 * s2 * ((d * d + s2 * s2 - r * r) / (2.0 * d * s2)).clamp(-1.0, 1.0).acos();
    let p3 = 0.5 * ((-d + r + s2) * (d + r - s2) * (d - r + s2) * (d + r + s2)).max(0.0).sqrt();
    ((p1 + p2 - p3) / (std::f32::consts::PI * r * r)).clamp(0.0, 1.0)
}

/// Lumière gardée (0..1) de chaque soleil à la caméra : une lune ou une planète devant lui.
#[derive(Resource, Default)]
pub struct SunDim(HashMap<usize, f32>);

impl SunDim {
    pub fn factor(&self, star: usize) -> f32 {
        self.0.get(&star).copied().unwrap_or(1.0)
    }
}

/// Un astre qui peut cacher un soleil : position (monde) et rayon.
#[derive(Clone, Copy)]
struct Disk {
    kind: TargetKind,
    pos: Vec3,
    radius: f32,
}

fn bodies(settings: &GameSettings, planets: &Query<(Entity, &Transform, &PlanetId), (With<PlanetRoot>, Without<MoonRoot>)>, moons: &Query<(Entity, &Transform, &MoonId), With<MoonRoot>>) -> Vec<Disk> {
    let mut out = Vec::new();
    for (_, t, id) in planets {
        if let Some(p) = settings.systems.get(id.0 / 1000).and_then(|s| s.planets().get(id.0 % 1000)) {
            out.push(Disk { kind: TargetKind::Planet(id.0), pos: t.translation, radius: p.radius });
        }
    }
    for (_, t, id) in moons {
        let r = settings.systems.get(id.planet_idx / 1000).and_then(|s| s.planets().get(id.planet_idx % 1000)).and_then(|p| p.moons.get(id.moon_idx)).map_or(0.0, |m| m.radius);
        out.push(Disk { kind: TargetKind::Moon(id.planet_idx, id.moon_idx), pos: t.translation, radius: r });
    }
    out
}

/// Lumière gardée d'un soleil (`star`, rayon `star_r`) vue depuis `eye`, en ignorant l'astre `skip`.
fn sunlight_at(eye: Vec3, star: Vec3, star_r: f32, disks: &[Disk], skip: Option<TargetKind>) -> f32 {
    let to_s = star - eye;
    let ds = to_s.length().max(1.0);
    let (s, a) = (to_s / ds, star_r / ds);
    let mut keep = 1.0;
    for d in disks {
        if Some(d.kind) == skip {
            continue;
        }
        let to_b = d.pos - eye;
        let db = to_b.length();
        if db <= d.radius || db >= ds {
            continue;
        }
        keep *= 1.0 - occultation(s, a, to_b / db, d.radius / db);
    }
    keep
}

#[allow(clippy::too_many_arguments)]
fn update_eclipses(
    settings: Res<GameSettings>,
    surface: Res<Surface>,
    stars: Query<(&Transform, &StarId), With<StarRoot>>,
    planets: Query<(Entity, &Transform, &PlanetId), (With<PlanetRoot>, Without<MoonRoot>)>,
    moons: Query<(Entity, &Transform, &MoonId), With<MoonRoot>>,
    cam_q: Query<&Transform, With<Camera3d>>,
    weather: Res<crate::weather::WeatherNow>,
    mut dim: ResMut<SunDim>,
) {
    dim.0.clear();
    // Au sol ou en vol bas seulement : ailleurs, la lumière des étoiles n'a pas d'ombres
    let (true, Ok(cam)) = (surface.suns_on(), cam_q.get_single()) else { return };
    let disks = bodies(&settings, &planets, &moons);
    for (t, id) in &stars {
        let Some(cfg) = settings.systems.get(id.0 / 1000).and_then(|s| s.stars.get(id.0 % 1000)) else { continue };
        // Éclipse, puis nuages et poussière (C5)
        let keep = sunlight_at(cam.translation, t.translation, cfg.radius, &disks, surface.body()) * weather.light();
        if keep < 0.999 {
            dim.0.insert(id.0, keep);
        }
    }
}

/// Taches d'ombre (éclipses de soleil vues de l'espace) et voiles des lunes éclipsées.
#[derive(Resource, Default)]
struct SkyFx {
    spot_mesh: Option<Handle<Mesh>>,
    spots: Vec<(Entity, Handle<StandardMaterial>)>,
    /// Lune -> voile (entité, matériau).
    veils: HashMap<(usize, usize), (Entity, Handle<StandardMaterial>)>,
    /// Rideaux d'aurore : planète, entités, matériaux.
    curtains: Option<(usize, [Entity; 2], [Handle<StandardMaterial>; 2])>,
}

#[derive(Component)]
struct SkyPart;

/// Disque unité (plan xz), sombre au centre, transparent au bord.
fn spot_mesh() -> Mesh {
    const N: usize = 48;
    let mut pos = vec![[0.0, 0.0, 0.0]];
    let mut col = vec![[1.0, 1.0, 1.0, 1.0]];
    for ring in 1..=4 {
        let r = ring as f32 / 4.0;
        let a = (1.0 - r * r).max(0.0).powf(0.8);
        for k in 0..N {
            let t = k as f32 / N as f32 * std::f32::consts::TAU;
            pos.push([r * t.cos(), 0.0, r * t.sin()]);
            col.push([1.0, 1.0, 1.0, a]);
        }
    }
    let mut idx = Vec::new();
    for k in 0..N as u32 {
        idx.extend_from_slice(&[0, 1 + (k + 1) % N as u32, 1 + k]);
    }
    for ring in 0..3u32 {
        let (a0, b0) = (1 + ring * N as u32, 1 + (ring + 1) * N as u32);
        for k in 0..N as u32 {
            let k1 = (k + 1) % N as u32;
            idx.extend_from_slice(&[a0 + k, a0 + k1, b0 + k, a0 + k1, b0 + k1, b0 + k]);
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

/// Ombre d'une lune (`m`, rayon `rm`) sur sa planète (`p`, rayon `rp`) éclairée par un soleil
/// (`s`, rayon `rs`) : centre sur la planète, rayon de la pénombre, obscurité au centre.
pub fn shadow_spot(s: Vec3, rs: f32, m: Vec3, rm: f32, p: Vec3, rp: f32) -> Option<(Vec3, f32, f32)> {
    let axis = (m - s).normalize_or_zero();
    // Rayon M + λ·axis qui touche la sphère de la planète
    let oc = m - p;
    let b = oc.dot(axis);
    let c = oc.length_squared() - rp * rp;
    let disc = b * b - c;
    if disc < 0.0 || -b - disc.sqrt() < 0.0 {
        return None;
    }
    let lambda = -b - disc.sqrt();
    let hit = m + axis * lambda;
    let alpha = rs / (m - s).length().max(1.0);
    let penumbra = (rm + lambda * alpha).min(rp * 0.9);
    // Ombre totale (la lune cache tout le soleil) ou annulaire
    let dark = ((rm / (lambda * alpha).max(1e-3)).powi(2)).min(1.0);
    Some((hit, penumbra, dark))
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn eclipse_visuals(
    mut commands: Commands,
    settings: Res<GameSettings>,
    surface: Res<Surface>,
    stars: Query<(&Transform, &StarId), With<StarRoot>>,
    planets: Query<(Entity, &Transform, &PlanetId), (With<PlanetRoot>, Without<MoonRoot>)>,
    moons: Query<(Entity, &Transform, &MoonId), With<MoonRoot>>,
    mut parts: Query<(&mut Transform, &mut Visibility), (With<SkyPart>, Without<PlanetRoot>, Without<MoonRoot>, Without<StarRoot>)>,
    mut fx: ResMut<SkyFx>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let fx = &mut *fx;
    let mesh = fx.spot_mesh.get_or_insert_with(|| meshes.add(spot_mesh())).clone();
    let here = surface.body();
    // Le soleil principal de chaque système (le plus gros)
    let star = stars
        .iter()
        .filter_map(|(t, id)| settings.systems.get(id.0 / 1000).and_then(|s| s.stars.get(id.0 % 1000)).map(|c| (t.translation, c.radius)))
        .max_by(|a, b| a.1.total_cmp(&b.1));
    let Some((sp, sr)) = star else { return };

    // ── Taches d'ombre des lunes sur leur planète ──
    let mut used = 0;
    for (_, mt, mid) in &moons {
        let Some((_, pt, _)) = planets.iter().find(|(_, _, p)| p.0 == mid.planet_idx) else { continue };
        let Some(planet) = settings.systems.get(mid.planet_idx / 1000).and_then(|s| s.planets().get(mid.planet_idx % 1000)) else { continue };
        let Some(moon) = planet.moons.get(mid.moon_idx) else { continue };
        if here == Some(TargetKind::Planet(mid.planet_idx)) {
            continue;
        }
        let Some((hit, r, dark)) = shadow_spot(sp, sr, mt.translation, moon.radius, pt.translation, planet.radius) else { continue };
        if used == fx.spots.len() {
            let mat = materials.add(StandardMaterial { base_color: Color::BLACK, unlit: true, alpha_mode: AlphaMode::Blend, ..default() });
            let e = commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(mat.clone()), Transform::IDENTITY, Visibility::Hidden, NotShadowCaster, SkyPart)).id();
            fx.spots.push((e, mat));
            continue;
        }
        let (e, mat) = &fx.spots[used];
        used += 1;
        let n = (hit - pt.translation).normalize_or(Vec3::Y);
        let lift = planet.terrain_height * 1.1 + planet.radius * 0.002;
        if let Ok((mut tf, mut v)) = parts.get_mut(*e) {
            *tf = Transform::from_translation(hit + n * lift).with_rotation(Quat::from_rotation_arc(Vec3::Y, n)).with_scale(Vec3::new(r, 1.0, r));
            *v = Visibility::Inherited;
        }
        if let Some(m) = materials.get_mut(mat) {
            m.base_color = Color::srgba(0.0, 0.0, 0.0, 0.92 * dark);
        }
    }
    for (e, _) in fx.spots.iter().skip(used) {
        if let Ok((_, mut v)) = parts.get_mut(*e) {
            if *v != Visibility::Hidden {
                *v = Visibility::Hidden;
            }
        }
    }

    // ── Lunes dans l'ombre de leur planète : voile sombre et rouge ──
    let mut seen = Vec::new();
    for (me, mt, mid) in &moons {
        let key = (mid.planet_idx, mid.moon_idx);
        seen.push(key);
        let Some((_, pt, _)) = planets.iter().find(|(_, _, p)| p.0 == mid.planet_idx) else { continue };
        let Some(planet) = settings.systems.get(mid.planet_idx / 1000).and_then(|s| s.planets().get(mid.planet_idx % 1000)) else { continue };
        let Some(moon) = planet.moons.get(mid.moon_idx) else { continue };
        let disk = [Disk { kind: TargetKind::Planet(mid.planet_idx), pos: pt.translation, radius: planet.radius }];
        let hidden = 1.0 - sunlight_at(mt.translation, sp, sr, &disk, None);
        let veil = fx.veils.entry(key).or_insert_with(|| {
            let mat = materials.add(StandardMaterial { base_color: Color::NONE, unlit: true, alpha_mode: AlphaMode::Blend, ..default() });
            let r = moon.radius * 1.012 + moon.terrain_height.max(moon.radius * 0.045) * 1.05;
            let e = commands.spawn((Mesh3d(meshes.add(Sphere::new(r).mesh().uv(48, 24))), MeshMaterial3d(mat.clone()), Transform::IDENTITY, Visibility::Hidden, NotShadowCaster, SkyPart)).id();
            commands.entity(me).add_child(e);
            (e, mat)
        });
        let on_it = here == Some(TargetKind::Moon(mid.planet_idx, mid.moon_idx));
        let show = hidden > 0.01 && !on_it;
        if let Ok((_, mut v)) = parts.get_mut(veil.0) {
            let wanted = if show { Visibility::Inherited } else { Visibility::Hidden };
            if *v != wanted {
                *v = wanted;
            }
        }
        if show {
            if let Some(m) = materials.get_mut(&veil.1) {
                // La lumière rougie de l'atmosphère de la planète, comme la Lune éclipsée
                m.base_color = Color::srgba(0.16, 0.03, 0.01, 0.88 * hidden);
            }
        }
    }
    fx.veils.retain(|k, _| seen.contains(k));
}

// ─────────────────────────────────────────────────────────────────────────
//  Marées
// ─────────────────────────────────────────────────────────────────────────

/// Marée maximale (voxels) : visible au bord de l'eau, sans noyer les côtes.
pub const MAX_TIDE_VOXELS: f32 = 3.0;

/// Amplitude (voxels) de la marée qu'un astre de masse `m_raiser` à la distance `d` lève sur un
/// astre de masse `m_body` et de rayon `r_body` (mêmes unités pour `d` et `r_body`). Réglée pour
/// qu'une lune comme la nôtre, à 3,5 rayons (distances du jeu, avant l'étirement), lève un voxel.
pub fn tide_voxels(m_raiser: f32, m_body: f32, r_body: f32, d: f32) -> f32 {
    const K: f32 = 1.0 / (0.0123 / (3.5 * 3.5 * 3.5));
    if m_body <= 0.0 || d <= r_body {
        return 0.0;
    }
    K * (m_raiser / m_body) * (r_body / d).powi(3)
}

/// Marée solaire (voxels) : 0,45 pour la Terre (moitié de la lunaire), d'après la masse de
/// l'étoile (M☉), le rayon (R⊕) et la masse (M⊕) de l'astre et sa distance (UA).
pub fn solar_tide_voxels(m_star: f32, r_body: f32, m_body: f32, au: f32) -> f32 {
    if m_body <= 0.0 || au <= 0.0 {
        return 0.0;
    }
    0.45 * m_star * r_body.powi(3) / (m_body * au.powi(3))
}

#[allow(clippy::too_many_arguments)]
fn update_tides(
    time: Res<Time>,
    settings: Res<GameSettings>,
    mut surface: ResMut<Surface>,
    stars: Query<(&Transform, &StarId), With<StarRoot>>,
    planets: Query<(Entity, &Transform, &PlanetId), (With<PlanetRoot>, Without<MoonRoot>)>,
    moons: Query<(Entity, &Transform, &MoonId), With<MoonRoot>>,
    mut last: Local<f64>,
) {
    // Quatre fois par seconde, au sol ou en vol bas sur un monde à mers
    let now = time.elapsed_secs_f64();
    if now - *last < 0.25 {
        return;
    }
    *last = now;
    let (Some(kind), Some(terrain), Some(p)) = (surface.body(), surface.terrain(), surface.local_point()) else { return };
    if terrain.params.hydro.liquid == crate::planetgen::hydrology::Liquid::None || terrain.params.gaseous {
        return;
    }
    let voxel = terrain.voxel();
    // L'astre, ses tireurs (lunes ou planète, étoiles) : position, rotation, masse
    let mut raisers: Vec<(Vec3, f32)> = Vec::new();
    let (center, rot) = match kind {
        TargetKind::Planet(id) => {
            let Some((_, t, _)) = planets.iter().find(|(_, _, pid)| pid.0 == id) else { return };
            let Some(cfg) = settings.systems.get(id / 1000).and_then(|s| s.planets().get(id % 1000)) else { return };
            for (_, mt, mid) in moons.iter().filter(|(_, _, m)| m.planet_idx == id) {
                if let Some(m) = cfg.moons.get(mid.moon_idx) {
                    let d = mt.translation.distance(t.translation) / SPACE_STRETCH;
                    raisers.push((mt.translation, tide_voxels(m.mass_earth, cfg.mass_earth, cfg.radius, d)));
                }
            }
            add_stars(&settings, &stars, id / 1000, cfg.radius_earth, cfg.mass_earth, cfg.semi_major_au, &mut raisers);
            (t.translation, t.rotation)
        }
        TargetKind::Moon(pid, mi) => {
            let Some((_, t, _)) = moons.iter().find(|(_, _, m)| m.planet_idx == pid && m.moon_idx == mi) else { return };
            let Some(pc) = settings.systems.get(pid / 1000).and_then(|s| s.planets().get(pid % 1000)) else { return };
            let Some(m) = pc.moons.get(mi) else { return };
            if let Some((_, ptf, _)) = planets.iter().find(|(_, _, p)| p.0 == pid) {
                let d = ptf.translation.distance(t.translation) / SPACE_STRETCH;
                raisers.push((ptf.translation, tide_voxels(pc.mass_earth, m.mass_earth, m.radius, d)));
            }
            add_stars(&settings, &stars, pid / 1000, m.radius_earth, m.mass_earth, pc.semi_major_au, &mut raisers);
            (t.translation, t.rotation)
        }
        _ => return,
    };
    // Jamais plus de 3 voxels en tout
    let total: f32 = raisers.iter().map(|r| r.1).sum();
    let k = if total > MAX_TIDE_VOXELS { MAX_TIDE_VOXELS / total } else { 1.0 };
    raisers.sort_by(|a, b| b.1.total_cmp(&a.1));
    let mut tide = Tide::default();
    for (i, (pos, a)) in raisers.iter().take(4).enumerate() {
        tide.dirs[i] = (rot.inverse() * (*pos - center)).normalize_or(Vec3::X);
        tide.amps[i] = a * k * voxel;
    }
    // Nouvelle marée seulement quand le niveau change d'un voxel sous le joueur (reconstruction
    // des tuiles une à une, comme pour les saisons)
    let dir = p.normalize_or(Vec3::Y);
    let level = |t: &Tide| (t.at(dir) / voxel).round() as i32;
    let current = surface.tide().unwrap_or_default();
    if current.is_calm() != tide.is_calm() || level(&current) != level(&tide) {
        surface.set_tide(tide);
    }
}

fn add_stars(settings: &GameSettings, stars: &Query<(&Transform, &StarId), With<StarRoot>>, si: usize, r_earth: f32, m_earth: f32, au: f32, out: &mut Vec<(Vec3, f32)>) {
    let Some(sys) = settings.systems.get(si) else { return };
    for (t, id) in stars.iter().filter(|(_, id)| id.0 / 1000 == si) {
        let mass = sys.star_physics_of(id.0 % 1000).map_or(1.0, |p| p.mass_sun as f32);
        out.push((t.translation, solar_tide_voxels(mass, r_earth, m_earth, au)));
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  Phases des lunes (scanner)
// ─────────────────────────────────────────────────────────────────────────

/// Part éclairée (0..1) d'une lune vue d'un observateur, et le nom de sa phase.
pub fn phase(observer: Vec3, moon: Vec3, star: Vec3) -> (f32, &'static str, bool) {
    let to_sun = (star - moon).normalize_or(Vec3::X);
    let to_obs = (observer - moon).normalize_or(Vec3::X);
    let lit = 0.5 * (1.0 + to_sun.dot(to_obs));
    // Croissante si le soleil est « devant » dans le sens de l'orbite (sinon décroissante) : on
    // regarde de quel côté du plan observateur-lune se trouve le soleil
    let waxing = to_obs.cross(to_sun).y >= 0.0;
    let name = match lit {
        l if l < 0.03 => "nouvelle lune",
        l if l > 0.97 => "pleine lune",
        l if (0.42..0.58).contains(&l) => if waxing { "premier quartier" } else { "dernier quartier" },
        l if l < 0.5 => "croissant",
        _ => "gibbeuse",
    };
    (lit, name, waxing)
}

/// Ligne du scanner : phases des lunes de la planète ciblée, ou de la lune ciblée.
#[derive(Resource, Default)]
pub struct MoonPhases {
    pub target: Option<TargetKind>,
    pub text: String,
}

#[allow(clippy::too_many_arguments)]
fn update_moon_phases(
    time: Res<Time>,
    target: Res<CameraTarget>,
    stars: Query<&Transform, With<StarRoot>>,
    planets: Query<(Entity, &Transform, &PlanetId), (With<PlanetRoot>, Without<MoonRoot>)>,
    moons: Query<(Entity, &Transform, &MoonId), With<MoonRoot>>,
    mut phases: ResMut<MoonPhases>,
    mut last: Local<f64>,
) {
    let now = time.elapsed_secs_f64();
    if now - *last < 0.5 && phases.target == Some(target.0) {
        return;
    }
    *last = now;
    let planet = match target.0 {
        TargetKind::Planet(id) | TargetKind::Moon(id, _) => id,
        _ => {
            phases.target = None;
            phases.text.clear();
            return;
        }
    };
    let Some((_, pt, _)) = planets.iter().find(|(_, _, p)| p.0 == planet) else { return };
    let Some(star) = stars.iter().map(|s| s.translation).min_by(|a, b| a.distance_squared(pt.translation).total_cmp(&b.distance_squared(pt.translation))) else { return };
    let mut list: Vec<(usize, String)> = moons
        .iter()
        .filter(|(_, _, m)| m.planet_idx == planet && !matches!(target.0, TargetKind::Moon(_, mi) if mi != m.moon_idx))
        .map(|(_, mt, m)| {
            let (lit, name, _) = phase(pt.translation, mt.translation, star);
            (m.moon_idx, format!("lune {} : {name} ({:.0} %)", m.moon_idx + 1, lit * 100.0))
        })
        .collect();
    list.sort_by_key(|x| x.0);
    phases.target = Some(target.0);
    phases.text = if list.is_empty() { String::new() } else { format!("Phases (vues de la planete) : {}", list.into_iter().map(|x| x.1).collect::<Vec<_>>().join(", ")) };
}

// ─────────────────────────────────────────────────────────────────────────
//  Aurores vues du sol
// ─────────────────────────────────────────────────────────────────────────

/// Rideau d'aurore autour du pôle (repère de la planète) : bande verticale qui ondule, verte en
/// bas, rouge et pâle en haut.
fn curtain_mesh(radius: f32, lift: f32, lat: f32, north: bool) -> Mesh {
    const N: usize = 360;
    let (bottom, height) = (radius + lift, radius * 0.03);
    let sign = if north { 1.0 } else { -1.0 };
    let (mut pos, mut col, mut idx) = (Vec::new(), Vec::new(), Vec::<u32>::new());
    for k in 0..=N {
        let lon = k as f32 / N as f32 * std::f32::consts::TAU;
        // Plis du rideau : la latitude ondule
        let l = lat + 0.035 * (5.0 * lon).sin() + 0.015 * (13.0 * lon + 1.3).sin();
        let dir = Vec3::new(l.cos() * lon.cos(), sign * l.sin(), l.cos() * lon.sin());
        let bright = 0.6 + 0.4 * (7.0 * lon).sin().abs();
        for (j, (r, c)) in [(bottom, [0.25, 1.0, 0.45]), (bottom + height * 0.45, [0.2, 0.8, 0.4]), (bottom + height, [0.6, 0.15, 0.4])].into_iter().enumerate() {
            pos.push((dir * r).to_array());
            let fade = if j == 2 { 0.0 } else { bright * if j == 0 { 0.8 } else { 1.0 } };
            col.push([c[0] * fade, c[1] * fade, c[2] * fade, 1.0]);
        }
    }
    for k in 0..N as u32 {
        let a = k * 3;
        let b = a + 3;
        idx.extend_from_slice(&[a, b, a + 1, b, b + 1, a + 1, a + 1, b + 1, a + 2, b + 1, b + 2, a + 2]);
    }
    let n = pos.len();
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; n]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
    mesh.insert_indices(Indices::U32(idx));
    mesh
}

#[allow(clippy::too_many_arguments)]
fn ground_aurora(
    mut commands: Commands,
    time: Res<Time>,
    settings: Res<GameSettings>,
    surface: Res<Surface>,
    storms: Res<Storms>,
    planets: Query<(Entity, &Transform, &PlanetId), (With<PlanetRoot>, Without<MoonRoot>)>,
    mut vis: Query<&mut Visibility, With<SkyPart>>,
    mut fx: ResMut<SkyFx>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let fx = &mut *fx;
    let here = match surface.body() {
        Some(TargetKind::Planet(id)) => settings.systems.get(id / 1000).and_then(|s| s.planets().get(id % 1000)).and_then(|p| p.aurora.map(|a| (id, a, p.radius, p.terrain_height))),
        _ => None,
    };
    // Plus de séjour sur cette planète : on retire les rideaux
    if fx.curtains.as_ref().is_some_and(|c| Some(c.0) != here.map(|h| h.0)) {
        if let Some((_, es, _)) = fx.curtains.take() {
            for e in es {
                commands.entity(e).try_despawn_recursive();
            }
        }
    }
    let Some((id, aurora, radius, relief)) = here else { return };
    let Some((root, _, _)) = planets.iter().find(|(_, _, p)| p.0 == id) else { return };
    if fx.curtains.is_none() {
        let lat = aurora.latitude.to_radians();
        let make = |north: bool, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, commands: &mut Commands| {
            let mat = materials.add(StandardMaterial { base_color: Color::BLACK, unlit: true, alpha_mode: AlphaMode::Add, cull_mode: None, double_sided: true, ..default() });
            let e = commands
                .spawn((Mesh3d(meshes.add(curtain_mesh(radius, relief * 1.5 + radius * 0.012, lat, north))), MeshMaterial3d(mat.clone()), Transform::IDENTITY, Visibility::Hidden, NotShadowCaster, SkyPart))
                .id();
            commands.entity(root).add_child(e);
            (e, mat)
        };
        let (n, s) = (make(true, &mut meshes, &mut materials, &mut commands), make(false, &mut meshes, &mut materials, &mut commands));
        fx.curtains = Some((id, [n.0, s.0], [n.1, s.1]));
    }
    let Some((_, es, mats)) = &fx.curtains else { return };
    // La nuit seulement (le soleil sous l'horizon), plus vive pendant un orage
    let night = smooth(0.1, -0.15, surface.sun_height()) * (1.0 - surface.underground());
    let t = time.elapsed_secs();
    let k = night * aurora.strength * storms.aurora(id) * (0.75 + 0.25 * (t * 0.7).sin() * (t * 0.23 + 1.0).cos());
    for (e, mat) in es.iter().zip(mats) {
        if let Ok(mut v) = vis.get_mut(*e) {
            let wanted = if k > 0.01 { Visibility::Inherited } else { Visibility::Hidden };
            if *v != wanted {
                *v = wanted;
            }
        }
        if let Some(m) = materials.get_mut(mat) {
            let c = aurora.color;
            m.base_color = Color::linear_rgb(c[0].max(0.3) * k, c[1].max(0.3) * k, c[2].max(0.3) * k);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  /eclipse : aller à la prochaine
// ─────────────────────────────────────────────────────────────────────────

/// `/eclipse` (de soleil, vue d'une planète) ou `/eclipse lune`.
#[derive(Event)]
pub struct EclipseCommand(pub String);

/// Prochaine éclipse trouvée : instant, planète, lune, et direction (repère fixe de la planète)
/// sous l'ombre, pour une éclipse de soleil.
#[derive(Clone, Copy, Debug)]
pub struct NextEclipse {
    pub t: f64,
    pub planet: usize,
    pub moon: usize,
    pub ground: Vec3,
}

fn planet_elements(p: &crate::settings::PlanetConfig) -> OrbitalElements {
    OrbitalElements { a: p.orbit_distance, e: p.eccentricity, i: p.inclination, omega_big: p.ascending_node, omega: p.arg_periapsis, m0: p.mean_anomaly_0 }
}

fn moon_elements(m: &crate::settings::MoonConfig) -> OrbitalElements {
    OrbitalElements { a: m.orbit_distance, e: m.eccentricity, i: m.inclination, omega_big: m.ascending_node, omega: m.arg_periapsis, m0: m.mean_anomaly_0 }
}

/// Cherche la prochaine éclipse du système `si` après `t0` (jusqu'à `horizon` secondes), comme les
/// orbites de `planet.rs` (même formules, mêmes `mu`).
pub fn next_eclipse(settings: &GameSettings, si: usize, t0: f64, horizon: f64, solar: bool) -> Option<NextEclipse> {
    let sys = settings.systems.get(si)?;
    let star_r = sys.stars.first()?.radius;
    let star_at = |t: f64| sys.stars[0].orbit.map_or(bevy::math::DVec3::ZERO, |o| o.position(t)).as_vec3();
    let planet_mu = DEFAULT_MU * crate::planet::PLANET_MU_SCALE;
    let moon_mu = DEFAULT_MU * 0.001;
    let planets = sys.planets();
    const STEP: f64 = 20.0;
    let mut t = t0;
    while t < t0 + horizon {
        let s = star_at(t);
        for (pi, p) in planets.iter().enumerate().filter(|(_, p)| !p.rogue && !p.moons.is_empty()) {
            let pp = planet_elements(p).position(t, planet_mu);
            for (mi, m) in p.moons.iter().enumerate() {
                let mp = pp + moon_elements(m).position(t, moon_mu);
                let found = if solar {
                    // L'ombre de la lune touche la planète
                    shadow_spot(s, star_r, mp, m.radius, pp, p.radius).map(|(hit, _, _)| hit)
                } else {
                    // La lune dans l'ombre de la planète
                    let disk = [Disk { kind: TargetKind::Planet(0), pos: pp, radius: p.radius }];
                    (sunlight_at(mp, s, star_r, &disk, None) < 0.5).then_some(mp)
                };
                if let Some(at) = found {
                    let rot = Spin::planet(p).rotation(t, -pp);
                    return Some(NextEclipse { t, planet: si * 1000 + pi, moon: mi, ground: rot.inverse() * (at - pp).normalize_or(Vec3::Y) });
                }
            }
        }
        t += STEP;
    }
    None
}

/// Recherche jusqu'à 3 jours de jeu en avant.
const ECLIPSE_HORIZON: f64 = 3.0 * 24.0 * 3600.0;

#[allow(clippy::too_many_arguments)]
fn go_eclipse(
    time: Res<Time>,
    settings: Res<GameSettings>,
    spawned: Res<SpawnedSystems>,
    mut clock: ResMut<WorldClock>,
    mut events: EventReader<EclipseCommand>,
    mut surface: ResMut<Surface>,
    mut target: ResMut<CameraTarget>,
    mut net: ResMut<Net>,
    mut cam_q: Query<&mut CameraController>,
) {
    for EclipseCommand(arg) in events.read() {
        let now = time.elapsed_secs_f64();
        if surface.active() {
            net.notify("Remontez d'abord en orbite (molette ou V).", now);
            continue;
        }
        let Some(&si) = spawned.0.iter().next() else {
            net.notify("Aucun systeme charge : approchez-vous d'une etoile.", now);
            continue;
        };
        let solar = !arg.trim().starts_with("lune");
        let what = if solar { "eclipse de soleil" } else { "eclipse de lune" };
        let Some(e) = next_eclipse(&settings, si, clock.secs + 30.0, ECLIPSE_HORIZON, solar) else {
            net.notify(&format!("Pas d'{what} dans ce systeme pendant les 3 prochains jours."), now);
            continue;
        };
        let wait = e.t - clock.secs;
        // L'hôte (ou en solo) avance l'horloge juste avant ; sinon, un compte à rebours
        let jumped = wait > 90.0 && net.may_set_clock();
        if jumped {
            clock.secs = e.t - 45.0;
        }
        let name = settings.systems.get(si).map_or(String::new(), |s| s.name.clone());
        if solar {
            target.0 = TargetKind::Planet(e.planet);
            surface.set_hover(TargetKind::Planet(e.planet), e.ground);
        } else {
            target.0 = TargetKind::Moon(e.planet, e.moon);
        }
        let radius = settings.systems.get(si).and_then(|s| s.planets().get(e.planet % 1000)).map_or(10_000.0, |p| p.radius);
        if let Ok(mut ctrl) = cam_q.get_single_mut() {
            ctrl.zoom_goal = Some(radius * if solar { 3.0 } else { 6.0 });
        }
        let when = if jumped { "dans une minute (horloge avancee)".to_string() } else { format!("dans {:.0} min", (wait / 60.0).max(0.0)) };
        let how = if solar { "Le vaisseau est au-dessus de l'ombre : V pour atterrir dedans." } else { "La lune va s'assombrir et rougir." };
        net.notify(&format!("{what} : {name} {}, lune {} {when}. {how}", e.planet % 1000 + 1, e.moon + 1), now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn occultation_covers_partial_total_and_annular() {
        let s = Vec3::Z;
        assert_eq!(occultation(s, 0.01, Vec3::X, 0.01), 0.0);
        assert!((occultation(s, 0.01, s, 0.02) - 1.0).abs() < 1e-6, "totale");
        assert!((occultation(s, 0.01, s, 0.005) - 0.25).abs() < 1e-3, "annulaire");
        let half = occultation(s, 0.01, Vec3::new(0.01, 0.0, 1.0).normalize(), 0.01);
        assert!(half > 0.3 && half < 0.5, "{half}");
    }

    #[test]
    fn moon_shadow_falls_on_the_planet_behind_it() {
        let (s, p) = (Vec3::new(-1.0e7, 0.0, 0.0), Vec3::ZERO);
        let (hit, r, dark) = shadow_spot(s, 1.0e5, Vec3::new(-30_000.0, 0.0, 0.0), 1500.0, p, 6000.0).expect("ombre");
        assert!((hit - Vec3::new(-6000.0, 0.0, 0.0)).length() < 1.0);
        assert!(r > 1500.0 && dark > 0.99, "{r} {dark}");
        assert!(shadow_spot(s, 1.0e5, Vec3::new(-30_000.0, 9000.0, 0.0), 1500.0, p, 6000.0).is_none());
    }

    #[test]
    fn storms_come_and_go_more_often_on_active_stars() {
        let count = |act: f32| (0..400).filter(|i| storm(7, act, *i as f64 * STORM_BIN + STORM_BIN * 0.3) > 0.1).count();
        let (calm, active) = (count(0.0), count(1.0));
        assert!(calm > 20 && active > calm * 2, "{calm} {active}");
        for i in 0..2000 {
            let v = storm(3, 0.5, i as f64 * 7.3);
            assert!((0.0..=1.0).contains(&v));
        }
    }

    #[test]
    fn tides_are_about_one_voxel_for_an_earth_like_moon() {
        let moon = tide_voxels(0.0123, 1.0, 6000.0, 3.5 * 6000.0);
        assert!((moon - 1.0).abs() < 0.01);
        assert!((solar_tide_voxels(1.0, 1.0, 1.0, 1.0) - 0.45).abs() < 1e-6);
        // Haute vers la lune et à l'opposé, basse à 90°
        let t = Tide { dirs: [Vec3::X, Vec3::Y, Vec3::Y, Vec3::Y], amps: [10.0, 0.0, 0.0, 0.0] };
        assert_eq!(t.at(Vec3::X), 10.0);
        assert_eq!(t.at(Vec3::NEG_X), 10.0);
        assert_eq!(t.at(Vec3::Z), -5.0);
    }

    #[test]
    fn phases_go_from_new_to_full() {
        let (obs, moon) = (Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0));
        assert_eq!(phase(obs, moon, Vec3::new(1e6, 0.0, 0.0)).1, "nouvelle lune");
        assert_eq!(phase(obs, moon, Vec3::new(-1e6, 0.0, 0.0)).1, "pleine lune");
        let q = phase(obs, moon, Vec3::new(10.0, 0.0, 1e6));
        assert!(q.1.contains("quartier"), "{q:?}");
    }

    #[test]
    fn eclipses_happen_in_the_generated_world() {
        let settings = GameSettings::default();
        let found = (0..200).filter_map(|si| next_eclipse(&settings, si, 0.0, 24.0 * 3600.0, true)).count();
        assert!(found > 20, "{found} systemes avec une eclipse de soleil en un jour");
        let lunar = (0..200).filter_map(|si| next_eclipse(&settings, si, 0.0, 24.0 * 3600.0, false)).count();
        assert!(lunar > 20, "{lunar}");
    }
}
