use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;
use crate::astre::{AstreLodRoot, ReloadAstre};
use noise::{NoiseFn, Perlin, Fbm, MultiFractal};

// ─────────────────────────────────────────────
//  Config
// ─────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct StormConfig {
    /// Latitude en radians ∈ [-PI/2, PI/2]
    pub latitude:        f32,
    /// Longitude initiale en radians ∈ [0, TAU]
    pub longitude:       f32,
    /// Vitesse de dérive longitudinale (rad/s)
    pub drift_speed:     f32,
    /// Rayon angulaire de la tempête (radians)
    pub angular_radius:  f32,
    /// Couleur centrale de l'œil
    pub color_eye:       [f32; 3],
    /// Couleur du bord de la tempête
    pub color_edge:      [f32; 3],
    pub emissive:        f32,
    /// Vitesse de rotation propre (rad/s), positif = anti-horaire
    pub spin_speed:      f32,
    pub voxel_size:      f32,
    pub voxel_count:     u32,
}

impl Default for StormConfig {
    fn default() -> Self {
        Self {
            latitude:       -0.38,
            longitude:      1.2,
            drift_speed:    0.008,
            angular_radius: 0.22,
            color_eye:      [1.0, 0.55, 0.1],
            color_edge:     [0.75, 0.3, 0.05],
            emissive:       0.6,
            spin_speed:     0.25,
            voxel_size:     10.0,
            voxel_count:    180,
        }
    }
}

#[derive(Clone, Debug)]
pub struct GasRingConfig {
    /// inner radius (depuis le centre de la planète)
    pub inner_radius:    f32,
    pub outer_radius:    f32,
    pub thickness:       f32,   // épaisseur verticale
    pub voxel_size:      f32,
    pub count:           u32,
    pub color:           [f32; 3],
    pub emissive:        f32,
    pub opacity:         f32,
    /// Vitesse de rotation de la ceinture (rad/s)
    pub rotation_speed:  f32,
}

#[derive(Clone, Debug)]
pub struct AsteroidRingConfig {
    pub inner_radius:    f32,
    pub outer_radius:    f32,
    pub thickness:       f32,
    pub count:           u32,
    pub min_size:        f32,
    pub max_size:        f32,
    pub color:           [f32; 3],
    pub rotation_speed:  f32,
}

#[derive(Clone, Debug)]
pub struct GasPlanetConfig {
    // --- Identité ---
    pub seed:               u32,
    pub orbit_distance:     f32,
    pub orbit_speed:        f32,
    pub radius:             f32,
    // --- Éléments orbitaux ---
    pub eccentricity:       f32,
    pub inclination:        f32,
    pub ascending_node:     f32,
    pub arg_periapsis:      f32,
    pub mean_anomaly_0:     f32,

    // --- Couleurs de bandes ---
    /// Couleurs des bandes (alternées selon latitude)
    pub band_colors:        Vec<[f32; 3]>,
    pub band_emissive:      f32,
    /// Nombre de bandes principales
    pub band_count:         f32,
    /// Vitesse différentielle : les bandes équatoriales vont plus vite
    pub band_speed_equator: f32,
    pub band_speed_pole:    f32,

    // --- Tourbillons ---
    pub swirl_enabled:      bool,
    /// Fréquence du bruit de tourbillon
    pub swirl_frequency:    f64,
    pub swirl_strength:     f32,   // amplitude du déplacement UV en radians
    pub swirl_speed:        f32,

    // --- Voxels surface ---
    pub surface_voxel_size: f32,
    pub surface_resolution: u32,

    // --- Tempêtes ---
    pub storms:             Vec<StormConfig>,

    // --- Ceinture de gaz ---
    pub gas_ring:           Option<GasRingConfig>,

    // --- Ceinture d'astéroïdes ---
    pub asteroid_ring:      Option<AsteroidRingConfig>,
}

impl Default for GasPlanetConfig {
    fn default() -> Self {
        Self {
            seed:               3,
            orbit_distance:     1200.0,
            orbit_speed:        0.018,
            radius:             220.0,
            eccentricity:       0.0,
            inclination:        0.0,
            ascending_node:     0.0,
            arg_periapsis:      0.0,
            mean_anomaly_0:     0.0,

            band_colors: vec![
                [0.85, 0.55, 0.25],  // ocre
                [0.70, 0.38, 0.15],  // brun
                [0.92, 0.78, 0.55],  // beige clair
                [0.60, 0.28, 0.10],  // rouille
                [0.95, 0.88, 0.70],  // crème
                [0.75, 0.45, 0.20],  // orange
            ],
            band_emissive:      0.35,
            band_count:         7.0,
            band_speed_equator: 0.12,
            band_speed_pole:    0.03,

            swirl_enabled:      true,
            swirl_frequency:    3.5,
            swirl_strength:     0.18,
            swirl_speed:        0.04,

            surface_voxel_size: 16.0,
            surface_resolution: 40,

            storms: vec![
                StormConfig {
                    latitude:       -0.35,
                    longitude:      1.0,
                    drift_speed:    0.007,
                    angular_radius: 0.28,
                    color_eye:      [1.0, 0.5, 0.08],
                    color_edge:     [0.65, 0.22, 0.05],
                    emissive:       0.8,
                    spin_speed:     0.22,
                    voxel_size:     9.0,
                    voxel_count:    200,
                },
                StormConfig {
                    latitude:       0.55,
                    longitude:      4.1,
                    drift_speed:    -0.011,
                    angular_radius: 0.14,
                    color_eye:      [0.8, 0.7, 0.9],
                    color_edge:     [0.4, 0.3, 0.6],
                    emissive:       0.5,
                    spin_speed:     -0.35,
                    voxel_size:     8.0,
                    voxel_count:    100,
                },
                StormConfig {
                    latitude:       0.18,
                    longitude:      2.8,
                    drift_speed:    0.015,
                    angular_radius: 0.10,
                    color_eye:      [1.0, 0.85, 0.5],
                    color_edge:     [0.8, 0.55, 0.2],
                    emissive:       0.6,
                    spin_speed:     0.45,
                    voxel_size:     7.0,
                    voxel_count:    80,
                },
            ],

            gas_ring: Some(GasRingConfig {
                inner_radius:   280.0,
                outer_radius:   520.0,
                thickness:      18.0,
                voxel_size:     14.0,
                count:          1200,
                color:          [0.75, 0.65, 0.45],
                emissive:       0.4,
                opacity:        0.55,
                rotation_speed: 0.022,
            }),

            asteroid_ring: Some(AsteroidRingConfig {
                inner_radius:   600.0,
                outer_radius:   850.0,
                thickness:      40.0,
                count:          400,
                min_size:       4.0,
                max_size:       18.0,
                color:          [0.50, 0.47, 0.42],
                rotation_speed: 0.010,
            }),
        }
    }
}

// ── Spawn descriptor (read by system_gen) ──────────────────────────────
pub const SPAWN_PROPS: crate::system_gen::SpawnProps = crate::system_gen::SpawnProps {
    category:  crate::system_gen::AstreCategory::GasPlanet,
    weight:    0.40,
    orbit_min: 3000.0,
    orbit_max: f32::MAX,
};

pub fn generate_random(
    rng: &mut crate::system_gen::SeedRng,
    planets: &mut Vec<GasPlanetConfig>,
    orbit: f32,
) {
    let gas_seed = rng.u32();
    let radius = rng.range_f32(180.0, 400.0);

    let num_bands = rng.range_u32(4, 8);
    let mut band_colors = Vec::new();
    for _ in 0..num_bands {
        band_colors.push([
            rng.range_f32(0.3, 1.0),
            rng.range_f32(0.2, 0.9),
            rng.range_f32(0.05, 0.7),
        ]);
    }

    let num_storms = rng.range_u32(0, 4);
    let mut storms = Vec::new();
    for si in 0..num_storms {
        let eye_r = rng.range_f32(0.5, 1.0);
        let eye_g = rng.range_f32(0.2, 0.8);
        let eye_b = rng.range_f32(0.05, 0.5);
        storms.push(StormConfig {
            latitude: rng.range_f32(-1.2, 1.2),
            longitude: rng.range_f32(0.0, std::f32::consts::TAU),
            drift_speed: rng.range_f32(-0.015, 0.015),
            angular_radius: rng.range_f32(0.08, 0.28),
            color_eye: [eye_r, eye_g, eye_b],
            color_edge: [eye_r * 0.7, eye_g * 0.5, eye_b * 0.4],
            emissive: rng.range_f32(0.4, 0.9),
            spin_speed: rng.range_f32(-0.4, 0.4),
            voxel_size: rng.range_f32(7.0, 12.0),
            voxel_count: rng.range_u32(60, 200) * (si + 1).min(2),
        });
    }

    let has_ring = rng.f32() < 0.45;
    let gas_ring = if has_ring {
        Some(GasRingConfig {
            inner_radius: radius + rng.range_f32(50.0, 100.0),
            outer_radius: radius + rng.range_f32(200.0, 400.0),
            thickness: rng.range_f32(10.0, 25.0),
            voxel_size: rng.range_f32(10.0, 16.0),
            count: rng.range_u32(600, 1400),
            color: [
                rng.range_f32(0.5, 0.8),
                rng.range_f32(0.4, 0.7),
                rng.range_f32(0.3, 0.6),
            ],
            emissive: rng.range_f32(0.2, 0.5),
            opacity: rng.range_f32(0.4, 0.7),
            rotation_speed: rng.range_f32(0.015, 0.035),
        })
    } else {
        None
    };

    planets.push(GasPlanetConfig {
        seed: gas_seed,
        orbit_distance: orbit,
        orbit_speed: rng.range_f32(0.008, 0.025),
        radius,
        eccentricity: rng.range_f32(0.0, 0.08),
        inclination: rng.range_f32(-0.06, 0.06),
        ascending_node: rng.range_f32(0.0, std::f32::consts::TAU),
        arg_periapsis: rng.range_f32(0.0, std::f32::consts::TAU),
        mean_anomaly_0: rng.range_f32(0.0, std::f32::consts::TAU),
        band_colors,
        band_count: num_bands as f32,
        band_emissive: rng.range_f32(0.2, 0.5),
        band_speed_equator: rng.range_f32(0.06, 0.15),
        band_speed_pole: rng.range_f32(0.01, 0.05),
        swirl_enabled: true,
        swirl_frequency: rng.range_f32(2.0, 5.0) as f64,
        swirl_strength: rng.range_f32(0.1, 0.25),
        swirl_speed: rng.range_f32(0.02, 0.06),
        surface_voxel_size: rng.range_f32(12.0, 20.0),
        surface_resolution: rng.range_u32(30, 45),
        storms,
        gas_ring,
        asteroid_ring: None,
    });
}

// ─────────────────────────────────────────────
//  Plugin
// ─────────────────────────────────────────────

pub struct GasPlanetPlugin;

impl Plugin for GasPlanetPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<GasPlanetRes>()
            .add_event::<RegenerateGasPlanet>()
            .add_systems(Startup, spawn_gas_planets)
            .add_systems(Update, (
                orbit_gas_planets,
                animate_surface_bands,
                animate_storms,
                rotate_gas_rings,
                rotate_asteroid_rings,
                regenerate_gas_planets,
                reload_gas_planets,
            ));
    }
}

// ─────────────────────────────────────────────
//  Resource & Events
// ─────────────────────────────────────────────

#[derive(Resource)]
pub struct GasPlanetRes {
    pub planets: Vec<GasPlanetConfig>,
}

impl Default for GasPlanetRes {
    fn default() -> Self {
        Self { planets: vec![
            GasPlanetConfig { orbit_distance: 4500.0, seed: 3, ..Default::default() },
        ]}
    }
}

#[derive(Event)]
pub struct RegenerateGasPlanet;

// ─────────────────────────────────────────────
//  Composants ECS
// ─────────────────────────────────────────────

#[derive(Component)]
pub struct GasPlanetRoot {
    pub idx: usize,
}

/// Voxel de surface atmosphérique (bandes + tourbillons)
#[derive(Component)]
pub struct GasSurfaceVoxel {
    pub planet_idx: usize,
    /// Latitude sphérique ∈ [-PI/2, PI/2]
    pub lat:        f32,
    /// Longitude sphérique ∈ [0, TAU]
    pub lon:        f32,
    /// Rayon initial
    pub radius:     f32,
    /// Seed propre pour les micro-variations
    pub seed:       f32,
    /// Index de bande (détermine la vitesse de rotation)
    pub band_idx:   usize,
}

/// Voxel de tempête (ovale animé)
#[derive(Component)]
pub struct StormVoxel {
    pub planet_idx:  usize,
    pub storm_idx:   usize,
    /// Position relative au centre de la tempête (normalisée)
    pub local_r:     f32,   // distance au centre, ∈ [0,1]
    pub local_angle: f32,   // angle initial dans la tempête
    pub seed:        f32,
}

/// Racine de la ceinture de gaz (enfant de GasPlanetRoot)
#[derive(Component)]
pub struct GasRingRoot {
    pub planet_idx: usize,
}

/// Voxel de la ceinture de gaz
#[derive(Component)]
pub struct GasRingVoxel {
    pub planet_idx: usize,
    /// Angle initial dans la ceinture
    pub angle:      f32,
    pub radius:     f32,
    pub height:     f32,
    pub seed:       f32,
}

/// Racine de la ceinture d'astéroïdes
#[derive(Component)]
pub struct AsteroidRingRoot {
    pub planet_idx: usize,
}

// ─────────────────────────────────────────────
//  Helpers
// ─────────────────────────────────────────────

fn pseudo_hash(a: f32, b: f32) -> f32 {
    ((a * 12.9898 + b * 78.233).sin() * 43758.5453).fract()
}

fn snap_grid(v: Vec3, grid: f32) -> Vec3 {
    Vec3::new(
        (v.x / grid).round() * grid,
        (v.y / grid).round() * grid,
        (v.z / grid).round() * grid,
    )
}

fn lerp_color(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// Convertit latitude/longitude/rayon → Vec3 (Y-up)
fn sphere_pos(lat: f32, lon: f32, r: f32) -> Vec3 {
    Vec3::new(
        r * lat.cos() * lon.cos(),
        r * lat.sin(),
        r * lat.cos() * lon.sin(),
    )
}

/// Couleur de bande selon latitude + tourbillon fbm
fn band_color(
    cfg:        &GasPlanetConfig,
    lat:        f32,
    lon:        f32,
    fbm:        &Fbm<Perlin>,
    time_swirl: f32,
) -> [f32; 3] {
    // Tourbillon : déplace le lat/lon d'échantillonnage
    let swirl_offset = if cfg.swirl_enabled {
        let s = fbm.get([
            lon as f64 * cfg.swirl_frequency,
            lat as f64 * cfg.swirl_frequency,
            time_swirl as f64,
        ]) as f32;
        s * cfg.swirl_strength
    } else {
        0.0
    };

    // Latitude effective après tourbillon
    let eff_lat = lat + swirl_offset;

    // Index de bande : on mappe sin(lat * band_count) → [0, 1]
    let band_t = (eff_lat * cfg.band_count).sin() * 0.5 + 0.5;

    // Mélange entre deux bandes adjacentes
    let n       = cfg.band_colors.len();
    let idx_f   = band_t * (n as f32 - 1.0);
    let idx_lo  = (idx_f as usize).min(n - 2);
    let idx_hi  = idx_lo + 1;
    let frac    = idx_f - idx_lo as f32;

    // Bruit de variation fine sur la bande
    let detail = fbm.get([
        lon as f64 * cfg.swirl_frequency * 2.0,
        eff_lat as f64 * cfg.swirl_frequency * 3.0,
        time_swirl as f64 * 0.3,
    ]) as f32;
    let detail_frac = (frac + detail * 0.18).clamp(0.0, 1.0);

    lerp_color(cfg.band_colors[idx_lo], cfg.band_colors[idx_hi], detail_frac)
}

// ─────────────────────────────────────────────
//  Spawn
// ─────────────────────────────────────────────

fn spawn_gas_planets(
    mut commands:  Commands,
    res:           Res<GasPlanetRes>,
    mut meshes:    ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (idx, cfg) in res.planets.iter().enumerate() {
        build_gas_planet(&mut commands, cfg, idx, &mut meshes, &mut materials);
    }
}

fn build_gas_planet(
    commands:  &mut Commands,
    cfg:       &GasPlanetConfig,
    idx:       usize,
    meshes:    &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) {
    let fbm: Fbm<Perlin> = Fbm::<Perlin>::new(cfg.seed)
        .set_octaves(5)
        .set_frequency(1.0)
        .set_lacunarity(2.1)
        .set_persistence(0.48);

    let initial_pos = Vec3::new(cfg.orbit_distance, 0.0, 0.0);

    let root = commands.spawn((
        Transform::from_translation(initial_pos),
        Visibility::default(),
        GasPlanetRoot { idx },
        AstreLodRoot { cull_dist: 8000.0, radius: cfg.radius, streamable: true, label: "GasPlanet" },
    )).id();

    // ── Surface voxels ─────────────────────────────────────────────────────
    let res   = cfg.surface_resolution;
    let vstep = std::f32::consts::PI / res as f32;           // pas latitude
    let hstep = std::f32::consts::TAU / (res * 2) as f32;   // pas longitude

    let voxel_mesh = meshes.add(Mesh::from(Cuboid::new(
        cfg.surface_voxel_size,
        cfg.surface_voxel_size,
        cfg.surface_voxel_size,
    )));

    let mut lon_i = 0u32;
    let mut lon = 0.0_f32;
    while lon < std::f32::consts::TAU {
        let mut lat = -std::f32::consts::FRAC_PI_2;
        let mut lat_i = 0u32;
        while lat <= std::f32::consts::FRAC_PI_2 {
            let [r, g, b] = band_color(cfg, lat, lon, &fbm, 0.0);
            let be = cfg.band_emissive;

            let mat = materials.add(StandardMaterial {
                base_color: Color::srgb(r, g, b),
                emissive:   LinearRgba::new(r * be, g * be, b * be, 1.0),
                unlit:      true,
                ..default()
            });

            let pos = sphere_pos(lat, lon, cfg.radius);
            let snapped = snap_grid(pos, cfg.surface_voxel_size);

            // Index de bande : latitude normalisée
            let band_norm = (lat + std::f32::consts::FRAC_PI_2)
                / std::f32::consts::PI;
            let band_idx  = (band_norm * cfg.band_count) as usize;
            let seed_val  = pseudo_hash(lon_i as f32, lat_i as f32 + 0.5);

            let v = commands.spawn((
                Mesh3d(voxel_mesh.clone()),
                MeshMaterial3d(mat),
                Transform::from_translation(snapped),
                Visibility::default(),
                NotShadowCaster,
                GasSurfaceVoxel {
                    planet_idx: idx,
                    lat,
                    lon,
                    radius: cfg.radius,
                    seed: seed_val,
                    band_idx,
                },
            )).id();
            commands.entity(root).add_child(v);

            lat += vstep;
            lat_i += 1;
        }
        lon += hstep;
        lon_i += 1;
    }

    // ── Tempêtes ───────────────────────────────────────────────────────────
    for (si, storm) in cfg.storms.iter().enumerate() {
        let storm_mesh = meshes.add(Mesh::from(Cuboid::new(
            storm.voxel_size, storm.voxel_size, storm.voxel_size,
        )));

        for vi in 0..storm.voxel_count {
            let h1 = pseudo_hash(cfg.seed as f32 + si as f32 * 13.7, vi as f32);
            let h2 = pseudo_hash(cfg.seed as f32 + si as f32 * 7.3,  vi as f32 + 100.0);
            // Distribution concentrée (racine carrée → plus de voxels au centre)
            let local_r     = h1.sqrt();
            let local_angle = h2 * std::f32::consts::TAU;
            let seed_val    = pseudo_hash(si as f32, vi as f32 * 2.31);

            // Couleur : interpolée du centre vers le bord
            let [er, eg, eb] = lerp_color(storm.color_eye, storm.color_edge, local_r);
            let se = storm.emissive;
            let storm_mat = materials.add(StandardMaterial {
                base_color: Color::srgb(er, eg, eb),
                emissive:   LinearRgba::new(er * se, eg * se, eb * se, 1.0),
                alpha_mode: AlphaMode::Add,
                unlit:      true,
                ..default()
            });

            // Position initiale (sera recalculée chaque frame)
            let lat_s  = storm.latitude + local_r * storm.angular_radius * local_angle.sin();
            let lon_s  = storm.longitude + local_r * storm.angular_radius * local_angle.cos();
            let pos    = sphere_pos(lat_s, lon_s, cfg.radius + storm.voxel_size * 0.3);
            let snapped = snap_grid(pos, storm.voxel_size);

            let v = commands.spawn((
                Mesh3d(storm_mesh.clone()),
                MeshMaterial3d(storm_mat),
                Transform::from_translation(snapped),
                Visibility::default(),
                NotShadowCaster,
                StormVoxel {
                    planet_idx:  idx,
                    storm_idx:   si,
                    local_r,
                    local_angle,
                    seed:        seed_val,
                },
            )).id();
            commands.entity(root).add_child(v);
        }
    }

    // ── Ceinture de gaz ────────────────────────────────────────────────────
    if let Some(ring) = &cfg.gas_ring {
        let ring_root = commands.spawn((
            Transform::IDENTITY,
            Visibility::default(),
            GasRingRoot { planet_idx: idx },
        )).id();
        commands.entity(root).add_child(ring_root);

        let [rr, rg, rb] = ring.color;
        let re = ring.emissive;
        let ring_mat = materials.add(StandardMaterial {
            base_color: Color::srgba(rr, rg, rb, ring.opacity),
            emissive:   LinearRgba::new(rr * re, rg * re, rb * re, 1.0),
            alpha_mode: AlphaMode::Add,
            unlit:      true,
            ..default()
        });
        let ring_mesh = meshes.add(Mesh::from(Cuboid::new(
            ring.voxel_size, ring.voxel_size * 0.4, ring.voxel_size,
        )));

        for i in 0..ring.count {
            let h1 = pseudo_hash(cfg.seed as f32 + 200.0, i as f32);
            let h2 = pseudo_hash(cfg.seed as f32 + 201.0, i as f32);
            let h3 = pseudo_hash(cfg.seed as f32 + 202.0, i as f32);
            let h4 = pseudo_hash(cfg.seed as f32 + 203.0, i as f32);

            let angle  = h1 * std::f32::consts::TAU;
            let radius = ring.inner_radius + h2 * (ring.outer_radius - ring.inner_radius);
            let height = (h3 * 2.0 - 1.0) * ring.thickness * 0.5;
            let seed_v = h4;

            let pos = Vec3::new(angle.cos() * radius, height, angle.sin() * radius);
            let snapped = snap_grid(pos, ring.voxel_size);

            let v = commands.spawn((
                Mesh3d(ring_mesh.clone()),
                MeshMaterial3d(ring_mat.clone()),
                Transform::from_translation(snapped),
                Visibility::default(),
                NotShadowCaster,
                GasRingVoxel { planet_idx: idx, angle, radius, height, seed: seed_v },
            )).id();
            commands.entity(ring_root).add_child(v);
        }
    }

    // ── Ceinture d'astéroïdes ──────────────────────────────────────────────
    if let Some(belt) = &cfg.asteroid_ring {
        let belt_root = commands.spawn((
            Transform::IDENTITY,
            Visibility::default(),
            AsteroidRingRoot { planet_idx: idx },
        )).id();
        commands.entity(root).add_child(belt_root);

        let [ar, ag, ab] = belt.color;
        let ast_mat = materials.add(StandardMaterial {
            base_color:          Color::srgb(ar, ag, ab),
            perceptual_roughness: 0.95,
            ..default()
        });

        for i in 0..belt.count {
            let h1 = pseudo_hash(cfg.seed as f32 + 300.0, i as f32);
            let h2 = pseudo_hash(cfg.seed as f32 + 301.0, i as f32);
            let h3 = pseudo_hash(cfg.seed as f32 + 302.0, i as f32);
            let h4 = pseudo_hash(cfg.seed as f32 + 303.0, i as f32);
            let h5 = pseudo_hash(cfg.seed as f32 + 304.0, i as f32);

            let angle  = h1 * std::f32::consts::TAU;
            let radius = belt.inner_radius + h2 * (belt.outer_radius - belt.inner_radius);
            let height = (h3 * 2.0 - 1.0) * belt.thickness * 0.5;
            let size   = belt.min_size + h4 * (belt.max_size - belt.min_size);

            let pos = Vec3::new(angle.cos() * radius, height, angle.sin() * radius);

            let rotation = Quat::from_euler(
                EulerRot::XYZ,
                h5 * std::f32::consts::TAU,
                pseudo_hash(h1, h2) * std::f32::consts::TAU,
                pseudo_hash(h2, h3) * std::f32::consts::TAU,
            );

            let mesh = meshes.add(Mesh::from(Cuboid::new(size, size * 0.65, size * 0.8)));

            let ast = commands.spawn((
                Mesh3d(mesh),
                MeshMaterial3d(ast_mat.clone()),
                Transform::from_translation(pos).with_rotation(rotation),
                NotShadowCaster,
            )).id();
            commands.entity(belt_root).add_child(ast);
        }
    }
}

// ─────────────────────────────────────────────
//  Reload (streaming)
// ─────────────────────────────────────────────

fn reload_gas_planets(
    mut commands:  Commands,
    mut events:    EventReader<ReloadAstre>,
    res:           Res<GasPlanetRes>,
    roots:         Query<(Entity, &GasPlanetRoot)>,
    mut meshes:    ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for ev in events.read() {
        let Ok((entity, root)) = roots.get(ev.0) else { continue };
        let idx = root.idx;
        let Some(cfg) = res.planets.get(idx) else { continue };
        if let Some(ec) = commands.get_entity(entity) { ec.despawn_recursive(); }
        build_gas_planet(&mut commands, cfg, idx, &mut meshes, &mut materials);
    }
}

// ─────────────────────────────────────────────
//  Orbite
// ─────────────────────────────────────────────

fn orbit_gas_planets(
    time:      Res<Time>,
    res:       Res<GasPlanetRes>,
    mut root_q: Query<(&mut Transform, &GasPlanetRoot)>,
) {
    use crate::kepler::{OrbitalElements, DEFAULT_MU};
    let t = time.elapsed_secs();
    for (mut tf, root) in &mut root_q {
        let Some(cfg) = res.planets.get(root.idx) else { continue; };
        let elems = OrbitalElements {
            a: cfg.orbit_distance,
            e: cfg.eccentricity,
            i: cfg.inclination,
            omega_big: cfg.ascending_node,
            omega: cfg.arg_periapsis,
            m0: cfg.mean_anomaly_0,
        };
        tf.translation = elems.position(t as f64, DEFAULT_MU);
    }
}

// ─────────────────────────────────────────────
//  Animation bandes + tourbillons
// ─────────────────────────────────────────────

fn animate_surface_bands(
    time:       Res<Time>,
    res:        Res<GasPlanetRes>,
    mut surf_q: Query<(&GasSurfaceVoxel, &mut Transform, &MeshMaterial3d<StandardMaterial>)>,
    mut mats:   ResMut<Assets<StandardMaterial>>,
) {
    let t = time.elapsed_secs();

    // Un FBM par frame (partagé) — Bevy ne permet pas de le cacher facilement
    // sans Resource, on le reconstruit à chaque frame (léger car resolution basse)
    // Pour optimiser : mettre le Fbm dans une Resource dédiée.
    for (vx, mut tf, mat_handle) in &mut surf_q {
        let Some(cfg) = res.planets.get(vx.planet_idx) else { continue; };

        let fbm: Fbm<Perlin> = Fbm::<Perlin>::new(cfg.seed)
            .set_octaves(4)
            .set_frequency(1.0)
            .set_lacunarity(2.1)
            .set_persistence(0.45);

        // Vitesse différentielle selon la latitude (équateur plus rapide)
        let lat_norm   = (vx.lat + std::f32::consts::FRAC_PI_2) / std::f32::consts::PI;
        let band_speed = cfg.band_speed_pole
            + (cfg.band_speed_equator - cfg.band_speed_pole)
            * (1.0 - (2.0 * lat_norm - 1.0).abs()).powf(1.5);

        // Longitude animée
        let anim_lon = vx.lon + t * band_speed;

        // Tourbillon temporel
        let time_swirl = t * cfg.swirl_speed;

        // Nouvelle couleur selon la longitude animée
        let [r, g, b] = band_color(cfg, vx.lat, anim_lon, &fbm, time_swirl);
        let be = cfg.band_emissive;

        if let Some(mat) = mats.get_mut(&mat_handle.0) {
            mat.base_color = Color::srgb(r, g, b);
            mat.emissive   = LinearRgba::new(r * be, g * be, b * be, 1.0);
        }

        // Position mise à jour (la longitude change = le voxel tourne autour de l'axe Y)
        let new_pos = sphere_pos(vx.lat, anim_lon, vx.radius);
        tf.translation = snap_grid(new_pos, cfg.surface_voxel_size);

        // Légère oscillation radiale (effet atmosphérique)
        let pulse = 1.0 + (t * 0.8 + vx.seed * 6.28).sin() * 0.012;
        tf.scale = Vec3::splat(pulse);
    }
}

// ─────────────────────────────────────────────
//  Animation tempêtes
// ─────────────────────────────────────────────

fn animate_storms(
    time:        Res<Time>,
    res:         Res<GasPlanetRes>,
    mut storm_q: Query<(&StormVoxel, &mut Transform)>,
) {
    let t = time.elapsed_secs();

    for (sv, mut tf) in &mut storm_q {
        let Some(cfg) = res.planets.get(sv.planet_idx) else { continue; };
        let Some(storm) = cfg.storms.get(sv.storm_idx) else { continue; };

        // Longitude de la tempête animée (dérive globale)
        let storm_lon = storm.longitude + t * storm.drift_speed;

        // Rotation propre des voxels autour du centre de la tempête
        let spin_angle = sv.local_angle + t * storm.spin_speed * (1.0 - sv.local_r * 0.4);

        // Déformation ovale : axe X légèrement compressé
        let oval_x = sv.local_r * storm.angular_radius * spin_angle.cos() * 1.0;
        let oval_y = sv.local_r * storm.angular_radius * spin_angle.sin() * 0.65;

        // Position en sphère
        let lat_s = storm.latitude + oval_y;
        let lon_s = storm_lon + oval_x / lat_s.cos().max(0.1);

        let r   = cfg.radius + storm.voxel_size * (0.25 + sv.local_r * 0.15);
        let pos = sphere_pos(lat_s, lon_s, r);

        tf.translation = snap_grid(pos, storm.voxel_size);

        // Pulsation de l'œil
        let pulse = 1.0 + (t * 1.5 + sv.seed * std::f32::consts::TAU).sin() * 0.08;
        let size  = (1.0 - sv.local_r * 0.6).max(0.2) * pulse;
        tf.scale  = Vec3::splat(size);
    }
}

// ─────────────────────────────────────────────
//  Rotation ceintures
// ─────────────────────────────────────────────

fn rotate_gas_rings(
    time:       Res<Time>,
    res:        Res<GasPlanetRes>,
    mut ring_q: Query<(&GasRingVoxel, &mut Transform)>,
) {
    let t = time.elapsed_secs();

    for (rv, mut tf) in &mut ring_q {
        let Some(cfg) = res.planets.get(rv.planet_idx) else { continue; };
        let Some(ring) = &cfg.gas_ring else { continue; };

        // Vitesse différentielle selon le rayon (plus proche = plus rapide)
        let speed = ring.rotation_speed * (ring.outer_radius / rv.radius.max(1.0)).sqrt();
        let angle = rv.angle + t * speed;

        let pos = Vec3::new(
            angle.cos() * rv.radius,
            rv.height,
            angle.sin() * rv.radius,
        );

        // Léger shimmer vertical
        let shimmer = (t * 0.9 + rv.seed * std::f32::consts::TAU).sin() * ring.thickness * 0.08;
        tf.translation = snap_grid(pos + Vec3::Y * shimmer, ring.voxel_size);
    }
}

fn rotate_asteroid_rings(
    time:       Res<Time>,
    res:        Res<GasPlanetRes>,
    mut belt_q: Query<(&mut Transform, &AsteroidRingRoot)>,
) {
    let t = time.elapsed_secs();
    for (mut tf, belt) in &mut belt_q {
        let Some(cfg) = res.planets.get(belt.planet_idx) else { continue; };
        let Some(b) = &cfg.asteroid_ring else { continue; };
        tf.rotation = Quat::from_rotation_y(t * b.rotation_speed);
    }
}

// ─────────────────────────────────────────────
//  Régénération à chaud
// ─────────────────────────────────────────────

fn regenerate_gas_planets(
    mut commands:  Commands,
    mut events:    EventReader<RegenerateGasPlanet>,
    res:           Res<GasPlanetRes>,
    root_q:        Query<Entity, With<GasPlanetRoot>>,
    mut meshes:    ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut fired = false;
    for _ in events.read() { fired = true; }
    if !fired { return; }

    for entity in &root_q {
        if let Some(ec) = commands.get_entity(entity) { ec.despawn_recursive(); }
    }

    for (idx, cfg) in res.planets.iter().enumerate() {
        build_gas_planet(&mut commands, cfg, idx, &mut meshes, &mut materials);
    }
}
