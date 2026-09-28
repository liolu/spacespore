use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

// ─────────────────────────────────────────────────────────────────────────
//  Dossier de données centralisé
// ─────────────────────────────────────────────────────────────────────────

pub const SAVE_VERSION: u32 = 3;

pub fn data_dir() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("spacespore");
    fs::create_dir_all(&path).ok();
    path
}

// ─────────────────────────────────────────────────────────────────────────
//  Configs existantes (inchangées)
// ─────────────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MoonConfig {
    pub orbit_distance: f32,
    pub radius:         f32,
    pub seed:           u32,
    #[serde(default)] pub eccentricity:   f32,
    #[serde(default)] pub inclination:    f32,
    #[serde(default)] pub ascending_node: f32,
    #[serde(default)] pub arg_periapsis:  f32,
    #[serde(default)] pub mean_anomaly_0: f32,
}
impl Default for MoonConfig {
    fn default() -> Self { Self {
        orbit_distance: 400.0, radius: 60.0, seed: 77,
        eccentricity: 0.0, inclination: 0.0, ascending_node: 0.0,
        arg_periapsis: 0.0, mean_anomaly_0: 0.0,
    } }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PlanetConfig {
    pub orbit_distance:  f32,
    pub radius:          f32,
    pub sea_level:       f32,
    pub terrain_height:  f32,
    pub seed:            u32,
    pub noise_scale:     f32,
    pub detail_scale:    f32,
    #[serde(default)] pub moons:        Vec<MoonConfig>,
    #[serde(default)] pub atmosphere:   bool,
    #[serde(default = "default_cloud_density")]  pub cloud_density:  f32,
    #[serde(default = "default_cloud_altitude")] pub cloud_altitude: f32,
    #[serde(default = "default_cloud_speed")]    pub cloud_speed:    f32,
    #[serde(default)] pub eccentricity:   f32,
    #[serde(default)] pub inclination:    f32,
    #[serde(default)] pub ascending_node: f32,
    #[serde(default)] pub arg_periapsis:  f32,
    #[serde(default)] pub mean_anomaly_0: f32,
}
fn default_cloud_density()  -> f32 { 0.5 }
fn default_cloud_altitude() -> f32 { 20.0 }
fn default_cloud_speed()    -> f32 { 0.02 }
impl Default for PlanetConfig {
    fn default() -> Self {
        Self {
            orbit_distance: 2250.0, radius: 250.0, sea_level: 0.4,
            terrain_height: 110.0, seed: 42, noise_scale: 2.0, detail_scale: 4.0,
            moons: Vec::new(), atmosphere: false,
            cloud_density: 0.5, cloud_altitude: 100.0, cloud_speed: 0.02,
            eccentricity: 0.0, inclination: 0.0, ascending_node: 0.0,
            arg_periapsis: 0.0, mean_anomaly_0: 0.0,
        }
    }
}
impl PlanetConfig {
    pub fn temperature(&self) -> f32 {
        -270.0 + 50_000_000.0 / self.orbit_distance.max(10.0)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct StarConfig {
    pub orbit_distance: f32,
    pub radius:         f32,
    #[serde(default = "default_star_intensity")]     pub intensity:      f32,
    #[serde(default = "default_star_light_range")]   pub light_range:    f32,
    #[serde(default = "default_star_light_color_r")] pub light_color_r:  f32,
    #[serde(default = "default_star_light_color_g")] pub light_color_g:  f32,
    #[serde(default = "default_star_light_color_b")] pub light_color_b:  f32,
    #[serde(default = "default_flare_count")]         pub flare_count:    u32,
    #[serde(default = "default_flare_height")]        pub flare_height:   f32,
    #[serde(default = "default_flare_speed")]         pub flare_speed:    f32,
    #[serde(default = "default_flare_size")]          pub flare_size:     f32,
    #[serde(default = "default_flare_distance")]      pub flare_distance: f32,
}
fn default_star_intensity()     -> f32 { 20.0 }
fn default_star_light_range()   -> f32 { 10000.0 }
fn default_star_light_color_r() -> f32 { 1.0 }
fn default_star_light_color_g() -> f32 { 0.92 }
fn default_star_light_color_b() -> f32 { 0.65 }
fn default_flare_count()        -> u32 { 5 }
fn default_flare_height()       -> f32 { 60.0 }
fn default_flare_speed()        -> f32 { 1.0 }
fn default_flare_size()         -> f32 { 6.0 }
fn default_flare_distance()     -> f32 { 15.0 }
impl StarConfig {
    pub fn temperature(&self) -> f32 {
        let r = self.light_color_r;
        let b = self.light_color_b;
        if b > r {
            8000.0 + (b - r) * 40000.0
        } else {
            3000.0 + (1.0 - (r - b)) * 5000.0
        }
    }
}

impl Default for StarConfig {
    fn default() -> Self {
        Self {
            orbit_distance: 0.0, radius: 250.0, intensity: 20.0, light_range: 10000.0,
            light_color_r: 1.0, light_color_g: 0.92, light_color_b: 0.65,
            flare_count: 5, flare_height: 60.0, flare_speed: 1.0,
            flare_size: 6.0, flare_distance: 15.0,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AsteroidBeltConfig {
    pub distance: f32, pub width: f32,
    pub min_size: f32, pub max_size: f32, pub count: u32,
}
impl Default for AsteroidBeltConfig {
    fn default() -> Self { Self { distance: 1500.0, width: 1200.0, min_size: 30.0, max_size: 120.0, count: 300 } }
}

// ─────────────────────────────────────────────────────────────────────────
//  Système stellaire
// ─────────────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct StarSystemConfig {
    pub name: String,
    pub position: [f32; 3],
    #[serde(default = "default_stars")]   pub stars:          Vec<StarConfig>,
    #[serde(default = "default_planets")] pub planets:        Vec<PlanetConfig>,
    #[serde(default)]                     pub asteroid_belts: Vec<AsteroidBeltConfig>,
}

impl Default for StarSystemConfig {
    fn default() -> Self {
        Self {
            name: "Systeme Sol".into(),
            position: [0.0, 0.0, 0.0],
            stars: default_stars(),
            planets: default_planets(),
            asteroid_belts: Vec::new(),
        }
    }
}

impl StarSystemConfig {
    pub fn center(&self) -> bevy::math::Vec3 {
        bevy::math::Vec3::new(self.position[0], self.position[1], self.position[2])
    }
}

fn pseudo_rand(seed: u32) -> f32 {
    let mut x = seed;
    x ^= x >> 16;
    x = x.wrapping_mul(0x45d9f3b);
    x ^= x >> 16;
    x = x.wrapping_mul(0x45d9f3b);
    x ^= x >> 16;
    (x & 0xFFFF) as f32 / 65535.0
}

pub const SYSTEM_GRID_SIZE: usize = 30;
pub const SYSTEM_CELL_SIZE: f32 = 100_000.0;
pub const STREAM_RADIUS: f32 = 5.0;
pub const CLICKABLE_RADIUS: f32 = 30.0;

fn default_systems() -> Vec<StarSystemConfig> {
    let cols = SYSTEM_GRID_SIZE;
    let rows = SYSTEM_GRID_SIZE;
    let cell = SYSTEM_CELL_SIZE;
    let half_grid_x = cols as f32 * cell / 2.0;
    let half_grid_z = rows as f32 * cell / 2.0;
    let margin = cell * 0.15;

    let star_names = [
        "Sol", "Vega", "Altair", "Sirius", "Kepler",
        "Proxima", "Rigel", "Deneb", "Polaris", "Antares",
        "Aldebaran", "Betelgeuse", "Capella", "Fomalhaut", "Arcturus",
        "Spica", "Regulus", "Achernar", "Canopus", "Procyon",
        "Mira", "Castor", "Pollux", "Bellatrix", "Shaula",
        "Toliman", "Hadar", "Mimosa", "Gacrux", "Acrux",
        "Wezen", "Sargas", "Kaus", "Avior", "Menkalinan",
        "Atria", "Alhena", "Mirfak", "Saiph", "Alnitak",
        "Alnilam", "Mintaka", "Rasalhague", "Schedar", "Alphard",
    ];
    let prefixes = ["HD", "GJ", "HR", "TYC", "HIP"];
    let gen_name = |idx: usize| -> String {
        if idx < star_names.len() {
            star_names[idx].to_string()
        } else {
            let pi = idx % prefixes.len();
            format!("{}-{}", prefixes[pi], idx * 7 + 1031)
        }
    };

    let mut systems = Vec::new();
    let mut idx = 0u32;
    for row in 0..rows {
        for col in 0..cols {
            let cell_idx = (row * cols + col) as u32;

            if cell_idx > 0 && pseudo_rand(cell_idx * 7 + 9999) < 0.35 {
                idx += 1;
                continue;
            }

            let cell_x = col as f32 * cell - half_grid_x;
            let cell_z = row as f32 * cell - half_grid_z;

            let x = cell_x + margin + pseudo_rand(cell_idx * 5 + 7) * (cell - 2.0 * margin);
            let z = cell_z + margin + pseudo_rand(cell_idx * 5 + 13) * (cell - 2.0 * margin);
            let y = (pseudo_rand(cell_idx * 5 + 19) - 0.5) * 6000.0;

            let r = pseudo_rand(cell_idx * 5 + 31);
            let star_radius = 75.0 + r * 175.0;
            let star_intensity = 8.0 + r * 27.0;
            let color_seed = pseudo_rand(cell_idx * 5 + 37);
            let sc = if color_seed < 0.2 {
                [1.0, 0.5, 0.3]
            } else if color_seed < 0.4 {
                [1.0, 0.7, 0.4]
            } else if color_seed < 0.6 {
                [1.0, 0.92, 0.65]
            } else if color_seed < 0.8 {
                [0.8, 0.85, 1.0]
            } else {
                [0.6, 0.7, 1.0]
            };

            let seed_base = (cell_idx + 1) * 100;
            let num_planets = 1 + (pseudo_rand(cell_idx * 3 + 41) * 3.0) as usize;
            let mut planets = Vec::new();
            for pi in 0..num_planets {
                let p_orbit = 1750.0 + pi as f32 * 1500.0 + pseudo_rand(seed_base + pi as u32 + 60) * 800.0;
                planets.push(PlanetConfig {
                    orbit_distance: p_orbit,
                    radius: 150.0 + (pi as f32 * 75.0) + pseudo_rand(seed_base + pi as u32 + 50) * 100.0,
                    seed: seed_base + pi as u32,
                    atmosphere: pseudo_rand(cell_idx * 11 + pi as u32 + 71) < 0.4,
                    ..Default::default()
                });
            }
            let belts = if pseudo_rand(cell_idx * 13 + 83) < 0.25 {
                vec![AsteroidBeltConfig { distance: 1750.0 + num_planets as f32 * 1500.0 + 1000.0, ..Default::default() }]
            } else {
                Vec::new()
            };
            systems.push(StarSystemConfig {
                name: gen_name(cell_idx as usize),
                position: [x, y, z],
                stars: vec![StarConfig {
                    radius: star_radius,
                    intensity: star_intensity,
                    light_color_r: sc[0], light_color_g: sc[1], light_color_b: sc[2],
                    ..Default::default()
                }],
                planets,
                asteroid_belts: belts,
            });
            idx += 1;
        }
    }

    // Remplacer le premier système par le chunk de test
    if let Some(first) = systems.first_mut() {
        first.name = "TestLab".to_string();
        first.position = [0.0, 0.0, 0.0];
        first.stars = vec![
            StarConfig {
                radius: 300.0, intensity: 25.0,
                light_color_r: 1.0, light_color_g: 0.92, light_color_b: 0.65,
                ..Default::default()
            },
        ];
        let spacing = 2000.0;
        first.planets = vec![
            // Petite planète rocheuse
            PlanetConfig {
                orbit_distance: spacing,
                radius: 80.0, sea_level: 0.0, terrain_height: 40.0,
                seed: 900, noise_scale: 3.0, detail_scale: 5.0,
                atmosphere: false, ..Default::default()
            },
            // Planète océan avec atmosphère
            PlanetConfig {
                orbit_distance: spacing * 2.0,
                radius: 200.0, sea_level: 0.55, terrain_height: 80.0,
                seed: 901, noise_scale: 2.0, detail_scale: 4.0,
                atmosphere: true, cloud_density: 0.8, cloud_altitude: 80.0, cloud_speed: 0.03,
                moons: vec![
                    MoonConfig { orbit_distance: 500.0, radius: 40.0, seed: 910, ..Default::default() },
                    MoonConfig { orbit_distance: 800.0, radius: 25.0, seed: 911, ..Default::default() },
                ],
                ..Default::default()
            },
            // Grosse planète désertique
            PlanetConfig {
                orbit_distance: spacing * 3.0,
                radius: 350.0, sea_level: 0.1, terrain_height: 150.0,
                seed: 902, noise_scale: 1.5, detail_scale: 3.0,
                atmosphere: true, cloud_density: 0.3, cloud_altitude: 120.0, cloud_speed: 0.01,
                moons: vec![
                    MoonConfig { orbit_distance: 600.0, radius: 55.0, seed: 920, ..Default::default() },
                ],
                ..Default::default()
            },
            // Petite lune/planétoïde sans mer
            PlanetConfig {
                orbit_distance: spacing * 4.0,
                radius: 50.0, sea_level: 0.0, terrain_height: 25.0,
                seed: 903, noise_scale: 4.0, detail_scale: 6.0,
                atmosphere: false, ..Default::default()
            },
            // Planète montagneuse avec atmosphère dense
            PlanetConfig {
                orbit_distance: spacing * 5.0,
                radius: 280.0, sea_level: 0.3, terrain_height: 120.0,
                seed: 904, noise_scale: 2.5, detail_scale: 5.0,
                atmosphere: true, cloud_density: 1.0, cloud_altitude: 100.0, cloud_speed: 0.05,
                moons: vec![
                    MoonConfig { orbit_distance: 450.0, radius: 35.0, seed: 930, ..Default::default() },
                    MoonConfig { orbit_distance: 700.0, radius: 50.0, seed: 931, ..Default::default() },
                    MoonConfig { orbit_distance: 1000.0, radius: 20.0, seed: 932, ..Default::default() },
                ],
                ..Default::default()
            },
        ];
        first.asteroid_belts = vec![
            AsteroidBeltConfig { distance: spacing * 6.0, width: 800.0, min_size: 5.0, max_size: 30.0, count: 200 },
        ];
    }

    systems
}

// ─────────────────────────────────────────────────────────────────────────
//  Configs légères pour draw_orbits (orbit_distance uniquement nécessaire)
// ─────────────────────────────────────────────────────────────────────────

/// Config minimale partagée par tous les nouveaux astres pour draw_orbits
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct OrbitConfig {
    #[serde(default)] pub orbit_distance: f32,
}

// ─────────────────────────────────────────────────────────────────────────
//  GameSettings
// ─────────────────────────────────────────────────────────────────────────

fn default_planets()  -> Vec<PlanetConfig>  { vec![PlanetConfig::default()] }
fn default_stars()    -> Vec<StarConfig>    { vec![StarConfig::default()] }

#[derive(Resource, Serialize, Deserialize, Clone, Debug)]
pub struct GameSettings {
    #[serde(default)] pub save_version: u32,

    pub mouse_sensitivity:      f32,
    pub scroll_speed:           f32,
    pub keyboard_speed:         f32,
    pub invert_y:               bool,
    #[serde(default)] pub show_light_indicator: bool,
    #[serde(default)] pub show_orbits:          bool,
    #[serde(default)] pub show_systems:         bool,
    pub planet_chunk_divisions: usize,

    #[serde(default)] pub world_seed: u64,

    // ── Systèmes stellaires ─────────────────────────────────────────────
    #[serde(default = "default_systems")] pub systems: Vec<StarSystemConfig>,

    // ── Corps historiques (rétrocompat, migré vers systems[0]) ───────────
    #[serde(default = "default_planets")] pub planets:       Vec<PlanetConfig>,
    #[serde(default = "default_stars")]   pub stars:         Vec<StarConfig>,
    #[serde(default)]                     pub asteroid_belts: Vec<AsteroidBeltConfig>,

    // ── Planètes & corps ─────────────────────────────────────────────────
    #[serde(default)] pub comets:    Vec<OrbitConfig>,
    #[serde(default)] pub meteoroids: Vec<OrbitConfig>,

    // ── Étoiles ──────────────────────────────────────────────────────────
    #[serde(default)] pub voxel_stars:    Vec<OrbitConfig>,
    #[serde(default)] pub protostars:     Vec<OrbitConfig>,
    #[serde(default)] pub dwarf_stars:    Vec<OrbitConfig>,
    #[serde(default)] pub main_sequence:  Vec<OrbitConfig>,
    #[serde(default)] pub giants:         Vec<OrbitConfig>,
    #[serde(default)] pub supergiants:    Vec<OrbitConfig>,
    #[serde(default)] pub hypergiants:    Vec<OrbitConfig>,

    // ── Rémanents stellaires ─────────────────────────────────────────────
    #[serde(default)] pub black_holes:   Vec<OrbitConfig>,
    #[serde(default)] pub pulsars:       Vec<OrbitConfig>,
    #[serde(default)] pub magnetars:     Vec<OrbitConfig>,
    #[serde(default)] pub neutron_stars: Vec<OrbitConfig>,
    #[serde(default)] pub supernovae:    Vec<OrbitConfig>,
}

impl Default for GameSettings {
    fn default() -> Self {
        Self {
            save_version: SAVE_VERSION,
            mouse_sensitivity: 0.5, scroll_speed: 10.0,
            keyboard_speed: 2.0, invert_y: true,
            show_light_indicator: false, show_orbits: false, show_systems: false,
            planet_chunk_divisions: 6,
            world_seed: 42,
            systems: default_systems(),
            planets: default_planets(), stars: default_stars(),
            asteroid_belts: Vec::new(),
            comets: Vec::new(), meteoroids: Vec::new(),
            voxel_stars: Vec::new(), protostars: Vec::new(),
            dwarf_stars: Vec::new(), main_sequence: Vec::new(),
            giants: Vec::new(), supergiants: Vec::new(), hypergiants: Vec::new(),
            black_holes: Vec::new(), pulsars: Vec::new(),
            magnetars: Vec::new(), neutron_stars: Vec::new(), supernovae: Vec::new(),
        }
    }
}

impl GameSettings {
    fn config_path() -> PathBuf {
        data_dir().join("settings.json")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            match fs::read_to_string(&path) {
                Ok(contents) => {
                    let mut s: Self = serde_json::from_str(&contents).unwrap_or_default();
                    if s.save_version < SAVE_VERSION {
                        info!("Save version {} -> {}: regeneration du monde (preferences conservees)",
                              s.save_version, SAVE_VERSION);
                        let fresh = Self::default();
                        s.save_version = SAVE_VERSION;
                        s.systems = fresh.systems;
                        s.planets = fresh.planets;
                        s.stars = fresh.stars;
                        s.asteroid_belts = fresh.asteroid_belts;
                        s.comets = fresh.comets;
                        s.meteoroids = fresh.meteoroids;
                        s.voxel_stars = fresh.voxel_stars;
                        s.protostars = fresh.protostars;
                        s.dwarf_stars = fresh.dwarf_stars;
                        s.main_sequence = fresh.main_sequence;
                        s.giants = fresh.giants;
                        s.supergiants = fresh.supergiants;
                        s.hypergiants = fresh.hypergiants;
                        s.black_holes = fresh.black_holes;
                        s.pulsars = fresh.pulsars;
                        s.magnetars = fresh.magnetars;
                        s.neutron_stars = fresh.neutron_stars;
                        s.supernovae = fresh.supernovae;
                        let astres_path = data_dir().join("astres.json");
                        let _ = fs::remove_file(astres_path);
                        s.save();
                    }
                    if s.planets.is_empty() { s.planets = default_planets(); }
                    if s.stars.is_empty()   { s.stars   = default_stars(); }
                    s
                }
                Err(_) => Self::default(),
            }
        } else {
            let settings = Self::default();
            settings.save();
            settings
        }
    }

    pub fn save(&self) {
        let path = Self::config_path();
        if let Ok(json) = serde_json::to_string_pretty(self) {
            fs::write(path, json).ok();
        }
    }
}
