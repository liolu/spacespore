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
    /// Physique réelle (phase 2) : rayon (R⊕), masse (M⊕), gravité de surface (g).
    #[serde(default)] pub radius_earth:   f32,
    #[serde(default)] pub mass_earth:     f32,
    #[serde(default = "default_moon_gravity")] pub gravity_g: f32,
    /// Climat (phase 3) : lune sans air.
    #[serde(default)] pub climate:        Option<Climate>,
    /// Relief (phase 5) : cratères, volcans d'une lune chauffée par les marées.
    #[serde(default)] pub relief:         Option<Relief>,
    // ── Toute la chaîne de génération (phase 9) : une lune est un monde comme un autre ──
    #[serde(default)] pub sea_level:      f32,
    #[serde(default)] pub terrain_height: f32,
    #[serde(default)] pub noise_scale:    f32,
    #[serde(default)] pub detail_scale:   f32,
    #[serde(default)] pub atmosphere:     bool,
    /// Chauffage par les marées de sa planète (0..1).
    #[serde(default)] pub tidal_heat:     f32,
    #[serde(default)] pub air:            Air,
    #[serde(default)] pub hydrology:      Hydrology,
    #[serde(default)] pub geology:        Geology,
    #[serde(default)] pub biomes:         BiomeParams,
    #[serde(default)] pub habitability:   Habitability,
    #[serde(default)] pub traits:         Vec<Trait>,
    #[serde(default)] pub life:           Life,
    /// Composition globale et gisements (phase 8).
    #[serde(default)] pub resources:      crate::planetgen::resources::Resources,
}
fn default_moon_gravity() -> f32 { 0.16 }
impl MoonConfig {
    /// La chaîne de génération a été appliquée à cette lune (phase 9).
    pub fn generated(&self) -> bool {
        self.biomes.defined
    }

    /// La lune vue comme une planète rocheuse (même terrain, même rendu, même profil), à la
    /// distance de l'étoile de sa planète.
    pub fn as_planet(&self, parent: &PlanetConfig) -> PlanetConfig {
        PlanetConfig {
            orbit_distance: self.orbit_distance,
            radius: self.radius,
            sea_level: self.sea_level,
            terrain_height: if self.terrain_height > 0.0 { self.terrain_height } else { self.radius * 0.045 },
            seed: self.seed,
            noise_scale: self.noise_scale,
            detail_scale: self.detail_scale,
            moons: Vec::new(),
            star_radius: parent.star_radius,
            atmosphere: self.atmosphere,
            cloud_density: self.air.cloud_cover,
            cloud_altitude: 40.0 + self.radius * 0.05,
            cloud_speed: 0.02 * (self.air.wind_ms / 10.0).clamp(0.2, 4.0),
            eccentricity: self.eccentricity,
            inclination: self.inclination,
            ascending_node: self.ascending_node,
            arg_periapsis: self.arg_periapsis,
            mean_anomaly_0: self.mean_anomaly_0,
            kind: PlanetKind::Rocky,
            hot: false,
            mass_earth: self.mass_earth,
            radius_earth: self.radius_earth,
            semi_major_au: parent.semi_major_au,
            period_days: 0.0,
            rotation_h: 0.0,
            axial_tilt: 0.0,
            tidally_locked: true,
            gravity_g: self.gravity_g,
            temperature_c: self.climate.map(|c| c.mean_c),
            climate: self.climate,
            air: self.air.clone(),
            hydrology: self.hydrology.clone(),
            geology: self.geology.clone(),
            biomes: self.biomes,
            ring: None,
            aurora: None,
            habitability: self.habitability.clone(),
            traits: self.traits.clone(),
            life: self.life.clone(),
            resources: self.resources.clone(),
            rogue: false,
        }
    }
}
impl Default for MoonConfig {
    fn default() -> Self { Self {
        orbit_distance: 400.0, radius: 60.0, seed: 77,
        eccentricity: 0.0, inclination: 0.0, ascending_node: 0.0,
        arg_periapsis: 0.0, mean_anomaly_0: 0.0,
        radius_earth: 0.0, mass_earth: 0.0, gravity_g: default_moon_gravity(), climate: None, relief: None,
        sea_level: 0.5, terrain_height: 0.0, noise_scale: 2.0, detail_scale: 4.0, atmosphere: false, tidal_heat: 0.0,
        air: Air::default(), hydrology: Hydrology::default(), geology: Geology::default(), biomes: BiomeParams::default(),
        habitability: Habitability::default(), traits: Vec::new(), life: Life::default(),
        resources: Default::default(),
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
    // ── Physique réelle (phase 2 de `roadmaps/fait/ROADMAP-0.10.md`, voir `planetgen::system`) ──
    /// Rocheuse, mini-Neptune, géante de glace ou gazeuse (sans sol).
    #[serde(default)] pub kind:           PlanetKind,
    /// Géante chaude (Jupiter chaud, tout près de son étoile).
    #[serde(default)] pub hot:            bool,
    #[serde(default)] pub mass_earth:     f32,
    #[serde(default)] pub radius_earth:   f32,
    /// Distance physique à l'étoile (UA) ; `orbit_distance` est la distance affichée.
    #[serde(default)] pub semi_major_au:  f32,
    #[serde(default)] pub period_days:    f32,
    /// Période de rotation (heures) : stockée, le jour/nuit vient en 0.11.
    #[serde(default)] pub rotation_h:     f32,
    /// Inclinaison de l'axe (degrés).
    #[serde(default)] pub axial_tilt:     f32,
    /// Rotation synchrone : une face toujours tournée vers l'étoile.
    #[serde(default)] pub tidally_locked: bool,
    /// Gravité de surface (g).
    #[serde(default = "default_gravity")] pub gravity_g: f32,
    /// Température moyenne (°C) ; `None` : ancienne formule (distance en rayons d'étoile).
    #[serde(default)] pub temperature_c:  Option<f32>,
    /// Climat (phase 3) : température selon la latitude et l'altitude ; `None` : d'après
    /// `temperature()`.
    #[serde(default)] pub climate:        Option<Climate>,
    /// Atmosphère (phase 3) : composition, pression, nuages, vents, couleurs du ciel.
    #[serde(default)] pub air:            Air,
    /// Eau et glace (phase 4) : liquide des mers, couverture, calottes, eau souterraine.
    #[serde(default)] pub hydrology:      Hydrology,
    /// Géologie et relief (phase 5).
    #[serde(default)] pub geology:        Geology,
    /// Sols et biomes (phase 6).
    #[serde(default)] pub biomes:         BiomeParams,
    /// Anneaux et aurores, habitabilité, dangers et traits (phase 9).
    #[serde(default)] pub ring:           Option<Ring>,
    #[serde(default)] pub aurora:         Option<Aurora>,
    #[serde(default)] pub habitability:   Habitability,
    #[serde(default)] pub traits:         Vec<Trait>,
    /// Vie (phase 7) : niveau, chimie, plantes, faune (paramètres).
    #[serde(default)] pub life:           Life,
    /// Composition globale et gisements (phase 8).
    #[serde(default)] pub resources:      crate::planetgen::resources::Resources,
    /// Planète errante (C2) : loin de toute étoile, elle ne tourne pas autour (dernière de la liste).
    #[serde(default)] pub rogue:          bool,
}
fn default_gravity() -> f32 { 1.0 }
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
            kind: PlanetKind::Rocky, hot: false, mass_earth: 0.0, radius_earth: 0.0,
            semi_major_au: 0.0, period_days: 0.0, rotation_h: 0.0, axial_tilt: 0.0,
            tidally_locked: false, gravity_g: 1.0, temperature_c: None, climate: None, air: Air::default(), hydrology: Hydrology::default(), geology: Geology::default(), biomes: BiomeParams::default(),
            ring: None, aurora: None, habitability: Habitability::default(), traits: Vec::new(), life: Life::default(), resources: Default::default(), rogue: false,
        }
    }
}
impl PlanetConfig {
    /// Température moyenne (°C) : celle de la génération (insolation de l'étoile en UA), sinon
    /// -270 + PLANET_HEAT_RATIO / (distance en rayons d'étoile) pour une planète faite à la main.
    pub fn temperature(&self) -> f32 {
        self.temperature_c.unwrap_or_else(|| -270.0 + PLANET_HEAT_RATIO * self.star_radius.max(1.0) / self.orbit_distance.max(10.0))
    }

    /// Géante gazeuse ou neptunienne : pas de sol.
    pub fn gaseous(&self) -> bool {
        self.kind.gaseous()
    }

    /// Climat : celui de la génération, sinon déduit de la température moyenne.
    pub fn climate(&self) -> Climate {
        self.climate.unwrap_or_else(|| Climate::from_mean(self.temperature(), self.atmosphere))
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
    /// Type de l'étoile (phase 1 de `roadmaps/fait/ROADMAP-0.10.md`) ; sa physique complète se recalcule depuis
    /// la graine du système (`planetgen::star::StarPhysics`).
    #[serde(default)]                                 pub class:          StarClass,
    /// Température de surface (K) ; 0 = inconnue (déduite de la couleur).
    #[serde(default)]                                 pub temperature_k:  f32,
    /// Rayon qu'aurait une étoile G dans ce système (600 000 à 1 500 000) : l'échelle des orbites,
    /// des planètes et de la lumière, quel que soit le type. 0 = le rayon de l'étoile.
    #[serde(default)]                                 pub orbit_scale:    f32,
    /// Étoile double ou triple (C3) : son orbite autour du centre du système ; `None` : au centre.
    #[serde(default)]                                 pub orbit:          Option<crate::planetgen::multiple::StarOrbit>,
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
        radius * 16.0 * SPACE_STRETCH
    }

    /// Étoile générée d'après sa physique ; `g_radius` : échelle G du système.
    pub fn from_physics(p: &StarPhysics, g_radius: f32) -> Self {
        let radius = p.render_radius(g_radius as f64);
        let (flare_count, flare_height, flare_speed) = p.flares();
        Self {
            radius,
            intensity: p.light_intensity(),
            // Couvre les orbites (échelle G), même repoussées hors d'une géante
            light_range: Self::light_range_for(g_radius) + radius * 2.0,
            light_color_r: p.color[0],
            light_color_g: p.color[1],
            light_color_b: p.color[2],
            flare_count,
            flare_height,
            flare_speed,
            class: p.class,
            temperature_k: p.temperature_k as f32,
            orbit_scale: g_radius,
            ..Default::default()
        }
    }

    /// Lumière propre de la surface (matériau émissif) : couleur du corps noir, plus forte pour
    /// une étoile lumineuse, plus faible pour une naine brune.
    pub fn emissive_rgb(&self) -> [f32; 3] {
        let k = 10.0 * (self.intensity / 20.0).sqrt().clamp(0.4, 1.6);
        [self.light_color_r * 1.1 * k, self.light_color_g * k, self.light_color_b * 0.9 * k]
    }

    /// Éclat d'une étoile vue de loin (taille du point), 1 pour le Soleil.
    pub fn glow(&self) -> f32 {
        (self.intensity / 20.0).powf(0.6).clamp(0.3, 2.0)
    }

    /// Échelle des orbites et de la lumière (rayon d'une G dans ce système).
    pub fn scale(&self) -> f32 {
        if self.orbit_scale > 0.0 { self.orbit_scale } else { self.radius }
    }

    /// Flux lumineux (lumens) : environ 3 000 lux à 3 fois l'échelle du système (~3 rayons d'une
    /// G, étirés comme les orbites) pour une intensité de 20, quelle que soit la taille réelle de
    /// l'étoile : les planètes, plus loin, reçoivent la même lumière qu'avant l'étirement.
    pub fn lumens(&self) -> f32 {
        let d = self.scale() * 3.0 * SPACE_STRETCH;
        3_000.0 * (self.intensity / 20.0) * 4.0 * std::f32::consts::PI * d * d
    }

    pub fn temperature(&self) -> f32 {
        if self.temperature_k > 0.0 {
            return self.temperature_k;
        }
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
            class: StarClass::G, temperature_k: 0.0, orbit_scale: 0.0, orbit: None,
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
//  Origine flottante
//
//  Les positions du monde sont des `f32`, qui n'ont que ~7 chiffres : à 100 millions d'unités du
//  centre, une unité de précision en vaut 8. La galaxie fait des centaines de millions d'unités,
//  mais on ne regarde de près que ce qui est autour du joueur. Les positions « monde » (celles
//  des entités Bevy) sont donc relatives à une ORIGINE mobile, en `f64`, que `origin.rs`
//  rapproche du vaisseau dès qu'il s'en éloigne : coordonnée absolue = origine + position monde.
//  Les positions absolues (systèmes, galaxies, trous de ver) restent, elles, immuables.
// ─────────────────────────────────────────────────────────────────────────

use bevy::math::{DVec3, Vec3};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

use crate::planetgen::genome::SystemGenome;
use crate::planetgen::star::{StarClass, StarPhysics};
use crate::planetgen::atmosphere::Air;
use crate::planetgen::climate::Climate;
use crate::planetgen::biome::BiomeParams;
use crate::planetgen::habitability::Habitability;
use crate::planetgen::life::Life;
use crate::planetgen::profile::Trait;
use crate::planetgen::system::{Aurora, Ring};
use crate::planetgen::geology::{Geology, Relief};
use crate::planetgen::hydrology::Hydrology;
use crate::planetgen::system::PlanetKind;
use crate::planetgen::live::WorldDeltas;

static ORIGIN: [AtomicU64; 3] = [AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0)];

/// Origine du repère monde, en coordonnées absolues.
pub fn origin() -> DVec3 {
    let f = |i: usize| f64::from_bits(ORIGIN[i].load(Ordering::Relaxed));
    DVec3::new(f(0), f(1), f(2))
}

pub fn set_origin(o: DVec3) {
    for (i, v) in [o.x, o.y, o.z].into_iter().enumerate() {
        ORIGIN[i].store(v.to_bits(), Ordering::Relaxed);
    }
}

/// Position absolue → position monde (relative à l'origine).
pub fn to_local(abs: Vec3) -> Vec3 {
    (abs.as_dvec3() - origin()).as_vec3()
}

/// Position monde → position absolue.
pub fn to_abs(local: Vec3) -> DVec3 {
    local.as_dvec3() + origin()
}

// ─────────────────────────────────────────────────────────────────────────
//  Système stellaire
// ─────────────────────────────────────────────────────────────────────────

/// Un système stellaire. Ses planètes et ses lunes ne sont pas stockées (règle 1 de
/// `roadmaps/fait/ROADMAP-0.10.md`) : `planets()` les recalcule depuis le génome à la première demande et les
/// garde en cache tant que le système est proche du vaisseau (`planetgen::cache`).
#[derive(Clone, Debug)]
pub struct StarSystemConfig {
    pub name: String,
    pub position: [f32; 3],
    /// Galaxie d'appartenance (0 = galaxie principale, 1.. = galaxies extérieures).
    pub galaxy_id:      u32,
    pub stars:          Vec<StarConfig>,
    pub asteroid_belts: Vec<AsteroidBeltConfig>,
    /// Graines des planètes ; `None` : planètes données explicitement (éditeur), jamais recalculées.
    genome: Option<SystemGenome>,
    /// Planètes recalculées (cache), ou planètes explicites si `genome` est `None`.
    planets: OnceLock<Vec<PlanetConfig>>,
}

impl Default for StarSystemConfig {
    fn default() -> Self {
        Self {
            name: "Systeme Sol".into(),
            position: [0.0, 0.0, 0.0],
            galaxy_id: 0,
            stars: default_stars(),
            asteroid_belts: Vec::new(),
            genome: None,
            planets: OnceLock::from(default_planets()),
        }
    }
}

impl StarSystemConfig {
    fn generated(name: String, position: [f32; 3], galaxy_id: u32, star: (StarConfig, StarPhysics), genome: SystemGenome) -> Self {
        let (mut primary, physics) = star;
        // Étoiles doubles et triples (C3) : compagnons d'après la graine et l'étoile principale
        let st = crate::planetgen::multiple::generate(genome.seed as u64, &physics, primary.scale() as f64, primary.radius as f64);
        primary.orbit = st.primary;
        let g = primary.scale();
        let mut stars = Vec::with_capacity(1 + st.companions.len());
        stars.push(primary);
        stars.extend(st.companions.iter().map(|c| StarConfig { orbit: Some(c.orbit), ..StarConfig::from_physics(&c.physics, g) }));
        Self { name, position, galaxy_id, stars, asteroid_belts: Vec::new(), genome: Some(genome), planets: OnceLock::new() }
    }

    fn generate_planets(&self) -> Vec<PlanetConfig> {
        let (Some(genome), Some(star), Some(light), Some(st)) = (self.genome, self.stars.first(), self.lighting(), self.stellar()) else {
            return Vec::new();
        };
        genome.planets(&light, star.scale(), star.radius, st.limits())
    }

    /// Étoiles du système (C3) : double, triple, et ce qu'elles imposent aux orbites.
    pub fn stellar(&self) -> Option<crate::planetgen::multiple::Stellar> {
        let (physics, star) = (self.star_physics()?, self.stars.first()?);
        Some(crate::planetgen::multiple::generate(self.genome?.seed as u64, &physics, star.scale() as f64, star.radius as f64))
    }

    /// La lumière que reçoivent les planètes : l'étoile principale, avec la luminosité des étoiles
    /// du centre additionnées (paire serrée, C3).
    pub fn lighting(&self) -> Option<StarPhysics> {
        let mut p = self.star_physics()?;
        if let Some(st) = self.stellar() {
            p.luminosity_sun = st.luminosity;
            p.mass_sun = st.mass;
        }
        Some(p)
    }

    /// Physique de l'étoile `i` (0 : la principale, puis les compagnons).
    pub fn star_physics_of(&self, i: usize) -> Option<StarPhysics> {
        if i == 0 {
            return self.star_physics();
        }
        self.stellar()?.companions.get(i - 1).map(|c| c.physics.clone())
    }

    /// Planètes du système (et leurs lunes), recalculées à la première demande.
    pub fn planets(&self) -> &[PlanetConfig] {
        self.planets.get_or_init(|| self.generate_planets())
    }

    /// Ceintures d'astéroïdes (C1) : recalculées depuis le génome et les planètes ; celles de
    /// l'éditeur pour un système fait à la main.
    pub fn belts(&self) -> Vec<crate::planetgen::belts::Belt> {
        match (self.genome, self.stars.first(), self.lighting(), self.stellar()) {
            (Some(genome), Some(star), Some(physics), Some(st)) => {
                crate::planetgen::belts::generate(genome, &physics, star.scale(), star.radius, &self.planets_uncached(), st.limits())
            }
            _ => self.asteroid_belts.iter().enumerate().map(|(i, c)| crate::planetgen::belts::Belt::from_config(c, i)).collect(),
        }
    }

    /// Troyens (C2) : essaims aux points L4 / L5 des géantes.
    pub fn swarms(&self) -> Vec<crate::planetgen::belts::Swarm> {
        match self.genome {
            Some(genome) => crate::planetgen::belts::trojans(genome, &self.planets_uncached()),
            None => Vec::new(),
        }
    }

    /// Comètes (C2) : orbites très excentriques.
    pub fn comets(&self) -> Vec<crate::planetgen::comets::Comet> {
        match (self.genome, self.stars.first(), self.lighting(), self.stellar()) {
            (Some(genome), Some(star), Some(physics), Some(st)) => {
                crate::planetgen::comets::generate(genome, &physics, star.scale(), star.radius, &self.planets_uncached(), &self.belts(), st.limits())
            }
            _ => Vec::new(),
        }
    }

    /// Planètes sans remplir le cache (pour parcourir tous les systèmes d'un coup).
    pub fn planets_uncached(&self) -> std::borrow::Cow<'_, [PlanetConfig]> {
        match self.planets.get() {
            Some(p) => std::borrow::Cow::Borrowed(p),
            None => std::borrow::Cow::Owned(self.generate_planets()),
        }
    }

    /// Planètes modifiables (éditeur) : elles deviennent explicites et ne sont plus recalculées.
    pub fn planets_mut(&mut self) -> &mut Vec<PlanetConfig> {
        self.planets();
        self.genome = None;
        self.planets.get_mut().expect("planetes calculees juste avant")
    }

    /// Les planètes sont actuellement en mémoire.
    pub fn planets_cached(&self) -> bool {
        self.genome.is_some() && self.planets.get().is_some()
    }

    /// Libère les planètes recalculables (elles seront recalculées à la prochaine demande).
    pub fn forget_planets(&mut self) {
        if self.genome.is_some() {
            self.planets.take();
        }
    }

    /// Physique complète de l'étoile principale, recalculée depuis la graine (`None` : étoile
    /// donnée à la main, sans génome).
    pub fn star_physics(&self) -> Option<StarPhysics> {
        let seed = self.genome?.seed;
        let rank = pseudo_rand(seed.wrapping_mul(5).wrapping_add(31));
        Some(StarPhysics::generate(seed as u64, rank as f64, None))
    }

    /// Graine du système (sert aux sous-graines de l'étoile).
    pub fn body_seed(&self) -> u64 {
        self.genome.map_or(0, |g| g.seed as u64)
    }

    /// Position absolue (immuable) du centre du système.
    pub fn abs_center(&self) -> Vec3 {
        Vec3::new(self.position[0], self.position[1], self.position[2])
    }

    /// Position du centre dans le repère monde (relatif à l'origine flottante).
    pub fn center(&self) -> Vec3 {
        to_local(self.abs_center())
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
/// Échelle de la galaxie : toutes les distances entre étoiles (et entre galaxies) sont multipliées
/// par ce facteur, les systèmes eux-mêmes (étoile, planètes, lunes) gardent leur taille. Sans
/// limite de précision grâce à l'origine flottante (voir plus haut et `origin.rs`) : la galaxie
/// principale fait 2,7 milliards de rayon ; la voisine la plus proche d'une étoile est à ~15 M en
/// médiane (~4,5 M dans les autres galaxies), plus qu'un système avec ses 1 à 8 planètes (~4 M en
/// médiane, 11 M pour 99 % d'entre eux). 300 depuis la phase 2 (100 avant).
pub const GALAXY_SCALE: f32 = 300.0 * SPACE_STRETCH;
/// Étirement visuel des distances (×5 depuis la 0.11) : entre les étoiles, les galaxies, et les
/// orbites des planètes et des lunes. Les tailles des astres et toute la physique (UA, périodes,
/// températures, marées) ne changent pas ; les orbites affichées tournent selon Kepler, donc plus
/// lentement.
pub const SPACE_STRETCH: f32 = 5.0;
/// Échelle des tailles galactiques (trous noirs centraux) : celle d'avant l'étirement.
pub const GALAXY_SIZE_SCALE: f32 = 300.0;
pub const SYSTEM_CELL_SIZE: f32 = 100_000.0 * GALAXY_SCALE;
/// Graine du monde par défaut (partagée par tous les joueurs).
pub const DEFAULT_WORLD_SEED: u64 = 42;
pub const GALAXY_RADIUS: f32 = 9_000_000.0 * GALAXY_SCALE;

/// Nombre de galaxies extérieures (ids 1..=NUM_DISTANT_GALAXIES). Chaque galaxie a 5 000 à
/// 10 000 étoiles : à 100 galaxies, cela ferait plus de 700 000 systèmes (plusieurs Go de mémoire
/// et des centaines de milliers d'entités à afficher). 20 galaxies : une de chaque type (voir
/// `GalaxyKind`), soit ~160 000 systèmes.
pub const NUM_DISTANT_GALAXIES: usize = 20;
/// Galaxies lointaines (ids après les extérieures) : de 1,3 à 10 fois la distance de la plus
/// lointaine des galaxies extérieures, 10 000 galaxies en tout. De vraies galaxies (systèmes,
/// trous noirs, trous de ver, factions), générées quand on s'en approche ou qu'on y accède
/// (`systems::Systems`) ; de très loin, de simples points comme des étoiles.
pub const NUM_OUTER_GALAXIES: usize = 10_000 - 1 - NUM_DISTANT_GALAXIES;

/// Nombre d'étoiles d'une galaxie, le même intervalle pour toutes : (étoiles de bras, dispersées).
pub fn star_budget(seed: u32) -> (usize, usize) {
    let total = 5_000 + (pseudo_rand(seed.wrapping_mul(0x9E37_79B1) ^ 0x57A2) * 5_000.0) as usize;
    let arm = total * 8 / 10;
    (arm, total - arm)
}
/// Aucun système n'est généré à moins de cette distance d'un trou noir galactique.
pub const CORE_EXCLUSION: f32 = 150_000.0 * GALAXY_SCALE;

/// Forme d'une galaxie. Index 0 = galaxie principale, 1.. = galaxies extérieures.
#[derive(Clone, Debug)]
pub struct GalaxyConfig {
    /// Position absolue (immuable) du centre ; `center()` la donne dans le repère monde.
    pub abs_center:    bevy::math::Vec3,
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

impl GalaxyConfig {
    pub fn center(&self) -> Vec3 {
        to_local(self.abs_center)
    }
}

/// Galaxie principale + galaxies extérieures disposées sur une méta-spirale.
pub fn default_galaxies(world_seed: u64) -> Vec<GalaxyConfig> {
    use bevy::math::{EulerRot, Quat, Vec3};
    const META_ARMS: usize = 5;
    const META_RADIUS: f32 = 200_000_000.0 * GALAXY_SCALE;
    const META_TWIST: f32 = 4.0;
    const MIN_DIST: f32 = 40_000_000.0 * GALAXY_SCALE;
    /// Deux galaxies restent séparées d'au moins ce multiple de la somme de leurs rayons.
    const SPACING: f32 = 2.0;
    let tau = std::f32::consts::TAU;

    let world_hash = {
        let x = (world_seed as u32) ^ ((world_seed >> 32) as u32);
        (pseudo_rand(x ^ 0x6A09_E667) * 65_535.0) as u32 * 2 + (x & 1)
    };
    let mut galaxies = Vec::with_capacity(NUM_DISTANT_GALAXIES + NUM_OUTER_GALAXIES + 1);
    galaxies.push(GalaxyConfig {
        abs_center: Vec3::ZERO,
        tilt: Quat::IDENTITY,
        radius: GALAXY_RADIUS,
        num_arms: 5,
        twist: 5.0,
        kind: crate::galaxy_shape::GalaxyKind::Spiral,
        core_radius: 30_000.0 * GALAXY_SIZE_SCALE,
        seed: 0,
        arm_stars: 0,
        scatter_stars: 0,
    });

    for gi in 0..NUM_DISTANT_GALAXIES {
        // La graine du monde décale tout : un autre monde, d'autres galaxies
        let gs = gi as u32 + 300_000 + world_hash % 90_000;
        let rk = |k: u32| pseudo_rand(gs * 13 + k);
        let rk_k = |n: u32, k: u32| pseudo_rand((gs * 13 + n).wrapping_add(k));

        let radius = (1_300_000.0 + (rk(7) * 0.6 + rk(27) * 0.4).powf(1.3) * 6_000_000.0) * GALAXY_SCALE;

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
                (rk_k(5, k) - 0.5) * 16_000_000.0 * GALAXY_SCALE,
                r * theta.sin(),
            );
            // Marge restante par rapport au voisin le plus proche (>= 0 : place libre)
            let slack = galaxies
                .iter()
                .map(|g| c.distance(g.abs_center) - SPACING * (g.radius + radius))
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
            abs_center: center,
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
            core_radius: (10_000.0 + rk(23) * 20_000.0) * GALAXY_SIZE_SCALE,
            seed: gs * 1000,
            arm_stars: star_budget(gs).0,
            scatter_stars: star_budget(gs).1,
        });
    }

    // Galaxies lointaines : au-delà des extérieures, jusqu'à 10 fois la plus lointaine. Une
    // grille de cellules (~0,7 fois la plus lointaine) couvre la coquille ; chaque galaxie est à un
    // point tiré dans sa cellule, loin de ses bords : elles ne se touchent jamais, et le placement
    // se fait en un seul passage (rien de quadratique). Ajoutées après les autres : les galaxies
    // et les systèmes déjà connus ne changent pas.
    let farthest = galaxies.iter().map(|g| g.abs_center.length()).fold(0.0f32, f32::max);
    let (inner, outer) = (farthest * 1.3, farthest * 10.0);
    let cell = farthest * 0.7;
    let n_cells = (outer / cell).ceil() as i32;
    let near_dense = |c: Vec3| galaxies.iter().any(|g| c.distance(g.abs_center) < SPACING * (g.radius + 1.2e10) + cell * 0.2);
    let mut spots: Vec<(u32, Vec3)> = Vec::new();
    for x in -n_cells..n_cells {
        for y in -n_cells..n_cells {
            for z in -n_cells..n_cells {
                let k = (((x + 512) as u32) << 20) ^ (((y + 512) as u32) << 10) ^ ((z + 512) as u32);
                let jitter = |a: u32| 0.2 + 0.6 * pseudo_rand(k.wrapping_mul(2_654_435_761).wrapping_add(a) ^ world_hash);
                let c = Vec3::new(
                    (x as f32 + jitter(1)) * cell,
                    (y as f32 + jitter(2)) * cell,
                    (z as f32 + jitter(3)) * cell,
                );
                let d = c.length();
                if d < inner || d > outer || near_dense(c) {
                    continue;
                }
                spots.push((k, c));
            }
        }
    }
    // Les cellules retenues : tirées au sort (toujours les mêmes pour la même graine), puis
    // rangées de la plus proche à la plus lointaine (les numéros croissent vers l'extérieur)
    spots.sort_by_key(|(k, _)| (pseudo_rand(k ^ world_hash ^ 0x0BAD_5EED) * 4_000_000_000.0) as u64);
    spots.truncate(NUM_OUTER_GALAXIES);
    spots.sort_by(|a, b| a.1.length().total_cmp(&b.1.length()));
    for (gi, (_, center)) in spots.into_iter().enumerate() {
        let gs = gi as u32 + 700_000 + world_hash % 90_000;
        let rk = |k: u32| pseudo_rand(gs * 13 + k);
        let radius = (1_300_000.0 + (rk(7) * 0.6 + rk(27) * 0.4).powf(1.3) * 6_000_000.0) * GALAXY_SCALE;
        galaxies.push(GalaxyConfig {
            abs_center: center,
            tilt: Quat::from_euler(EulerRot::XYZ, (rk(13) - 0.5) * 1.5, rk(15) * tau, (rk(17) - 0.5) * 1.0),
            radius,
            num_arms: 2 + (rk(9) * 5.0) as usize,
            twist: 1.5 + rk(11) * 7.5,
            kind: crate::galaxy_shape::GalaxyKind::for_index(NUM_DISTANT_GALAXIES + gi, world_hash),
            core_radius: (10_000.0 + rk(23) * 20_000.0) * GALAXY_SIZE_SCALE,
            seed: gs * 1000,
            arm_stars: star_budget(gs).0,
            scatter_stars: star_budget(gs).1,
        });
    }
    galaxies
}

/// Systèmes de l'univers : notre galaxie et les extérieures tout de suite, les lointaines à la
/// demande (`systems::Systems`).
pub(crate) fn default_systems(galaxies: &[GalaxyConfig], world_seed: u64) -> crate::systems::Systems {
    crate::systems::Systems::new(galaxies, world_seed, 1 + NUM_DISTANT_GALAXIES)
}

const SYSTEM_PREFIXES: [&str; 8] = ["HD", "GJ", "HR", "TYC", "HIP", "NGC", "IC", "SAO"];
const STAR_NAMES: [&str; 45] = [
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

/// Fabrique les systèmes d'une galaxie d'après la graine du monde (au départ ou à la demande).
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemMaker {
    world_seed: u64,
}

impl SystemMaker {
    pub fn new(world_seed: u64) -> Self {
        Self { world_seed }
    }

    // Tout ce qui vit dans un système (étoile, planètes, lunes) dépend de la graine du monde
    // (les positions suivent la forme des galaxies). Rien d'autre que + - * / ici : le résultat
    // doit être identique sur toutes les machines (empreinte du monde en multijoueur).
    fn mixed(&self, x: u32) -> u32 {
        let sd = ((self.world_seed as u32) ^ ((self.world_seed >> 32) as u32)).wrapping_mul(0x9E37_79B1);
        x.wrapping_add(sd)
    }

    // Échelle G du système : 600 000 à 1 500 000, au moins 100 fois ses planètes (4 200 à 14 000).
    // L'étoile a un type tiré au sort (voir `planetgen::star`) : une G garde ce rayon, les autres
    // ont les proportions réelles. Aucun type imposé : le système de départ est tiré comme les autres.
    fn make_star(&self, seed: u32) -> (StarConfig, StarPhysics) {
        let seed = self.mixed(seed);
        let r_f = pseudo_rand(seed.wrapping_mul(5).wrapping_add(31));
        let g_radius = 600_000.0 + r_f * 900_000.0;
        let physics = StarPhysics::generate(seed as u64, r_f as f64, None);
        (StarConfig::from_physics(&physics, g_radius), physics)
    }

    // Planètes et lunes : seulement leur génome, recalculées à la demande (`planetgen::genome`).
    // `sb` : base des graines de planètes (doit rester loin de u32::MAX).
    fn genome(&self, seed: u32, sb: u32) -> SystemGenome {
        SystemGenome { seed: self.mixed(seed), planet_base: self.mixed(sb) }
    }

    /// Notre galaxie (en tête de `galaxies`) et les galaxies extérieures, comme avant la 0.11 :
    /// numéros contigus. Renvoie aussi la plage de numéros de chaque galaxie.
    pub fn dense(&self, galaxies: &[GalaxyConfig]) -> (Vec<StarSystemConfig>, Vec<std::ops::Range<usize>>) {
        const NUM_ARMS: usize = 5;
        const ARM_TWIST: f32 = 5.0;
        let world_seed = self.world_seed;
        // Galaxie principale : 5 000 à 10 000 étoiles d'après la graine du monde
        let (arm_stars, scatter_stars) = star_budget(((world_seed as u32) ^ ((world_seed >> 32) as u32)).wrapping_mul(0x9E37_79B1) ^ 0x4D41_494E);
        let tau = std::f32::consts::TAU;
        let gr = GALAXY_RADIUS;
        let gen_name = |idx: usize| -> String {
            if idx < STAR_NAMES.len() {
                STAR_NAMES[idx].to_string()
            } else {
                let pi = idx % SYSTEM_PREFIXES.len();
                format!("{}-{}", SYSTEM_PREFIXES[pi], idx * 7 + 1031)
            }
        };
        let distant_count: usize = galaxies.iter().skip(1).map(|g| g.arm_stars + g.scatter_stars).sum();
        let mut systems = Vec::with_capacity(arm_stars + scatter_stars + distant_count);
        let mut ranges = Vec::with_capacity(galaxies.len());
        if galaxies.is_empty() {
            return (systems, ranges);
        }

        // ── Bras spiraux ──────────────────────────────────────────────────
        for i in 0..arm_stars {
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
            let thickness = 60000.0 * GALAXY_SCALE * (1.0 - r / gr * 0.7);
            let y = (pseudo_rand(s * 7 + 7) - 0.5) * thickness;

            systems.push(StarSystemConfig::generated(gen_name(i), [x, y, z], 0, self.make_star(s), self.genome(s, (s + 1) * 100)));
        }

        // ── Étoiles dispersées entre les bras ─────────────────────────────
        for i in 0..scatter_stars {
            let s = (arm_stars + i) as u32 + 100;
            let t = pseudo_rand(s * 7 + 3);
            let r = t * t * gr * 0.85;
            if r < CORE_EXCLUSION { continue; }
            let theta = pseudo_rand(s * 7 + 5) * tau;
            let x = r * theta.cos();
            let z = r * theta.sin();
            let y = (pseudo_rand(s * 7 + 7) - 0.5) * 3000.0 * GALAXY_SCALE;

            systems.push(StarSystemConfig::generated(
                gen_name(arm_stars + i), [x, y, z], 0, self.make_star(s), self.genome(s, (s + 1) * 100),
            ));
        }
        ranges.push(0..systems.len());

        // ── Galaxies extérieures : mêmes systèmes (étoile + planètes) ─────
        for (gid, gal) in galaxies.iter().enumerate().skip(1) {
            let start = systems.len();
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
                let world = gal.abs_center + gal.tilt * local;

                let global_idx = systems.len() as u32;
                let pi = local_idx % SYSTEM_PREFIXES.len();
                systems.push(StarSystemConfig::generated(
                    format!("G{}-{}-{}", gid, SYSTEM_PREFIXES[pi], local_idx * 7 + 1031),
                    [world.x, world.y, world.z],
                    gid as u32,
                    self.make_star(s),
                    self.genome(s, (global_idx + 1) * 100),
                ));
                local_idx += 1;
            }
            ranges.push(start..systems.len());
        }
        (systems, ranges)
    }

    /// Systèmes d'une galaxie lointaine, générée à la demande : le système `i` de son budget
    /// d'étoiles porte le numéro `base + i` (vide s'il tombe dans le trou noir central).
    pub fn lazy(&self, gid: u32, gal: &GalaxyConfig, base: usize) -> Vec<Option<StarSystemConfig>> {
        let shape = gal.shape();
        let total = gal.arm_stars + gal.scatter_stars;
        let on_structure = (total as f32 * shape.structure_share) as usize;
        (0..total)
            .map(|i| {
                let s = i as u32 + gal.seed;
                let mut rng = crate::galaxy_shape::Rng::new(s);
                let local = if i < on_structure { shape.sample_structure(&mut rng, 0.0) } else { shape.sample_background(&mut rng) };
                if local.length() < CORE_EXCLUSION {
                    return None;
                }
                let world = gal.abs_center + gal.tilt * local;
                // Base des graines de planètes : tirée du numéro (loin de u32::MAX)
                let idx = (base + i) as u64;
                let sb = ((idx.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 33) % 4_000_000_000) as u32 + 100;
                let pi = i % SYSTEM_PREFIXES.len();
                Some(StarSystemConfig::generated(
                    format!("G{}-{}-{}", gid, SYSTEM_PREFIXES[pi], i * 7 + 1031),
                    [world.x, world.y, world.z],
                    gid,
                    self.make_star(s),
                    self.genome(s, sb),
                ))
            })
            .collect()
    }
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
fn default_terrain_detail() -> u8 { 3 }
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
    /// Zones chaude, habitable et froide du système chargé (`zones.rs`).
    #[serde(default)] pub show_zones:           bool,
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
    /// Distance de détail du sol (0 Bas .. 3 Ultra, `graphics::TERRAIN_DETAIL`, 0.13 T4).
    #[serde(default = "default_terrain_detail")] pub terrain_detail: u8,
    /// Le relief projette des ombres jusqu'à 4 000 voxels (montagnes lointaines ; ~30 % d'images/s).
    #[serde(default)] pub relief_shadows: bool,

    #[serde(default)] pub world_seed: u64,

    // ── Multijoueur ──────────────────────────────────────────────────────
    #[serde(default = "default_player_name")] pub player_name: String,
    #[serde(default = "default_aura_color")]  pub aura_color:  [f32; 3],
    /// Modèles de l'éditeur utilisés en jeu (E7) : fichiers `.ssvox` (relatifs au dossier des
    /// sauvegardes) ; aucun = le modèle par défaut.
    #[serde(default)]                         pub ship_model: Option<String>,
    #[serde(default)]                         pub character_model: Option<String>,
    /// Distance de la caméra derrière le personnage (voxels, 3 à 12 ; molette en 3e personne).
    #[serde(default = "default_walker_cam")]  pub walker_cam: f32,
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
    /// Modifications des astres depuis leur génération (règle 7, minage en 0.14) : sauvées dans
    /// `world.json`, vides pour l'instant.
    #[serde(skip)]                            pub body_deltas: WorldDeltas,
    /// Horloge du monde au chargement (secondes de jeu, `world.json`) : ensuite `WorldClock`.
    #[serde(skip)]                            pub world_clock: f64,
    /// Cellules voxel modifiées, par astre (minage en 0.14 ; vide pour l'instant) : `world.json`.
    #[serde(skip)]                            pub voxel_deltas: crate::voxel::VoxelDeltas,
    /// Ce jeu a pris une identité de secours (un autre jeu utilisait la même sauvegarde) :
    /// il ne réécrit plus `settings.json`, pour ne pas écraser le compte de l'autre.
    #[serde(skip)]                            pub temp_identity: bool,
    /// Premier lancement (aucune sauvegarde) : création du personnage dans l'éditeur (0.12).
    #[serde(skip)]                            pub first_launch: bool,

    // ── Systèmes stellaires (régénérés au lancement, jamais sauvegardés) ─
    #[serde(skip)] pub systems: crate::systems::Systems,
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
            show_light_indicator: false, show_orbits: false, show_zones: false, show_systems: false,
            planet_chunk_divisions: 6,
            vsync: true, fps_limit: 0, msaa_samples: 4, shadows: true,
            lod_quality: 1.0, show_clouds: true, show_flares: true, render_scale: 1.0, terrain_detail: 3, relief_shadows: false,
            world_seed: DEFAULT_WORLD_SEED,
            player_name: default_player_name(),
            aura_color: default_aura_color(),
            ship_model: None,
            character_model: None,
            walker_cam: default_walker_cam(),
            last_join_address: String::new(),
            clan_tag: String::new(),
            claims: Vec::new(),
            allies: Vec::new(),
            enemies: Vec::new(),
            player_id: 0,
            known_wormholes: Vec::new(),
            guild: None,
            guild_archive: Vec::new(),
            body_deltas: WorldDeltas::new(),
            world_clock: 0.0,
            voxel_deltas: Default::default(),
            temp_identity: false,
            first_launch: false,
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
            let mut settings = Self::default();
            settings.save();
            settings.first_launch = true;
            settings
        };
        s.apply_world_save();
        s.galaxies = default_galaxies(s.world_seed);
        s.systems = default_systems(&s.galaxies, s.world_seed);
        // Le monde démarre centré sur le système de départ : tout ce qui apparaît est précis
        set_origin(s.systems.first().map_or(DVec3::ZERO, |sys| sys.abs_center().as_dvec3()));
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

    /// Réécrit seulement `world.json` (l'heure du monde y est enregistrée régulièrement).
    pub fn save_world(&self) {
        if self.temp_identity || cfg!(test) {
            return;
        }
        let world = WorldSave::from(self);
        if let Ok(json) = serde_json::to_string_pretty(&world) {
            fs::write(data_dir().join("world.json"), json).ok();
        }
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
        self.save_world();
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
        self.body_deltas = world.body_deltas;
        self.world_clock = world.clock;
        // Cellules repérées à une autre échelle du sol (avant la 0.13) : elles ne tombent plus sur les
        // mêmes voxels, on les oublie (cratères d'impact ; le minage arrive en 0.14)
        self.voxel_deltas = if world.ground_scale == crate::terrain::GROUND_SCALE { world.voxel_deltas } else { Default::default() };
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
    /// Deltas des astres (minage, destruction) : départ + delta, voir `planetgen::live`.
    #[serde(default, skip_serializing_if = "WorldDeltas::is_empty")] body_deltas: WorldDeltas,
    /// Horloge du monde (secondes de jeu depuis sa création, règle 9).
    #[serde(default)] clock: f64,
    /// Cellules voxel modifiées (0.11 B1 : format prêt, minage en 0.14).
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")] voxel_deltas: crate::voxel::VoxelDeltas,
    /// Échelle du sol avec laquelle les cellules ont été repérées (0.13 E2 : absente = 1).
    #[serde(default = "old_ground_scale")] ground_scale: u32,
}

fn old_ground_scale() -> u32 {
    1
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
            body_deltas: s.body_deltas.clone(),
            clock: crate::world_clock::saved_secs().max(s.world_clock),
            voxel_deltas: s.voxel_deltas.clone(),
            ground_scale: crate::terrain::GROUND_SCALE,
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
    /// Galaxies lointaines ajoutées (générées après le départ).
    far: std::collections::HashSet<u32>,
}

impl SystemSpatialIndex {
    /// Cellule (colonne, ligne) d'une position ABSOLUE.
    fn cell_of(abs: DVec3) -> (i32, i32) {
        let cell = SYSTEM_CELL_SIZE as f64;
        let half = SYSTEM_GRID_SIZE as f64 * cell / 2.0;
        (((abs.x + half) / cell).floor() as i32, ((abs.z + half) / cell).floor() as i32)
    }

    /// L'index est construit en coordonnées absolues ; les requêtes prennent des positions monde
    /// (relatives à l'origine flottante) et les convertissent.
    pub fn build(settings: &GameSettings) -> Self {
        let mut cells: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
        for (si, sys) in settings.systems.iter() {
            cells.entry(Self::cell_of(sys.abs_center().as_dvec3())).or_default().push(si);
        }
        Self { cells, far: std::collections::HashSet::new() }
    }

    /// Ajoute les systèmes d'une galaxie lointaine qui vient d'être générée.
    pub fn add_galaxy(&mut self, gid: u32, systems: &[(usize, &StarSystemConfig)]) {
        if !self.far.insert(gid) {
            return;
        }
        for &(si, sys) in systems {
            self.cells.entry(Self::cell_of(sys.abs_center().as_dvec3())).or_default().push(si);
        }
    }

    /// Galaxies lointaines déjà dans l'index.
    pub fn has_galaxy(&self, gid: u32) -> bool {
        self.far.contains(&gid)
    }

    /// Système le plus proche de `pos`, en explorant la grille par anneaux croissants.
    pub fn nearest(&self, pos: Vec3, settings: &GameSettings) -> Option<usize> {
        let cell = SYSTEM_CELL_SIZE as f64;
        let abs = to_abs(pos);
        let (cx, cz) = Self::cell_of(abs);
        let mut best: Option<(usize, f64)> = None;
        for r in 0..=(SYSTEM_GRID_SIZE as i32 * 2) {
            // Tout système hors de l'anneau r est à plus de (r - 1) cellules
            if let Some((_, d2)) = best {
                let min_d = (r - 1).max(0) as f64 * cell;
                if min_d * min_d > d2 { break; }
            }
            for col in (cx - r)..=(cx + r) {
                for row in (cz - r)..=(cz + r) {
                    if (col - cx).abs() != r && (row - cz).abs() != r { continue; }
                    let Some(indices) = self.cells.get(&(col, row)) else { continue };
                    for &si in indices {
                        let Some(sys) = settings.systems.get(si) else { continue };
                        let d2 = abs.distance_squared(sys.abs_center().as_dvec3());
                        if best.map_or(true, |(_, b)| d2 < b) { best = Some((si, d2)); }
                    }
                }
            }
        }
        best.map(|(si, _)| si)
    }

    pub fn systems_in_radius(&self, pos: Vec3, radius: f32) -> Vec<usize> {
        let r_cells = (radius / SYSTEM_CELL_SIZE).ceil() as i32 + 1;
        let (cx, cz) = Self::cell_of(to_abs(pos));
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

    /// 10 000 galaxies : les lointaines après les autres (rien de déjà connu ne change), de 1,3 à
    /// 10 fois la plus lointaine des extérieures, jamais en contact ; leurs systèmes ont des
    /// numéros fixes et ne sont générés qu'à la demande.
    #[test]
    fn ten_thousand_galaxies_generated_on_demand() {
        let t = std::time::Instant::now();
        let galaxies = default_galaxies(DEFAULT_WORLD_SEED);
        assert_eq!(galaxies.len(), 10_000);
        let farthest = galaxies[..=NUM_DISTANT_GALAXIES].iter().map(|g| g.abs_center.length()).fold(0.0f32, f32::max);
        for g in &galaxies[NUM_DISTANT_GALAXIES + 1..] {
            let d = g.abs_center.length();
            assert!(d >= farthest * 1.29 && d <= farthest * 10.01, "{d} pour {farthest}");
        }
        // Jamais en contact (grille : on vérifie chaque galaxie contre ses voisines proches)
        let mut sorted: Vec<&GalaxyConfig> = galaxies.iter().collect();
        sorted.sort_by(|a, b| a.abs_center.x.total_cmp(&b.abs_center.x));
        for (i, a) in sorted.iter().enumerate() {
            for b in sorted[i + 1..].iter().take_while(|b| b.abs_center.x - a.abs_center.x < 3.0e10) {
                assert!(a.abs_center.distance(b.abs_center) > a.radius + b.radius, "galaxies qui se touchent");
            }
        }
        // Mêmes galaxies pour la même graine
        let again = default_galaxies(DEFAULT_WORLD_SEED);
        assert!(galaxies.iter().zip(&again).all(|(a, b)| a.abs_center == b.abs_center));
        // Les systèmes : notre galaxie et les extérieures tout de suite, les autres à la demande
        let systems = default_systems(&galaxies, DEFAULT_WORLD_SEED);
        assert!(t.elapsed().as_secs_f32() < 5.0, "{:?}", t.elapsed());
        assert!(systems.len() > 50_000_000, "{}", systems.len());
        assert_eq!(systems.loaded_far_galaxies().len(), 0);
        let far = 5_000u32;
        let range = systems.galaxy_range(far);
        assert!(range.start > systems.dense().len() && range.len() > 4_000);
        // Un accès génère la galaxie ; ses numéros ne dépendent pas de l'ordre des visites
        let some = (range.start..range.end).find_map(|i| systems.get(i).map(|s| (i, s.name.clone()))).unwrap();
        assert!(systems.is_loaded(far));
        assert_eq!(systems.loaded_far_galaxies(), vec![far]);
        let other = default_systems(&galaxies, DEFAULT_WORLD_SEED);
        other.load(9_000);
        assert_eq!(other.get(some.0).map(|s| s.name.clone()), Some(some.1));
        assert!(other.get(some.0).is_some_and(|s| s.galaxy_id == far));
        // Le parcours ne donne que les systèmes générés, avec leur numéro
        assert!(systems.iter().all(|(i, s)| systems.peek(i).is_some_and(|p| p.name == s.name)));
        assert!(systems.iter().count() < systems.dense().len() + 12_000);
    }

    #[test]
    fn galaxies_do_not_touch_each_other() {
        let g = default_galaxies(DEFAULT_WORLD_SEED);
        let mut worst = f32::MAX;
        for i in 0..g.len() {
            for j in (i + 1)..g.len() {
                let gap = g[i].abs_center.distance(g[j].abs_center) / (g[i].radius + g[j].radius);
                worst = worst.min(gap);
            }
        }
        // Jamais collées : au moins 1,5 fois la somme des rayons entre deux centres
        assert!(worst >= 1.5, "deux galaxies trop proches : {worst}");
    }

    #[test]
    fn planets_follow_their_star_and_never_touch() {
        use crate::planetgen::system::PlanetKind;
        let galaxies = default_galaxies(DEFAULT_WORLD_SEED);
        let systems = default_systems(&galaxies, DEFAULT_WORLD_SEED);
        let (mut hot, mut temperate, mut cold) = (0, 0, 0);
        let mut kinds = std::collections::HashMap::new();
        for sys in systems.dense().iter().take(3000) {
            let star = &sys.stars[0];
            // Échelle G du système (rayon d'une G) : la taille réelle de l'étoile dépend de son type
            let scale = star.scale();
            assert!((600_000.0..=1_500_000.0).contains(&scale), "echelle {scale}");
            if star.class == crate::planetgen::star::StarClass::G {
                assert!((star.radius - scale).abs() <= 5.0, "une G garde l'echelle d'avant");
            }
            let planets = sys.planets();
            // 1 à 8 planètes en orbite, plus parfois une planète errante (C2) au bout de la liste
            let orbiting = planets.iter().filter(|p| !p.rogue).count();
            assert!((1..=8).contains(&orbiting), "{orbiting} planetes");
            assert!(planets.len() <= orbiting + 1 && planets.iter().take(orbiting).all(|p| !p.rogue));
            let earth = scale / 109.0;
            let mut previous_edge = star.radius;
            let mut previous_au = 0.0;
            for p in planets {
                *kinds.entry(p.kind).or_insert(0usize) += 1;
                // Proportions réelles : 1 R⊕ = échelle / 109
                assert!((p.radius / earth / p.radius_earth - 1.0).abs() < 0.01, "{} {}", p.radius, p.radius_earth);
                assert!(p.radius_earth > 0.2 && p.radius_earth < 16.0, "{}", p.radius_earth);
                assert!(p.semi_major_au > previous_au, "orbites dans l'ordre");
                previous_au = p.semi_major_au;
                // Ni dans l'étoile ni dans la précédente (lunes comprises), même au périastre
                let reach = p.moons.iter().map(|m| m.orbit_distance + m.radius).fold(p.radius, f32::max);
                let periapsis = p.orbit_distance * (1.0 - p.eccentricity);
                assert!(periapsis - reach > previous_edge, "planete dans l'etoile ou dans la precedente");
                previous_edge = p.orbit_distance * (1.0 + p.eccentricity) + reach;
                assert!(p.gravity_g > 0.0 && p.mass_earth > 0.0);
                if p.gaseous() {
                    assert!(!p.atmosphere && p.terrain_height == 0.0);
                } else {
                    assert!(p.terrain_height > 0.0 && p.terrain_height < p.radius * 0.06);
                }
                assert_eq!(p.star_radius, scale);
                let mut moon_edge = p.radius + p.terrain_height;
                for m in &p.moons {
                    assert!(m.radius * 3.0 <= p.radius * 1.01, "lune {} pour une planete de {}", m.radius, p.radius);
                    assert!(m.orbit_distance - m.radius > moon_edge, "lune dans la planete ou dans la lune precedente");
                    moon_edge = m.orbit_distance + m.radius;
                    assert!(m.gravity_g > 0.0 && m.gravity_g < 0.6, "{}", m.gravity_g);
                }
                if p.kind == PlanetKind::Rocky {
                    match p.temperature() {
                        t if t > 100.0 => hot += 1,
                        t if t > -20.0 => temperate += 1,
                        _ => cold += 1,
                    }
                }
            }
        }
        // Des mondes brulants, temperes et glaces : de quoi varier les paysages
        assert!(hot > 100 && temperate > 100 && cold > 100, "chaud {hot}, tempere {temperate}, froid {cold}");
        for kind in [PlanetKind::Rocky, PlanetKind::MiniNeptune, PlanetKind::IceGiant, PlanetKind::GasGiant] {
            assert!(kinds.get(&kind).copied().unwrap_or(0) > 300, "{kinds:?}");
        }
    }

    #[test]
    fn the_world_seed_changes_stars_and_planets() {
        let galaxies = default_galaxies(DEFAULT_WORLD_SEED);
        let a = default_systems(&galaxies, 42);
        let b = default_systems(&galaxies, 43);
        let different = a.dense().iter().zip(b.dense()).take(300).filter(|(x, y)| {
            x.stars[0].radius.to_bits() != y.stars[0].radius.to_bits()
                || x.planets().len() != y.planets().len()
                || x.planets()[0].radius.to_bits() != y.planets()[0].radius.to_bits()
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
        for (x, y) in a.dense().iter().zip(b.dense()).take(500) {
            assert_eq!(x.planets().len(), y.planets().len());
            for (p, q) in x.planets().iter().zip(y.planets()) {
                assert_eq!(p.radius.to_bits(), q.radius.to_bits());
                assert_eq!(p.orbit_distance.to_bits(), q.orbit_distance.to_bits());
                assert_eq!(p.moons.len(), q.moons.len());
            }
        }
    }
}



/// Caméra à pied : 6 voxels derrière le personnage (C4, Q1).
fn default_walker_cam() -> f32 {
    6.0
}
