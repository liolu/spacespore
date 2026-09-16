use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;
use rand::Rng;

use crate::lod::{compute_lod_level, LodChunk, LodLevel};
use crate::mesher::{build_celestial_chunk_mesh, build_chunk_mesh};
use crate::settings::GameSettings;

const ORBIT_SPEED: f32 = 0.03;
const STAR_ORBIT_SPEED: f32 = 0.015;
const MOON_ORBIT_SPEED: f32 = 0.08;
const STAR_DIVISIONS: usize = 4;
const MOON_DIVISIONS: usize = 3;

pub struct PlanetPlugin;

impl Plugin for PlanetPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<RegeneratePlanet>()
            .add_systems(Startup, generate_all)
            .add_systems(
                Update,
                (orbit_planets, orbit_stars, orbit_moons, orbit_asteroid_belts, update_flare_voxels, rotate_clouds, regenerate_all, update_lod, update_star_visibility),
            );
    }
}

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
pub struct MoonRoot;

#[derive(Component)]
pub struct MoonId {
    pub planet_idx: usize,
    pub moon_idx: usize,
}

#[derive(Component)]
pub struct FlareVoxel {
    pub star_idx: usize,
    pub flare_idx: u32,
    pub sample_idx: u32,
}

#[derive(Component)]
pub struct SystemOffset(pub Vec3);

#[derive(Component)]
pub struct AsteroidBeltRoot;

#[derive(Component)]
pub struct AsteroidBeltId(pub usize);

#[derive(Component)]
pub struct CloudVoxel {
    pub planet_idx: usize,
    pub theta: f32,
    pub phi: f32,
    pub altitude: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VoxelType {
    Air,
    Water,
    Sand,
    Grass,
    Stone,
    Snow,
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
        }
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

fn generate_all(
    mut commands: Commands,
    settings: Res<GameSettings>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let cam_pos = settings
        .planets
        .first()
        .map(|p| Vec3::new(p.orbit_distance, 80.0, 200.0))
        .unwrap_or(Vec3::new(450.0, 80.0, 200.0));

    // Système legacy (system[0] = planets/stars du GameSettings racine)
    spawn_all_bodies(
        &mut commands,
        &settings,
        &mut meshes,
        &mut materials,
        cam_pos,
    );

    // Systèmes supplémentaires (à partir de l'index 1)
    warn!("=== SYSTEMS COUNT: {} ===", settings.systems.len());
    for (si, sys) in settings.systems.iter().skip(1).enumerate() {
        let center = sys.center();
        warn!("=== SPAWNING SYSTEM {} '{}' at {:?} — stars:{} planets:{} ===", si+1, sys.name, center, sys.stars.len(), sys.planets.len());
        let sys_cam = center + Vec3::new(0.0, 80.0, 200.0);
        spawn_system_bodies(
            &mut commands,
            sys,
            &settings,
            &mut meshes,
            &mut materials,
            sys_cam,
            center,
        );
    }
}

fn spawn_all_bodies(
    commands: &mut Commands,
    settings: &GameSettings,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    cam_pos: Vec3,
) {
    let star_material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        emissive: LinearRgba::new(12.0, 10.0, 3.0, 1.0),
        unlit: true,
        ..default()
    });

    let star_color_low: [f32; 4] = [0.85, 0.35, 0.05, 1.0];
    let star_color_high: [f32; 4] = [1.0, 0.95, 0.55, 1.0];
    let star_lod = LodLevel::Lod2;

    for (i, star_cfg) in settings.stars.iter().enumerate() {
        let pos = if star_cfg.orbit_distance > 1.0 {
            Vec3::new(star_cfg.orbit_distance, 0.0, 0.0)
        } else {
            Vec3::ZERO
        };

        let star_entity = commands
            .spawn((
                Transform::from_translation(pos),
                Visibility::default(),
                StarRoot,
                StarId(i),
            ))
            .id();

        for face in CubeFace::all() {
            for gx in 0..STAR_DIVISIONS {
                for gy in 0..STAR_DIVISIONS {
                    let mesh = build_celestial_chunk_mesh(
                        face,
                        gx,
                        gy,
                        STAR_DIVISIONS,
                        star_cfg.radius,
                        5.0,
                        99 + i as u32,
                        2.0,
                        star_color_low,
                        star_color_high,
                        star_lod,
                    );
                    let chunk = commands
                        .spawn((
                            Mesh3d(meshes.add(mesh)),
                            MeshMaterial3d(star_material.clone()),
                            Transform::IDENTITY,
                            NotShadowCaster,
                            StarChunk,
                        ))
                        .id();
                    commands.entity(star_entity).add_child(chunk);
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

        let intensity = star_cfg.intensity * 2_000_000.0 * 100.0;
        let light = commands
            .spawn((
                PointLight {
                    intensity,
                    range: star_cfg.light_range,
                    shadows_enabled: true,
                    shadow_depth_bias: 0.02,
                    shadow_normal_bias: 1.0,
                    color: Color::srgb(star_cfg.light_color_r, star_cfg.light_color_g, star_cfg.light_color_b),
                    ..default()
                },
                Transform::IDENTITY,
            ))
            .id();
        commands.entity(star_entity).add_child(light);

        // Flare voxels
        let voxel_size = 6.0;
        let samples_per_flare = 64_u32;
        let r = star_cfg.light_color_r;
        let g = star_cfg.light_color_g;
        let b = star_cfg.light_color_b;
        let flare_material = materials.add(StandardMaterial {
            base_color: Color::srgb(r * 1.0, g * 0.7, b * 0.4),
            emissive: LinearRgba::new(r * 2.0, g * 1.2, b * 0.5, 1.0),
            unlit: true,
            ..default()
        });
        let flare_mesh = meshes.add(Mesh::from(Cuboid::new(voxel_size, voxel_size, voxel_size)));

        for fi in 0..star_cfg.flare_count {
            for si in 0..samples_per_flare {
                commands.spawn((
                    Mesh3d(flare_mesh.clone()),
                    MeshMaterial3d(flare_material.clone()),
                    Transform::from_translation(Vec3::ZERO).with_scale(Vec3::ZERO),
                    Visibility::default(),
                    FlareVoxel { star_idx: i, flare_idx: fi, sample_idx: si },
                    NotShadowCaster,
                ));
            }
        }
    }

    let planet_material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.9,
        ..default()
    });

    let divs = settings.planet_chunk_divisions;

    for (i, pcfg) in settings.planets.iter().enumerate() {
        let planet_world_pos = Vec3::new(pcfg.orbit_distance, 0.0, 0.0);
        let cam_local = cam_pos - planet_world_pos;

        let root = commands
            .spawn((
                Transform::from_translation(planet_world_pos),
                Visibility::default(),
                PlanetRoot,
                PlanetId(i),
            ))
            .id();

        let temp = pcfg.temperature();

        for face in CubeFace::all() {
            for gx in 0..divs {
                for gy in 0..divs {
                    let u = (gx as f32 + 0.5) / divs as f32;
                    let v = (gy as f32 + 0.5) / divs as f32;
                    let center = face.to_sphere_pos(u, v) * pcfg.radius;
                    let lod = compute_lod_level(cam_local, center, pcfg.radius);

                    let mesh = build_chunk_mesh(
                        face,
                        gx,
                        gy,
                        divs,
                        pcfg.radius,
                        pcfg.sea_level,
                        pcfg.terrain_height,
                        pcfg.seed,
                        pcfg.noise_scale,
                        pcfg.detail_scale,
                        lod,
                        temp,
                    );

                    let chunk = commands
                        .spawn((
                            Mesh3d(meshes.add(mesh)),
                            MeshMaterial3d(planet_material.clone()),
                            Transform::IDENTITY,
                            PlanetChunk {
                                face,
                                grid_x: gx,
                                grid_y: gy,
                                current_lod: lod,
                                planet_id: i,
                            },
                            LodChunk,
                        ))
                        .id();
                    commands.entity(root).add_child(chunk);
                }
            }
        }

        // Atmosphere clouds — one mesh per cloud patch
        if pcfg.atmosphere {
            let cloud_material = materials.add(StandardMaterial {
                base_color: Color::srgb(0.95, 0.95, 0.97),
                alpha_mode: AlphaMode::Opaque,
                unlit: false,
                perceptual_roughness: 1.0,
                ..default()
            });
            let density = pcfg.cloud_density.clamp(0.1, 1.0);
            let cloud_r = pcfg.radius + pcfg.terrain_height + pcfg.cloud_altitude;

            use noise::{NoiseFn, Perlin};
            let perlin = Perlin::new(pcfg.seed + 500);
            let perlin2 = Perlin::new(pcfg.seed + 501);

            let patch_count = 20 + (density * 30.0) as u32;
            let patch_grid = 16_i32;
            let cell = pcfg.radius * 0.03;

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

                // Build height map from 2D noise
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

                        // Fade at edges of patch
                        let dx = (gx as f32 - patch_grid as f32 * 0.5).abs() / (patch_grid as f32 * 0.5);
                        let dz = (gz as f32 - patch_grid as f32 * 0.5).abs() / (patch_grid as f32 * 0.5);
                        let edge = 1.0 - (dx * dx + dz * dz).sqrt().min(1.0);

                        if n > threshold && edge as f64 > 0.2 {
                            let h = (1 + ((n - threshold) * 3.0) as u32).min(4);
                            grid[(gx * patch_grid + gz) as usize] = h;
                        }
                    }
                }

                // Build mesh with face culling
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

                            // Top face (always if top layer)
                            if y_layer == h - 1 {
                                let vi = positions.len() as u32;
                                positions.extend_from_slice(&[
                                    [x0, y1, z1], [x1, y1, z1], [x1, y1, z0], [x0, y1, z0],
                                ]);
                                normals.extend_from_slice(&[[0.0,1.0,0.0]; 4]);
                                indices.extend_from_slice(&[vi, vi+1, vi+2, vi, vi+2, vi+3]);
                            }

                            // Bottom face (only if bottom layer)
                            if y_layer == 0 {
                                let vi = positions.len() as u32;
                                positions.extend_from_slice(&[
                                    [x0, y0, z0], [x1, y0, z0], [x1, y0, z1], [x0, y0, z1],
                                ]);
                                normals.extend_from_slice(&[[0.0,-1.0,0.0]; 4]);
                                indices.extend_from_slice(&[vi, vi+1, vi+2, vi, vi+2, vi+3]);
                            }

                            // -X face
                            if get_h(gx - 1, gz) <= y_layer {
                                let vi = positions.len() as u32;
                                positions.extend_from_slice(&[
                                    [x0, y0, z0], [x0, y1, z0], [x0, y1, z1], [x0, y0, z1],
                                ]);
                                normals.extend_from_slice(&[[-1.0,0.0,0.0]; 4]);
                                indices.extend_from_slice(&[vi, vi+1, vi+2, vi, vi+2, vi+3]);
                            }

                            // +X face
                            if get_h(gx + 1, gz) <= y_layer {
                                let vi = positions.len() as u32;
                                positions.extend_from_slice(&[
                                    [x1, y0, z1], [x1, y1, z1], [x1, y1, z0], [x1, y0, z0],
                                ]);
                                normals.extend_from_slice(&[[1.0,0.0,0.0]; 4]);
                                indices.extend_from_slice(&[vi, vi+1, vi+2, vi, vi+2, vi+3]);
                            }

                            // -Z face
                            if get_h(gx, gz - 1) <= y_layer {
                                let vi = positions.len() as u32;
                                positions.extend_from_slice(&[
                                    [x1, y0, z0], [x1, y1, z0], [x0, y1, z0], [x0, y0, z0],
                                ]);
                                normals.extend_from_slice(&[[0.0,0.0,-1.0]; 4]);
                                indices.extend_from_slice(&[vi, vi+1, vi+2, vi, vi+2, vi+3]);
                            }

                            // +Z face
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

                let mut mesh = Mesh::new(
                    bevy::render::mesh::PrimitiveTopology::TriangleList,
                    bevy::render::render_asset::RenderAssetUsages::default(),
                );
                mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
                mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
                mesh.insert_indices(bevy::render::mesh::Indices::U32(indices));

                let rotation = Quat::from_rotation_arc(Vec3::Y, center_dir);
                let pos = center_dir * cloud_r;

                commands.spawn((
                    Mesh3d(meshes.add(mesh)),
                    MeshMaterial3d(cloud_material.clone()),
                    Transform::from_translation(pos).with_rotation(rotation),
                    CloudVoxel {
                        planet_idx: i,
                        theta: theta_c,
                        phi: phi_c,
                        altitude: cloud_r,
                    },
                ));
            }
        }
    }

    // Moons
    let moon_material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.95,
        ..default()
    });

    let moon_color_low: [f32; 4] = [0.55, 0.53, 0.50, 1.0];
    let moon_color_high: [f32; 4] = [0.80, 0.78, 0.75, 1.0];
    let moon_lod = LodLevel::Lod2;

    for (pi, pcfg) in settings.planets.iter().enumerate() {
        for (mi, mcfg) in pcfg.moons.iter().enumerate() {
            let offset = Vec3::new(mcfg.orbit_distance, 0.0, 0.0);
            let moon_entity = commands
                .spawn((
                    Transform::from_translation(Vec3::new(pcfg.orbit_distance, 0.0, 0.0) + offset),
                    Visibility::default(),
                    MoonRoot,
                    MoonId { planet_idx: pi, moon_idx: mi },
                ))
                .id();

            for face in CubeFace::all() {
                for gx in 0..MOON_DIVISIONS {
                    for gy in 0..MOON_DIVISIONS {
                        let mesh = build_celestial_chunk_mesh(
                            face,
                            gx,
                            gy,
                            MOON_DIVISIONS,
                            mcfg.radius,
                            2.0,
                            mcfg.seed,
                            1.5,
                            moon_color_low,
                            moon_color_high,
                            moon_lod,
                        );
                        let chunk = commands
                            .spawn((
                                Mesh3d(meshes.add(mesh)),
                                MeshMaterial3d(moon_material.clone()),
                                Transform::IDENTITY,
                            ))
                            .id();
                        commands.entity(moon_entity).add_child(chunk);
                    }
                }
            }
        }
    }

    // Asteroid belts
    let asteroid_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.45, 0.42, 0.38),
        perceptual_roughness: 0.95,
        ..default()
    });

    let mut rng = rand::thread_rng();

    for (i, belt) in settings.asteroid_belts.iter().enumerate() {
        let belt_entity = commands
            .spawn((
                Transform::IDENTITY,
                Visibility::default(),
                AsteroidBeltRoot,
                AsteroidBeltId(i),
            ))
            .id();

        for _ in 0..belt.count.min(300) {
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

fn spawn_system_bodies(
    commands: &mut Commands,
    sys: &crate::settings::StarSystemConfig,
    settings: &GameSettings,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    cam_pos: Vec3,
    center: Vec3,
) {
    let star_material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        emissive: LinearRgba::new(12.0, 10.0, 3.0, 1.0),
        unlit: true,
        ..default()
    });
    let star_color_low: [f32; 4] = [0.85, 0.35, 0.05, 1.0];
    let star_color_high: [f32; 4] = [1.0, 0.95, 0.55, 1.0];
    let star_lod = LodLevel::Lod2;

    for (i, star_cfg) in sys.stars.iter().enumerate() {
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
                StarId(10000 + i),
                SystemOffset(center),
            ))
            .id();

        for face in CubeFace::all() {
            for gx in 0..STAR_DIVISIONS {
                for gy in 0..STAR_DIVISIONS {
                    let mesh = build_celestial_chunk_mesh(
                        face, gx, gy, STAR_DIVISIONS,
                        star_cfg.radius, 5.0,
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
        let intensity = star_cfg.intensity * 2_000_000.0 * (star_cfg.radius / 200.0).powi(2).max(0.1);
        let light = commands
            .spawn((
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
            ))
            .id();
        commands.entity(star_entity).add_child(light);
    }

    let planet_material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.9,
        ..default()
    });
    let divs = settings.planet_chunk_divisions;

    for (i, pcfg) in sys.planets.iter().enumerate() {
        let planet_world_pos = center + Vec3::new(pcfg.orbit_distance, 0.0, 0.0);
        let cam_local = cam_pos - planet_world_pos;

        let root = commands
            .spawn((
                Transform::from_translation(planet_world_pos),
                Visibility::default(),
                PlanetRoot,
                PlanetId(10000 + i),
                SystemOffset(center),
            ))
            .id();

        let temp = pcfg.temperature();
        for face in CubeFace::all() {
            for gx in 0..divs {
                for gy in 0..divs {
                    let u = (gx as f32 + 0.5) / divs as f32;
                    let v = (gy as f32 + 0.5) / divs as f32;
                    let chunk_center = face.to_sphere_pos(u, v) * pcfg.radius;
                    let lod = compute_lod_level(cam_local, chunk_center, pcfg.radius);

                    let mesh = build_chunk_mesh(
                        face, gx, gy, divs,
                        pcfg.radius, pcfg.sea_level, pcfg.terrain_height,
                        pcfg.seed, pcfg.noise_scale, pcfg.detail_scale, lod, temp,
                    );
                    let child = commands
                        .spawn((
                            Mesh3d(meshes.add(mesh)),
                            MeshMaterial3d(planet_material.clone()),
                            Transform::IDENTITY,
                            LodChunk,
                        ))
                        .id();
                    commands.entity(root).add_child(child);
                }
            }
        }

        for (mi, mcfg) in pcfg.moons.iter().enumerate() {
            let offset = Vec3::new(mcfg.orbit_distance, 0.0, 0.0);
            let moon_root = commands
                .spawn((
                    Transform::from_translation(planet_world_pos + offset),
                    Visibility::default(),
                    MoonRoot,
                    MoonId { planet_idx: 10000 + i, moon_idx: mi },
                    SystemOffset(center),
                ))
                .id();

            for face in CubeFace::all() {
                for gx in 0..MOON_DIVISIONS {
                    for gy in 0..MOON_DIVISIONS {
                        let mesh = build_celestial_chunk_mesh(
                            face, gx, gy, MOON_DIVISIONS,
                            mcfg.radius, 2.0, mcfg.seed, 1.5,
                            [0.45, 0.44, 0.42, 1.0], [0.7, 0.68, 0.65, 1.0],
                            LodLevel::Lod2,
                        );
                        let child = commands
                            .spawn((
                                Mesh3d(meshes.add(mesh)),
                                MeshMaterial3d(planet_material.clone()),
                                Transform::IDENTITY,
                            ))
                            .id();
                        commands.entity(moon_root).add_child(child);
                    }
                }
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
                AsteroidBeltId(10000 + i),
                SystemOffset(center),
            ))
            .id();

        for _ in 0..belt.count.min(300) {
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

fn orbit_planets(
    time: Res<Time>,
    settings: Res<GameSettings>,
    mut planet_q: Query<(&mut Transform, &PlanetId, Option<&SystemOffset>), With<PlanetRoot>>,
) {
    let n = settings.planets.len().max(1) as f32;
    for (mut tf, pid, sys_off) in &mut planet_q {
        let center = sys_off.map(|s| s.0).unwrap_or(Vec3::ZERO);
        if pid.0 < 10000 {
            if let Some(cfg) = settings.planets.get(pid.0) {
                let angle = time.elapsed_secs() * ORBIT_SPEED
                    + pid.0 as f32 * std::f32::consts::TAU / n;
                tf.translation.x = center.x + angle.cos() * cfg.orbit_distance;
                tf.translation.z = center.z + angle.sin() * cfg.orbit_distance;
                tf.translation.y = center.y;
            }
        } else {
            let local_idx = pid.0 - 10000;
            for sys in settings.systems.iter().skip(1) {
                if let Some(cfg) = sys.planets.get(local_idx) {
                    let sc = sys.center();
                    let pn = sys.planets.len().max(1) as f32;
                    let angle = time.elapsed_secs() * ORBIT_SPEED
                        + local_idx as f32 * std::f32::consts::TAU / pn;
                    tf.translation.x = sc.x + angle.cos() * cfg.orbit_distance;
                    tf.translation.z = sc.z + angle.sin() * cfg.orbit_distance;
                    tf.translation.y = sc.y;
                    break;
                }
            }
        }
    }
}

fn orbit_stars(
    time: Res<Time>,
    settings: Res<GameSettings>,
    mut star_q: Query<(&mut Transform, &StarId, Option<&SystemOffset>), With<StarRoot>>,
) {
    let n = settings.stars.len().max(1) as f32;
    for (mut tf, sid, sys_off) in &mut star_q {
        let center = sys_off.map(|s| s.0).unwrap_or(Vec3::ZERO);
        if sid.0 < 10000 {
            if let Some(cfg) = settings.stars.get(sid.0) {
                if cfg.orbit_distance > 1.0 {
                    let angle = time.elapsed_secs() * STAR_ORBIT_SPEED
                        + sid.0 as f32 * std::f32::consts::TAU / n;
                    tf.translation.x = center.x + angle.cos() * cfg.orbit_distance;
                    tf.translation.z = center.z + angle.sin() * cfg.orbit_distance;
                    tf.translation.y = center.y;
                }
            }
        } else {
            let local_idx = sid.0 - 10000;
            for sys in settings.systems.iter().skip(1) {
                if let Some(cfg) = sys.stars.get(local_idx) {
                    if cfg.orbit_distance > 1.0 {
                        let sc = sys.center();
                        let sn = sys.stars.len().max(1) as f32;
                        let angle = time.elapsed_secs() * STAR_ORBIT_SPEED
                            + local_idx as f32 * std::f32::consts::TAU / sn;
                        tf.translation.x = sc.x + angle.cos() * cfg.orbit_distance;
                        tf.translation.z = sc.z + angle.sin() * cfg.orbit_distance;
                        tf.translation.y = sc.y;
                    }
                    break;
                }
            }
        }
    }
}

fn orbit_moons(
    time: Res<Time>,
    settings: Res<GameSettings>,
    planet_q: Query<(&GlobalTransform, &PlanetId), With<PlanetRoot>>,
    mut moon_q: Query<(&mut Transform, &MoonId), With<MoonRoot>>,
) {
    for (mut tf, mid) in &mut moon_q {
        let planet_pos = planet_q
            .iter()
            .find(|(_, pid)| pid.0 == mid.planet_idx)
            .map(|(gt, _)| gt.translation())
            .unwrap_or_default();

        let mcfg = if mid.planet_idx < 10000 {
            settings.planets
                .get(mid.planet_idx)
                .and_then(|p| p.moons.get(mid.moon_idx))
        } else {
            let local_idx = mid.planet_idx - 10000;
            settings.systems.iter().skip(1)
                .find_map(|sys| sys.planets.get(local_idx))
                .and_then(|p| p.moons.get(mid.moon_idx))
        };

        if let Some(mcfg) = mcfg {
            let angle = time.elapsed_secs() * MOON_ORBIT_SPEED
                + mid.moon_idx as f32 * std::f32::consts::TAU / 4.0;
            tf.translation.x = planet_pos.x + angle.cos() * mcfg.orbit_distance;
            tf.translation.y = planet_pos.y;
            tf.translation.z = planet_pos.z + angle.sin() * mcfg.orbit_distance;
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

fn update_flare_voxels(
    time: Res<Time>,
    settings: Res<GameSettings>,
    star_q: Query<(&GlobalTransform, &StarId), With<StarRoot>>,
    cam_q: Query<&GlobalTransform, With<Camera3d>>,
    mut flare_q: Query<(&FlareVoxel, &mut Transform, &mut Visibility)>,
) {
    let max_samples = 64_u32;
    let cam_pos = cam_q.iter().next().map(|gt| gt.translation()).unwrap_or_default();

    for (fv, mut tf, mut vis) in &mut flare_q {
        let Some(scfg) = settings.stars.get(fv.star_idx) else {
            *vis = Visibility::Hidden;
            continue;
        };

        if fv.flare_idx >= scfg.flare_count {
            *vis = Visibility::Hidden;
            continue;
        }

        let star_pos = star_q
            .iter()
            .find(|(_, sid)| sid.0 == fv.star_idx)
            .map(|(gt, _)| gt.translation())
            .unwrap_or_default();

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

        if fv.sample_idx >= active_samples {
            *vis = Visibility::Hidden;
            continue;
        }

        let t = time.elapsed_secs() * scfg.flare_speed;
        let base_seed = (fv.star_idx as f32 + 1.0) * 100.0 + fv.flare_idx as f32 * 37.7;

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
            *vis = Visibility::Hidden;
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

        let height = scfg.flare_height * life;
        let half_spread = scfg.flare_distance.max(5.0) * 0.5;

        let start_dir = (dir + perp * half_spread / scfg.radius).normalize();
        let end_dir = (dir - perp * half_spread / scfg.radius).normalize();
        let start = star_pos + start_dir * scfg.radius;
        let end = star_pos + end_dir * scfg.radius;
        let control = star_pos + dir * (scfg.radius + height);

        let frac = fv.sample_idx as f32 / active_samples as f32;

        if frac > life {
            *vis = Visibility::Hidden;
            continue;
        }

        let voxel_size = scfg.flare_size;
        let inv = 1.0 - frac;
        let point = start * inv * inv + control * 2.0 * inv * frac + end * frac * frac;
        let snapped = snap_grid(point, voxel_size.max(1.0));

        *vis = Visibility::Visible;
        tf.translation = snapped;
        let s = voxel_size / 6.0;
        tf.scale = Vec3::splat(s);
    }
}

fn rotate_clouds(
    time: Res<Time>,
    settings: Res<GameSettings>,
    planet_q: Query<(&GlobalTransform, &PlanetId), With<PlanetRoot>>,
    mut cloud_q: Query<(&CloudVoxel, &mut Transform)>,
) {
    let t = time.elapsed_secs();

    for (cloud, mut tf) in &mut cloud_q {
        let Some(pcfg) = settings.planets.get(cloud.planet_idx) else {
            continue;
        };

        let planet_pos = planet_q
            .iter()
            .find(|(_, pid)| pid.0 == cloud.planet_idx)
            .map(|(gt, _)| gt.translation())
            .unwrap_or_default();

        let speed = pcfg.cloud_speed;
        let theta = cloud.theta + t * speed;
        let phi = cloud.phi;
        let alt = cloud.altitude;

        let dir = Vec3::new(
            phi.sin() * theta.cos(),
            phi.cos(),
            phi.sin() * theta.sin(),
        ).normalize();
        tf.translation = planet_pos + dir * alt;
        tf.rotation = Quat::from_rotation_arc(Vec3::Y, dir);
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
    spawn_all_bodies(
        &mut commands,
        &settings,
        &mut meshes,
        &mut materials,
        cam_pos,
    );
}

const MAX_LOD_UPDATES_PER_FRAME: usize = 4;

fn update_lod(
    settings: Res<GameSettings>,
    camera_q: Query<&Transform, With<Camera3d>>,
    planet_q: Query<(&GlobalTransform, &PlanetId), With<PlanetRoot>>,
    mut chunk_q: Query<(&mut PlanetChunk, &Mesh3d, &LodChunk)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let cam_world = camera_q.single().translation;
    let mut updates = 0;

    for (mut chunk, mesh_handle, _) in chunk_q.iter_mut() {
        if updates >= MAX_LOD_UPDATES_PER_FRAME {
            break;
        }

        let Some(pcfg) = settings.planets.get(chunk.planet_id) else {
            continue;
        };

        let planet_pos = planet_q
            .iter()
            .find(|(_, pid)| pid.0 == chunk.planet_id)
            .map(|(gt, _)| gt.translation())
            .unwrap_or_default();

        let cam_local = cam_world - planet_pos;
        let divs = settings.planet_chunk_divisions;
        let u = (chunk.grid_x as f32 + 0.5) / divs as f32;
        let v = (chunk.grid_y as f32 + 0.5) / divs as f32;
        let center = chunk.face.to_sphere_pos(u, v) * pcfg.radius;
        let new_lod = compute_lod_level(cam_local, center, pcfg.radius);

        if new_lod != chunk.current_lod {
            let temp = pcfg.temperature();
            let new_mesh = build_chunk_mesh(
                chunk.face,
                chunk.grid_x,
                chunk.grid_y,
                divs,
                pcfg.radius,
                pcfg.sea_level,
                pcfg.terrain_height,
                pcfg.seed,
                pcfg.noise_scale,
                pcfg.detail_scale,
                new_lod,
                temp,
            );
            if let Some(mesh) = meshes.get_mut(&mesh_handle.0) {
                *mesh = new_mesh;
            }
            chunk.current_lod = new_lod;
            updates += 1;
        }
    }
}

const STAR_DETAIL_DIST: f32 = 5000.0;

fn update_star_visibility(
    camera_q: Query<&GlobalTransform, With<Camera3d>>,
    star_q: Query<(&GlobalTransform, &Children, &StarId), With<StarRoot>>,
    mut chunk_vis_q: Query<&mut Visibility, (With<StarChunk>, Without<StarBeacon>)>,
    mut beacon_vis_q: Query<&mut Visibility, (With<StarBeacon>, Without<StarChunk>)>,
    mut frame: Local<u32>,
) {
    let cam_pos = camera_q.single().translation();
    *frame += 1;

    for (star_gt, children, sid) in &star_q {
        let dist = cam_pos.distance(star_gt.translation());
        let far = dist > STAR_DETAIL_DIST;
        if *frame % 300 == 1 {
            warn!("Star id={} pos={:?} dist={:.0} far={} children={}", sid.0, star_gt.translation(), dist, far, children.len());
        }

        for &child in children.iter() {
            if let Ok(mut vis) = chunk_vis_q.get_mut(child) {
                *vis = if far { Visibility::Hidden } else { Visibility::Inherited };
            }
            if let Ok(mut vis) = beacon_vis_q.get_mut(child) {
                *vis = if far { Visibility::Inherited } else { Visibility::Hidden };
            }
        }
    }
}
