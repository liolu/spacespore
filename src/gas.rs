// ─────────────────────────────────────────────────────────────────────────
//  Géantes gazeuses et neptuniennes (phase 2 de `ROADMAP-0.10.md`)
//
//  Pas de sol : on peut y entrer en vol (navigation basse altitude), mais la pression abîme la
//  coque du vaisseau tant qu'il y reste, d'autant plus vite qu'il descend profond et que la
//  planète est lourde. À 0 PV il est détruit (`combat.rs`) puis réapparaît en orbite, hors de
//  l'atmosphère. À l'intérieur, brouillard et ciel prennent la couleur des nuages, de plus en
//  plus sombre avec la profondeur.
// ─────────────────────────────────────────────────────────────────────────

use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;

use crate::mesher::GasLook;
use crate::net::Net;
use crate::planet::{PlanetId, PlanetRoot};
use crate::settings::GameSettings;
use crate::ship::Ship;
use crate::terrain::GAS_CORE;

pub struct GasPlugin;

impl Plugin for GasPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GasState>().add_systems(
            Update,
            (detect_gas, gas_atmosphere).chain().after(crate::surface::SurfaceControl),
        );
    }
}

/// Le vaisseau dans une géante.
#[derive(Clone, Debug, PartialEq)]
pub struct Inside {
    pub planet_id: usize,
    pub name: String,
    /// 0 au sommet des nuages, 1 au cœur.
    pub depth: f32,
    /// PV perdus par seconde.
    pub damage_per_sec: f32,
    /// Couleur des nuages autour du vaisseau.
    pub color: [f32; 3],
}

#[derive(Resource, Default)]
pub struct GasState {
    pub inside: Option<Inside>,
}

/// PV perdus par seconde à la profondeur `depth` (0 au sommet, 1 au cœur) d'une géante de
/// gravité `gravity` (g) : quelques PV par seconde dans la haute atmosphère (une minute pour
/// ressortir), une quarantaine près du cœur (détruit en 2 à 3 secondes).
pub fn pressure_damage(depth: f32, gravity: f32) -> f32 {
    let d = depth.clamp(0.0, 1.0);
    (1.5 + 40.0 * d * d) * (0.5 + 0.5 * gravity.clamp(0.3, 3.0))
}

/// Profondeur (0..1) d'un point à `r` du centre d'une géante de rayon `radius`.
pub fn depth_at(r: f32, radius: f32) -> f32 {
    ((radius - r) / (radius * (1.0 - GAS_CORE))).clamp(0.0, 1.0)
}

/// Couleur moyenne des nuages d'une géante.
fn mean_color(look: &GasLook) -> [f32; 3] {
    let n = look.palette.len().max(1) as f32;
    let sum = look.palette.iter().fold([0.0; 3], |a, c| [a[0] + c[0], a[1] + c[1], a[2] + c[2]]);
    [sum[0] / n, sum[1] / n, sum[2] / n]
}

fn detect_gas(
    time: Res<Time>,
    settings: Res<GameSettings>,
    ship_q: Query<&GlobalTransform, With<Ship>>,
    planets: Query<(&GlobalTransform, &PlanetId), With<PlanetRoot>>,
    mut state: ResMut<GasState>,
    mut net: ResMut<Net>,
) {
    let Ok(ship) = ship_q.get_single() else { return };
    let ship = ship.translation();
    let mut inside = None;
    // Vaisseau détruit : il n'est plus nulle part
    if net.local.hp > 0 {
        for (gt, pid) in &planets {
            let Some(sys) = settings.systems.get(pid.0 / 1000) else { continue };
            let Some(p) = sys.planets().get(pid.0 % 1000) else { continue };
            if !p.gaseous() {
                continue;
            }
            let r = ship.distance(gt.translation());
            if r < p.radius {
                let depth = depth_at(r, p.radius);
                inside = Some(Inside {
                    planet_id: pid.0,
                    name: format!("{} {}", sys.name, pid.0 % 1000 + 1),
                    depth,
                    damage_per_sec: pressure_damage(depth, p.gravity_g),
                    color: mean_color(&crate::planet::gas_look(p)),
                });
                break;
            }
        }
    }
    let entered = match (&state.inside, &inside) {
        (None, Some(now)) => Some(now.name.clone()),
        (Some(before), Some(now)) if before.planet_id != now.planet_id => Some(now.name.clone()),
        _ => None,
    };
    if let Some(name) = entered {
        net.notify(
            &format!("ALERTE : vous entrez dans l'atmosphere de {name}. La pression abime la coque : remontez (Espace) !"),
            time.elapsed_secs_f64(),
        );
    }
    if state.inside != inside {
        state.inside = inside;
    }
}

/// Brouillard et ciel aux couleurs des nuages, de plus en plus sombres avec la profondeur.
fn gas_atmosphere(
    mut commands: Commands,
    state: Res<GasState>,
    surface: Res<crate::surface::Surface>,
    mut clear: ResMut<ClearColor>,
    cam_q: Query<(Entity, Has<DistanceFog>), With<Camera3d>>,
) {
    let Ok((cam, has_fog)) = cam_q.get_single() else { return };
    match &state.inside {
        Some(g) => {
            let light = 1.0 - 0.8 * g.depth;
            let c = Color::srgb(g.color[0] * light, g.color[1] * light, g.color[2] * light);
            clear.0 = c;
            // On voit à quelques milliers d'unités sous les nuages, à peine plus loin que le vaisseau au cœur
            let visibility = 30_000.0 * (1.0 - g.depth) + 600.0;
            commands.entity(cam).insert(DistanceFog {
                color: c,
                falloff: FogFalloff::from_visibility(visibility),
                ..default()
            });
        }
        None if has_fog => {
            commands.entity(cam).remove::<DistanceFog>();
            // Sorti de l'atmosphère : la navigation repeint le ciel elle-même, sinon c'est l'espace
            if !surface.active() {
                clear.0 = crate::surface::SPACE_SKY;
            }
        }
        None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deeper_and_heavier_hurts_more() {
        assert!(pressure_damage(0.0, 1.0) < 3.0, "sommet des nuages : on a le temps de ressortir");
        assert!(pressure_damage(1.0, 1.0) > 30.0, "au coeur : quelques secondes");
        assert!(pressure_damage(0.5, 2.5) > pressure_damage(0.5, 1.0));
        let mut last = 0.0;
        for i in 0..=10 {
            let d = pressure_damage(i as f32 / 10.0, 1.0);
            assert!(d > last);
            last = d;
        }
        // Rester au sommet détruit le vaisseau (100 PV) en moins de deux minutes, pas en un instant
        let secs = 100.0 / pressure_damage(0.05, 1.0);
        assert!((20.0..120.0).contains(&secs), "{secs}");
    }

    #[test]
    fn depth_goes_from_the_clouds_to_the_core() {
        assert_eq!(depth_at(100_000.0, 100_000.0), 0.0);
        assert_eq!(depth_at(100_000.0 * GAS_CORE, 100_000.0), 1.0);
        assert!((depth_at(65_000.0, 100_000.0) - 0.5).abs() < 1e-4);
        assert_eq!(depth_at(150_000.0, 100_000.0), 0.0);
    }
}
