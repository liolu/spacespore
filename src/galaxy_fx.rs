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
use std::collections::HashSet;

use crate::net::Net;
use crate::net_ui::NetPanel;
use crate::settings::{pseudo_rand, GameSettings, SystemSpatialIndex, CORE_EXCLUSION};
use crate::ui::MenuState;
use crate::{CameraController, ZoomLevel};

// ── Liaisons ────────────────────────────────────────────────────────────

/// Liaisons par étoile au plus.
const MAX_LINKS_PER_STAR: usize = 3;
/// Étoiles de départ au plus par image (borne le nombre de traits).
const MAX_LINK_STARS: usize = 250;
/// Portée d'une liaison : la galaxie principale est plus dense que les autres.
const LINK_RANGE_MAIN: f32 = 30_000.0;
const LINK_RANGE_OTHER: f32 = 60_000.0;

pub struct GalaxyFxPlugin;

impl Plugin for GalaxyFxPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ShowLinks(true))
            .add_systems(Startup, spawn_clouds)
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
    let text = if show.0 { "Liaisons entre etoiles affichees (L)." } else { "Liaisons entre etoiles masquees (L)." };
    net.notify(text, time.elapsed_secs_f64());
}

/// Les étoiles voisines de `sys` (les plus proches d'abord), à portée de liaison.
fn neighbours(settings: &GameSettings, spatial: &SystemSpatialIndex, sys: usize) -> Vec<usize> {
    let a = &settings.systems[sys];
    let range = if a.galaxy_id == 0 { LINK_RANGE_MAIN } else { LINK_RANGE_OTHER };
    let center = a.center();
    let mut near: Vec<(usize, f32)> = spatial
        .systems_in_radius(center, range)
        .into_iter()
        .filter(|&i| i != sys)
        .filter_map(|i| {
            let b = settings.systems.get(i)?;
            let d = b.center().distance(center);
            (b.galaxy_id == a.galaxy_id && d <= range).then_some((i, d))
        })
        .collect();
    near.sort_by(|x, y| x.1.total_cmp(&y.1));
    near.into_iter().take(MAX_LINKS_PER_STAR).map(|(i, _)| i).collect()
}

fn draw_links(
    show: Res<ShowLinks>,
    zoom: Res<ZoomLevel>,
    settings: Res<GameSettings>,
    spatial: Res<SystemSpatialIndex>,
    cam_q: Query<(&GlobalTransform, &CameraController)>,
    mut gizmos: Gizmos,
) {
    if !show.0 || !matches!(*zoom, ZoomLevel::System | ZoomLevel::Sector | ZoomLevel::Galaxy) {
        return;
    }
    let Ok((cam, ctrl)) = cam_q.get_single() else { return };
    let cam_pos = cam.translation();
    let window = (ctrl.distance * 1.5).clamp(60_000.0, 1_500_000.0);

    // Étoiles autour de la caméra, les plus proches d'abord
    let mut stars: Vec<(usize, f32)> = spatial
        .systems_in_radius(cam_pos, window)
        .into_iter()
        .filter_map(|i| {
            let s = settings.systems.get(i)?;
            let d = s.center().distance(cam_pos);
            (d <= window && s.center().distance(settings.galaxies.get(s.galaxy_id as usize).map_or(Vec3::ZERO, |g| g.center)) >= CORE_EXCLUSION)
                .then_some((i, d))
        })
        .collect();
    stars.sort_by(|a, b| a.1.total_cmp(&b.1));
    stars.truncate(MAX_LINK_STARS);

    let mut drawn: HashSet<(usize, usize)> = HashSet::new();
    let color = Color::srgba(0.55, 0.75, 1.0, 0.35);
    for (a, _) in stars {
        for b in neighbours(&settings, &spatial, a) {
            if drawn.insert((a.min(b), a.max(b))) {
                gizmos.line(settings.systems[a].center(), settings.systems[b].center(), color);
            }
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
const CLOUD_FADE_START: f32 = 50_000_000.0;
const CLOUD_FADE_END: f32 = 60_000_000.0;

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
        for i in 0..cloud_count(gid, gal.radius) as u32 {
            // Sur un bras (même spirale que les bras), avec un peu de dispersion
            let arm = (rnd(i * 11 + 1) * gal.num_arms as f32) as usize % gal.num_arms.max(1);
            let t = 0.12 + 0.83 * rnd(i * 11 + 2).powf(0.8);
            let r = t * t * gal.radius;
            let theta = arm as f32 * tau / gal.num_arms.max(1) as f32 + t * t * gal.twist;
            let scatter = (rnd(i * 11 + 3) - 0.5) * gal.radius * 0.07 * (0.4 + t);
            let along = (rnd(i * 11 + 4) - 0.5) * gal.radius * 0.05;
            let local = Vec3::new(
                r * theta.cos() - theta.sin() * scatter + theta.cos() * along,
                (rnd(i * 11 + 5) - 0.5) * gal.radius * 0.02,
                r * theta.sin() + theta.cos() * scatter + theta.sin() * along,
            );
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
                    color: (rnd(i * 11 + 7) * CLOUD_COLORS as f32) as usize % CLOUD_COLORS,
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
    fn clouds_fade_when_close_and_when_far() {
        let size = 100_000.0;
        // Au milieu du nuage : presque invisible ; à bonne distance : plein
        assert!(cloud_fade(10_000.0, size, 1.0e6) < 0.05);
        assert!(cloud_fade(400_000.0, size, 1.0e6) > 0.99);
        // Galaxie très lointaine : plus rien
        assert_eq!(cloud_fade(400_000.0, size, 70_000_000.0), 0.0);
        assert!(cloud_fade(400_000.0, size, 55_000_000.0) < 1.0);
    }

    #[test]
    fn links_join_close_stars_only_within_their_galaxy() {
        let settings = GameSettings::default();
        let spatial = SystemSpatialIndex::build(&settings);
        let mut linked = 0;
        for sys in (0..settings.systems.len()).step_by(37) {
            let n = neighbours(&settings, &spatial, sys);
            assert!(n.len() <= MAX_LINKS_PER_STAR);
            let a = &settings.systems[sys];
            let range = if a.galaxy_id == 0 { LINK_RANGE_MAIN } else { LINK_RANGE_OTHER };
            for b in &n {
                let b = &settings.systems[*b];
                assert_eq!(a.galaxy_id, b.galaxy_id);
                assert!(a.center().distance(b.center()) <= range);
            }
            linked += usize::from(!n.is_empty());
        }
        assert!(linked > 0);
    }
}
