//! Météores et impacts (0.11, phase B5).
//!
//! - Étoiles filantes : la nuit, sur les mondes avec de l'air. Leur nombre et leur trajet sont
//!   une fonction de (graine de l'astre, horloge du monde) (règle 9) : deux joueurs au même endroit
//!   voient les mêmes. Quelques-unes sont des bolides (plus gros, plus lents, un éclair).
//! - Pluies de météores : trois par année de l'astre (passage dans le sillage d'une comète), le
//!   nombre d'étoiles filantes y est multiplié par vingt.
//! - Impacts : rares, près du joueur (on les voit), ou forcés par `/impact`. Le météore descend,
//!   un éclair, et un cratère est creusé dans les voxels : un delta (règle 7) sauvé dans
//!   `world.json` et envoyé aux autres joueurs (`net::Msg::Voxels`). Sans air : pas de traînée.

use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;

use crate::planet::{MoonId, MoonRoot, PlanetId, PlanetRoot, VoxelType};
use crate::settings::GameSettings;
use crate::surface::{Frame, Surface};
use crate::terrain::Terrain;
use crate::ui::TargetKind;
use crate::voxel::{BodyVoxels, Cell, VoxelEdit};
use crate::world_clock::WorldClock;

/// Étoiles filantes sporadiques : une toutes les ~30 s de jeu (la nuit, avec de l'air).
const SPORADIC_PER_SEC: f64 = 1.0 / 30.0;
/// Au plus fort d'une pluie de météores, en plus.
const SHOWER_PER_SEC: f64 = 0.7;
/// Impacts spontanés près du joueur : un toutes les ~40 min de temps réel en moyenne.
const IMPACT_PER_SEC: f64 = 1.0 / 2400.0;

pub struct MeteorsPlugin;

impl Plugin for MeteorsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Meteors>()
            .add_event::<ImpactCommand>()
            .add_systems(Startup, setup_meteors)
            .add_systems(
                Update,
                (spawn_meteors, run_impact_commands, update_meteors, apply_remote_voxels)
                    .chain()
                    .after(crate::surface::SurfaceControl),
            );
    }
}

/// `/impact` : un météore s'écrase tout près (test).
#[derive(Event)]
pub struct ImpactCommand;

#[derive(Clone, Copy)]
enum Kind {
    Streak,
    Bolide,
    Impact,
}

/// Un météore en vol (repère fixe de l'astre).
struct Flight {
    entity: Entity,
    kind: Kind,
    body: TargetKind,
    from: Vec3,
    to: Vec3,
    start: f64,
    dur: f64,
    width: f32,
    /// Impact : direction du point d'arrivée et rayon du cratère (voxels).
    crater: Option<(Vec3, f32)>,
    trail: bool,
}

#[derive(Resource, Default)]
pub struct Meteors {
    flights: Vec<Flight>,
    last_bin: Option<i64>,
    shower_told: bool,
    last_impact_check: f64,
    mesh: Handle<Mesh>,
    trail: Handle<StandardMaterial>,
    flash: Handle<StandardMaterial>,
    /// Éclairs d'impact : entité, lumière, fin.
    flashes: Vec<(Entity, f64)>,
}

#[derive(Component)]
struct MeteorVisual;

fn setup_meteors(mut meteors: ResMut<Meteors>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    meteors.mesh = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    meteors.trail = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.95, 0.8, 0.8),
        emissive: LinearRgba::new(6.0, 5.2, 3.5, 1.0),
        unlit: true,
        alpha_mode: AlphaMode::Add,
        ..default()
    });
    meteors.flash = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.8, 0.5, 0.9),
        emissive: LinearRgba::new(20.0, 12.0, 5.0, 1.0),
        unlit: true,
        alpha_mode: AlphaMode::Add,
        ..default()
    });
}

fn hash(seed: u32, n: i64, salt: u32) -> u64 {
    let mut x = (seed as u64) ^ ((salt as u64) << 48) ^ (n as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    x ^= x >> 31;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 29;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 32)
}

fn unit(h: u64, k: u32) -> f32 {
    ((h.rotate_left(k * 11) >> 40) as f32) / (1u64 << 24) as f32
}

/// Force d'une pluie de météores (0 à 1) à ce moment de l'année de l'astre : trois pluies par an.
pub fn shower_strength(seed: u32, year_fraction: f64) -> f32 {
    (0..3)
        .map(|i| {
            let center = unit(hash(seed, i, 77), 1) as f64;
            let d = (year_fraction - center + 0.5).rem_euclid(1.0) - 0.5;
            (-(d / 0.012).powi(2)).exp() as f32
        })
        .fold(0.0, f32::max)
}

/// Météores de la tranche de temps `bin` (demi-seconde de l'horloge du monde) sur l'astre de
/// graine `seed` : (étoile filante ?, bolide ?, tirage). Identique sur toutes les machines.
pub fn meteor_in_bin(seed: u32, bin: i64, shower: f32) -> Option<(bool, u64)> {
    let h = hash(seed, bin, 1);
    let rate = (SPORADIC_PER_SEC + SHOWER_PER_SEC * shower as f64) * 0.5;
    ((unit(h, 0) as f64) < rate).then(|| (unit(h, 2) < 0.05, h))
}

/// Cellules d'un cratère d'impact de `radius` voxels centré sur `dir` : un bol creusé et un
/// bourrelet de roche fondue autour (basalte).
pub fn impact_crater(t: &Terrain, dir: Vec3, radius: f32) -> Vec<(Cell, VoxelType)> {
    let v = t.voxel();
    let up = dir.normalize();
    let east = Vec3::Y.cross(up).normalize_or(Vec3::X);
    let north = up.cross(east);
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    let reach = (radius * 1.4).ceil() as i32;
    let ground0 = t.ground(up).top;
    for a in -reach * 2..=reach * 2 {
        for b in -reach * 2..=reach * 2 {
            let (x, y) = (a as f32 * 0.5, b as f32 * 0.5);
            let d = (x * x + y * y).sqrt() / radius;
            if d > 1.4 {
                continue;
            }
            let col_dir = (up * ground0 + (east * x + north * y) * v).normalize();
            let (face, i, j) = t.cell_of(col_dir);
            if !seen.insert((face, i, j)) {
                continue;
            }
            let top = t.ground(col_dir).top;
            let top_k = t.layer(top + 0.05 * v);
            if d < 1.0 {
                // Bol : profondeur ~ un tiers du rayon au centre
                let depth = (radius * 0.35 * (1.0 - d * d)).round() as i32 + 1;
                for k in top_k - depth..top_k {
                    out.push((Cell { face, i, j, k }, VoxelType::Air));
                }
                // Le fond est vitrifié
                out.push((Cell { face, i, j, k: top_k - depth - 1 }, VoxelType::Basalt));
            } else {
                // Bourrelet
                let h = if d < 1.2 { 2 } else { 1 };
                for k in top_k..top_k + h {
                    out.push((Cell { face, i, j, k }, VoxelType::Basalt));
                }
            }
        }
    }
    out
}

/// Repère (centre, orientation) de l'astre `kind`, s'il est chargé.
fn frame_of(
    kind: TargetKind,
    planets: &Query<(&Transform, &PlanetId), (With<PlanetRoot>, Without<MeteorVisual>)>,
    moons: &Query<(&Transform, &MoonId), (With<MoonRoot>, Without<MeteorVisual>)>,
) -> Option<Frame> {
    let tf = match kind {
        TargetKind::Planet(id) => planets.iter().find(|(_, p)| p.0 == id).map(|(t, _)| *t),
        TargetKind::Moon(pid, mi) => moons.iter().find(|(_, m)| m.planet_idx == pid && m.moon_idx == mi).map(|(t, _)| *t),
        _ => None,
    }?;
    Some(Frame { center: tf.translation, rot: tf.rotation })
}

#[allow(clippy::too_many_arguments)]
fn spawn_meteors(
    mut commands: Commands,
    time: Res<Time>,
    clock: Res<WorldClock>,
    settings: Res<GameSettings>,
    surface: Res<Surface>,
    mut meteors: ResMut<Meteors>,
    mut net: ResMut<crate::net::Net>,
) {
    let (Some(kind), Some(t), Some(p)) = (surface.body(), surface.terrain(), surface.local_point()) else {
        meteors.last_bin = None;
        meteors.shower_told = false;
        return;
    };
    if t.params.gaseous {
        return;
    }
    let now = time.elapsed_secs_f64();
    let bin = (clock.secs * 2.0).floor() as i64;
    // Temps très accéléré : on ne rattrape pas les tranches sautées
    let from = meteors.last_bin.map_or(bin, |b| (b + 1).max(bin - 4));
    meteors.last_bin = Some(bin);
    let Some((spin, _, _)) = crate::world_clock::body_spin(&settings, &kind) else { return };
    let seed = t.params.seed;
    let shower = shower_strength(seed, spin.year_fraction(clock.secs));
    let visible = t.params.atmosphere && t.params.pressure >= 0.05 && surface.dark() && surface.underground() < 0.5;
    if visible && shower > 0.5 && !meteors.shower_told {
        meteors.shower_told = true;
        net.notify("Pluie de meteores cette nuit : levez les yeux !", now);
    } else if shower < 0.2 {
        meteors.shower_told = false;
    }
    let v = t.voxel();
    let up = p.normalize_or(Vec3::Y);
    let ground = t.ground(up).top;
    if visible {
        for b in from..=bin {
            let Some((bolide, h)) = meteor_in_bin(seed, b, shower) else { continue };
            // Un trajet dans le ciel : haut au-dessus du joueur, en biais vers l'horizon
            let east = Vec3::Y.cross(up).normalize_or(Vec3::X);
            let north = up.cross(east);
            let a = unit(h, 3) * std::f32::consts::TAU;
            let side = east * a.cos() + north * a.sin();
            let height = (400.0 + 400.0 * unit(h, 4)) * v;
            let offset = (unit(h, 5) - 0.5) * 900.0 * v;
            let cross = up.cross(side);
            let start = up * (ground + height) + side * offset + cross * (unit(h, 6) - 0.5) * 600.0 * v;
            let length = (150.0 + 250.0 * unit(h, 7)) * v * if bolide { 1.8 } else { 1.0 };
            let dir = (side * 0.8 - up * 0.6).normalize();
            let (dur, width) = if bolide { (2.5, 4.0 * v) } else { (0.5 + 0.7 * unit(h, 8) as f64, 1.0 * v) };
            let flight = Flight {
                entity: Entity::PLACEHOLDER,
                kind: if bolide { Kind::Bolide } else { Kind::Streak },
                body: kind,
                from: start,
                to: start + dir * length,
                start: now,
                dur,
                width,
                crater: None,
                trail: true,
            };
            spawn_flight(&mut commands, &mut meteors, flight);
        }
    }
    // Impacts spontanés (temps réel, rares) : tout près, pour qu'on les voie
    if now - meteors.last_impact_check >= 1.0 {
        meteors.last_impact_check = now;
        let h = hash(seed, (now * 10.0) as i64, 9);
        if (unit(h, 0) as f64) < IMPACT_PER_SEC {
            let flight = impact_flight(t, kind, p, now, h);
            spawn_flight(&mut commands, &mut meteors, flight);
        }
    }
}

/// Un météore qui s'écrase à 40 à 120 voxels du joueur.
fn impact_flight(t: &Terrain, body: TargetKind, p: Vec3, now: f64, h: u64) -> Flight {
    let v = t.voxel();
    let up = p.normalize_or(Vec3::Y);
    let east = Vec3::Y.cross(up).normalize_or(Vec3::X);
    let north = up.cross(east);
    let a = unit(h, 3) * std::f32::consts::TAU;
    let side = east * a.cos() + north * a.sin();
    let dist = (40.0 + 80.0 * unit(h, 4)) * v;
    let hit_dir = (up * t.ground(up).top + side * dist).normalize();
    let hit = hit_dir * t.ground(hit_dir).top;
    let come_from = (hit_dir * 1.0 + side * 0.7 + up.cross(side) * (unit(h, 5) - 0.5)).normalize();
    let air = t.params.atmosphere && t.params.pressure >= 0.05;
    Flight {
        entity: Entity::PLACEHOLDER,
        kind: Kind::Impact,
        body,
        from: hit + come_from * 600.0 * v,
        to: hit,
        start: now,
        dur: if air { 2.2 } else { 0.9 },
        width: 3.0 * v,
        crater: Some((hit_dir, 4.0 + 4.0 * unit(h, 6))),
        trail: air,
    }
}

fn spawn_flight(commands: &mut Commands, meteors: &mut Meteors, mut flight: Flight) {
    flight.entity = commands
        .spawn((
            Mesh3d(meteors.mesh.clone()),
            MeshMaterial3d(meteors.trail.clone()),
            Transform::from_scale(Vec3::ZERO),
            NotShadowCaster,
            MeteorVisual,
        ))
        .id();
    meteors.flights.push(flight);
}

#[allow(clippy::too_many_arguments)]
fn run_impact_commands(
    mut commands: Commands,
    time: Res<Time>,
    mut events: EventReader<ImpactCommand>,
    surface: Res<Surface>,
    mut meteors: ResMut<Meteors>,
    mut net: ResMut<crate::net::Net>,
) {
    let now = time.elapsed_secs_f64();
    for _ in events.read() {
        let (Some(kind), Some(t), Some(p)) = (surface.body(), surface.terrain(), surface.local_point()) else {
            net.notify("Posez-vous ou volez bas sur un astre solide d'abord (zoom sous 1000).", now);
            continue;
        };
        if t.params.gaseous {
            net.notify("Pas de sol ici : une geante gazeuse avale les meteores.", now);
            continue;
        }
        let h = hash(t.params.seed, (now * 1000.0) as i64, 31);
        let flight = impact_flight(t, kind, p, now, h);
        spawn_flight(&mut commands, &mut meteors, flight);
        net.notify("Impact en approche !", now);
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn update_meteors(
    mut commands: Commands,
    time: Res<Time>,
    mut settings: ResMut<GameSettings>,
    mut surface: ResMut<Surface>,
    mut meteors: ResMut<Meteors>,
    mut net: ResMut<crate::net::Net>,
    mut changed: ResMut<crate::surface::VoxelsChanged>,
    cam_q: Query<&Transform, (With<Camera3d>, Without<MeteorVisual>, Without<PlanetRoot>, Without<MoonRoot>)>,
    planets: Query<(&Transform, &PlanetId), (With<PlanetRoot>, Without<MeteorVisual>)>,
    moons: Query<(&Transform, &MoonId), (With<MoonRoot>, Without<MeteorVisual>)>,
    mut visuals: Query<&mut Transform, (With<MeteorVisual>, Without<Camera3d>, Without<PlanetRoot>, Without<MoonRoot>)>,
) {
    let now = time.elapsed_secs_f64();
    let cam = cam_q.get_single().map(|c| c.translation).unwrap_or(Vec3::ZERO);
    // Éclairs d'impact qui s'éteignent
    let mut i = 0;
    while i < meteors.flashes.len() {
        if now >= meteors.flashes[i].1 {
            let (e, _) = meteors.flashes.swap_remove(i);
            if let Some(ec) = commands.get_entity(e) { ec.despawn_recursive(); }
        } else {
            i += 1;
        }
    }
    let flights = std::mem::take(&mut meteors.flights);
    for f in flights {
        let u = ((now - f.start) / f.dur) as f32;
        let frame = frame_of(f.body, &planets, &moons);
        let (Some(frame), true) = (frame, surface.body() == Some(f.body)) else {
            if let Some(ec) = commands.get_entity(f.entity) { ec.despawn_recursive(); }
            continue;
        };
        if u >= 1.0 {
            if let Some(ec) = commands.get_entity(f.entity) { ec.despawn_recursive(); }
            if let Some((dir, radius)) = f.crater {
                land_impact(&mut commands, &mut settings, &mut surface, &mut meteors, &mut net, &mut changed, &frame, f.body, dir, radius, now);
            }
            continue;
        }
        // Tête du météore, et sa traînée derrière elle (plus courte sans air)
        let head = f.from.lerp(f.to, u);
        let along = (f.to - f.from).normalize_or(Vec3::Y);
        let trail = if f.trail { (f.to - f.from).length() * 0.25 } else { f.width * 2.0 };
        let fade = match f.kind {
            Kind::Streak => (u * 4.0).min(1.0) * (1.0 - u).powf(0.5),
            Kind::Bolide => (u * 3.0).min(1.0),
            Kind::Impact => 1.0,
        };
        let mid = head - along * trail * 0.5;
        let world_mid = frame.center + frame.rot * mid;
        let world_along = frame.rot * along;
        // Toujours au moins visible : la largeur grandit avec la distance à la caméra
        let width = f.width.max(cam.distance(world_mid) * 0.0015) * fade;
        if let Ok(mut tf) = visuals.get_mut(f.entity) {
            *tf = Transform::from_translation(world_mid).looking_to(world_along, frame.rot * mid.normalize_or(Vec3::Y)).with_scale(Vec3::new(width, width, trail));
        }
        meteors.flights.push(f);
    }
}

/// L'impact : éclair, cratère dans les voxels (delta sauvé et envoyé aux autres joueurs).
#[allow(clippy::too_many_arguments)]
fn land_impact(
    commands: &mut Commands,
    settings: &mut GameSettings,
    surface: &mut Surface,
    meteors: &mut Meteors,
    net: &mut crate::net::Net,
    changed: &mut crate::surface::VoxelsChanged,
    frame: &Frame,
    body: TargetKind,
    dir: Vec3,
    radius: f32,
    now: f64,
) {
    let Some(t) = surface.terrain() else { return };
    let v = t.voxel();
    let hit = dir * t.ground(dir).top;
    // Éclair
    let flash = commands
        .spawn((
            Mesh3d(meteors.mesh.clone()),
            MeshMaterial3d(meteors.flash.clone()),
            Transform::from_translation(frame.center + frame.rot * hit).with_scale(Vec3::splat(radius * v * 2.5)),
            NotShadowCaster,
            MeteorVisual,
        ))
        .with_children(|c| {
            c.spawn((PointLight { intensity: 4.0e9, range: 400.0 * v, color: Color::srgb(1.0, 0.75, 0.45), shadows_enabled: false, ..default() }, Transform::IDENTITY));
        })
        .id();
    meteors.flashes.push((flash, now + 0.6));
    // Cratère : delta voxel (règle 7)
    let cells = impact_crater(t, dir, radius);
    let Some(key) = crate::chat_cmd::target_body(&body, false).map(|id| id.key()) else { return };
    let mut edits: std::collections::BTreeMap<String, Vec<(u16, u8)>> = std::collections::BTreeMap::new();
    let deltas = settings.voxel_deltas.entry(key.clone()).or_insert_with(BodyVoxels::default);
    for (cell, kind) in &cells {
        deltas.set(*cell, *kind);
        let (block, index) = cell.block();
        edits.entry(block.text()).or_default().push((index, crate::voxel::voxel_code(*kind)));
    }
    settings.save_world();
    for (block, cells) in edits {
        net.voxel_outbox.push(VoxelEdit { body: key.clone(), block, cells });
    }
    changed.0 = true;
    net.notify(&format!("Impact de meteorite a {:.0} unites : un cratere de {:.0} voxels.", (hit - surface.local_point().unwrap_or(hit)).length(), radius * 2.0), now);
}

/// Cratères des autres joueurs : leurs deltas reçus par le réseau.
fn apply_remote_voxels(mut settings: ResMut<GameSettings>, mut net: ResMut<crate::net::Net>, mut changed: ResMut<crate::surface::VoxelsChanged>) {
    if net.voxel_inbox.is_empty() {
        return;
    }
    let edits = std::mem::take(&mut net.voxel_inbox);
    let mut any = false;
    for e in &edits {
        any |= e.apply(&mut settings.voxel_deltas);
    }
    if any {
        settings.save_world();
        changed.0 = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Règle 9 : les mêmes étoiles filantes sur deux machines ; une pluie de météores en
    /// multiplie le nombre.
    #[test]
    fn meteors_depend_only_on_the_seed_and_the_clock() {
        let count = |seed: u32, shower: f32| (0..20_000).filter(|&b| meteor_in_bin(seed, b, shower).is_some()).count();
        assert_eq!(count(5, 0.0), count(5, 0.0));
        let calm = count(5, 0.0);
        let storm = count(5, 1.0);
        // ~1 toutes les 30 s (10 000 s de jeu ici)
        assert!((200..500).contains(&calm), "{calm}");
        assert!(storm > calm * 10, "{storm} vs {calm}");
        // Trois pluies par an
        let peaks = (0..1000).filter(|&k| shower_strength(5, k as f64 / 1000.0) > 0.5).count();
        assert!((20..120).contains(&peaks), "{peaks}");
    }

    /// Un impact creuse un cratère (delta) : le sol descend au centre, un bourrelet l'entoure.
    #[test]
    fn an_impact_digs_a_crater_with_a_rim() {
        let settings = GameSettings::default();
        let p = settings.systems.dense().iter().take(200).flat_map(|s| s.planets().iter().filter(|p| !p.gaseous()).cloned().collect::<Vec<_>>()).next().unwrap();
        let params = crate::terrain::BodyParams::planet(&p);
        let t = Terrain::new(params);
        let dir = Vec3::new(0.3, 0.8, -0.5).normalize();
        let before = t.ground(dir).top;
        let cells = impact_crater(&t, dir, 6.0);
        let mut body = BodyVoxels::default();
        for (c, k) in &cells {
            body.set(*c, *k);
        }
        let hit = Terrain::new(params).with_voxels(Some(std::sync::Arc::new(body)));
        let after = hit.ground(dir).top;
        assert!(after < before - t.voxel(), "{after} vs {before}");
        // Le bourrelet : à 1,1 rayon, plus haut qu'avant
        let east = Vec3::Y.cross(dir).normalize();
        let rim = (dir * before + east * 6.6 * t.voxel()).normalize();
        assert!(hit.ground(rim).top > t.ground(rim).top);
        // Le même impact donne les mêmes cellules (deux machines)
        assert_eq!(cells.len(), impact_crater(&t, dir, 6.0).len());
    }
}
