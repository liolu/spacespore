//! Points d'intérêt (0.14 D6) : les lieux rares d'un astre, au scanner (section « Lieux ») et joignables par
//! `/lieu <sorte>` : entrée de grotte, arche, cascade (un lit de rivière qui tombe), source chaude (bassin d'un
//! geyser), geyser, évent de lave, et trois **lieux hachés** très rares par cellule de `LANDMARK_VOXELS` :
//! épave (fictif, un vaisseau écrasé, sur tout astre à air), ruines (fictif, seulement avec le trait « ruines d'une
//! civilisation disparue »), monolithe (fictif, trait « monolithe parfait » : le trait devient réel, règle 20). Les
//! objets sont posés par le décor (`decor::tile_decor` : `DecorKind::Wreck`, `Ruin`, `Monolith`).
//!
//! **Marqueurs des joueurs** : `/marqueur <nom>` pose une balise (pilier lumineux) là où l'on est ; elle part en
//! réseau (`net::Msg::Marker`, relayée par l'hôte à tous) et chacun la voit et la lit au scanner.

use bevy::prelude::*;
use std::collections::HashMap;

use crate::planet::{MoonId, MoonRoot, PlanetId, PlanetRoot, VoxelType};
use crate::planetgen::seeds::splitmix64;
use crate::surface::{Surface, SurfaceControl};
use crate::terrain::{dir_to_face, face_dir, BodyParams, Terrain};
use crate::ui::TargetKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PoiKind {
    Grotte,
    Arche,
    Cascade,
    SourceChaude,
    Geyser,
    Lave,
    Epave,
    Ruines,
    Monolithe,
    Marqueur,
}

impl PoiKind {
    pub const ALL: [PoiKind; 10] = [PoiKind::Grotte, PoiKind::Arche, PoiKind::Cascade, PoiKind::SourceChaude, PoiKind::Geyser, PoiKind::Lave, PoiKind::Epave, PoiKind::Ruines, PoiKind::Monolithe, PoiKind::Marqueur];

    pub fn name(self) -> &'static str {
        match self {
            PoiKind::Grotte => "grotte",
            PoiKind::Arche => "arche",
            PoiKind::Cascade => "cascade",
            PoiKind::SourceChaude => "source chaude",
            PoiKind::Geyser => "geyser",
            PoiKind::Lave => "lave",
            PoiKind::Epave => "epave (fictif)",
            PoiKind::Ruines => "ruines (fictif)",
            PoiKind::Monolithe => "monolithe (fictif)",
            PoiKind::Marqueur => "marqueur",
        }
    }

    /// Mot tapé (`/lieu <mot>`).
    pub fn parse(s: &str) -> Option<PoiKind> {
        let s = s.trim().to_lowercase();
        if s.is_empty() {
            return None;
        }
        Self::ALL.into_iter().find(|k| k.name().replace(' ', "").starts_with(&s.replace(' ', "")) || k.name().starts_with(&s))
    }
}

/// Un lieu : sorte, direction (repère fixe de l'astre), nom affiché.
#[derive(Clone, Debug, PartialEq)]
pub struct Poi {
    pub kind: PoiKind,
    pub dir: Vec3,
    pub name: String,
}

// ─────────────────────────────────────────────────────────────────────────
//  Lieux hachés : épaves, ruines, monolithes
// ─────────────────────────────────────────────────────────────────────────

/// Côté d'une cellule de lieux rares, en voxels.
pub const LANDMARK_VOXELS: f32 = 2500.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Landmark {
    pub kind: PoiKind,
    pub dir: Vec3,
    pub seed: u64,
}

fn unit(h: u64) -> f32 {
    (h >> 40) as f32 / (1u64 << 24) as f32
}

pub fn landmark_cells(p: &BodyParams) -> i64 {
    ((std::f32::consts::FRAC_PI_2 * p.radius) / (LANDMARK_VOXELS * p.layout().voxel)).round().max(1.0) as i64
}

pub fn landmark_cell_of(p: &BodyParams, dir: Vec3) -> (u8, i64, i64) {
    let n = landmark_cells(p);
    let (face, s, t) = dir_to_face(dir);
    let x = (((s + 1.0) * 0.5 * n as f32).floor() as i64).clamp(0, n - 1);
    let y = (((t + 1.0) * 0.5 * n as f32).floor() as i64).clamp(0, n - 1);
    (face, x, y)
}

/// Le lieu rare de la cellule, s'il y en a un : épave (1 cellule sur 25, astre à air), ruines (1 sur 6 avec le
/// trait), monolithe (1 sur 8 avec le trait) ; jamais en mer.
pub fn landmark_of(p: &BodyParams, (face, x, y): (u8, i64, i64), ground: &dyn Fn(Vec3) -> VoxelType) -> Option<Landmark> {
    if p.gaseous || p.asteroid.is_some() {
        return None;
    }
    let n = landmark_cells(p) as f32;
    let mut h = splitmix64(splitmix64((p.seed as u64) ^ ((face as u64) << 56) ^ 0x1A1D) ^ (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F));
    let mut next = || {
        h = splitmix64(h);
        unit(h)
    };
    let roll = next();
    let b = &p.biomes;
    let mut acc = 0.0;
    let mut pick = |chance: f32| {
        acc += chance;
        roll < acc
    };
    let kind = if b.ruins && pick(1.0 / 6.0) {
        PoiKind::Ruines
    } else if b.monolith && pick(1.0 / 8.0) {
        PoiKind::Monolithe
    } else if p.atmosphere && !p.airless && pick(1.0 / 25.0) {
        PoiKind::Epave
    } else {
        return None;
    };
    let s = -1.0 + 2.0 * (x as f32 + 0.2 + 0.6 * next()) / n;
    let t = -1.0 + 2.0 * (y as f32 + 0.2 + 0.6 * next()) / n;
    let dir = face_dir(face, s, t);
    if ground(dir).is_liquid() {
        return None;
    }
    Some(Landmark { kind, dir, seed: splitmix64(h) })
}

/// Lieux rares dans un carré de (2 `reach` + 1)² cellules autour de `dir` (même face).
pub fn landmarks_near(p: &BodyParams, dir: Vec3, reach: i64, ground: &dyn Fn(Vec3) -> VoxelType) -> Vec<Landmark> {
    let n = landmark_cells(p);
    let (face, x, y) = landmark_cell_of(p, dir);
    let mut out = Vec::new();
    for cy in (y - reach).max(0)..=(y + reach).min(n - 1) {
        for cx in (x - reach).max(0)..=(x + reach).min(n - 1) {
            out.extend(landmark_of(p, (face, cx, cy), ground));
        }
    }
    out
}

// ─────────────────────────────────────────────────────────────────────────
//  Marqueurs des joueurs
// ─────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub struct Marker {
    /// Astre (`format!("{:?}", TargetKind)`).
    pub body: String,
    pub dir: Vec3,
    pub name: String,
    pub by: String,
}

/// Les marqueurs connus (les miens et ceux reçus) et leurs balises à l'écran.
#[derive(Resource, Default)]
pub struct Markers {
    pub all: Vec<Marker>,
    beacons: HashMap<usize, Entity>,
    body: Option<TargetKind>,
    assets: Option<(Handle<Mesh>, Handle<StandardMaterial>)>,
}

pub fn body_key(kind: &TargetKind) -> String {
    match *kind {
        TargetKind::Planet(id) => format!("p{id}"),
        TargetKind::Moon(pid, mi) => format!("m{pid}.{mi}"),
        TargetKind::Star(id) => format!("s{id}"),
        TargetKind::Asteroid(ref k) => format!("a{k:?}"),
        _ => "?".into(),
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  Les lieux autour du joueur
// ─────────────────────────────────────────────────────────────────────────

/// Le lieu le plus proche de chaque sorte autour de `here` (direction), jusqu'à `reach_voxels`.
pub fn nearby(t: &Terrain, here: Vec3, markers: &[Marker], body: &str, clock: f64, reach_voxels: f32) -> Vec<Poi> {
    let p = &t.params;
    let v = t.voxel();
    let radius = p.radius.max(1.0);
    let reach = reach_voxels * v / radius;
    let dist = |d: Vec3| (d - here).length();
    let mut out: Vec<Poi> = Vec::new();
    let mut keep = |kind: PoiKind, dir: Vec3, name: String| {
        if dist(dir) > reach {
            return;
        }
        match out.iter_mut().find(|q| q.kind == kind) {
            Some(q) if dist(q.dir) <= dist(dir) => {}
            Some(q) => *q = Poi { kind, dir, name },
            None => out.push(Poi { kind, dir, name }),
        }
    };
    // Grotte : l'entrée la plus proche
    if let Some(caves) = t.caves.as_ref() {
        let at = here * t.surface_r(here);
        if let Some((e, kind)) = caves.nearest_entrance(at, 6, &|d| t.surface_r(d)) {
            keep(PoiKind::Grotte, e.normalize(), format!("grotte {}", kind.name()));
        }
    }
    // Arche
    if let Some(rocks) = t.rocks.as_ref() {
        if let Some((_, at, _)) = rocks.nearest(here, Some(crate::rocks::Feature::Arch), 20, &|d| t.rock_ground(d)) {
            keep(PoiKind::Arche, at.normalize(), "arche naturelle".into());
        }
    }
    // Cascade et source chaude
    if let Some(net) = t.rivers.as_ref() {
        for (dir, drop) in net.waterfalls(here, reach) {
            keep(PoiKind::Cascade, dir, format!("cascade de {:.0} voxels", drop / v));
        }
    }
    if let Some(forms) = t.geo.as_ref() {
        for dir in forms.pools_near(t, here, 1) {
            keep(PoiKind::SourceChaude, dir, "source chaude".into());
        }
    }
    // Geysers et lave
    for vent in crate::geoactive::vents_near(p, here, 2, &|d| t.vent_ground(d)) {
        match vent.kind {
            crate::geoactive::VentKind::Geyser => keep(PoiKind::Geyser, vent.dir, if p.geo.giant_geysers { "geyser geant".into() } else { "geyser".into() }),
            crate::geoactive::VentKind::Lava => {
                let active = crate::geoactive::strength_in(&vent, clock, &p.geo) > 0.3;
                keep(PoiKind::Lave, vent.dir, if active { "event de lave en eruption".into() } else { "event de lave".into() })
            }
            _ => {}
        }
    }
    // Lieux rares
    for lm in landmarks_near(p, here, 1, &|d| t.vent_ground(d)) {
        keep(lm.kind, lm.dir, lm.kind.name().to_string());
    }
    // Marqueurs des joueurs (astre courant)
    for m in markers.iter().filter(|m| m.body == body) {
        keep(PoiKind::Marqueur, m.dir, format!("marqueur \"{}\" de {}", m.name, m.by));
    }
    out.sort_by(|a, b| dist(a.dir).total_cmp(&dist(b.dir)));
    out
}

/// Direction cardinale et distance en texte.
pub fn where_text(here: Vec3, dir: Vec3, radius: f32, voxel: f32) -> String {
    let north = (Vec3::Y - here * here.y).normalize_or(Vec3::Z);
    let east = north.cross(here);
    let d = dir - here;
    let angle = d.dot(east).atan2(d.dot(north)).to_degrees().rem_euclid(360.0);
    let names = ["N", "NE", "E", "SE", "S", "SO", "O", "NO"];
    let side = names[((angle / 45.0).round() as usize) % 8];
    let vox = d.length() * radius / voxel;
    format!("{vox:.0} vox {side}")
}

// ─────────────────────────────────────────────────────────────────────────
//  Systèmes
// ─────────────────────────────────────────────────────────────────────────

pub struct PoiPlugin;

impl Plugin for PoiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NearbyPois>()
            .init_resource::<Markers>()
            .add_event::<PoiCommand>()
            .add_event::<MarkerCommand>()
            .add_systems(Update, (find_pois, go_poi, markers).after(SurfaceControl));
    }
}

/// Les lieux autour du joueur (scanner), recalculés toutes les deux secondes.
#[derive(Resource, Default)]
pub struct NearbyPois {
    pub text: String,
    pub pois: Vec<Poi>,
    last: f64,
}

fn find_pois(time: Res<Time>, clock: Res<crate::world_clock::WorldClock>, surface: Res<Surface>, markers: Res<Markers>, mut nearby_res: ResMut<NearbyPois>) {
    let now = time.elapsed_secs_f64();
    if now - nearby_res.last < 2.0 {
        return;
    }
    nearby_res.last = now;
    let (Some(t), Some(p), Some(body)) = (surface.terrain(), surface.local_point(), surface.body()) else {
        if !nearby_res.text.is_empty() {
            *nearby_res = NearbyPois { last: now, ..default() };
        }
        return;
    };
    let here = p.normalize_or(Vec3::Y);
    let pois = nearby(t, here, &markers.all, &body_key(&body), clock.secs, 2500.0);
    let (radius, v) = (t.params.radius, t.voxel());
    nearby_res.text = if pois.is_empty() {
        "Lieux : rien de remarquable a 2 500 voxels.".into()
    } else {
        let items: Vec<String> = pois.iter().take(6).map(|q| format!("{} ({})", q.name, where_text(here, q.dir, radius, v))).collect();
        format!("Lieux : {}  /lieu <sorte> : y aller", items.join(" ; "))
    };
    nearby_res.pois = pois;
}

/// `/lieu <sorte>` : aller au lieu le plus proche de cette sorte.
#[derive(Event)]
pub struct PoiCommand(pub String);

fn go_poi(time: Res<Time>, mut events: EventReader<PoiCommand>, nearby_res: Res<NearbyPois>, mut surface: ResMut<Surface>, mut net: ResMut<crate::net::Net>) {
    let now = time.elapsed_secs_f64();
    for PoiCommand(arg) in events.read() {
        let Some(kind) = PoiKind::parse(arg) else {
            let names: Vec<&str> = PoiKind::ALL.iter().map(|k| k.name()).collect();
            net.notify(&format!("/lieu <sorte> : {}", names.join(", ")), now);
            continue;
        };
        let Some(q) = nearby_res.pois.iter().find(|q| q.kind == kind).cloned() else {
            net.notify(&format!("Aucun lieu \"{}\" a moins de 2 500 voxels (voir le scanner : Lieux).", kind.name()), now);
            continue;
        };
        let Some(t) = surface.terrain() else { continue };
        let (v, radius) = (t.voxel(), t.params.radius);
        let east = Vec3::Y.cross(q.dir).normalize_or(Vec3::X);
        // Un peu à côté, le regard vers le lieu
        let side = (q.dir + east * (10.0 * v / radius)).normalize();
        let top = t.sea_surface(q.dir).map_or(t.ground(q.dir).top, |(r, _)| r);
        if surface.teleport(side, -east, side * (top + 25.0 * v)) {
            net.notify(&format!("Lieu : {}.", q.name), now);
        } else {
            net.notify("Attendez la fin de l'atterrissage.", now);
        }
    }
}

/// `/marqueur <nom>` : poser une balise ici, pour tous.
#[derive(Event)]
pub struct MarkerCommand(pub String);

#[derive(Component)]
struct Beacon(usize);

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn markers(
    mut commands: Commands,
    time: Res<Time>,
    settings: Res<crate::settings::GameSettings>,
    surface: Res<Surface>,
    mut events: EventReader<MarkerCommand>,
    mut store: ResMut<Markers>,
    mut net: ResMut<crate::net::Net>,
    planets: Query<(Entity, &PlanetId), With<PlanetRoot>>,
    moons: Query<(Entity, &MoonId), With<MoonRoot>>,
    beacons: Query<Entity, With<Beacon>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let now = time.elapsed_secs_f64();
    let body = surface.body();
    // Les miens
    for MarkerCommand(name) in events.read() {
        let name = name.trim().chars().take(24).collect::<String>();
        let (Some(p), Some(kind)) = (surface.local_point(), body) else {
            net.notify("Posez-vous ou volez bas pour poser un marqueur.", now);
            continue;
        };
        if name.is_empty() {
            net.notify("/marqueur <nom> : pose une balise ici, visible par tous les joueurs.", now);
            continue;
        }
        let dir = p.normalize_or(Vec3::Y);
        let key = body_key(&kind);
        store.all.push(Marker { body: key.clone(), dir, name: name.clone(), by: settings.player_name.clone() });
        net.marker_out.push((key, dir.to_array(), name.clone()));
        net.notify(&format!("Marqueur \"{name}\" pose (les autres joueurs le voient)."), now);
    }
    // Ceux des autres
    for (key, dir, name, from) in std::mem::take(&mut net.markers_in) {
        let m = Marker { body: key, dir: Vec3::from_array(dir).normalize_or(Vec3::Y), name, by: from };
        if !store.all.contains(&m) {
            net.notify(&format!("{} a pose un marqueur \"{}\".", m.by, m.name), now);
            store.all.push(m);
        }
    }
    let extra = store.all.len().saturating_sub(200);
    if extra > 0 {
        store.all.drain(0..extra);
    }
    // Balises de l'astre où l'on est : un pilier lumineux
    if store.body != body {
        store.body = body;
        for e in &beacons {
            if let Some(ec) = commands.get_entity(e) {
                ec.despawn_recursive();
            }
        }
        store.beacons.clear();
    }
    let (Some(kind), Some(t)) = (body, surface.terrain()) else { return };
    let root = match kind {
        TargetKind::Planet(id) => planets.iter().find(|(_, p)| p.0 == id).map(|(e, _)| e),
        TargetKind::Moon(pid, mi) => moons.iter().find(|(_, m)| m.planet_idx == pid && m.moon_idx == mi).map(|(e, _)| e),
        _ => None,
    };
    let Some(root) = root else { return };
    let key = body_key(&kind);
    let v = t.voxel();
    let (mesh, mat) = store
        .assets
        .get_or_insert_with(|| {
            (meshes.add(Cuboid::new(1.0, 60.0, 1.0)), materials.add(StandardMaterial { base_color: Color::srgb(1.0, 0.85, 0.3), emissive: LinearRgba::new(4.0, 3.0, 0.8, 1.0), unlit: true, alpha_mode: AlphaMode::Blend, ..default() }))
        })
        .clone();
    let wanted: Vec<(usize, Vec3)> = store.all.iter().enumerate().filter(|(_, m)| m.body == key).map(|(i, m)| (i, m.dir)).collect();
    for (i, dir) in wanted {
        if store.beacons.contains_key(&i) {
            continue;
        }
        let top = t.ground(dir).top;
        let e = commands
            .spawn((Mesh3d(mesh.clone()), MeshMaterial3d(mat.clone()), Transform { translation: dir * (top + 30.0 * v), rotation: Quat::from_rotation_arc(Vec3::Y, dir), scale: Vec3::splat(v) }, Beacon(i)))
            .id();
        commands.entity(root).add_child(e);
        store.beacons.insert(i, e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Les lieux rares : hachés (mêmes pour la même graine), ruines et monolithes seulement avec leur trait, épaves
    /// seulement avec de l'air, jamais en mer.
    #[test]
    fn landmarks_follow_the_traits() {
        let mut p = crate::terrain::tests::earth_like();
        let count = |p: &BodyParams, kind: PoiKind| -> usize {
            let n = landmark_cells(p);
            (0..n).flat_map(|x| (0..n).map(move |y| (x, y))).filter(|&(x, y)| landmark_of(p, (3, x, y), &|_| VoxelType::Stone).is_some_and(|l| l.kind == kind)).count()
        };
        let wrecks = count(&p, PoiKind::Epave);
        assert!(wrecks > 0, "des épaves sur un monde à air");
        assert_eq!(count(&p, PoiKind::Ruines) + count(&p, PoiKind::Monolithe), 0);
        assert_eq!(wrecks, count(&p, PoiKind::Epave));
        p.biomes.ruins = true;
        p.biomes.monolith = true;
        assert!(count(&p, PoiKind::Ruines) > count(&p, PoiKind::Epave));
        assert!(count(&p, PoiKind::Monolithe) > 0);
        // En mer : rien
        let n = landmark_cells(&p);
        assert!((0..n).all(|x| (0..n).all(|y| landmark_of(&p, (3, x, y), &|_| VoxelType::Water).is_none())));
        let mut airless = p;
        airless.atmosphere = false;
        airless.airless = true;
        airless.biomes.ruins = false;
        airless.biomes.monolith = false;
        assert_eq!(count(&airless, PoiKind::Epave), 0);
    }

    /// `/lieu` comprend les sortes, et le texte de direction est lisible.
    #[test]
    fn kinds_parse_and_directions_read() {
        assert_eq!(PoiKind::parse("cascade"), Some(PoiKind::Cascade));
        assert_eq!(PoiKind::parse("source"), Some(PoiKind::SourceChaude));
        assert_eq!(PoiKind::parse("mono"), Some(PoiKind::Monolithe));
        assert_eq!(PoiKind::parse(""), None);
        let here = Vec3::Y;
        let there = (Vec3::Y + Vec3::Z * 0.001).normalize();
        let t = where_text(here, there, 9000.0, 0.5);
        assert!(t.ends_with(" N"), "{t}");
        assert!(t.starts_with("18 vox"), "{t}");
    }

    /// Un monde à ruines a des lieux autour d'une ruine : la ruine elle-même en tête.
    #[test]
    fn nearby_finds_the_landmark_next_to_it() {
        let mut p = crate::terrain::tests::earth_like();
        p.biomes.ruins = true;
        let t = Terrain::new(p);
        let n = landmark_cells(&p);
        let lm = (0..n).flat_map(|x| (0..n).map(move |y| (x, y))).find_map(|(x, y)| landmark_of(&p, (3, x, y), &|d| t.vent_ground(d))).expect("un lieu");
        let pois = nearby(&t, lm.dir, &[], "x", 0.0, 2500.0);
        assert!(pois.iter().any(|q| q.kind == lm.kind && (q.dir - lm.dir).length() < 1e-5), "{pois:?}");
        let m = Marker { body: "x".into(), dir: lm.dir, name: "ici".into(), by: "moi".into() };
        let with = nearby(&t, lm.dir, &[m], "x", 0.0, 2500.0);
        assert!(with.iter().any(|q| q.kind == PoiKind::Marqueur));
    }
}
