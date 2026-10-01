//! Vie (phase 7) : probabilité de vie microbienne, simple, complexe — indépendante de
//! l'habitabilité (un humain ne pourrait pas vivre sur Titan, des microbes peut-être) — et
//! paramètres de la faune (créatures visibles en 0.11).
//!
//! - La vie demande un liquide (eau en surface, océan sous la glace, méthane, ammoniac) et du
//!   temps : la vie simple après ~1 Gyr, la vie complexe après ~2,5 Gyr. La radiation au sol freine
//!   la vie de surface, pas celle des océans cachés.
//! - Plantes en surface seulement avec de la vie au moins simple, un liquide et de l'air : sinon les
//!   biomes « verts » restent nus (`biome.rs`).

use serde::{Deserialize, Serialize};

use super::hydrology::{Liquid, WaterState};
use super::seeds::LayerRng;

/// Niveau de vie.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum LifeLevel {
    #[default]
    None,
    Microbial,
    Simple,
    Complex,
}

impl LifeLevel {
    pub fn name(self) -> &'static str {
        match self {
            LifeLevel::None => "aucune",
            LifeLevel::Microbial => "microbienne",
            LifeLevel::Simple => "simple (tapis, algues, plantes)",
            LifeLevel::Complex => "complexe (animaux)",
        }
    }
}

/// Faune : paramètres seulement (créatures visibles en 0.11).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Fauna {
    /// Nombre d'espèces estimé.
    pub species: u32,
    /// Taille du plus grand animal (m) : plus petite sous une forte gravité ou sans oxygène.
    pub max_size_m: f32,
    pub locomotion: Vec<String>,
    pub diets: Vec<String>,
    /// Actifs surtout la nuit (monde brûlant ou très irradié).
    pub nocturnal: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Life {
    pub level: LifeLevel,
    /// Chances (0..1) de vie microbienne, simple, complexe.
    pub chance_microbial: f32,
    pub chance_simple: f32,
    pub chance_complex: f32,
    /// Chimie du vivant (réaliste, spéculative ou fictive).
    pub biochemistry: String,
    /// Des plantes couvrent les sols (biomes verts, décor végétal).
    pub flora: bool,
    pub fauna: Option<Fauna>,
}

/// Données d'entrée.
pub struct LifeInput {
    pub liquid: Liquid,
    pub water_state: WaterState,
    pub subsurface_ocean: bool,
    pub atmosphere: bool,
    pub pressure: f64,
    pub oxygen: f64,
    pub fictional_gas: bool,
    pub radiation: f64,
    pub age_gyr: f64,
    pub gravity: f64,
    pub ocean_fraction: f64,
    pub mean_c: f64,
}

/// Il faut du temps : rien avant `needed` Gyr, puis de plus en plus probable.
fn age_factor(age: f64, needed: f64) -> f64 {
    ((age - needed * 0.5) / needed).clamp(0.0, 1.0)
}

pub fn generate(input: &LifeInput, rng: &mut LayerRng) -> Life {
    let surface_water = input.liquid == Liquid::Water && input.water_state == WaterState::Liquid && input.atmosphere;
    let exotic = matches!(input.liquid, Liquid::Methane | Liquid::Ammonia) && input.atmosphere;
    // (microbes, simple, complexe) selon le milieu ; la surface subit la radiation
    let shield = (1.0 - input.radiation).clamp(0.0, 1.0);
    let (m, s, c, surface) = if surface_water {
        (0.6, 0.3, 0.12, true)
    } else if exotic {
        (0.2, 0.06, 0.015, true)
    } else if input.subsurface_ocean {
        (0.25, 0.05, 0.01, false)
    } else if input.atmosphere {
        (0.03, 0.0, 0.0, true)
    } else {
        (0.01, 0.0, 0.0, false)
    };
    let surf = if surface { shield } else { 1.0 };
    let chance_microbial = m * age_factor(input.age_gyr, 0.3) * surf.max(0.3);
    let chance_simple = (s * age_factor(input.age_gyr, 1.0) * surf).min(chance_microbial);
    let chance_complex = (c * age_factor(input.age_gyr, 2.5) * surf).min(chance_simple);
    let u = rng.unit();
    let level = if u < chance_complex {
        LifeLevel::Complex
    } else if u < chance_simple {
        LifeLevel::Simple
    } else if u < chance_microbial {
        LifeLevel::Microbial
    } else {
        LifeLevel::None
    };
    let biochemistry = if level == LifeLevel::None {
        String::new()
    } else if input.fictional_gas {
        "silicium et aetherion (fictif)".to_string()
    } else {
        match input.liquid {
            Liquid::Methane => "azotosomes dans le methane (speculatif)".to_string(),
            Liquid::Ammonia => "carbone dans l'ammoniac (speculatif)".to_string(),
            _ => "carbone et eau (realiste)".to_string(),
        }
    };
    let flora = level >= LifeLevel::Simple && surface;
    let fauna = (level == LifeLevel::Complex).then(|| {
        let g = input.gravity.clamp(0.1, 5.0);
        let oxygen = if input.oxygen > 0.1 { 1.0 } else { 0.3 };
        let mut locomotion = vec!["marche".to_string()];
        if input.pressure >= 0.5 && g < 1.5 {
            locomotion.push("vol".into());
        }
        if input.ocean_fraction > 0.1 {
            locomotion.push("nage".into());
        }
        if input.radiation > 0.2 || input.mean_c < -20.0 {
            locomotion.push("terriers".into());
        }
        Fauna {
            species: (10f64.powf(rng.range(3.0, 7.0)) * oxygen) as u32,
            max_size_m: (rng.range(5.0, 30.0) * oxygen / g.powf(0.7)) as f32,
            locomotion,
            diets: vec!["herbivores".into(), "predateurs".into(), "decomposeurs".into()],
            nocturnal: input.mean_c > 40.0 || input.radiation > 0.3,
        }
    });
    Life {
        level,
        chance_microbial: chance_microbial as f32,
        chance_simple: chance_simple as f32,
        chance_complex: chance_complex as f32,
        biochemistry,
        flora,
        fauna,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planetgen::seeds::Layer;

    fn earth() -> LifeInput {
        LifeInput {
            liquid: Liquid::Water,
            water_state: WaterState::Liquid,
            subsurface_ocean: false,
            atmosphere: true,
            pressure: 1.0,
            oxygen: 0.21,
            fictional_gas: false,
            radiation: 0.01,
            age_gyr: 4.6,
            gravity: 1.0,
            ocean_fraction: 0.7,
            mean_c: 15.0,
        }
    }

    fn count(input: &LifeInput) -> [usize; 4] {
        let mut out = [0; 4];
        for i in 0..20_000 {
            let life = generate(input, &mut LayerRng::new(i, Layer::Biology));
            out[life.level as usize] += 1;
            if life.flora {
                assert!(life.level >= LifeLevel::Simple);
            }
            if life.level != LifeLevel::Complex {
                assert!(life.fauna.is_none());
            }
        }
        out
    }

    #[test]
    fn water_worlds_often_live_young_and_dry_worlds_rarely() {
        let terra = count(&earth());
        assert!(terra[3] > 1_500 && terra[2] > 2_500 && terra[1] > 4_000, "{terra:?}");
        // Trop jeune pour la vie complexe
        let young = count(&LifeInput { age_gyr: 0.8, ..earth() });
        assert_eq!(young[3], 0, "{young:?}");
        // Monde sec et sans air : quelques microbes au plus
        let dead = count(&LifeInput { liquid: Liquid::None, water_state: WaterState::Absent, atmosphere: false, pressure: 0.0, ..earth() });
        assert!(dead[2] == 0 && dead[3] == 0 && dead[1] < 400, "{dead:?}");
    }

    #[test]
    fn life_is_independent_of_habitability() {
        // Titan : inhabitable pour un humain, mais des microbes du méthane restent possibles
        let titan = count(&LifeInput { liquid: Liquid::Methane, oxygen: 0.0, mean_c: -179.0, ..earth() });
        assert!(titan[1] + titan[2] + titan[3] > 1_000, "{titan:?}");
        // Europe : océan sous la glace, pas de plantes en surface
        let europa = LifeInput { liquid: Liquid::None, water_state: WaterState::Ice, subsurface_ocean: true, atmosphere: false, ..earth() };
        for i in 0..5_000 {
            let life = generate(&europa, &mut LayerRng::new(i, Layer::Biology));
            assert!(!life.flora);
        }
        // Forte radiation : peu de vie de surface
        let irradiated = count(&LifeInput { radiation: 0.9, ..earth() });
        assert!(irradiated[3] < 300, "{irradiated:?}");
    }

    #[test]
    fn big_animals_need_low_gravity_and_oxygen() {
        let mut light = 0.0;
        let mut heavy = 0.0;
        for i in 0..20_000u64 {
            if let Some(f) = generate(&LifeInput { gravity: 0.5, ..earth() }, &mut LayerRng::new(i, Layer::Biology)).fauna {
                light += f.max_size_m;
            }
            if let Some(f) = generate(&LifeInput { gravity: 2.5, ..earth() }, &mut LayerRng::new(i, Layer::Biology)).fauna {
                heavy += f.max_size_m;
            }
        }
        assert!(light > heavy * 2.0, "{light} {heavy}");
    }
}
