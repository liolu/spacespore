//! Fumée du vaisseau (0.13.6) : de petits cubes laissés derrière lui sur son chemin, qui dérivent un peu,
//! tournent et rétrécissent jusqu'à disparaître. Positions gardées en absolu (origine flottante).

use bevy::math::DVec3;
use bevy::pbr::{NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;

use crate::settings::origin;
use crate::ship::Ship;

/// Durée de vie d'un cube (s).
const LIFE: f32 = 2.6;
/// Cubes au plus en même temps.
const MAX_CUBES: usize = 400;
/// Un cube tous les combien de longueurs de vaisseau parcourues.
const SPACING: f32 = 0.35;

pub struct SmokePlugin;

impl Plugin for SmokePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SmokeState>().add_systems(Startup, setup).add_systems(PostUpdate, (emit, animate).chain().before(TransformSystem::TransformPropagate));
    }
}

#[derive(Resource, Default)]
struct SmokeState {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    last: Option<DVec3>,
    count: usize,
}

#[derive(Component)]
pub(crate) struct SmokeCube {
    abs: DVec3,
    drift: Vec3,
    spin: Vec3,
    size: f32,
    age: f32,
}

fn setup(mut state: ResMut<SmokeState>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    state.mesh = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    state.material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.82, 0.84, 0.88, 0.55),
        emissive: LinearRgba::rgb(0.06, 0.06, 0.07),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 1.0,
        ..default()
    });
}

fn emit(
    mut commands: Commands,
    mut state: ResMut<SmokeState>,
    ship_q: Query<(&Transform, &Visibility), With<Ship>>,
    travel: Res<crate::wormhole::WormholeTravel>,
    cine: Res<crate::cinematic::Cinematic>,
) {
    let Ok((ship, vis)) = ship_q.get_single() else { return };
    let abs = ship.translation.as_dvec3() + origin();
    let len = ship.scale.x.max(0.01);
    let Some(last) = state.last else {
        state.last = Some(abs);
        return;
    };
    let moved = (abs - last).length() as f32;
    // Téléportation, voyage, séquence ou vaisseau caché : pas de fumée, on repart d'ici
    if *vis == Visibility::Hidden || travel.active() || cine.active() || moved > len * 60.0 {
        state.last = Some(abs);
        return;
    }
    let step = len * SPACING;
    if moved < step {
        return;
    }
    let back = ship.rotation * Vec3::Z * len * 0.55;
    let n = ((moved / step) as usize).min(6);
    for k in 0..n {
        if state.count >= MAX_CUBES {
            break;
        }
        let f = (k as f64 + 1.0) / n as f64;
        let at = last + (abs - last) * f + back.as_dvec3();
        let h = (at.x * 0.37 + at.y * 1.31 + at.z * 0.73).sin() as f32;
        let size = len * (0.10 + 0.06 * h.abs());
        commands.spawn((
            Mesh3d(state.mesh.clone()),
            MeshMaterial3d(state.material.clone()),
            Transform::from_translation((at - origin()).as_vec3()).with_scale(Vec3::splat(size)),
            NotShadowCaster,
            NotShadowReceiver,
            SmokeCube { abs: at, drift: Vec3::new(h, h * 0.5 + 0.6, -h) * len * 0.12, spin: Vec3::new(1.3, 0.7 + h, 0.9) * (1.0 + h), size, age: 0.0 },
        ));
        state.count += 1;
    }
    state.last = Some(abs);
}

fn animate(mut commands: Commands, time: Res<Time>, mut state: ResMut<SmokeState>, mut q: Query<(Entity, &mut SmokeCube, &mut Transform)>) {
    let dt = time.delta_secs().min(0.1);
    let o = origin();
    for (e, mut c, mut tf) in &mut q {
        c.age += dt;
        if c.age >= LIFE {
            commands.entity(e).try_despawn_recursive();
            state.count = state.count.saturating_sub(1);
            continue;
        }
        let drift = c.drift;
        c.abs += (drift * dt).as_dvec3();
        let k = 1.0 - c.age / LIFE;
        // Grossit un peu au début, puis fond
        let s = c.size * (0.6 + 0.8 * (1.0 - k)).min(1.2) * k.sqrt();
        tf.translation = (c.abs - o).as_vec3();
        tf.scale = Vec3::splat(s.max(0.0001));
        tf.rotate_local(Quat::from_scaled_axis(c.spin * dt));
    }
}
