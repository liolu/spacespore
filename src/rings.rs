//! Anneaux planétaires (C2 de `roadmaps/fait/ROADMAP-0.11.md`).
//!
//! - Profil radial (`Ring::profile`) : bandes, divisions vides, bords adoucis ; glace claire ou
//!   roche sombre selon `Ring::ice`.
//! - Ombres : la planète fait de l'ombre sur l'anneau (le cylindre derrière elle), l'anneau en fait
//!   sur la planète (une coquille sombre transparente juste au-dessus du sol). Les deux sont
//!   calculées dans un repère qui garde l'étoile à l'azimut 0 : l'anneau est symétrique, il suffit
//!   de tourner ce repère autour de l'axe de la planète à chaque image, et de ne recalculer les
//!   couleurs que lorsque la hauteur de l'étoile au-dessus de l'anneau change (saisons).
//! - Les particules, quand on traverse un anneau, viennent du champ d'astéroïdes (`asteroids.rs`).

use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::render::render_asset::RenderAssetUsages;

use crate::planet::{PlanetId, PlanetRoot, StarRoot};
use crate::planetgen::system::Ring;

pub struct RingsPlugin;

impl Plugin for RingsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, update_rings.after(crate::planet::orbit_planets));
    }
}

/// Repère d'un anneau (enfant de la racine de la planète) : plan équatorial, étoile à l'azimut 0.
#[derive(Component)]
pub struct RingFrame {
    planet_id: usize,
    ring: Ring,
    radius: f32,
    ring_mesh: Handle<Mesh>,
    shell_mesh: Handle<Mesh>,
    shell: Entity,
    /// Hauteur de l'étoile (radians) avec laquelle les ombres ont été calculées.
    beta: f32,
}

/// Divisions de la grille de l'anneau (rayon, tour) et de la coquille d'ombre (latitude, longitude).
const RING_RADIAL: usize = 140;
const RING_AROUND: usize = 192;
const SHELL_LAT: usize = 48;
const SHELL_LON: usize = 96;

/// Ombre de la planète sur l'anneau : part de lumière gardée.
const PLANET_SHADOW: f32 = 0.1;
/// Opacité maximale de l'ombre de l'anneau sur la planète.
const RING_SHADOW: f32 = 0.8;

fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Éclairement (0..1) d'un point `p` du plan de l'anneau, l'étoile étant dans la direction `s` :
/// dans le cylindre d'ombre de la planète (rayon `r`) derrière elle, il fait sombre.
pub fn ring_light(p: Vec3, s: Vec3, r: f32) -> f32 {
    let along = p.dot(s);
    if along >= 0.0 {
        return 1.0;
    }
    let off = (p - s * along).length();
    PLANET_SHADOW + (1.0 - PLANET_SHADOW) * smooth(r * 0.98, r * 1.02, off)
}

/// Ombre de l'anneau (opacité 0..1) en un point `q` de la planète, l'étoile dans la direction `s`.
pub fn ring_shadow(ring: &Ring, q: Vec3, s: Vec3) -> f32 {
    // Côté nuit : rien à assombrir de plus
    let day = smooth(-0.05, 0.1, q.normalize_or_zero().dot(s));
    if day <= 0.0 || s.y.abs() < 1e-4 || q.y * s.y > 0.0 {
        return 0.0;
    }
    // Le rayon vers l'étoile traverse le plan de l'anneau
    let lambda = -q.y / s.y;
    let h = q + s * lambda;
    let rho = Vec2::new(h.x, h.z).length();
    let f = (rho - ring.inner) / (ring.outer - ring.inner).max(1.0);
    ring.profile(f) * RING_SHADOW * day
}

fn ring_colors(ring: &Ring, radius: f32, beta: f32) -> Vec<[f32; 4]> {
    let s = Vec3::new(beta.cos(), beta.sin(), 0.0);
    let mut out = Vec::with_capacity((RING_RADIAL + 1) * (RING_AROUND + 1));
    for j in 0..=RING_AROUND {
        let a = j as f32 / RING_AROUND as f32 * std::f32::consts::TAU;
        for i in 0..=RING_RADIAL {
            let f = i as f32 / RING_RADIAL as f32;
            let rho = ring.inner + (ring.outer - ring.inner) * f;
            let p = Vec3::new(rho * a.cos(), 0.0, rho * a.sin());
            let alpha = ring.profile(f);
            // Bandes plus claires là où l'anneau est dense
            let v = (0.75 + 0.45 * alpha / ring.opacity.max(0.05)).min(1.2) * ring_light(p, s, radius);
            out.push([ring.color[0] * v, ring.color[1] * v, ring.color[2] * v, alpha]);
        }
    }
    out
}

fn ring_mesh(ring: &Ring, radius: f32) -> Mesh {
    let mut pos = Vec::with_capacity((RING_RADIAL + 1) * (RING_AROUND + 1));
    let mut idx: Vec<u32> = Vec::with_capacity(RING_RADIAL * RING_AROUND * 6);
    for j in 0..=RING_AROUND {
        let a = j as f32 / RING_AROUND as f32 * std::f32::consts::TAU;
        for i in 0..=RING_RADIAL {
            let rho = ring.inner + (ring.outer - ring.inner) * i as f32 / RING_RADIAL as f32;
            pos.push([rho * a.cos(), 0.0, rho * a.sin()]);
        }
    }
    let row = (RING_RADIAL + 1) as u32;
    for j in 0..RING_AROUND as u32 {
        for i in 0..RING_RADIAL as u32 {
            let k = j * row + i;
            idx.extend_from_slice(&[k, k + 1, k + row, k + 1, k + row + 1, k + row]);
        }
    }
    let n = pos.len();
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; n]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, ring_colors(ring, radius, 0.3));
    mesh.insert_indices(Indices::U32(idx));
    mesh
}

fn shell_points(radius: f32) -> Vec<Vec3> {
    let mut out = Vec::with_capacity((SHELL_LAT + 1) * (SHELL_LON + 1));
    for j in 0..=SHELL_LAT {
        let lat = -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * j as f32 / SHELL_LAT as f32;
        for i in 0..=SHELL_LON {
            let lon = std::f32::consts::TAU * i as f32 / SHELL_LON as f32;
            out.push(Vec3::new(lat.cos() * lon.cos(), lat.sin(), lat.cos() * lon.sin()) * radius);
        }
    }
    out
}

fn shell_colors(ring: &Ring, radius: f32, beta: f32) -> Vec<[f32; 4]> {
    let s = Vec3::new(beta.cos(), beta.sin(), 0.0);
    shell_points(radius).into_iter().map(|q| [0.0, 0.0, 0.0, ring_shadow(ring, q, s)]).collect()
}

fn shell_mesh(ring: &Ring, radius: f32) -> Mesh {
    let pos = shell_points(radius);
    let normals: Vec<[f32; 3]> = pos.iter().map(|p| p.normalize().to_array()).collect();
    let row = (SHELL_LON + 1) as u32;
    let mut idx: Vec<u32> = Vec::with_capacity(SHELL_LAT * SHELL_LON * 6);
    for j in 0..SHELL_LAT as u32 {
        for i in 0..SHELL_LON as u32 {
            let k = j * row + i;
            idx.extend_from_slice(&[k, k + row, k + 1, k + 1, k + row, k + row + 1]);
        }
    }
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos.iter().map(|p| p.to_array()).collect::<Vec<_>>());
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, shell_colors(ring, radius, 0.3));
    mesh.insert_indices(Indices::U32(idx));
    mesh
}

/// Crée l'anneau d'une planète (et la coquille de son ombre) sous la racine `root`.
#[allow(clippy::too_many_arguments)]
pub fn spawn_ring(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, ring: &Ring, radius: f32, relief: f32, planet_id: usize, root: Entity) {
    let ring_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        double_sided: true,
        perceptual_roughness: 1.0,
        ..default()
    });
    let shell_mat = materials.add(StandardMaterial { base_color: Color::WHITE, unlit: true, alpha_mode: AlphaMode::Blend, ..default() });
    let ring_mesh = meshes.add(ring_mesh(ring, radius));
    // Juste au-dessus du sol (et des montagnes d'une rocheuse)
    let shell_r = radius * 1.004 + relief * 1.05;
    let shell_mesh = meshes.add(shell_mesh(ring, shell_r));
    let shell = commands.spawn((Mesh3d(shell_mesh.clone()), MeshMaterial3d(shell_mat), Transform::IDENTITY, Visibility::default(), NotShadowCaster)).id();
    let ring_e = commands.spawn((Mesh3d(ring_mesh.clone()), MeshMaterial3d(ring_mat), Transform::IDENTITY, NotShadowCaster)).id();
    let frame = commands
        .spawn((
            Transform::IDENTITY,
            Visibility::default(),
            RingFrame { planet_id, ring: *ring, radius, ring_mesh, shell_mesh, shell, beta: 0.3 },
        ))
        .add_children(&[ring_e, shell])
        .id();
    commands.entity(root).add_child(frame);
}

fn set_colors(meshes: &mut Assets<Mesh>, handle: &Handle<Mesh>, colors: Vec<[f32; 4]>) {
    if let Some(mesh) = meshes.get_mut(handle) {
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, VertexAttributeValues::Float32x4(colors));
    }
}

#[allow(clippy::type_complexity)]
fn update_rings(
    surface: Res<crate::surface::Surface>,
    stars: Query<&Transform, (With<StarRoot>, Without<RingFrame>)>,
    roots: Query<(&Transform, &PlanetId), (With<PlanetRoot>, Without<RingFrame>)>,
    cam_q: Query<&Transform, (With<Camera3d>, Without<RingFrame>, Without<PlanetRoot>)>,
    mut frames: Query<(&mut RingFrame, &mut Transform, &Parent)>,
    mut vis: Query<&mut Visibility>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let cam = cam_q.get_single().ok().map(|c| c.translation);
    for (mut frame, mut tf, parent) in &mut frames {
        let Ok((root, _)) = roots.get(parent.get()) else { continue };
        let Some(star) = stars.iter().map(|s| s.translation).min_by(|a, b| a.distance_squared(root.translation).total_cmp(&b.distance_squared(root.translation))) else {
            continue;
        };
        let to_star = (star - root.translation).normalize_or(Vec3::X);
        // Plan de l'anneau sans la rotation du jour, puis l'étoile ramenée à l'azimut 0
        let plane = Quat::from_rotation_arc(Vec3::Y, root.rotation * Vec3::Y);
        let s = plane.inverse() * to_star;
        let azimuth = (-s.z).atan2(s.x);
        let rot = root.rotation.inverse() * plane * Quat::from_rotation_y(azimuth);
        if tf.rotation != rot {
            tf.rotation = rot;
        }
        let beta = s.y.clamp(-1.0, 1.0).asin();
        if (beta - frame.beta).abs() > 0.004 {
            frame.beta = beta;
            let (ring, radius) = (frame.ring, frame.radius);
            set_colors(&mut meshes, &frame.ring_mesh, ring_colors(&ring, radius, beta));
            let shell_r = meshes.get(&frame.shell_mesh).and_then(|m| m.attribute(Mesh::ATTRIBUTE_POSITION)).and_then(|p| p.as_float3()).and_then(|p| p.first().copied()).map_or(radius, |p| Vec3::from(p).length());
            set_colors(&mut meshes, &frame.shell_mesh, shell_colors(&ring, shell_r, beta));
        }
        // La coquille d'ombre disparaît près du sol (vol bas, atterrissage) : le terrain prend le relais
        let near = cam.is_some_and(|c| c.distance(root.translation) < frame.radius * 1.15)
            || surface.body() == Some(crate::ui::TargetKind::Planet(frame.planet_id));
        if let Ok(mut v) = vis.get_mut(frame.shell) {
            let wanted = if near { Visibility::Hidden } else { Visibility::Inherited };
            if *v != wanted {
                *v = wanted;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ring() -> Ring {
        Ring { inner: 1300.0, outer: 2300.0, color: [0.9, 0.85, 0.78], opacity: 0.7, ice: 0.9, gaps: [[0.55, 0.05], [0.0, 0.0], [0.0, 0.0]], seed: 77 }
    }

    #[test]
    fn profile_has_gaps_and_soft_edges() {
        let r = ring();
        assert!(r.profile(0.55) < 0.01, "division vide");
        assert!(r.profile(0.3) > 0.1 && r.profile(0.8) > 0.1);
        assert_eq!(r.profile(-0.1), 0.0);
        assert!(r.profile(0.001) < r.profile(0.3));
        for i in 0..=100 {
            let v = r.profile(i as f32 / 100.0);
            assert!((0.0..=r.opacity).contains(&v));
        }
    }

    #[test]
    fn planet_shades_the_ring_behind_it() {
        let s = Vec3::X;
        assert_eq!(ring_light(Vec3::new(1800.0, 0.0, 0.0), s, 1000.0), 1.0, "cote jour");
        assert!(ring_light(Vec3::new(-1800.0, 0.0, 0.0), s, 1000.0) < 0.2, "derriere la planete");
        assert_eq!(ring_light(Vec3::new(-1800.0, 0.0, 1500.0), s, 1000.0), 1.0, "a cote de l'ombre");
    }

    #[test]
    fn ring_shades_the_planet_on_the_winter_side() {
        let r = ring();
        // Étoile au-dessus de l'anneau (été du nord) : l'ombre tombe sur l'hémisphère sud, côté jour
        let beta = 0.4f32;
        let s = Vec3::new(beta.cos(), beta.sin(), 0.0);
        let shaded = (0..90).map(|k| -(k as f32).to_radians()).map(|lat| ring_shadow(&r, Vec3::new(lat.cos(), lat.sin(), 0.0) * 1000.0, s)).fold(0.0f32, f32::max);
        assert!(shaded > 0.3, "{shaded}");
        // Hémisphère nord : rien
        let north = (0..90).map(|k| (k as f32).to_radians()).map(|lat| ring_shadow(&r, Vec3::new(lat.cos(), lat.sin(), 0.0) * 1000.0, s)).fold(0.0f32, f32::max);
        assert_eq!(north, 0.0);
        // Côté nuit : rien
        assert_eq!(ring_shadow(&r, Vec3::new(-0.9, -0.3, 0.0).normalize() * 1000.0, s), 0.0);
    }
}
