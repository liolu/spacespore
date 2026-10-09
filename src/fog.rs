//! Brouillard (0.13 C1) : bancs de brouillard volumétriques au ras du sol (`FogVolume` et
//! `VolumetricFog` de Bevy 0.15), plus épais dans les vallées, au-dessus des mers froides et des
//! marais, au matin ; ils montent avec la densité. Les rayons de soleil et les phares y dessinent
//! des faisceaux (`VolumetricLight`). Sous terre, un brouillard léger. La brume de l'horizon reste
//! `gas.rs` (en voxels, règle 14), teintée par le soleil.
//!
//! Le banc est un volume aligné sur la verticale du lieu, posé sur le sol sous le joueur et qui le
//! suit ; sa densité vient de `fog_density` (météo + relief + eau), lissée dans le temps.

#![allow(dead_code)]

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::pbr::{DistanceFog, FogFalloff, FogVolume, VolumetricFog, VolumetricLight};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::planet::VoxelType;
use crate::settings::GameSettings;
use crate::surface::{Surface, SurfaceControl};

pub struct FogPlugin;

impl Plugin for FogPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FogState>()
            .add_event::<FogCommand>()
            .add_systems(Startup, spawn_volume)
            .add_systems(Update, (fog_commands, fog_state, update_volume, fog_lights).chain().after(SurfaceControl).after(crate::water::update_water))
            .add_systems(Update, cave_fog.after(crate::gas::gas_atmosphere));
    }
}

/// `/brouillard [oui | non | auto | 0-100]` : option, ou densité forcée (tests).
#[derive(Event)]
pub struct FogCommand(pub String);

/// Densité voulue et courante du banc (0..1.4), hauteur (voxels), couleur ; `force` = tests.
#[derive(Resource, Default)]
pub struct FogState {
    pub target: f32,
    pub density: f32,
    pub height_vox: f32,
    pub force: Option<f32>,
    /// Ce qui compose la densité (tests, scanner) : matin, mer froide, marais, vallée.
    pub parts: [f32; 4],
}

#[derive(Component)]
struct FogBank;

/// Densité du banc dans son cube : pleine au sol et au centre, qui s'efface vers le haut et vers les
/// bords (le banc n'a ni plafond ni parois visibles ; il monte et s'éclaircit avec l'altitude).
fn density_texture(images: &mut Assets<Image>) -> Handle<Image> {
    let (w, h) = (32usize, 24usize);
    let mut data = Vec::with_capacity(w * h * w);
    for z in 0..w {
        for y in 0..h {
            for x in 0..w {
                let u = (x as f32 + 0.5) / w as f32 - 0.5;
                let v = (z as f32 + 0.5) / w as f32 - 0.5;
                let r = u.hypot(v) * 2.0;
                let side = 1.0 - ((r - 0.45) / 0.55).clamp(0.0, 1.0).powi(2) * (3.0 - 2.0 * ((r - 0.45) / 0.55).clamp(0.0, 1.0));
                let up = (y as f32 + 0.5) / h as f32;
                let tall = (1.0 - up).powf(1.4);
                data.push((side * tall * 255.0) as u8);
            }
        }
    }
    let mut image = Image::new(Extent3d { width: w as u32, height: h as u32, depth_or_array_layers: w as u32 }, TextureDimension::D3, data, TextureFormat::R8Unorm, RenderAssetUsages::RENDER_WORLD);
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::ClampToEdge,
        address_mode_v: ImageAddressMode::ClampToEdge,
        address_mode_w: ImageAddressMode::ClampToEdge,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        ..default()
    });
    images.add(image)
}

fn spawn_volume(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    commands.spawn((
        FogVolume { fog_color: Color::srgb(0.8, 0.84, 0.9), density_factor: 0.0, density_texture: Some(density_texture(&mut images)), absorption: 0.02, scattering: 0.4, scattering_asymmetry: 0.5, light_tint: Color::WHITE, light_intensity: 1.0, ..default() },
        Transform::default(),
        Visibility::Hidden,
        FogBank,
    ));
}

fn fog_commands(mut events: EventReader<FogCommand>, mut settings: ResMut<GameSettings>, mut state: ResMut<FogState>, mut net: ResMut<crate::net::Net>, time: Res<Time>) {
    let now = time.elapsed_secs_f64();
    for FogCommand(arg) in events.read() {
        let a = arg.trim().to_lowercase();
        match a.as_str() {
            "oui" | "on" | "1" => {
                settings.volumetric_fog = true;
                settings.save();
                net.notify("Brouillard volumetrique actif.", now);
            }
            "non" | "off" | "0" => {
                settings.volumetric_fog = false;
                settings.save();
                net.notify("Brouillard volumetrique coupe (reste la brume de l'horizon).", now);
            }
            "auto" | "" => {
                state.force = None;
                net.notify(&format!("Brouillard : selon la meteo, le relief et l'eau (option {}). /brouillard 0-100 force une densite (tests).", if settings.volumetric_fog { "active" } else { "coupee" }), now);
            }
            n => match n.trim_end_matches('%').parse::<f32>() {
                Ok(p) if (0.0..=100.0).contains(&p) => {
                    state.force = Some(p / 100.0 * 1.4);
                    net.notify(&format!("Brouillard force a {p:.0} %."), now);
                }
                _ => net.notify("/brouillard [oui | non | auto | 0-100]", now),
            },
        }
    }
}

/// Densité du brouillard (0..1,4) au point `dir` du sol, et ses quatre parts : matin (météo), mer
/// froide, marais, vallée. Le même calcul sert aux tests.
pub fn fog_density(morning: f32, temp_c: f32, kind: VoxelType, valley: f32, humid: bool) -> (f32, [f32; 4]) {
    let cold_sea = if kind.is_clear_liquid() { ((14.0 - temp_c) / 24.0).clamp(0.0, 1.0) * 0.45 } else { 0.0 };
    let swamp = if kind == VoxelType::Swamp { 0.45 } else { 0.0 };
    let valley_part = valley.clamp(0.0, 1.0) * 0.4;
    // Un voile léger sur un monde humide, jamais nul
    let base = if humid { 0.04 } else { 0.0 };
    let m = morning.clamp(0.0, 1.0);
    // Dans une vallée ou sur l'eau, le brouillard de la météo est plus épais
    let d = (m * (1.0 + 0.6 * valley.clamp(0.0, 1.0) + 0.3 * (cold_sea > 0.0) as u8 as f32) + cold_sea + swamp + valley_part * (0.3 + m) + base).clamp(0.0, 1.4);
    (d, [m, cold_sea, swamp, valley_part])
}

/// Hauteur du banc (voxels) : de 18 à ~150 selon la densité.
pub fn fog_height(density: f32) -> f32 {
    18.0 + 90.0 * density.clamp(0.0, 1.4)
}

/// Densité par unité de distance pour une visibilité de `visibility_vox` voxels.
pub fn density_for(visibility_vox: f32, voxel: f32) -> f32 {
    3.0 / (visibility_vox.max(1.0) * voxel)
}

#[allow(clippy::too_many_arguments)]
fn fog_state(
    time: Res<Time>,
    surface: Res<Surface>,
    settings: Res<GameSettings>,
    weather: Res<crate::weather::WeatherNow>,
    local: Res<crate::world_clock::LocalWeather>,
    under: Res<crate::water::Underwater>,
    mut state: ResMut<FogState>,
) {
    let dt = time.delta_secs().min(0.1);
    let env = std::env::var("SPACESPORE_TEST_FOG").ok().and_then(|v| v.parse::<f32>().ok());
    let force = state.force.or(env.map(|p| p * 1.4));
    let mut target = 0.0;
    if let (Some(t), Some(p), true) = (surface.terrain(), surface.local_point(), settings.volumetric_fog && surface.underground() < 0.5 && !under.active()) {
        let dir = p.normalize_or(Vec3::Y);
        let v = t.voxel();
        let params = &t.params;
        if !params.gaseous && params.atmosphere && params.pressure > 0.02 {
            let ground = t.ground(dir);
            // Vallée : le sol d'ici est plus bas que la moyenne d'un anneau de 150 voxels
            let east = Vec3::Y.cross(dir).normalize_or(Vec3::X);
            let north = dir.cross(east);
            let ds = 150.0 * v / params.radius;
            let ring: f32 = (0..8)
                .map(|k| {
                    let a = k as f32 * std::f32::consts::FRAC_PI_4;
                    t.ground((dir + (east * a.cos() + north * a.sin()) * ds).normalize()).top
                })
                .sum::<f32>()
                / 8.0;
            let valley = ((ring - ground.top) / (40.0 * v)).clamp(0.0, 1.0);
            let humid = params.hydro.liquid == crate::planetgen::hydrology::Liquid::Water;
            let (d, parts) = fog_density(weather.sample.fog, local.temp, ground.kind, valley, humid);
            state.parts = parts;
            target = d;
        }
    }
    if let Some(f) = force {
        if surface.active() && surface.underground() < 0.5 {
            target = f;
        }
    }
    state.target = target;
    state.density += (target - state.density) * (1.0 - (-1.2 * dt).exp());
    state.height_vox = fog_height(state.density);
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn update_volume(
    mut commands: Commands,
    surface: Res<Surface>,
    state: Res<FogState>,
    clear: Res<ClearColor>,
    ambient: Res<AmbientLight>,
    settings: Res<GameSettings>,
    mut volumes: Query<(&mut FogVolume, &mut Transform, &mut Visibility), With<FogBank>>,
    cam_q: Query<(Entity, Has<VolumetricFog>), With<Camera3d>>,
) {
    let Ok((cam, has)) = cam_q.get_single() else { return };
    let relief_shadows = crate::graphics::terrain_detail(&settings).3 && settings.shadows;
    let on = state.density > 0.01 && surface.active();
    if on && !has {
        commands.entity(cam).insert(VolumetricFog { ambient_color: Color::srgb(0.85, 0.9, 1.0), ambient_intensity: 0.9, jitter: 0.5, step_count: 64 });
    } else if !on && has {
        commands.entity(cam).remove::<VolumetricFog>();
    }
    let (Ok((mut vol, mut tf, mut vis)), Some(frame), Some(t), Some(p)) = (volumes.get_single_mut(), surface.frame(), surface.terrain(), surface.local_point()) else {
        for (_, _, mut v) in &mut volumes {
            *v = Visibility::Hidden;
        }
        return;
    };
    let v = t.voxel();
    let dir = p.normalize_or(Vec3::Y);
    let ground = t.ground(dir).top;
    let h = state.height_vox * v;
    // Au-dessus du banc (vol), on ne le montre pas : vu d'en haut, c'était une boîte posée autour du vaisseau
    let inside = p.length() - ground < h * 1.2;
    if !on || !inside {
        *vis = Visibility::Hidden;
        return;
    }
    *vis = Visibility::Inherited;
    // Le banc ne dépasse pas la portée des ombres du soleil (sinon le brouillard lointain n'est pas éclairé)
    let width = if relief_shadows { 1800.0 * v } else { 400.0 * v };
    let d = state.density.clamp(0.0, 1.4);
    // Visibilité : de 450 voxels (léger) à 25 voxels (dense)
    let visibility = 450.0 * (1.0 - (d / 1.4).clamp(0.0, 1.0)).powi(2) + 25.0;
    vol.density_factor = density_for(visibility, v) * 2.2;
    // Couleur : celle du ciel du moment, éclaircie ; sombre la nuit
    let sky = clear.0.to_linear();
    let lum = (sky.red + sky.green + sky.blue) / 3.0;
    let day = surface.daylight();
    let tint = |c: f32| (0.55 + 0.45 * c) * (0.25 + 0.75 * day);
    vol.fog_color = Color::linear_rgb(tint(0.82f32.max(sky.red)), tint(0.86f32.max(sky.green)), tint(0.92f32.max(sky.blue)));
    vol.light_intensity = 1.0 + 1.5 * d.min(1.0);
    let _ = (lum, ambient.brightness);
    let center = dir * (ground + h * 0.5);
    let world = frame.to_world(Transform { translation: center, rotation: Quat::from_rotation_arc(Vec3::Y, dir), scale: Vec3::new(width, h, width) });
    *tf = world;
}

/// Les soleils et les phares éclairent le brouillard (faisceaux).
#[allow(clippy::type_complexity)]
fn fog_lights(
    mut commands: Commands,
    state: Res<FogState>,
    surface: Res<Surface>,
    suns: Query<(Entity, Has<VolumetricLight>), With<crate::surface::SurfaceSun>>,
    mut lamps: Query<(Entity, &mut SpotLight, Has<VolumetricLight>, &Visibility), With<crate::surface::Lamp>>,
) {
    let on = state.density > 0.03 && surface.active();
    for (e, has) in &suns {
        if on && !has {
            commands.entity(e).insert(VolumetricLight);
        } else if !on && has {
            commands.entity(e).remove::<VolumetricLight>();
        }
    }
    for (e, mut spot, has, vis) in &mut lamps {
        let lit = on && *vis != Visibility::Hidden;
        if lit != spot.shadows_enabled {
            spot.shadows_enabled = lit;
        }
        if lit && !has {
            commands.entity(e).insert(VolumetricLight);
        } else if !lit && has {
            commands.entity(e).remove::<VolumetricLight>();
        }
    }
}

/// Sous terre : un brouillard léger et froid (poussière en suspension).
fn cave_fog(mut commands: Commands, surface: Res<Surface>, under: Res<crate::water::Underwater>, cam_q: Query<Entity, With<Camera3d>>) {
    let u = surface.underground();
    if u < 0.5 || under.active() {
        return;
    }
    let Ok(cam) = cam_q.get_single() else { return };
    let v = surface.terrain().map_or(1.0, |t| t.voxel());
    let color = Color::srgb(0.07, 0.08, 0.1);
    commands.entity(cam).insert(DistanceFog { color, falloff: FogFalloff::from_visibility(260.0 * v), ..default() });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fog_is_thicker_in_valleys_over_cold_seas_and_in_swamps() {
        let (flat, _) = fog_density(0.3, 15.0, VoxelType::Grass, 0.0, true);
        let (valley, _) = fog_density(0.3, 15.0, VoxelType::Grass, 1.0, true);
        let (sea_cold, _) = fog_density(0.3, 2.0, VoxelType::Water, 0.0, true);
        let (sea_warm, _) = fog_density(0.3, 28.0, VoxelType::Water, 0.0, true);
        let (swamp, _) = fog_density(0.3, 15.0, VoxelType::Swamp, 0.0, true);
        assert!(valley > flat && sea_cold > sea_warm && swamp > flat, "{flat} {valley} {sea_cold} {sea_warm} {swamp}");
    }

    #[test]
    fn a_dry_clear_world_has_no_fog() {
        let (d, _) = fog_density(0.0, 20.0, VoxelType::Sand, 0.0, false);
        assert_eq!(d, 0.0);
        assert!(fog_density(2.0, -50.0, VoxelType::Water, 1.0, true).0 <= 1.4);
    }

    #[test]
    fn the_bank_rises_and_thickens_together() {
        assert!(fog_height(1.0) > fog_height(0.1));
        assert!(density_for(30.0, 0.5) > density_for(300.0, 0.5));
    }
}
