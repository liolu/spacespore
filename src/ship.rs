use bevy::prelude::*;
use crate::settings::GameSettings;

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

/// Poussée demandée dans l'espace (repère du vaisseau : -z = avant ; longueur 0 à 1) : le pilote
/// automatique en croisière, rien en stationnement. Les propulseurs du modèle la suivent (C3).
#[derive(Component, Default)]
pub struct ShipThrust {
    pub push: Vec3,
}

#[derive(Resource, Debug, Clone, PartialEq)]
pub enum ShipMode {
    Ship,
    Free,
}

/// Matériau du contour : couleur pleine, non éclairée, faces avant masquées.
pub fn outline_material(color: [f32; 3]) -> StandardMaterial {
    StandardMaterial {
        base_color: Color::srgb(color[0], color[1], color[2]),
        unlit: true,
        cull_mode: Some(bevy::render::render_resource::Face::Front),
        ..default()
    }
}

/// Matériau du contour du vaisseau local (couleur mise à jour depuis le panneau).
#[derive(Component)]
pub struct LocalOutline(pub Handle<StandardMaterial>);

fn spawn_ship(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    settings: Res<GameSettings>,
) {
    let sys0 = settings.systems.first();
    let sys0_center = sys0.map(|s| s.center()).unwrap_or(Vec3::ZERO);
    let orbit_dist = sys0.and_then(|s| s.planets().first()).map(|p| p.orbit_distance).unwrap_or(450.0);
    let radius = sys0.and_then(|s| s.planets().first()).map(|p| p.radius).unwrap_or(50.0);
    let start = sys0_center + Vec3::new(orbit_dist + radius * 1.6, radius * 0.2, 0.0);

    let outline = materials.add(outline_material(settings.aura_color));

    commands.spawn((
        Transform::from_translation(start),
        Visibility::default(),
        Ship,
        ShipThrust::default(),
        LocalOutline(outline.clone()),
    )).with_children(|p| {
        // Le modèle du joueur (E7), choisi dans l'éditeur ; le vaisseau par défaut sinon
        p.spawn((
            crate::models::Rig::new(crate::models::ModelKey::default_ship(), crate::models::Fit::Ship, "etat:vol"),
            Transform::IDENTITY,
            Visibility::default(),
            crate::models::ShipRig,
        ));
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
