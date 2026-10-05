//! Eau (0.13 O1 / O2) : la surface des mers et des lacs est un maillage à part (par tuile, au niveau
//! de la mer, marées comprises) avec un matériau transparent (`water.wgsl`) : couleur et opacité
//! selon la profondeur (le fond marin reste visible près du bord), Fresnel, reflet du ciel et du
//! soleil, vagues selon le vent de `weather.rs`, écume. Sous l'eau : brouillard coloré, lumière
//! atténuée avec la profondeur (`Underwater`), caustiques sur le fond (`caustics.wgsl`).

#![allow(dead_code)]

use bevy::asset::load_internal_asset;
use bevy::pbr::{DistanceFog, ExtendedMaterial, FogFalloff, MaterialExtension, MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::mesh::MeshVertexBufferLayoutRef;
use bevy::render::render_resource::{AsBindGroup, RenderPipelineDescriptor, ShaderRef, ShaderType, SpecializedMeshPipelineError};

use crate::planet::{MoonId, MoonRoot, PlanetId, PlanetRoot, VoxelType};
use crate::ui::TargetKind;
use crate::surface::{Surface, SurfaceSun};

pub const WATER_SHADER: Handle<Shader> = Handle::weak_from_u128(0x5ea_f00d_0a7e_c0de_1234_5678_9abc);
pub const CAUSTICS_SHADER: Handle<Shader> = Handle::weak_from_u128(0x5ea_f00d_0a7e_c0de_1234_5678_9abd);

/// Matériau du sol des tuiles : le matériau standard et la lumière du fond marin (`caustics.wgsl`).
pub type TerrainMaterial = ExtendedMaterial<StandardMaterial, Caustics>;

#[allow(dead_code)]
#[derive(ShaderType, Reflect, Clone, Copy, Debug, Default)]
pub struct CausticParams {
    pub to0: Vec4,
    pub to1: Vec4,
    pub to2: Vec4,
    pub sea: Vec4,
    pub sun: Vec4,
    pub absorb: Vec4,
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct Caustics {
    #[uniform(100)]
    pub params: CausticParams,
}

impl MaterialExtension for Caustics {
    fn fragment_shader() -> ShaderRef {
        CAUSTICS_SHADER.into()
    }
}

/// Nombres d'onde des 5 vagues (colonnes par période de `terrain::WAVE_PERIOD`), les mêmes que dans
/// `water.wgsl`.
const WAVES: [(f32, f32); 5] = [(96.0, 0.0), (-70.0, 150.0), (190.0, 90.0), (-60.0, -310.0), (410.0, -120.0)];

#[allow(dead_code)]
#[derive(ShaderType, Clone, Copy, Debug, Default)]
pub struct WaterParams {
    pub sun: Vec4,
    pub sky: Vec4,
    pub cam: Vec4,
    pub time: Vec4,
    pub w1: Vec4,
    pub w2: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug, Default)]
pub struct WaterMaterial {
    #[uniform(0)]
    pub params: WaterParams,
}

impl Material for WaterMaterial {
    fn vertex_shader() -> ShaderRef {
        WATER_SHADER.into()
    }

    fn fragment_shader() -> ShaderRef {
        WATER_SHADER.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    // Vue de dessous aussi
    fn specialize(
        _pipeline: &MaterialPipeline<Self>,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

/// Le matériau d'eau, partagé par toutes les tuiles (ses paramètres changent à chaque image).
#[derive(Resource)]
pub struct WaterAssets {
    pub material: Handle<WaterMaterial>,
}

/// Marque la surface d'eau d'une tuile.
#[derive(Component)]
pub struct WaterSurface;

/// La caméra est sous l'eau : profondeur (unités), liquide et teinte.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct Underwater {
    pub depth: f32,
    pub kind: Option<VoxelType>,
    /// Lumière qui arrive à cette profondeur (0..1).
    pub light: f32,
}

impl Underwater {
    pub fn active(&self) -> bool {
        self.kind.is_some()
    }
}

pub struct WaterPlugin;

impl Plugin for WaterPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, WATER_SHADER, "water.wgsl", Shader::from_wgsl);
        load_internal_asset!(app, CAUSTICS_SHADER, "caustics.wgsl", Shader::from_wgsl);
        app.add_plugins(MaterialPlugin::<WaterMaterial>::default());
        app.add_plugins(MaterialPlugin::<TerrainMaterial>::default());
        let material = app.world_mut().resource_mut::<Assets<WaterMaterial>>().add(WaterMaterial::default());
        app.insert_resource(WaterAssets { material })
            .init_resource::<Underwater>()
            .add_systems(Update, underwater_fog.after(crate::gas::gas_atmosphere).after(update_water));
    }
}

/// Visibilité sous l'eau (colonnes) selon le liquide : l'eau est claire, le méthane et
/// l'ammoniac troubles.
fn clarity(kind: VoxelType) -> f32 {
    match kind {
        VoxelType::Water => 60.0,
        VoxelType::Ammonia => 28.0,
        _ => 12.0,
    }
}

/// Poids des cinq vagues d'après le vent : toutes avancent un peu avec lui, celles qui lui font
/// face le plus (le signe donne le sens de marche).
fn wave_weights(dir: Vec3, wind_dir: Vec3) -> [f32; 5] {
    let (face, s, t) = crate::terrain::dir_to_face(dir);
    let eps = 1e-3;
    let e_u = (crate::terrain::face_dir(face, s + eps, t) - crate::terrain::face_dir(face, s, t)).normalize_or(Vec3::X);
    let e_v = (crate::terrain::face_dir(face, s, t + eps) - crate::terrain::face_dir(face, s, t)).normalize_or(Vec3::Z);
    WAVES.map(|(mi, mj)| {
        let d = (e_u * mi + e_v * mj).normalize_or(Vec3::X);
        let c = d.dot(wind_dir);
        let c = if wind_dir == Vec3::ZERO { 0.5 } else { c };
        let sign = if c < 0.0 { -1.0 } else { 1.0 };
        sign * (0.35 + 0.65 * c.abs())
    })
}

/// Met à jour le matériau d'eau (soleil, ciel, caméra, vent, temps) et, sous l'eau, atténue la
/// lumière avec la profondeur.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn update_water(
    time: Res<Time>,
    surface: Res<Surface>,
    clear: Res<ClearColor>,
    weather: Res<crate::weather::WeatherNow>,
    assets: Res<WaterAssets>,
    mut materials: ResMut<Assets<WaterMaterial>>,
    mut terrain_mats: ResMut<Assets<TerrainMaterial>>,
    mut under: ResMut<Underwater>,
    mut ambient: ResMut<AmbientLight>,
    cam_q: Query<&Transform, (With<Camera3d>, Without<PlanetRoot>, Without<MoonRoot>)>,
    planets: Query<(&PlanetId, &Transform), (With<PlanetRoot>, Without<Camera3d>)>,
    moons: Query<(&MoonId, &Transform), (With<MoonRoot>, Without<Camera3d>)>,
    mut suns: Query<(&SurfaceSun, &mut DirectionalLight, &Transform), Without<Camera3d>>,
) {
    let (Some(kind), Some(terrain), Ok(cam)) = (surface.body(), surface.terrain(), cam_q.get_single()) else {
        if under.kind.is_some() {
            *under = Underwater::default();
        }
        return;
    };
    if !surface.active() {
        if under.kind.is_some() {
            *under = Underwater::default();
        }
        return;
    }
    let root = match kind {
        TargetKind::Planet(id) => planets.iter().find(|(p, _)| p.0 == id).map(|(_, t)| *t),
        TargetKind::Moon(pi, mi) => moons.iter().find(|(m, _)| m.planet_idx == pi && m.moon_idx == mi).map(|(_, t)| *t),
        _ => None,
    };
    let Some(root) = root else { return };
    let to_local = root.rotation.inverse();
    let cam_local = to_local * (cam.translation - root.translation);
    let dir = cam_local.normalize_or(Vec3::Y);
    let voxel = terrain.voxel();

    // Soleil le plus brillant (repère de l'astre)
    let mut sun_dir = Vec3::Y;
    let mut best = 0.0;
    for (_, light, tf) in &suns {
        if light.illuminance > best {
            best = light.illuminance;
            sun_dir = to_local * (-*tf.forward());
        }
    }
    let day = (surface.daylight() * weather.light()).clamp(0.0, 1.0);
    let sky = clear.0.to_linear();
    let night = (ambient.brightness / 1_500.0).clamp(0.0, 1.0);

    // Vent : direction (plan tangent) et force
    let east = Vec3::Y.cross(dir).normalize_or(Vec3::X);
    let north = dir.cross(east);
    let s = &weather.sample;
    let wind = east * s.east + north * s.north;
    let speed = s.wind_speed();
    let wind_dir = if speed > 0.3 { wind / speed } else { Vec3::ZERO };
    let amp = (0.04 + 0.03 * speed + 0.4 * s.storm).min(2.0);
    let foam = ((speed - 8.0) / 12.0).clamp(0.0, 1.0).max(s.storm * 0.7);
    let w = wave_weights(dir, wind_dir);
    let gravity = 9.8 * terrain.params.gravity.clamp(0.05, 4.0);

    if let Some(m) = materials.get_mut(&assets.material) {
        let p = &mut m.params;
        p.sun = sun_dir.extend(day);
        p.sky = Vec4::new(sky.red, sky.green, sky.blue, (1.0 - day) * night);
        p.cam = cam_local.extend(voxel);
        p.time = Vec4::new(time.elapsed_secs_wrapped(), amp, gravity, foam);
        p.w1 = Vec4::new(w[0], w[1], w[2], w[3]);
        p.w2 = Vec4::new(w[4], if under.active() { 1.0 } else { 0.0 }, 60.0, 500.0);
    }

    // Lumière du fond marin : absorption et caustiques sur le sol des tuiles
    let sea_kind = terrain.sea_kind(dir);
    let tint = sea_kind.map_or([1.0; 4], |k| k.color());
    let caustic_power = match sea_kind {
        Some(VoxelType::Water) => 1.0,
        Some(VoxelType::Ammonia) => 0.35,
        _ => 0.0,
    };
    let m = Mat3::from_quat(to_local);
    let params = CausticParams {
        to0: m.row(0).extend(root.translation.x),
        to1: m.row(1).extend(root.translation.y),
        to2: m.row(2).extend(root.translation.z),
        sea: Vec4::new(if sea_kind.is_some() { terrain.params.radius + terrain.params.tide.at(dir) } else { 0.0 }, day, time.elapsed_secs_wrapped(), voxel),
        sun: sun_dir.extend(caustic_power),
        absorb: Vec4::new((1.0 - tint[0]) * 0.05 + 0.004, (1.0 - tint[1]) * 0.05 + 0.004, (1.0 - tint[2]) * 0.05 + 0.004, 0.0),
    };
    for (_, mat) in terrain_mats.iter_mut() {
        mat.extension.params = params;
    }

    // Sous l'eau ? (la caméra sous la surface de la colonne où elle se trouve)
    let sea = terrain.sea_surface(dir);
    let now = match sea {
        Some((sea_r, liquid)) if cam_local.length() < sea_r - 0.02 * voxel => {
            let depth = sea_r - cam_local.length();
            Underwater { depth, kind: Some(liquid), light: (-(depth / voxel) / clarity(liquid) * 1.6).exp().clamp(0.0, 1.0) }
        }
        _ => Underwater::default(),
    };
    if now.kind != under.kind || (now.depth - under.depth).abs() > 1e-3 {
        *under = now;
    }
    if under.active() {
        // Moins de lumière du soleil et du ciel avec la profondeur
        let f = under.light;
        for (_, mut light, _) in &mut suns {
            light.illuminance *= f;
        }
        ambient.brightness *= 0.35 + 0.65 * f;
    }
}

/// Brouillard et ciel de l'eau quand la caméra est dessous.
fn underwater_fog(mut commands: Commands, under: Res<Underwater>, surface: Res<Surface>, mut clear: ResMut<ClearColor>, cam_q: Query<Entity, With<Camera3d>>) {
    let (Some(liquid), Ok(cam)) = (under.kind, cam_q.get_single()) else { return };
    let voxel = surface.terrain().map_or(1.0, |t| t.voxel());
    let tint = liquid.color();
    let day = surface.daylight().max(0.05);
    let f = 0.15 + 0.85 * day * under.light;
    let color = Color::srgb(tint[0] * f * 0.8, tint[1] * f * 0.8, tint[2] * f * 0.8);
    clear.0 = color;
    commands.entity(cam).insert(DistanceFog { color, falloff: FogFalloff::from_visibility(clarity(liquid) * voxel), ..default() });
}
