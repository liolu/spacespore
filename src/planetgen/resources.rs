//! Ressources (phase 8) : composition globale d'un astre et ses gisements de minerais réels et
//! fictifs (abondance, profondeur, distribution, rareté, difficulté d'extraction, quantité).
//!
//! - Composition : la densité dit la part du noyau de fer ; au-delà de la ligne des glaces, des
//!   glaces ; une géante est faite de gaz.
//! - Gisements : chaque minerai a ses conditions (fer et nickel du noyau, cuivre, or et uranium
//!   des fluides chauds de la tectonique et du volcanisme, titane des basaltes, hélium-3 du
//!   régolithe exposé au vent de l'étoile, deutérium des océans et des géantes...). Les minerais
//!   fictifs (Xenium, Aetherite, Chronite) sont rares et étiquetés (règle 5).
//! - Difficulté : profondeur, gravité, températures extrêmes, pression, dureté.
//!
//! Pas encore de minage (0.14), mais les données sont prêtes : la quantité extraite de chaque
//! minerai sera un delta de `world.json` (`BodyDelta::ores`), et le reste se lit toujours
//! « départ − extrait ».

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::profile::Realism;
use super::seeds::LayerRng;

/// Minerais connus.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Ore {
    Iron,
    Nickel,
    Copper,
    Aluminium,
    Titanium,
    Gold,
    Platinum,
    Uranium,
    RareEarths,
    Silicon,
    WaterIce,
    Helium3,
    Deuterium,
    Hydrocarbons,
    Xenium,
    Aetherite,
    Chronite,
}

impl Ore {
    pub const ALL: [Ore; 17] = [
        Ore::Iron,
        Ore::Nickel,
        Ore::Copper,
        Ore::Aluminium,
        Ore::Titanium,
        Ore::Gold,
        Ore::Platinum,
        Ore::Uranium,
        Ore::RareEarths,
        Ore::Silicon,
        Ore::WaterIce,
        Ore::Helium3,
        Ore::Deuterium,
        Ore::Hydrocarbons,
        Ore::Xenium,
        Ore::Aetherite,
        Ore::Chronite,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Ore::Iron => "fer",
            Ore::Nickel => "nickel",
            Ore::Copper => "cuivre",
            Ore::Aluminium => "aluminium",
            Ore::Titanium => "titane",
            Ore::Gold => "or",
            Ore::Platinum => "platine",
            Ore::Uranium => "uranium",
            Ore::RareEarths => "terres rares",
            Ore::Silicon => "silicium",
            Ore::WaterIce => "glace d'eau",
            Ore::Helium3 => "helium-3",
            Ore::Deuterium => "deuterium",
            Ore::Hydrocarbons => "hydrocarbures",
            Ore::Xenium => "xenium",
            Ore::Aetherite => "aetherite",
            Ore::Chronite => "chronite",
        }
    }

    pub fn realism(self) -> Realism {
        match self {
            Ore::Xenium | Ore::Aetherite | Ore::Chronite => Realism::Fictional,
            _ => Realism::Realistic,
        }
    }

    /// Bien de l'économie (rayon « Ressources ») : le minerai raffiné.
    pub fn good(self) -> crate::economy::GoodId {
        use crate::economy::{FIRST_ORE_GOOD as F, IRON};
        match self {
            Ore::Iron => IRON,
            Ore::Deuterium => 13,
            Ore::Nickel => F,
            Ore::Copper => F + 1,
            Ore::Aluminium => F + 2,
            Ore::Titanium => F + 3,
            Ore::Gold => F + 4,
            Ore::Platinum => F + 5,
            Ore::Uranium => F + 6,
            Ore::RareEarths => F + 7,
            Ore::Silicon => F + 8,
            Ore::WaterIce => F + 9,
            Ore::Helium3 => F + 10,
            Ore::Hydrocarbons => F + 11,
            Ore::Xenium => F + 12,
            Ore::Aetherite => F + 13,
            Ore::Chronite => F + 14,
        }
    }

    /// Valeur de base (crédits par tonne raffinée) : le prix de base du bien.
    pub fn value(self) -> f64 {
        crate::economy::GOODS[self.good()].2 as f64
    }

    /// Dureté de la roche hôte (0 : meuble, 1 : très dure).
    fn hardness(self) -> f64 {
        match self {
            Ore::WaterIce | Ore::Hydrocarbons | Ore::Helium3 | Ore::Deuterium => 0.1,
            Ore::Gold | Ore::Copper | Ore::Silicon | Ore::Aluminium => 0.4,
            Ore::Iron | Ore::Nickel | Ore::RareEarths | Ore::Uranium => 0.6,
            Ore::Titanium | Ore::Platinum => 0.8,
            Ore::Xenium | Ore::Aetherite | Ore::Chronite => 0.9,
        }
    }
}

/// Comment le minerai est réparti.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Distribution {
    /// Filons dans la roche (fluides chauds).
    Veins,
    /// Couches sédimentaires, nappes.
    Layers,
    /// Sables et graviers (alluvions, régolithe).
    Placers,
    /// Nodules au fond des mers ou dans les cratères.
    Nodules,
    /// Dans l'atmosphère ou l'océan (pompage).
    Fluid,
    /// Cristaux isolés.
    Crystals,
}

impl Distribution {
    pub fn name(self) -> &'static str {
        match self {
            Distribution::Veins => "filons",
            Distribution::Layers => "couches",
            Distribution::Placers => "placers (sables)",
            Distribution::Nodules => "nodules",
            Distribution::Fluid => "fluide (pompage)",
            Distribution::Crystals => "cristaux",
        }
    }
}

/// Un gisement.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Deposit {
    pub ore: Ore,
    /// Abondance relative (0 : traces, 1 : très riche).
    pub abundance: f32,
    /// Profondeur typique (m) ; 0 en surface.
    pub depth_m: f32,
    pub distribution: Distribution,
    /// Rareté dans la galaxie (0 : courant, 1 : exceptionnel).
    pub rarity: f32,
    /// Difficulté d'extraction (0 : facile, 1 : extrême).
    pub difficulty: f32,
    /// Quantité de départ exploitable (tonnes) : le minage (0.14) en retirera des deltas.
    pub amount_t: f64,
}

impl Deposit {
    /// Valeur indicative du gisement (crédits) : quantité × valeur, décotée par la difficulté.
    pub fn worth(&self) -> f64 {
        self.amount_t * self.ore.value() * (1.0 - 0.6 * self.difficulty as f64)
    }
}

/// Composition globale (fractions de masse) et gisements d'un astre.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Resources {
    /// « fer » (noyau), « silicates », « glaces », « gaz » : fractions de masse.
    pub bulk: BTreeMap<String, f32>,
    /// Gisements, du plus intéressant au moins intéressant.
    pub deposits: Vec<Deposit>,
}

impl Resources {
    pub fn deposit(&self, ore: Ore) -> Option<&Deposit> {
        self.deposits.iter().find(|d| d.ore == ore)
    }
}

/// Données d'entrée.
pub struct ResourceInput {
    pub gaseous: bool,
    pub mass: f64,
    pub radius: f64,
    pub density: f64,
    pub icy: bool,
    pub gravity: f64,
    pub mean_c: f64,
    pub pressure: f64,
    pub atmosphere: bool,
    pub volcanism: f64,
    pub plates: bool,
    pub surface_age: f64,
    pub craters: f64,
    pub liquid_water: bool,
    pub ocean_fraction: f64,
    pub methane_seas: bool,
    /// Vent de l'étoile reçu (Terre = 1) : implante l'hélium-3 dans le régolithe.
    pub stellar_wind: f64,
    pub fictional_gas: bool,
    pub exotic_biomes: bool,
}

/// Composition globale (fractions de masse).
pub fn bulk(input: &ResourceInput) -> BTreeMap<String, f32> {
    let mut out = BTreeMap::new();
    if input.gaseous {
        out.insert("gaz (H2, He)".into(), 0.85);
        out.insert("glaces".into(), 0.1);
        out.insert("roche et fer (coeur)".into(), 0.05);
        return out;
    }
    // Densité « décompressée » (la gravité tasse les grosses planètes) : Terre ~4,4, Mercure ~5,1,
    // Lune ~3,2 ; d'où la part du noyau de fer (Terre ~0,35, Mercure ~0,6, Lune ~0,02)
    let uncompressed = input.density / (1.0 + 0.25 * input.mass.max(0.0).sqrt());
    let iron = ((uncompressed - 3.3) / 3.2).clamp(0.02, 0.8);
    let ice = if input.icy { (0.45 * (1.0 - iron)).min(0.5) } else { 0.0 };
    out.insert("fer (noyau)".into(), iron as f32);
    out.insert("silicates".into(), (1.0 - iron - ice).max(0.0) as f32);
    if ice > 0.0 {
        out.insert("glaces".into(), ice as f32);
    }
    out
}

/// Gisements d'un astre.
pub fn generate(input: &ResourceInput, rng: &mut LayerRng) -> Resources {
    let bulk = bulk(input);
    let iron = *bulk.get("fer (noyau)").unwrap_or(&0.05) as f64;
    let mut candidates: Vec<(Ore, f64, f64, Distribution)> = Vec::new(); // (minerai, abondance, profondeur m, distribution)
    let mut add = |ore: Ore, abundance: f64, depth: f64, dist: Distribution| {
        if abundance > 0.02 {
            candidates.push((ore, abundance.min(1.0), depth, dist));
        }
    };
    if input.gaseous {
        add(Ore::Deuterium, 0.8, 0.0, Distribution::Fluid);
        add(Ore::Helium3, 0.3, 0.0, Distribution::Fluid);
        if input.fictional_gas {
            add(Ore::Aetherite, 0.2, 0.0, Distribution::Fluid);
        }
    } else {
        let hydro = (input.volcanism * 0.7 + if input.plates { 0.5 } else { 0.0 }).min(1.0);
        add(Ore::Iron, 0.3 + iron, 300.0 + 2_000.0 * (1.0 - iron), if input.surface_age > 2.0 { Distribution::Nodules } else { Distribution::Layers });
        add(Ore::Nickel, iron * 0.8, 1_500.0, Distribution::Nodules);
        add(Ore::Silicon, 0.6 * (1.0 - iron), 0.0, Distribution::Placers);
        add(Ore::Aluminium, 0.5 * (1.0 - iron) * if input.liquid_water { 1.2 } else { 0.6 }, 50.0, Distribution::Layers);
        add(Ore::Copper, 0.15 + 0.6 * hydro, 800.0, Distribution::Veins);
        add(Ore::Gold, 0.4 * hydro, 1_500.0, if input.liquid_water { Distribution::Placers } else { Distribution::Veins });
        add(Ore::Platinum, 0.3 * iron + 0.2 * input.craters, 2_500.0, Distribution::Veins);
        add(Ore::Titanium, 0.15 + 0.6 * input.volcanism, 400.0, Distribution::Placers);
        add(Ore::RareEarths, 0.5 * input.volcanism * if input.plates { 1.3 } else { 0.7 }, 900.0, Distribution::Veins);
        // L'uranium se concentre dans les granites : il faut des plaques, et il décroît avec l'âge
        add(Ore::Uranium, if input.plates { 0.5 } else { 0.08 } * (1.0 - input.surface_age / 15.0).max(0.2), 1_200.0, Distribution::Veins);
        if input.icy || input.ocean_fraction > 0.0 && input.mean_c < 0.0 {
            add(Ore::WaterIce, 0.9, 0.0, Distribution::Layers);
        }
        if input.liquid_water {
            add(Ore::Deuterium, 0.3 * input.ocean_fraction, 0.0, Distribution::Fluid);
        }
        if input.methane_seas {
            add(Ore::Hydrocarbons, 0.9, 0.0, Distribution::Fluid);
        }
        // Hélium-3 : régolithe sans air, exposé longtemps au vent de l'étoile (la Lune)
        if !input.atmosphere {
            add(Ore::Helium3, 0.25 * input.stellar_wind.min(4.0).sqrt() * (input.surface_age / 4.0).min(1.0), 3.0, Distribution::Placers);
        }
        // Fictifs : rares, sous conditions
        if input.craters > 0.4 && rng.unit() < 0.02 {
            add(Ore::Xenium, rng.range(0.1, 0.6), 50.0, Distribution::Nodules);
        }
        // L'aetherite cristallise là où l'air contient de l'aether ; ailleurs, très rarement
        let aether = if input.fictional_gas { 0.3 } else if input.exotic_biomes { 0.005 } else { 0.0 };
        if rng.unit() < aether {
            add(Ore::Aetherite, rng.range(0.2, 0.7), 10.0, Distribution::Crystals);
        }
        if rng.unit() < 0.004 {
            add(Ore::Chronite, rng.range(0.05, 0.3), 4_000.0, Distribution::Crystals);
        }
    }

    // Difficulté d'extraction : profondeur, gravité, chaleur ou froid extrêmes, pression, dureté
    let env = ((input.gravity - 1.0).max(0.0) * 0.3
        + ((input.mean_c - 40.0).max(0.0) / 400.0)
        + ((-input.mean_c - 60.0).max(0.0) / 300.0)
        + (input.pressure / 100.0).min(0.5))
    .min(0.6);
    let mut deposits: Vec<Deposit> = candidates
        .into_iter()
        .map(|(ore, abundance, depth, distribution)| {
            let jitter = rng.range(0.7, 1.3);
            let abundance = (abundance * jitter).clamp(0.02, 1.0);
            let depth = if input.gaseous { 0.0 } else { depth * rng.range(0.5, 1.6) };
            let difficulty = (0.15 * ore.hardness() + (depth / 5_000.0).min(0.5) + env + if input.gaseous { 0.3 } else { 0.0 }).clamp(0.02, 1.0);
            // Quantité : abondance × taille de la planète × rareté du minerai dans la croûte
            let crust = 1.0e9 * input.radius.max(0.05).powi(2) * input.mass.max(0.001).powf(0.2);
            let amount = crust * abundance * (50.0 / ore.value()).min(2.0);
            Deposit {
                ore,
                abundance: abundance as f32,
                depth_m: depth as f32,
                distribution,
                rarity: match ore.realism() {
                    Realism::Fictional => 0.85 + 0.15 * ore.value() / 9_000.0,
                    _ => 0.8 * (ore.value() / 1_500.0).powf(0.4).min(1.0),
                } as f32,
                difficulty: difficulty as f32,
                amount_t: amount.round(),
            }
        })
        .collect();
    deposits.sort_by(|a, b| b.worth().total_cmp(&a.worth()));
    Resources { bulk, deposits }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planetgen::seeds::Layer;

    fn earth() -> ResourceInput {
        ResourceInput {
            gaseous: false,
            mass: 1.0,
            radius: 1.0,
            density: 5.51,
            icy: false,
            gravity: 1.0,
            mean_c: 15.0,
            pressure: 1.0,
            atmosphere: true,
            volcanism: 0.5,
            plates: true,
            surface_age: 0.3,
            craters: 0.0,
            liquid_water: true,
            ocean_fraction: 0.7,
            methane_seas: false,
            stellar_wind: 1.0,
            fictional_gas: false,
            exotic_biomes: false,
        }
    }

    #[test]
    fn earth_has_an_iron_core_and_common_ores() {
        let b = bulk(&earth());
        let iron = b["fer (noyau)"];
        assert!((0.2..0.45).contains(&iron), "{iron}");
        let r = generate(&earth(), &mut LayerRng::new(1, Layer::Resources));
        for ore in [Ore::Iron, Ore::Copper, Ore::Gold, Ore::Uranium, Ore::Silicon] {
            assert!(r.deposit(ore).is_some(), "{ore:?} absent : {:?}", r.deposits.iter().map(|d| d.ore).collect::<Vec<_>>());
        }
        assert!(r.deposit(Ore::Helium3).is_none(), "pas d'helium-3 sous une atmosphere");
        for d in &r.deposits {
            assert!(d.amount_t > 0.0 && (0.0..=1.0).contains(&d.difficulty) && (0.0..=1.0).contains(&d.abundance));
        }
    }

    #[test]
    fn the_moon_has_helium3_and_giants_are_gas() {
        let moon = ResourceInput { mass: 0.0123, radius: 0.273, density: 3.34, atmosphere: false, pressure: 0.0, plates: false, volcanism: 0.0, surface_age: 4.4, craters: 0.9, liquid_water: false, ocean_fraction: 0.0, gravity: 0.165, mean_c: -20.0, ..earth() };
        let r = generate(&moon, &mut LayerRng::new(2, Layer::Resources));
        assert!(r.deposit(Ore::Helium3).is_some());
        assert!(r.deposit(Ore::Uranium).map_or(true, |d| d.abundance < 0.2), "peu d'uranium sans plaques");
        let jupiter = ResourceInput { gaseous: true, mass: 318.0, radius: 11.2, density: 1.33, ..earth() };
        let g = generate(&jupiter, &mut LayerRng::new(3, Layer::Resources));
        assert_eq!(g.deposits[0].distribution, Distribution::Fluid);
        assert!(g.deposit(Ore::Gold).is_none());
        assert!(bulk(&jupiter)["gaz (H2, He)"] > 0.8);
    }

    #[test]
    fn harsh_worlds_are_harder_to_mine() {
        let easy = generate(&earth(), &mut LayerRng::new(4, Layer::Resources));
        let hell = generate(&ResourceInput { mean_c: 460.0, pressure: 92.0, gravity: 2.5, ..earth() }, &mut LayerRng::new(4, Layer::Resources));
        let d = |r: &Resources| r.deposit(Ore::Iron).unwrap().difficulty;
        assert!(d(&hell) > d(&easy) + 0.3);
    }

    #[test]
    fn fictional_ores_are_rare_and_labelled() {
        let mut fictional = 0;
        let n = 5_000;
        for i in 0..n {
            let r = generate(&ResourceInput { craters: 0.6, ..earth() }, &mut LayerRng::new(i, Layer::Resources));
            for d in &r.deposits {
                if d.ore.realism() == Realism::Fictional {
                    fictional += 1;
                    assert!(d.rarity > 0.8);
                }
            }
        }
        assert!(fictional > 30 && fictional < n / 10, "{fictional}");
    }
}
