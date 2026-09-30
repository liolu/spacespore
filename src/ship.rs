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

#[derive(Resource, Debug, Clone, PartialEq)]
pub enum ShipMode {
    Ship,
    Free,
}

/// Épaisseur du contour coloré autour du vaisseau (unités, avant mise à l'échelle).
const OUTLINE_WIDTH: f32 = 0.18;

/// Pièces du vaisseau : (dimensions, position, matériau).
const SHIP_PARTS: [([f32; 3], [f32; 3], usize); 6] = [
    ([2.0, 0.6, 5.0], [0.0, 0.0, 0.0], 0),    // coque
    ([3.5, 0.12, 1.8], [-2.2, 0.0, 0.4], 1),  // aile gauche
    ([3.5, 0.12, 1.8], [2.2, 0.0, 0.4], 1),   // aile droite
    ([0.8, 0.35, 1.0], [0.0, 0.35, -1.2], 2), // cockpit
    ([0.6, 0.4, 0.4], [-0.5, 0.0, 2.6], 3),   // réacteur gauche
    ([0.6, 0.4, 0.4], [0.5, 0.0, 2.6], 3),    // réacteur droit
];

/// Meshes et matériaux partagés entre le vaisseau local et ceux des autres joueurs.
#[derive(Resource, Clone)]
pub struct ShipAssets {
    /// Par pièce : (mesh, mesh du contour, matériau, position)
    parts: Vec<(Handle<Mesh>, Handle<Mesh>, Handle<StandardMaterial>, Vec3)>,
}

impl ShipAssets {
    fn new(meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) -> Self {
        let mats = [
            materials.add(StandardMaterial {
                base_color: Color::srgb(0.55, 0.55, 0.6),
                metallic: 0.8,
                perceptual_roughness: 0.3,
                ..default()
            }),
            materials.add(StandardMaterial {
                base_color: Color::srgb(0.4, 0.4, 0.45),
                metallic: 0.7,
                perceptual_roughness: 0.4,
                ..default()
            }),
            materials.add(StandardMaterial {
                base_color: Color::srgb(0.15, 0.4, 0.7),
                metallic: 0.9,
                perceptual_roughness: 0.1,
                ..default()
            }),
            materials.add(StandardMaterial {
                base_color: Color::srgb(0.9, 0.4, 0.1),
                emissive: bevy::color::LinearRgba::new(2.0, 0.8, 0.2, 1.0),
                ..default()
            }),
        ];
        let parts = SHIP_PARTS
            .iter()
            .map(|(size, pos, mat)| {
                let size = Vec3::from_array(*size);
                (
                    meshes.add(Cuboid::from_size(size)),
                    meshes.add(Cuboid::from_size(size + Vec3::splat(OUTLINE_WIDTH * 2.0))),
                    mats[*mat].clone(),
                    Vec3::from_array(*pos),
                )
            })
            .collect();
        Self { parts }
    }

    /// Ajoute la coque du vaisseau (sans lumière ni contour) comme enfants de `p`.
    pub fn spawn_model(&self, p: &mut ChildBuilder) {
        for (mesh, _, mat, pos) in &self.parts {
            p.spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(mat.clone()),
                Transform::from_translation(*pos),
            ));
        }
    }

    /// Ajoute le contour coloré : chaque pièce est dupliquée un peu plus grande
    /// et seules ses faces arrière sont dessinées (« coque inversée »), ce qui
    /// laisse apparaître un liseré de couleur tout autour de la silhouette.
    pub fn spawn_outline(&self, p: &mut ChildBuilder, material: Handle<StandardMaterial>) {
        for (_, outline_mesh, _, pos) in &self.parts {
            p.spawn((
                Mesh3d(outline_mesh.clone()),
                MeshMaterial3d(material.clone()),
                Transform::from_translation(*pos),
                bevy::pbr::NotShadowCaster,
            ));
        }
    }
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
    let outline = materials.add(outline_material(settings.aura_color));

    commands.spawn((
        Transform::from_translation(start),
        Visibility::default(),
        Ship,
        LocalOutline(outline.clone()),
    )).with_children(|p| {
        assets.spawn_model(p);
        assets.spawn_outline(p, outline);
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
