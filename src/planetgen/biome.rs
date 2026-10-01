//! Sols et biomes (phase 6).
//!
//! - Sols : sable, argile, régolithe, volcanique (basalte), glace, sel, métal (rouille), d'après la
//!   géologie (volcanisme, densité, âge) et le climat (eau, sécheresse, froid).
//! - Humidité locale : celle de la planète (océans, pluie), modulée par les grandes ceintures
//!   (équateur humide, déserts vers 25-30°, latitudes moyennes humides, pôles secs) et du bruit.
//! - Radiation au sol : UV et rayons X de l'étoile, arrêtés par l'atmosphère et le champ magnétique.
//! - Biomes : température × humidité × altitude × sol × radiation. Terrestres si l'air contient de
//!   l'oxygène, extraterrestres sinon (forêt de cristal, plaines de spores, jungle fongique...).
//!   Chaque biome a sa matière voxel : la couleur de la planète vue de l'espace est celle de ses
//!   biomes, comme au sol (même fonction pour `terrain.rs` et `mesher.rs`).

use bevy::math::Vec3;
use noise::{NoiseFn, Perlin};
use serde::{Deserialize, Serialize};

use super::climate::{land_material, relative_altitude, Climate, CO2_FROST_C, DESERT_C, FREEZE_C, SCORCH_C};
use super::hydrology::{Hydro, Liquid};
use super::profile::Realism;
use crate::planet::VoxelType;

/// Sol dominant d'une région.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Soil {
    Sand,
    Clay,
    Regolith,
    Volcanic,
    Ice,
    Salt,
    Metal,
}

impl Soil {
    pub fn name(self) -> &'static str {
        match self {
            Soil::Sand => "sable",
            Soil::Clay => "argile",
            Soil::Regolith => "regolithe",
            Soil::Volcanic => "volcanique (basalte)",
            Soil::Ice => "glace",
            Soil::Salt => "sel",
            Soil::Metal => "metallique (rouille)",
        }
    }
}

/// Un biome : son nom, sa rigueur (règle 5) et sa matière voxel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Biome {
    // ── Terrestres ──
    IceSheet,
    Tundra,
    Taiga,
    Forest,
    Grassland,
    Steppe,
    Savanna,
    Jungle,
    Swamp,
    Desert,
    Beach,
    Alpine,
    // ── Minéraux (sans vie) ──
    Regolith,
    BasaltField,
    SaltFlat,
    RustPlain,
    // ── Extraterrestres ──
    CrystalForest,
    SporePlain,
    FungalJungle,
    GlassDesert,
    SulfurMarsh,
}

impl Biome {
    pub const ALL: [Biome; 21] = [
        Biome::IceSheet,
        Biome::Tundra,
        Biome::Taiga,
        Biome::Forest,
        Biome::Grassland,
        Biome::Steppe,
        Biome::Savanna,
        Biome::Jungle,
        Biome::Swamp,
        Biome::Desert,
        Biome::Beach,
        Biome::Alpine,
        Biome::Regolith,
        Biome::BasaltField,
        Biome::SaltFlat,
        Biome::RustPlain,
        Biome::CrystalForest,
        Biome::SporePlain,
        Biome::FungalJungle,
        Biome::GlassDesert,
        Biome::SulfurMarsh,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Biome::IceSheet => "inlandsis",
            Biome::Tundra => "toundra",
            Biome::Taiga => "taiga",
            Biome::Forest => "foret temperee",
            Biome::Grassland => "prairie",
            Biome::Steppe => "steppe",
            Biome::Savanna => "savane",
            Biome::Jungle => "foret tropicale",
            Biome::Swamp => "marais",
            Biome::Desert => "desert",
            Biome::Beach => "plage",
            Biome::Alpine => "haute montagne",
            Biome::Regolith => "plaine de regolithe",
            Biome::BasaltField => "champ de basalte",
            Biome::SaltFlat => "desert de sel",
            Biome::RustPlain => "plaine de rouille",
            Biome::CrystalForest => "foret de cristal",
            Biome::SporePlain => "plaine de spores",
            Biome::FungalJungle => "jungle fongique",
            Biome::GlassDesert => "desert de verre",
            Biome::SulfurMarsh => "marais de soufre",
        }
    }

    pub fn realism(self) -> Realism {
        match self {
            Biome::CrystalForest | Biome::SporePlain | Biome::FungalJungle => Realism::Fictional,
            Biome::GlassDesert | Biome::SulfurMarsh => Realism::Speculative,
            _ => Realism::Realistic,
        }
    }

    /// Végétation (terrestre ou extraterrestre) : la phase 7 y posera le décor.
    pub fn vegetated(self) -> bool {
        matches!(
            self,
            Biome::Tundra | Biome::Taiga | Biome::Forest | Biome::Grassland | Biome::Steppe | Biome::Savanna | Biome::Jungle | Biome::Swamp
                | Biome::CrystalForest | Biome::SporePlain | Biome::FungalJungle
        )
    }

    pub fn voxel(self) -> VoxelType {
        match self {
            Biome::IceSheet => VoxelType::Snow,
            Biome::Tundra => VoxelType::Tundra,
            Biome::Taiga => VoxelType::Taiga,
            Biome::Forest => VoxelType::Forest,
            Biome::Grassland => VoxelType::Grass,
            Biome::Steppe => VoxelType::Steppe,
            Biome::Savanna => VoxelType::Savanna,
            Biome::Jungle => VoxelType::Jungle,
            Biome::Swamp => VoxelType::Swamp,
            Biome::Desert | Biome::Beach => VoxelType::Sand,
            Biome::Alpine | Biome::Regolith => VoxelType::Stone,
            Biome::BasaltField => VoxelType::Basalt,
            Biome::SaltFlat => VoxelType::Salt,
            Biome::RustPlain => VoxelType::Rust,
            Biome::CrystalForest => VoxelType::Crystal,
            Biome::SporePlain => VoxelType::Spore,
            Biome::FungalJungle => VoxelType::Fungus,
            Biome::GlassDesert => VoxelType::Glass,
            Biome::SulfurMarsh => VoxelType::Sulfur,
        }
    }
}

/// Ce qu'il faut savoir d'une planète pour classer ses biomes (léger, copié dans `BodyParams`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BiomeParams {
    /// `false` : planète faite à la main ou lune, anciennes règles (`climate::land_material`).
    pub defined: bool,
    pub seed: u32,
    /// Humidité moyenne (0 : aride, 1 : détrempée).
    pub wetness: f32,
    /// Radiation au sol (0 : abritée comme la Terre, 1 : grillée).
    pub radiation: f32,
    /// Végétation extraterrestre (pas d'oxygène dans l'air).
    pub alien: bool,
    pub volcanism: f32,
    /// Croûte riche en fer (planète dense) : plaines de rouille.
    pub metal: bool,
    /// Air soufré : marais de soufre.
    pub sulfur: bool,
    /// Vieille surface sans air : régolithe.
    pub regolith: bool,
    /// Des mers se sont évaporées : déserts de sel dans les bassins.
    pub salt: bool,
}

/// Données d'entrée.
pub struct BiomeInput {
    pub seed: u32,
    pub ocean_fraction: f64,
    pub liquid_water: bool,
    pub cloud_cover: f64,
    pub pressure: f64,
    pub oxygen: f64,
    pub sulfur: f64,
    pub volcanism: f64,
    pub density: f64,
    pub surface_age: f64,
    pub magnetic_field: f64,
    /// De l'eau, mais plus liquide en surface (évaporée ou partie).
    pub dried_water: bool,
    /// UV et rayons X reçus (Terre = 1).
    pub uv: f64,
    pub xray: f64,
}

/// Radiation au sol (0..1). Les UV traversent l'air (l'ozone les filtre s'il y a de l'oxygène),
/// les rayons X sont arrêtés dès quelques dixièmes de bar, les particules des éruptions sont
/// déviées par le champ magnétique et freinées par l'air.
pub fn surface_radiation(uv: f64, xray: f64, pressure: f64, magnetic: f64, oxygen: f64) -> f64 {
    let p = pressure.max(0.0);
    let ozone = if oxygen > 0.05 { 0.1 } else { 1.0 };
    let uv_ground = uv * (-p * 2.5).exp() * ozone;
    let xray_ground = xray * (-p * 50.0).exp();
    let particles = xray.max(0.0).sqrt() * (-p * 5.0).exp() / (1.0 + 3.0 * magnetic.max(0.0));
    let dose = uv_ground + xray_ground + particles;
    (dose / (dose + 5.0)).clamp(0.0, 1.0)
}

pub fn generate(input: &BiomeInput) -> BiomeParams {
    let wetness = if input.liquid_water {
        (0.25 + 0.5 * input.ocean_fraction + 0.4 * input.cloud_cover).clamp(0.05, 1.0)
    } else {
        0.05
    };
    BiomeParams {
        defined: true,
        seed: input.seed,
        wetness: wetness as f32,
        radiation: surface_radiation(input.uv, input.xray, input.pressure, input.magnetic_field, input.oxygen) as f32,
        alien: input.oxygen < 0.05,
        volcanism: input.volcanism as f32,
        metal: input.density > 6.2,
        sulfur: input.sulfur > 0.001,
        regolith: input.pressure < 0.01 && input.surface_age > 1.0,
        salt: input.dried_water,
    }
}

/// Humidité des grandes ceintures selon |sin(latitude)| : pluies à l'équateur, déserts vers 25-30°,
/// latitudes moyennes humides, pôles secs.
pub fn belt_humidity(sin_lat: f32) -> f32 {
    let lat = sin_lat.clamp(0.0, 1.0).asin().to_degrees();
    match lat {
        l if l < 12.0 => 1.0,
        l if l < 22.0 => 1.0 - (l - 12.0) / 10.0 * 0.65,
        l if l < 35.0 => 0.35,
        l if l < 45.0 => 0.35 + (l - 35.0) / 10.0 * 0.45,
        l if l < 62.0 => 0.8,
        l => 0.8 - ((l - 62.0) / 28.0).min(1.0) * 0.45,
    }
}

/// Biomes d'un astre, prêts à être évalués colonne par colonne.
pub struct BiomeField {
    params: BiomeParams,
    moisture: Perlin,
    soil: Perlin,
}

impl BiomeField {
    pub fn new(params: BiomeParams) -> Self {
        Self { params, moisture: Perlin::new(params.seed.wrapping_add(600)), soil: Perlin::new(params.seed.wrapping_add(700)) }
    }

    /// Humidité locale (0..1).
    pub fn humidity(&self, dir: Vec3) -> f32 {
        let n = self.moisture.get([dir.x as f64 * 2.5, dir.y as f64 * 2.5, dir.z as f64 * 2.5]) as f32;
        (self.params.wetness * belt_humidity(dir.y.abs()) + 0.3 * n).clamp(0.0, 1.0)
    }

    /// Sol d'une région sèche ou sans vie.
    pub fn soil(&self, dir: Vec3, climate: &Climate, hydro: &Hydro, rh: f32, t: f32) -> Soil {
        let p = &self.params;
        let n = self.soil.get([dir.x as f64 * 4.0, dir.y as f64 * 4.0, dir.z as f64 * 4.0]) as f32 * 0.5 + 0.5;
        let _ = climate;
        if t < FREEZE_C && hydro.snow {
            Soil::Ice
        } else if p.volcanism > 0.3 && n > 1.0 - p.volcanism * 0.6 {
            Soil::Volcanic
        } else if p.metal && n < 0.3 {
            Soil::Metal
        } else if rh < 0.0 && p.salt {
            // Bassin à sec : une mer s'est évaporée là, elle a laissé son sel
            Soil::Salt
        } else if p.regolith {
            Soil::Regolith
        } else if self.params.wetness > 0.4 && t < DESERT_C {
            Soil::Clay
        } else {
            Soil::Sand
        }
    }

    /// Biome du sol émergé (ou d'un bassin à sec) dans la direction `dir`. `rh` : hauteur
    /// au-dessus de la mer.
    pub fn biome(&self, climate: &Climate, hydro: &Hydro, airless: bool, atmosphere: bool, rh: f32, dir: Vec3) -> Biome {
        let p = &self.params;
        if airless {
            return Biome::Regolith;
        }
        let sin_lat = dir.y.abs();
        let t = climate.temperature(sin_lat.clamp(0.0, 1.0).asin(), relative_altitude(rh), None);
        let barren = |soil: Soil| match soil {
            Soil::Volcanic => Biome::BasaltField,
            Soil::Metal => Biome::RustPlain,
            Soil::Salt => Biome::SaltFlat,
            Soil::Regolith => Biome::Regolith,
            Soil::Ice => Biome::IceSheet,
            Soil::Sand | Soil::Clay => {
                if t > SCORCH_C && p.radiation > 0.5 {
                    Biome::GlassDesert
                } else if rh > 0.3 {
                    Biome::Alpine
                } else {
                    Biome::Desert
                }
            }
        };
        if t > SCORCH_C {
            return barren(self.soil(dir, climate, hydro, rh, t));
        }
        if t < FREEZE_C {
            return if hydro.snow || (hydro.co2_frost && t < CO2_FROST_C) { Biome::IceSheet } else { barren(self.soil(dir, climate, hydro, rh, t)) };
        }
        let h = self.humidity(dir);
        let living = atmosphere && hydro.liquid == Liquid::Water;
        if !living {
            // Pas de pluie : soufre là où les volcans dégazent, sinon les sols nus
            if p.sulfur && p.volcanism > 0.2 && rh < 0.1 && h > 0.0 {
                return Biome::SulfurMarsh;
            }
            return barren(self.soil(dir, climate, hydro, rh, t));
        }
        // Haute montagne : roche nue, neige au sommet
        if rh > 0.42 {
            return if t < -5.0 && hydro.snow { Biome::IceSheet } else { Biome::Alpine };
        }
        // Sous le niveau de la mer mais à sec : l'eau s'est évaporée là (elle bout à l'équateur
        // d'un monde à l'air mince) et a laissé son sel
        if rh < 0.0 {
            return Biome::SaltFlat;
        }
        if rh < 0.008 {
            return Biome::Beach;
        }
        let soil = self.soil(dir, climate, hydro, rh, t);
        if soil == Soil::Volcanic && rh > 0.05 {
            return Biome::BasaltField;
        }
        if t > DESERT_C || h < 0.12 {
            return if p.radiation > 0.6 { Biome::GlassDesert } else if soil == Soil::Metal { Biome::RustPlain } else { Biome::Desert };
        }
        // Une radiation forte grille la végétation fragile
        if p.radiation > 0.75 {
            return barren(soil);
        }
        let alien = p.alien;
        if rh < 0.05 && h > 0.75 {
            return if alien && p.sulfur { Biome::SulfurMarsh } else { Biome::Swamp };
        }
        if t < 2.0 {
            if alien { Biome::CrystalForest } else { Biome::Tundra }
        } else if t < 9.0 {
            if h > 0.45 {
                if alien { Biome::CrystalForest } else { Biome::Taiga }
            } else if alien {
                Biome::CrystalForest
            } else {
                Biome::Tundra
            }
        } else if t < 22.0 {
            if h > 0.6 {
                if alien { Biome::FungalJungle } else { Biome::Forest }
            } else if h > 0.3 {
                if alien { Biome::SporePlain } else { Biome::Grassland }
            } else if alien {
                Biome::SporePlain
            } else {
                Biome::Steppe
            }
        } else if h > 0.7 {
            if alien { Biome::FungalJungle } else { Biome::Jungle }
        } else if h > 0.35 {
            if alien { Biome::SporePlain } else { Biome::Savanna }
        } else {
            Biome::Desert
        }
    }

    /// Matière voxel du sol émergé.
    pub fn material(&self, climate: &Climate, hydro: &Hydro, airless: bool, atmosphere: bool, rh: f32, dir: Vec3) -> VoxelType {
        if !self.params.defined {
            return land_material(climate, hydro, airless, atmosphere, rh, dir.y.abs());
        }
        self.biome(climate, hydro, airless, atmosphere, rh, dir).voxel()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sphere(n: usize) -> impl Iterator<Item = Vec3> {
        (0..n).map(move |i| {
            let z = 1.0 - 2.0 * (i as f32 + 0.5) / n as f32;
            let a = i as f32 * 2.399_963;
            let r = (1.0 - z * z).sqrt();
            Vec3::new(r * a.cos(), z, r * a.sin())
        })
    }

    fn earth_params(alien: bool) -> BiomeParams {
        BiomeParams { defined: true, seed: 3, wetness: 0.8, radiation: 0.05, alien, volcanism: 0.2, metal: false, sulfur: false, regolith: false, salt: false }
    }

    #[test]
    fn belts_are_wet_dry_wet_dry() {
        assert!(belt_humidity(0.0) > belt_humidity(0.45));
        assert!(belt_humidity(0.8) > belt_humidity(0.45));
        assert!(belt_humidity(1.0) < belt_humidity(0.8));
    }

    #[test]
    fn earth_gets_varied_terrestrial_biomes_and_never_jungle_in_the_cold() {
        let field = BiomeField::new(earth_params(false));
        let climate = Climate { mean_c: 25.0, span: 50.0, lapse: 50.0, diurnal: 10.0, tilt: 23.0 };
        let hydro = Hydro::default();
        let mut seen = std::collections::HashSet::new();
        for dir in sphere(6000) {
            for rh in [0.004, 0.08, 0.2, 0.35, 0.44] {
                let b = field.biome(&climate, &hydro, false, true, rh, dir);
                seen.insert(b);
                let t = climate.temperature(dir.y.abs().asin(), relative_altitude(rh), None);
                // Invariants de la feuille de route
                if matches!(b, Biome::Jungle | Biome::Savanna) {
                    assert!(t >= 22.0, "{b:?} a {t} C");
                }
                if b.vegetated() {
                    assert!((FREEZE_C..=DESERT_C).contains(&t), "{b:?} a {t} C");
                }
                assert_eq!(b.realism(), if b.vegetated() { Realism::Realistic } else { b.realism() });
            }
        }
        for b in [Biome::Forest, Biome::Grassland, Biome::Desert, Biome::Tundra, Biome::IceSheet, Biome::Jungle, Biome::Beach, Biome::Alpine] {
            assert!(seen.contains(&b), "{b:?} absent : {seen:?}");
        }
    }

    #[test]
    fn worlds_without_oxygen_grow_alien_biomes() {
        let field = BiomeField::new(earth_params(true));
        let climate = Climate { mean_c: 15.0, span: 50.0, lapse: 50.0, diurnal: 10.0, tilt: 23.0 };
        let mut seen = std::collections::HashSet::new();
        for dir in sphere(4000) {
            let b = field.biome(&climate, &Hydro::default(), false, true, 0.15, dir);
            assert!(!matches!(b, Biome::Forest | Biome::Grassland | Biome::Jungle | Biome::Taiga), "{b:?}");
            seen.insert(b);
        }
        assert!(seen.iter().any(|b| b.realism() == Realism::Fictional), "{seen:?}");
    }

    #[test]
    fn dead_worlds_show_their_soils() {
        let mut p = earth_params(true);
        p.wetness = 0.05;
        p.volcanism = 0.8;
        p.metal = true;
        p.regolith = true;
        p.salt = true;
        let field = BiomeField::new(p);
        let climate = Climate { mean_c: -20.0, span: 60.0, lapse: 0.0, diurnal: 40.0, tilt: 0.0 };
        let mut seen = std::collections::HashSet::new();
        for dir in sphere(4000) {
            for rh in [-0.05, 0.1, 0.3] {
                let b = field.biome(&climate, &Hydro::DRY, false, false, rh, dir);
                assert!(!b.vegetated(), "{b:?} sans eau ni air");
                assert_ne!(b, Biome::IceSheet, "pas de glace sans eau");
                seen.insert(b);
            }
        }
        for b in [Biome::BasaltField, Biome::RustPlain, Biome::SaltFlat, Biome::Regolith] {
            assert!(seen.contains(&b), "{b:?} absent : {seen:?}");
        }
    }

    #[test]
    fn radiation_is_shielded_by_air_and_magnetism() {
        let earth = surface_radiation(1.0, 1.0, 1.0, 1.0, 0.21);
        let mars = surface_radiation(0.43, 0.43, 0.006, 0.0, 0.0);
        let flare_star_bare = surface_radiation(0.3, 20_000.0, 0.0, 0.0, 0.0);
        let flare_star_air = surface_radiation(0.3, 20_000.0, 1.0, 0.0, 0.0);
        assert!(earth < 0.01 && mars > earth * 10.0, "{earth} {mars}");
        // Une naine rouge active grille un monde sans air ; une atmosphère épaisse protège
        assert!(flare_star_bare > 0.95 && flare_star_air < 0.3, "{flare_star_bare} {flare_star_air}");
    }
}
