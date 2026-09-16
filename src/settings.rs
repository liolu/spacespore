use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

// ─────────────────────────────────────────────────────────────────────────
//  Configs existantes (inchangées)
// ─────────────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MoonConfig {
    pub orbit_distance: f32,
    pub radius:         f32,
    pub seed:           u32,
}
impl Default for MoonConfig {
    fn default() -> Self { Self { orbit_distance: 80.0, radius: 12.0, seed: 77 } }
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
}
fn default_cloud_density()  -> f32 { 0.5 }
fn default_cloud_altitude() -> f32 { 20.0 }
fn default_cloud_speed()    -> f32 { 0.02 }
impl Default for PlanetConfig {
    fn default() -> Self {
        Self {
            orbit_distance: 450.0, radius: 50.0, sea_level: 0.4,
            terrain_height: 22.0, seed: 42, noise_scale: 2.0, detail_scale: 4.0,
            moons: Vec::new(), atmosphere: false,
            cloud_density: 0.5, cloud_altitude: 20.0, cloud_speed: 0.02,
        }
    }
}
impl PlanetConfig {
    pub fn temperature(&self) -> f32 {
        -270.0 + 500000.0 / self.orbit_distance.max(10.0)
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
impl Default for StarConfig {
    fn default() -> Self {
        Self {
            orbit_distance: 0.0, radius: 200.0, intensity: 20.0, light_range: 10000.0,
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
    fn default() -> Self { Self { distance: 300.0, width: 140.0, min_size: 1.0, max_size: 5.0, count: 100 } }
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

fn default_systems() -> Vec<StarSystemConfig> {
    vec![
        StarSystemConfig {
            name: "Systeme Sol".into(),
            position: [0.0, 0.0, 0.0],
            stars: vec![StarConfig::default()],
            planets: vec![PlanetConfig::default()],
            asteroid_belts: vec![AsteroidBeltConfig::default()],
        },
        StarSystemConfig {
            name: "Systeme Alpha".into(),
            position: [25000.0, 0.0, 5000.0],
            stars: vec![StarConfig {
                radius: 150.0,
                light_color_r: 0.7, light_color_g: 0.8, light_color_b: 1.0,
                ..Default::default()
            }],
            planets: vec![
                PlanetConfig { orbit_distance: 400.0, radius: 40.0, seed: 101, ..Default::default() },
                PlanetConfig { orbit_distance: 700.0, radius: 65.0, seed: 202, atmosphere: true, ..Default::default() },
            ],
            asteroid_belts: Vec::new(),
        },
        StarSystemConfig {
            name: "Systeme Proxima".into(),
            position: [-18000.0, 3000.0, -20000.0],
            stars: vec![StarConfig {
                radius: 80.0, intensity: 8.0,
                light_color_r: 1.0, light_color_g: 0.5, light_color_b: 0.3,
                ..Default::default()
            }],
            planets: vec![
                PlanetConfig { orbit_distance: 250.0, radius: 30.0, seed: 333, ..Default::default() },
            ],
            asteroid_belts: vec![AsteroidBeltConfig { distance: 500.0, ..Default::default() }],
        },
    ]
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
    pub mouse_sensitivity:      f32,
    pub scroll_speed:           f32,
    pub keyboard_speed:         f32,
    pub invert_y:               bool,
    #[serde(default)] pub show_light_indicator: bool,
    #[serde(default)] pub show_orbits:          bool,
    pub planet_chunk_divisions: usize,

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
            mouse_sensitivity: 0.5, scroll_speed: 10.0,
            keyboard_speed: 2.0, invert_y: true,
            show_light_indicator: false, show_orbits: false,
            planet_chunk_divisions: 6,
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
        let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push("spacespore");
        fs::create_dir_all(&path).ok();
        path.push("settings.json");
        path
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            match fs::read_to_string(&path) {
                Ok(contents) => {
                    let mut s: Self = serde_json::from_str(&contents).unwrap_or_default();
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
