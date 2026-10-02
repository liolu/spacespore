use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;
use crate::net::UniverseClock;
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use rand::Rng;

use std::collections::{HashMap, HashSet};
use crate::kepler::{OrbitalElements, DEFAULT_MU};
use crate::lod::{compute_lod_level, LodChunk, LodLevel};
use crate::mesher::{build_celestial_chunk_mesh, build_chunk_mesh, build_gas_giant_mesh, GasLook};
use crate::galaxy_shape::{CapMode, Shape};
use crate::settings::{GalaxyConfig, GameSettings, PlanetConfig, SystemSpatialIndex, CORE_EXCLUSION, GALAXY_SCALE, SYSTEM_CELL_SIZE, STREAM_RADIUS};
use crate::surface::{FarMesh, Surface};
use bevy::render::view::NoFrustumCulling;
use bevy::tasks::{block_on, futures_lite::future, AsyncComputeTaskPool, ComputeTaskPool, Task};
use crate::astre::{AstreLodRoot, ReloadAstre};
use crate::ship::Ship;
/// Les planètes tournent plus vite que ne le voudrait la gravité d'orbites aussi larges : sans
/// cela, un tour durerait des heures et le soleil ne bougerait jamais dans le ciel.
const PLANET_MU_SCALE: f32 = 250.0;
const STAR_DIVISIONS: usize = 4;
const MOON_DIVISIONS: usize = 3;

const LOD_STARS_END: f32 = 100_000_000.0 * GALAXY_SCALE;
const LOD_STARS_GONE: f32 = 120_000_000.0 * GALAXY_SCALE;
const LOD_CAPS_START: f32 = 70_000_000.0 * GALAXY_SCALE;
const LOD_CAPS_FULL: f32 = 110_000_000.0 * GALAXY_SCALE;
const LOD_STEPS: usize = 10;
const STAR_BRIGHTNESS_STEPS: usize = 10;
const ARM_COLORS: usize = 5;

const ARM_PALETTE: [[f32; 3]; ARM_COLORS] = [
    [1.0, 0.55, 0.15],
    [1.0, 0.82, 0.4],
    [0.85, 0.75, 1.0],
    [0.55, 0.45, 1.0],
    [0.35, 0.55, 1.0],
];

fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[derive(Resource)]
pub struct GalaxyLodMaterials {
    pub capsule_steps: Vec<Vec<Handle<StandardMaterial>>>,
}

#[derive(Resource)]
pub struct StarBrightnessMaterials {
    pub steps: Vec<Handle<StandardMaterial>>,
}

pub struct PlanetPlugin;

impl Plugin for PlanetPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<RegeneratePlanet>()
            .insert_resource(SpawnedSystems(HashSet::new()))
            .init_resource::<StarSectors>()
            .add_systems(Startup, (build_spatial_index, generate_all).chain())
            .add_systems(
                Update,
                (orbit_planets, orbit_stars, orbit_moons, orbit_asteroid_belts, update_flare_voxels, rotate_clouds, shimmer_auroras, regenerate_all, update_lod, update_star_visibility, update_far_star_scale, update_arm_capsule_lod, stream_system_bodies, reload_stars, reload_planets, reload_moons, reload_asteroid_belts, cleanup_hidden_toplevel, rotate_accretion_disk),
            );
    }
}

#[derive(Resource)]
pub struct SpawnedSystems(pub HashSet<usize>);

#[derive(Event)]
pub struct RegeneratePlanet;

#[derive(Component)]
pub struct PlanetRoot;

#[derive(Component)]
pub struct PlanetId(pub usize);

#[derive(Component)]
pub struct StarRoot;

#[derive(Component)]
pub struct StarId(pub usize);

#[derive(Component)]
pub struct StarBeacon;

#[derive(Component)]
pub struct StarChunk;

#[derive(Component)]
pub struct FarStar {
    pub sys_idx: usize,
    pub radius: f32,
    /// Éclat selon la luminosité du type d'étoile (1 = Soleil).
    pub glow: f32,
    pub galaxy_id: u32,
}

/// Taille visée d'un secteur : environ 100 étoiles voisines d'une même galaxie.
const SECTOR_STARS: usize = 100;

/// Un secteur : ~100 étoiles voisines, affichées (ou masquées) et mises à jour ensemble.
/// À l'échelle de la galaxie, les étoiles lointaines bougent très lentement dans le ciel : un
/// secteur lointain n'est recalculé que si la caméra a assez bougé pour que cela se voie.
pub struct Sector {
    pub galaxy: u32,
    /// Centre absolu et rayon (distance à l'étoile la plus éloignée).
    pub abs_center: Vec3,
    pub radius: f32,
    systems: Vec<usize>,
    members: Vec<Entity>,
    hidden: bool,
    last_cam: Vec3,
    last_fwd: Vec3,
    last_epoch: u32,
    last_keep: f32,
}

#[derive(Resource, Default)]
pub struct StarSectors {
    pub list: Vec<Sector>,
    /// Secteur de chaque système (par indice de système).
    of_system: HashMap<usize, usize>,
}

type FarStarItem = (u32, usize, Vec3, Entity);

impl StarSectors {
    /// (numéro du secteur, nombre d'étoiles) du système.
    pub fn sector_of(&self, sys: usize) -> Option<(usize, usize)> {
        let i = *self.of_system.get(&sys)?;
        Some((i, self.list[i].systems.len()))
    }

    /// Découpe récursivement les étoiles de chaque galaxie (médiane sur l'axe le plus long)
    /// jusqu'à des secteurs de `SECTOR_STARS` étoiles au plus.
    fn build(stars: Vec<FarStarItem>) -> Self {
        fn split(items: &mut [FarStarItem], out: &mut Vec<Sector>) {
            if items.is_empty() {
                return;
            }
            if items.len() <= SECTOR_STARS {
                let center = items.iter().map(|i| i.2).sum::<Vec3>() / items.len() as f32;
                out.push(Sector {
                    galaxy: items[0].0,
                    abs_center: center,
                    radius: items.iter().map(|i| i.2.distance(center)).fold(0.0, f32::max),
                    systems: items.iter().map(|i| i.1).collect(),
                    members: items.iter().map(|i| i.3).collect(),
                    hidden: false,
                    last_cam: Vec3::splat(f32::NAN),
                    last_fwd: Vec3::ZERO,
                    last_epoch: u32::MAX,
                    last_keep: 1.0,
                });
                return;
            }
            let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
            for i in items.iter() {
                lo = lo.min(i.2);
                hi = hi.max(i.2);
            }
            let size = hi - lo;
            let axis = if size.x >= size.y && size.x >= size.z { 0 } else if size.y >= size.z { 1 } else { 2 };
            items.sort_by(|a, b| a.2[axis].total_cmp(&b.2[axis]));
            let mid = items.len() / 2;
            let (left, right) = items.split_at_mut(mid);
            split(left, out);
            split(right, out);
        }
        let mut by_galaxy: std::collections::BTreeMap<u32, Vec<FarStarItem>> = Default::default();
        for star in stars {
            by_galaxy.entry(star.0).or_default().push(star);
        }
        let mut list = Vec::new();
        for (_, mut items) in by_galaxy {
            split(&mut items, &mut list);
        }
        let mut of_system = HashMap::new();
        for (i, sector) in list.iter().enumerate() {
            for &sys in &sector.systems {
                of_system.insert(sys, i);
            }
        }
        Self { list, of_system }
    }
}

#[derive(Component)]
pub struct MoonRoot;

#[derive(Component)]
pub struct MoonId {
    pub planet_idx: usize,
    pub moon_idx: usize,
}

#[derive(Component)]
/// Éruptions d'une étoile : un seul maillage regroupant tous les cubes,
/// reconstruit à chaque image (au lieu de 320 entités par étoile).
pub struct FlareVoxel {
    pub star_idx: usize,
}

#[derive(Component)]
#[allow(dead_code)] // décalage gardé avec le système, pas encore relu
pub struct SystemOffset(pub Vec3);

#[derive(Component)]
pub struct SystemIdx(pub usize);

#[derive(Component)]
pub struct AsteroidBeltRoot;

#[derive(Component)]
pub struct AsteroidBeltId(pub usize);

#[derive(Component)]
pub struct GalacticCore;

#[derive(Component)]
pub struct AccretionDisk;

#[derive(Component)]
pub struct DistantGalaxyCore {
    pub galaxy_id: u32,
}

#[derive(Component)]
pub struct GalaxyMeta {
    pub id: u32,
}

#[derive(Component)]
pub struct ArmCapsule {
    pub galaxy_id: u32,
    pub base_scale: Vec3,
    pub color_idx: usize,
    pub detail: bool,
}

#[derive(Component)]
/// Couche de nuages d'une planète : un seul maillage qui tourne autour de l'axe Y.
pub struct CloudVoxel {
    pub planet_idx: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VoxelType {
    Air,
    Water,
    Sand,
    Grass,
    Stone,
    Snow,
    /// Banquise, glace de mer (phase 4).
    Ice,
    /// Mers de méthane (Titan), d'ammoniac, de lave (phase 4).
    Methane,
    Ammonia,
    Lava,
    /// Une matière par biome (phase 6).
    Tundra,
    Taiga,
    Forest,
    Steppe,
    Savanna,
    Jungle,
    Swamp,
    Basalt,
    Salt,
    Rust,
    Crystal,
    Spore,
    Fungus,
    Glass,
    Sulfur,
}

impl VoxelType {
    pub fn color(self) -> [f32; 4] {
        match self {
            VoxelType::Air => [0.0, 0.0, 0.0, 0.0],
            VoxelType::Water => [0.18, 0.42, 0.85, 1.0],
            VoxelType::Sand => [0.88, 0.82, 0.52, 1.0],
            VoxelType::Grass => [0.22, 0.58, 0.14, 1.0],
            VoxelType::Stone => [0.48, 0.46, 0.50, 1.0],
            VoxelType::Snow => [0.93, 0.94, 0.98, 1.0],
            VoxelType::Ice => [0.72, 0.85, 0.95, 1.0],
            VoxelType::Methane => [0.22, 0.15, 0.09, 1.0],
            VoxelType::Ammonia => [0.45, 0.62, 0.6, 1.0],
            VoxelType::Lava => [1.0, 0.38, 0.06, 1.0],
            VoxelType::Tundra => [0.48, 0.5, 0.36, 1.0],
            VoxelType::Taiga => [0.12, 0.33, 0.22, 1.0],
            VoxelType::Forest => [0.12, 0.42, 0.12, 1.0],
            VoxelType::Steppe => [0.62, 0.62, 0.32, 1.0],
            VoxelType::Savanna => [0.7, 0.62, 0.28, 1.0],
            VoxelType::Jungle => [0.07, 0.4, 0.1, 1.0],
            VoxelType::Swamp => [0.25, 0.32, 0.16, 1.0],
            VoxelType::Basalt => [0.2, 0.19, 0.2, 1.0],
            VoxelType::Salt => [0.94, 0.9, 0.88, 1.0],
            VoxelType::Rust => [0.62, 0.28, 0.14, 1.0],
            VoxelType::Crystal => [0.55, 0.8, 0.95, 1.0],
            VoxelType::Spore => [0.62, 0.32, 0.6, 1.0],
            VoxelType::Fungus => [0.85, 0.45, 0.3, 1.0],
            VoxelType::Glass => [0.16, 0.24, 0.2, 1.0],
            VoxelType::Sulfur => [0.9, 0.82, 0.25, 1.0],
        }
    }

    /// Un liquide : on y nage, il s'assombrit avec la profondeur.
    pub fn is_liquid(self) -> bool {
        matches!(self, VoxelType::Water | VoxelType::Methane | VoxelType::Ammonia | VoxelType::Lava)
    }

    pub fn is_solid(self) -> bool {
        !matches!(self, VoxelType::Air)
    }
}

#[derive(Component)]
pub struct PlanetChunk {
    pub face: CubeFace,
    pub grid_x: usize,
    pub grid_y: usize,
    pub current_lod: LodLevel,
    pub planet_id: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CubeFace {
    PosX,
    NegX,
    PosY,
    NegY,
    PosZ,
    NegZ,
}

impl CubeFace {
    pub fn all() -> [CubeFace; 6] {
        [
            CubeFace::PosX,
            CubeFace::NegX,
            CubeFace::PosY,
            CubeFace::NegY,
            CubeFace::PosZ,
            CubeFace::NegZ,
        ]
    }

    pub fn to_sphere_pos(self, u: f32, v: f32) -> Vec3 {
        let (u, v) = (u * 2.0 - 1.0, v * 2.0 - 1.0);
        let pos = match self {
            CubeFace::PosX => Vec3::new(1.0, v, -u),
            CubeFace::NegX => Vec3::new(-1.0, v, u),
            CubeFace::PosY => Vec3::new(u, 1.0, -v),
            CubeFace::NegY => Vec3::new(u, -1.0, v),
            CubeFace::PosZ => Vec3::new(u, v, 1.0),
            CubeFace::NegZ => Vec3::new(-u, v, -1.0),
        };
        pos.normalize()
    }
}

fn star_color_group(r: f32, g: f32, b: f32) -> usize {
    const PALETTES: [[f32; 3]; 5] = [
        [1.0, 0.5, 0.3],
        [1.0, 0.7, 0.4],
        [1.0, 0.92, 0.65],
        [0.8, 0.85, 1.0],
        [0.6, 0.7, 1.0],
    ];
    let mut best = 0;
    let mut best_d = f32::MAX;
    for (i, p) in PALETTES.iter().enumerate() {
        let d = (r - p[0]) * (r - p[0]) + (g - p[1]) * (g - p[1]) + (b - p[2]) * (b - p[2]);
        if d < best_d { best_d = d; best = i; }
    }
    best
}

const ATLAS_COLS: u32 = 5;
const ATLAS_CELL: u32 = 32;

const PALETTE: [[f32; 3]; 5] = [
    [1.0, 0.5, 0.3],
    [1.0, 0.7, 0.4],
    [1.0, 0.92, 0.65],
    [0.8, 0.85, 1.0],
    [0.6, 0.7, 1.0],
];

fn create_star_atlas(images: &mut Assets<Image>) -> Handle<Image> {
    let w = ATLAS_CELL * ATLAS_COLS;
    let h = ATLAS_CELL;
    let center = ATLAS_CELL as f32 / 2.0;
    let mut data = vec![0u8; (w * h * 4) as usize];

    for (gi, col) in PALETTE.iter().enumerate() {
        let ox = gi as u32 * ATLAS_CELL;
        for y in 0..ATLAS_CELL {
            for x in 0..ATLAS_CELL {
                let dx = x as f32 - center + 0.5;
                let dy = y as f32 - center + 0.5;
                let d = (dx * dx + dy * dy).sqrt() / center;
                let alpha = (1.0 - d).clamp(0.0, 1.0).powf(1.5);
                let px = ((y * w + ox + x) * 4) as usize;
                data[px]     = (col[0] * 255.0) as u8;
                data[px + 1] = (col[1] * 255.0) as u8;
                data[px + 2] = (col[2] * 255.0) as u8;
                data[px + 3] = (alpha * 255.0) as u8;
            }
        }
    }

    images.add(Image::new(
        Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    ))
}

fn make_atlas_quad(meshes: &mut Assets<Mesh>, group: usize) -> Handle<Mesh> {
    let u_min = group as f32 / ATLAS_COLS as f32;
    let u_max = (group as f32 + 1.0) / ATLAS_COLS as f32;
    let mut mesh = Mesh::from(Rectangle::new(2.0, 2.0));
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![
        [u_min, 1.0],
        [u_max, 1.0],
        [u_max, 0.0],
        [u_min, 0.0],
    ]);
    meshes.add(mesh)
}

fn build_spatial_index(mut commands: Commands, settings: Res<GameSettings>) {
    commands.insert_resource(SystemSpatialIndex::build(&settings));
}

fn generate_all(
    mut commands: Commands,
    settings: Res<GameSettings>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut spawned: ResMut<SpawnedSystems>,
) {
    let atlas = create_star_atlas(&mut images);

    let mut star_brightness_steps: Vec<Handle<StandardMaterial>> = Vec::with_capacity(STAR_BRIGHTNESS_STEPS);
    for step in 0..STAR_BRIGHTNESS_STEPS {
        let b = (step as f32 / (STAR_BRIGHTNESS_STEPS - 1) as f32).max(0.05);
        star_brightness_steps.push(materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 1.0, 1.0, b),
            base_color_texture: Some(atlas.clone()),
            emissive: LinearRgba::new(7.0 * b, 6.0 * b, 3.0 * b, 1.0),
            emissive_texture: Some(atlas.clone()),
            unlit: true,
            alpha_mode: AlphaMode::Add,
            ..default()
        }));
    }
    commands.insert_resource(StarBrightnessMaterials { steps: star_brightness_steps.clone() });
    let atlas_mat = star_brightness_steps.last().unwrap().clone();

    let quad_meshes: Vec<Handle<Mesh>> = (0..5)
        .map(|i| make_atlas_quad(&mut meshes, i))
        .collect();

    // Toutes les galaxies (principale et extérieures) : un billboard par système
    let mut far_stars: Vec<FarStarItem> = Vec::with_capacity(settings.systems.len());
    for (si, sys) in settings.systems.iter().enumerate() {
        let center = sys.center();
        let gal_center = settings.galaxies.get(sys.galaxy_id as usize).map_or(Vec3::ZERO, |g| g.center());
        if center.distance(gal_center) < CORE_EXCLUSION { continue; }
        if let Some(star_cfg) = sys.stars.first() {
            let group = star_color_group(
                star_cfg.light_color_r,
                star_cfg.light_color_g,
                star_cfg.light_color_b,
            );
            let entity = commands
                .spawn((
                    Mesh3d(quad_meshes[group].clone()),
                    MeshMaterial3d(atlas_mat.clone()),
                    Transform::from_translation(center).with_scale(Vec3::splat(star_cfg.radius * 0.5)),
                    NotShadowCaster,
                    FarStar { sys_idx: si, radius: star_cfg.radius, glow: star_cfg.glow(), galaxy_id: sys.galaxy_id },
                ))
                .id();
            far_stars.push((sys.galaxy_id, si, sys.abs_center(), entity));
        }
    }
    commands.insert_resource(StarSectors::build(far_stars));

    if let Some(sys) = settings.systems.first() {
        let center = sys.center();
        let sys_cam = center + Vec3::new(0.0, 80.0, 200.0);
        spawn_system_bodies(
            &mut commands, sys, &settings, 0,
            &mut meshes, &mut materials, sys_cam, center,
        );
        spawned.0.insert(0);
    }

    spawn_galactic_core(&mut commands, &mut meshes, &mut materials);

    // Capsule partagée + matériaux LOD (10 niveaux d'opacité)
    let capsule_mesh = meshes.add(Capsule3d::new(1.0, 1.0));
    let mut capsule_steps: Vec<Vec<Handle<StandardMaterial>>> = Vec::with_capacity(ARM_COLORS);
    for ci in 0..ARM_COLORS {
        let c = ARM_PALETTE[ci];
        let mut steps = Vec::with_capacity(LOD_STEPS);
        for step in 0..LOD_STEPS {
            let o = step as f32 / (LOD_STEPS - 1) as f32;
            steps.push(materials.add(StandardMaterial {
                base_color: Color::srgba(c[0], c[1], c[2], 0.07 * o),
                emissive: LinearRgba::new(
                    0.45 * c[0] * o,
                    0.45 * c[1] * o,
                    0.45 * c[2] * o,
                    1.0,
                ),
                unlit: true,
                alpha_mode: AlphaMode::Add,
                ..default()
            }));
        }
        capsule_steps.push(steps);
    }
    commands.insert_resource(GalaxyLodMaterials { capsule_steps: capsule_steps.clone() });
    let capsule_mat = capsule_steps[1].last().unwrap().clone();

    // GalaxyMeta + capsules pour chaque galaxie (0 = principale)
    for (gid, gal) in settings.galaxies.iter().enumerate() {
        commands.spawn(GalaxyMeta { id: gid as u32 });
        spawn_arm_capsules(
            &mut commands, &capsule_mesh, &capsule_mat,
            gid as u32, gal.center(), gal.tilt, &gal.shape(), gal.radius,
            // Chaque galaxie extérieure décale sa palette : elles n'ont pas toutes les mêmes couleurs
            if gid == 0 { 0 } else { (crate::settings::pseudo_rand(gid as u32 * 31 + 7) * ARM_COLORS as f32) as usize % ARM_COLORS },
        );
    }

    spawn_distant_galaxy_cores(&mut commands, &mut meshes, &mut materials, &settings.galaxies);
}

fn spawn_galactic_core(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) {
    let core_radius = 30_000.0_f32 * GALAXY_SCALE; // trou noir central : 10 fois plus grand

    let core_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.01, 0.0, 0.02),
        emissive: LinearRgba::new(0.0, 0.0, 0.0, 1.0),
        unlit: true,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(core_radius).mesh().ico(4).unwrap())),
        MeshMaterial3d(core_mat),
        Transform::from_translation(Vec3::ZERO),
        NotShadowCaster,
        GalacticCore,
    ));

    let ring_segments = 128_u32;
    let ring_layers = 3_u32;
    let inner_r = core_radius * 1.3;
    let outer_r = core_radius * 4.0;

    for layer in 0..ring_layers {
        let t = layer as f32 / ring_layers as f32;
        let r_in = inner_r + (outer_r - inner_r) * t;
        let r_out = inner_r + (outer_r - inner_r) * (t + 1.0 / ring_layers as f32);
        let thickness = (r_out - r_in) * 0.15;

        let mut positions: Vec<[f32; 3]> = Vec::new();
        let mut normals: Vec<[f32; 3]> = Vec::new();
        let mut colors: Vec<[f32; 4]> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();

        for i in 0..=ring_segments {
            let angle = i as f32 / ring_segments as f32 * std::f32::consts::TAU;
            let cos = angle.cos();
            let sin = angle.sin();

            let brightness = 1.0 - t * 0.6;
            let inner_color = [1.0 * brightness, 0.6 * brightness, 0.15 * brightness, 0.9];
            let outer_color = [0.6 * brightness, 0.1 * brightness, 0.4 * brightness, 0.3];

            positions.push([cos * r_in, 0.0, sin * r_in]);
            normals.push([0.0, 1.0, 0.0]);
            colors.push(inner_color);

            positions.push([cos * r_out, 0.0, sin * r_out]);
            normals.push([0.0, 1.0, 0.0]);
            colors.push(outer_color);

            if i < ring_segments {
                let base = i * 2;
                indices.push(base);
                indices.push(base + 1);
                indices.push(base + 2);
                indices.push(base + 1);
                indices.push(base + 3);
                indices.push(base + 2);
            }
        }

        let mut mesh = Mesh::new(
            bevy::render::mesh::PrimitiveTopology::TriangleList,
            bevy::render::render_asset::RenderAssetUsages::default(),
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
        mesh.insert_indices(bevy::render::mesh::Indices::U32(indices));

        let disk_mat = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            emissive: LinearRgba::new(4.0 * (1.0 - t), 1.5 * (1.0 - t), 0.5, 1.0),
            unlit: true,
            alpha_mode: AlphaMode::Add,
            double_sided: true,
            cull_mode: None,
            ..default()
        });

        let tilt = Quat::from_rotation_x(0.25) * Quat::from_rotation_z(0.1);
        let y_off = thickness * (layer as f32 - 1.0) * 0.3;
        commands.spawn((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(disk_mat),
            Transform::from_translation(Vec3::new(0.0, y_off, 0.0)).with_rotation(tilt),
            NotShadowCaster,
            AccretionDisk,
        ));
    }
}

fn spawn_arm_capsules(
    commands: &mut Commands,
    capsule_mesh: &Handle<Mesh>,
    capsule_mat: &Handle<StandardMaterial>,
    galaxy_id: u32,
    center: Vec3,
    tilt: Quat,
    shape: &Shape,
    radius: f32,
    color_shift: usize,
) {
    const ARM_BASE: &[f32] = &[0.12, 0.22, 0.32, 0.42, 0.52, 0.62, 0.72, 0.82, 0.92];
    const ARM_DETAIL: &[f32] = &[0.17, 0.27, 0.37, 0.47, 0.57, 0.67, 0.77, 0.87];
    const LOOP: &[f32] = &[0.02, 0.10, 0.18, 0.26, 0.34, 0.42, 0.50, 0.58, 0.66, 0.74, 0.82, 0.90];
    const FEW: &[f32] = &[0.5];

    for curve in &shape.curves {
        let passes: &[(bool, &[f32])] = match curve.caps {
            CapMode::Arm => &[(false, ARM_BASE), (true, ARM_DETAIL)],
            CapMode::Loop => &[(false, LOOP)],
            CapMode::Few => &[(false, FEW)],
            CapMode::None => &[],
        };
        for &(is_detail, samples) in passes {
            for &t in samples {
                let (local_pos, tangent) = curve.at(t);
                let world_pos = center + tilt * local_pos;
                let world_tangent = tilt * tangent;

                let cap_half_len = radius * 0.12;
                let cap_radius = radius * 0.006 * (1.0 - t * 0.5);

                let rot = Quat::from_rotation_arc(Vec3::Y, world_tangent);
                let base_scale = Vec3::new(cap_radius, cap_half_len, cap_radius);
                let color_idx = (((t * (ARM_COLORS as f32 - 0.01)) as usize).min(ARM_COLORS - 1) + color_shift) % ARM_COLORS;

                commands.spawn((
                    Mesh3d(capsule_mesh.clone()),
                    MeshMaterial3d(capsule_mat.clone()),
                    Transform {
                        translation: world_pos,
                        rotation: rot,
                        scale: Vec3::ZERO,
                    },
                    Visibility::Hidden,
                    NotShadowCaster,
                    ArmCapsule { galaxy_id, base_scale, color_idx, detail: is_detail },
                ));
            }
        }
    }
}

/// Trou noir + disque d'accrétion des galaxies extérieures. Les étoiles de ces
/// galaxies sont de vrais systèmes (`settings.systems`) affichés en `FarStar`.
fn spawn_distant_galaxy_cores(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    galaxies: &[GalaxyConfig],
) {
    let tau = std::f32::consts::TAU;
    let core_mesh = meshes.add(Sphere::new(1.0).mesh().ico(3).unwrap());

    for (gi, gal) in galaxies.iter().skip(1).enumerate() {
        let center = gal.center();
        let tilt = gal.tilt;

        // Trou noir central
        let core_r = gal.core_radius;
        let core_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(0.01, 0.0, 0.02),
            emissive: LinearRgba::new(0.0, 0.0, 0.0, 1.0),
            unlit: true,
            ..default()
        });
        commands.spawn((
            Mesh3d(core_mesh.clone()),
            MeshMaterial3d(core_mat),
            Transform::from_translation(center).with_scale(Vec3::splat(core_r)),
            NotShadowCaster,
            DistantGalaxyCore { galaxy_id: gi as u32 + 1 },
        ));

        // Disque d'accrétion (1 seul anneau par galaxie pour rester léger)
        let ring_seg = 64_u32;
        let inner = core_r * 1.3;
        let outer = core_r * 3.5;

        let mut positions: Vec<[f32; 3]> = Vec::new();
        let mut normals: Vec<[f32; 3]> = Vec::new();
        let mut colors: Vec<[f32; 4]> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();

        for i in 0..=ring_seg {
            let angle = i as f32 / ring_seg as f32 * tau;
            let cos = angle.cos();
            let sin = angle.sin();

            positions.push([cos * inner, 0.0, sin * inner]);
            normals.push([0.0, 1.0, 0.0]);
            colors.push([1.0, 0.6, 0.15, 0.9]);

            positions.push([cos * outer, 0.0, sin * outer]);
            normals.push([0.0, 1.0, 0.0]);
            colors.push([0.6, 0.1, 0.4, 0.2]);

            if i < ring_seg {
                let base = i * 2;
                indices.push(base);
                indices.push(base + 1);
                indices.push(base + 2);
                indices.push(base + 1);
                indices.push(base + 3);
                indices.push(base + 2);
            }
        }

        let mut mesh = Mesh::new(
            bevy::render::mesh::PrimitiveTopology::TriangleList,
            bevy::render::render_asset::RenderAssetUsages::default(),
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
        mesh.insert_indices(bevy::render::mesh::Indices::U32(indices));

        let disk_mat = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            emissive: LinearRgba::new(3.0, 1.2, 0.4, 1.0),
            unlit: true,
            alpha_mode: AlphaMode::Add,
            double_sided: true,
            cull_mode: None,
            ..default()
        });

        let disk_tilt = tilt * Quat::from_rotation_x(0.25);
        commands.spawn((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(disk_mat),
            Transform::from_translation(center).with_rotation(disk_tilt),
            NotShadowCaster,
            DistantGalaxyCore { galaxy_id: gi as u32 + 1 },
        ));
    }
}

/// Couche de nuages d'une planète : tous les patchs fusionnés en un seul
/// maillage, dans le repère local de la planète.
fn build_cloud_layer_mesh(pcfg: &PlanetConfig) -> Option<Mesh> {
    let mut all_positions: Vec<[f32; 3]> = Vec::new();
    let mut all_normals: Vec<[f32; 3]> = Vec::new();
    let mut all_indices: Vec<u32> = Vec::new();

    let density = pcfg.cloud_density.clamp(0.1, 1.0);
    let cloud_r = pcfg.radius + pcfg.terrain_height + pcfg.cloud_altitude;

    use noise::{NoiseFn, Perlin};
    let perlin = Perlin::new(pcfg.seed + 500);
    let perlin2 = Perlin::new(pcfg.seed + 501);

    let patch_count = 40 + (density * 60.0) as u32;
    let patch_grid = 16_i32;
    let cell = cloud_r * 0.04;

    for ci in 0..patch_count {
        let seed_a = pseudo_hash(pcfg.seed as f32, ci as f32 * 3.7);
        let seed_b = pseudo_hash(pcfg.seed as f32, ci as f32 * 7.1);
        let theta_c = seed_a * std::f32::consts::TAU;
        let phi_c = (seed_b * 2.0 - 1.0).clamp(-1.0, 1.0).acos();

        let center_dir = Vec3::new(
            theta_c.cos() * phi_c.sin(),
            phi_c.cos(),
            theta_c.sin() * phi_c.sin(),
        ).normalize();

        let up = if center_dir.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
        let tangent_u = center_dir.cross(up).normalize();
        let tangent_v = center_dir.cross(tangent_u).normalize();

        let mut grid = vec![0_u32; (patch_grid * patch_grid) as usize];
        let threshold = 0.1 - density as f64 * 0.15;

        for gx in 0..patch_grid {
            for gz in 0..patch_grid {
                let lx = (gx as f32 - patch_grid as f32 * 0.5) * cell;
                let lz = (gz as f32 - patch_grid as f32 * 0.5) * cell;

                let world = center_dir * cloud_r + tangent_u * lx + tangent_v * lz;
                let wd = world.normalize();
                let nx = wd.x as f64;
                let ny = wd.y as f64;
                let nz = wd.z as f64;

                let n1 = perlin.get([nx * 4.0 + ci as f64 * 10.0, ny * 4.0, nz * 4.0]);
                let n2 = perlin2.get([nx * 8.0 + ci as f64 * 5.0, ny * 8.0, nz * 8.0]);
                let n = n1 * 0.65 + n2 * 0.35;

                let dx = (gx as f32 - patch_grid as f32 * 0.5).abs() / (patch_grid as f32 * 0.5);
                let dz = (gz as f32 - patch_grid as f32 * 0.5).abs() / (patch_grid as f32 * 0.5);
                let edge = 1.0 - (dx * dx + dz * dz).sqrt().min(1.0);

                if n > threshold && edge as f64 > 0.2 {
                    let h = (1 + ((n - threshold) * 3.0) as u32).min(4);
                    grid[(gx * patch_grid + gz) as usize] = h;
                }
            }
        }

        let mut positions: Vec<[f32; 3]> = Vec::new();
        let mut normals: Vec<[f32; 3]> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();

        let get_h = |x: i32, z: i32| -> u32 {
            if x < 0 || x >= patch_grid || z < 0 || z >= patch_grid { return 0; }
            grid[(x * patch_grid + z) as usize]
        };

        for gx in 0..patch_grid {
            for gz in 0..patch_grid {
                let h = get_h(gx, gz);
                if h == 0 { continue; }

                let x0 = (gx as f32 - patch_grid as f32 * 0.5) * cell;
                let z0 = (gz as f32 - patch_grid as f32 * 0.5) * cell;
                let x1 = x0 + cell;
                let z1 = z0 + cell;

                for y_layer in 0..h {
                    let y0 = y_layer as f32 * cell * 0.5;
                    let y1 = y0 + cell * 0.5;

                    if y_layer == h - 1 {
                        let vi = positions.len() as u32;
                        positions.extend_from_slice(&[
                            [x0, y1, z1], [x1, y1, z1], [x1, y1, z0], [x0, y1, z0],
                        ]);
                        normals.extend_from_slice(&[[0.0,1.0,0.0]; 4]);
                        indices.extend_from_slice(&[vi, vi+1, vi+2, vi, vi+2, vi+3]);
                    }

                    if y_layer == 0 {
                        let vi = positions.len() as u32;
                        positions.extend_from_slice(&[
                            [x0, y0, z0], [x1, y0, z0], [x1, y0, z1], [x0, y0, z1],
                        ]);
                        normals.extend_from_slice(&[[0.0,-1.0,0.0]; 4]);
                        indices.extend_from_slice(&[vi, vi+1, vi+2, vi, vi+2, vi+3]);
                    }

                    if get_h(gx - 1, gz) <= y_layer {
                        let vi = positions.len() as u32;
                        positions.extend_from_slice(&[
                            [x0, y0, z0], [x0, y1, z0], [x0, y1, z1], [x0, y0, z1],
                        ]);
                        normals.extend_from_slice(&[[-1.0,0.0,0.0]; 4]);
                        indices.extend_from_slice(&[vi, vi+1, vi+2, vi, vi+2, vi+3]);
                    }

                    if get_h(gx + 1, gz) <= y_layer {
                        let vi = positions.len() as u32;
                        positions.extend_from_slice(&[
                            [x1, y0, z1], [x1, y1, z1], [x1, y1, z0], [x1, y0, z0],
                        ]);
                        normals.extend_from_slice(&[[1.0,0.0,0.0]; 4]);
                        indices.extend_from_slice(&[vi, vi+1, vi+2, vi, vi+2, vi+3]);
                    }

                    if get_h(gx, gz - 1) <= y_layer {
                        let vi = positions.len() as u32;
                        positions.extend_from_slice(&[
                            [x1, y0, z0], [x1, y1, z0], [x0, y1, z0], [x0, y0, z0],
                        ]);
                        normals.extend_from_slice(&[[0.0,0.0,-1.0]; 4]);
                        indices.extend_from_slice(&[vi, vi+1, vi+2, vi, vi+2, vi+3]);
                    }

                    if get_h(gx, gz + 1) <= y_layer {
                        let vi = positions.len() as u32;
                        positions.extend_from_slice(&[
                            [x0, y0, z1], [x0, y1, z1], [x1, y1, z1], [x1, y0, z1],
                        ]);
                        normals.extend_from_slice(&[[0.0,0.0,1.0]; 4]);
                        indices.extend_from_slice(&[vi, vi+1, vi+2, vi, vi+2, vi+3]);
                    }
                }
            }
        }

        if positions.is_empty() { continue; }

        let rotation = Quat::from_rotation_arc(Vec3::Y, center_dir);
        let offset = center_dir * cloud_r;
        let base = all_positions.len() as u32;
        all_positions.extend(positions.iter().map(|p| (rotation * Vec3::from_array(*p) + offset).to_array()));
        all_normals.extend(normals.iter().map(|n| (rotation * Vec3::from_array(*n)).to_array()));
        all_indices.extend(indices.iter().map(|i| i + base));
    }

    if all_positions.is_empty() {
        return None;
    }
    let mut mesh = Mesh::new(
        bevy::render::mesh::PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, all_positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, all_normals);
    mesh.insert_indices(bevy::render::mesh::Indices::U32(all_indices));
    Some(mesh)
}

fn spawn_cloud_layer(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    pcfg: &PlanetConfig,
    planet_idx: usize,
    planet_pos: Vec3,
) {
    let Some(mesh) = build_cloud_layer_mesh(pcfg) else { return };
    // Couleur selon le type de nuages (eau, acide sulfurique, méthane...)
    let c = if pcfg.air.present() { pcfg.air.cloud_color } else { [0.95, 0.95, 0.97] };
    let cloud_material = materials.add(StandardMaterial {
        base_color: Color::srgb(c[0], c[1], c[2]),
        alpha_mode: AlphaMode::Opaque,
        unlit: false,
        perceptual_roughness: 1.0,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(cloud_material),
        Transform::from_translation(planet_pos),
        CloudVoxel { planet_idx },
    ));
}

/// Allure d'une géante d'après sa nature et sa graine : Jupiter ou Saturne, Jupiter chaud
/// (sombre et rougeoyant), Neptune ou Uranus, mini-Neptune (brumeuse, presque unie).
pub fn gas_look(p: &PlanetConfig) -> GasLook {
    use crate::planetgen::system::PlanetKind;
    let h = |k: u32| crate::settings::pseudo_rand(p.seed.wrapping_mul(7).wrapping_add(k));
    // Légère teinte propre à chaque planète
    let tint = |c: [f32; 3], k: u32| {
        let t = (h(k) - 0.5) * 0.12;
        [(c[0] + t).clamp(0.0, 1.0), (c[1] + t * 0.5).clamp(0.0, 1.0), (c[2] - t).clamp(0.0, 1.0)]
    };
    let (palette, bands, swirl, contrast): (Vec<[f32; 3]>, f32, f32, f32) = match p.kind {
        PlanetKind::GasGiant if p.hot => (
            vec![[0.35, 0.15, 0.12], [0.55, 0.25, 0.15], [0.25, 0.1, 0.15], [0.7, 0.35, 0.2]],
            6.0 + h(1) * 4.0, 0.25, 0.8,
        ),
        PlanetKind::GasGiant if h(2) < 0.6 => (
            vec![[0.85, 0.55, 0.25], [0.70, 0.38, 0.15], [0.92, 0.78, 0.55], [0.60, 0.28, 0.10], [0.95, 0.88, 0.70], [0.75, 0.45, 0.20]],
            8.0 + h(1) * 8.0, 0.15 + h(3) * 0.15, 0.9,
        ),
        PlanetKind::GasGiant => (
            vec![[0.92, 0.85, 0.62], [0.85, 0.75, 0.5], [0.95, 0.9, 0.75], [0.8, 0.68, 0.45]],
            10.0 + h(1) * 6.0, 0.1, 0.7,
        ),
        PlanetKind::IceGiant if h(2) < 0.5 => (
            vec![[0.15, 0.28, 0.7], [0.2, 0.35, 0.85], [0.25, 0.45, 0.95], [0.35, 0.55, 0.95]],
            3.0 + h(1) * 3.0, 0.12, 0.5,
        ),
        PlanetKind::IceGiant => (vec![[0.55, 0.8, 0.88], [0.6, 0.85, 0.9], [0.65, 0.9, 0.92]], 3.0 + h(1) * 2.0, 0.06, 0.3),
        _ => (vec![[0.5, 0.65, 0.78], [0.55, 0.7, 0.8], [0.6, 0.75, 0.82]], 2.0 + h(1) * 2.0, 0.08, 0.35),
    };
    GasLook { palette: palette.iter().enumerate().map(|(i, c)| tint(*c, 10 + i as u32)).collect(), bands, swirl, contrast }
}

/// Maillage lointain d'un chunk de lune : mêmes relief, mers et biomes que son terrain voxel
/// (phase 9), ou l'ancienne roche grise pour une lune faite à la main.
fn moon_chunk_mesh(mcfg: &crate::settings::MoonConfig, view: Option<&PlanetConfig>, face: CubeFace, gx: usize, gy: usize) -> Mesh {
    match view {
        Some(p) => build_chunk_mesh(
            face, gx, gy, MOON_DIVISIONS,
            p.radius, p.sea_level, p.terrain_height, p.seed, p.noise_scale, p.detail_scale,
            LodLevel::Lod2, p.climate(), p.hydrology.hydro, p.atmosphere, p.geology.relief, p.biomes,
        ),
        None => build_celestial_chunk_mesh(
            face, gx, gy, MOON_DIVISIONS,
            mcfg.radius, mcfg.radius * 0.03, mcfg.seed, 1.5,
            [0.45, 0.44, 0.42, 1.0], [0.7, 0.68, 0.65, 1.0],
            LodLevel::Lod2,
        ),
    }
}

/// Tous les chunks lointains d'une lune, construits en parallèle sur tous les cœurs.
fn build_moon_chunks(mcfg: &crate::settings::MoonConfig, view: Option<&PlanetConfig>) -> Vec<Mesh> {
    let mut jobs = Vec::with_capacity(6 * MOON_DIVISIONS * MOON_DIVISIONS);
    for face in CubeFace::all() {
        for gx in 0..MOON_DIVISIONS {
            for gy in 0..MOON_DIVISIONS {
                jobs.push((face, gx, gy));
            }
        }
    }
    ComputeTaskPool::get().scope(|scope| {
        for (face, gx, gy) in jobs {
            scope.spawn(async move { moon_chunk_mesh(mcfg, view, face, gx, gy) });
        }
    })
}

/// Anneau plat (plan XZ) entre deux rayons, normales vers le haut.
fn annulus_mesh(inner: f32, outer: f32, segments: usize) -> Mesh {
    let mut positions = Vec::with_capacity(segments * 2 + 2);
    let mut normals = Vec::with_capacity(segments * 2 + 2);
    let mut uvs = Vec::with_capacity(segments * 2 + 2);
    let mut indices = Vec::with_capacity(segments * 6);
    for i in 0..=segments {
        let a = i as f32 / segments as f32 * std::f32::consts::TAU;
        let (c, s) = (a.cos(), a.sin());
        positions.push([c * inner, 0.0, s * inner]);
        positions.push([c * outer, 0.0, s * outer]);
        normals.push([0.0, 1.0, 0.0]);
        normals.push([0.0, 1.0, 0.0]);
        uvs.push([0.0, i as f32 / segments as f32]);
        uvs.push([1.0, i as f32 / segments as f32]);
    }
    for i in 0..segments as u32 {
        let k = i * 2;
        indices.extend_from_slice(&[k, k + 1, k + 2, k + 1, k + 3, k + 2]);
    }
    let mut mesh = Mesh::new(bevy::render::mesh::PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(bevy::render::mesh::Indices::U32(indices));
    mesh
}

/// Aurore : rideau lumineux qui ondule (intensité de base, déphasage).
#[derive(Component)]
pub struct AuroraGlow {
    pub base: LinearRgba,
    pub phase: f32,
}

/// Anneaux et aurores d'une planète (phase 9), inclinés comme son axe.
fn spawn_ring_and_aurora(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    pcfg: &PlanetConfig,
    root: Entity,
) {
    let tilt = Quat::from_rotation_x(pcfg.axial_tilt.to_radians());
    if let Some(ring) = pcfg.ring {
        let c = ring.color;
        let material = materials.add(StandardMaterial {
            base_color: Color::srgba(c[0], c[1], c[2], ring.opacity),
            alpha_mode: AlphaMode::Blend,
            cull_mode: None,
            double_sided: true,
            perceptual_roughness: 1.0,
            ..default()
        });
        let child = commands
            .spawn((Mesh3d(meshes.add(annulus_mesh(ring.inner, ring.outer, 160))), MeshMaterial3d(material), Transform::from_rotation(tilt), NotShadowCaster))
            .id();
        commands.entity(root).add_child(child);
    }
    if let Some(aurora) = pcfg.aurora {
        let r = pcfg.radius * 1.04;
        let lat = aurora.latitude.to_radians();
        let band = 0.07;
        let (inner, outer) = (r * (lat + band).cos(), r * (lat - band).cos());
        let c = aurora.color;
        let base = LinearRgba::new(c[0] * 3.0 * aurora.strength, c[1] * 3.0 * aurora.strength, c[2] * 3.0 * aurora.strength, 1.0);
        for (k, side) in [1.0f32, -1.0].into_iter().enumerate() {
            let material = materials.add(StandardMaterial {
                base_color: Color::srgba(c[0], c[1], c[2], 0.35 * aurora.strength),
                emissive: base,
                unlit: true,
                alpha_mode: AlphaMode::Add,
                cull_mode: None,
                double_sided: true,
                ..default()
            });
            let offset = tilt * Vec3::new(0.0, side * r * lat.sin(), 0.0);
            let child = commands
                .spawn((
                    Mesh3d(meshes.add(annulus_mesh(inner, outer, 96))),
                    MeshMaterial3d(material),
                    Transform::from_translation(offset).with_rotation(tilt),
                    NotShadowCaster,
                    AuroraGlow { base, phase: k as f32 * 1.7 + (pcfg.seed % 100) as f32 * 0.1 },
                ))
                .id();
            commands.entity(root).add_child(child);
        }
    }
}

/// Les aurores ondulent.
fn shimmer_auroras(time: Res<Time>, glows: Query<(&AuroraGlow, &MeshMaterial3d<StandardMaterial>)>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let t = time.elapsed_secs();
    for (glow, mat) in &glows {
        if let Some(m) = materials.get_mut(&mat.0) {
            let k = 0.55 + 0.45 * (t * 0.8 + glow.phase).sin() * (t * 0.31 + glow.phase * 2.0).cos();
            m.emissive = LinearRgba::new(glow.base.red * k, glow.base.green * k, glow.base.blue * k, 1.0);
        }
    }
}

/// Maillages d'une planète : sphère à bandes pour une géante (pas de relief ni de nuages), sinon
/// les chunks de terrain (avec leur niveau de détail) et la couche de nuages.
#[allow(clippy::too_many_arguments)]
fn spawn_planet_meshes(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    pcfg: &PlanetConfig,
    planet_id: usize,
    root: Entity,
    planet_world_pos: Vec3,
    cam_local: Vec3,
    divs: usize,
    rock_material: &Handle<StandardMaterial>,
) {
    spawn_ring_and_aurora(commands, meshes, materials, pcfg, root);
    if pcfg.gaseous() {
        // Visible des deux côtés : on peut y plonger
        let material = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.85,
            cull_mode: None,
            double_sided: true,
            ..default()
        });
        let mesh = build_gas_giant_mesh(pcfg.radius, pcfg.seed, &gas_look(pcfg), GAS_RESOLUTION);
        let child = commands
            .spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(material), Transform::IDENTITY, FarMesh))
            .id();
        commands.entity(root).add_child(child);
        return;
    }
    for (face, gx, gy, lod, mesh) in build_planet_chunks(pcfg, divs, cam_local) {
        let child = commands
            .spawn((
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(rock_material.clone()),
                Transform::IDENTITY,
                PlanetChunk { face, grid_x: gx, grid_y: gy, current_lod: lod, planet_id },
                LodChunk,
                FarMesh,
            ))
            .id();
        commands.entity(root).add_child(child);
    }
    if pcfg.atmosphere {
        spawn_cloud_layer(commands, meshes, materials, pcfg, planet_id, planet_world_pos);
    }
}

/// Carreaux par face de la sphère d'une géante.
const GAS_RESOLUTION: usize = 64;

/// Construit tous les chunks d'une planète en parallèle sur tous les cœurs.
fn build_planet_chunks(
    pcfg: &PlanetConfig,
    divs: usize,
    cam_local: Vec3,
) -> Vec<(CubeFace, usize, usize, LodLevel, Mesh)> {
    let (climate, hydro, atmosphere, relief, biomes) = (pcfg.climate(), pcfg.hydrology.hydro, pcfg.atmosphere, pcfg.geology.relief, pcfg.biomes);
    let mut jobs = Vec::with_capacity(6 * divs * divs);
    for face in CubeFace::all() {
        for gx in 0..divs {
            for gy in 0..divs {
                let u = (gx as f32 + 0.5) / divs as f32;
                let v = (gy as f32 + 0.5) / divs as f32;
                let chunk_center = face.to_sphere_pos(u, v) * pcfg.radius;
                jobs.push((face, gx, gy, compute_lod_level(cam_local, chunk_center, pcfg.radius)));
            }
        }
    }
    ComputeTaskPool::get().scope(|scope| {
        for (face, gx, gy, lod) in jobs {
            scope.spawn(async move {
                let mesh = build_chunk_mesh(
                    face, gx, gy, divs,
                    pcfg.radius, pcfg.sea_level, pcfg.terrain_height,
                    pcfg.seed, pcfg.noise_scale, pcfg.detail_scale, lod, climate, hydro, atmosphere, relief, biomes,
                );
                (face, gx, gy, lod, mesh)
            });
        }
    })
}

/// Maillage vide (un cube) pour les éruptions, remplacé à chaque image.
fn spawn_flare_mesh(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    star_idx: usize,
    (r, g, b): (f32, f32, f32),
    star_pos: Vec3,
) {
    let flare_material = materials.add(StandardMaterial {
        base_color: Color::srgb(r * 1.0, g * 0.7, b * 0.4),
        emissive: LinearRgba::new(r * 2.0, g * 1.2, b * 0.5, 1.0),
        unlit: true,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Mesh::from(Cuboid::new(1.0, 1.0, 1.0)))),
        MeshMaterial3d(flare_material),
        Transform::from_translation(star_pos),
        Visibility::Hidden,
        FlareVoxel { star_idx },
        NotShadowCaster,
        // Le maillage change à chaque image : sa boîte englobante initiale serait fausse
        NoFrustumCulling,
    ));
}

fn spawn_system_bodies(
    commands: &mut Commands,
    sys: &crate::settings::StarSystemConfig,
    settings: &GameSettings,
    sys_idx: usize,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    cam_pos: Vec3,
    center: Vec3,
) {
    let id_base = sys_idx * 1000;
    let star_lod = LodLevel::Lod0;

    for (i, star_cfg) in sys.stars.iter().enumerate() {
        let r = star_cfg.light_color_r;
        let g = star_cfg.light_color_g;
        let b = star_cfg.light_color_b;
        let star_material = materials.add(StandardMaterial {
            base_color: Color::srgb(r, g, b),
            emissive: { let [er, eg, eb] = star_cfg.emissive_rgb(); LinearRgba::new(er, eg, eb, 1.0) },
            unlit: true,
            ..default()
        });
        let star_color_low: [f32; 4] = [r * 0.85, g * 0.4, b * 0.15, 1.0];
        let star_color_high: [f32; 4] = [r, g, b, 1.0];
        let pos = center + if star_cfg.orbit_distance > 1.0 {
            Vec3::new(star_cfg.orbit_distance, 0.0, 0.0)
        } else {
            Vec3::ZERO
        };

        let star_entity = commands
            .spawn((
                Transform::from_translation(pos),
                Visibility::default(),
                StarRoot,
                StarId(id_base + i),
                SystemIdx(sys_idx),
                SystemOffset(center),
                AstreLodRoot { cull_dist: 50000.0, radius: star_cfg.radius, streamable: true, label: "Star" },
            ))
            .id();

        for face in CubeFace::all() {
            for gx in 0..STAR_DIVISIONS {
                for gy in 0..STAR_DIVISIONS {
                    let mesh = build_celestial_chunk_mesh(
                        face, gx, gy, STAR_DIVISIONS,
                        star_cfg.radius, (star_cfg.radius * 0.004).max(5.0),
                        99 + i as u32, 2.0,
                        star_color_low, star_color_high, star_lod,
                    );
                    let child = commands
                        .spawn((
                            Mesh3d(meshes.add(mesh)),
                            MeshMaterial3d(star_material.clone()),
                            Transform::IDENTITY,
                            NotShadowCaster,
                            StarChunk,
                        ))
                        .id();
                    commands.entity(star_entity).add_child(child);
                }
            }
        }

        let beacon_radius = star_cfg.radius * 0.8;
        let beacon = commands
            .spawn((
                Mesh3d(meshes.add(Sphere::new(beacon_radius).mesh().ico(3).unwrap())),
                MeshMaterial3d(star_material.clone()),
                Transform::IDENTITY,
                Visibility::Hidden,
                NotShadowCaster,
                StarBeacon,
            ))
            .id();
        commands.entity(star_entity).add_child(beacon);

        let r = star_cfg.light_color_r;
        let g = star_cfg.light_color_g;
        let b = star_cfg.light_color_b;
        let intensity = star_cfg.lumens();
        let light = commands
            .spawn((
                PointLight {
                    intensity,
                    range: star_cfg.light_range,
                    shadows_enabled: false,
                    color: Color::srgb(r, g, b),
                    ..default()
                },
                Transform::IDENTITY,
            ))
            .id();
        commands.entity(star_entity).add_child(light);

        spawn_flare_mesh(commands, meshes, materials, id_base + i, (r, g, b), pos);
    }

    let planet_material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.9,
        ..default()
    });
    let divs = settings.planet_chunk_divisions;

    for (i, pcfg) in sys.planets().iter().enumerate() {
        let planet_world_pos = center + Vec3::new(pcfg.orbit_distance, 0.0, 0.0);
        let cam_local = cam_pos - planet_world_pos;

        let root = commands
            .spawn((
                Transform::from_translation(planet_world_pos),
                Visibility::default(),
                PlanetRoot,
                PlanetId(id_base + i),
                SystemIdx(sys_idx),
                SystemOffset(center),
                AstreLodRoot { cull_dist: 20000.0, radius: pcfg.radius, streamable: true, label: "Planet" },
            ))
            .id();

        spawn_planet_meshes(commands, meshes, materials, pcfg, id_base + i, root, planet_world_pos, cam_local, divs, &planet_material);

        for (mi, mcfg) in pcfg.moons.iter().enumerate() {
            let moon_view = mcfg.generated().then(|| mcfg.as_planet(pcfg));
            let offset = Vec3::new(mcfg.orbit_distance, 0.0, 0.0);
            let moon_root = commands
                .spawn((
                    Transform::from_translation(planet_world_pos + offset),
                    Visibility::default(),
                    MoonRoot,
                    MoonId { planet_idx: id_base + i, moon_idx: mi },
                    SystemIdx(sys_idx),
                    SystemOffset(center),
                    AstreLodRoot { cull_dist: 10000.0, radius: mcfg.radius, streamable: true, label: "Moon" },
                ))
                .id();

            for mesh in build_moon_chunks(mcfg, moon_view.as_ref()) {
                let child = commands
                    .spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(planet_material.clone()), Transform::IDENTITY, FarMesh))
                    .id();
                commands.entity(moon_root).add_child(child);
            }
        }
    }

    let asteroid_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.45, 0.42, 0.38),
        perceptual_roughness: 0.95,
        ..default()
    });
    let mut rng = rand::thread_rng();

    for (i, belt) in sys.asteroid_belts.iter().enumerate() {
        let belt_entity = commands
            .spawn((
                Transform::from_translation(center),
                Visibility::default(),
                AsteroidBeltRoot,
                AsteroidBeltId(id_base + i),
                SystemIdx(sys_idx),
                SystemOffset(center),
                AstreLodRoot { cull_dist: 30000.0, radius: belt.distance, streamable: true, label: "AsteroidBelt" },
            ))
            .id();

        for _ in 0..belt.count.min(500) {
            let angle = rng.gen::<f32>() * std::f32::consts::TAU;
            let dist_offset = (rng.gen::<f32>() - 0.5) * belt.width;
            let dist = belt.distance + dist_offset;
            let y_offset = (rng.gen::<f32>() - 0.5) * belt.width * 0.3;
            let size = belt.min_size + rng.gen::<f32>() * (belt.max_size - belt.min_size);
            let pos = Vec3::new(angle.cos() * dist, y_offset, angle.sin() * dist);
            let rotation = Quat::from_euler(
                EulerRot::XYZ,
                rng.gen::<f32>() * std::f32::consts::TAU,
                rng.gen::<f32>() * std::f32::consts::TAU,
                rng.gen::<f32>() * std::f32::consts::TAU,
            );
            let mesh = Mesh::from(Cuboid::new(size, size * 0.7, size * 0.85));
            let asteroid = commands
                .spawn((
                    Mesh3d(meshes.add(mesh)),
                    MeshMaterial3d(asteroid_material.clone()),
                    Transform::from_translation(pos).with_rotation(rotation),
                ))
                .id();
            commands.entity(belt_entity).add_child(asteroid);
        }
    }
}

pub(crate) fn orbit_planets(
    time: Res<Time>,
    clock: Res<UniverseClock>,
    settings: Res<GameSettings>,
    mut planet_q: Query<(&mut Transform, &PlanetId, &SystemIdx), With<PlanetRoot>>,
) {
    let t = clock.secs(&time);
    for (mut tf, pid, si) in &mut planet_q {
        let Some(sys) = settings.systems.get(si.0) else { continue };
        let local_idx = pid.0 - si.0 * 1000;
        let Some(cfg) = sys.planets().get(local_idx) else { continue };
        let sc = sys.center();
        let elems = OrbitalElements {
            a: cfg.orbit_distance,
            e: cfg.eccentricity,
            i: cfg.inclination,
            omega_big: cfg.ascending_node,
            omega: cfg.arg_periapsis,
            m0: cfg.mean_anomaly_0,
        };
        let pos = elems.position(t, DEFAULT_MU * PLANET_MU_SCALE);
        tf.translation = sc + pos;
    }
}

fn orbit_stars(
    time: Res<Time>,
    clock: Res<UniverseClock>,
    settings: Res<GameSettings>,
    mut star_q: Query<(&mut Transform, &StarId, &SystemIdx), With<StarRoot>>,
) {
    let t = clock.secs(&time);
    for (mut tf, sid, si) in &mut star_q {
        let Some(sys) = settings.systems.get(si.0) else { continue };
        let local_idx = sid.0 - si.0 * 1000;
        let Some(cfg) = sys.stars.get(local_idx) else { continue };
        if cfg.orbit_distance > 1.0 {
            let sc = sys.center();
            let sn = sys.stars.len().max(1) as f32;
            let elems = OrbitalElements::circular(
                cfg.orbit_distance,
                local_idx as f32 * std::f32::consts::TAU / sn,
            );
            let pos = elems.position(t, DEFAULT_MU);
            tf.translation = sc + pos;
        }
    }
}

pub(crate) fn orbit_moons(
    time: Res<Time>,
    clock: Res<UniverseClock>,
    settings: Res<GameSettings>,
    planet_q: Query<(&GlobalTransform, &PlanetId), With<PlanetRoot>>,
    mut moon_q: Query<(&mut Transform, &MoonId, &SystemIdx), With<MoonRoot>>,
) {
    let t = clock.secs(&time);
    let moon_mu = DEFAULT_MU * 0.001;
    for (mut tf, mid, si) in &mut moon_q {
        let planet_pos = planet_q
            .iter()
            .find(|(_, pid)| pid.0 == mid.planet_idx)
            .map(|(gt, _)| gt.translation())
            .unwrap_or_default();

        let local_planet = mid.planet_idx - si.0 * 1000;
        let mcfg = settings.systems.get(si.0)
            .and_then(|sys| sys.planets().get(local_planet))
            .and_then(|p| p.moons.get(mid.moon_idx));

        if let Some(mcfg) = mcfg {
            let elems = OrbitalElements {
                a: mcfg.orbit_distance,
                e: mcfg.eccentricity,
                i: mcfg.inclination,
                omega_big: mcfg.ascending_node,
                omega: mcfg.arg_periapsis,
                m0: mcfg.mean_anomaly_0,
            };
            let pos = elems.position(t, moon_mu);
            tf.translation = planet_pos + pos;
        }
    }
}

fn orbit_asteroid_belts(
    time: Res<Time>,
    mut belt_q: Query<&mut Transform, With<AsteroidBeltRoot>>,
) {
    for mut tf in &mut belt_q {
        tf.rotate_y(time.delta_secs() * 0.005);
    }
}

fn snap_grid(v: Vec3, grid: f32) -> Vec3 {
    Vec3::new(
        (v.x / grid).round() * grid,
        (v.y / grid).round() * grid,
        (v.z / grid).round() * grid,
    )
}

fn pseudo_hash(a: f32, b: f32) -> f32 {
    ((a * 12.9898 + b * 78.233).sin() * 43758.5453).fract()
}

/// Ajoute un cube (centre, demi-taille) à un maillage en construction.
fn push_cube(c: Vec3, h: f32, positions: &mut Vec<[f32; 3]>, normals: &mut Vec<[f32; 3]>, indices: &mut Vec<u32>) {
    const FACES: [([f32; 3], [[f32; 3]; 4]); 6] = [
        ([1.0, 0.0, 0.0], [[1.0, -1.0, 1.0], [1.0, -1.0, -1.0], [1.0, 1.0, -1.0], [1.0, 1.0, 1.0]]),
        ([-1.0, 0.0, 0.0], [[-1.0, -1.0, -1.0], [-1.0, -1.0, 1.0], [-1.0, 1.0, 1.0], [-1.0, 1.0, -1.0]]),
        ([0.0, 1.0, 0.0], [[-1.0, 1.0, 1.0], [1.0, 1.0, 1.0], [1.0, 1.0, -1.0], [-1.0, 1.0, -1.0]]),
        ([0.0, -1.0, 0.0], [[-1.0, -1.0, -1.0], [1.0, -1.0, -1.0], [1.0, -1.0, 1.0], [-1.0, -1.0, 1.0]]),
        ([0.0, 0.0, 1.0], [[-1.0, -1.0, 1.0], [1.0, -1.0, 1.0], [1.0, 1.0, 1.0], [-1.0, 1.0, 1.0]]),
        ([0.0, 0.0, -1.0], [[1.0, -1.0, -1.0], [-1.0, -1.0, -1.0], [-1.0, 1.0, -1.0], [1.0, 1.0, -1.0]]),
    ];
    for (n, corners) in FACES {
        let vi = positions.len() as u32;
        for k in corners {
            positions.push([c.x + k[0] * h, c.y + k[1] * h, c.z + k[2] * h]);
            normals.push(n);
        }
        indices.extend_from_slice(&[vi, vi + 1, vi + 2, vi, vi + 2, vi + 3]);
    }
}

const MAX_FLARES_PER_STAR: u32 = 32;

fn update_flare_voxels(
    time: Res<Time>,
    settings: Res<GameSettings>,
    star_q: Query<(&GlobalTransform, &StarId), With<StarRoot>>,
    cam_q: Query<&GlobalTransform, With<Camera3d>>,
    mut flare_q: Query<(&FlareVoxel, &Mesh3d, &mut Transform, &mut Visibility)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    if !settings.show_flares {
        for (_, _, _, mut vis) in &mut flare_q {
            if *vis != Visibility::Hidden { *vis = Visibility::Hidden; }
        }
        return;
    }

    let max_samples = 64_u32;
    let cam_pos = cam_q.iter().next().map(|gt| gt.translation()).unwrap_or_default();
    let star_positions: HashMap<usize, Vec3> =
        star_q.iter().map(|(gt, sid)| (sid.0, gt.translation())).collect();

    for (fv, mesh3d, mut tf, mut vis) in &mut flare_q {
        let sys_i = fv.star_idx / 1000;
        let local_i = fv.star_idx % 1000;
        let (Some(scfg), Some(&star_pos)) = (
            settings.systems.get(sys_i).and_then(|s| s.stars.get(local_i)),
            star_positions.get(&fv.star_idx),
        ) else {
            if *vis != Visibility::Hidden { *vis = Visibility::Hidden; }
            continue;
        };
        tf.translation = star_pos;

        // LOD: fewer samples when far away
        let dist = cam_pos.distance(star_pos);
        let ratio = dist / scfg.radius.max(1.0);
        let active_samples = if ratio < 2.0 {
            max_samples
        } else if ratio < 3.5 {
            48
        } else if ratio < 5.0 {
            32
        } else {
            16
        };

        let t = time.elapsed_secs() * scfg.flare_speed;
        // Les réglages d'éruption sont donnés pour une étoile de rayon 250
        let k = (scfg.radius / 250.0).max(1.0);
        let voxel_size = scfg.flare_size * k;
        let half = voxel_size * 0.5;

        let mut positions: Vec<[f32; 3]> = Vec::new();
        let mut normals: Vec<[f32; 3]> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();

        for flare_idx in 0..scfg.flare_count.min(MAX_FLARES_PER_STAR) {
            let base_seed = (fv.star_idx as f32 + 1.0) * 100.0 + flare_idx as f32 * 37.7;

            // Each flare has a unique cycle duration and pause
            let rise_dur = 2.0 + pseudo_hash(base_seed, 1.0) * 3.0;
            let hold_dur = 0.5 + pseudo_hash(base_seed, 2.0) * 1.5;
            let fall_dur = 2.0 + pseudo_hash(base_seed, 3.0) * 3.0;
            let pause_dur = 1.0 + pseudo_hash(base_seed, 4.0) * 4.0;
            let total_dur = rise_dur + hold_dur + fall_dur + pause_dur;
            let offset = pseudo_hash(base_seed, 5.0) * total_dur;
            let local_t = ((t + offset) % total_dur).max(0.0);

            let life = if local_t < rise_dur {
                local_t / rise_dur
            } else if local_t < rise_dur + hold_dur {
                1.0
            } else if local_t < rise_dur + hold_dur + fall_dur {
                1.0 - (local_t - rise_dur - hold_dur) / fall_dur
            } else {
                0.0
            };
            if life <= 0.001 {
                continue;
            }

            // Each cycle gets a different position
            let cycle_id = ((t + offset) / total_dur).floor();
            let h1 = pseudo_hash(base_seed, cycle_id * 3.1);
            let h2 = pseudo_hash(base_seed, cycle_id * 7.3);

            let theta = h1 * std::f32::consts::TAU;
            let phi = (h2 * 2.0 - 1.0).clamp(-1.0, 1.0).acos();

            let dir = Vec3::new(
                theta.cos() * phi.sin(),
                phi.cos(),
                theta.sin() * phi.sin(),
            )
            .normalize();

            let perp = if dir.y.abs() > 0.9 {
                dir.cross(Vec3::X).normalize()
            } else {
                dir.cross(Vec3::Y).normalize()
            };

            let height = scfg.flare_height * k * life;
            let half_spread = scfg.flare_distance.max(5.0) * k * 0.5;

            // Positions relatives à l'étoile (le maillage suit l'étoile)
            let start = (dir + perp * half_spread / scfg.radius).normalize() * scfg.radius;
            let end = (dir - perp * half_spread / scfg.radius).normalize() * scfg.radius;
            let control = dir * (scfg.radius + height);

            for sample_idx in 0..active_samples {
                let frac = sample_idx as f32 / active_samples as f32;
                if frac > life {
                    break;
                }
                let inv = 1.0 - frac;
                let point = start * inv * inv + control * 2.0 * inv * frac + end * frac * frac;
                let snapped = snap_grid(point, voxel_size.max(1.0));
                push_cube(snapped, half, &mut positions, &mut normals, &mut indices);
            }
        }

        if positions.is_empty() {
            if *vis != Visibility::Hidden { *vis = Visibility::Hidden; }
            continue;
        }
        if *vis != Visibility::Inherited { *vis = Visibility::Inherited; }

        let mut mesh = Mesh::new(
            bevy::render::mesh::PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        mesh.insert_indices(bevy::render::mesh::Indices::U32(indices));
        if let Some(m) = meshes.get_mut(&mesh3d.0) {
            *m = mesh;
        }
    }
}

fn rotate_clouds(
    time: Res<Time>,
    settings: Res<GameSettings>,
    planet_q: Query<(&GlobalTransform, &PlanetId), With<PlanetRoot>>,
    mut cloud_q: Query<(&CloudVoxel, &mut Transform, &mut Visibility)>,
) {
    if !settings.show_clouds {
        for (_, _, mut vis) in &mut cloud_q {
            if *vis != Visibility::Hidden { *vis = Visibility::Hidden; }
        }
        return;
    }

    let t = time.elapsed_secs();
    let planet_positions: HashMap<usize, Vec3> =
        planet_q.iter().map(|(gt, pid)| (pid.0, gt.translation())).collect();

    for (cloud, mut tf, mut vis) in &mut cloud_q {
        if *vis == Visibility::Hidden { *vis = Visibility::Inherited; }
        let sys_i = cloud.planet_idx / 1000;
        let local_i = cloud.planet_idx % 1000;
        let Some(pcfg) = settings.systems.get(sys_i).and_then(|s| s.planets().get(local_i)) else {
            continue;
        };
        let Some(&planet_pos) = planet_positions.get(&cloud.planet_idx) else { continue };

        // Toute la couche tourne d'un bloc autour de l'axe Y de la planète
        tf.translation = planet_pos;
        tf.rotation = Quat::from_rotation_y(-t * pcfg.cloud_speed);
    }
}

fn regenerate_all(
    mut commands: Commands,
    settings: Res<GameSettings>,
    mut events: EventReader<RegeneratePlanet>,
    planet_q: Query<Entity, With<PlanetRoot>>,
    star_q: Query<Entity, With<StarRoot>>,
    moon_q: Query<Entity, With<MoonRoot>>,
    flare_q: Query<Entity, With<FlareVoxel>>,
    cloud_q: Query<Entity, With<CloudVoxel>>,
    belt_q: Query<Entity, With<AsteroidBeltRoot>>,
    camera_q: Query<&Transform, With<Camera3d>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    spawned: Res<SpawnedSystems>,
) {
    let mut fired = false;
    for _ in events.read() {
        fired = true;
    }
    if !fired {
        return;
    }

    let cam_pos = camera_q.single().translation;
    for entity in &planet_q {
        commands.entity(entity).despawn_recursive();
    }
    for entity in &star_q {
        commands.entity(entity).despawn_recursive();
    }
    for entity in &moon_q {
        commands.entity(entity).despawn_recursive();
    }
    for entity in &flare_q {
        commands.entity(entity).despawn_recursive();
    }
    for entity in &cloud_q {
        commands.entity(entity).despawn_recursive();
    }
    for entity in &belt_q {
        commands.entity(entity).despawn_recursive();
    }
    // Seul le système chargé est régénéré (les autres ne sont pas instanciés)
    for &si in &spawned.0 {
        let Some(sys) = settings.systems.get(si) else { continue };
        spawn_system_bodies(
            &mut commands,
            sys,
            &settings,
            si,
            &mut meshes,
            &mut materials,
            cam_pos,
            sys.center(),
        );
    }
}

/// Nombre max de chunks reconstruits en même temps en arrière-plan.
const MAX_LOD_TASKS_IN_FLIGHT: usize = 8;

/// Reconstruction d'un chunk de planète en cours sur le pool de threads async.
#[derive(Component)]
pub struct LodTask {
    task: Task<Mesh>,
    lod: LodLevel,
}

fn update_lod(
    mut commands: Commands,
    surface: Res<Surface>,
    settings: Res<GameSettings>,
    camera_q: Query<&Transform, With<Camera3d>>,
    planet_q: Query<(&GlobalTransform, &PlanetId), With<PlanetRoot>>,
    mut chunk_q: Query<(Entity, &mut PlanetChunk, &Mesh3d, Option<&mut LodTask>), With<LodChunk>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let cam_world = camera_q.single().translation;
    let divs = settings.planet_chunk_divisions;
    let planet_positions: HashMap<usize, Vec3> =
        planet_q.iter().map(|(gt, pid)| (pid.0, gt.translation())).collect();

    // 1. Récupère les maillages terminés (sans jamais bloquer l'image)
    let mut in_flight = 0;
    for (entity, mut chunk, mesh_handle, task) in chunk_q.iter_mut() {
        let Some(mut task) = task else { continue };
        if let Some(new_mesh) = block_on(future::poll_once(&mut task.task)) {
            if let Some(mesh) = meshes.get_mut(&mesh_handle.0) {
                *mesh = new_mesh;
            }
            chunk.current_lod = task.lod;
            commands.entity(entity).remove::<LodTask>();
        } else {
            in_flight += 1;
        }
    }

    // 2. Lance les reconstructions nécessaires en arrière-plan
    let pool = AsyncComputeTaskPool::get();
    for (entity, chunk, _, task) in chunk_q.iter_mut() {
        if in_flight >= MAX_LOD_TASKS_IN_FLIGHT {
            break;
        }
        if task.is_some() {
            continue;
        }
        // Planète où l'on se pose : le terrain voxel remplace ce maillage
        if surface.active_planet() == Some(chunk.planet_id) {
            continue;
        }

        let sys_i = chunk.planet_id / 1000;
        let local_i = chunk.planet_id % 1000;
        let Some(pcfg) = settings.systems.get(sys_i).and_then(|s| s.planets().get(local_i)) else {
            continue;
        };
        let planet_pos = planet_positions.get(&chunk.planet_id).copied().unwrap_or_default();

        let cam_local = cam_world - planet_pos;
        let u = (chunk.grid_x as f32 + 0.5) / divs as f32;
        let v = (chunk.grid_y as f32 + 0.5) / divs as f32;
        let center = chunk.face.to_sphere_pos(u, v) * pcfg.radius;
        let new_lod = compute_lod_level(cam_local, center, pcfg.radius);

        if new_lod != chunk.current_lod {
            let (face, gx, gy) = (chunk.face, chunk.grid_x, chunk.grid_y);
            let (radius, sea, height, seed, noise, detail) = (
                pcfg.radius, pcfg.sea_level, pcfg.terrain_height,
                pcfg.seed, pcfg.noise_scale, pcfg.detail_scale,
            );
            let (climate, hydro, atmosphere, relief, biomes) = (pcfg.climate(), pcfg.hydrology.hydro, pcfg.atmosphere, pcfg.geology.relief, pcfg.biomes);
            let task = pool.spawn(async move {
                build_chunk_mesh(face, gx, gy, divs, radius, sea, height, seed, noise, detail, new_lod, climate, hydro, atmosphere, relief, biomes)
            });
            // `try_insert` : le morceau a pu disparaître dans la même image (système quitté,
            // téléportation `/aller`) ; un `insert` ferait planter le jeu
            commands.entity(entity).try_insert(LodTask { task, lod: new_lod });
            in_flight += 1;
        }
    }
}

/// L'étoile est dessinée en détail tant que la caméra est à moins de 6 rayons, sinon une sphère lisse.
const STAR_DETAIL_RADII: f32 = 6.0;

fn update_star_visibility(
    camera_q: Query<&GlobalTransform, With<Camera3d>>,
    star_q: Query<(&GlobalTransform, &Children, &AstreLodRoot), With<StarRoot>>,
    mut chunk_vis_q: Query<&mut Visibility, (With<StarChunk>, Without<StarBeacon>)>,
    mut beacon_q: Query<(&mut Visibility, &mut Transform), (With<StarBeacon>, Without<StarChunk>)>,
) {
    let cam_pos = camera_q.single().translation();

    for (star_gt, children, lod) in &star_q {
        let dist = cam_pos.distance(star_gt.translation());
        let detail_dist = (lod.radius * STAR_DETAIL_RADII).max(1000.0);
        let far = dist > detail_dist;

        for &child in children.iter() {
            if let Ok(mut vis) = chunk_vis_q.get_mut(child) {
                *vis = if far { Visibility::Hidden } else { Visibility::Inherited };
            }
            if let Ok((mut vis, mut tf)) = beacon_q.get_mut(child) {
                *vis = if far { Visibility::Inherited } else { Visibility::Hidden };
                if far {
                    // De loin la boule ne rétrécit pas sous ~0,25 % de la distance : l'étoile reste un point visible
                    let scale_factor = (dist * 0.0025 / (lod.radius * 0.8).max(1.0)).max(1.0);
                    tf.scale = Vec3::splat(scale_factor);
                } else {
                    tf.scale = Vec3::ONE;
                }
            }
        }
    }
}

fn cleanup_hidden_toplevel(
    mut commands: Commands,
    hidden_stars: Query<&StarId, With<crate::astre::AstreUnloaded>>,
    hidden_planets: Query<&PlanetId, With<crate::astre::AstreUnloaded>>,
    flare_q: Query<(Entity, &FlareVoxel)>,
    cloud_q: Query<(Entity, &CloudVoxel)>,
) {
    for sid in &hidden_stars {
        for (e, fv) in &flare_q {
            if fv.star_idx == sid.0 {
                commands.entity(e).despawn_recursive();
            }
        }
    }
    for pid in &hidden_planets {
        for (e, cv) in &cloud_q {
            if cv.planet_idx == pid.0 {
                commands.entity(e).despawn_recursive();
            }
        }
    }
}

fn reload_stars(
    mut commands: Commands,
    mut events: EventReader<ReloadAstre>,
    settings: Res<GameSettings>,
    roots: Query<(Entity, &StarId, &SystemIdx, &GlobalTransform), With<StarRoot>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for ev in events.read() {
        let Ok((entity, sid, si, gt)) = roots.get(ev.0) else { continue };
        let local_i = sid.0 % 1000;
        let Some(sys) = settings.systems.get(si.0) else { continue };
        let Some(star_cfg) = sys.stars.get(local_i) else { continue };

        let r = star_cfg.light_color_r;
        let g = star_cfg.light_color_g;
        let b = star_cfg.light_color_b;
        let star_material = materials.add(StandardMaterial {
            base_color: Color::srgb(r, g, b),
            emissive: { let [er, eg, eb] = star_cfg.emissive_rgb(); LinearRgba::new(er, eg, eb, 1.0) },
            unlit: true,
            ..default()
        });
        let star_color_low: [f32; 4] = [r * 0.85, g * 0.4, b * 0.15, 1.0];
        let star_color_high: [f32; 4] = [r, g, b, 1.0];
        let star_lod = LodLevel::Lod0;

        for face in CubeFace::all() {
            for gx in 0..STAR_DIVISIONS {
                for gy in 0..STAR_DIVISIONS {
                    let mesh = build_celestial_chunk_mesh(
                        face, gx, gy, STAR_DIVISIONS,
                        star_cfg.radius, (star_cfg.radius * 0.004).max(5.0),
                        99 + local_i as u32, 2.0,
                        star_color_low, star_color_high, star_lod,
                    );
                    let child = commands.spawn((
                        Mesh3d(meshes.add(mesh)),
                        MeshMaterial3d(star_material.clone()),
                        Transform::IDENTITY,
                        NotShadowCaster,
                        StarChunk,
                    )).id();
                    commands.entity(entity).add_child(child);
                }
            }
        }

        let beacon_radius = star_cfg.radius * 0.8;
        let beacon = commands.spawn((
            Mesh3d(meshes.add(Sphere::new(beacon_radius).mesh().ico(3).unwrap())),
            MeshMaterial3d(star_material.clone()),
            Transform::IDENTITY,
            Visibility::Hidden,
            NotShadowCaster,
            StarBeacon,
        )).id();
        commands.entity(entity).add_child(beacon);

        let intensity = star_cfg.lumens();
        let light = commands.spawn((
            PointLight {
                intensity,
                range: star_cfg.light_range,
                shadows_enabled: true,
                shadow_depth_bias: 0.02,
                shadow_normal_bias: 1.0,
                color: Color::srgb(r, g, b),
                ..default()
            },
            Transform::IDENTITY,
        )).id();
        commands.entity(entity).add_child(light);

        spawn_flare_mesh(&mut commands, &mut meshes, &mut materials, sid.0, (r, g, b), gt.translation());
    }
}

fn reload_planets(
    mut commands: Commands,
    mut events: EventReader<ReloadAstre>,
    settings: Res<GameSettings>,
    roots: Query<(Entity, &PlanetId, &SystemIdx, &GlobalTransform), With<PlanetRoot>>,
    camera_q: Query<&Transform, With<Camera3d>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for ev in events.read() {
        let Ok((entity, pid, si, gt)) = roots.get(ev.0) else { continue };
        let local_i = pid.0 % 1000;
        let Some(sys) = settings.systems.get(si.0) else { continue };
        let Some(pcfg) = sys.planets().get(local_i) else { continue };

        let planet_material = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.9,
            ..default()
        });

        let planet_world_pos = gt.translation();
        let cam_pos = camera_q.single().translation;
        let cam_local = cam_pos - planet_world_pos;
        let divs = settings.planet_chunk_divisions;
        spawn_planet_meshes(&mut commands, &mut meshes, &mut materials, pcfg, pid.0, entity, planet_world_pos, cam_local, divs, &planet_material);
    }
}

fn reload_moons(
    mut commands: Commands,
    mut events: EventReader<ReloadAstre>,
    settings: Res<GameSettings>,
    roots: Query<(Entity, &MoonId, &SystemIdx), With<MoonRoot>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for ev in events.read() {
        let Ok((entity, mid, si)) = roots.get(ev.0) else { continue };
        let local_planet = mid.planet_idx % 1000;
        let Some(sys) = settings.systems.get(si.0) else { continue };
        let Some(pcfg) = sys.planets().get(local_planet) else { continue };
        let Some(mcfg) = pcfg.moons.get(mid.moon_idx) else { continue };

        let planet_material = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.9,
            ..default()
        });
        let moon_view = mcfg.generated().then(|| mcfg.as_planet(pcfg));

        for mesh in build_moon_chunks(mcfg, moon_view.as_ref()) {
            let child = commands
                .spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(planet_material.clone()), Transform::IDENTITY, FarMesh))
                .id();
            commands.entity(entity).add_child(child);
        }
    }
}

fn reload_asteroid_belts(
    mut commands: Commands,
    mut events: EventReader<ReloadAstre>,
    settings: Res<GameSettings>,
    roots: Query<(Entity, &AsteroidBeltId, &SystemIdx), With<AsteroidBeltRoot>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    use rand::Rng;

    for ev in events.read() {
        let Ok((entity, bid, si)) = roots.get(ev.0) else { continue };
        let local_i = bid.0 % 1000;
        let Some(sys) = settings.systems.get(si.0) else { continue };
        let Some(belt) = sys.asteroid_belts.get(local_i) else { continue };

        let asteroid_material = materials.add(StandardMaterial {
            base_color: Color::srgb(0.45, 0.42, 0.38),
            perceptual_roughness: 0.95,
            ..default()
        });
        let mut rng = rand::thread_rng();

        for _ in 0..belt.count.min(500) {
            let angle = rng.gen::<f32>() * std::f32::consts::TAU;
            let dist_offset = (rng.gen::<f32>() - 0.5) * belt.width;
            let dist = belt.distance + dist_offset;
            let y_offset = (rng.gen::<f32>() - 0.5) * belt.width * 0.3;
            let size = belt.min_size + rng.gen::<f32>() * (belt.max_size - belt.min_size);
            let pos = Vec3::new(angle.cos() * dist, y_offset, angle.sin() * dist);
            let rotation = Quat::from_euler(
                EulerRot::XYZ,
                rng.gen::<f32>() * std::f32::consts::TAU,
                rng.gen::<f32>() * std::f32::consts::TAU,
                rng.gen::<f32>() * std::f32::consts::TAU,
            );
            let mesh = Mesh::from(Cuboid::new(size, size * 0.7, size * 0.85));
            let asteroid = commands
                .spawn((
                    Mesh3d(meshes.add(mesh)),
                    MeshMaterial3d(asteroid_material.clone()),
                    Transform::from_translation(pos).with_rotation(rotation),
                ))
                .id();
            commands.entity(entity).add_child(asteroid);
        }
    }
}

fn update_far_star_scale(
    camera_q: Query<(&GlobalTransform, &Transform), With<Camera3d>>,
    spawned: Res<SpawnedSystems>,
    settings: Res<GameSettings>,
    brightness_mats: Res<StarBrightnessMaterials>,
    epoch: Res<crate::origin::OriginEpoch>,
    dim: Res<crate::surface::GalaxyDim>,
    mut sectors: ResMut<StarSectors>,
    mut far_q: Query<(&FarStar, &mut Transform, &mut Visibility, &mut MeshMaterial3d<StandardMaterial>), Without<Camera3d>>,
) {
    let (cam_gt, cam_tf) = camera_q.single();
    let cam_pos = cam_gt.translation();
    let cam_fwd = cam_tf.forward().as_vec3();
    let system_changed = spawned.is_changed();
    let keep = dim.0;

    // Fondu / luminosité calculés une fois par galaxie (distance caméra → centre galactique)
    let gal_lod: Vec<(f32, f32, usize)> = settings.galaxies.iter().enumerate().map(|(gid, g)| {
        let gal_dist = cam_pos.distance(g.center());
        let raw = ((gal_dist - LOD_STARS_END) / (LOD_STARS_GONE - LOD_STARS_END)).clamp(0.0, 1.0);
        let fade = smoothstep(raw);
        // La galaxie principale s'estompe près d'une planète ou d'une lune
        let near_body = if gid == 0 { keep } else { 1.0 };
        let brightness = (5_000_000.0 * GALAXY_SCALE / gal_dist.max(1.0)).clamp(0.05, 1.0) * (1.0 - fade) * near_body;
        let step = ((brightness * (STAR_BRIGHTNESS_STEPS - 1) as f32).round() as usize).min(STAR_BRIGHTNESS_STEPS - 1);
        (gal_dist, fade, step)
    }).collect();

    for sector in sectors.list.iter_mut() {
        let (gal_dist, lod_fade, step) = gal_lod.get(sector.galaxy as usize).copied().unwrap_or((f32::MAX, 1.0, 0));
        let center = crate::settings::to_local(sector.abs_center);
        let d = cam_pos.distance(center);

        // Tout le secteur est invisible : galaxie estompée, ou secteur lointain dans notre dos
        let behind = d > sector.radius * 2.0 + 1000.0 && cam_fwd.dot((center - cam_pos) / d) < -0.5;
        if lod_fade >= 1.0 || behind {
            if !sector.hidden {
                sector.hidden = true;
                for &e in &sector.members {
                    if let Ok((_, _, mut vis, _)) = far_q.get_mut(e) {
                        if *vis != Visibility::Hidden { *vis = Visibility::Hidden; }
                    }
                }
            }
            continue;
        }

        // Un secteur lointain bouge à peine dans le ciel : on ne le recalcule que si la caméra a
        // parcouru une fraction visible de sa distance (ou tourné, ou si le monde a été recentré)
        let reach = (d - sector.radius).max(1.0);
        let still = !sector.hidden
            && sector.last_epoch == epoch.0
            && !system_changed
            && cam_pos.distance(sector.last_cam) < reach * 0.003
            && cam_fwd.dot(sector.last_fwd) > 0.999
            && (keep - sector.last_keep).abs() < 0.03;
        if still {
            continue;
        }
        sector.hidden = false;
        sector.last_cam = cam_pos;
        sector.last_fwd = cam_fwd;
        sector.last_epoch = epoch.0;
        sector.last_keep = keep;

        for &e in &sector.members {
            let Ok((fs, mut tf, mut vis, mut mat)) = far_q.get_mut(e) else { continue };
            if spawned.0.contains(&fs.sys_idx) {
                if *vis != Visibility::Hidden { *vis = Visibility::Hidden; }
                continue;
            }

            let to_star = tf.translation - cam_pos;
            let dist = to_star.length();

            if dist > 1000.0 && cam_fwd.dot(to_star / dist) < -0.5 {
                if *vis != Visibility::Hidden { *vis = Visibility::Hidden; }
                continue;
            }

            if *vis != Visibility::Inherited { *vis = Visibility::Inherited; }
            let min_scale = fs.radius * 0.5;
            // Une étoile lumineuse paraît plus grosse qu'une naine rouge ou brune
            let angular_scale = dist * 0.005 * fs.glow;
            let dist_shrink = (20_000_000.0 * GALAXY_SCALE / gal_dist.max(1.0)).clamp(0.05, 1.0);
            let mut scale = angular_scale.max(min_scale) * dist_shrink;
            if lod_fade > 0.0 { scale *= 1.0 - lod_fade; }
            tf.scale = Vec3::splat(scale);

            if mat.0 != brightness_mats.steps[step] {
                mat.0 = brightness_mats.steps[step].clone();
            }

            let n = to_star / dist.max(0.001);
            tf.look_to(n, Vec3::Y);
        }
    }
}

fn update_arm_capsule_lod(
    camera_q: Query<&GlobalTransform, With<Camera3d>>,
    galaxy_q: Query<&GalaxyMeta>,
    settings: Res<GameSettings>,
    dim: Res<crate::surface::GalaxyDim>,
    lod_mats: Res<GalaxyLodMaterials>,
    mut capsule_q: Query<(&ArmCapsule, &mut Transform, &mut Visibility, &mut MeshMaterial3d<StandardMaterial>)>,
) {
    let cam_pos = camera_q.single().translation();

    let mut gal_dists: HashMap<u32, f32> = HashMap::new();
    for meta in &galaxy_q {
        if let Some(g) = settings.galaxies.get(meta.id as usize) {
            gal_dists.insert(meta.id, (cam_pos - g.center()).length());
        }
    }

    const DETAIL_CUTOFF: f32 = 200_000_000.0 * GALAXY_SCALE;
    for (cap, mut tf, mut vis, mut mat) in &mut capsule_q {
        let dist = gal_dists.get(&cap.galaxy_id).copied().unwrap_or(f32::MAX);

        if cap.detail && dist > DETAIL_CUTOFF {
            if *vis != Visibility::Hidden { *vis = Visibility::Hidden; }
            tf.scale = Vec3::ZERO;
            continue;
        }

        let raw = ((dist - LOD_CAPS_START) / (LOD_CAPS_FULL - LOD_CAPS_START)).clamp(0.0, 1.0);
        let t = smoothstep(raw);

        if t <= 0.0 {
            if *vis != Visibility::Hidden { *vis = Visibility::Hidden; }
            tf.scale = Vec3::ZERO;
        } else {
            tf.scale = cap.base_scale * t;
            if *vis != Visibility::Inherited { *vis = Visibility::Inherited; }
            // La galaxie principale s'estompe près d'une planète ou d'une lune (forme inchangée)
            let shown = if cap.galaxy_id == 0 { t * dim.0 } else { t };
            let step = ((shown * (LOD_STEPS - 1) as f32).round() as usize).min(LOD_STEPS - 1);
            let ci = cap.color_idx.min(ARM_COLORS - 1);
            let target = &lod_mats.capsule_steps[ci][step];
            if mat.0 != *target { mat.0 = target.clone(); }
        }
    }
}

fn rotate_accretion_disk(
    time: Res<Time>,
    mut disk_q: Query<&mut Transform, With<AccretionDisk>>,
) {
    for mut tf in &mut disk_q {
        tf.rotate_y(time.delta_secs() * 0.08);
    }
}

/// Distance du centre d'un système au-delà de laquelle on l'a quitté : dernière orbite, avec ses
/// lunes et le rayon de la planète, plus une marge.
/// Rayon de recherche (au plus) d'un système qui contient le vaisseau.
pub(crate) const MAX_SYSTEM_REACH: f32 = 30_000_000.0;

fn system_reach(sys: &crate::settings::StarSystemConfig) -> f32 {
    let planets = sys.planets().iter().map(|p| {
        let moons = p.moons.iter().map(|m| m.orbit_distance + m.radius).fold(0.0_f32, f32::max);
        p.orbit_distance + p.radius.max(moons)
    });
    let stars = sys.stars.iter().map(|st| st.orbit_distance + st.radius);
    planets.chain(stars).fold(10_000.0_f32, f32::max) * 1.2 + 10_000.0
}

pub(crate) fn stream_system_bodies(
    mut commands: Commands,
    settings: Res<GameSettings>,
    spatial: Res<SystemSpatialIndex>,
    camera_q: Query<&GlobalTransform, With<Camera3d>>,
    ship_q: Query<&GlobalTransform, With<Ship>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut spawned: ResMut<SpawnedSystems>,
    mut wide: Local<(u32, Option<usize>)>,
    star_q: Query<(Entity, &SystemIdx), With<StarRoot>>,
    planet_q: Query<(Entity, &SystemIdx), With<PlanetRoot>>,
    moon_q: Query<(Entity, &SystemIdx), With<MoonRoot>>,
    belt_q: Query<(Entity, &SystemIdx), With<AsteroidBeltRoot>>,
    flare_q: Query<(Entity, &FlareVoxel)>,
    cloud_q: Query<(Entity, &CloudVoxel)>,
) {
    let cam_pos = camera_q.single().translation();
    let ship_pos = ship_q.single().translation();
    let stream_dist = STREAM_RADIUS * SYSTEM_CELL_SIZE;
    let radius_sq = stream_dist * stream_dist;

    let nearby = spatial.systems_in_radius(ship_pos, stream_dist);
    let mut closest: Option<(usize, f32)> = None;
    for si in &nearby {
        if let Some(sys) = settings.systems.get(*si) {
            let d = ship_pos.distance_squared(sys.center());
            if d < radius_sq {
                if closest.is_none() || d < closest.unwrap().1 {
                    closest = Some((*si, d));
                }
            }
        }
    }

    // Le système chargé reste chargé tant que le vaisseau est dans sa zone : ses orbites sont
    // larges, un système voisin peut avoir un centre plus proche sans que l'on ait quitté le nôtre.
    let staying = spawned
        .0
        .iter()
        .next()
        .copied()
        .filter(|&si| settings.systems.get(si).is_some_and(|sys| ship_pos.distance(sys.center()) < system_reach(sys)));
    let mut want = staying.or(closest.map(|(si, _)| si));
    // Zone d'un grand système loin de son centre (arrivée par un trou de ver) : recherche large,
    // peu fréquente, dont le résultat est gardé tant que le vaisseau est dans la zone.
    if want.is_none() {
        wide.0 += 1;
        if wide.0 % 20 == 1 {
            wide.1 = spatial
                .systems_in_radius(ship_pos, MAX_SYSTEM_REACH)
                .into_iter()
                .filter_map(|si| settings.systems.get(si).map(|sys| (si, ship_pos.distance(sys.center()), system_reach(sys))))
                .filter(|(_, d, reach)| d < reach)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(si, _, _)| si);
        }
        want = wide
            .1
            .filter(|&si| settings.systems.get(si).is_some_and(|sys| ship_pos.distance(sys.center()) < system_reach(sys)));
    }

    let mut to_despawn: Vec<usize> = Vec::new();
    for &si in spawned.0.iter() {
        if Some(si) != want {
            to_despawn.push(si);
        }
    }

    for si in &to_despawn {
        let id_base = si * 1000;
        for (e, idx) in &star_q {
            if idx.0 == *si {
                for (fe, fv) in &flare_q {
                    if fv.star_idx >= id_base && fv.star_idx < id_base + 1000 {
                        if let Some(ec) = commands.get_entity(fe) { ec.despawn_recursive(); }
                    }
                }
                if let Some(ec) = commands.get_entity(e) { ec.despawn_recursive(); }
            }
        }
        for (e, idx) in &planet_q {
            if idx.0 == *si {
                let pid_base = si * 1000;
                for (ce, cv) in &cloud_q {
                    if cv.planet_idx >= pid_base && cv.planet_idx < pid_base + 1000 {
                        if let Some(ec) = commands.get_entity(ce) { ec.despawn_recursive(); }
                    }
                }
                if let Some(ec) = commands.get_entity(e) { ec.despawn_recursive(); }
            }
        }
        for (e, idx) in &moon_q {
            if idx.0 == *si {
                if let Some(ec) = commands.get_entity(e) { ec.despawn_recursive(); }
            }
        }
        for (e, idx) in &belt_q {
            if idx.0 == *si {
                if let Some(ec) = commands.get_entity(e) { ec.despawn_recursive(); }
            }
        }
        spawned.0.remove(si);
    }

    if let Some(si) = want {
        if !spawned.0.contains(&si) {
            if let Some(sys) = settings.systems.get(si) {
                let center = sys.center();
                spawn_system_bodies(
                    &mut commands, sys, &settings, si,
                    &mut meshes, &mut materials, cam_pos, center,
                );
                spawned.0.insert(si);
            }
        }
    }
}

#[cfg(test)]
mod moon_bench {
    use super::*;

    /// Temps des chunks lointains des lunes d'un système (`cargo test --release bench_moon_chunks -- --ignored --nocapture`).
    #[test]
    #[ignore]
    fn bench_moon_chunks() {
        ComputeTaskPool::get_or_init(bevy::tasks::TaskPool::default);
        let settings = GameSettings::default();
        let sys = &settings.systems[25];
        let start = std::time::Instant::now();
        let mut n = 0;
        for p in sys.planets() {
            for m in &p.moons {
                let view = m.generated().then(|| m.as_planet(p));
                n += build_moon_chunks(m, view.as_ref()).len();
            }
        }
        println!("LUNES {n} chunks en {:.0} ms", start.elapsed().as_secs_f64() * 1000.0);
    }
}
