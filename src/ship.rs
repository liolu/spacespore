use bevy::prelude::*;
use crate::settings::GameSettings;
use crate::ui::{CameraTarget, MenuState, TargetKind};

pub struct ShipPlugin;

impl Plugin for ShipPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ShipMode::Orbit)
            .add_systems(Startup, spawn_ship)
            .add_systems(Update, (
                toggle_ship_mode,
                ship_free_move,
            ));
    }
}

#[derive(Component)]
pub struct Ship;

#[derive(Resource, Debug, Clone, PartialEq)]
pub enum ShipMode {
    Orbit,
    Free,
}

fn spawn_ship(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    settings: Res<GameSettings>,
) {
    let sys0 = settings.systems.first();
    let sys0_center = sys0.map(|s| s.center()).unwrap_or(Vec3::ZERO);
    let orbit_dist = sys0.and_then(|s| s.planets.first()).map(|p| p.orbit_distance).unwrap_or(450.0);
    let radius = sys0.and_then(|s| s.planets.first()).map(|p| p.radius).unwrap_or(50.0);
    let start = sys0_center + Vec3::new(orbit_dist + radius * 1.6, radius * 0.2, 0.0);

    let body_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.55, 0.55, 0.6),
        metallic: 0.8,
        perceptual_roughness: 0.3,
        ..default()
    });
    let wing_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.4, 0.4, 0.45),
        metallic: 0.7,
        perceptual_roughness: 0.4,
        ..default()
    });
    let cockpit_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.15, 0.4, 0.7),
        metallic: 0.9,
        perceptual_roughness: 0.1,
        ..default()
    });
    let engine_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.9, 0.4, 0.1),
        emissive: bevy::color::LinearRgba::new(2.0, 0.8, 0.2, 1.0),
        ..default()
    });

    let body_mesh = meshes.add(Cuboid::new(2.0, 0.6, 5.0));
    let wing_mesh = meshes.add(Cuboid::new(3.5, 0.12, 1.8));
    let cockpit_mesh = meshes.add(Cuboid::new(0.8, 0.35, 1.0));
    let engine_mesh = meshes.add(Cuboid::new(0.6, 0.4, 0.4));

    commands.spawn((
        Transform::from_translation(start),
        Visibility::default(),
        Ship,
    )).with_children(|p| {
        p.spawn((
            Mesh3d(body_mesh),
            MeshMaterial3d(body_mat),
            Transform::default(),
        ));
        p.spawn((
            Mesh3d(wing_mesh.clone()),
            MeshMaterial3d(wing_mat.clone()),
            Transform::from_translation(Vec3::new(-2.2, 0.0, 0.4)),
        ));
        p.spawn((
            Mesh3d(wing_mesh),
            MeshMaterial3d(wing_mat),
            Transform::from_translation(Vec3::new(2.2, 0.0, 0.4)),
        ));
        p.spawn((
            Mesh3d(cockpit_mesh),
            MeshMaterial3d(cockpit_mat),
            Transform::from_translation(Vec3::new(0.0, 0.35, -1.2)),
        ));
        p.spawn((
            Mesh3d(engine_mesh.clone()),
            MeshMaterial3d(engine_mat.clone()),
            Transform::from_translation(Vec3::new(-0.5, 0.0, 2.6)),
        ));
        p.spawn((
            Mesh3d(engine_mesh),
            MeshMaterial3d(engine_mat),
            Transform::from_translation(Vec3::new(0.5, 0.0, 2.6)),
        ));
    });
}

fn toggle_ship_mode(
    keys: Res<ButtonInput<KeyCode>>,
    mut mode: ResMut<ShipMode>,
) {
    if keys.just_pressed(KeyCode::F1) {
        *mode = match *mode {
            ShipMode::Free => ShipMode::Orbit,
            ShipMode::Orbit => ShipMode::Free,
        };
    }
}

fn ship_free_move(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut mouse_motion: EventReader<bevy::input::mouse::MouseMotion>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mode: Res<ShipMode>,
    menu_state: Res<MenuState>,
    settings: Res<GameSettings>,
    mut ship_q: Query<&mut Transform, With<Ship>>,
) {
    if *mode != ShipMode::Free { mouse_motion.clear(); return; }
    if menu_state.open { mouse_motion.clear(); return; }

    let Ok(mut tf) = ship_q.get_single_mut() else { return };
    let dt = time.delta_secs();

    let sens = settings.mouse_sensitivity * 0.003;

    for ev in mouse_motion.read() {
        let yaw = -ev.delta.x * sens;
        let pitch = -ev.delta.y * sens;
        tf.rotate_y(yaw);
        tf.rotate_local_x(pitch);
    }

    let forward = tf.rotation * Vec3::NEG_Z;
    let right = tf.rotation * Vec3::X;
    let speed = 300.0 * dt;

    let mut dir = Vec3::ZERO;
    if keys.pressed(KeyCode::KeyW) { dir += forward; }
    if keys.pressed(KeyCode::KeyS) { dir -= forward; }
    if keys.pressed(KeyCode::KeyA) { dir -= right; }
    if keys.pressed(KeyCode::KeyD) { dir += right; }
    if keys.pressed(KeyCode::Space) { dir += Vec3::Y; }
    if keys.pressed(KeyCode::ShiftLeft) { dir -= Vec3::Y; }

    if dir.length_squared() > 0.0 {
        tf.translation += dir.normalize() * speed;
    }
}
