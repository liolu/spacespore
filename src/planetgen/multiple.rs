//! Étoiles doubles et triples (C3 de `ROADMAP-0.11.md`).
//!
//! Environ un système sur trois (plus souvent autour des étoiles massives) :
//! - **double serrée** : deux étoiles proches tournent autour de leur centre de masse, les planètes
//!   tournent autour des deux (orbites de type P), au-delà de ~3 fois leur écart (stabilité) ;
//! - **double large** : un compagnon lointain ; les planètes tournent autour de l'étoile principale
//!   (type S), en deçà du quart de l'écart au plus près ;
//! - **triple** : une paire serrée au centre et un compagnon lointain (les deux contraintes).
//!
//! Les planètes voient la lumière des étoiles proches (luminosités additionnées : zone habitable
//! plus lointaine). Tout se recalcule depuis la graine du système et l'étoile principale.

use serde::{Deserialize, Serialize};

use super::seeds::{Layer, LayerRng};
use super::star::{StarClass, StarPhysics};
use super::system::display_distance;
use crate::settings::SPACE_STRETCH;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Multiplicity {
    #[default]
    Single,
    Close,
    Wide,
    Triple,
}

impl Multiplicity {
    pub fn name(self) -> &'static str {
        match self {
            Multiplicity::Single => "etoile simple",
            Multiplicity::Close => "etoile double serree (planetes autour des deux)",
            Multiplicity::Wide => "etoile double large (planetes autour d'une seule)",
            Multiplicity::Triple => "etoile triple",
        }
    }
}

/// Orbite d'une étoile (unités affichées) : position = `factor` × orbite de Kepler relative.
/// Paire au centre : les deux étoiles ont la même orbite relative, de part et d'autre du centre
/// de masse (`factor` < 0 pour la principale) ; compagnon lointain : `factor` = 1.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StarOrbit {
    pub a: f32,
    pub e: f32,
    pub i: f32,
    pub node: f32,
    pub peri: f32,
    pub m0: f32,
    pub factor: f32,
    /// Période réelle (secondes de jeu : 1 jour de la planète = 24 min) ; 0 = Kepler avec les
    /// distances affichées (rapprochées par l'échelle log, elles donneraient une fausse période).
    #[serde(default)]
    pub period: f32,
}

impl StarOrbit {
    pub fn elements(&self) -> crate::asteroids::Elements {
        crate::asteroids::Elements {
            a: self.a as f64,
            e: self.e as f64,
            i: self.i as f64,
            node: self.node as f64,
            peri: self.peri as f64,
            m0: self.m0 as f64,
            frozen: false,
        }
    }

    /// Position depuis le centre du système à l'instant `t`.
    pub fn position(&self, t: f64) -> bevy::math::DVec3 {
        if self.period > 0.0 {
            let lead = (std::f64::consts::TAU * t / self.period as f64).rem_euclid(std::f64::consts::TAU);
            return self.elements().position(0.0, lead) * self.factor as f64;
        }
        self.elements().position(t, 0.0) * self.factor as f64
    }

    /// Distance maximale au centre.
    pub fn reach(&self) -> f32 {
        self.a * (1.0 + self.e) * self.factor.abs()
    }
}

/// Un compagnon : sa physique et son orbite.
#[derive(Clone, Debug)]
pub struct Companion {
    pub physics: StarPhysics,
    pub orbit: StarOrbit,
}

/// Les étoiles d'un système et ce qu'elles imposent aux planètes.
#[derive(Clone, Debug, Default)]
pub struct Stellar {
    pub kind: Multiplicity,
    /// Orbite de l'étoile principale (paire serrée au centre), sinon immobile au centre.
    pub primary: Option<StarOrbit>,
    pub companions: Vec<Companion>,
    /// Luminosité reçue par les planètes (L☉) et masse autour de laquelle elles tournent (M☉) :
    /// étoiles du centre additionnées.
    pub luminosity: f64,
    pub mass: f64,
    /// Planètes : au-delà de `min_au` et du rayon affiché `exclusion`, en deçà de `max_au`.
    pub min_au: f64,
    pub max_au: f64,
    pub exclusion: f64,
    /// Rien du système (ceintures, comètes) ne s'approche plus que ça du compagnon lointain.
    pub outer_limit: f64,
}

impl Stellar {
    pub fn limits(&self) -> super::system::OrbitLimits {
        super::system::OrbitLimits { min_au: self.min_au, max_au: self.max_au, exclusion: self.exclusion, outer: self.outer_limit }
    }
}

/// Étoile de la séquence principale (ou naine brune) de masse `m` (M☉).
/// Un jour de la planète en secondes de jeu (1 h = 1 min, A1).
const DAY_SECS: f64 = 24.0 * 60.0;

pub fn star_of_mass(seed: u64, m: f64) -> StarPhysics {
    if m < 0.075 {
        return StarPhysics::generate(seed, ((m - 0.013) / 0.062).clamp(0.0, 1.0), Some(StarClass::BrownDwarf));
    }
    for class in [StarClass::M, StarClass::K, StarClass::G, StarClass::F, StarClass::A, StarClass::B, StarClass::O] {
        if let Some((lo, hi, _)) = class.main_sequence() {
            if m < hi || class == StarClass::O {
                let u = ((m.max(lo) / lo).ln() / (hi / lo).ln()).clamp(0.0, 1.0);
                return StarPhysics::generate(seed, u, Some(class));
            }
        }
    }
    StarPhysics::generate(seed, 0.5, Some(StarClass::M))
}

/// Part des étoiles qui ont au moins un compagnon, selon le type (les massives plus souvent).
fn multiple_chance(class: StarClass) -> f64 {
    match class {
        StarClass::O | StarClass::B => 0.7,
        StarClass::A => 0.5,
        StarClass::F | StarClass::G => 0.45,
        StarClass::K => 0.35,
        StarClass::M => 0.25,
        StarClass::WhiteDwarf => 0.2,
        StarClass::BrownDwarf => 0.15,
        _ => 0.3,
    }
}

/// Étoiles du système de graine `seed` autour de l'étoile principale `primary` (rayon affiché
/// `primary_radius`, échelle G `g_radius`).
pub fn generate(seed: u64, primary: &StarPhysics, g_radius: f64, primary_radius: f64) -> Stellar {
    let mut rng = LayerRng::new(seed ^ 0x4D55_4C54, Layer::Star);
    // Tous les tirages sont faits : un système ne change pas parce qu'un autre tirage a changé
    let roll = rng.unit();
    let kind_roll = rng.unit();
    let (q_close, q_wide) = (rng.range(0.25, 1.0), rng.range(0.1, 1.0));
    // Période de la paire serrée : 1 à 200 jours (écart d'après Kepler et la masse, plus bas)
    let close_days = 10f64.powf(rng.range(0.0, 2.3));
    let close_e = rng.range(0.0, 0.15);
    let wide_au = 10f64.powf(rng.range(1.7, 3.3));
    let wide_e = rng.range(0.1, 0.6);
    let angles = [rng.range(-0.08, 0.08), rng.range(0.0, std::f64::consts::TAU), rng.range(0.0, std::f64::consts::TAU), rng.range(0.0, std::f64::consts::TAU)];
    let wide_angles = [rng.range(-0.6, 0.6), rng.range(0.0, std::f64::consts::TAU), rng.range(0.0, std::f64::consts::TAU), rng.range(0.0, std::f64::consts::TAU)];
    let (seed_close, seed_wide) = (rng.next_u64(), rng.next_u64());

    let kind = if roll >= multiple_chance(primary.class) {
        Multiplicity::Single
    } else if kind_roll < 0.35 {
        Multiplicity::Close
    } else if kind_roll < 0.8 {
        Multiplicity::Wide
    } else {
        Multiplicity::Triple
    };
    let mut out = Stellar { kind, luminosity: primary.luminosity_sun, mass: primary.mass_sun, min_au: 0.0, max_au: f64::INFINITY, exclusion: 0.0, outer_limit: f64::INFINITY, ..Default::default() };
    let stretch = SPACE_STRETCH as f64;
    let m1 = primary.mass_sun.max(0.013);

    // ── Paire serrée au centre (planètes autour des deux) ──
    if matches!(kind, Multiplicity::Close | Multiplicity::Triple) {
        let b = star_of_mass(seed_close, (m1 * q_close).max(0.013));
        let close_au = ((close_days / 365.25).powi(2) * (m1 + b.mass_sun)).cbrt();
        let r2 = b.render_radius(g_radius) as f64;
        out.luminosity += b.luminosity_sun;
        out.mass += b.mass_sun;
        let hz = out.luminosity.max(1e-7).sqrt();
        // Une paire serrée est serrée : un quart de la première orbite permise aux planètes
        // (3 fois l'écart réel), sans que les deux étoiles se touchent. (L'échelle log des planètes
        // l'aurait mise aussi loin qu'une planète intérieure.)
        let first_orbit = display_distance(3.0 * close_au * (1.0 + close_e), hz) * g_radius * stretch;
        let sep = (first_orbit * 0.25).max((primary_radius + r2) * 1.6);
        let m2 = b.mass_sun;
        let (f1, f2) = (m2 / (m1 + m2), m1 / (m1 + m2));
        let orbit = |factor: f64| StarOrbit {
            a: sep as f32,
            e: close_e as f32,
            i: angles[0] as f32,
            node: angles[1] as f32,
            peri: angles[2] as f32,
            m0: angles[3] as f32,
            factor: factor as f32,
            // Sa vraie période : on la voit tourner (1 à 200 jours = 24 min à 80 h de jeu)
            period: (close_days * DAY_SECS) as f32,
        };
        out.primary = Some(orbit(-f1));
        out.companions.push(Companion { physics: b, orbit: orbit(f2) });
        // Stabilité (Holman et Wiegert) : à plus de ~3 fois l'écart ; et jamais dans les étoiles
        out.min_au = 3.0 * close_au * (1.0 + close_e);
        out.exclusion = sep * (1.0 + close_e) * f1.max(f2) + primary_radius.max(r2);
        out.exclusion *= 2.0;
    }

    // ── Compagnon lointain (planètes autour du centre seulement) ──
    if matches!(kind, Multiplicity::Wide | Multiplicity::Triple) {
        let c = star_of_mass(seed_wide, (m1 * q_wide).max(0.013));
        let hz = out.luminosity.max(1e-7).sqrt();
        // Triple : le compagnon reste assez loin pour laisser de la place autour de la paire
        let wide_au = wide_au.max(out.min_au * 5.0 / (0.25 * (1.0 - wide_e)));
        out.max_au = 0.25 * wide_au * (1.0 - wide_e);
        let limit_d = display_distance(out.max_au, hz) * g_radius * stretch;
        // Au plus près, bien au-delà de tout ce qui tourne autour du centre
        // Sa distance (même échelle que les planètes), assez loin pour ne pas troubler leurs orbites
        // (les orbites affichées, excentricité et lunes comprises, vont jusqu'à deux fois la limite)
        let a = (display_distance(wide_au, hz) * g_radius * stretch).max(limit_d * 4.0 / (1.0 - wide_e));
        let years = (wide_au.powi(3) / (out.mass + c.mass_sun)).sqrt();
        out.outer_limit = a * (1.0 - wide_e) * 0.5;
        out.companions.push(Companion {
            physics: c,
            orbit: StarOrbit {
                a: a as f32,
                e: wide_e as f32,
                i: wide_angles[0] as f32,
                node: wide_angles[1] as f32,
                peri: wide_angles[2] as f32,
                m0: wide_angles[3] as f32,
                factor: 1.0,
                period: (years * 365.25 * DAY_SECS) as f32,
            },
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use crate::settings::{default_galaxies, default_systems};

    #[test]
    fn about_a_third_of_systems_are_multiple_and_planets_stay_stable() {
        let systems = default_systems(&default_galaxies(42), 42);
        let (mut n, mut multiple) = (0, 0);
        let mut kinds = std::collections::HashMap::new();
        for sys in systems.dense().iter().take(3000) {
            n += 1;
            let Some(st) = sys.stellar() else { continue };
            *kinds.entry(st.kind).or_insert(0usize) += 1;
            if st.companions.is_empty() {
                continue;
            }
            multiple += 1;
            assert_eq!(sys.stars.len(), 1 + st.companions.len(), "{}", sys.name);
            let planets = sys.planets_uncached();
            for p in planets.iter().filter(|p| !p.rogue) {
                let au = p.semi_major_au as f64;
                assert!(au >= st.min_au * 0.999 && au <= st.max_au * 1.001, "{} : planete a {au} UA, {}..{}", sys.name, st.min_au, st.max_au);
                // Jamais dans les étoiles du centre
                assert!(p.orbit_distance * (1.0 - p.eccentricity) - p.radius > st.exclusion as f32 * 0.999, "{}", sys.name);
            }
            // Le compagnon lointain passe au large de tout ce qui tourne autour du centre
            if let Some(far) = st.companions.iter().find(|c| c.orbit.factor == 1.0) {
                let peri = far.orbit.a * (1.0 - far.orbit.e);
                let extent = planets.iter().filter(|p| !p.rogue).map(|p| p.orbit_distance * (1.0 + p.eccentricity)).fold(0.0, f32::max);
                let belts = sys.belts().iter().map(|b| b.outer).fold(0.0, f32::max);
                assert!(peri > extent.max(belts) * 1.5, "{} : compagnon a {peri}, planetes jusqu'a {extent}, ceintures {belts}", sys.name);
            }
            // Zone habitable recalculée : luminosité des étoiles du centre additionnées
            // Paire serrée : elle tourne vraiment (24 min à 80 h par tour) et reste bien plus près que
            // les planètes
            if let Some(o) = st.primary {
                assert!(o.period > 0.0 && o.period <= 200.0 * 1440.0 * 1.01, "{} : periode {}", sys.name, o.period);
                let quarter = o.position(0.0).distance(o.position(o.period as f64 / 4.0));
                assert!(quarter > 0.3 * (o.a * o.factor.abs()) as f64, "{} : immobile", sys.name);
                if let Some(first) = planets.iter().filter(|p| !p.rogue).map(|p| p.orbit_distance * (1.0 - p.eccentricity)).reduce(f32::min) {
                    assert!(o.a * 2.0 < first, "{} : paire a {}, premiere planete a {first}", sys.name, o.a);
                }
            }
            let l = sys.lighting().unwrap().luminosity_sun;
            assert!(l >= sys.star_physics().unwrap().luminosity_sun);
        }
        let share = multiple as f64 / n as f64;
        assert!((0.22..0.45).contains(&share), "systemes multiples {share} {kinds:?}");
        assert_eq!(kinds.len(), 4, "{kinds:?}");
    }
}
