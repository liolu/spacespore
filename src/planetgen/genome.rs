//! Génome d'un système : les quelques nombres dont on recalcule ses planètes et ses lunes.
//!
//! La liste des ~145 000 systèmes ne garde plus les planètes (règle 1) : chaque système garde son
//! génome (8 octets), et `StarSystemConfig::planets()` recalcule les planètes à la demande
//! (`planetgen::system`).

use super::star::StarPhysics;
use crate::settings::PlanetConfig;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SystemGenome {
    /// Graine du système, déjà mélangée à la graine du monde.
    pub seed: u32,
    /// Base des graines de planètes, déjà mélangée à la graine du monde (loin de `u32::MAX`).
    pub planet_base: u32,
}

impl SystemGenome {
    /// Planètes et lunes autour de l'étoile `star` (voir `planetgen::system`).
    pub fn planets(&self, star: &StarPhysics, scale: f32, star_radius: f32, limits: super::system::OrbitLimits) -> Vec<PlanetConfig> {
        super::system::generate(*self, star, scale, star_radius, limits)
    }
}
