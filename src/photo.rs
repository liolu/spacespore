//! Mode photo (0.13.6) : F9 entre / sort (l'interface est masquée), F10 change de filtre, F11 prend la photo.
//!
//! - Filtres = étalonnage des couleurs de la caméra (`ColorGrading`) : naturel, noir et blanc, sépia, vif, froid,
//!   chaud, cinéma. Le filtre reste appliqué à la photo.
//! - Qualité maximale pendant le mode : MSAA x8 et ombres des étoiles, remis comme avant à la sortie.
//! - La photo est enregistrée en PNG sans perte (« brut ») dans `photos/` (à côté de `world.json`), à la
//!   résolution de la fenêtre.
//! - Il ne reste que les astres : interface (y compris ce qui apparaît pendant le mode), tous les gizmos
//!   (orbites, cercles, indicateurs, traînées, portées), vaisseau et ses effets, autres joueurs, personnages,
//!   fumée et vitre du cockpit sont cachés à chaque image (`hide_for_photo`), puis remis comme avant.

use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::render::view::{ColorGrading, ColorGradingGlobal, ColorGradingSection};

use crate::net::Net;
use crate::settings::GameSettings;

pub struct PhotoPlugin;

impl Plugin for PhotoPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Photo>()
            .add_systems(Startup, spawn_hint)
            .add_systems(Update, (photo_keys, apply_photo).chain())
            .add_systems(PostUpdate, hide_for_photo.before(bevy::render::view::VisibilitySystems::VisibilityPropagate));
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
    /// Visibilité d'avant de chaque entité cachée, et gizmos actifs avant le mode.
    hidden: std::collections::HashMap<Entity, Visibility>,
    gizmos: Vec<(std::any::TypeId, bool)>,
    restore: bool,
    /// L'aide ne se montre que quelques secondes (entrée, changement de filtre).
    hint_at: f64,
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
    mut hint: Query<(&mut Text, &mut Visibility), With<PhotoHint>>,
) {
    if panel.focus.is_some() {
        return;
    }
    let now = time.elapsed_secs_f64();
    // Test : `SPACESPORE_TEST_PHOTO=<s>` entre en mode photo à `<s>` s
    let test = std::env::var("SPACESPORE_TEST_PHOTO").ok().and_then(|v| v.parse::<f64>().ok()).is_some_and(|s| now >= s && now - time.delta_secs_f64() < s);
    if keys.just_pressed(KeyCode::F9) || test {
        photo.active = !photo.active;
        photo.dirty = true;
        photo.hint_at = now;
        if photo.active {
            photo.saved_msaa = Some(settings.msaa_samples);
            settings.msaa_samples = 8;
            photo.saved_shadows = Some(settings.shadows);
            settings.shadows = true;
        } else {
            photo.restore = true;
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
        photo.hint_at = now;
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
            // Seulement 3 s après l'entrée ou un changement de filtre, jamais sur la photo
            *vis = if now - photo.shot_at > 0.5 && now - photo.hint_at < 3.0 { Visibility::Visible } else { Visibility::Hidden };
        } else if *vis != Visibility::Hidden {
            *vis = Visibility::Hidden;
        }
    }
}

/// Tout ce qui n'est pas un astre disparaît pendant le mode photo (à chaque image : ce qui apparaît ou que
/// d'autres systèmes remontrent est recaché), puis tout est remis comme avant.
#[allow(clippy::type_complexity)]
fn hide_for_photo(
    mut photo: ResMut<Photo>,
    mut store: ResMut<GizmoConfigStore>,
    mut q: Query<
        (Entity, &mut Visibility),
        (
            Without<PhotoHint>,
            Or<(
                (With<Node>, Without<Parent>),
                With<crate::ship::Ship>,
                With<crate::net::RemoteShip>,
                With<crate::models::LocalWalker>,
                With<crate::models::RemoteWalker>,
                With<crate::smoke::SmokeCube>,
                With<crate::approche_ui::Glass>,
                With<crate::approche_fx::BodyFx>,
                With<crate::approche_fx::RemoteFx>,
            )>,
        ),
    >,
) {
    let photo = &mut *photo;
    if photo.restore {
        photo.restore = false;
        for (e, v) in photo.hidden.drain() {
            if let Ok((_, mut vis)) = q.get_mut(e) {
                *vis = v;
            }
        }
        for (id, on) in photo.gizmos.drain(..) {
            for (tid, config, _) in store.iter_mut() {
                if *tid == id {
                    config.enabled = on;
                }
            }
        }
        return;
    }
    if !photo.active {
        return;
    }
    if photo.gizmos.is_empty() {
        for (tid, config, _) in store.iter_mut() {
            photo.gizmos.push((*tid, config.enabled));
        }
    }
    for (_, config, _) in store.iter_mut() {
        config.enabled = false;
    }
    for (e, mut vis) in &mut q {
        if *vis != Visibility::Hidden {
            photo.hidden.entry(e).or_insert(*vis);
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
