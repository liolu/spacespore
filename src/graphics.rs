use bevy::prelude::*;
use bevy::render::camera::RenderTarget;
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy::window::{PresentMode, PrimaryWindow, WindowRef};
use std::time::{Duration, Instant};

use crate::settings::GameSettings;

// ─────────────────────────────────────────────────────────────────────────
//  Options graphiques / performances
//
//  Applique les réglages de GameSettings au moteur : VSync, limite FPS,
//  MSAA, ombres, détail des planètes (biais LOD).
//  Les nuages et éruptions sont gérés directement dans planet.rs.
// ─────────────────────────────────────────────────────────────────────────

pub const MSAA_CHOICES: [u32; 4] = [1, 2, 4, 8];
pub const FPS_LIMIT_CHOICES: [u32; 8] = [0, 30, 60, 90, 120, 144, 165, 240];
pub const LOD_QUALITY_CHOICES: [f32; 4] = [0.5, 0.75, 1.0, 1.5];
pub const RENDER_SCALE_CHOICES: [f32; 5] = [0.5, 0.67, 0.75, 0.85, 1.0];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum QualityPreset {
    Low,
    Medium,
    High,
    Ultra,
}

impl QualityPreset {
    pub const ALL: [QualityPreset; 4] = [Self::Low, Self::Medium, Self::High, Self::Ultra];

    pub fn label(self) -> &'static str {
        match self {
            Self::Low => "Bas",
            Self::Medium => "Moyen",
            Self::High => "Haut",
            Self::Ultra => "Ultra",
        }
    }

    /// (msaa, ombres, détail planètes, nuages, éruptions)
    fn values(self) -> (u32, bool, f32, bool, bool) {
        match self {
            Self::Low => (1, false, 0.5, false, false),
            Self::Medium => (2, false, 0.75, true, true),
            Self::High => (4, true, 1.0, true, true),
            Self::Ultra => (8, true, 1.5, true, true),
        }
    }

    pub fn apply(self, s: &mut GameSettings) {
        let (msaa, shadows, lod, clouds, flares) = self.values();
        s.msaa_samples = msaa;
        s.shadows = shadows;
        s.lod_quality = lod;
        s.show_clouds = clouds;
        s.show_flares = flares;
    }

    /// Préréglage correspondant aux réglages actuels, `None` = personnalisé.
    pub fn detect(s: &GameSettings) -> Option<Self> {
        Self::ALL.into_iter().find(|p| {
            let (msaa, shadows, lod, clouds, flares) = p.values();
            s.msaa_samples == msaa
                && s.shadows == shadows
                && (s.lod_quality - lod).abs() < 0.01
                && s.show_clouds == clouds
                && s.show_flares == flares
        })
    }
}

pub fn lod_quality_label(q: f32) -> &'static str {
    if q < 0.6 { "Bas" } else if q < 0.9 { "Moyen" } else if q < 1.2 { "Haut" } else { "Ultra" }
}

pub fn fps_limit_label(limit: u32) -> String {
    if limit == 0 { "Illimite".into() } else { format!("{limit}") }
}

pub fn render_scale_label(scale: f32) -> String {
    format!("{:.0}%", scale * 100.0)
}

pub fn msaa_label(samples: u32) -> String {
    if samples <= 1 { "Off".into() } else { format!("{samples}x") }
}

/// Valeur suivante dans une liste de choix (boucle en fin de liste).
pub fn next_choice<T: PartialEq + Copy>(choices: &[T], current: T) -> T {
    let i = choices.iter().position(|c| *c == current).map(|i| i + 1).unwrap_or(0);
    choices[i % choices.len()]
}

/// Conversion entre coordonnées fenêtre (logiques) et coordonnées du viewport
/// de la caméra 3D. Vaut 1 sans échelle de rendu ; sinon la caméra rend dans
/// une image plus petite et ses coordonnées écran sont multipliées par ce facteur.
#[derive(Resource)]
pub struct ViewportScale(pub f32);

impl ViewportScale {
    /// Position curseur (fenêtre) → viewport de la caméra 3D.
    pub fn to_viewport(&self, p: Vec2) -> Vec2 {
        p * self.0
    }

    /// Position viewport de la caméra 3D → fenêtre (pour placer de l'UI).
    pub fn to_window(&self, p: Vec2) -> Vec2 {
        p / self.0
    }
}

/// Rendu à résolution réduite : image cible + caméra d'affichage plein écran.
#[derive(Resource, Default)]
struct ScaledRender {
    image: Option<Handle<Image>>,
    blit_camera: Option<Entity>,
    blit_node: Option<Entity>,
}

/// Lumière créée avec des ombres : on peut les couper/rétablir selon le réglage.
#[derive(Component)]
struct ShadowCapable;

pub struct GraphicsPlugin;

impl Plugin for GraphicsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ViewportScale(1.0))
            .init_resource::<ScaledRender>()
            .add_systems(
                Update,
                (apply_present_mode, apply_msaa, apply_shadows, apply_lod_quality, apply_render_scale),
            )
        .add_systems(Last, limit_fps);
    }
}

fn apply_present_mode(
    settings: Res<GameSettings>,
    mut window_q: Query<&mut Window, With<PrimaryWindow>>,
) {
    let wanted = if settings.vsync { PresentMode::AutoVsync } else { PresentMode::AutoNoVsync };
    for mut w in &mut window_q {
        if w.present_mode != wanted {
            w.present_mode = wanted;
        }
    }
}

fn apply_msaa(
    mut commands: Commands,
    settings: Res<GameSettings>,
    cam_q: Query<(Entity, Option<&Msaa>), With<Camera3d>>,
) {
    let wanted = match settings.msaa_samples {
        0 | 1 => Msaa::Off,
        2 => Msaa::Sample2,
        8 => Msaa::Sample8,
        _ => Msaa::Sample4,
    };
    for (e, msaa) in &cam_q {
        if msaa != Some(&wanted) {
            commands.entity(e).insert(wanted);
        }
    }
}

fn apply_shadows(
    mut commands: Commands,
    settings: Res<GameSettings>,
    mut lights: Query<(Entity, &mut PointLight, Has<ShadowCapable>)>,
) {
    let changed = settings.is_changed();
    for (e, mut light, capable) in &mut lights {
        if light.is_added() {
            if light.shadows_enabled {
                commands.entity(e).insert(ShadowCapable);
                if !settings.shadows {
                    light.shadows_enabled = false;
                }
            }
        } else if capable && changed && light.shadows_enabled != settings.shadows {
            light.shadows_enabled = settings.shadows;
        }
    }
}

fn apply_lod_quality(settings: Res<GameSettings>) {
    if settings.is_changed() {
        crate::lod::set_lod_quality(settings.lod_quality);
    }
}

/// Échelle de rendu : sous 100 %, la caméra 3D rend dans une image réduite,
/// affichée étirée en fond d'écran ; l'interface reste à pleine résolution.
fn apply_render_scale(
    mut commands: Commands,
    settings: Res<GameSettings>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    mut cam_q: Query<&mut Camera, With<Camera3d>>,
    mut images: ResMut<Assets<Image>>,
    mut state: ResMut<ScaledRender>,
    mut viewport: ResMut<ViewportScale>,
) {
    let Ok(window) = window_q.get_single() else { return };
    let Ok(mut cam) = cam_q.get_single_mut() else { return };
    let scale = settings.render_scale.clamp(0.25, 1.0);

    if scale >= 0.999 {
        if state.image.take().is_some() {
            cam.target = RenderTarget::Window(WindowRef::Primary);
            for e in [state.blit_camera.take(), state.blit_node.take()].into_iter().flatten() {
                commands.entity(e).despawn_recursive();
            }
        }
        if viewport.0 != 1.0 {
            viewport.0 = 1.0;
        }
        return;
    }

    let size = Extent3d {
        width: ((window.physical_width() as f32 * scale).round() as u32).max(1),
        height: ((window.physical_height() as f32 * scale).round() as u32).max(1),
        depth_or_array_layers: 1,
    };

    match &state.image {
        Some(handle) => {
            // Fenêtre redimensionnée ou échelle modifiée
            if images.get(handle).is_some_and(|img| img.texture_descriptor.size != size) {
                if let Some(img) = images.get_mut(handle) {
                    img.resize(size);
                }
            }
        }
        None => {
            let mut image = Image::new_fill(
                size,
                TextureDimension::D2,
                &[0, 0, 0, 255],
                TextureFormat::Bgra8UnormSrgb,
                RenderAssetUsages::default(),
            );
            image.texture_descriptor.usage =
                TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
            let handle = images.add(image);
            cam.target = RenderTarget::Image(handle.clone());

            // Caméra d'affichage : devient la caméra UI par défaut (ordre le plus élevé)
            let blit_camera = commands
                .spawn((
                    Camera2d,
                    Camera {
                        order: 1,
                        clear_color: ClearColorConfig::Custom(Color::BLACK),
                        ..default()
                    },
                ))
                .id();
            let blit_node = commands
                .spawn((
                    ImageNode::new(handle.clone()),
                    Node {
                        position_type: PositionType::Absolute,
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    GlobalZIndex(i32::MIN),
                    TargetCamera(blit_camera),
                ))
                .id();

            state.image = Some(handle);
            state.blit_camera = Some(blit_camera);
            state.blit_node = Some(blit_node);
        }
    }

    let factor = size.width as f32 / window.width().max(1.0);
    if (viewport.0 - factor).abs() > f32::EPSILON {
        viewport.0 = factor;
    }
}

/// Limiteur d'images : attend en fin de frame pour ne pas dépasser la cible.
fn limit_fps(settings: Res<GameSettings>, mut last: Local<Option<Instant>>) {
    let limit = settings.fps_limit;
    if limit == 0 {
        *last = None;
        return;
    }
    let frame = Duration::from_secs_f64(1.0 / limit as f64);
    if let Some(prev) = *last {
        let target = prev + frame;
        // Sommeil jusqu'à ~1 ms avant la cible, puis attente active pour la précision
        let now = Instant::now();
        if target > now + Duration::from_millis(1) {
            std::thread::sleep(target - now - Duration::from_millis(1));
        }
        while Instant::now() < target {
            std::hint::spin_loop();
        }
        // Si on a pris du retard, on repart de maintenant plutôt que de rattraper
        *last = Some(if Instant::now() > target + frame { Instant::now() } else { target });
    } else {
        *last = Some(Instant::now());
    }
}
