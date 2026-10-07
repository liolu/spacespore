//! Météo (C5 de `roadmaps/fait/ROADMAP-0.11.md`).
//!
//! Tout est une fonction de (graine de l'astre, horloge du monde, lieu) (règle 9) : deux machines
//! à la même heure voient la même météo, rien ne passe par le réseau.
//! - **Vents** zonaux selon la latitude (alizés, vents d'ouest), d'après la vitesse moyenne des
//!   vents de l'atmosphère (phase 3) ; ils emportent les nuages (bruit advecté) qui se forment et se
//!   défont lentement (deux champs fondus l'un dans l'autre).
//! - **Précipitations** sous les nuages épais : pluie, neige, grêle dans les orages ; exotiques
//!   selon l'atmosphère (méthane comme Titan, acide comme Vénus, verre et fer sur les mondes
//!   brûlants). **Orages** et éclairs, **tempêtes de poussière** sur les mondes secs et venteux,
//!   **brouillard** du matin.
//! - Rendu : la couche de nuages de chaque planète est reconstruite en arrière-plan (cubes, comme
//!   le reste) ; au sol, particules autour de la caméra, ciel gris, lumière voilée, brouillard,
//!   éclairs ; en vol bas, le vent pousse le vaisseau.

use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::view::NoFrustumCulling;
use bevy::tasks::{block_on, futures_lite::future, AsyncComputeTaskPool, Task};
use std::collections::HashMap;

use crate::asteroids::{fbm, vnoise};
use crate::planet::{CloudVoxel, PlanetId, PlanetRoot};
use crate::planetgen::atmosphere::CloudKind;
use crate::planetgen::seeds::splitmix64;
use crate::settings::{GameSettings, PlanetConfig};
use crate::surface::{Surface, SurfaceControl};
use crate::ui::TargetKind;
use crate::world_clock::{LocalWeather, WorldClock};

pub struct WeatherPlugin;

impl Plugin for WeatherPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WeatherNow>()
            .init_resource::<CloudTasks>()
            .add_systems(Update, (update_weather_now, push_ship).chain().before(SurfaceControl))
            .add_systems(Update, (rebuild_clouds, weather_fx).after(SurfaceControl));
    }
}

fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

// ─────────────────────────────────────────────────────────────────────────
//  Le modèle
// ─────────────────────────────────────────────────────────────────────────

/// Ce qui tombe du ciel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Precip {
    #[default]
    None,
    Rain,
    Snow,
    Hail,
    /// Pluie de méthane (Titan).
    Methane,
    /// Pluie d'acide sulfurique (Vénus).
    Acid,
    /// Neige carbonique (Mars).
    DryIce,
    /// Pluie de verre et de fer (mondes brûlants).
    Glass,
    Iron,
}

impl Precip {
    pub fn name(self) -> &'static str {
        match self {
            Precip::None => "",
            Precip::Rain => "pluie",
            Precip::Snow => "neige",
            Precip::Hail => "grele",
            Precip::Methane => "pluie de methane",
            Precip::Acid => "pluie d'acide",
            Precip::DryIce => "neige carbonique",
            Precip::Glass => "pluie de verre",
            Precip::Iron => "pluie de fer",
        }
    }

    /// Couleur (sRGB), vitesse de chute (voxels/s) et forme (traînée ou flocon).
    fn look(self) -> ([f32; 3], f32, bool) {
        match self {
            Precip::Rain => ([0.7, 0.78, 0.9], 28.0, true),
            Precip::Snow => ([0.95, 0.96, 1.0], 2.5, false),
            Precip::Hail => ([0.88, 0.92, 0.96], 40.0, false),
            Precip::Methane => ([0.85, 0.6, 0.3], 9.0, true),
            Precip::Acid => ([0.9, 0.85, 0.35], 24.0, true),
            Precip::DryIce => ([0.98, 0.95, 0.92], 1.8, false),
            Precip::Glass => ([0.75, 0.9, 1.0], 45.0, true),
            Precip::Iron => ([1.0, 0.45, 0.15], 50.0, true),
            Precip::None => ([1.0; 3], 0.0, false),
        }
    }
}

/// Ce que l'atmosphère d'un astre permet (tiré de son profil, phase 3).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeatherParams {
    pub seed: u32,
    /// Couverture nuageuse moyenne (0..1), vent moyen (m/s), pression (bar).
    pub cover: f32,
    pub wind: f32,
    pub pressure: f32,
    pub clouds: CloudKind,
    /// Mers d'eau liquide (part de la surface) : humidité, brouillard, orages.
    pub ocean: f32,
    pub water: bool,
    /// Rayon réel (m) : les vents en m/s deviennent des vitesses angulaires.
    pub radius_m: f32,
    /// Température moyenne (°C).
    pub mean_c: f32,
}

impl WeatherParams {
    /// Seulement les astres solides avec de l'air.
    pub fn of(p: &PlanetConfig) -> Option<Self> {
        if p.gaseous() || !p.air.present() || p.air.pressure_bar < 0.01 {
            return None;
        }
        let water = p.hydrology.hydro.liquid == crate::planetgen::hydrology::Liquid::Water;
        Some(Self {
            seed: p.seed ^ 0x5745_4154,
            cover: p.air.cloud_cover.clamp(0.0, 1.0),
            wind: p.air.wind_ms.max(1.0),
            pressure: p.air.pressure_bar,
            clouds: p.air.clouds,
            ocean: p.hydrology.ocean_fraction.clamp(0.0, 1.0),
            water,
            radius_m: (p.radius_earth.max(0.05) * 6.371e6) as f32,
            mean_c: p.climate.map_or(15.0, |c| c.mean_c),
        })
    }
}

/// La météo en un lieu à un instant.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sample {
    /// Nuages (0..1) et leur épaisseur, précipitations (0..1) et leur nature.
    pub cloud: f32,
    pub precip: f32,
    pub kind: Precip,
    /// Orage (0..1), poussière (0..1), brouillard (0..1).
    pub storm: f32,
    pub dust: f32,
    pub fog: f32,
    /// Vent (m/s) : vers l'est (+) / l'ouest (−) et vers le nord.
    pub east: f32,
    pub north: f32,
    /// Rafale en cours (0 : vent établi, 1 : forte rafale).
    pub gust: f32,
}

impl Sample {
    pub fn wind_speed(&self) -> f32 {
        (self.east * self.east + self.north * self.north).sqrt()
    }
}

/// Le temps de jeu va 60 fois plus vite que le temps réel des planètes (A1).
const GAME_SPEEDUP: f64 = 60.0;
/// Les nuages se forment et se défont en ~15 min de jeu.
const MORPH_SECS: f64 = 900.0;
/// Les nuages vus de l'espace avancent plus vite que le vent réel (sinon on ne les voit pas bouger).
const CLOUD_DRIFT: f64 = 6.0;

/// Vent zonal (fraction du vent moyen, + = vers l'est) : alizés vers l'ouest sous 30°, vents
/// d'ouest aux latitudes moyennes, vents polaires d'est.
pub fn zonal(lat: f32) -> f32 {
    let a = lat.abs();
    if a < 1.05 { -(3.0 * lat).cos() } else { -0.4 }
}

/// Champ de nuages (0..1) au point `dir` (repère fixe de l'astre) à l'instant `t`.
pub fn cloud_field(w: &WeatherParams, dir: Vec3, t: f64) -> (f32, f32) {
    let lat = dir.y.clamp(-1.0, 1.0).asin();
    // Advection : chaque latitude tourne à la vitesse de son vent
    let omega = zonal(lat) as f64 * w.wind as f64 / w.radius_m as f64 * GAME_SPEEDUP * CLOUD_DRIFT;
    // Formation et dissipation : deux champs fondus. Chaque champ ne vit que deux périodes : il
    // n'est poussé par le vent que depuis sa naissance (sinon, depuis le début de la partie, les
    // latitudes voisines finissent décalées de dizaines de tours : des bandes comme sur Jupiter)
    let tau = t / MORPH_SECS;
    let k = tau.floor();
    let f = (tau - k) as f32;
    // Taille des nuages propre à chaque monde
    let scale = 2.6 + 1.8 * (splitmix64(w.seed as u64 ^ 0x5CA1E) % 1000) as f32 / 1000.0;
    let field = |k: f64| {
        let h = splitmix64(w.seed as u64 ^ (k as i64 as u64).wrapping_mul(0xA24B_AED4_963E_E407));
        let o = Vec3::new((h & 0xFFFF) as f32 / 650.0, ((h >> 16) & 0xFFFF) as f32 / 650.0, ((h >> 32) & 0xFFFF) as f32 / 650.0);
        let age = t - (k - 1.0) * MORPH_SECS;
        let p = Quat::from_rotation_y((omega * age) as f32) * dir;
        fbm(p * scale + o, w.seed, 4)
    };
    let raw = field(k) * (1.0 - smooth(0.0, 1.0, f)) + field(k + 1.0) * smooth(0.0, 1.0, f);
    // Plus de nuages à l'équateur et vers 60°, moins vers 30° (déserts)
    let band = 0.05 * (6.0 * lat).cos();
    let thr = 0.62 - 0.26 * w.cover - band;
    let cloud = smooth(thr - 0.05, thr + 0.08, raw);
    // Épaisseur (précipitations) : le cœur des nuages denses
    let thick = smooth(thr + 0.07, thr + 0.2, raw);
    (cloud, thick)
}

/// Vent au point `dir` à l'instant `t` (m/s vers l'est, vers le nord, rafale 0..1) : le vent zonal
/// du lieu, dont la direction tourne lentement (heures), qui forcit et faiblit (dizaines de minutes),
/// avec des rafales de quelques secondes et de la turbulence dans les orages. f(graine, temps, lieu).
pub fn wind(w: &WeatherParams, dir: Vec3, t: f64, storm: f32) -> (f32, f32, f32) {
    let lat = dir.y.clamp(-1.0, 1.0).asin();
    let at = |scale: f32, period: f64, salt: u32| vnoise(dir * scale + Vec3::new((t / period) as f32, 0.37, -(t / period) as f32 * 0.61), w.seed ^ salt);
    // Vent établi : zonal, dont la direction tourne avec les dépressions qui passent
    let base_e = zonal(lat) * w.wind;
    let base_n = 0.25 * w.wind * (6.0 * lat).sin();
    let turn = (at(3.0, 5_400.0, 0x61) - 0.5) * 2.4;
    let (s, c) = turn.sin_cos();
    let (e, n) = (base_e * c - base_n * s, base_e * s + base_n * c);
    // Un fond qui tourne lui aussi : jamais toujours le même sens
    let calm = 0.3 * w.wind;
    let (e, n) = (e + calm * (turn * 1.7).cos(), n + calm * (turn * 1.7).sin());
    // Force : forcit et faiblit lentement
    let strength = 0.6 + 0.8 * at(6.0, 600.0, 0x60);
    // Rafales : pics courts (quelques secondes), plus fréquents dans les orages
    let g = at(40.0, 3.5, 0x62);
    let gust = ((g - (0.62 - 0.2 * storm)) / 0.38).clamp(0.0, 1.0).powi(2);
    // Turbulence des orages : la direction s'agite vite
    let jitter = (at(50.0, 0.8, 0x63) - 0.5) * 1.6 * storm;
    let (s, c) = jitter.sin_cos();
    let k = strength * (1.0 + 0.9 * gust + 0.6 * storm);
    ((e * c - n * s) * k, (e * s + n * c) * k, gust)
}

/// La météo complète au point `dir` (repère fixe de l'astre), à l'instant `t`, pour une
/// température locale `temp_c` (°C) et une heure locale `hour`.
pub fn sample(w: &WeatherParams, dir: Vec3, t: f64, temp_c: f32, hour: f32) -> Sample {
    let (cloud, thick) = cloud_field(w, dir, t);
    let humid = if w.water { 0.5 + 0.5 * w.ocean } else { 0.35 };
    let precip = thick * humid;
    // Orages : précipitations fortes dans un air chaud et humide
    let storm = precip * smooth(8.0, 26.0, temp_c) * if w.water { 1.0 } else { 0.4 };
    let (east, north, gust) = wind(w, dir, t, storm);
    let speed = (east * east + north * north).sqrt();
    let kind = if precip < 0.02 {
        Precip::None
    } else if temp_c > 1600.0 {
        Precip::Iron
    } else if temp_c > 900.0 {
        Precip::Glass
    } else {
        match w.clouds {
            CloudKind::Sulfuric => Precip::Acid,
            CloudKind::Methane if temp_c < -150.0 => Precip::Methane,
            CloudKind::CarbonDioxide if temp_c < -75.0 => Precip::DryIce,
            CloudKind::Water | CloudKind::Exotic | CloudKind::None if w.water || w.clouds == CloudKind::Water => {
                if storm > 0.55 && temp_c > 5.0 {
                    Precip::Hail
                } else if temp_c < 0.0 {
                    Precip::Snow
                } else {
                    Precip::Rain
                }
            }
            _ => Precip::None,
        }
    };
    let precip = if kind == Precip::None { 0.0 } else { precip };
    // Poussière : monde sec et venteux, pas de pluie
    let dry = 1.0 - w.ocean;
    let dust_noise = vnoise(dir * 4.0 + Vec3::splat((t / 900.0) as f32), w.seed ^ 0xD5);
    let dust = smooth(0.62, 0.8, dust_noise) * dry * smooth(6.0, 20.0, speed) * (1.0 - precip) * if w.pressure > 0.005 { 1.0 } else { 0.0 };
    // Brouillard du matin : humide, calme, vers 7 h
    let morning = 1.0 - smooth(0.0, 3.0, (hour - 7.0).abs());
    let fog = morning * humid * w.ocean.max(0.2) * (1.0 - smooth(3.0, 12.0, speed)) * smooth(0.3, 0.6, vnoise(dir * 9.0, w.seed ^ 0xF0)) * if w.water { 1.0 } else { 0.0 };
    Sample { cloud, precip, kind, storm, dust, fog, east, north, gust }
}

/// Éclair près du point `dir` dans la tranche d'une demi-seconde qui contient `t` : direction de
/// l'impact (repère fixe) s'il y en a un. Jamais sans nuage d'orage : il faut un nuage épais
/// au-dessus du point et au-dessus de l'impact.
pub fn lightning(w: &WeatherParams, dir: Vec3, storm: f32, t: f64, spread: f32) -> Option<Vec3> {
    if storm < 0.15 {
        return None;
    }
    let bin = (t * 2.0).floor() as i64 as u64;
    let cell = ((dir * 40.0).floor().as_ivec3()).to_array();
    let h = splitmix64(w.seed as u64 ^ bin.wrapping_mul(0x9E37_79B9) ^ ((cell[0] as u64) << 40) ^ ((cell[1] as u64 & 0xFFFFF) << 20) ^ (cell[2] as u64 & 0xFFFFF));
    let u = (h >> 11) as f64 / (1u64 << 53) as f64;
    if u > (storm as f64 * 0.3) {
        return None;
    }
    let a = ((h >> 8) & 0xFFFF) as f32 / 65536.0 * std::f32::consts::TAU;
    let r = ((h >> 24) & 0xFFFF) as f32 / 65536.0 * spread;
    let e1 = dir.any_orthonormal_vector();
    let e2 = dir.cross(e1);
    let hit = (dir + (e1 * a.cos() + e2 * a.sin()) * r).normalize();
    let stormy = |d: Vec3| {
        let (c, thick) = cloud_field(w, d, t);
        c > 0.6 && thick > 0.25
    };
    (stormy(dir) && stormy(hit)).then_some(hit)
}

/// Paramètres météo de l'astre `kind` (planète ou lune).
pub fn params_of(settings: &GameSettings, kind: &TargetKind) -> Option<WeatherParams> {
    match *kind {
        TargetKind::Planet(id) => WeatherParams::of(settings.systems.get(id / 1000)?.planets().get(id % 1000)?),
        TargetKind::Moon(pid, mi) => {
            let p = settings.systems.get(pid / 1000)?.planets().get(pid % 1000)?;
            let m = p.moons.get(mi)?;
            m.generated().then(|| WeatherParams::of(&m.as_planet(p))).flatten()
        }
        _ => None,
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  La météo là où l'on est
// ─────────────────────────────────────────────────────────────────────────

/// Météo au point où l'on séjourne (au sol ou en vol bas), et son résumé.
#[derive(Resource, Default)]
pub struct WeatherNow {
    pub body: Option<TargetKind>,
    pub params: Option<WeatherParams>,
    pub sample: Sample,
    /// Éclair en cours : direction de l'impact (repère fixe) et instant (horloge).
    pub flash: Option<(Vec3, f64)>,
    pub text: String,
}

impl WeatherNow {
    /// Lumière gardée sous les nuages et la poussière (0,3 à 1).
    pub fn light(&self) -> f32 {
        if self.body.is_none() {
            return 1.0;
        }
        let s = &self.sample;
        (1.0 - 0.55 * s.cloud - 0.15 * s.precip - 0.5 * s.dust).clamp(0.3, 1.0)
    }

    /// Visibilité gardée (brouillard, pluie, poussière), 0,05 à 1.
    pub fn visibility(&self) -> f32 {
        if self.body.is_none() {
            return 1.0;
        }
        let s = &self.sample;
        ((1.0 - 0.9 * s.fog) * (1.0 - 0.6 * s.precip) * (1.0 - 0.85 * s.dust)).clamp(0.05, 1.0)
    }

    /// Éclat de l'éclair (0..1) à l'instant `t`.
    pub fn flash_at(&self, t: f64) -> f32 {
        self.flash.map_or(0.0, |(_, t0)| (1.0 - ((t - t0) / 0.35) as f32).clamp(0.0, 1.0))
    }
}

fn describe(s: &Sample) -> String {
    let mut parts = Vec::new();
    let sky = match s.cloud {
        c if c < 0.15 => "ciel degage",
        c if c < 0.5 => "nuageux",
        _ => "couvert",
    };
    parts.push(sky.to_string());
    if s.precip > 0.02 {
        let force = if s.precip > 0.6 { " forte" } else if s.precip < 0.2 { " faible" } else { "" };
        parts.push(format!("{}{force}", s.kind.name()));
    }
    if s.storm > 0.25 {
        parts.push("orage".into());
    }
    if s.dust > 0.2 {
        parts.push("tempete de poussiere".into());
    }
    if s.fog > 0.2 {
        parts.push("brouillard".into());
    }
    let from = if s.east.abs() >= s.north.abs() { if s.east > 0.0 { "d'ouest" } else { "d'est" } } else if s.north > 0.0 { "du sud" } else { "du nord" };
    parts.push(format!("vent {:.0} km/h {from}", s.wind_speed() * 3.6));
    format!("Meteo : {}", parts.join(", "))
}

#[allow(clippy::too_many_arguments)]
fn update_weather_now(
    clock: Res<WorldClock>,
    settings: Res<GameSettings>,
    surface: Res<Surface>,
    local: Res<LocalWeather>,
    mut now: ResMut<WeatherNow>,
) {
    let (Some(kind), Some(p)) = (surface.body(), surface.local_point()) else {
        if now.body.is_some() {
            *now = WeatherNow::default();
        }
        return;
    };
    if now.body != Some(kind) {
        now.params = params_of(&settings, &kind);
        now.body = Some(kind);
        now.flash = None;
    }
    let Some(w) = now.params else {
        now.text.clear();
        now.sample = Sample::default();
        return;
    };
    let dir = p.normalize_or(Vec3::Y);
    let temp = if local.body == Some(kind) { local.temp } else { w.mean_c };
    let s = sample(&w, dir, clock.secs, temp, local.hour);
    // Éclairs autour de soi (un par demi-seconde au plus)
    if let Some(hit) = lightning(&w, dir, s.storm, clock.secs, 0.02) {
        let t0 = (clock.secs * 2.0).floor() / 2.0;
        if now.flash.map_or(true, |(_, t)| t != t0) {
            now.flash = Some((hit, t0));
        }
    }
    let under = surface.underground() > 0.5;
    now.sample = if under { Sample { east: s.east, north: s.north, ..Default::default() } } else { s };
    now.text = describe(&now.sample);
}

/// En vol bas, le vent pousse le vaisseau (plus fort en altitude, en rafale et dans les orages) et
/// le fait tanguer et rouler (`Surface::set_wind`).
fn push_ship(time: Res<Time>, now: Res<WeatherNow>, mut surface: ResMut<Surface>) {
    if !surface.flying() || now.params.is_none() {
        surface.set_wind(Vec3::ZERO, 0.0, String::new());
        return;
    }
    let Some(p) = surface.local_point() else { return };
    let up = p.normalize_or(Vec3::Y);
    // Est et nord dans le repère fixe de l'astre (axe = y)
    let east = Vec3::Y.cross(up).normalize_or(Vec3::X);
    let north = up.cross(east);
    let s = now.sample;
    // Plus fort en altitude (le sol freine le vent)
    let radius = surface.params().map_or(p.length(), |b| b.radius);
    let height = ((p.length() - radius) / (radius * 0.01).max(50.0)).clamp(0.0, 3.0);
    let k = 1.5 * (1.0 + s.storm) * (0.6 + 0.4 * height);
    let v = (east * s.east + north * s.north) * k;
    surface.drift(v * time.delta_secs().min(0.1));
    let speed = s.wind_speed() * (0.6 + 0.4 * height);
    let text = if speed < 0.5 {
        "Vent : calme".to_string()
    } else {
        format!("Vent : {:.0} m/s, du {}{}", speed, compass(-s.east, -s.north), if s.gust > 0.3 { ", rafale !" } else { "" })
    };
    surface.set_wind(v, s.gust, text);
}

/// Point cardinal d'une direction (vers l'est, vers le nord).
fn compass(e: f32, n: f32) -> &'static str {
    const NAMES: [&str; 8] = ["E", "NE", "N", "NO", "O", "SO", "S", "SE"];
    let a = n.atan2(e).rem_euclid(std::f32::consts::TAU);
    NAMES[((a / (std::f32::consts::TAU / 8.0)).round() as usize) % 8]
}

// ─────────────────────────────────────────────────────────────────────────
//  Couche de nuages (vue de l'espace et du sol)
// ─────────────────────────────────────────────────────────────────────────

/// Reconstructions en cours : planète -> tâche (maillage pour l'instant prévu).
#[derive(Resource, Default)]
struct CloudTasks {
    tasks: HashMap<usize, Task<Mesh>>,
    last: HashMap<usize, f64>,
}

/// Carreaux par face de la couche de nuages.
const CLOUD_CELLS: usize = 48;

/// Couche de nuages en cubes : chaque carreau nuageux est une dalle (plus épaisse dans les orages),
/// plus sombre quand il pleut.
pub fn cloud_mesh(w: &WeatherParams, t: f64, radius: f32) -> Mesh {
    let n = CLOUD_CELLS;
    let cell = std::f32::consts::FRAC_PI_2 * radius / n as f32;
    let (mut pos, mut nor, mut col, mut idx) = (Vec::new(), Vec::new(), Vec::new(), Vec::<u32>::new());
    let mut quad = |c: [Vec3; 4], normal: Vec3, color: [f32; 4]| {
        let base = pos.len() as u32;
        for p in c {
            pos.push(p.to_array());
            nor.push(normal.to_array());
            col.push(color);
        }
        let flip = (c[1] - c[0]).cross(c[2] - c[0]).dot(normal) < 0.0;
        if flip {
            idx.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
        } else {
            idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    };
    for face in 0..6u8 {
        let at = |i: f32, j: f32| crate::terrain::face_dir(face, -1.0 + 2.0 * i / n as f32, -1.0 + 2.0 * j / n as f32);
        // Épaisseur de chaque carreau (0 : ciel clair)
        let mut h = vec![0.0f32; n * n];
        let mut shade = vec![1.0f32; n * n];
        for j in 0..n {
            for i in 0..n {
                let d = at(i as f32 + 0.5, j as f32 + 0.5);
                let (c, thick) = cloud_field(w, d, t);
                if c > 0.35 {
                    h[j * n + i] = cell * (0.25 + 0.75 * thick) * (1.0 + (c > 0.8) as u8 as f32);
                    shade[j * n + i] = 1.0 - 0.55 * thick;
                }
            }
        }
        for j in 0..n {
            for i in 0..n {
                let k = h[j * n + i];
                if k <= 0.0 {
                    continue;
                }
                let s = shade[j * n + i];
                let color = [s, s, s * 1.02, 1.0];
                let (fi, fj) = (i as f32, j as f32);
                let corners = [at(fi, fj), at(fi + 1.0, fj), at(fi + 1.0, fj + 1.0), at(fi, fj + 1.0)];
                let (lo, hi) = (radius, radius + k);
                let up = at(fi + 0.5, fj + 0.5);
                quad(corners.map(|d| d * hi), up, color);
                quad(corners.map(|d| d * lo), -up, [s * 0.8, s * 0.8, s * 0.85, 1.0]);
                // Côtés là où le voisin est plus bas (ou au bord de la face)
                let side = |di: i32, dj: i32| {
                    let (ni, nj) = (i as i32 + di, j as i32 + dj);
                    if ni < 0 || nj < 0 || ni >= n as i32 || nj >= n as i32 { 0.0 } else { h[nj as usize * n + ni as usize] }
                };
                for (di, dj, a, b) in [(-1, 0, 3, 0), (1, 0, 1, 2), (0, -1, 0, 1), (0, 1, 2, 3)] {
                    let other = side(di, dj);
                    if other < k {
                        let (ca, cb) = (corners[a], corners[b]);
                        let base = radius + other;
                        let normal = (ca + cb - up * 2.0).normalize_or(up);
                        quad([ca * base, cb * base, cb * hi, ca * hi], normal, [s * 0.9, s * 0.9, s * 0.93, 1.0]);
                    }
                }
            }
        }
    }
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, nor);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
    mesh.insert_indices(Indices::U32(idx));
    mesh
}

/// Rayon de la couche de nuages d'une planète.
pub fn cloud_radius(p: &PlanetConfig) -> f32 {
    p.radius + p.terrain_height + p.cloud_altitude
}

#[allow(clippy::too_many_arguments)]
fn rebuild_clouds(
    time: Res<Time>,
    clock: Res<WorldClock>,
    settings: Res<GameSettings>,
    surface: Res<Surface>,
    target: Res<crate::ui::CameraTarget>,
    mut tasks: ResMut<CloudTasks>,
    mut layers: Query<(&CloudVoxel, &mut Mesh3d)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let now = time.elapsed_secs_f64();
    let tasks = &mut *tasks;
    // Maillages terminés
    let done: Vec<usize> = tasks.tasks.keys().copied().collect();
    for id in done {
        let Some(task) = tasks.tasks.get_mut(&id) else { continue };
        let Some(mesh) = block_on(future::poll_once(task)) else { continue };
        tasks.tasks.remove(&id);
        for (cv, mut m) in &mut layers {
            if cv.planet_idx == id {
                m.0 = meshes.add(mesh.clone());
            }
        }
    }
    let focus = surface.body().or(Some(target.0));
    let pool = AsyncComputeTaskPool::get();
    for (cv, _) in &layers {
        let id = cv.planet_idx;
        if tasks.tasks.contains_key(&id) {
            continue;
        }
        // La planète où l'on est (ou ciblée) toutes les 2 s, les autres toutes les 30 s
        let every = if focus == Some(TargetKind::Planet(id)) { 2.0 } else { 30.0 };
        if tasks.last.get(&id).is_some_and(|l| now - l < every) {
            continue;
        }
        let Some(p) = settings.systems.get(id / 1000).and_then(|s| s.planets().get(id % 1000)) else { continue };
        let Some(w) = WeatherParams::of(p) else { continue };
        tasks.last.insert(id, now);
        let (t, r) = (clock.secs + every.min(2.0) * 0.5, cloud_radius(p));
        tasks.tasks.insert(id, pool.spawn(async move { cloud_mesh(&w, t, r) }));
    }
    tasks.last.retain(|id, _| layers.iter().any(|(c, _)| c.planet_idx == *id));
}

// ─────────────────────────────────────────────────────────────────────────
//  Au sol : précipitations, éclairs
// ─────────────────────────────────────────────────────────────────────────

const PARTICLES: usize = 900;

#[derive(Component)]
struct WeatherFx;

#[derive(Component)]
struct Bolt;

/// Particules (repère fixe de l'astre, autour de la caméra) : position de chaque grain à
/// l'instant `t`, dans une boîte qui suit la caméra sans les traîner (réseau fixe de l'astre).
fn particle_positions(cam: Vec3, up: Vec3, fall: f32, wind: Vec3, t: f64, box_size: f32, count: usize, seed: u32) -> Vec<Vec3> {
    let e1 = up.any_orthonormal_vector();
    let e2 = up.cross(e1);
    // Caméra dans le repère (e1, e2, up)
    let c = Vec3::new(cam.dot(e1), cam.dot(e2), cam.dot(up));
    let vel = Vec3::new(wind.dot(e1), wind.dot(e2), -fall);
    (0..count)
        .map(|i| {
            let h = splitmix64(seed as u64 ^ (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
            let u = Vec3::new((h & 0xFFFF) as f32, ((h >> 16) & 0xFFFF) as f32, ((h >> 32) & 0xFFFF) as f32) / 65536.0;
            let tt = (t % 10_000.0) as f32;
            let q = u * box_size + vel * tt - c;
            let r = q - (q / box_size).floor() * box_size - Vec3::splat(box_size * 0.5);
            let local = c + r;
            e1 * local.x + e2 * local.y + up * local.z
        })
        .collect()
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn weather_fx(
    mut commands: Commands,
    clock: Res<WorldClock>,
    settings: Res<GameSettings>,
    surface: Res<Surface>,
    now: Res<WeatherNow>,
    planets: Query<(Entity, &Transform, &PlanetId), With<PlanetRoot>>,
    moons: Query<(Entity, &Transform, &crate::planet::MoonId), With<crate::planet::MoonRoot>>,
    cam_q: Query<&Transform, (With<Camera3d>, Without<PlanetRoot>)>,
    mut fx: Query<(Entity, &Mesh3d, &MeshMaterial3d<StandardMaterial>, &mut Visibility, Option<&Parent>), (With<WeatherFx>, Without<Bolt>)>,
    mut bolts: Query<(Entity, &mut Transform, &mut Visibility, Option<&Parent>), (With<Bolt>, Without<WeatherFx>, Without<PlanetRoot>, Without<crate::planet::MoonRoot>, Without<Camera3d>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Racine de l'astre où l'on est
    let root = match now.body {
        Some(TargetKind::Planet(id)) => planets.iter().find(|(_, _, p)| p.0 == id).map(|(e, t, _)| (e, *t)),
        Some(TargetKind::Moon(pid, mi)) => moons.iter().find(|(_, _, m)| m.planet_idx == pid && m.moon_idx == mi).map(|(e, t, _)| (e, *t)),
        _ => None,
    };
    let mut s = now.sample;
    // Tests : `SPACESPORE_TEST_PRECIP=<0..1>` force la pluie autour de la caméra
    if let Some(p) = std::env::var("SPACESPORE_TEST_PRECIP").ok().and_then(|v| v.parse::<f32>().ok()) {
        s.precip = p;
        s.dust = 0.0;
    }
    let active = root.is_some() && now.params.is_some() && surface.active() && surface.underground() < 0.5;
    let (Some((root_e, root_tf)), Ok(cam), true) = (root, cam_q.get_single(), active) else {
        for (_, _, _, mut v, _) in &mut fx {
            if *v != Visibility::Hidden {
                *v = Visibility::Hidden;
            }
        }
        for (_, _, mut v, _) in &mut bolts {
            if *v != Visibility::Hidden {
                *v = Visibility::Hidden;
            }
        }
        return;
    };
    let voxel = surface.terrain().map_or(5.0, |t| t.voxel());
    // Le nuage de particules (créé une fois, rattaché à l'astre où l'on est)
    if fx.is_empty() {
        let mat = materials.add(StandardMaterial { base_color: Color::WHITE, unlit: true, alpha_mode: AlphaMode::Blend, cull_mode: None, double_sided: true, ..default() });
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0f32; 3]; PARTICLES * 8]);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[1.0f32; 4]; PARTICLES * 8]);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 1.0, 0.0]; PARTICLES * 8]);
        let idx: Vec<u32> = (0..PARTICLES as u32 * 2).flat_map(|q| [q * 4, q * 4 + 1, q * 4 + 2, q * 4, q * 4 + 2, q * 4 + 3]).collect();
        mesh.insert_indices(Indices::U32(idx));
        // Sommets réécrits à chaque image : la boîte englobante calculée au départ (tout à zéro, au
        // centre de l'astre) ferait disparaître les particules hors de la vue
        let e = commands.spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(mat), Transform::IDENTITY, Visibility::Hidden, NotShadowCaster, NoFrustumCulling, WeatherFx)).id();
        commands.entity(root_e).add_child(e);
        let bolt_mat = materials.add(StandardMaterial { base_color: Color::linear_rgb(6.0, 6.0, 8.0), unlit: true, ..default() });
        let b = commands.spawn((Mesh3d(meshes.add(bolt_mesh(1))), MeshMaterial3d(bolt_mat), Transform::IDENTITY, Visibility::Hidden, NotShadowCaster, Bolt)).id();
        commands.entity(root_e).add_child(b);
        return;
    }
    let cam_local = root_tf.rotation.inverse() * (cam.translation - root_tf.translation);
    let up = cam_local.normalize_or(Vec3::Y);
    let (east, north) = {
        let e = Vec3::Y.cross(up).normalize_or(Vec3::X);
        (e, up.cross(e))
    };
    for (e, mesh_h, mat_h, mut v, parent) in &mut fx {
        // Changement d'astre : on rattache les particules au nouveau
        if parent.map(|p| p.get()) != Some(root_e) {
            commands.entity(root_e).add_child(e);
        }
        let (kind, amount) = if s.dust > s.precip { (None, s.dust) } else { (Some(s.kind), s.precip) };
        let count = ((amount.clamp(0.0, 1.0) * PARTICLES as f32) as usize).min(PARTICLES);
        let wanted = if count > 0 { Visibility::Inherited } else { Visibility::Hidden };
        if *v != wanted {
            *v = wanted;
        }
        if count == 0 {
            continue;
        }
        let (color, fall, streak) = match kind {
            Some(k) => k.look(),
            None => ([0.72, 0.55, 0.36], 0.3, false),
        };
        let wind = (east * s.east + north * s.north) * 0.25;
        let box_size = voxel * 48.0;
        let pts = particle_positions(cam_local, up, fall * voxel, wind * voxel, clock.secs, box_size, count, now.params.map_or(0, |w| w.seed));
        let side = up.cross(east).normalize_or(Vec3::X);
        let (len, wid) = if streak { (voxel * 0.9, voxel * 0.03) } else if kind.is_none() { (voxel * 0.35, voxel * 0.35) } else { (voxel * 0.12, voxel * 0.12) };
        let dirv = (-up * fall + wind).normalize_or(-up) * len;
        let alpha = if kind.is_none() { 0.35 } else { 0.55 };
        if let Some(mesh) = meshes.get_mut(&mesh_h.0) {
            let mut pos = Vec::with_capacity(PARTICLES * 8);
            for p in &pts {
                for w in [east * wid, side * wid] {
                    pos.extend_from_slice(&[(*p - w).to_array(), (*p + w).to_array(), (*p + w + dirv).to_array(), (*p - w + dirv).to_array()]);
                }
            }
            pos.resize(PARTICLES * 8, [0.0; 3]);
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, VertexAttributeValues::Float32x3(pos));
        }
        if let Some(m) = materials.get_mut(&mat_h.0) {
            m.base_color = Color::srgba(color[0], color[1], color[2], alpha);
        }
    }
    // Éclair : un trait du nuage au sol, une fraction de seconde
    let flash = now.flash_at(clock.secs);
    for (e, mut tf, mut v, parent) in &mut bolts {
        if parent.map(|p| p.get()) != Some(root_e) {
            commands.entity(root_e).add_child(e);
        }
        let wanted = if flash > 0.3 { Visibility::Inherited } else { Visibility::Hidden };
        if *v != wanted {
            *v = wanted;
        }
        if let (Some((hit, _)), true) = (now.flash, flash > 0.3) {
            let ground = surface.terrain().map_or(hit * cam_local.length(), |t| hit * t.ground(hit).top);
            let top = settings_cloud_base(&settings, now.body, ground.length() + voxel * 60.0);
            *tf = Transform::from_translation(ground).with_rotation(Quat::from_rotation_arc(Vec3::Y, hit)).with_scale(Vec3::new(voxel * 0.6, top - ground.length(), voxel * 0.6));
        }
    }
}

/// Base des nuages au-dessus du sol (rayon depuis le centre), d'après la planète.
fn settings_cloud_base(settings: &GameSettings, body: Option<TargetKind>, fallback: f32) -> f32 {
    match body {
        Some(TargetKind::Planet(id)) => settings.systems.get(id / 1000).and_then(|s| s.planets().get(id % 1000)).map_or(fallback, |p| cloud_radius(p).max(fallback)),
        _ => fallback,
    }
}

/// Éclair : une ligne brisée de hauteur 1 (de y = 0 au sol à y = 1 au nuage).
fn bolt_mesh(seed: u64) -> Mesh {
    const N: usize = 14;
    let mut pts = vec![Vec3::ZERO];
    let mut h = splitmix64(seed);
    for k in 1..=N {
        h = splitmix64(h);
        let jx = ((h & 0xFFFF) as f32 / 65536.0 - 0.5) * 30.0;
        let jz = (((h >> 16) & 0xFFFF) as f32 / 65536.0 - 0.5) * 30.0;
        pts.push(Vec3::new(jx * (k < N) as u8 as f32, k as f32 / N as f32, jz * (k < N) as u8 as f32));
    }
    let (mut pos, mut idx) = (Vec::new(), Vec::<u32>::new());
    for w in pts.windows(2) {
        let base = pos.len() as u32;
        for side in [Vec3::X, Vec3::Z] {
            let b = pos.len() as u32;
            pos.extend_from_slice(&[(w[0] - side).to_array(), (w[0] + side).to_array(), (w[1] + side).to_array(), (w[1] - side).to_array()]);
            idx.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
        }
        let _ = base;
    }
    let n = pos.len();
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; n]);
    mesh.insert_indices(Indices::U32(idx));
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    fn earth_like() -> WeatherParams {
        WeatherParams { seed: 42, cover: 0.6, wind: 12.0, pressure: 1.0, clouds: CloudKind::Water, ocean: 0.7, water: true, radius_m: 6.371e6, mean_c: 15.0 }
    }

    /// C3 : le vent n'est pas toujours dans le même sens, il a des rafales de quelques secondes,
    /// et c'est toujours le même pour la même graine, le même lieu, le même instant.
    #[test]
    fn wind_turns_and_gusts() {
        let w = earth_like();
        let dir = Vec3::new(0.3, 0.5, 0.8).normalize();
        let mut angles = Vec::new();
        let mut gusts = 0;
        for i in 0..2_000 {
            let t = i as f64 * 30.0;
            let (e, n, g) = wind(&w, dir, t, 0.0);
            assert!(e.is_finite() && n.is_finite() && (0.0..=1.0).contains(&g));
            angles.push(n.atan2(e));
            gusts += (g > 0.3) as usize;
        }
        let spread = angles.iter().map(|a| (a.cos(), a.sin())).fold((0.0f32, 0.0f32), |(x, y), (c, s)| (x + c, y + s));
        let mean_len = (spread.0 * spread.0 + spread.1 * spread.1).sqrt() / angles.len() as f32;
        assert!(mean_len < 0.9, "direction trop constante : {mean_len}");
        assert!(gusts > 20 && gusts < 1_000, "rafales : {gusts}");
        // Une rafale dure quelques secondes : pas de saut d'une image à l'autre
        let a = wind(&w, dir, 1_000.0, 0.0);
        let b = wind(&w, dir, 1_000.0 + 1.0 / 60.0, 0.0);
        assert!((a.0 - b.0).abs() < 1.0 && (a.1 - b.1).abs() < 1.0, "{a:?} {b:?}");
        assert_eq!(wind(&w, dir, 1234.5, 0.3), wind(&w, dir, 1234.5, 0.3));
    }

    #[test]
    fn two_machines_same_time_same_weather() {
        let w = earth_like();
        for k in 0..200 {
            let d = Vec3::new((k as f32 * 0.37).sin(), (k as f32 * 0.11).cos(), (k as f32 * 0.73).sin()).normalize();
            let t = 1000.0 + k as f64 * 371.3;
            assert_eq!(sample(&w, d, t, 12.0, 7.5), sample(&w, d, t, 12.0, 7.5));
            assert_eq!(lightning(&w, d, 0.8, t, 0.02), lightning(&w, d, 0.8, t, 0.02));
            // Pas d'éclair sous un ciel clair
            if let Some(hit) = lightning(&w, d, 0.8, t, 0.02) {
                assert!(cloud_field(&w, hit, t).0 > 0.6 && cloud_field(&w, d, t).0 > 0.6);
            }
        }
    }

    #[test]
    fn clouds_cover_about_their_share_and_move_with_the_wind() {
        let w = earth_like();
        let dirs: Vec<Vec3> = (0..4000).map(|k| {
            let z = -1.0 + 2.0 * ((k as f32 + 0.5) / 4000.0);
            let a = k as f32 * 2.399_963;
            let s = (1.0 - z * z).sqrt();
            Vec3::new(s * a.cos(), z, s * a.sin())
        }).collect();
        let cover = dirs.iter().map(|d| cloud_field(&w, *d, 5000.0).0).sum::<f32>() / dirs.len() as f32;
        assert!((0.3..0.8).contains(&cover), "couverture {cover}");
        // Une heure plus tard, ce n'est plus le même ciel au même endroit
        let changed = dirs.iter().filter(|d| (cloud_field(&w, **d, 5000.0).0 - cloud_field(&w, **d, 8600.0).0).abs() > 0.3).count();
        assert!(changed > 200, "{changed}");
        // Alizés vers l'ouest, vents d'ouest aux latitudes moyennes
        assert!(zonal(0.0) < 0.0 && zonal(0.8) > 0.0);
    }

    #[test]
    fn clouds_never_stretch_into_bands() {
        // Longtemps après le début de la partie : pas de traînées est-ouest
        let w = earth_like();
        let at = |lat: f32, lon: f32| Vec3::new(lat.cos() * lon.cos(), lat.sin(), lat.cos() * lon.sin());
        let d = 0.03;
        for t in [5_000.0, 3.0e6, 9.0e7] {
            let (mut ew, mut ns) = (0.0, 0.0);
            for i in 0..60 {
                for j in 0..60 {
                    let (lat, lon) = (-1.2 + i as f32 * 0.04, j as f32 * 0.1);
                    let c = cloud_field(&w, at(lat, lon), t).0;
                    ew += (c - cloud_field(&w, at(lat, lon + d), t).0).abs();
                    ns += (c - cloud_field(&w, at(lat + d, lon), t).0).abs();
                }
            }
            assert!(ns < ew * 2.5, "t {t} : bandes (nord-sud {ns}, est-ouest {ew})");
        }
    }

    #[test]
    fn precipitation_follows_temperature_and_atmosphere() {
        let mut found = HashMap::new();
        let mut look = |w: WeatherParams, temp: f32| {
            for k in 0..3000 {
                let d = Vec3::new((k as f32 * 0.37).sin(), (k as f32 * 0.11).cos(), (k as f32 * 0.73).sin()).normalize();
                let s = sample(&w, d, k as f64 * 97.0, temp, 13.0);
                if s.precip > 0.05 {
                    *found.entry(s.kind).or_insert(0) += 1;
                }
            }
        };
        look(earth_like(), 15.0);
        look(earth_like(), -15.0);
        look(earth_like(), 28.0);
        look(WeatherParams { clouds: CloudKind::Methane, water: false, ocean: 0.2, ..earth_like() }, -180.0);
        look(WeatherParams { clouds: CloudKind::Sulfuric, water: false, ocean: 0.0, ..earth_like() }, 460.0);
        look(WeatherParams { clouds: CloudKind::Exotic, water: false, ocean: 0.0, ..earth_like() }, 1200.0);
        for k in [Precip::Rain, Precip::Snow, Precip::Hail, Precip::Methane, Precip::Acid, Precip::Glass] {
            assert!(found.get(&k).copied().unwrap_or(0) > 0, "{k:?} jamais vu : {found:?}");
        }
    }

    #[test]
    fn dust_storms_on_dry_windy_worlds_and_morning_fog_on_wet_ones() {
        let mars = WeatherParams { clouds: CloudKind::CarbonDioxide, water: false, ocean: 0.0, wind: 25.0, pressure: 0.006, ..earth_like() };
        let dusty = (0..3000).filter(|k| sample(&mars, Vec3::new((*k as f32).sin(), 0.2, (*k as f32).cos()).normalize(), *k as f64 * 61.0, -60.0, 13.0).dust > 0.2).count();
        assert!(dusty > 50, "{dusty}");
        let calm = WeatherParams { wind: 2.0, ..earth_like() };
        let fog = |hour: f32| (0..3000).filter(|k| sample(&calm, Vec3::new((*k as f32).sin(), 0.2, (*k as f32).cos()).normalize(), *k as f64 * 61.0, 10.0, hour).fog > 0.2).count();
        assert!(fog(7.0) > 100 && fog(15.0) == 0, "{} {}", fog(7.0), fog(15.0));
    }

    #[test]
    fn particles_stay_in_their_box_around_the_camera() {
        let cam = Vec3::new(10.0, 6000.0, -4.0);
        let up = cam.normalize();
        for t in [0.0, 1.5, 100.0, 7777.7] {
            for p in particle_positions(cam, up, 30.0, Vec3::X * 5.0, t, 200.0, 300, 7) {
                assert!((p - cam).abs().max_element() <= 200.0, "{p} loin de {cam}");
            }
        }
    }

    #[test]
    #[ignore]
    fn bench_cloud_layer() {
        let w = earth_like();
        let t0 = std::time::Instant::now();
        let m = cloud_mesh(&w, 1234.0, 6500.0);
        println!("couche de nuages : {:.1} ms, {} sommets", t0.elapsed().as_secs_f64() * 1000.0, m.count_vertices());
    }
}
