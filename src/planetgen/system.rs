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

use super::atmosphere::{self, AirInput};
use super::climate::Climate;
use super::genome::SystemGenome;
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

/// Rayon du Soleil en UA.
const SUN_RADIUS_AU: f64 = 0.004_65;

/// Position affichée (en échelles G) d'une orbite de `a` UA, `hz` : distance de la zone habitable.
pub fn display_distance(a: f64, hz: f64) -> f64 {
    let x = (a / hz).log2();
    if x >= -1.0 { 3.2 + 0.9 * x } else { 2.3 + 0.3 * (x + 1.0) }.max(1.25)
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
pub fn generate(genome: SystemGenome, star: &StarPhysics, scale: f32, star_radius: f32) -> Vec<PlanetConfig> {
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
    let mut drafts = Vec::with_capacity(n);
    for pi in 0..n {
        if pi > 0 {
            a *= orbit.range(1.4, 2.3);
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

    // ── Lunes, orbites affichées, physique ───────────────────────────────
    let mut planets: Vec<PlanetConfig> = Vec::with_capacity(n);
    for (pi, d) in drafts.iter().enumerate() {
        let pu = pi as u32;
        let seed = genome.planet_base.wrapping_add(pu);
        let mut phys = LayerRng::new(seed as u64, Layer::Physics);
        let _ = (phys.unit(), phys.unit()); // nature et masse, déjà tirées
        let mut relief = LayerRng::new(seed as u64, Layer::Relief);
        let mut spin = LayerRng::new(seed as u64, Layer::Orbit);
        let radius = q(d.radius * earth, 1.0).max(50.0);
        let gaseous = d.kind.gaseous();

        // Lunes : 0 à 2 pour une rocheuse, davantage pour une géante
        let moon_count = match d.kind {
            PlanetKind::Rocky if d.radius < 0.5 => spin.weighted(&[0.6, 0.4]),
            PlanetKind::Rocky | PlanetKind::MiniNeptune => spin.weighted(&[0.3, 0.45, 0.25]),
            PlanetKind::IceGiant => 1 + spin.weighted(&[0.3, 0.3, 0.25, 0.15]),
            PlanetKind::GasGiant => 2 + spin.weighted(&[0.35, 0.35, 0.3]),
        };
        let icy = d.au > snow;
        // Lunes sans air : température d'équilibre (albédo 0,12), grands écarts jour / nuit
        let moon_t = atmosphere::equilibrium_temperature(lum, d.au, 0.12) as f32;
        let moon_climate = Climate { mean_c: moon_t - 273.15, span: moon_t * 0.35, lapse: 0.0, diurnal: moon_t * 0.3, tilt: 0.0 };
        let mut moons = Vec::with_capacity(moon_count);
        for mi in 0..moon_count {
            let mu = mi as u32;
            let moon_seed = genome.planet_base.wrapping_add(500 + pu * 8 + mu);
            let mut m = LayerRng::new(moon_seed as u64, Layer::Physics);
            // Rocheuse : 1/8 à 1/3 de sa planète ; géante : de 0,08 à 0,45 R⊕ (Ganymède 0,41)
            let r_earth = if gaseous {
                m.range(0.08, 0.45).min(d.radius / 3.0)
            } else {
                d.radius * m.range(0.12, 0.33)
            };
            let moon_radius = q(r_earth * earth, 1.0).max(40.0);
            let spread = if gaseous { (1.8, 1.1, 0.4) } else { (2.4, 1.7, 0.5) };
            let orbit_distance = radius as f64 * (spread.0 + spread.1 * mi as f64 + spread.2 * m.unit()) + moon_radius as f64 * 3.0;
            let mass = moon_mass(r_earth, icy);
            moons.push(MoonConfig {
                orbit_distance: q(orbit_distance, 1.0),
                radius: moon_radius,
                seed: moon_seed,
                eccentricity: q(m.range(0.0, 0.03), 1e-4),
                inclination: q(m.range(-0.05, 0.05), 1e-4),
                ascending_node: q(m.range(0.0, std::f64::consts::TAU), 1e-4),
                arg_periapsis: q(m.range(0.0, std::f64::consts::TAU), 1e-4),
                mean_anomaly_0: q(m.range(0.0, std::f64::consts::TAU), 1e-4),
                radius_earth: r_earth as f32,
                mass_earth: mass as f32,
                gravity_g: (mass / (r_earth * r_earth)) as f32,
                climate: Some(moon_climate),
            });
        }

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

        // Atmosphère et climat (phase 3) : rétention, composition, serre, nuages, vents, ciel
        let (air, climate) = atmosphere::generate(
            &AirInput {
                kind: d.kind,
                mass: d.mass,
                radius: d.radius,
                au: d.au,
                luminosity: lum,
                xray: star.xray_flux,
                star_color: star.color,
                locked,
                rotation_h,
                axial_tilt,
            },
            &mut LayerRng::new(seed as u64, Layer::Atmosphere),
        );
        let atmosphere = !gaseous && air.present();

        planets.push(PlanetConfig {
            orbit_distance: 0.0, // placée plus bas
            radius,
            sea_level: if gaseous { 0.0 } else if atmosphere { q(0.2 + relief.unit() * 0.4, 1e-4) } else { q(relief.unit() * 0.1, 1e-4) },
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
            ..Default::default()
        });
    }

    // ── Orbites affichées : échelle logarithmique, puis écartées ─────────
    // `reach` : rayon de la planète et de ses lunes ; marge entre deux voisines
    let reach = |p: &PlanetConfig| -> f64 {
        p.moons.iter().map(|m| (m.orbit_distance + m.radius) as f64).fold(p.radius as f64, f64::max)
    };
    let margin = 0.08 * scale;
    let mut previous_apoapsis = star_radius as f64 * 1.3;
    let mut previous_reach = 0.0;
    for (p, d) in planets.iter_mut().zip(&drafts) {
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
        p.orbit_distance = q(distance, 10.0);
        previous_apoapsis = p.orbit_distance as f64 * (1.0 + e);
        previous_reach = r;
    }
    planets
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
