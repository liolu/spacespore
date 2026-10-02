// ─────────────────────────────────────────────────────────────────────────
//  Zones chaude, habitable et froide du système chargé (option « Afficher zones »)
//
//  Trois anneaux dans le plan du système : rouge (trop chaud pour l'eau liquide), vert (zone
//  habitable : √(L/1,1) à √(L/0,53) UA, l'insolation de la Terre ±), bleu (froid, jusqu'au bout
//  du système). Les orbites affichées sont comprimées (échelle log) puis écartées : les limites
//  sont placées en interpolant entre les vraies planètes, pour qu'une planète dessinée dans la
//  bande verte soit vraiment dans la zone habitable.
// ─────────────────────────────────────────────────────────────────────────

use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;

use crate::planet::{SpawnedSystems, StarId, StarRoot};
use crate::planetgen::system::display_distance;
use crate::settings::{GameSettings, StarSystemConfig};

pub struct ZonesPlugin;

impl Plugin for ZonesPlugin {
    fn build(&self, app: &mut App) {
        // Après le chargement / déchargement des systèmes : jamais d'anneau rattaché à une étoile
        // en train de disparaître
        app.init_resource::<ZonesState>()
            .add_systems(Update, (update_zones, draw_zone_edges).after(crate::planet::stream_system_bodies));
    }
}

/// Rayons (unités du jeu, depuis l'étoile) des limites des zones.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Zones {
    /// Surface de l'étoile (début de la zone chaude).
    pub star: f32,
    pub habitable_inner: f32,
    pub habitable_outer: f32,
    /// Fin de la zone froide (bout du système).
    pub edge: f32,
}

/// Limites physiques de la zone habitable (UA) d'une étoile de luminosité `l` (L☉).
pub fn habitable_au(l: f64) -> (f64, f64) {
    let l = l.max(1e-7);
    ((l / 1.1).sqrt(), (l / 0.53).sqrt())
}

/// Distance affichée d'une orbite de `au` UA, d'après les planètes du système (interpolation en
/// log(UA) entre elles ; au-delà, l'échelle log du générateur, recalée sur la planète la plus proche).
pub fn display_for(sys: &StarSystemConfig, au: f64, hz: f64) -> f32 {
    let scale = sys.stars.first().map_or(1_000_000.0, |s| s.scale()) as f64;
    let mut pts: Vec<(f64, f64)> = sys
        .planets()
        .iter()
        .filter(|p| p.semi_major_au > 0.0)
        .map(|p| ((p.semi_major_au as f64).ln(), p.orbit_distance as f64))
        .collect();
    pts.sort_by(|a, b| a.0.total_cmp(&b.0));
    let x = au.max(1e-6).ln();
    let fallback = |a: f64| display_distance(a, hz) * scale;
    let d = match (pts.first(), pts.last()) {
        (Some(&(x0, d0)), _) if x <= x0 => d0 * fallback(au) / fallback(x0.exp()),
        (_, Some(&(x1, d1))) if x >= x1 => d1 * fallback(au) / fallback(x1.exp()),
        (Some(_), Some(_)) => {
            let k = pts.windows(2).position(|w| x >= w[0].0 && x <= w[1].0).unwrap_or(0);
            let (a, b) = (pts[k], pts[k + 1]);
            let t = if b.0 > a.0 { (x - a.0) / (b.0 - a.0) } else { 0.0 };
            a.1 + (b.1 - a.1) * t
        }
        _ => fallback(au),
    };
    d as f32
}

/// Zones d'un système généré (`None` : étoile faite à la main, sans physique).
pub fn zones_of(sys: &StarSystemConfig) -> Option<Zones> {
    let physics = sys.star_physics()?;
    let star = sys.stars.first()?;
    let hz = physics.luminosity_sun.max(1e-7).sqrt();
    let (inner, outer) = habitable_au(physics.luminosity_sun);
    let star_r = star.radius * 1.05;
    let hi = display_for(sys, inner, hz).max(star_r * 1.1);
    let ho = display_for(sys, outer, hz).max(hi * 1.05);
    let last = sys
        .planets()
        .iter()
        .map(|p| p.orbit_distance * (1.0 + p.eccentricity) + p.radius)
        .fold(0.0_f32, f32::max);
    Some(Zones { star: star_r, habitable_inner: hi, habitable_outer: ho, edge: (last * 1.15).max(ho * 1.6) })
}

#[derive(Resource, Default)]
struct ZonesState {
    system: Option<usize>,
    entities: Vec<Entity>,
    zones: Option<Zones>,
}

/// Anneau plat (plan XZ), normales vers le haut.
fn band(inner: f32, outer: f32) -> Mesh {
    use bevy::render::mesh::{Indices, PrimitiveTopology};
    use bevy::render::render_asset::RenderAssetUsages;
    let seg = 192;
    let mut pos = Vec::with_capacity(seg * 2 + 2);
    let mut idx = Vec::with_capacity(seg * 6);
    for i in 0..=seg {
        let a = i as f32 / seg as f32 * std::f32::consts::TAU;
        let (c, s) = (a.cos(), a.sin());
        pos.push([c * inner, 0.0, s * inner]);
        pos.push([c * outer, 0.0, s * outer]);
    }
    for i in 0..seg as u32 {
        let k = i * 2;
        idx.extend_from_slice(&[k, k + 1, k + 2, k + 1, k + 3, k + 2]);
    }
    let normals = vec![[0.0, 1.0, 0.0]; pos.len()];
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_indices(Indices::U32(idx));
    mesh
}

pub const HOT: [f32; 3] = [1.0, 0.35, 0.15];
pub const HABITABLE: [f32; 3] = [0.25, 1.0, 0.4];
pub const COLD: [f32; 3] = [0.3, 0.55, 1.0];

#[allow(clippy::too_many_arguments)]
fn update_zones(
    mut commands: Commands,
    settings: Res<GameSettings>,
    spawned: Res<SpawnedSystems>,
    stars: Query<(Entity, &StarId), With<StarRoot>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut state: ResMut<ZonesState>,
) {
    let loaded = spawned.0.iter().next().copied();
    let wanted = if settings.show_zones { loaded } else { None };
    let root = wanted.and_then(|si| stars.iter().find(|(_, id)| id.0 == si * 1000).map(|(e, _)| e));
    if state.system == wanted && (wanted.is_none() || !state.entities.is_empty() || root.is_none()) {
        return;
    }
    for e in state.entities.drain(..) {
        if let Some(ec) = commands.get_entity(e) {
            ec.despawn_recursive();
        }
    }
    state.system = wanted;
    state.zones = None;
    let (Some(si), Some(root)) = (wanted, root) else { return };
    let Some(sys) = settings.systems.get(si) else { return };
    let Some(z) = zones_of(sys) else { return };
    state.zones = Some(z);
    for (inner, outer, c, alpha) in [
        (z.star, z.habitable_inner, HOT, 0.10),
        (z.habitable_inner, z.habitable_outer, HABITABLE, 0.16),
        (z.habitable_outer, z.edge, COLD, 0.08),
    ] {
        let material = materials.add(StandardMaterial {
            base_color: Color::srgba(c[0], c[1], c[2], alpha),
            emissive: LinearRgba::new(c[0] * 0.4, c[1] * 0.4, c[2] * 0.4, 1.0),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            cull_mode: None,
            double_sided: true,
            ..default()
        });
        let e = commands.spawn((Mesh3d(meshes.add(band(inner, outer))), MeshMaterial3d(material), Transform::IDENTITY, NotShadowCaster)).id();
        commands.entity(root).add_child(e);
        state.entities.push(e);
    }
}

/// Traits nets aux limites (visibles même de loin, quand les bandes sont pâles).
fn draw_zone_edges(
    settings: Res<GameSettings>,
    state: Res<ZonesState>,
    stars: Query<(&GlobalTransform, &StarId), With<StarRoot>>,
    mut gizmos: Gizmos,
) {
    let (true, Some(si), Some(z)) = (settings.show_zones, state.system, state.zones) else { return };
    let Some(center) = stars.iter().find(|(_, id)| id.0 == si * 1000).map(|(gt, _)| gt.translation()) else { return };
    let flat = Isometry3d::new(center, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2));
    for (r, c) in [(z.habitable_inner, HOT), (z.habitable_outer, HABITABLE), (z.edge, COLD)] {
        gizmos.circle(flat, r, Color::srgb(c[0], c[1], c[2])).resolution(128);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sun_s_habitable_zone_contains_the_earth() {
        let (inner, outer) = habitable_au(1.0);
        assert!(inner < 1.0 && outer > 1.0 && inner > 0.9 && outer < 1.5, "{inner} {outer}");
        // Une naine rouge : zone habitable très proche
        let (i, o) = habitable_au(0.002);
        assert!(i < 0.05 && o < 0.07);
    }

    #[test]
    fn zones_are_ordered_and_planets_in_the_green_band_are_habitable_distance() {
        let settings = GameSettings::default();
        let mut checked = 0;
        for sys in settings.systems.iter().take(400) {
            let Some(z) = zones_of(sys) else { continue };
            assert!(z.star < z.habitable_inner && z.habitable_inner < z.habitable_outer && z.habitable_outer < z.edge, "{z:?}");
            let l = sys.star_physics().unwrap().luminosity_sun;
            let (inner, outer) = habitable_au(l);
            for p in sys.planets() {
                let au = p.semi_major_au as f64;
                let d = p.orbit_distance;
                if au > inner * 1.01 && au < outer * 0.99 && d > z.star * 1.2 {
                    assert!(d >= z.habitable_inner * 0.999 && d <= z.habitable_outer * 1.001, "planete a {au} UA dessinee a {d}, zone {z:?}");
                    checked += 1;
                }
                if au < inner * 0.99 && d > z.habitable_inner * 1.001 {
                    panic!("planete chaude ({au} UA) dessinee dans la zone habitable");
                }
            }
            assert!(z.edge >= sys.planets().iter().map(|p| p.orbit_distance).fold(0.0, f32::max));
        }
        assert!(checked > 10, "{checked}");
    }
}
