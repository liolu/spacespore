use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

// ─────────────────────────────────────────────────────────────────────────
//  Dossier de données centralisé
// ─────────────────────────────────────────────────────────────────────────

pub const SAVE_VERSION: u32 = 9;

/// Dossier des sauvegardes de CETTE version : `saves/v0.8.0/` (un dossier par version installée).
/// Il contient `settings.json` (réglages et progression), `world.json` (la sauvegarde du monde :
/// graine, territoires, trous de ver connus, guilde...) et `info.json` (résumé lisible : version,
/// graine, joueur, dernière partie).
///
/// `saves/` se trouve à côté de l'exécutable (installation portable). Si ce dossier n'est pas
/// accessible en écriture (installation système sous Linux, bundle `.app` en lecture seule sous
/// macOS), on se rabat sur le dossier de données de l'utilisateur :
///   Windows : %APPDATA%\SpaceSpore\saves
///   Linux   : ~/.local/share/SpaceSpore/saves
///   macOS   : ~/Library/Application Support/SpaceSpore/saves
pub fn data_dir() -> PathBuf {
    static DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let beside_exe = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("saves")));
        let base = match beside_exe {
            Some(path) if is_writable_dir(&path) => path,
            _ => {
                let fallback = dirs::data_dir()
                    .map(|d| d.join("SpaceSpore").join("saves"))
                    .unwrap_or_else(|| PathBuf::from("saves"));
                fs::create_dir_all(&fallback).ok();
                fallback
            }
        };
        let dir = base.join(version_folder_name());
        let fresh = !dir.exists();
        fs::create_dir_all(&dir).ok();
        if fresh {
            migrate_saves(&base, &dir);
        }
        dir
    })
    .clone()
}

/// `v0.8.0`, `v0.8.0-instable` ou `v0.8.0-dev` (une compilation locale ne touche pas aux vraies sauvegardes).
pub fn version_folder_name() -> String {
    let suffix = match spacespore_common::CHANNEL {
        spacespore_common::Channel::Stable => "",
        spacespore_common::Channel::Unstable => "-instable",
        spacespore_common::Channel::Dev => "-dev",
    };
    format!("v{}{}", spacespore_common::VERSION, suffix)
}

/// Fichiers repris à la première ouverture d'une nouvelle version : ceux de l'ancien dossier
/// commun `saves/`, sinon ceux du dossier de la version la plus récemment utilisée.
const CARRIED_FILES: [&str; 3] = ["settings.json", "economy.json", "net_cache.json"];

fn migrate_saves(base: &std::path::Path, dir: &std::path::Path) {
    let legacy = CARRIED_FILES.iter().any(|f| base.join(f).exists());
    let source = if legacy {
        Some(base.to_path_buf())
    } else {
        fs::read_dir(base)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path() != dir && e.path().join("settings.json").exists())
            .max_by_key(|e| e.path().join("settings.json").metadata().and_then(|m| m.modified()).ok())
            .map(|e| e.path())
    };
    let Some(source) = source else { return };
    for name in CARRIED_FILES {
        let from = source.join(name);
        if from.exists() {
            let _ = fs::copy(&from, dir.join(name));
        }
    }
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
    /// Rayon de l'étoile qui l'éclaire (sert à la température).
    #[serde(default = "default_star_radius")] pub star_radius: f32,
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
fn default_star_radius()    -> f32 { 250.0 }
fn default_cloud_density()  -> f32 { 0.5 }
fn default_cloud_altitude() -> f32 { 20.0 }
fn default_cloud_speed()    -> f32 { 0.02 }
impl Default for PlanetConfig {
    fn default() -> Self {
        Self {
            orbit_distance: 2250.0, radius: 250.0, sea_level: 0.4,
            terrain_height: 110.0, seed: 42, noise_scale: 2.0, detail_scale: 4.0,
            moons: Vec::new(), star_radius: 250.0, atmosphere: false,
            cloud_density: 0.5, cloud_altitude: 100.0, cloud_speed: 0.02,
            eccentricity: 0.0, inclination: 0.0, ascending_node: 0.0,
            arg_periapsis: 0.0, mean_anomaly_0: 0.0,
        }
    }
}
impl PlanetConfig {
    /// -270 + PLANET_HEAT_RATIO / (distance en rayons d'étoile) : la chaleur dépend de la taille
    /// de l'étoile, pas de l'unité de distance.
    pub fn temperature(&self) -> f32 {
        -270.0 + PLANET_HEAT_RATIO * self.star_radius.max(1.0) / self.orbit_distance.max(10.0)
    }
}

/// Chaleur reçue par une planète, pour une distance exprimée en rayons de son étoile : brûlante
/// vers 2,4 rayons (≈ 190 °C), tempérée vers 4, glacée au-delà de 5.
const PLANET_HEAT_RATIO: f32 = 1100.0;

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
    /// Portée de la lumière d'une étoile de ce rayon : couvre les orbites les plus lointaines.
    pub fn light_range_for(radius: f32) -> f32 {
        radius * 16.0
    }

    /// Flux lumineux (lumens) : environ 3 000 lux à 3 rayons de l'étoile pour une intensité de 20.
    pub fn lumens(&self) -> f32 {
        let d = self.radius * 3.0;
        3_000.0 * (self.intensity / 20.0) * 4.0 * std::f32::consts::PI * d * d
    }

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

pub(crate) fn pseudo_rand(seed: u32) -> f32 {
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
/// Graine du monde par défaut (partagée par tous les joueurs).
pub const DEFAULT_WORLD_SEED: u64 = 42;
pub const GALAXY_RADIUS: f32 = 9_000_000.0;

/// Nombre de galaxies extérieures (ids 1..=NUM_DISTANT_GALAXIES).
pub const NUM_DISTANT_GALAXIES: usize = 100;
/// Aucun système n'est généré à moins de cette distance d'un trou noir galactique.
pub const CORE_EXCLUSION: f32 = 150_000.0;

/// Forme d'une galaxie. Index 0 = galaxie principale, 1.. = galaxies extérieures.
#[derive(Clone, Debug)]
pub struct GalaxyConfig {
    pub center:        bevy::math::Vec3,
    pub tilt:          bevy::math::Quat,
    pub radius:        f32,
    pub num_arms:      usize,
    pub twist:         f32,
    /// Type de galaxie (spirale, annulaire, filamentaire…) : voir `galaxy_shape`.
    pub kind:          crate::galaxy_shape::GalaxyKind,
    pub core_radius:   f32,
    /// Graine des étoiles de la galaxie (galaxies extérieures uniquement).
    pub seed:          u32,
    pub arm_stars:     usize,
    pub scatter_stars: usize,
}

/// Galaxie principale + galaxies extérieures disposées sur une méta-spirale.
pub fn default_galaxies(world_seed: u64) -> Vec<GalaxyConfig> {
    use bevy::math::{EulerRot, Quat, Vec3};
    const META_ARMS: usize = 5;
    const META_RADIUS: f32 = 200_000_000.0;
    const META_TWIST: f32 = 4.0;
    const MIN_DIST: f32 = 40_000_000.0;
    /// Deux galaxies restent séparées d'au moins ce multiple de la somme de leurs rayons.
    const SPACING: f32 = 2.0;
    let tau = std::f32::consts::TAU;

    let world_hash = {
        let x = (world_seed as u32) ^ ((world_seed >> 32) as u32);
        (pseudo_rand(x ^ 0x6A09_E667) * 65_535.0) as u32 * 2 + (x & 1)
    };
    let mut galaxies = Vec::with_capacity(NUM_DISTANT_GALAXIES + 1);
    galaxies.push(GalaxyConfig {
        center: Vec3::ZERO,
        tilt: Quat::IDENTITY,
        radius: GALAXY_RADIUS,
        num_arms: 5,
        twist: 5.0,
        kind: crate::galaxy_shape::GalaxyKind::Spiral,
        core_radius: 30_000.0,
        seed: 0,
        arm_stars: 0,
        scatter_stars: 0,
    });

    for gi in 0..NUM_DISTANT_GALAXIES {
        // La graine du monde décale tout : un autre monde, d'autres galaxies
        let gs = gi as u32 + 300_000 + world_hash % 90_000;
        let rk = |k: u32| pseudo_rand(gs * 13 + k);
        let rk_k = |n: u32, k: u32| pseudo_rand((gs * 13 + n).wrapping_add(k));

        let radius = 1_300_000.0 + (rk(7) * 0.6 + rk(27) * 0.4).powf(1.3) * 6_000_000.0;

        // Position de la galaxie sur les bras de la méta-spirale, à l'écart des autres :
        // on retire au sort jusqu'à trouver une place libre (à défaut, la moins serrée)
        let arm = gi % META_ARMS;
        let arm_base = arm as f32 * tau / META_ARMS as f32;
        let mut best: Option<(Vec3, f32)> = None;
        for attempt in 0..80u32 {
            let k = attempt.wrapping_mul(100_003);
            let t = rk_k(1, k);
            let r = MIN_DIST + t * t * (META_RADIUS - MIN_DIST);
            let spiral = arm_base + (r / META_RADIUS) * META_TWIST;
            let scatter = (rk_k(3, k) - 0.5) * 0.4;
            let theta = spiral + scatter;
            let c = Vec3::new(
                r * theta.cos(),
                (rk_k(5, k) - 0.5) * 16_000_000.0,
                r * theta.sin(),
            );
            // Marge restante par rapport au voisin le plus proche (>= 0 : place libre)
            let slack = galaxies
                .iter()
                .map(|g| c.distance(g.center) - SPACING * (g.radius + radius))
                .fold(f32::MAX, f32::min);
            if best.map_or(true, |(_, s)| slack > s) {
                best = Some((c, slack));
            }
            if slack >= 0.0 {
                break;
            }
        }
        let center = best.map_or(Vec3::ZERO, |(c, _)| c);

        galaxies.push(GalaxyConfig {
            center,
            tilt: Quat::from_euler(
                EulerRot::XYZ,
                (rk(13) - 0.5) * 1.5,
                rk(15) * tau,
                (rk(17) - 0.5) * 1.0,
            ),
            radius,
            num_arms: 2 + (rk(9) * 5.0) as usize,
            twist: 1.5 + rk(11) * 7.5,
            kind: crate::galaxy_shape::GalaxyKind::for_index(gi, world_hash),
            core_radius: 10_000.0 + rk(23) * 20_000.0,
            seed: gs * 1000,
            arm_stars: 170 + (rk(19) * 380.0) as usize,
            scatter_stars: 40 + (rk(31) * 120.0) as usize,
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

fn default_systems(galaxies: &[GalaxyConfig], world_seed: u64) -> Vec<StarSystemConfig> {
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

    // Tout ce qui vit dans un système (étoile, planètes, lunes) dépend de la graine du monde
    // (les positions suivent la forme des galaxies). Rien d'autre que + - * / ici : le résultat
    // doit être identique sur toutes les machines (empreinte du monde en multijoueur).
    let sd = ((world_seed as u32) ^ ((world_seed >> 32) as u32)).wrapping_mul(0x9E37_79B1);
    let mixed = |x: u32| x.wrapping_add(sd);

    // Une étoile fait 90 000 à 160 000 de rayon : au moins 100 fois ses planètes
    let make_star = |seed: u32| -> (f32, f32, [f32; 3]) {
        let seed = mixed(seed);
        let r_f = pseudo_rand(seed.wrapping_mul(5).wrapping_add(31));
        let radius = 90_000.0 + r_f * 70_000.0;
        let intensity = 8.0 + r_f * 27.0;
        let sc = star_color(seed.wrapping_mul(5).wrapping_add(37));
        (radius, intensity, sc)
    };

    // Proportions (R = rayon de l'étoile) : planète ≤ R/100, lune ≤ planète/3 ; première orbite
    // à 2,4 R, puis 1 à 1,6 R d'écart ; 1 à 3 planètes, 1 à 2 lunes chacune. Les planètes sont
    // assez petites pour tenir loin de l'étoile et rester explorables à pied (voir `surface.rs`).
    // `sb` : base des graines de planètes (doit rester loin de u32::MAX).
    let make_planets = |seed: u32, sb: u32, star_radius: f32| -> Vec<PlanetConfig> {
        let (seed, sb) = (mixed(seed), mixed(sb));
        let n = 1 + (pseudo_rand(seed.wrapping_mul(3).wrapping_add(41)) * 2.999) as usize;
        let mut orbit = 0.0_f32;
        let mut planets = Vec::with_capacity(n);
        for pi in 0..n {
            let pu = pi as u32;
            let r = |k: u32| pseudo_rand(sb.wrapping_add(pu).wrapping_add(k));
            orbit = if pi == 0 {
                star_radius * (2.4 + 1.2 * r(60))
            } else {
                orbit + star_radius * (1.0 + 0.6 * r(60))
            };
            let radius = star_radius / 100.0 * (0.5 + 0.45 * r(50));
            let atmosphere = pseudo_rand(seed.wrapping_mul(11).wrapping_add(pu).wrapping_add(71)) < 0.35;

            let moons = (0..1 + (r(90) * 1.999) as usize)
                .map(|mi| {
                    let mu = mi as u32;
                    let m = |k: u32| pseudo_rand(sb.wrapping_add(200 + pu * 16 + mu * 4 + k));
                    let moon_radius = radius / 3.0 * (0.7 + 0.3 * m(0));
                    MoonConfig {
                        orbit_distance: radius * 2.4 + moon_radius * 3.0 + mi as f32 * radius * 1.7 + m(1) * radius * 0.5,
                        radius: moon_radius,
                        seed: sb.wrapping_add(500 + pu * 8 + mu),
                        mean_anomaly_0: m(2) * std::f32::consts::TAU,
                        ..Default::default()
                    }
                })
                .collect();

            planets.push(PlanetConfig {
                orbit_distance: orbit,
                radius,
                sea_level: if atmosphere { 0.2 + r(83) * 0.4 } else { r(83) * 0.1 },
                terrain_height: radius * (0.025 + r(80) * 0.025),
                seed: sb.wrapping_add(pu),
                noise_scale: 1.5 + r(81) * 2.0,
                detail_scale: 3.0 + r(82) * 3.0,
                atmosphere,
                cloud_altitude: 80.0 + radius * 0.05 * (1.0 + r(84)),
                star_radius,
                moons,
                ..Default::default()
            });
        }
        planets
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
        // Pas d'étoile dans le trou noir central ni son disque
        if r < CORE_EXCLUSION { continue; }

        let spiral = arm_base + (r / gr) * ARM_TWIST;
        let width = 0.9 * (1.0 - r / gr * 0.5);
        let scatter = (pseudo_rand(s * 7 + 5) - 0.5) * width;
        let theta = spiral + scatter;

        let x = r * theta.cos();
        let z = r * theta.sin();
        let thickness = 60000.0 * (1.0 - r / gr * 0.7);
        let y = (pseudo_rand(s * 7 + 7) - 0.5) * thickness;

        let (sr, si, sc) = make_star(s);
        systems.push(StarSystemConfig {
            name: gen_name(i),
            position: [x, y, z],
            galaxy_id: 0,
            stars: vec![StarConfig {
                radius: sr, intensity: si,
                light_color_r: sc[0], light_color_g: sc[1], light_color_b: sc[2],
                light_range: StarConfig::light_range_for(sr),
                ..Default::default()
            }],
            planets: make_planets(s, (s + 1) * 100, sr),
            asteroid_belts: Vec::new(),
        });
    }

    // ── Étoiles dispersées entre les bras ─────────────────────────────
    for i in 0..SCATTER_STARS {
        let s = (ARM_STARS + i) as u32 + 100;
        let t = pseudo_rand(s * 7 + 3);
        let r = t * t * gr * 0.85;
        if r < CORE_EXCLUSION { continue; }
        let theta = pseudo_rand(s * 7 + 5) * tau;
        let x = r * theta.cos();
        let z = r * theta.sin();
        let y = (pseudo_rand(s * 7 + 7) - 0.5) * 3000.0;

        let (sr, si, sc) = make_star(s);
        systems.push(StarSystemConfig {
            name: gen_name(ARM_STARS + i),
            position: [x, y, z],
            galaxy_id: 0,
            stars: vec![StarConfig {
                radius: sr, intensity: si,
                light_color_r: sc[0], light_color_g: sc[1], light_color_b: sc[2],
                light_range: StarConfig::light_range_for(sr),
                ..Default::default()
            }],
            planets: make_planets(s, (s + 1) * 100, sr),
            asteroid_belts: Vec::new(),
        });
    }

    // ── Galaxies extérieures : mêmes systèmes (étoile + planètes) ─────
    for (gid, gal) in galaxies.iter().enumerate().skip(1) {
        let shape = gal.shape();
        let total = gal.arm_stars + gal.scatter_stars;
        let on_structure = (total as f32 * shape.structure_share) as usize;
        let mut local_idx = 0usize;
        for i in 0..total {
            let s = i as u32 + gal.seed;
            let mut rng = crate::galaxy_shape::Rng::new(s);
            let local = if i < on_structure {
                shape.sample_structure(&mut rng, 0.0)
            } else {
                shape.sample_background(&mut rng)
            };
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
                    light_range: StarConfig::light_range_for(sr),
                    ..Default::default()
                }],
                planets: make_planets(s, (global_idx + 1) * 100, sr),
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
    /// Tag de clan / guilde affiché entre crochets devant le pseudo (vide = sans guilde).
    #[serde(default)]                         pub clan_tag: String,
    /// Systèmes stellaires revendiqués par le joueur (5 au plus).
    #[serde(default)]                         pub claims: Vec<u32>,
    /// Factions déclarées alliées / ennemies (les autres sont neutres).
    #[serde(default)]                         pub allies: Vec<String>,
    #[serde(default)]                         pub enemies: Vec<String>,
    /// Identifiant permanent du joueur (tiré au hasard au premier lancement).
    #[serde(default)]                         pub player_id: u64,
    /// Trous de ver déjà empruntés (système de départ) : leur destination est alors connue.
    #[serde(default)]                         pub known_wormholes: Vec<u32>,
    /// Ma guilde (fiche complète), et fiches gardées après un départ ou une dissolution.
    #[serde(default)]                         pub guild: Option<crate::guild::GuildRecord>,
    #[serde(default)]                         pub guild_archive: Vec<crate::guild::GuildRecord>,
    /// Ce jeu a pris une identité de secours (un autre jeu utilisait la même sauvegarde) :
    /// il ne réécrit plus `settings.json`, pour ne pas écraser le compte de l'autre.
    #[serde(skip)]                            pub temp_identity: bool,

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
            world_seed: DEFAULT_WORLD_SEED,
            player_name: default_player_name(),
            aura_color: default_aura_color(),
            last_join_address: String::new(),
            clan_tag: String::new(),
            claims: Vec::new(),
            allies: Vec::new(),
            enemies: Vec::new(),
            player_id: 0,
            known_wormholes: Vec::new(),
            guild: None,
            guild_archive: Vec::new(),
            temp_identity: false,
            systems: default_systems(&default_galaxies(DEFAULT_WORLD_SEED), DEFAULT_WORLD_SEED),
            galaxies: default_galaxies(DEFAULT_WORLD_SEED),
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
        s.apply_world_save();
        s.galaxies = default_galaxies(s.world_seed);
        s.systems = default_systems(&s.galaxies, s.world_seed);
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
        // Nouvelle version (ou ancienne sauvegarde) : on écrit tout de suite world.json et info.json
        if !data_dir().join("world.json").exists() {
            s.save();
        }
        s
    }

    pub fn save(&self) {
        // Identité de secours : la sauvegarde appartient à l'autre jeu
        if self.temp_identity {
            return;
        }
        // Les tests ne touchent pas aux fichiers du joueur
        if cfg!(test) {
            return;
        }
        let path = Self::config_path();
        if let Ok(json) = serde_json::to_string(self) {
            fs::write(path, json).ok();
        }
        let world = WorldSave::from(self);
        if let Ok(json) = serde_json::to_string_pretty(&world) {
            fs::write(data_dir().join("world.json"), json).ok();
        }
        let info = SaveInfo::from(self);
        if let Ok(json) = serde_json::to_string_pretty(&info) {
            fs::write(data_dir().join("info.json"), json).ok();
        }
    }

    /// La sauvegarde du monde prime sur `settings.json` pour tout ce qui la compose.
    fn apply_world_save(&mut self) {
        let Ok(text) = fs::read_to_string(data_dir().join("world.json")) else { return };
        let Ok(world) = serde_json::from_str::<WorldSave>(&text) else { return };
        self.world_seed = world.world_seed;
        self.claims = world.claims;
        self.allies = world.allies;
        self.enemies = world.enemies;
        self.known_wormholes = world.known_wormholes;
        self.clan_tag = world.clan_tag;
        self.guild = world.guild;
        self.guild_archive = world.guild_archive;
    }
}

/// Sauvegarde du monde : sa graine et ce que le joueur y a fait (`world.json`).
#[derive(Serialize, Deserialize)]
struct WorldSave {
    version: String,
    world_seed: u64,
    #[serde(default)] claims: Vec<u32>,
    #[serde(default)] allies: Vec<String>,
    #[serde(default)] enemies: Vec<String>,
    #[serde(default)] known_wormholes: Vec<u32>,
    #[serde(default)] clan_tag: String,
    #[serde(default)] guild: Option<crate::guild::GuildRecord>,
    #[serde(default)] guild_archive: Vec<crate::guild::GuildRecord>,
}

impl From<&GameSettings> for WorldSave {
    fn from(s: &GameSettings) -> Self {
        Self {
            version: spacespore_common::VERSION.to_string(),
            world_seed: s.world_seed,
            claims: s.claims.clone(),
            allies: s.allies.clone(),
            enemies: s.enemies.clone(),
            known_wormholes: s.known_wormholes.clone(),
            clan_tag: s.clan_tag.clone(),
            guild: s.guild.clone(),
            guild_archive: s.guild_archive.clone(),
        }
    }
}

/// Résumé lisible de la partie (`info.json`) : jamais relu par le jeu.
#[derive(Serialize)]
struct SaveInfo {
    game_version: String,
    save_version: u32,
    world_seed: u64,
    galaxies: usize,
    systems: usize,
    player_name: String,
    player_id: u64,
    clan_tag: String,
    claimed_systems: usize,
    known_wormholes: usize,
    last_saved_unix: u64,
}

impl From<&GameSettings> for SaveInfo {
    fn from(s: &GameSettings) -> Self {
        Self {
            game_version: spacespore_common::VERSION.to_string(),
            save_version: s.save_version,
            world_seed: s.world_seed,
            galaxies: s.galaxies.len(),
            systems: s.systems.len(),
            player_name: s.player_name.clone(),
            player_id: s.player_id,
            clan_tag: s.clan_tag.clone(),
            claimed_systems: s.claims.len(),
            known_wormholes: s.known_wormholes.len(),
            last_saved_unix: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs()),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn galaxies_do_not_touch_each_other() {
        let g = default_galaxies(DEFAULT_WORLD_SEED);
        let mut worst = f32::MAX;
        for i in 0..g.len() {
            for j in (i + 1)..g.len() {
                let gap = g[i].center.distance(g[j].center) / (g[i].radius + g[j].radius);
                worst = worst.min(gap);
            }
        }
        // Jamais collées : au moins 1,5 fois la somme des rayons entre deux centres
        assert!(worst >= 1.5, "deux galaxies trop proches : {worst}");
    }

    #[test]
    fn stars_dwarf_planets_and_planets_dwarf_moons() {
        let galaxies = default_galaxies(DEFAULT_WORLD_SEED);
        let systems = default_systems(&galaxies, DEFAULT_WORLD_SEED);
        let (mut hot, mut temperate, mut cold) = (0, 0, 0);
        for sys in systems.iter().take(3000) {
            let star = &sys.stars[0];
            assert!((90_000.0..=160_000.0).contains(&star.radius), "etoile de rayon {}", star.radius);
            assert!((1..=3).contains(&sys.planets.len()), "{} planetes", sys.planets.len());
            let mut previous_edge = star.radius;
            for p in &sys.planets {
                // Étoile au moins 100 fois plus grande que la planète, lune au moins 3 fois plus petite
                assert!(p.radius * 100.0 <= star.radius, "planete {} pour une etoile de {}", p.radius, star.radius);
                assert!(p.radius >= star.radius / 100.0 * 0.5 - 1.0);
                assert!(p.orbit_distance - p.radius > previous_edge, "planete dans l'etoile ou dans la precedente");
                previous_edge = p.orbit_distance + p.radius;
                assert!(p.orbit_distance + p.radius < star.radius * 8.0, "systeme trop etendu");
                assert!(p.terrain_height > 0.0 && p.terrain_height < p.radius * 0.06);
                assert_eq!(p.star_radius, star.radius);
                assert!((1..=2).contains(&p.moons.len()), "{} lunes", p.moons.len());
                let mut moon_edge = p.radius + p.terrain_height;
                for m in &p.moons {
                    assert!(m.radius * 3.0 <= p.radius * 1.0001, "lune {} pour une planete de {}", m.radius, p.radius);
                    assert!(m.orbit_distance - m.radius > moon_edge, "lune dans la planete ou dans la lune precedente");
                    moon_edge = m.orbit_distance + m.radius;
                }
                match p.temperature() {
                    t if t > 100.0 => hot += 1,
                    t if t > -20.0 => temperate += 1,
                    _ => cold += 1,
                }
            }
        }
        // Des mondes brulants, temperes et glaces : de quoi varier les paysages
        assert!(hot > 100 && temperate > 100 && cold > 100, "chaud {hot}, tempere {temperate}, froid {cold}");
    }

    #[test]
    fn the_world_seed_changes_stars_and_planets() {
        let galaxies = default_galaxies(DEFAULT_WORLD_SEED);
        let a = default_systems(&galaxies, 42);
        let b = default_systems(&galaxies, 43);
        let different = a.iter().zip(&b).take(300).filter(|(x, y)| {
            x.stars[0].radius.to_bits() != y.stars[0].radius.to_bits()
                || x.planets.len() != y.planets.len()
                || x.planets[0].radius.to_bits() != y.planets[0].radius.to_bits()
        });
        assert!(different.count() > 250);
    }

    #[test]
    fn a_new_version_folder_inherits_the_latest_saves() {
        let base = std::env::temp_dir().join(format!("spacespore-saves-test-{}", std::process::id()));
        let old = base.join("v0.7.0");
        let new = base.join("v0.8.0");
        fs::create_dir_all(&old).unwrap();
        fs::create_dir_all(&new).unwrap();
        fs::write(old.join("settings.json"), "{\"player_id\":7}").unwrap();
        fs::write(old.join("economy.json"), "{}").unwrap();
        migrate_saves(&base, &new);
        assert_eq!(fs::read_to_string(new.join("settings.json")).unwrap(), "{\"player_id\":7}");
        assert!(new.join("economy.json").exists());
        // Le dossier de l'ancienne version n'est pas touché
        assert!(old.join("settings.json").exists());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn world_save_keeps_the_seed_and_the_player_progress() {
        let mut s = GameSettings::default();
        s.world_seed = 1234;
        s.claims = vec![3, 9];
        s.known_wormholes = vec![5];
        let json = serde_json::to_string(&WorldSave::from(&s)).unwrap();
        let back: WorldSave = serde_json::from_str(&json).unwrap();
        assert_eq!((back.world_seed, back.claims, back.known_wormholes), (1234, vec![3, 9], vec![5]));
        assert!(version_folder_name().starts_with('v'));
    }

    #[test]
    fn the_generated_world_is_reproducible() {
        let galaxies = default_galaxies(DEFAULT_WORLD_SEED);
        let a = default_systems(&galaxies, DEFAULT_WORLD_SEED);
        let b = default_systems(&galaxies, DEFAULT_WORLD_SEED);
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(&b).take(500) {
            assert_eq!(x.planets.len(), y.planets.len());
            for (p, q) in x.planets.iter().zip(&y.planets) {
                assert_eq!(p.radius.to_bits(), q.radius.to_bits());
                assert_eq!(p.orbit_distance.to_bits(), q.orbit_distance.to_bits());
                assert_eq!(p.moons.len(), q.moons.len());
            }
        }
    }
}
