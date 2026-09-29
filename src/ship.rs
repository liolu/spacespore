use bevy::prelude::*;
use crate::settings::GameSettings;
use crate::ui::{CameraTarget, MenuState, TargetKind};

pub struct ShipPlugin;

impl Plugin for ShipPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ShipMode::Ship)
            .add_systems(Startup, spawn_ship)
            .add_systems(Update, toggle_ship_mode);
    }
}

#[derive(Component)]
pub struct Ship;

#[derive(Resource, Debug, Clone, PartialEq)]
pub enum ShipMode {
    Ship,
    Free,
}

/// Meshes et matériaux partagés entre le vaisseau local et ceux des autres joueurs.
#[derive(Resource, Clone)]
pub struct ShipAssets {
    body_mesh: Handle<Mesh>,
    wing_mesh: Handle<Mesh>,
    cockpit_mesh: Handle<Mesh>,
    engine_mesh: Handle<Mesh>,
    body_mat: Handle<StandardMaterial>,
    wing_mat: Handle<StandardMaterial>,
    cockpit_mat: Handle<StandardMaterial>,
    engine_mat: Handle<StandardMaterial>,
    pub aura_mesh: Handle<Mesh>,
}

impl ShipAssets {
    fn new(meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) -> Self {
        Self {
            body_mesh: meshes.add(Cuboid::new(2.0, 0.6, 5.0)),
            wing_mesh: meshes.add(Cuboid::new(3.5, 0.12, 1.8)),
            cockpit_mesh: meshes.add(Cuboid::new(0.8, 0.35, 1.0)),
            engine_mesh: meshes.add(Cuboid::new(0.6, 0.4, 0.4)),
            body_mat: materials.add(StandardMaterial {
                base_color: Color::srgb(0.55, 0.55, 0.6),
                metallic: 0.8,
                perceptual_roughness: 0.3,
                ..default()
            }),
            wing_mat: materials.add(StandardMaterial {
                base_color: Color::srgb(0.4, 0.4, 0.45),
                metallic: 0.7,
                perceptual_roughness: 0.4,
                ..default()
            }),
            cockpit_mat: materials.add(StandardMaterial {
                base_color: Color::srgb(0.15, 0.4, 0.7),
                metallic: 0.9,
                perceptual_roughness: 0.1,
                ..default()
            }),
            engine_mat: materials.add(StandardMaterial {
                base_color: Color::srgb(0.9, 0.4, 0.1),
                emissive: bevy::color::LinearRgba::new(2.0, 0.8, 0.2, 1.0),
                ..default()
            }),
            aura_mesh: meshes.add(Sphere::new(1.0).mesh().ico(3).unwrap()),
        }
    }

    /// Ajoute la coque du vaisseau (sans lumière ni aura) comme enfants de `p`.
    pub fn spawn_model(&self, p: &mut ChildBuilder) {
        p.spawn((
            Mesh3d(self.body_mesh.clone()),
            MeshMaterial3d(self.body_mat.clone()),
            Transform::default(),
        ));
        for x in [-2.2, 2.2] {
            p.spawn((
                Mesh3d(self.wing_mesh.clone()),
                MeshMaterial3d(self.wing_mat.clone()),
                Transform::from_translation(Vec3::new(x, 0.0, 0.4)),
            ));
        }
        p.spawn((
            Mesh3d(self.cockpit_mesh.clone()),
            MeshMaterial3d(self.cockpit_mat.clone()),
            Transform::from_translation(Vec3::new(0.0, 0.35, -1.2)),
        ));
        for x in [-0.5, 0.5] {
            p.spawn((
                Mesh3d(self.engine_mesh.clone()),
                MeshMaterial3d(self.engine_mat.clone()),
                Transform::from_translation(Vec3::new(x, 0.0, 2.6)),
            ));
        }
    }

    /// Ajoute l'aura colorée (deux sphères additives) comme enfants de `p`.
    pub fn spawn_aura(&self, p: &mut ChildBuilder, inner: Handle<StandardMaterial>, outer: Handle<StandardMaterial>) {
        p.spawn((
            Mesh3d(self.aura_mesh.clone()),
            MeshMaterial3d(inner),
            Transform::from_scale(Vec3::splat(4.0)),
            bevy::pbr::NotShadowCaster,
        ));
        p.spawn((
            Mesh3d(self.aura_mesh.clone()),
            MeshMaterial3d(outer),
            Transform::from_scale(Vec3::splat(6.5)),
            bevy::pbr::NotShadowCaster,
        ));
    }
}

/// Matériaux de l'aura (intérieur plus dense, extérieur plus diffus).
pub fn aura_materials(color: [f32; 3]) -> (StandardMaterial, StandardMaterial) {
    let make = |alpha: f32| StandardMaterial {
        base_color: Color::srgba(color[0], color[1], color[2], alpha),
        unlit: true,
        alpha_mode: AlphaMode::Add,
        cull_mode: None,
        ..default()
    };
    (make(0.22), make(0.08))
}

/// Marqueur des matériaux d'aura du vaisseau local (couleur mise à jour depuis les options).
#[derive(Component)]
pub struct LocalAura {
    pub inner: Handle<StandardMaterial>,
    pub outer: Handle<StandardMaterial>,
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

    let assets = ShipAssets::new(&mut meshes, &mut materials);
    let (inner, outer) = aura_materials(settings.aura_color);
    let inner = materials.add(inner);
    let outer = materials.add(outer);

    commands.spawn((
        Transform::from_translation(start),
        Visibility::default(),
        Ship,
        LocalAura { inner: inner.clone(), outer: outer.clone() },
    )).with_children(|p| {
        assets.spawn_model(p);
        assets.spawn_aura(p, inner, outer);
        p.spawn((
            PointLight {
                intensity: 800_000.0,
                range: 150.0,
                color: Color::srgb(0.6, 0.8, 1.0),
                shadows_enabled: false,
                ..default()
            },
            Transform::from_translation(Vec3::new(0.0, 1.0, 0.0)),
        ));
    });

    commands.insert_resource(assets);
}

fn toggle_ship_mode(
    keys: Res<ButtonInput<KeyCode>>,
    mut mode: ResMut<ShipMode>,
) {
    if keys.just_pressed(KeyCode::F1) {
        *mode = match *mode {
            ShipMode::Ship => ShipMode::Free,
            ShipMode::Free => ShipMode::Ship,
        };
    }
}
