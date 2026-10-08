//! Comètes d'un système (C2 de `roadmaps/fait/ROADMAP-0.11.md`).
//!
//! Noyaux de glace sale sur des orbites très excentriques : la famille de Jupiter (aphélie près
//! d'une géante, quelques centaines d'heures de jeu par tour) et les comètes à longue période
//! (aphélie loin au-delà des planètes, orbites inclinées dans tous les sens). Elles ne s'activent
//! qu'en passant la ligne des glaces : la queue de gaz (droite, opposée à l'étoile) et celle de
//! poussière (courbée, en retard sur l'orbite) grandissent à mesure qu'elles approchent.
//!
//! Rien n'est stocké : `sys.comets()` les recalcule d'après la graine du système ; leur position
//! est une fonction de l'horloge (règle 9).

use super::belts::Belt;
use super::genome::SystemGenome;
use super::seeds::{Layer, LayerRng};
use super::star::StarPhysics;
use super::system::display_distance;
use crate::settings::{PlanetConfig, SPACE_STRETCH};

/// Une comète : orbite de Kepler (unités affichées, autour de l'étoile) et noyau.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Comet {
    pub seed: u32,
    /// Demi-grand axe, excentricité, inclinaison, nœud, argument du périastre, anomalie à t = 0.
    pub a: f64,
    pub e: f64,
    pub inc: f64,
    pub node: f64,
    pub peri: f64,
    pub m0: f64,
    /// Rayon du noyau (unités du jeu).
    pub radius: f32,
    /// Périhélie en UA (physique) et distance affichée où elle s'active (ligne des glaces).
    pub q_au: f32,
    pub active_at: f32,
    /// Longueur maximale de la queue (au périhélie), unités du jeu.
    pub tail: f32,
    /// Famille de Jupiter (courte période) ou longue période.
    pub short: bool,
}

impl Comet {
    pub fn perihelion(&self) -> f64 {
        self.a * (1.0 - self.e)
    }

    pub fn aphelion(&self) -> f64 {
        self.a * (1.0 + self.e)
    }

    pub fn name(&self) -> &'static str {
        if self.short { "comete de courte periode" } else { "comete a longue periode" }
    }

    /// Activité (0 : endormie, 1 : au périhélie) à la distance affichée `r` de l'étoile.
    pub fn activity(&self, r: f64) -> f32 {
        let q = self.perihelion().max(1.0);
        if r >= self.active_at as f64 {
            return 0.0;
        }
        // Rien à la ligne des glaces, tout au périhélie (le flux de l'étoile va en 1/r²)
        let x = ((self.active_at as f64 - r) / (self.active_at as f64 - q).max(1.0)).clamp(0.0, 1.0);
        (x * x * (q / r).min(1.0).powf(0.5)) as f32
    }
}

/// Comètes du système : 0 à 6, d'après la graine.
pub fn generate(genome: SystemGenome, star: &StarPhysics, scale: f32, star_radius: f32, planets: &[PlanetConfig], belts: &[Belt], limits: super::system::OrbitLimits) -> Vec<Comet> {
    let mut rng = LayerRng::new(genome.seed as u64 ^ 0x434F_4D45, Layer::Belts);
    let scale = scale as f64;
    let stretch = SPACE_STRETCH as f64;
    let hz = star.luminosity_sun.max(1e-7).sqrt();
    let to_display = |au: f64| display_distance(au, hz) * scale * stretch;
    let orbits: Vec<&PlanetConfig> = planets.iter().filter(|p| !p.rogue).collect();
    // Bord du système : la ceinture glacée, sinon la dernière planète
    let edge = belts
        .iter()
        .map(|b| b.outer as f64)
        .chain(orbits.iter().map(|p| (p.orbit_distance * (1.0 + p.eccentricity)) as f64))
        .fold(4.0 * scale * stretch, f64::max);
    let giant = orbits.iter().find(|p| p.kind.gaseous() && !p.hot).map(|p| p.orbit_distance as f64);
    let count = rng.weighted(&[0.15, 0.2, 0.2, 0.2, 0.15, 0.1]);
    let active_at = to_display(3.0 * hz);
    (0..count)
        .filter_map(|_| {
            let short = giant.is_some() && rng.unit() < 0.35;
            let q_au = hz * 10f64.powf(rng.range(-0.7, 0.4));
            let q = to_display(q_au).max(star_radius as f64 * 3.0);
            let big_q = match (short, giant) {
                (true, Some(g)) => g * rng.range(0.9, 1.3),
                _ => edge * rng.range(1.0, 2.5),
            }
            .max(q * 5.0)
            // Compagnon lointain (C3) : les comètes restent en deçà
            .min(limits.outer);
            let inc = if short { rng.range(0.0, 25f64.to_radians()) } else { (rng.range(-1.0, 1.0)).acos() };
            let u = rng.unit();
            let seed = rng.next_u64() as u32;
            let (node, peri, m0) = (rng.range(0.0, std::f64::consts::TAU), rng.range(0.0, std::f64::consts::TAU), rng.range(0.0, std::f64::consts::TAU));
            (big_q >= q * 5.0).then_some(Comet {
                seed,
                a: 0.5 * (q + big_q),
                e: (big_q - q) / (big_q + q),
                inc,
                node,
                peri,
                m0,
                // Noyaux de 2 à 25 unités (beaucoup de petits)
                radius: (2.0 * (12.5f64).powf(u * u)) as f32,
                q_au: q_au as f32,
                active_at: active_at as f32,
                tail: (active_at * 0.25) as f32,
                short,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::settings::{default_galaxies, default_systems};

    #[test]
    fn comets_are_eccentric_and_reach_the_inner_system() {
        let systems = default_systems(&default_galaxies(42), 42);
        let (mut n, mut short) = (0, 0);
        for sys in systems.dense().iter().take(600) {
            let comets = sys.comets();
            assert!(comets.len() <= 6);
            for c in &comets {
                n += 1;
                short += c.short as usize;
                assert!(c.e > 0.5 && c.e < 1.0, "{c:?}");
                assert!(c.perihelion() < c.active_at as f64, "elle s'active en approchant : {c:?}");
                assert!(c.radius >= 2.0 && c.radius <= 25.0);
                // Endormie loin de l'étoile, active au périhélie
                assert_eq!(c.activity(c.aphelion()), 0.0);
                assert!(c.activity(c.perihelion()) > 0.5);
            }
            assert_eq!(sys.comets(), comets, "reproductible");
        }
        // (0.13.6 : moins de comètes à longue période, celles qui sortaient de la zone d'influence de l'étoile)
        assert!(n > 450 && short > 50 && short < n / 2, "{n} cometes, {short} courtes");
    }

    #[test]
    fn trojans_follow_giants_and_rogues_are_rare_and_far() {
        let systems = default_systems(&default_galaxies(42), 42);
        let (mut swarms, mut rogues, mut n) = (0, 0, 0);
        for sys in systems.dense().iter().take(1500) {
            n += 1;
            let planets = sys.planets_uncached();
            for s in sys.swarms() {
                swarms += 1;
                let p = &planets[s.planet as usize];
                assert!(p.kind.gaseous() && !p.rogue);
            }
            if let Some(r) = planets.iter().find(|p| p.rogue) {
                rogues += 1;
                assert!(std::ptr::eq(r, planets.last().unwrap()), "la planete errante est la derniere");
                let far = planets.iter().filter(|p| !p.rogue).map(|p| p.orbit_distance * (1.0 + p.eccentricity)).fold(0.0, f32::max);
                assert!(r.orbit_distance > far * 2.0, "{} : {} vs {far}", sys.name, r.orbit_distance);
                assert!(r.climate.map_or(true, |c| c.mean_c < -150.0), "sans etoile, glacee");
            }
            assert_eq!(planets.iter().filter(|p| p.rogue).count() <= 1, true);
        }
        assert!(swarms > 300, "{swarms} essaims");
        let share = rogues as f64 / n as f64;
        assert!((0.01..0.07).contains(&share), "planetes errantes {share}");
    }
}
