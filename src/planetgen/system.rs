//! Planètes et lunes d'un système (phase 2) : orbites en UA, classes de taille, masse → rayon,
//! densité, gravité, vitesse de libération, rotation, géantes gazeuses et neptuniennes.
//!
//! Tout est calculé en unités réelles puis converti pour l'affichage :
//! - tailles : 1 R⊕ = échelle G du système / 109 (proportions réelles avec une étoile G) ;
//! - distances : échelle logarithmique (chaque doublement de la distance en UA = même écart à
//!   l'écran), la zone habitable tombant vers 3,2 fois l'échelle, comme les planètes tempérées
//!   d'avant. Les orbites sont ensuite écartées pour que planètes et lunes ne se touchent jamais.
//!
//! Les valeurs affichées sont arrondies (rayon à 1 unité, orbite à 10) : identiques sur toutes
//! les machines malgré `powf` et `ln` (empreinte réseau).

use serde::{Deserialize, Serialize};

use super::atmosphere::{self, Air, AirInput};
use super::climate::Climate;
use super::habitability::{self, HabInput, Habitability};
use super::traits;
use super::genome::SystemGenome;
use super::biome::{self, BiomeInput, BiomeParams};
use super::geology::{self, GeoInput, Geology};
use super::hydrology::{self, HydroInput, Hydrology, Liquid, WaterState};
use super::life::{self, Life, LifeInput};
use super::seeds::{Layer, LayerRng};
use super::star::{StarClass, StarPhysics};
use crate::settings::{MoonConfig, PlanetConfig};

/// Nature d'une planète.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlanetKind {
    #[default]
    Rocky,
    /// 1,5 à 3,5 R⊕, enveloppe de gaz : pas de surface.
    MiniNeptune,
    /// Neptune, Uranus : 3 à 5 R⊕.
    IceGiant,
    /// Jupiter, Saturne : 8 à 14 R⊕.
    GasGiant,
}

impl PlanetKind {
    /// Pas de sol : on y entre en vol, la pression abîme le vaisseau.
    pub fn gaseous(self) -> bool {
        self != PlanetKind::Rocky
    }
}

/// Classe de taille (« minuscule » à « géante »).
pub fn size_class(kind: PlanetKind, radius_earth: f64, hot: bool) -> &'static str {
    match kind {
        PlanetKind::GasGiant if hot => "geante chaude",
        PlanetKind::GasGiant => "geante gazeuse",
        PlanetKind::IceGiant => "geante de glace",
        PlanetKind::MiniNeptune => "mini-Neptune",
        PlanetKind::Rocky => match radius_earth {
            r if r < 0.5 => "minuscule",
            r if r < 0.8 => "petite",
            r if r < 1.25 => "terrestre",
            _ => "super-Terre",
        },
    }
}

/// Rayon (R⊕) d'après la masse (M⊕) : relation de Chen et Kipping (2017), rocheuse jusqu'à
/// 2 M⊕, « neptunienne » jusqu'à 130 M⊕, puis quasi constante (les géantes se tassent).
pub fn radius_from_mass(kind: PlanetKind, mass: f64) -> f64 {
    match kind {
        PlanetKind::Rocky => mass.powf(0.279),
        _ if mass < 130.0 => 1.22 * (mass / 2.04).powf(0.589),
        _ => 12.1 * (mass / 130.0).powf(-0.044),
    }
}

/// Masse d'une lune (M⊕) d'après son rayon (R⊕) : roche et glace (Lune ≈ 0,012 M⊕).
pub fn moon_mass(radius: f64, icy: bool) -> f64 {
    0.9 * radius.powf(3.4) * if icy { 0.6 } else { 1.0 }
}

/// Arrondi au multiple de `step` (valeurs affichées, identiques sur toutes les machines).
fn q(x: f64, step: f64) -> f32 {
    ((x / step).round() * step) as f32
}

const SPACE_STRETCH_F64: f64 = crate::settings::SPACE_STRETCH as f64;

/// Une planète errante sur 30 systèmes.
pub const ROGUE_CHANCE: f64 = 1.0 / 30.0;
/// « Distance » d'une planète errante à l'étoile pour la physique (pas de lumière, pas d'orbite).
const ROGUE_AU: f64 = 5000.0;

/// Rayon du Soleil en UA.
const SUN_RADIUS_AU: f64 = 0.004_65;

/// Position affichée (en échelles G) d'une orbite de `a` UA, `hz` : distance de la zone habitable.
pub fn display_distance(a: f64, hz: f64) -> f64 {
    let x = (a / hz).log2();
    if x >= -1.0 { 3.2 + 0.9 * x } else { 2.3 + 0.3 * (x + 1.0) }.max(1.25)
}

/// Ce que les étoiles imposent aux orbites (C3) : planètes au-delà de `min_au` (paire serrée au
/// centre) et du rayon affiché `exclusion`, en deçà de `max_au` (compagnon lointain) ; ceintures et
/// comètes en deçà du rayon affiché `outer`.
#[derive(Clone, Copy, Debug)]
pub struct OrbitLimits {
    pub min_au: f64,
    pub max_au: f64,
    pub exclusion: f64,
    pub outer: f64,
}

impl Default for OrbitLimits {
    fn default() -> Self {
        Self { min_au: 0.0, max_au: f64::INFINITY, exclusion: 0.0, outer: f64::INFINITY }
    }
}

/// Une planète en cours de génération (unités réelles).
struct Draft {
    kind: PlanetKind,
    au: f64,
    mass: f64,
    radius: f64,
    hot: bool,
}

/// Planètes et lunes d'un système.
///
/// `scale` : échelle G du système (rayon d'une G, 600 000 à 1 500 000) ; `star_radius` : rayon
/// affiché de l'étoile (les orbites restent hors d'elle).
pub fn generate(genome: SystemGenome, star: &StarPhysics, scale: f32, star_radius: f32, limits: OrbitLimits) -> Vec<PlanetConfig> {
    let scale = scale as f64;
    let earth = scale / 109.0;
    let lum = star.luminosity_sun.max(1e-7);
    let hz = lum.sqrt();
    let snow = 2.7 * hz;
    let remnant = matches!(star.class, StarClass::WhiteDwarf | StarClass::BrownDwarf);
    let fgk = matches!(star.class, StarClass::F | StarClass::G | StarClass::K);

    let mut orbit = LayerRng::new(genome.seed as u64, Layer::Orbit);
    // 1 à 8 planètes (surtout 3 à 6) ; 1 à 4 autour d'une naine blanche ou brune
    let n = if remnant {
        1 + orbit.weighted(&[0.35, 0.3, 0.2, 0.15])
    } else {
        1 + orbit.weighted(&[0.06, 0.1, 0.15, 0.17, 0.17, 0.14, 0.11, 0.1])
    };

    // ── Orbites et nature des planètes (unités réelles) ─────────────────
    let mut a = hz * 10f64.powf(orbit.range(-1.1, -0.35));
    // Jamais dans l'étoile (une géante rouge a englouti ses planètes proches)
    a = a.max(star.radius_sun * SUN_RADIUS_AU * 3.0);
    // Étoile double serrée : au-delà de la zone instable autour de la paire (C3)
    a = a.max(limits.min_au * 1.1);
    let mut drafts = Vec::with_capacity(n);
    for pi in 0..n {
        if pi > 0 {
            a *= orbit.range(1.4, 2.3);
        }
        // Compagnon lointain : pas d'orbite stable au-delà (C3)
        if a > limits.max_au {
            if pi > 0 {
                break;
            }
            a = (limits.max_au * 0.6).max(limits.min_au * 1.1);
        }
        let seed = genome.planet_base.wrapping_add(pi as u32);
        let mut phys = LayerRng::new(seed as u64, Layer::Physics);
        let roll = phys.unit();
        let hot_jupiter = fgk && a < 0.12 * hz && roll < 0.03;
        let kind = if hot_jupiter {
            PlanetKind::GasGiant
        } else if a < snow || remnant {
            if roll < 0.2 { PlanetKind::MiniNeptune } else { PlanetKind::Rocky }
        } else if roll < 0.35 {
            PlanetKind::GasGiant
        } else if roll < 0.65 {
            PlanetKind::IceGiant
        } else {
            PlanetKind::Rocky
        };
        let u = phys.unit();
        let mass = match kind {
            PlanetKind::Rocky => 10f64.powf(-1.7 + 2.6 * u.powf(1.2)),
            PlanetKind::MiniNeptune => 3.0 * 4f64.powf(u),
            PlanetKind::IceGiant => 10.0 * 3f64.powf(u),
            PlanetKind::GasGiant => 40.0 * 100f64.powf(u.powf(1.5)),
        };
        let mut radius = radius_from_mass(kind, mass);
        if hot_jupiter {
            radius *= 1.2; // gonflée par la chaleur
        }
        drafts.push(Draft { kind, au: a, mass, radius, hot: hot_jupiter });
    }

    // ── Planète errante (C2, rare) : loin de toute étoile, sans lumière ──
    let mut rogue = LayerRng::new(genome.seed as u64 ^ 0x524F_4755, Layer::Belts);
    if rogue.unit() < ROGUE_CHANCE {
        let roll = rogue.unit();
        let kind = if roll < 0.6 { PlanetKind::Rocky } else if roll < 0.85 { PlanetKind::IceGiant } else { PlanetKind::GasGiant };
        let u = rogue.unit();
        let mass = match kind {
            PlanetKind::Rocky => 10f64.powf(-1.0 + 1.8 * u),
            PlanetKind::IceGiant => 10.0 * 3f64.powf(u),
            _ => 40.0 * 100f64.powf(u.powf(1.5)),
        };
        drafts.push(Draft { kind, au: ROGUE_AU, mass, radius: radius_from_mass(kind, mass), hot: false });
    }

    // ── Planètes et lunes : toute la chaîne (atmosphère → biomes), anneaux, aurores ──
    let mut planets: Vec<PlanetConfig> = Vec::with_capacity(n);
    for (pi, d) in drafts.iter().enumerate() {
        let pu = pi as u32;
        let seed = genome.planet_base.wrapping_add(pu);
        let mut relief = LayerRng::new(seed as u64, Layer::Relief);
        let mut spin = LayerRng::new(seed as u64, Layer::Orbit);
        let radius = q(d.radius * earth, 1.0).max(50.0);
        let gaseous = d.kind.gaseous();
        let icy = d.au > snow;

        // Rotation : bloquée près de l'étoile (une face toujours éclairée), sinon 10 à 40 h
        // (9 à 17 h pour les géantes) ; inclinaison de l'axe surtout faible, parfois couchée
        let period_days = 365.25 * (d.au.powi(3) / star.mass_sun.max(0.01)).sqrt();
        let locked = !gaseous && d.au < 0.4 * star.mass_sun.max(0.01).cbrt();
        let rotation_h = if locked {
            period_days * 24.0
        } else if gaseous {
            spin.range(9.0, 17.0)
        } else {
            10.0 * 4f64.powf(spin.unit())
        };
        let tilt_roll = spin.unit();
        let axial_tilt = if locked { 0.0 } else if tilt_roll < 0.05 { spin.range(60.0, 180.0) } else { 35.0 * spin.unit().powf(1.5) };

        let world = WorldInput { kind: d.kind, mass: d.mass, radius: d.radius, au: d.au, star, lum, snow, locked, rotation_h, axial_tilt, tidal: 0.0 };
        let layers = world_layers(&world, seed);

        // Lunes : 0 à 2 pour une rocheuse, davantage pour une géante ; toute la chaîne (une
        // lune peut avoir de l'air, comme Titan) et le chauffage par les marées (comme Io)
        let moon_count = match d.kind {
            PlanetKind::Rocky if d.radius < 0.5 => spin.weighted(&[0.6, 0.4]),
            PlanetKind::Rocky | PlanetKind::MiniNeptune => spin.weighted(&[0.3, 0.45, 0.25]),
            PlanetKind::IceGiant => 1 + spin.weighted(&[0.3, 0.3, 0.25, 0.15]),
            PlanetKind::GasGiant => 2 + spin.weighted(&[0.35, 0.35, 0.3]),
        };
        let mut moons: Vec<MoonConfig> = Vec::with_capacity(moon_count);
        for mi in 0..moon_count {
            let mu = mi as u32;
            let moon_seed = genome.planet_base.wrapping_add(500 + pu * 8 + mu);
            let mut m = LayerRng::new(moon_seed as u64, Layer::Physics);
            let mut moon_relief = LayerRng::new(moon_seed as u64, Layer::Relief);
            // Rocheuse : 1/8 à 1/3 de sa planète ; géante : de 0,08 à 0,45 R⊕ (Ganymède 0,41)
            let r_earth = if gaseous {
                m.range(0.08, 0.45).min(d.radius / 3.0)
            } else {
                d.radius * m.range(0.12, 0.33)
            };
            let moon_radius = q(r_earth * earth, 1.0).max(40.0);
            let spread = if gaseous { (1.8, 1.1, 0.4) } else { (2.4, 1.7, 0.5) };
            let mut orbit_distance = radius as f64 * (spread.0 + spread.1 * mi as f64 + spread.2 * m.unit()) + moon_radius as f64 * 3.0;
            // Jamais sur la précédente (une grosse lune sur une orbite large)
            if let Some(prev) = moons.last() {
                orbit_distance = orbit_distance.max((prev.orbit_distance + prev.radius) as f64 + 2.0 * moon_radius as f64 + radius as f64 * 0.3);
            }
            let mass = moon_mass(r_earth, icy);
            let eccentricity = q(m.range(0.0, 0.03), 1e-4);
            // Marées : fortes près d'une planète massive, sur une orbite un peu excentrique (Io : 0,5)
            let tidal = tidal_heating(d.mass, orbit_distance / radius as f64, eccentricity as f64);
            let mw = WorldInput {
                kind: PlanetKind::Rocky,
                mass,
                radius: r_earth,
                au: d.au,
                star,
                lum,
                snow,
                locked: false,
                rotation_h: 24.0 * moon_relief.range(2.0, 16.0),
                axial_tilt: 0.0,
                tidal,
            };
            let ml = world_layers(&mw, moon_seed);
            let habitability = evaluate_habitability(&ml, false, r_earth, mass);
            let traits = traits::traits_of(&trait_context(&ml, false, 0.0, false, false, false), &mut LayerRng::new(moon_seed as u64, Layer::Traits));
            moons.push(MoonConfig {
                orbit_distance: q(orbit_distance, 1.0),
                radius: moon_radius,
                seed: moon_seed,
                eccentricity,
                inclination: q(m.range(-0.05, 0.05), 1e-4),
                ascending_node: q(m.range(0.0, std::f64::consts::TAU), 1e-4),
                arg_periapsis: q(m.range(0.0, std::f64::consts::TAU), 1e-4),
                mean_anomaly_0: q(m.range(0.0, std::f64::consts::TAU), 1e-4),
                radius_earth: r_earth as f32,
                mass_earth: mass as f32,
                gravity_g: (mass / (r_earth * r_earth)) as f32,
                climate: Some(ml.climate),
                relief: Some(ml.geology.relief),
                sea_level: q(ml.sea_level as f64, 1e-4),
                terrain_height: q(moon_radius as f64 * (0.035 + moon_relief.unit() * 0.02), 0.1),
                noise_scale: q(1.5 + moon_relief.unit() * 2.0, 1e-4),
                detail_scale: q(3.0 + moon_relief.unit() * 3.0, 1e-4),
                atmosphere: ml.atmosphere,
                tidal_heat: tidal as f32,
                air: ml.air,
                hydrology: ml.hydrology,
                geology: ml.geology,
                biomes: ml.biomes,
                habitability,
                traits,
                life: ml.life,
                resources: ml.resources,
            });
        }

        // Anneaux : surtout les géantes ; glace claire au-delà de la ligne des glaces, roche sombre
        // en deçà ; jamais jusqu'à la première lune
        let ring_chance = match d.kind {
            PlanetKind::GasGiant => 0.6,
            PlanetKind::IceGiant => 0.4,
            PlanetKind::MiniNeptune => 0.1,
            PlanetKind::Rocky => 0.02,
        };
        let ring = if spin.unit() < ring_chance {
            let r = radius as f64;
            let inner = r * spin.range(1.25, 1.5);
            let mut outer = r * spin.range(1.8, 2.6);
            if let Some(first) = moons.first() {
                outer = outer.min((first.orbit_distance - 2.0 * first.radius) as f64);
            }
            let opacity = spin.range(0.35, 0.8) as f32;
            // Glace, divisions (C2) : tirages à part, les autres ne bougent pas
            let mut look = LayerRng::new(seed as u64 ^ 0x5249_4E47, Layer::Belts);
            let ice = if icy { look.range(0.7, 1.0) } else { look.range(0.0, 0.3) } as f32;
            let mut gaps = [[0.0f32; 2]; 3];
            for (k, g) in gaps.iter_mut().enumerate() {
                if k == 0 || look.unit() < 0.5 {
                    *g = [look.range(0.25, 0.85) as f32, look.range(0.015, 0.06) as f32];
                }
            }
            let rock = [0.48, 0.43, 0.38];
            let frost = [0.9, 0.86, 0.78];
            let color = [0, 1, 2].map(|i| rock[i] + (frost[i] - rock[i]) * ice);
            (outer > inner * 1.15).then(|| Ring {
                inner: q(inner, 1.0),
                outer: q(outer, 1.0),
                color,
                opacity,
                ice,
                gaps,
                seed: look.next_u64() as u32,
            })
        } else {
            None
        };
        let aurora = aurora_of(&layers, gaseous, star, d.au);
        let habitability = evaluate_habitability(&layers, gaseous, d.radius, d.mass);
        let traits = traits::traits_of(
            &trait_context(&layers, gaseous, axial_tilt as f32, locked, ring.is_some(), aurora.is_some()),
            &mut LayerRng::new(seed as u64, Layer::Traits),
        );

        let Layers { air, climate, hydrology, sea_level, geology, biomes, atmosphere, life, resources } = layers;
        planets.push(PlanetConfig {
            orbit_distance: 0.0, // placée plus bas
            radius,
            sea_level: q(sea_level as f64, 1e-4),
            terrain_height: if gaseous { 0.0 } else { q(radius as f64 * (0.025 + relief.unit() * 0.025), 0.1) },
            seed,
            noise_scale: q(1.5 + relief.unit() * 2.0, 1e-4),
            detail_scale: q(3.0 + relief.unit() * 3.0, 1e-4),
            moons,
            star_radius: scale as f32,
            atmosphere,
            cloud_density: air.cloud_cover,
            cloud_speed: 0.02 * (air.wind_ms / 10.0).clamp(0.2, 4.0),
            cloud_altitude: q(80.0 + radius as f64 * 0.05 * (1.0 + relief.unit()), 0.1),
            eccentricity: q(0.3 * orbit.unit().powi(3) * if d.au < 0.1 * hz { 0.2 } else { 1.0 }, 1e-4),
            inclination: q(orbit.range(-0.05, 0.05), 1e-4),
            ascending_node: q(orbit.range(0.0, std::f64::consts::TAU), 1e-4),
            arg_periapsis: q(orbit.range(0.0, std::f64::consts::TAU), 1e-4),
            mean_anomaly_0: q(orbit.range(0.0, std::f64::consts::TAU), 1e-4),
            kind: d.kind,
            hot: d.hot,
            mass_earth: d.mass as f32,
            radius_earth: d.radius as f32,
            semi_major_au: d.au as f32,
            period_days: period_days as f32,
            rotation_h: rotation_h as f32,
            axial_tilt: axial_tilt as f32,
            tidally_locked: locked,
            gravity_g: (d.mass / (d.radius * d.radius)) as f32,
            temperature_c: Some(climate.mean_c),
            climate: Some(climate),
            air,
            hydrology,
            geology,
            biomes,
            ring,
            aurora,
            habitability,
            traits,
            life,
            resources,
            ..Default::default()
        });
    }

    // ── Orbites affichées : échelle logarithmique, puis écartées ─────────
    // `reach` : rayon de la planète et de ses lunes ; marge entre deux voisines
    let reach = |p: &PlanetConfig| -> f64 {
        let ring = p.ring.map_or(0.0, |r| r.outer as f64);
        p.moons.iter().map(|m| (m.orbit_distance + m.radius) as f64).fold((p.radius as f64).max(ring), f64::max)
    };
    let margin = 0.08 * scale;
    // (les limites des étoiles sont en distances affichées, étirement compris)
    let mut previous_apoapsis = (star_radius as f64 * 1.3).max(limits.exclusion / SPACE_STRETCH_F64);
    let mut previous_reach = 0.0;
    // Compagnon lointain (C3) : rien ne tourne au-delà de la moitié de son passage au plus près
    let count = planets.len();
    let mut keep = count;
    for (k, (p, d)) in planets.iter_mut().zip(&drafts).enumerate() {
        if d.au >= ROGUE_AU {
            // Planète errante : bien au-delà de tout le reste, hors du plan des orbites, immobile
            p.rogue = true;
            p.orbit_distance = q((previous_apoapsis * rogue.range(2.5, 3.5)).max(4.0 * scale), 10.0);
            p.eccentricity = 0.0;
            p.inclination = q(rogue.range(0.5, 1.3) * if rogue.unit() < 0.5 { -1.0 } else { 1.0 }, 1e-4);
            continue;
        }
        let wanted = display_distance(d.au, hz) * scale;
        let mut e = p.eccentricity as f64;
        let r = reach(p);
        // Le périastre reste au-delà de l'apoastre de la précédente (et de l'étoile). Si la place
        // manque, l'orbite s'arrondit au lieu de repousser tout le reste du système.
        let min_periapsis = previous_apoapsis + previous_reach + r + margin;
        if wanted * (1.0 - e) < min_periapsis {
            e = e.min(0.05);
            p.eccentricity = q(e, 1e-4);
            e = p.eccentricity as f64;
        }
        let distance = wanted.max(min_periapsis / (1.0 - e));
        if k > 0 && keep == count && (distance * (1.0 + e) + r) * SPACE_STRETCH_F64 > limits.outer {
            keep = k;
        }
        p.orbit_distance = q(distance, 10.0);
        previous_apoapsis = p.orbit_distance as f64 * (1.0 + e);
        previous_reach = r;
    }
    if keep < planets.len() {
        // La planète errante reste (elle est ailleurs, loin de tout)
        let rogue = planets.pop().filter(|p| p.rogue);
        planets.truncate(keep);
        planets.extend(rogue);
    }
    // Étirement visuel (×5) : toute la disposition du système s'agrandit d'un bloc, après la
    // physique (marées, anneaux, espacement) calculée sur les distances d'origine
    let stretch = crate::settings::SPACE_STRETCH;
    for p in &mut planets {
        p.orbit_distance *= stretch;
        for m in &mut p.moons {
            m.orbit_distance *= stretch;
        }
    }
    planets
}

/// Un monde (planète ou lune) pour toute la chaîne de génération.
struct WorldInput<'a> {
    kind: PlanetKind,
    mass: f64,
    radius: f64,
    au: f64,
    star: &'a StarPhysics,
    lum: f64,
    snow: f64,
    locked: bool,
    rotation_h: f64,
    axial_tilt: f64,
    tidal: f64,
}

/// Couches calculées d'un monde.
struct Layers {
    air: Air,
    climate: Climate,
    hydrology: Hydrology,
    sea_level: f32,
    geology: Geology,
    biomes: BiomeParams,
    atmosphere: bool,
    life: Life,
    resources: super::resources::Resources,
}

/// Chauffage par les marées (0..1) d'une lune à `ratio` rayons (affichés) de sa planète de
/// `planet_mass` M⊕. Les orbites des lunes sont comprimées à l'écran : la distance physique vaut
/// ~2,5 fois la distance affichée. Io (Jupiter, 5,9 rayons) ≈ 0,3 à 0,8 selon son excentricité ;
/// Europe ≈ 0,05 ; négligeable autour d'une rocheuse.
pub fn tidal_heating(planet_mass: f64, ratio: f64, eccentricity: f64) -> f64 {
    let physical = 2.5 * ratio.max(1.0);
    (0.6 * (planet_mass / 318.0) * (6.0 / physical).powi(5) * (eccentricity / 0.01)).clamp(0.0, 1.0)
}

/// Atmosphère (phase 3), eau (phase 4), géologie (phase 5), biomes (phase 6).
fn world_layers(w: &WorldInput, seed: u32) -> Layers {
    let gaseous = w.kind.gaseous();
    let star = w.star;
    let (air, climate) = atmosphere::generate(
        &AirInput {
            kind: w.kind,
            mass: w.mass,
            radius: w.radius,
            au: w.au,
            luminosity: w.lum,
            xray: star.xray_flux,
            star_color: star.color,
            locked: w.locked,
            rotation_h: w.rotation_h,
            axial_tilt: w.axial_tilt,
        },
        &mut LayerRng::new(seed as u64, Layer::Atmosphere),
    );
    let atmosphere = !gaseous && air.present();
    let (mut hydrology, sea_level) = hydrology::generate(
        &HydroInput { kind: w.kind, snow_ratio: w.au / w.snow, mass: w.mass, air: &air, climate: &climate },
        &mut LayerRng::new(seed as u64, Layer::Hydrology),
    );
    // Une lune glacée chauffée par les marées cache un océan sous sa glace (Europe, Encelade)
    hydrology.subsurface_ocean = w.tidal > 0.05 && hydrology.water_state == WaterState::Ice && hydrology.inventory > 0.1;
    let liquid_water = hydrology.hydro.liquid == Liquid::Water && hydrology.water_state == WaterState::Liquid;
    let geology = if gaseous {
        // Une géante : dynamo d'hydrogène métallique, champ magnétique bien plus fort que la Terre
        Geology { age_gyr: star.age_gyr as f32, magnetic_field: (w.mass / 20.0).sqrt().clamp(1.0, 20.0) as f32, ..Default::default() }
    } else {
        geology::generate(
            &GeoInput {
                mass: w.mass,
                age_gyr: star.age_gyr,
                gravity: w.mass / (w.radius * w.radius),
                liquid_water,
                pressure: air.pressure_bar as f64,
                ice: hydrology.ice_caps > 0.05,
                wind_ms: air.wind_ms as f64,
                rotation_h: w.rotation_h,
                locked: w.locked,
                tidal: w.tidal,
            },
            seed,
            &mut LayerRng::new(seed as u64, Layer::Geology),
        )
    };
    // Vie (phase 7) : indépendante de l'habitabilité ; ses plantes verdissent les biomes
    let radiation = biome::surface_radiation(
        star.uv_flux * w.lum / (w.au * w.au),
        star.xray_flux / (w.au * w.au),
        air.pressure_bar as f64,
        geology.magnetic_field as f64,
        air.fraction("O2") as f64,
    );
    let life = if gaseous {
        Life::default()
    } else {
        life::generate(
            &LifeInput {
                liquid: hydrology.hydro.liquid,
                water_state: hydrology.water_state,
                subsurface_ocean: hydrology.subsurface_ocean,
                atmosphere,
                pressure: air.pressure_bar as f64,
                oxygen: air.fraction("O2") as f64,
                fictional_gas: air.gases.iter().any(|(f, x)| *x > 0.01 && matches!(f.as_str(), "Ae" | "Sp" | "Cx")),
                radiation,
                age_gyr: star.age_gyr,
                gravity: w.mass / (w.radius * w.radius),
                ocean_fraction: hydrology.ocean_fraction as f64,
                mean_c: climate.mean_c as f64,
            },
            &mut LayerRng::new(seed as u64, Layer::Biology),
        )
    };
    let biomes = if gaseous {
        BiomeParams::default()
    } else {
        biome::generate(&BiomeInput {
            seed,
            ocean_fraction: hydrology.ocean_fraction as f64,
            liquid_water,
            cloud_cover: air.cloud_cover as f64,
            pressure: air.pressure_bar as f64,
            oxygen: air.fraction("O2") as f64,
            sulfur: air.fraction("SO2") as f64,
            volcanism: geology.volcanism as f64,
            density: super::units::density(w.mass, w.radius),
            surface_age: geology.surface_age_gyr as f64,
            magnetic_field: geology.magnetic_field as f64,
            // UV de l'étoile donnés pour sa zone habitable (√L) : ramenés à cette orbite
            uv: star.uv_flux * w.lum / (w.au * w.au),
            xray: star.xray_flux / (w.au * w.au),
            dried_water: hydrology.inventory > 0.1 && !liquid_water,
            flora: life.flora,
        })
    };
    // Ressources (phase 8) : composition globale et gisements
    let resources = super::resources::generate(
        &super::resources::ResourceInput {
            gaseous,
            mass: w.mass,
            radius: w.radius,
            density: super::units::density(w.mass, w.radius),
            icy: w.au > w.snow,
            gravity: w.mass / (w.radius * w.radius),
            mean_c: climate.mean_c as f64,
            pressure: air.pressure_bar as f64,
            atmosphere,
            volcanism: geology.volcanism as f64,
            plates: geology.tectonics == super::geology::Tectonics::Plates,
            surface_age: geology.surface_age_gyr as f64,
            craters: geology.relief.craters as f64,
            liquid_water,
            ocean_fraction: hydrology.ocean_fraction as f64,
            methane_seas: hydrology.hydro.liquid == Liquid::Methane && hydrology.ocean_fraction > 0.0,
            stellar_wind: star.stellar_wind / (w.au * w.au),
            fictional_gas: air.gases.iter().any(|(f, x)| *x > 0.01 && matches!(f.as_str(), "Ae" | "Sp" | "Cx")),
            exotic_biomes: biomes.alien,
        },
        &mut LayerRng::new(seed as u64, Layer::Resources),
    );
    Layers { air, climate, hydrology, sea_level, geology, biomes, atmosphere, life, resources }
}

/// Aurores : un champ magnétique, de l'air (ou une géante) et le vent de l'étoile. Couleur selon
/// le gaz : vert de l'oxygène, violet de l'azote, rose de l'hydrogène, rouge du CO2.
fn aurora_of(l: &Layers, gaseous: bool, star: &StarPhysics, au: f64) -> Option<Aurora> {
    let mag = l.geology.magnetic_field as f64;
    if mag < 0.2 || !(l.atmosphere || gaseous) {
        return None;
    }
    let wind = (star.stellar_wind / (au * au)).max(0.0);
    let strength = (0.3 * mag.sqrt() * wind.powf(0.25)).clamp(0.0, 1.0) as f32;
    if strength < 0.1 {
        return None;
    }
    let main = l.air.gases.first().map_or("", |(f, _)| f.as_str());
    let color = if l.air.fraction("O2") > 0.05 {
        [0.3, 1.0, 0.5]
    } else {
        match main {
            "H2" => [1.0, 0.4, 0.65],
            "CO2" => [1.0, 0.45, 0.3],
            _ => [0.55, 0.45, 1.0],
        }
    };
    Some(Aurora { strength, color, latitude: (68.0 - 6.0 * strength) })
}

fn evaluate_habitability(l: &Layers, gaseous: bool, radius: f64, mass: f64) -> Habitability {
    let (equator, pole) = l.climate.range();
    habitability::evaluate(&HabInput {
        gaseous,
        mean_c: l.climate.mean_c,
        equator_c: equator,
        pole_c: pole,
        pressure: l.air.pressure_bar,
        gases: &l.air.gases,
        liquid_water: l.hydrology.hydro.liquid == Liquid::Water && l.hydrology.water_state == WaterState::Liquid,
        ocean_fraction: l.hydrology.ocean_fraction,
        radiation: l.biomes.radiation,
        gravity: (mass / (radius * radius)) as f32,
        volcanism: l.geology.volcanism,
        quakes: l.geology.quakes,
        wind_ms: l.air.wind_ms,
        lava: l.hydrology.hydro.liquid == Liquid::Lava,
        acid: l.air.clouds == super::atmosphere::CloudKind::Sulfuric,
    })
}

fn trait_context(l: &Layers, gaseous: bool, tilt: f32, locked: bool, ring: bool, aurora: bool) -> traits::TraitContext {
    traits::TraitContext {
        gaseous,
        atmosphere: l.atmosphere,
        volcanism: l.geology.volcanism,
        liquid_water: l.hydrology.hydro.liquid == Liquid::Water && l.hydrology.water_state == WaterState::Liquid,
        icy: l.hydrology.water_state == WaterState::Ice,
        magnetic: l.geology.magnetic_field,
        old_surface: l.geology.surface_age_gyr > 2.0,
        tilt,
        locked,
        ocean_fraction: l.hydrology.ocean_fraction,
        lava: l.hydrology.hydro.liquid == Liquid::Lava,
        ring,
        aurora,
    }
}

/// Anneaux d'une planète (distances depuis son centre, unités du jeu).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Ring {
    pub inner: f32,
    pub outer: f32,
    pub color: [f32; 3],
    pub opacity: f32,
    /// Part de glace (0 : roche sombre, 1 : glace claire comme Saturne) (C2).
    #[serde(default)]
    pub ice: f32,
    /// Divisions (comme celle de Cassini) : centre et largeur, en fraction de la largeur.
    #[serde(default)]
    pub gaps: [[f32; 2]; 3],
    #[serde(default)]
    pub seed: u32,
}

impl Ring {
    /// Opacité à la fraction `f` (0 : bord intérieur, 1 : extérieur) : bandes, divisions vides,
    /// bords adoucis. La même fonction dessine l'anneau, son ombre et ses particules.
    pub fn profile(&self, f: f32) -> f32 {
        if !(0.0..=1.0).contains(&f) {
            return 0.0;
        }
        let h = |i: u32| {
            let mut x = self.seed ^ i.wrapping_mul(0x9E37_79B9);
            x ^= x >> 16;
            x = x.wrapping_mul(0x7FEB_352D);
            x ^= x >> 15;
            x = x.wrapping_mul(0x846C_A68B);
            x ^= x >> 16;
            (x >> 8) as f32 / (1u32 << 24) as f32
        };
        // Bandes : bruit de valeur lissé à deux échelles
        let band = |cells: f32, salt: u32| {
            let x = f * cells;
            let (i, t) = (x.floor(), x.fract());
            let s = t * t * (3.0 - 2.0 * t);
            let a = h(i as u32 ^ salt);
            let b = h((i as u32 + 1) ^ salt);
            a + (b - a) * s
        };
        let mut v = 0.45 + 0.35 * band(9.0, 0x100) + 0.2 * band(41.0, 0x200);
        for [c, w] in self.gaps {
            if w > 0.0 {
                let d = ((f - c).abs() / (w * 0.5)).min(1.0);
                v *= d * d * (3.0 - 2.0 * d);
            }
        }
        let edge = (f / 0.04).min((1.0 - f) / 0.03).clamp(0.0, 1.0);
        (v * self.opacity * edge).clamp(0.0, 1.0)
    }
}

/// Aurores polaires : force (0..1), couleur, latitude (degrés).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Aurora {
    pub strength: f32,
    pub color: [f32; 3],
    pub latitude: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mass_radius_matches_the_solar_system() {
        // Terre, Mars, Neptune, Jupiter, Saturne (à ~15 % près)
        let close = |a: f64, b: f64| (a / b - 1.0).abs() < 0.15;
        assert!(close(radius_from_mass(PlanetKind::Rocky, 1.0), 1.0));
        assert!(close(radius_from_mass(PlanetKind::Rocky, 0.107), 0.53));
        assert!(close(radius_from_mass(PlanetKind::IceGiant, 17.1), 3.88));
        assert!(close(radius_from_mass(PlanetKind::GasGiant, 318.0), 11.2));
        assert!(radius_from_mass(PlanetKind::GasGiant, 95.0) > 8.0);
        // La Lune : ~0,012 M⊕, ~0,17 g
        let m = moon_mass(0.273, false);
        assert!((m - 0.0123).abs() < 0.003, "{m}");
        assert!((m / (0.273 * 0.273) - 0.165).abs() < 0.03);
    }

    #[test]
    fn tides_heat_io_not_the_moon() {
        // Io : 5,9 rayons physiques de Jupiter = 2,36 affichés
        let io = tidal_heating(318.0, 2.36, 0.01);
        assert!((0.4..0.9).contains(&io), "{io}");
        assert!(tidal_heating(318.0, 3.76, 0.009) < 0.1, "Europe");
        assert!(tidal_heating(1.0, 2.4, 0.03) < 0.05, "Lune");
    }

    #[test]
    fn display_keeps_order_and_puts_the_habitable_zone_at_3_scales() {
        assert!((display_distance(1.0, 1.0) - 3.2).abs() < 1e-9);
        let mut last = 0.0;
        for i in 0..60 {
            let a = 0.01 * 1.2f64.powi(i);
            let d = display_distance(a, 1.0);
            assert!(d >= last && d >= 1.25, "{a} -> {d}");
            last = d;
        }
        // Neptune (30 UA) reste à moins de 8 échelles
        assert!(display_distance(30.0, 1.0) < 8.0);
    }
}
