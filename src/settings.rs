use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

// ─────────────────────────────────────────────────────────────────────────
//  Dossier de données centralisé
// ─────────────────────────────────────────────────────────────────────────

pub const SAVE_VERSION: u32 = 7;

/// Dossier `saves/` à côté de l'exécutable (installation portable).
/// Si ce dossier n'est pas accessible en écriture (ex. installation système
/// sous Linux, ou bundle `.app` en lecture seule sous macOS), on se rabat sur
/// le dossier de données de l'utilisateur :
///   Windows : %APPDATA%\SpaceSpore\saves
///   Linux   : ~/.local/share/SpaceSpore/saves
///   macOS   : ~/Library/Application Support/SpaceSpore/saves
pub fn data_dir() -> PathBuf {
    static DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let beside_exe = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("saves")));
        if let Some(path) = beside_exe {
            if is_writable_dir(&path) {
                return path;
            }
        }
        let fallback = dirs::data_dir()
            .map(|d| d.join("SpaceSpore").join("saves"))
            .unwrap_or_else(|| PathBuf::from("saves"));
        fs::create_dir_all(&fallback).ok();
        fallback
    })
    .clone()
}

fn is_writable_dir(path: &std::path::Path) -> bool {
    if fs::create_dir_all(path).is_err() {
        return false;
    }
    let probe = path.join(".write_test");
    let ok = fs::write(&probe, b"").is_ok();
    let _ = fs::remove_file(&probe);
    ok
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
    /// Galaxie d'appartenance (0 = galaxie principale, 1.. = galaxies extérieures).
    #[serde(default)]                     pub galaxy_id:      u32,
    #[serde(default = "default_stars")]   pub stars:          Vec<StarConfig>,
    #[serde(default = "default_planets")] pub planets:        Vec<PlanetConfig>,
    #[serde(default)]                     pub asteroid_belts: Vec<AsteroidBeltConfig>,
}

impl Default for StarSystemConfig {
    fn default() -> Self {
        Self {
            name: "Systeme Sol".into(),
            position: [0.0, 0.0, 0.0],
            galaxy_id: 0,
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

pub const SYSTEM_GRID_SIZE: usize = 100;
pub const SYSTEM_CELL_SIZE: f32 = 100_000.0;
pub const STREAM_RADIUS: f32 = 3.0;
pub const CLICKABLE_RADIUS: f32 = 30.0;
pub const GALAXY_RADIUS: f32 = 4_500_000.0;

/// Nombre de galaxies extérieures (ids 1..=NUM_DISTANT_GALAXIES).
pub const NUM_DISTANT_GALAXIES: usize = 100;
/// Aucun système n'est généré à moins de cette distance d'un trou noir galactique.
pub const CORE_EXCLUSION: f32 = 15_000.0;

/// Forme d'une galaxie. Index 0 = galaxie principale, 1.. = galaxies extérieures.
#[derive(Clone, Debug)]
pub struct GalaxyConfig {
    pub center:        bevy::math::Vec3,
    pub tilt:          bevy::math::Quat,
    pub radius:        f32,
    pub num_arms:      usize,
    pub twist:         f32,
    pub core_radius:   f32,
    /// Graine des étoiles de la galaxie (galaxies extérieures uniquement).
    pub seed:          u32,
    pub arm_stars:     usize,
    pub scatter_stars: usize,
}

/// Galaxie principale + galaxies extérieures disposées sur une méta-spirale.
pub fn default_galaxies() -> Vec<GalaxyConfig> {
    use bevy::math::{EulerRot, Quat, Vec3};
    const META_ARMS: usize = 5;
    const META_RADIUS: f32 = 80_000_000.0;
    const META_TWIST: f32 = 4.0;
    const MIN_DIST: f32 = 15_000_000.0;
    let tau = std::f32::consts::TAU;

    let mut galaxies = Vec::with_capacity(NUM_DISTANT_GALAXIES + 1);
    galaxies.push(GalaxyConfig {
        center: Vec3::ZERO,
        tilt: Quat::IDENTITY,
        radius: GALAXY_RADIUS,
        num_arms: 5,
        twist: 5.0,
        core_radius: 3000.0,
        seed: 0,
        arm_stars: 0,
        scatter_stars: 0,
    });

    for gi in 0..NUM_DISTANT_GALAXIES {
        let gs = gi as u32 + 300_000;

        // Position de la galaxie sur les bras de la méta-spirale
        let arm = gi % META_ARMS;
        let arm_base = arm as f32 * tau / META_ARMS as f32;
        let t = pseudo_rand(gs * 13 + 1);
        let r = MIN_DIST + t * t * (META_RADIUS - MIN_DIST);
        let spiral = arm_base + (r / META_RADIUS) * META_TWIST;
        let scatter = (pseudo_rand(gs * 13 + 3) - 0.5) * 0.4;
        let theta = spiral + scatter;
        let center = Vec3::new(
            r * theta.cos(),
            (pseudo_rand(gs * 13 + 5) - 0.5) * 8_000_000.0,
            r * theta.sin(),
        );

        galaxies.push(GalaxyConfig {
            center,
            tilt: Quat::from_euler(
                EulerRot::XYZ,
                (pseudo_rand(gs * 13 + 13) - 0.5) * 1.5,
                pseudo_rand(gs * 13 + 15) * tau,
                (pseudo_rand(gs * 13 + 17) - 0.5) * 1.0,
            ),
            radius: 800_000.0 + pseudo_rand(gs * 13 + 7) * 2_500_000.0,
            num_arms: 2 + (pseudo_rand(gs * 13 + 9) * 4.0) as usize,
            twist: 3.0 + pseudo_rand(gs * 13 + 11) * 4.0,
            core_radius: 1000.0 + pseudo_rand(gs * 13 + 23) * 2000.0,
            seed: gs * 1000,
            arm_stars: 200 + (pseudo_rand(gs * 13 + 19) * 300.0) as usize,
            scatter_stars: 50 + (pseudo_rand(gs * 13 + 21) * 100.0) as usize,
        });
    }
    galaxies
}

fn star_color(seed: u32) -> [f32; 3] {
    let c = pseudo_rand(seed);
    if c < 0.2 { [1.0, 0.5, 0.3] }
    else if c < 0.4 { [1.0, 0.7, 0.4] }
    else if c < 0.6 { [1.0, 0.92, 0.65] }
    else if c < 0.8 { [0.8, 0.85, 1.0] }
    else { [0.6, 0.7, 1.0] }
}

fn default_systems(galaxies: &[GalaxyConfig]) -> Vec<StarSystemConfig> {
    const NUM_ARMS: usize = 5;
    const ARM_STARS: usize = 10_000;
    const SCATTER_STARS: usize = 2_500;
    const ARM_TWIST: f32 = 5.0;
    let tau = std::f32::consts::TAU;
    let gr = GALAXY_RADIUS;

    let prefixes = ["HD", "GJ", "HR", "TYC", "HIP", "NGC", "IC", "SAO"];
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
    let gen_name = |idx: usize| -> String {
        if idx < star_names.len() {
            star_names[idx].to_string()
        } else {
            let pi = idx % prefixes.len();
            format!("{}-{}", prefixes[pi], idx * 7 + 1031)
        }
    };

    let make_star = |seed: u32| -> (f32, f32, [f32; 3]) {
        let r_f = pseudo_rand(seed * 5 + 31);
        let radius = 75.0 + r_f * 175.0;
        let intensity = 8.0 + r_f * 27.0;
        let sc = star_color(seed * 5 + 37);
        (radius, intensity, sc)
    };

    // `sb` : base des graines de planètes (doit rester loin de u32::MAX)
    let make_planets = |seed: u32, sb: u32| -> Vec<PlanetConfig> {
        let n = (pseudo_rand(seed * 3 + 41) * 2.5) as usize;
        (0..n).map(|pi| {
            let pu = pi as u32;
            PlanetConfig {
                orbit_distance: 1750.0 + pi as f32 * 1500.0 + pseudo_rand(sb + pu + 60) * 800.0,
                radius: 100.0 + pi as f32 * 60.0 + pseudo_rand(sb + pu + 50) * 100.0,
                seed: sb + pu,
                atmosphere: pseudo_rand(seed * 11 + pu + 71) < 0.35,
                ..Default::default()
            }
        }).collect()
    };

    let distant_count: usize = galaxies.iter().skip(1).map(|g| g.arm_stars + g.scatter_stars).sum();
    let mut systems = Vec::with_capacity(ARM_STARS + SCATTER_STARS + distant_count);

    // ── Bras spiraux ──────────────────────────────────────────────────
    for i in 0..ARM_STARS {
        let s = i as u32 + 100;
        let arm = i % NUM_ARMS;
        let arm_base = arm as f32 * tau / NUM_ARMS as f32;

        let t = pseudo_rand(s * 7 + 3);
        let r = t * t * gr;

        let spiral = arm_base + (r / gr) * ARM_TWIST;
        let width = 0.35 * (1.0 - r / gr * 0.65);
        let scatter = (pseudo_rand(s * 7 + 5) - 0.5) * width;
        let theta = spiral + scatter;

        let x = r * theta.cos();
        let z = r * theta.sin();
        let thickness = 24000.0 * (1.0 - r / gr * 0.8);
        let y = (pseudo_rand(s * 7 + 7) - 0.5) * thickness;

        let (sr, si, sc) = make_star(s);
        systems.push(StarSystemConfig {
            name: gen_name(i),
            position: [x, y, z],
            galaxy_id: 0,
            stars: vec![StarConfig {
                radius: sr, intensity: si,
                light_color_r: sc[0], light_color_g: sc[1], light_color_b: sc[2],
                ..Default::default()
            }],
            planets: make_planets(s, (s + 1) * 100),
            asteroid_belts: Vec::new(),
        });
    }

    // ── Étoiles dispersées entre les bras ─────────────────────────────
    for i in 0..SCATTER_STARS {
        let s = (ARM_STARS + i) as u32 + 100;
        let t = pseudo_rand(s * 7 + 3);
        let r = t * t * gr * 0.85;
        let theta = pseudo_rand(s * 7 + 5) * tau;
        let x = r * theta.cos();
        let z = r * theta.sin();
        let y = (pseudo_rand(s * 7 + 7) - 0.5) * 1500.0;

        let (sr, si, sc) = make_star(s);
        systems.push(StarSystemConfig {
            name: gen_name(ARM_STARS + i),
            position: [x, y, z],
            galaxy_id: 0,
            stars: vec![StarConfig {
                radius: sr, intensity: si,
                light_color_r: sc[0], light_color_g: sc[1], light_color_b: sc[2],
                ..Default::default()
            }],
            planets: make_planets(s, (s + 1) * 100),
            asteroid_belts: Vec::new(),
        });
    }

    // ── Galaxies extérieures : mêmes systèmes (étoile + planètes) ─────
    for (gid, gal) in galaxies.iter().enumerate().skip(1) {
        let gr = gal.radius;
        let arms = gal.num_arms.max(1);
        let mut local_idx = 0usize;
        for i in 0..(gal.arm_stars + gal.scatter_stars) {
            let s = i as u32 + gal.seed;
            let (lx, ly, lz) = if i < gal.arm_stars {
                // Étoiles sur les bras
                let ab = (i % arms) as f32 * tau / arms as f32;
                let st = pseudo_rand(s * 7 + 3);
                let sr = st * st * gr;
                let sp = ab + (sr / gr) * gal.twist;
                let w = 0.35 * (1.0 - sr / gr * 0.65);
                let sc = (pseudo_rand(s * 7 + 5) - 0.5) * w;
                let thick = 12000.0 * (1.0 - sr / gr * 0.8);
                (sr * (sp + sc).cos(), (pseudo_rand(s * 7 + 7) - 0.5) * thick, sr * (sp + sc).sin())
            } else {
                // Étoiles dispersées entre les bras
                let st = pseudo_rand(s * 7 + 3);
                let sr = st * st * gr * 0.85;
                let stheta = pseudo_rand(s * 7 + 5) * tau;
                (sr * stheta.cos(), (pseudo_rand(s * 7 + 7) - 0.5) * 8000.0, sr * stheta.sin())
            };
            let local = bevy::math::Vec3::new(lx, ly, lz);
            // Pas de système dans le trou noir central
            if local.length() < CORE_EXCLUSION { continue; }
            let world = gal.center + gal.tilt * local;

            let (sr, si, sc) = make_star(s);
            let global_idx = systems.len() as u32;
            let pi = local_idx % prefixes.len();
            systems.push(StarSystemConfig {
                name: format!("G{}-{}-{}", gid, prefixes[pi], local_idx * 7 + 1031),
                position: [world.x, world.y, world.z],
                galaxy_id: gid as u32,
                stars: vec![StarConfig {
                    radius: sr, intensity: si,
                    light_color_r: sc[0], light_color_g: sc[1], light_color_b: sc[2],
                    ..Default::default()
                }],
                planets: make_planets(s, (global_idx + 1) * 100),
                asteroid_belts: Vec::new(),
            });
            local_idx += 1;
        }
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

/// Pseudo par défaut : le nom de session du système, sinon "Pilote".
pub fn default_player_name() -> String {
    let raw = std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_default();
    let name: String = raw.chars().filter(|c| !c.is_control()).take(16).collect();
    if name.trim().is_empty() { "Pilote".into() } else { name.trim().to_string() }
}
fn default_aura_color() -> [f32; 3] { [0.2, 0.9, 1.0] }
fn default_stars()    -> Vec<StarConfig>    { vec![StarConfig::default()] }
fn default_true()        -> bool { true }
fn default_msaa()        -> u32  { 4 }
fn default_lod_quality() -> f32  { 1.0 }
fn default_render_scale() -> f32 { 1.0 }

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

    // ── Graphismes / performances ────────────────────────────────────────
    #[serde(default = "default_true")]        pub vsync:         bool,
    #[serde(default)]                         pub fps_limit:     u32,
    #[serde(default = "default_msaa")]        pub msaa_samples:  u32,
    #[serde(default = "default_true")]        pub shadows:       bool,
    #[serde(default = "default_lod_quality")] pub lod_quality:   f32,
    #[serde(default = "default_true")]        pub show_clouds:   bool,
    #[serde(default = "default_true")]        pub show_flares:   bool,
    #[serde(default = "default_render_scale")] pub render_scale: f32,

    #[serde(default)] pub world_seed: u64,

    // ── Multijoueur ──────────────────────────────────────────────────────
    #[serde(default = "default_player_name")] pub player_name: String,
    #[serde(default = "default_aura_color")]  pub aura_color:  [f32; 3],
    #[serde(default)]                         pub last_join_address: String,

    // ── Systèmes stellaires (régénérés au lancement, jamais sauvegardés) ─
    #[serde(skip)] pub systems: Vec<StarSystemConfig>,
    /// Galaxies (index 0 = principale), régénérées au lancement.
    #[serde(skip)] pub galaxies: Vec<GalaxyConfig>,

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
            vsync: true, fps_limit: 0, msaa_samples: 4, shadows: true,
            lod_quality: 1.0, show_clouds: true, show_flares: true, render_scale: 1.0,
            world_seed: 42,
            player_name: default_player_name(),
            aura_color: default_aura_color(),
            last_join_address: String::new(),
            systems: default_systems(&default_galaxies()),
            galaxies: default_galaxies(),
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
        let mut s = if path.exists() {
            match fs::read_to_string(&path) {
                Ok(contents) => {
                    let mut s: Self = serde_json::from_str(&contents).unwrap_or_default();
                    if s.save_version < SAVE_VERSION {
                        info!("Save version {} -> {}: regeneration du monde (preferences conservees)",
                              s.save_version, SAVE_VERSION);
                        let fresh = Self::default();
                        s.save_version = SAVE_VERSION;
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
        };
        s.galaxies = default_galaxies();
        s.systems = default_systems(&s.galaxies);
        s.comets.clear();
        s.meteoroids.clear();
        s.voxel_stars.clear();
        s.protostars.clear();
        s.dwarf_stars.clear();
        s.main_sequence.clear();
        s.giants.clear();
        s.supergiants.clear();
        s.hypergiants.clear();
        s.black_holes.clear();
        s.pulsars.clear();
        s.magnetars.clear();
        s.neutron_stars.clear();
        s.supernovae.clear();
        s
    }

    pub fn save(&self) {
        let path = Self::config_path();
        if let Ok(json) = serde_json::to_string(self) {
            fs::write(path, json).ok();
        }
    }
}

use std::collections::HashMap;

#[derive(Resource, Default)]
pub struct SystemSpatialIndex {
    cells: HashMap<(i32, i32), Vec<usize>>,
}

impl SystemSpatialIndex {
    pub fn build(settings: &GameSettings) -> Self {
        let cell = SYSTEM_CELL_SIZE;
        let half = SYSTEM_GRID_SIZE as f32 * cell / 2.0;
        let mut cells: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
        for (si, sys) in settings.systems.iter().enumerate() {
            let c = sys.center();
            let col = ((c.x + half) / cell) as i32;
            let row = ((c.z + half) / cell) as i32;
            cells.entry((col, row)).or_default().push(si);
        }
        Self { cells }
    }

    /// Système le plus proche de `pos`, en explorant la grille par anneaux croissants.
    pub fn nearest(&self, pos: bevy::math::Vec3, settings: &GameSettings) -> Option<usize> {
        let cell = SYSTEM_CELL_SIZE;
        let half = SYSTEM_GRID_SIZE as f32 * cell / 2.0;
        let cx = ((pos.x + half) / cell) as i32;
        let cz = ((pos.z + half) / cell) as i32;
        let mut best: Option<(usize, f32)> = None;
        for r in 0..=(SYSTEM_GRID_SIZE as i32 * 2) {
            // Tout système hors de l'anneau r est à plus de (r - 1) cellules
            if let Some((_, d2)) = best {
                let min_d = (r - 1).max(0) as f32 * cell;
                if min_d * min_d > d2 { break; }
            }
            for col in (cx - r)..=(cx + r) {
                for row in (cz - r)..=(cz + r) {
                    if (col - cx).abs() != r && (row - cz).abs() != r { continue; }
                    let Some(indices) = self.cells.get(&(col, row)) else { continue };
                    for &si in indices {
                        let Some(sys) = settings.systems.get(si) else { continue };
                        let d2 = pos.distance_squared(sys.center());
                        if best.map_or(true, |(_, b)| d2 < b) { best = Some((si, d2)); }
                    }
                }
            }
        }
        best.map(|(si, _)| si)
    }

    pub fn systems_in_radius(&self, pos: bevy::math::Vec3, radius: f32) -> Vec<usize> {
        let cell = SYSTEM_CELL_SIZE;
        let half = SYSTEM_GRID_SIZE as f32 * cell / 2.0;
        let r_cells = (radius / cell).ceil() as i32 + 1;
        let cx = ((pos.x + half) / cell) as i32;
        let cz = ((pos.z + half) / cell) as i32;
        let mut result = Vec::new();
        for col in (cx - r_cells)..=(cx + r_cells) {
            for row in (cz - r_cells)..=(cz + r_cells) {
                if let Some(indices) = self.cells.get(&(col, row)) {
                    result.extend_from_slice(indices);
                }
            }
        }
        result
    }
}
