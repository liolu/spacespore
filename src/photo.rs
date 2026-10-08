//! Mode photo (0.13.6) : F9 entre / sort (l'interface est masquée), F10 change de filtre, F11 prend la photo.
//!
//! - Filtres = étalonnage des couleurs de la caméra (`ColorGrading`) : naturel, noir et blanc, sépia, vif, froid,
//!   chaud, cinéma. Le filtre reste appliqué à la photo.
//! - Qualité maximale pendant le mode : MSAA x8 et ombres des étoiles, remis comme avant à la sortie.
//! - La photo est enregistrée en PNG sans perte (« brut ») dans `photos/` (à côté de `world.json`), à la
//!   résolution de la fenêtre, sans l'interface.

use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::render::view::{ColorGrading, ColorGradingGlobal, ColorGradingSection};

use crate::net::Net;
use crate::settings::GameSettings;

pub struct PhotoPlugin;

impl Plugin for PhotoPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Photo>().add_systems(Startup, spawn_hint).add_systems(Update, (photo_keys, apply_photo).chain());
    }
}

/// Filtres : nom, puis (exposition, température, teinte, saturation, contraste).
const FILTERS: [(&str, f32, f32, f32, f32, f32); 7] = [
    ("naturel", 0.0, 0.0, 0.0, 1.0, 1.0),
    ("noir et blanc", 0.1, 0.0, 0.0, 0.0, 1.15),
    ("sepia", 0.05, 0.55, 0.12, 0.18, 1.05),
    ("vif", 0.1, 0.0, 0.0, 1.45, 1.15),
    ("froid", 0.0, -0.45, 0.0, 0.95, 1.05),
    ("chaud", 0.0, 0.45, 0.05, 1.05, 1.0),
    ("cinema", -0.15, 0.1, -0.05, 0.8, 1.25),
];

#[derive(Resource, Default)]
pub struct Photo {
    pub active: bool,
    filter: usize,
    /// Réglages remis à la sortie : MSAA, ombres, visibilité de chaque nœud d'interface racine.
    saved_msaa: Option<u32>,
    shot_at: f64,
    saved_shadows: Option<bool>,
    hidden: Vec<(Entity, Visibility)>,
    dirty: bool,
}

#[derive(Component)]
struct PhotoHint;

fn spawn_hint(mut commands: Commands) {
    commands.spawn((
        Text::new(""),
        TextFont { font_size: 16.0, ..default() },
        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.85)),
        Node { position_type: PositionType::Absolute, top: Val::Px(10.0), left: Val::Px(12.0), ..default() },
        GlobalZIndex(950),
        Visibility::Hidden,
        PhotoHint,
    ));
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn photo_keys(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut photo: ResMut<Photo>,
    mut settings: ResMut<GameSettings>,
    panel: Res<crate::net_ui::NetPanel>,
    mut net: ResMut<Net>,
    mut roots: Query<(Entity, &mut Visibility), (With<Node>, Without<Parent>, Without<PhotoHint>)>,
    mut hint: Query<(&mut Text, &mut Visibility), With<PhotoHint>>,
) {
    if panel.focus.is_some() {
        return;
    }
    let now = time.elapsed_secs_f64();
    if keys.just_pressed(KeyCode::F9) {
        photo.active = !photo.active;
        photo.dirty = true;
        if photo.active {
            // L'interface disparaît (on retient ce qui était visible)
            photo.hidden = roots.iter().map(|(e, v)| (e, *v)).collect();
            for (_, mut v) in &mut roots {
                *v = Visibility::Hidden;
            }
            photo.saved_msaa = Some(settings.msaa_samples);
            settings.msaa_samples = 8;
            photo.saved_shadows = Some(settings.shadows);
            settings.shadows = true;
        } else {
            for (e, v) in std::mem::take(&mut photo.hidden) {
                if let Ok((_, mut vis)) = roots.get_mut(e) {
                    *vis = v;
                }
            }
            if let Some(old) = photo.saved_msaa.take() {
                settings.msaa_samples = old;
            }
            if let Some(s) = photo.saved_shadows.take() {
                settings.shadows = s;
            }
            net.notify("Mode photo ferme.", now);
        }
    }
    if photo.active && keys.just_pressed(KeyCode::F10) {
        photo.filter = (photo.filter + 1) % FILTERS.len();
        photo.dirty = true;
    }
    if keys.just_pressed(KeyCode::F11) {
        let dir = crate::settings::data_dir().join("photos");
        let _ = std::fs::create_dir_all(&dir);
        let name = format!("photo-{}-{}.png", chrono::Local::now().format("%Y-%m-%d_%H-%M-%S"), FILTERS[photo.filter].0.replace(' ', "_"));
        let path = dir.join(name);
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path.clone()));
        photo.shot_at = now;
        if !photo.active {
            net.notify(&format!("Photo : {}", path.display()), now);
        }
        photo.dirty = true;
    }
    if let Ok((mut text, mut vis)) = hint.get_single_mut() {
        if photo.active {
            let line = format!("MODE PHOTO   filtre : {}   F10 filtre   F11 photo (PNG sans perte)   F9 sortir", FILTERS[photo.filter].0);
            if text.0 != line {
                text.0 = line;
            }
            // L'aide se cache au moment de la photo pour ne pas y figurer, puis dit où elle est
            *vis = if now - photo.shot_at < 0.5 { Visibility::Hidden } else { Visibility::Visible };
        } else if *vis != Visibility::Hidden {
            *vis = Visibility::Hidden;
        }
    }
}

/// Le filtre choisi, sur la caméra (naturel hors du mode photo).
fn apply_photo(mut photo: ResMut<Photo>, mut commands: Commands, cams: Query<Entity, With<Camera3d>>) {
    if !photo.dirty {
        return;
    }
    photo.dirty = false;
    let (_, exposure, temperature, tint, saturation, contrast) = if photo.active { FILTERS[photo.filter] } else { FILTERS[0] };
    let section = ColorGradingSection { saturation, contrast, ..default() };
    let grading = ColorGrading {
        global: ColorGradingGlobal { exposure, temperature, tint, ..default() },
        shadows: section,
        midtones: section,
        highlights: section,
    };
    for e in &cams {
        commands.entity(e).insert(grading.clone());
    }
}
