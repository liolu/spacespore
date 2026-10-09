//! Mondes exceptionnels (0.14 bloc X, règles 20 à 23 et 27 de `ROADMAP-0.14.md`).
//!
//! Un archétype est une couche de génération (`Layer::Archetype`, sous-graine figée) tirée **après** les
//! couches physiques, seulement si l'astre le permet. Il **modifie** les couches existantes par des
//! paramètres (climat, mer, relief, biomes, anneaux, aurores, rotation) : chaque archétype change le monde
//! et se voit (règle 20). Un astre ordinaire ne paie rien.
//!
//! - Raretés : celles des traits (98 / 1,5 / 0,4 / 0,1 %, `traits::TIERS`). **N** = naturel : découle de
//!   la physique (œil verrouillé), sans tirage.
//! - Zone calme (règle 23) : aucun archétype **tiré** dans les 50 systèmes les plus proches du système 0
//!   (`StarSystemConfig::calm`) ; les naturels restent possibles partout.
//! - Scanner (section « ANOMALIE »), dex, `/aller planete <archetype>`, `/stats`, signal de loin
//!   (cercle magenta), test de fréquence.
//!
//! Faits : X1 (œil, crépuscule étroit, jour sans fin), X2 (océan profond, peu profond, monde-pluie), X4
//! (anneaux bas), X5 (aurores géantes), X6 (cristal, métal, fongique, vertical, cubique), X8 (dévasté).
//! Les autres sortes du catalogue (planète étirée, creuse, cassée, refroidissement, ruines...) demandent des
//! formes ou des rendus nouveaux : elles s'ajouteront ici, une par une, sans changer les numéros existants.

use serde::{Deserialize, Serialize};

use super::biome::Biome;
use super::hydrology::{Liquid, WaterState};
use super::profile::Realism;
use super::seeds::LayerRng;
use super::system::{Aurora, Ring};
use super::traits::TIERS;
use crate::settings::PlanetConfig;

/// Les sortes de mondes exceptionnels. Ne jamais renuméroter (ajout seulement, à la fin).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Archetype {
    Eye,
    NarrowTwilight,
    EndlessDay,
    DeepOcean,
    ShallowOcean,
    RainWorld,
    GiantAuroras,
    LowRings,
    Crystal,
    Metal,
    Fungal,
    Vertical,
    Devastated,
    Cubic,
}

const R: Realism = Realism::Realistic;
const S: Realism = Realism::Speculative;
const F: Realism = Realism::Fictional;

/// Une sorte du catalogue : rareté (0 naturel, 1 peu commun, 2 rare, 3 légendaire), réalisme, phase,
/// mot pour `/aller`, condition.
pub struct Kind {
    pub arch: Archetype,
    pub tier: usize,
    pub realism: Realism,
    pub phase: &'static str,
    pub key: &'static str,
    pub name: &'static str,
    pub what: &'static str,
    allowed: fn(&PlanetConfig) -> bool,
}

fn solid(p: &PlanetConfig) -> bool {
    !p.gaseous() && !p.rogue
}

fn liquid_water(p: &PlanetConfig) -> bool {
    p.hydrology.hydro.liquid == Liquid::Water && p.hydrology.water_state == WaterState::Liquid
}

pub const CATALOG: [Kind; 14] = [
    Kind { arch: Archetype::Eye, tier: 0, realism: R, phase: "X1", key: "oeil", name: "oeil (rotation synchrone)", what: "face jour brulee, face nuit gelee, bande crepusculaire au coucher de soleil eternel", allowed: |p| solid(p) && p.tidally_locked && p.atmosphere },
    Kind { arch: Archetype::NarrowTwilight, tier: 1, realism: R, phase: "X1", key: "crepuscule", name: "crepuscule etroit", what: "une bande habitable etroite entre une face brulante et une face glacee", allowed: |p| solid(p) && p.tidally_locked && p.atmosphere },
    Kind { arch: Archetype::EndlessDay, tier: 2, realism: R, phase: "X1", key: "jour-sans-fin", name: "jour sans fin", what: "rotation tres lente : le coucher de soleil avance au pas, on peut le suivre", allowed: |p| solid(p) && !p.tidally_locked && p.atmosphere },
    Kind { arch: Archetype::DeepOcean, tier: 1, realism: R, phase: "X2", key: "ocean-profond", name: "monde-ocean profond", what: "aucune terre ou presque : quelques iles volcaniques", allowed: |p| solid(p) && liquid_water(p) && p.atmosphere && p.hydrology.ocean_fraction > 0.3 },
    Kind { arch: Archetype::ShallowOcean, tier: 1, realism: R, phase: "X2", key: "ocean-peu-profond", name: "ocean peu profond", what: "une mer mince sur tout le globe : on voit le fond partout", allowed: |p| solid(p) && liquid_water(p) && p.atmosphere && p.hydrology.ocean_fraction > 0.2 },
    Kind { arch: Archetype::RainWorld, tier: 1, realism: R, phase: "X2", key: "pluie", name: "monde-pluie", what: "nuages partout, pluie permanente, sols detrempes", allowed: |p| solid(p) && liquid_water(p) && p.atmosphere && p.air.pressure_bar > 0.3 },
    Kind { arch: Archetype::GiantAuroras, tier: 1, realism: R, phase: "X5", key: "aurores-geantes", name: "aurores geantes", what: "champ magnetique tres fort : aurores jusqu'a l'equateur", allowed: |p| !p.rogue && (p.atmosphere || p.gaseous()) && p.geology.magnetic_field > 0.2 },
    Kind { arch: Archetype::LowRings, tier: 2, realism: R, phase: "X4", key: "anneaux-bas", name: "anneaux bas", what: "un anneau juste au-dessus de l'atmosphere, arche immense dans le ciel", allowed: |p| !p.rogue },
    Kind { arch: Archetype::Crystal, tier: 2, realism: S, phase: "X6", key: "cristal", name: "monde cristallin", what: "croute et montagnes de cristal", allowed: solid },
    Kind { arch: Archetype::Metal, tier: 2, realism: S, phase: "X6", key: "metal", name: "monde metallique", what: "surface de fer et de rouille, noyau presque a nu", allowed: solid },
    Kind { arch: Archetype::Fungal, tier: 2, realism: S, phase: "X6", key: "fongique", name: "monde fongique", what: "champignons et spores partout, ciel violet", allowed: |p| solid(p) && p.atmosphere && p.climate.is_some_and(|c| (-15.0..55.0).contains(&c.mean_c)) },
    Kind { arch: Archetype::Vertical, tier: 2, realism: S, phase: "X6", key: "vertical", name: "monde vertical", what: "plateaux etages separes par des parois : on vit par niveaux", allowed: solid },
    Kind { arch: Archetype::Devastated, tier: 2, realism: F, phase: "X8", key: "devaste", name: "monde devaste", what: "deserts de verre et de cendres, radiation au sol", allowed: |p| solid(p) && p.atmosphere },
    Kind { arch: Archetype::Cubic, tier: 3, realism: F, phase: "X6", key: "cubique", name: "monde cubique", what: "relief en gros cubes, falaises a angle droit", allowed: solid },
];

impl Archetype {
    pub fn kind(self) -> &'static Kind {
        CATALOG.iter().find(|k| k.arch == self).expect("archetype du catalogue")
    }
}

/// Archétype de la planète : tirage par rareté (sauf zone calme), sinon le naturel s'il s'applique.
pub fn roll(p: &PlanetConfig, calm: bool, rng: &mut LayerRng) -> Option<Archetype> {
    let tier = rng.weighted(&TIERS.map(|(_, w)| w));
    let pick = rng.unit();
    if tier > 0 && !calm {
        let allowed: Vec<&Kind> = CATALOG.iter().filter(|k| k.tier == tier && (k.allowed)(p)).collect();
        if !allowed.is_empty() {
            return Some(allowed[(pick * allowed.len() as f64) as usize % allowed.len()].arch);
        }
    }
    CATALOG.iter().find(|k| k.tier == 0 && (k.allowed)(p)).map(|k| k.arch)
}

/// Applique l'archétype aux couches de la planète (paramètres, jamais tout remplacé).
pub fn apply(p: &mut PlanetConfig, a: Archetype) {
    p.archetype = Some(a);
    match a {
        Archetype::Eye | Archetype::NarrowTwilight => {
            // Climat selon l'angle au point sous l'étoile (`Climate::eye`) : la face jour beaucoup plus
            // chaude que la face nuit ; air mince = bande étroite
            if let Some(c) = p.climate.as_mut() {
                c.eye = true;
                c.span = c.span.max(if a == Archetype::Eye { 110.0 } else { 220.0 });
            }
        }
        Archetype::EndlessDay => {
            p.rotation_h = p.rotation_h.max(24.0 * 150.0);
            if let Some(c) = p.climate.as_mut() {
                c.diurnal *= 1.8;
            }
        }
        Archetype::DeepOcean => {
            p.hydrology.ocean_fraction = 1.0;
            p.sea_level = 0.93;
            p.terrain_height *= 0.8;
        }
        Archetype::ShallowOcean => {
            p.hydrology.ocean_fraction = 1.0;
            p.sea_level = 0.86;
            p.terrain_height *= 0.12;
        }
        Archetype::RainWorld => {
            p.air.cloud_cover = 1.0;
            p.cloud_density = 1.0;
            p.biomes.wetness = 1.0;
        }
        Archetype::GiantAuroras => {
            p.geology.magnetic_field = p.geology.magnetic_field.max(1.0) * 4.0;
            let color = p.aurora.map_or([0.3, 1.0, 0.5], |a| a.color);
            p.aurora = Some(Aurora { strength: 1.0, color, latitude: 8.0 });
        }
        Archetype::LowRings => {
            let r = p.radius;
            let inner = r * 1.03 + 2.0;
            let outer = p.moons.first().map_or(r * 1.8, |m| (m.orbit_distance - 2.0 * m.radius).min(r * 1.8));
            if outer > inner * 1.1 {
                let base = p.ring.unwrap_or(Ring { color: [0.72, 0.68, 0.6], opacity: 0.7, ice: 0.5, gaps: [[0.55, 0.04], [0.0, 0.0], [0.0, 0.0]], seed: p.seed, ..Default::default() });
                p.ring = Some(Ring { inner, outer, opacity: base.opacity.max(0.6), ..base });
            }
        }
        Archetype::Crystal => p.biomes.force = Some((Biome::CrystalForest, Biome::CrystalForest)),
        Archetype::Metal => {
            p.biomes.metal = true;
            p.biomes.force = Some((Biome::RustPlain, Biome::BasaltField));
        }
        Archetype::Fungal => {
            p.biomes.force = Some((Biome::FungalJungle, Biome::SporePlain));
            // Ciel teinté de spores, sans noyer le sol dans le violet : mélange léger avec le vrai ciel
            let tint = |c: [f32; 3], t: [f32; 3], k: f32| [0, 1, 2].map(|i| c[i] + (t[i] - c[i]) * k);
            p.air.sky = tint(p.air.sky, [0.62, 0.42, 0.78], 0.35);
            p.air.haze = tint(p.air.haze, [0.7, 0.55, 0.8], 0.2);
            p.air.sunset = tint(p.air.sunset, [0.9, 0.4, 0.6], 0.4);
        }
        Archetype::Vertical => {
            p.geology.relief.tier_step = 90.0;
            p.terrain_height = (p.terrain_height * 1.5).min(p.radius * 0.059);
        }
        Archetype::Devastated => {
            p.biomes.force = Some((Biome::GlassDesert, Biome::BasaltField));
            p.biomes.radiation = p.biomes.radiation.max(0.9);
        }
        Archetype::Cubic => p.geology.relief.cubic_step = 32.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_keys_are_unique_and_every_kind_is_listed() {
        let keys: std::collections::HashSet<&str> = CATALOG.iter().map(|k| k.key).collect();
        assert_eq!(keys.len(), CATALOG.len());
        for k in &CATALOG {
            assert_eq!(k.arch.kind().key, k.key);
            // Les fictifs sont rares ou légendaires (Q1)
            if k.realism == F {
                assert!(k.tier >= 2, "{}", k.key);
            }
        }
    }

    #[test]
    fn the_eye_is_hot_under_the_star_and_frozen_behind_with_a_twilight_band() {
        use bevy::math::Vec3;
        let mut p = PlanetConfig { tidally_locked: true, atmosphere: true, climate: Some(super::super::climate::Climate { mean_c: 10.0, ..Default::default() }), ..Default::default() };
        apply(&mut p, Archetype::Eye);
        let c = p.climate.unwrap();
        let t = |d: Vec3| c.temperature(c.lat_of(d), 0.0, None);
        let (day, dusk, night) = (t(Vec3::X), t(Vec3::Z), t(-Vec3::X));
        assert!(day > dusk && dusk > night && day - night > 100.0, "{day} {dusk} {night}");
        assert!((-10.0..45.0).contains(&dusk), "bande du crepuscule vivable : {dusk}");
        // Les pôles (géographiques) sont au crépuscule, comme toute la ligne jour / nuit
        assert!((t(Vec3::Y) - dusk).abs() < 15.0, "bordures irregulieres : a peu pres le meme climat");
    }

    #[test]
    fn vertical_and_cubic_worlds_are_stepped() {
        use super::super::geology::Relief;
        use super::super::landforms::{sculpt_dir, sculpt_height};
        use bevy::math::Vec3;
        let vertical = Relief { tier_step: 90.0, ..Default::default() };
        let cubic = Relief { cubic_step: 32.0, ..Default::default() };
        // Paliers presque plats, puis une paroi
        let a = sculpt_height(100.0, &vertical, 1.0);
        let b = sculpt_height(150.0, &vertical, 1.0);
        let c = sculpt_height(179.5, &vertical, 1.0);
        assert!((b - a).abs() < 6.0 && c > 170.0, "{a} {b} {c}");
        assert_eq!(sculpt_height(50.0, &cubic, 1.0) % 32.0, 0.0);
        // Deux directions voisines dans la même case lisent le même relief
        let d1 = Vec3::new(0.30, 0.1, 1.0).normalize();
        let d2 = Vec3::new(0.3001, 0.1, 1.0).normalize();
        assert_eq!(sculpt_dir(d1, &cubic, 10_000.0, 1.0), sculpt_dir(d2, &cubic, 10_000.0, 1.0));
        // Un monde ordinaire ne change rien
        assert_eq!(sculpt_dir(d1, &Relief::default(), 10_000.0, 1.0), d1);
        assert_eq!(sculpt_height(123.4, &Relief::default(), 1.0), 123.4);
    }

    /// Zone calme, raretés et fréquence dans la galaxie principale (règles 23 et 27).
    #[test]
    fn archetypes_are_rare_and_never_rolled_near_the_start() {
        let settings = crate::settings::GameSettings::default();
        let (mut rolled, mut planets) = (0usize, 0usize);
        let mut kinds = std::collections::HashMap::new();
        for sys in settings.systems.dense().iter().filter(|s| s.galaxy_id == 0) {
            for p in sys.planets_uncached().iter() {
                planets += 1;
                if let Some(a) = p.archetype {
                    let k = a.kind();
                    if k.tier > 0 {
                        assert!(!sys.calm, "{} tire dans la zone calme ({})", k.key, sys.name);
                        rolled += 1;
                    }
                    *kinds.entry(k.key).or_insert(0usize) += 1;
                }
            }
        }
        let calm = settings.systems.dense().iter().filter(|s| s.calm).count();
        assert_eq!(calm, 50, "zone calme de 50 systemes");
        let share = rolled as f64 / planets as f64;
        println!("{planets} planetes, {rolled} archetypes tires ({:.2} %) : {kinds:?}", share * 100.0);
        // 2 % au plus (98 % ordinaire), et pas absent
        assert!(share > 0.002 && share < 0.02, "{share}");
        assert!(kinds.len() >= 8, "{kinds:?}");
    }
}
