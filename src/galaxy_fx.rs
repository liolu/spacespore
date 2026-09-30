// ─────────────────────────────────────────────────────────────────────────
//  Décor des galaxies
//
//  - Liaisons : de fins traits bleutés entre étoiles voisines (3 par étoile
//    au plus), visibles aux zooms Système, Secteur et Galaxie autour de la
//    caméra. Touche L pour les masquer / les afficher.
//  - Nuages : grands nuages de gaz colorés et diffus le long des bras de
//    chaque galaxie. Ils s'estompent quand on s'en approche, pour ne pas
//    voiler les systèmes, et de très loin comme le reste de la galaxie.
// ─────────────────────────────────────────────────────────────────────────

use bevy::asset::RenderAssetUsages;
use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use std::collections::HashMap;

use crate::galaxy_shape::Rng;
use crate::guild::Guilds;
use crate::net::Net;
use crate::net_ui::NetPanel;
use crate::settings::{pseudo_rand, GameSettings, SystemSpatialIndex, CORE_EXCLUSION};
use crate::ui::MenuState;
use crate::{CameraController, ZoomLevel};

// ── Territoires ─────────────────────────────────────────────────────────
//
// Les liaisons ne sont pas un décor : un trait relie deux étoiles voisines
// d'un même propriétaire, dans sa couleur. Les territoires des joueurs et des
// guildes (étoiles revendiquées, voir `claims.rs`) et ceux des factions PNJ
// se lisent donc d'un coup d'œil. Une étoile sans propriétaire n'a pas de trait.

/// Portée de voisinage : sert seulement à faire grandir un territoire PNJ d'étoile en étoile
/// (les traits, eux, n'ont aucune limite de distance).
const LINK_RANGE_MAIN: f32 = 60_000.0;
const LINK_RANGE_OTHER: f32 = 120_000.0;
/// Factions PNJ de la galaxie principale ; les autres en ont selon leur taille.
const NPC_MAIN: usize = 14;
const NPC_MIN_STARS: usize = 10;
const NPC_MAX_STARS: usize = 25;

pub struct GalaxyFxPlugin;

impl Plugin for GalaxyFxPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ShowLinks(true))
            .init_resource::<NpcTerritories>()
            .add_systems(Startup, (spawn_clouds, build_npc_territories))
            .add_systems(Update, (toggle_links, draw_links, update_clouds));
    }
}

#[derive(Resource)]
struct ShowLinks(bool);

fn toggle_links(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    panel: Res<NetPanel>,
    menu: Res<MenuState>,
    mut show: ResMut<ShowLinks>,
    mut net: ResMut<Net>,
) {
    if !keys.just_pressed(KeyCode::KeyL) || panel.focus.is_some() || menu.open {
        return;
    }
    show.0 = !show.0;
    let text = if show.0 { "Territoires traces entre les etoiles (L)." } else { "Territoires masques (L)." };
    net.notify(text, time.elapsed_secs_f64());
}

fn link_range(settings: &GameSettings, sys: usize) -> f32 {
    if settings.systems[sys].galaxy_id == 0 { LINK_RANGE_MAIN } else { LINK_RANGE_OTHER }
}

/// Traits d'un groupe d'étoiles d'un même propriétaire, sans limite de distance :
/// un réseau minimal (arbre couvrant), donc un seul trait entre deux étoiles reliées
/// et jamais de triangle. Chaque étoile est rattachée à l'étoile déjà reliée la plus
/// proche, ce qui donne le moins de traits possible (une de moins que d'étoiles).
fn group_links(settings: &GameSettings, stars: &[usize]) -> Vec<(Vec3, Vec3)> {
    let pts: Vec<Vec3> = stars
        .iter()
        .filter_map(|&s| settings.systems.get(s).map(|sys| sys.center()))
        .collect();
    let n = pts.len();
    if n < 2 {
        return Vec::new();
    }
    // Algorithme de Prim : on part de la première étoile et on relie à chaque pas la
    // plus proche des étoiles restantes à une étoile déjà dans le réseau
    let mut inside = vec![false; n];
    inside[0] = true;
    // Pour chaque étoile hors réseau : (étoile du réseau la plus proche, distance)
    let mut nearest: Vec<(usize, f32)> = (0..n).map(|i| (0, pts[i].distance(pts[0]))).collect();
    let mut lines = Vec::with_capacity(n - 1);
    for _ in 1..n {
        let Some(next) = (0..n).filter(|&i| !inside[i]).min_by(|&a, &b| nearest[a].1.total_cmp(&nearest[b].1)) else {
            break;
        };
        lines.push((pts[nearest[next].0], pts[next]));
        inside[next] = true;
        for i in (0..n).filter(|&i| !inside[i]) {
            let d = pts[i].distance(pts[next]);
            if d < nearest[i].1 {
                nearest[i] = (next, d);
            }
        }
    }
    lines
}

// ── Factions PNJ ────────────────────────────────────────────────────────

#[allow(dead_code)] // `galaxy` et `stars` : informations publiques de la faction (utilisées par les tests)
pub struct NpcFaction {
    pub name: String,
    pub color: Color,
    pub galaxy: u32,
    pub stars: Vec<usize>,
    /// Traits du territoire, calculés une fois pour toutes.
    links: Vec<(Vec3, Vec3)>,
    /// Contour de ses cercles autour des étoiles (fusionnés), calculé une fois pour toutes.
    outline: Vec<(Vec3, Vec3)>,
    center: Vec3,
    /// Distance du centre à l'étoile la plus éloignée (pour ne dessiner que ce qui est proche).
    extent: f32,
}

/// Territoires des factions PNJ : les mêmes pour tous les joueurs (graine du monde).
#[derive(Resource, Default)]
pub struct NpcTerritories {
    pub factions: Vec<NpcFaction>,
    owner: HashMap<usize, usize>,
}

impl NpcTerritories {
    pub fn faction_of(&self, sys: usize) -> Option<&NpcFaction> {
        self.owner.get(&sys).map(|&i| &self.factions[i])
    }
}

fn mix(a: u32, b: u32, c: u32) -> u32 {
    let mut x = a.wrapping_mul(0x9E37_79B1) ^ b.wrapping_mul(0x85EB_CA6B) ^ c.wrapping_mul(0xC2B2_AE35);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    x = x.wrapping_mul(0x297A_2D39);
    x ^= x >> 15;
    x
}

fn faction_name(seed: u32) -> String {
    const KINDS: [&str; 8] = ["Empire", "Ligue", "Clan", "Union", "Consortium", "Royaume", "Alliance", "Flotte"];
    const A: [&str; 10] = ["Kal", "Vor", "Zen", "Thar", "Mir", "Ox", "Ryn", "Sol", "Dra", "Nex"];
    const B: [&str; 8] = ["ion", "ar", "eth", "os", "una", "ix", "ael", "or"];
    format!(
        "{} {}{}",
        KINDS[mix(seed, 1, 0) as usize % KINDS.len()],
        A[mix(seed, 2, 0) as usize % A.len()],
        B[mix(seed, 3, 0) as usize % B.len()]
    )
}

/// Étoiles voisines de `sys` (dans sa galaxie, à portée de liaison).
fn near_stars(settings: &GameSettings, spatial: &SystemSpatialIndex, sys: usize) -> Vec<usize> {
    let a = &settings.systems[sys];
    let range = link_range(settings, sys);
    spatial
        .systems_in_radius(a.center(), range)
        .into_iter()
        .filter(|&i| i != sys)
        .filter(|&i| {
            settings.systems.get(i).is_some_and(|b| b.galaxy_id == a.galaxy_id && b.center().distance(a.center()) <= range)
        })
        .collect()
}

/// Tous les territoires PNJ, déterministes d'après la graine du monde.
pub fn generate_npcs(settings: &GameSettings, spatial: &SystemSpatialIndex) -> NpcTerritories {
    let seed = (settings.world_seed as u32) ^ ((settings.world_seed >> 32) as u32) ^ 0x4E_50_43;
    let mut out = NpcTerritories::default();
    let mut counter = 0u32;
    for (gid, gal) in settings.galaxies.iter().enumerate() {
        let candidates: Vec<usize> = settings
            .systems
            .iter()
            .enumerate()
            .filter(|(_, s)| s.galaxy_id as usize == gid && s.center().distance(gal.center) >= CORE_EXCLUSION)
            .map(|(i, _)| i)
            .collect();
        if candidates.is_empty() {
            continue;
        }
        let wanted = if gid == 0 { NPC_MAIN } else { 1 + (gal.radius / 1_300_000.0) as usize };
        let mut made = 0;
        for attempt in 0..(wanted * 12) as u32 {
            if made >= wanted {
                break;
            }
            let start = candidates[mix(seed, gid as u32, attempt) as usize % candidates.len()];
            if out.owner.contains_key(&start) {
                continue;
            }
            let target = NPC_MIN_STARS + mix(seed ^ 0x51, gid as u32, attempt) as usize % (NPC_MAX_STARS - NPC_MIN_STARS + 1);
            // Le territoire grandit d'étoile voisine en étoile voisine, en restant compact
            let origin = settings.systems[start].center();
            let mut members = vec![start];
            while members.len() < target {
                let next = members
                    .iter()
                    .flat_map(|&m| near_stars(settings, spatial, m))
                    .filter(|s| !members.contains(s) && !out.owner.contains_key(s))
                    .filter(|&s| settings.systems[s].center().distance(gal.center) >= CORE_EXCLUSION)
                    .min_by(|&x, &y| {
                        let dx = settings.systems[x].center().distance(origin);
                        let dy = settings.systems[y].center().distance(origin);
                        dx.total_cmp(&dy)
                    });
                match next {
                    Some(s) => members.push(s),
                    None => break,
                }
            }
            // Trop petit (coincé contre d'autres territoires) : on essaie ailleurs
            if members.len() < NPC_MIN_STARS / 2 {
                continue;
            }
            let idx = out.factions.len();
            let hue = ((counter as f32 * 0.618_034) % 1.0) * 360.0;
            let center = members.iter().map(|&m| settings.systems[m].center()).sum::<Vec3>() / members.len() as f32;
            let extent = members.iter().map(|&m| settings.systems[m].center().distance(center)).fold(0.0, f32::max);
            for &m in &members {
                out.owner.insert(m, idx);
            }
            out.factions.push(NpcFaction {
                name: faction_name(mix(seed, counter, 99)),
                color: Color::hsl(hue, 0.85, 0.58),
                galaxy: gid as u32,
                links: group_links(settings, &members),
                outline: {
                    let borders: Vec<crate::claims::Border> = members.iter().filter_map(|&m| crate::claims::border_of(settings, m)).collect();
                    crate::claims::outline_segments(&borders, crate::claims::CLAIM_RADIUS)
                },
                stars: members,
                center,
                extent,
            });
            counter += 1;
            made += 1;
        }
    }
    out
}

fn build_npc_territories(settings: Res<GameSettings>, mut npcs: ResMut<NpcTerritories>) {
    let spatial = SystemSpatialIndex::build(&settings);
    *npcs = generate_npcs(&settings, &spatial);
}

fn draw_links(
    show: Res<ShowLinks>,
    zoom: Res<ZoomLevel>,
    settings: Res<GameSettings>,
    npcs: Res<NpcTerritories>,
    net: Res<Net>,
    guilds: Res<Guilds>,
    cam_q: Query<(&GlobalTransform, &CameraController)>,
    mut gizmos: Gizmos,
) {
    // Vue planète : trop près pour lire un territoire
    if !show.0 || *zoom == ZoomLevel::Planet {
        return;
    }
    let Ok((cam, ctrl)) = cam_q.get_single() else { return };
    let cam_pos = cam.translation();
    let window = (ctrl.distance * 1.5).clamp(60_000.0, 6_000_000.0);

    // Factions PNJ : seulement celles qui sont près de la caméra (il y en a des milliers dans l'univers)
    for faction in &npcs.factions {
        if cam_pos.distance(faction.center) > window + faction.extent {
            continue;
        }
        let color = faction.color.with_alpha(0.85);
        for &(a, b) in faction.outline.iter().chain(&faction.links) {
            gizmos.line(a, b, color);
        }
    }

    // Joueurs et guildes : étoiles revendiquées
    for (color, stars) in crate::claims::owner_groups(&net, &settings, &guilds) {
        let color = color.with_alpha(0.95);
        for (a, b) in group_links(&settings, &stars) {
            gizmos.line(a, b, color);
        }
    }
}

// ── Nuages ──────────────────────────────────────────────────────────────

const CLOUD_COLORS: usize = 5;
const CLOUD_PALETTE: [[f32; 3]; CLOUD_COLORS] = [
    [0.95, 0.30, 0.75], // rose
    [0.30, 0.50, 1.00], // bleu
    [1.00, 0.55, 0.25], // orange
    [0.25, 0.90, 0.80], // turquoise
    [0.65, 0.40, 1.00], // violet
];
/// Niveaux d'opacité (fondu) : un matériau par couleur et par niveau.
const CLOUD_STEPS: usize = 8;
const CLOUD_MAX_ALPHA: f32 = 0.22;
const CLOUD_TEXTURE: u32 = 128;
/// Nuages de la galaxie principale ; les autres en ont selon leur taille.
const CLOUDS_MAIN: usize = 220;
/// Au-delà de cette distance à sa galaxie, un nuage n'est plus dessiné.
const CLOUD_FADE_START: f32 = 100_000_000.0;
const CLOUD_FADE_END: f32 = 120_000_000.0;

#[derive(Resource)]
struct CloudMaterials {
    steps: Vec<Vec<Handle<StandardMaterial>>>,
}

#[derive(Component)]
struct GalaxyCloud {
    galaxy_id: u32,
    size: f32,
    color: usize,
    /// Étirement et rotation propres à chaque nuage (pour qu'ils ne se ressemblent pas).
    stretch: f32,
    roll: f32,
}

fn hash_noise(x: i32, y: i32, seed: u32) -> f32 {
    let h = (x as u32).wrapping_mul(374_761_393) ^ (y as u32).wrapping_mul(668_265_263) ^ seed.wrapping_mul(2_246_822_519);
    pseudo_rand(h)
}

/// Bruit de valeur lissé à l'échelle `cells` cases sur toute la texture.
fn value_noise(u: f32, v: f32, cells: f32, seed: u32) -> f32 {
    let (x, y) = (u * cells, v * cells);
    let (xi, yi) = (x.floor() as i32, y.floor() as i32);
    let (fx, fy) = (x - xi as f32, y - yi as f32);
    let s = |t: f32| t * t * (3.0 - 2.0 * t);
    let (sx, sy) = (s(fx), s(fy));
    let a = hash_noise(xi, yi, seed);
    let b = hash_noise(xi + 1, yi, seed);
    let c = hash_noise(xi, yi + 1, seed);
    let d = hash_noise(xi + 1, yi + 1, seed);
    (a + (b - a) * sx) + ((c + (d - c) * sx) - (a + (b - a) * sx)) * sy
}

/// Texture d'un nuage : blanc, opacité en volutes qui s'éteignent sur les bords.
fn cloud_pixels() -> Vec<u8> {
    let n = CLOUD_TEXTURE;
    let mut data = vec![255u8; (n * n * 4) as usize];
    for y in 0..n {
        for x in 0..n {
            let (u, v) = (x as f32 / n as f32, y as f32 / n as f32);
            let (dx, dy) = (u - 0.5, v - 0.5);
            let d = (dx * dx + dy * dy).sqrt() * 2.0;
            let falloff = { let t = ((1.0 - d) / 0.6).clamp(0.0, 1.0); t * t * (3.0 - 2.0 * t) };
            let noise = 0.55 * value_noise(u, v, 4.0, 1) + 0.30 * value_noise(u, v, 9.0, 2) + 0.15 * value_noise(u, v, 20.0, 3);
            // Volutes : le bruit creuse le nuage plutôt que d'en faire un simple disque
            let density = ((noise - 0.22) * 2.6).clamp(0.0, 1.0) * falloff;
            data[((y * n + x) * 4 + 3) as usize] = (density * 255.0) as u8;
        }
    }
    data
}

fn cloud_count(galaxy_id: usize, radius: f32) -> usize {
    if galaxy_id == 0 {
        CLOUDS_MAIN
    } else {
        (20.0 + radius / 100_000.0) as usize
    }
}

fn spawn_clouds(
    mut commands: Commands,
    settings: Res<GameSettings>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let texture = images.add(Image::new(
        Extent3d { width: CLOUD_TEXTURE, height: CLOUD_TEXTURE, depth_or_array_layers: 1 },
        TextureDimension::D2,
        cloud_pixels(),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    ));
    let mut steps: Vec<Vec<Handle<StandardMaterial>>> = Vec::with_capacity(CLOUD_COLORS);
    for c in CLOUD_PALETTE {
        let mut per_step = Vec::with_capacity(CLOUD_STEPS);
        for step in 0..CLOUD_STEPS {
            let a = CLOUD_MAX_ALPHA * step as f32 / (CLOUD_STEPS - 1) as f32;
            per_step.push(materials.add(StandardMaterial {
                base_color: Color::srgba(c[0], c[1], c[2], a),
                base_color_texture: Some(texture.clone()),
                unlit: true,
                alpha_mode: AlphaMode::Add,
                cull_mode: None,
                ..default()
            }));
        }
        steps.push(per_step);
    }
    let mesh = meshes.add(Rectangle::new(2.0, 2.0));
    let start_material = steps[0][CLOUD_STEPS - 1].clone();
    commands.insert_resource(CloudMaterials { steps });

    let tau = std::f32::consts::TAU;
    for (gid, gal) in settings.galaxies.iter().enumerate() {
        let seed = 900_000 + gid as u32 * 1_000;
        let rnd = |k: u32| pseudo_rand(seed.wrapping_mul(31).wrapping_add(k));
        let shape = gal.shape();
        // Une galaxie extérieure a une couleur dominante et une secondaire
        let main_color = (rnd(900) * CLOUD_COLORS as f32) as usize % CLOUD_COLORS;
        let second_color = (main_color + 1 + (rnd(901) * (CLOUD_COLORS - 1) as f32) as usize) % CLOUD_COLORS;
        for i in 0..cloud_count(gid, gal.radius) as u32 {
            // Sur la structure de la galaxie (bras, anneaux, filaments…), avec un peu de dispersion
            let mut srng = Rng::new(seed.wrapping_add(i * 7919));
            let jitter = Vec3::new(rnd(i * 11 + 3) - 0.5, (rnd(i * 11 + 5) - 0.5) * 0.3, rnd(i * 11 + 4) - 0.5)
                * gal.radius * 0.05;
            let local = shape.sample_structure(&mut srng, 0.12) + jitter;
            let world = gal.center + gal.tilt * local;
            // Pas de nuage collé au trou noir central
            if local.length() < CORE_EXCLUSION * 3.0 {
                continue;
            }
            commands.spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(start_material.clone()),
                Transform::from_translation(world).with_scale(Vec3::ZERO),
                Visibility::Hidden,
                NotShadowCaster,
                GalaxyCloud {
                    galaxy_id: gid as u32,
                    size: gal.radius * (0.04 + 0.07 * rnd(i * 11 + 6)),
                    color: if gid == 0 {
                        (rnd(i * 11 + 7) * CLOUD_COLORS as f32) as usize % CLOUD_COLORS
                    } else if rnd(i * 11 + 7) < 0.65 {
                        main_color
                    } else {
                        second_color
                    },
                    stretch: 1.0 + rnd(i * 11 + 8) * 0.9,
                    roll: rnd(i * 11 + 9) * tau,
                },
            ));
        }
    }
}

fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Opacité (0 à 1) d'un nuage vu depuis la caméra : discret de très près (on
/// traverse le nuage), plein de loin, éteint quand la galaxie est très loin.
fn cloud_fade(dist: f32, size: f32, galaxy_dist: f32) -> f32 {
    let near = smoothstep((dist - 0.25 * size) / (0.9 * size));
    let far = 1.0 - smoothstep((galaxy_dist - CLOUD_FADE_START) / (CLOUD_FADE_END - CLOUD_FADE_START));
    near * far
}

fn update_clouds(
    cam_q: Query<&GlobalTransform, With<Camera3d>>,
    settings: Res<GameSettings>,
    mats: Option<Res<CloudMaterials>>,
    mut clouds: Query<(&GalaxyCloud, &mut Transform, &mut Visibility, &mut MeshMaterial3d<StandardMaterial>), Without<Camera3d>>,
    mut last: Local<Option<(Vec3, Vec3)>>,
) {
    let Some(mats) = mats else { return };
    let Ok(cam) = cam_q.get_single() else { return };
    let cam_pos = cam.translation();
    let fwd = cam.forward().as_vec3();
    // Des milliers de nuages : inutile de tout recalculer si la caméra n'a presque pas bougé
    if let Some((p, f)) = *last {
        if f.dot(fwd) > 0.9995 && p.distance_squared(cam_pos) < 25.0 * 25.0 {
            return;
        }
    }
    *last = Some((cam_pos, fwd));

    let galaxy_dist: Vec<f32> = settings.galaxies.iter().map(|g| cam_pos.distance(g.center)).collect();
    for (cloud, mut tf, mut vis, mut mat) in &mut clouds {
        let gdist = galaxy_dist.get(cloud.galaxy_id as usize).copied().unwrap_or(f32::MAX);
        let to_cloud = tf.translation - cam_pos;
        let dist = to_cloud.length();
        let fade = cloud_fade(dist, cloud.size, gdist);
        let step = (fade * (CLOUD_STEPS - 1) as f32).round() as usize;
        // Invisible, ou derrière la caméra : rien à dessiner
        if step == 0 || (dist > cloud.size && fwd.dot(to_cloud / dist) < -0.4) {
            if *vis != Visibility::Hidden {
                *vis = Visibility::Hidden;
            }
            continue;
        }
        if *vis != Visibility::Inherited {
            *vis = Visibility::Inherited;
        }
        let target = &mats.steps[cloud.color][step];
        if mat.0 != *target {
            mat.0 = target.clone();
        }
        // Le nuage fait face à la caméra, tourné et étiré à sa façon
        tf.look_to(to_cloud / dist.max(0.001), Vec3::Y);
        tf.rotation *= Quat::from_rotation_z(cloud.roll);
        tf.scale = Vec3::new(cloud.size * cloud.stretch, cloud.size, 1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn cloud_texture_is_soft_and_wispy() {
        let px = cloud_pixels();
        let n = CLOUD_TEXTURE as usize;
        let alpha = |x: usize, y: usize| px[(y * n + x) * 4 + 3];
        // Bords quasi transparents (le nuage ne finit pas en carré)
        for i in 0..n {
            assert!(alpha(i, 0) <= 2);
            assert!(alpha(0, i) <= 2);
            assert!(alpha(n - 1, i) <= 2);
            assert!(alpha(i, n - 1) <= 2);
        }
        // Ni vide ni plein : des zones opaques et des trous
        let total = n * n;
        let opaque = (0..total).filter(|i| px[i * 4 + 3] > 128).count();
        let empty = (0..total).filter(|i| px[i * 4 + 3] < 8).count();
        assert!(opaque > total / 50 && opaque < total / 2, "{opaque}");
        assert!(empty > total / 4, "{empty}");
    }

    #[test]
    fn npc_territories_are_compact_connected_and_stable() {
        let settings = GameSettings::default();
        let spatial = SystemSpatialIndex::build(&settings);
        let npcs = generate_npcs(&settings, &spatial);
        // Toujours les mêmes, pour tous les joueurs
        let again = generate_npcs(&settings, &spatial);
        assert_eq!(npcs.factions.len(), again.factions.len());
        assert!(npcs.factions.iter().zip(&again.factions).all(|(a, b)| a.name == b.name && a.stars == b.stars));
        // Des factions dans la galaxie principale, et dans les autres aussi
        let main = npcs.factions.iter().filter(|f| f.galaxy == 0).count();
        assert!((NPC_MAIN / 2..=NPC_MAIN).contains(&main), "{main}");
        assert!(npcs.factions.iter().any(|f| f.galaxy > 0));

        let mut seen = HashSet::new();
        for f in &npcs.factions {
            assert!(f.stars.len() >= NPC_MIN_STARS / 2 && f.stars.len() <= NPC_MAX_STARS);
            for &s in &f.stars {
                // Une étoile n'appartient qu'à une faction, dans sa galaxie
                assert!(seen.insert(s));
                assert_eq!(settings.systems[s].galaxy_id, f.galaxy);
                assert_eq!(npcs.faction_of(s).map(|x| x.name.as_str()), Some(f.name.as_str()));
            }
            // Territoire d'un seul tenant : tous les traits relient les étoiles d'un même bloc
            let mut reached = HashSet::from([f.stars[0]]);
            let mut grew = true;
            while grew {
                grew = false;
                for &a in &f.stars {
                    for &b in &f.stars {
                        let d = settings.systems[a].center().distance(settings.systems[b].center());
                        if reached.contains(&a) && !reached.contains(&b) && d <= link_range(&settings, a) {
                            reached.insert(b);
                            grew = true;
                        }
                    }
                }
            }
            assert_eq!(reached.len(), f.stars.len(), "{} n'est pas d'un seul tenant", f.name);
            assert!(!f.links.is_empty());
        }
        // Les couleurs de deux factions voisines dans la liste diffèrent
        assert!(npcs.factions.windows(2).all(|w| w[0].color != w[1].color));
    }

    #[test]
    fn group_links_join_a_territory_whatever_the_distance() {
        let settings = GameSettings::default();
        let key = |v: Vec3| v.to_array().map(|x| x as i32);
        let reachable = |stars: &[usize], links: &[(Vec3, Vec3)]| -> bool {
            // Toutes les étoiles sont reliées entre elles par un chemin de traits
            let centers: Vec<[i32; 3]> = stars.iter().map(|&s| key(settings.systems[s].center())).collect();
            let mut seen = HashSet::from([centers[0]]);
            let mut grew = true;
            while grew {
                grew = false;
                for (a, b) in links {
                    let (a, b) = (key(*a), key(*b));
                    if seen.contains(&a) != seen.contains(&b) {
                        seen.insert(a);
                        seen.insert(b);
                        grew = true;
                    }
                }
            }
            centers.iter().all(|c| seen.contains(c))
        };

        // Étoiles voisines
        let near: Vec<usize> = (0..settings.systems.len()).filter(|&i| settings.systems[i].galaxy_id == 0).take(40).collect();
        // Étoiles très éloignées, y compris de galaxies différentes : liées quand même
        let far: Vec<usize> = (0..5).map(|g| (0..settings.systems.len()).find(|&i| settings.systems[i].galaxy_id == g).unwrap()).collect();
        assert!(settings.systems[far[0]].center().distance(settings.systems[far[4]].center()) > 10_000_000.0);

        for stars in [near, far] {
            let links = group_links(&settings, &stars);
            assert!(reachable(&stars, &links));
            // Un seul trait par étoile ajoutée : réseau minimal, donc aucun triangle ni doublon
            assert_eq!(links.len(), stars.len() - 1);
            // Rien en dehors du groupe, pas de doublon, pas de trait sur une seule étoile
            let centers: HashSet<[i32; 3]> = stars.iter().map(|&s| key(settings.systems[s].center())).collect();
            let mut pairs: Vec<_> = links.iter().map(|(a, b)| (key(*a), key(*b))).collect();
            assert!(pairs.iter().all(|(a, b)| a != b && centers.contains(a) && centers.contains(b)));
            pairs.sort();
            let n = pairs.len();
            pairs.dedup();
            assert_eq!(n, pairs.len());
        }
        // Une seule étoile : aucun trait
        assert!(group_links(&settings, &[3]).is_empty());
    }

    #[test]
    fn clouds_fade_when_close_and_when_far() {
        let size = 100_000.0;
        // Au milieu du nuage : presque invisible ; à bonne distance : plein
        assert!(cloud_fade(10_000.0, size, 1.0e6) < 0.05);
        assert!(cloud_fade(400_000.0, size, 1.0e6) > 0.99);
        // Galaxie très lointaine : plus rien
        assert_eq!(cloud_fade(400_000.0, size, CLOUD_FADE_END + 1.0), 0.0);
        assert!(cloud_fade(400_000.0, size, (CLOUD_FADE_START + CLOUD_FADE_END) / 2.0) < 1.0);
    }

}
