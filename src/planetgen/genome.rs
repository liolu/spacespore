//! Génome d'un système : les quelques nombres dont on recalcule ses planètes et ses lunes.
//!
//! La liste des ~145 000 systèmes ne garde plus les planètes (règle 1) : chaque système garde son
//! génome (8 octets), et `StarSystemConfig::planets()` recalcule les planètes à la demande, avec
//! exactement les mêmes fonctions qu'avant (même monde, au bit près).

use crate::settings::{pseudo_rand, MoonConfig, PlanetConfig};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SystemGenome {
    /// Graine du système, déjà mélangée à la graine du monde.
    pub seed: u32,
    /// Base des graines de planètes, déjà mélangée à la graine du monde (loin de `u32::MAX`).
    pub planet_base: u32,
}

impl SystemGenome {
    pub fn planets(&self, star_radius: f32) -> Vec<PlanetConfig> {
        make_planets(self.seed, self.planet_base, star_radius)
    }
}

/// Planètes et lunes d'un système. Rien d'autre que + - * / : le résultat doit être identique sur
/// toutes les machines (empreinte du monde en multijoueur).
///
/// Proportions (R = rayon de l'étoile) : planète ≤ R/100, lune ≤ planète/3 ; première orbite à
/// 2,4 R, puis 1 à 1,6 R d'écart ; 1 à 3 planètes, 1 à 2 lunes chacune. Les planètes sont assez
/// petites pour tenir loin de l'étoile et rester explorables à pied (voir `surface.rs`).
fn make_planets(seed: u32, sb: u32, star_radius: f32) -> Vec<PlanetConfig> {
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
        let radius = star_radius / 100.0 * (0.7 + 0.25 * r(50));
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
}
