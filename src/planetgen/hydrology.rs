//! Eau et glace (phase 4) : état de l'eau (liquide, glace, vapeur, supercritique) selon la
//! température et la pression, couverture océanique (elle fixe le niveau de la mer), calottes et
//! glaciers, océans exotiques (méthane, ammoniac, lave), eau souterraine (valeur seulement).
//!
//! - Réserve d'eau : faible en deçà de la ligne des glaces (apportée par les impacts), forte
//!   au-delà (mondes-océans).
//! - Ébullition selon la pression (Clausius-Clapeyron), point triple (pas d'eau liquide sous
//!   6 mbar), point critique (647 K, 221 bar : au-delà, fluide supercritique).
//! - Le relief suit une loi normale N(0,5 ; 0,09) (mesuré, voir `terrain.rs`) : le niveau de la mer
//!   qui donne une couverture f vaut 0,5 + 0,09 Φ⁻¹(f).
//! - Où il fait trop froid, la mer gèle (banquise) ; sur terre, neige et glaciers seulement s'il y a
//!   de l'eau (ou du givre de CO2 sous −78 °C, comme les calottes de Mars). Un monde sec reste nu.

use serde::{Deserialize, Serialize};

use super::atmosphere::Air;
use super::climate::Climate;
use super::seeds::LayerRng;
use super::system::PlanetKind;

/// Liquide des mers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Liquid {
    None,
    #[default]
    Water,
    /// Titan : lacs de méthane (et d'éthane) à −180 °C.
    Methane,
    /// Mélange eau-ammoniac, liquide jusqu'à −78 °C.
    Ammonia,
    /// Océan de magma d'un monde brûlant.
    Lava,
}

impl Liquid {
    pub fn name(self) -> &'static str {
        match self {
            Liquid::None => "aucun",
            Liquid::Water => "eau",
            Liquid::Methane => "methane",
            Liquid::Ammonia => "ammoniac",
            Liquid::Lava => "lave",
        }
    }
}

/// État de l'eau aux conditions moyennes de la surface.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum WaterState {
    /// Pas (ou plus) d'eau en surface.
    #[default]
    Absent,
    Liquid,
    Ice,
    Vapour,
    Supercritical,
}

impl WaterState {
    pub fn name(self) -> &'static str {
        match self {
            WaterState::Absent => "absente",
            WaterState::Liquid => "liquide",
            WaterState::Ice => "glace",
            WaterState::Vapour => "vapeur",
            WaterState::Supercritical => "supercritique",
        }
    }
}

/// Ce que le terrain doit savoir pour choisir ses matières (léger, copié dans `BodyParams`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hydro {
    pub liquid: Liquid,
    /// Le liquide gèle sous cette température (°C)...
    pub freeze_c: f32,
    /// ... et bout au-dessus de celle-ci (°C), à la pression du sol.
    pub boil_c: f32,
    /// De l'eau peut geler sur terre : neige, calottes, glaciers.
    pub snow: bool,
    /// Givre de CO2 sous −78 °C (calottes de Mars).
    pub co2_frost: bool,
}

impl Default for Hydro {
    /// Planète faite à la main : de l'eau, comme avant la phase 4.
    fn default() -> Self {
        Self { liquid: Liquid::Water, freeze_c: -10.0, boil_c: 100.0, snow: true, co2_frost: false }
    }
}

impl Hydro {
    /// Sans eau ni autre liquide (lunes, géantes).
    pub const DRY: Hydro = Hydro { liquid: Liquid::None, freeze_c: 0.0, boil_c: 0.0, snow: false, co2_frost: false };
}

/// Eau et glace d'une planète (profil).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Hydrology {
    pub hydro: Hydro,
    pub water_state: WaterState,
    /// Part de la surface sous le niveau des mers (remplie de liquide ou de banquise).
    pub ocean_fraction: f32,
    /// Part de la surface sous les calottes et la banquise, au niveau de la mer.
    pub ice_caps: f32,
    /// Réserve d'eau totale (0 : sèche, 1 : monde-océan).
    pub inventory: f32,
    /// Eau souterraine, en mètres d'eau répartis sur toute la planète (Terre : ~200 m).
    pub groundwater_m: f32,
    /// Océan liquide sous une croûte de glace, chauffé par les marées (Europe, Encelade).
    #[serde(default)]
    pub subsurface_ocean: bool,
}

/// Température d'ébullition (K) d'un liquide sous `pressure` bar, d'après sa température
/// d'ébullition à 1 bar et sa chaleur latente (L/R, K) : Clausius-Clapeyron.
pub fn boiling_point(t_boil_1bar: f64, latent_over_r: f64, pressure: f64) -> f64 {
    1.0 / (1.0 / t_boil_1bar - pressure.max(1e-6).ln() / latent_over_r)
}

/// Eau : point triple, point critique.
const WATER_TRIPLE_BAR: f64 = 0.006;
const WATER_CRITICAL_K: f64 = 647.0;
const WATER_CRITICAL_BAR: f64 = 220.6;

/// État de l'eau à `t` K sous `pressure` bar.
pub fn water_state(t: f64, pressure: f64) -> WaterState {
    if t > WATER_CRITICAL_K && pressure > WATER_CRITICAL_BAR {
        WaterState::Supercritical
    } else if t < 273.15 {
        WaterState::Ice
    } else if pressure < WATER_TRIPLE_BAR || t > boiling_point(373.15, 4_884.0, pressure) {
        WaterState::Vapour
    } else {
        WaterState::Liquid
    }
}

/// Fonction de répartition inverse de la loi normale (Abramowitz et Stegun 26.2.23, ±5e-4).
pub fn probit(p: f64) -> f64 {
    let p = p.clamp(1e-6, 1.0 - 1e-6);
    let (q, sign) = if p < 0.5 { (p, -1.0) } else { (1.0 - p, 1.0) };
    let t = (-2.0 * q.ln()).sqrt();
    sign * (t - (2.515_517 + 0.802_853 * t + 0.010_328 * t * t) / (1.0 + 1.432_788 * t + 0.189_269 * t * t + 0.001_308 * t * t * t))
}

/// Écart type et moyenne des hauteurs du relief (`terrain.rs`, mesurés).
pub const RELIEF_MEAN: f64 = 0.5;
pub const RELIEF_SD: f64 = 0.09;

/// Niveau de la mer qui met une part `fraction` de la surface sous les eaux.
pub fn sea_level_for(fraction: f64) -> f32 {
    (RELIEF_MEAN + RELIEF_SD * probit(fraction)) as f32
}

/// Part de la surface (au niveau de la mer) où il fait plus froid que `freeze_c`.
pub fn cold_fraction(climate: &Climate, freeze_c: f32) -> f32 {
    if climate.span <= 0.0 {
        return if climate.mean_c < freeze_c { 1.0 } else { 0.0 };
    }
    // T(lat) = moyenne + écart (1/3 − sin²) < gel  ⇔  sin² > 1/3 + (moyenne − gel) / écart
    let s2 = 1.0 / 3.0 + (climate.mean_c - freeze_c) / climate.span;
    1.0 - s2.clamp(0.0, 1.0).sqrt()
}

/// Données d'entrée.
pub struct HydroInput<'a> {
    pub kind: PlanetKind,
    /// Distance à l'étoile rapportée à la ligne des glaces.
    pub snow_ratio: f64,
    pub mass: f64,
    pub air: &'a Air,
    pub climate: &'a Climate,
}

/// Eau, glace et mers d'une planète, et le niveau de la mer correspondant.
pub fn generate(input: &HydroInput, rng: &mut LayerRng) -> (Hydrology, f32) {
    let relief_draw = rng.unit();
    if input.kind.gaseous() {
        return (Hydrology { hydro: Hydro::DRY, ..Default::default() }, 0.0);
    }
    let p = input.air.pressure_bar as f64;
    let t_mean = input.climate.mean_c as f64 + 273.15;
    // Réserve d'eau : sèche à humide près de l'étoile, monde-océan possible au-delà des glaces
    let u = rng.unit();
    let inventory = if input.snow_ratio < 1.0 { 0.65 * u * u } else { 0.15 + 0.85 * u };
    let state = water_state(t_mean, p);
    let co2 = input.air.fraction("CO2") > 0.5;

    // Liquide des mers, par ordre de priorité : lave, eau (liquide ou gelée), ammoniac, méthane
    let methane = input.air.fraction("CH4") >= 0.01 && p > 0.1;
    let t_methane_boil = boiling_point(111.7, 985.0, p);
    let methane_liquid = methane && (91.0..t_methane_boil).contains(&t_mean);
    let (liquid, fraction) = if t_mean > 1_300.0 {
        (Liquid::Lava, 0.3 + 0.6 * rng.unit())
    } else if state == WaterState::Ice && methane_liquid {
        // Titan : la glace d'eau fait le sol, les lacs sont de méthane
        (Liquid::Methane, 0.02 + 0.3 * rng.unit())
    } else if matches!(state, WaterState::Liquid | WaterState::Ice) && inventory > 0.02 && p >= WATER_TRIPLE_BAR {
        // Monde froid riche en eau : parfois un océan d'eau et d'ammoniac
        if (195.0..240.0).contains(&t_mean) && inventory > 0.4 && rng.unit() < 0.25 {
            (Liquid::Ammonia, inventory.min(0.95))
        } else {
            (Liquid::Water, inventory.min(0.97))
        }
    } else if methane_liquid {
        (Liquid::Methane, 0.02 + 0.3 * rng.unit())
    } else {
        (Liquid::None, 0.0)
    };
    let (freeze_c, boil_c) = match liquid {
        Liquid::Water => (-2.0, boiling_point(373.15, 4_884.0, p) - 273.15),
        Liquid::Ammonia => (-78.0, boiling_point(239.8, 2_800.0, p) - 273.15),
        Liquid::Methane => (-182.0, t_methane_boil - 273.15),
        Liquid::Lava => (1_000.0, 3_000.0),
        Liquid::None => (0.0, 0.0),
    };
    // De l'eau (même gelée, même sous une mer d'autre chose) blanchit les régions froides
    let snow = inventory > 0.02 && p >= WATER_TRIPLE_BAR && liquid != Liquid::Lava && t_mean < 400.0;
    let hydro = Hydro { liquid, freeze_c: freeze_c as f32, boil_c: boil_c as f32, snow, co2_frost: co2 && p > 0.001 };

    // Bassins : remplis par la mer, ou à sec (on garde du relief en creux)
    let ocean = fraction.clamp(0.0, 0.97);
    let sea_level = if liquid == Liquid::None { sea_level_for(0.05 + 0.3 * relief_draw) } else { sea_level_for(ocean.max(0.01)) };
    let ice_caps = if snow { cold_fraction(input.climate, -10.0) } else { 0.0 };
    let water_state = if inventory <= 0.02 || (p < WATER_TRIPLE_BAR && state != WaterState::Ice) { WaterState::Absent } else { state };
    // L'eau qui n'est pas en surface : dans le sol (gelée sous les calottes)
    let groundwater_m = (inventory * 300.0 * (1.0 - ocean * 0.5)) as f32;

    (
        Hydrology {
            hydro,
            water_state,
            ocean_fraction: if liquid == Liquid::None { 0.0 } else { ocean as f32 },
            ice_caps,
            inventory: inventory as f32,
            groundwater_m,
            subsurface_ocean: false,
        },
        sea_level,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn water_follows_its_phase_diagram() {
        assert_eq!(water_state(288.0, 1.0), WaterState::Liquid);
        assert_eq!(water_state(250.0, 1.0), WaterState::Ice);
        assert_eq!(water_state(380.0, 1.0), WaterState::Vapour);
        // Sous 2 bar, l'eau bout plus haut ; sur Mars (6 mbar), plus d'eau liquide
        assert_eq!(water_state(380.0, 2.0), WaterState::Liquid);
        assert_eq!(water_state(280.0, 0.004), WaterState::Vapour);
        assert_eq!(water_state(737.0, 92.0), WaterState::Vapour);
        assert_eq!(water_state(700.0, 300.0), WaterState::Supercritical);
        assert!((boiling_point(373.15, 4_884.0, 1.013) - 373.15).abs() < 0.5);
        // Méthane : ~112 K à 1 bar, ~118 K sur Titan (1,5 bar)
        assert!((boiling_point(111.7, 985.0, 1.5) - 118.0).abs() < 2.0);
    }

    #[test]
    fn probit_and_sea_level() {
        assert!(probit(0.5).abs() < 1e-3);
        assert!((probit(0.9) - 1.2816).abs() < 1e-3);
        assert!((probit(0.1) + 1.2816).abs() < 1e-3);
        assert!((sea_level_for(0.5) - 0.5).abs() < 1e-3);
        assert!(sea_level_for(0.71) > 0.5 && sea_level_for(0.1) < 0.4);
    }

    #[test]
    fn ice_caps_grow_with_the_cold() {
        let earth = Climate { mean_c: 15.0, span: 50.0, ..Default::default() };
        let caps = cold_fraction(&earth, -10.0);
        assert!(caps > 0.05 && caps < 0.25, "{caps}");
        assert_eq!(cold_fraction(&Climate { mean_c: -60.0, ..earth }, -10.0), 1.0);
        assert_eq!(cold_fraction(&Climate { mean_c: 80.0, ..earth }, -10.0), 0.0);
    }

    fn air(gases: &[(&str, f32)], pressure: f32) -> Air {
        Air { pressure_bar: pressure, gases: gases.iter().map(|(f, x)| (f.to_string(), *x)).collect(), ..Default::default() }
    }

    #[test]
    fn titan_has_methane_lakes_and_lava_worlds_have_magma() {
        let titan = air(&[("N2", 0.95), ("CH4", 0.05)], 1.5);
        let cold = Climate { mean_c: -179.0, span: 3.0, ..Default::default() };
        let mut rng = LayerRng::new(3, super::super::seeds::Layer::Hydrology);
        let (h, _) = generate(&HydroInput { kind: PlanetKind::Rocky, snow_ratio: 0.5, mass: 0.02, air: &titan, climate: &cold }, &mut rng);
        assert_eq!(h.hydro.liquid, Liquid::Methane, "{h:?}");
        assert!(h.ocean_fraction > 0.0 && h.ocean_fraction < 0.4);

        let hot = Climate { mean_c: 1_400.0, span: 300.0, ..Default::default() };
        let (h, _) = generate(&HydroInput { kind: PlanetKind::Rocky, snow_ratio: 0.1, mass: 1.0, air: &Air::default(), climate: &hot }, &mut rng);
        assert_eq!(h.hydro.liquid, Liquid::Lava);
        assert!(!h.hydro.snow);
    }

    #[test]
    fn dry_cold_worlds_have_no_snow_and_airless_ones_no_seas() {
        let mut rng = LayerRng::new(9, super::super::seeds::Layer::Hydrology);
        let cold = Climate { mean_c: -60.0, span: 60.0, ..Default::default() };
        for _ in 0..200 {
            let (h, _) = generate(&HydroInput { kind: PlanetKind::Rocky, snow_ratio: 0.5, mass: 0.1, air: &Air::default(), climate: &cold }, &mut rng);
            assert_eq!(h.hydro.liquid, Liquid::None);
            assert!(!h.hydro.snow);
            // Sans air, de la glace peut subsister (pôles de Mercure, Cérès), jamais de l'eau liquide
            assert!(matches!(h.water_state, WaterState::Absent | WaterState::Ice), "{h:?}");
        }
    }
}
