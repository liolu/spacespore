//! Climat d'un astre (phase 3) : température selon la latitude et l'altitude (et, à partir de la
//! 0.11, l'heure et la saison), et les matières de surface qui en découlent.
//!
//! Remplace l'ancienne température unique : la neige, les déserts, les mers gelées ou asséchées
//! se décident colonne par colonne avec la température locale. Le maillage lointain (`mesher.rs`)
//! et le terrain voxel (`terrain.rs`) utilisent les mêmes fonctions.

use serde::{Deserialize, Serialize};

use super::hydrology::{Hydro, Liquid};
use crate::planet::VoxelType;

/// Instant de la journée et de l'année (0.11) : 0 = minuit / début d'année, 0,5 = midi.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Moment {
    pub hour: f32,
    pub season: f32,
}

/// Paramètres de température d'un astre.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Climate {
    /// Température moyenne de la surface (°C), effet de serre compris.
    pub mean_c: f32,
    /// Écart de température équateur → pôle (K). Faible sous une atmosphère épaisse.
    pub span: f32,
    /// Refroidissement du niveau de la mer au sommet du relief (K) ; 0 sans atmosphère.
    pub lapse: f32,
    /// Amplitude jour / nuit (K), utilisée à partir de la 0.11.
    pub diurnal: f32,
    /// Inclinaison de l'axe (degrés) : les saisons (0.11).
    pub tilt: f32,
}

impl Default for Climate {
    fn default() -> Self {
        Self { mean_c: 15.0, span: 50.0, lapse: 50.0, diurnal: 10.0, tilt: 23.0 }
    }
}

/// Température (°C) qui ne laisse ni eau liquide ni herbe : la neige ou la glace la remplacent.
pub const FREEZE_C: f32 = -10.0;
/// Au-delà, plus d'eau liquide ni d'herbe : sable et roche.
pub const SCORCH_C: f32 = 100.0;
/// Au-delà, désert plutôt que prairie.
pub const DESERT_C: f32 = 45.0;

impl Climate {
    /// Climat d'un astre sans phase 3 (planète faite à la main) : ancienne température moyenne.
    pub fn from_mean(mean_c: f32, atmosphere: bool) -> Self {
        Self { mean_c, lapse: if atmosphere { 50.0 } else { 0.0 }, ..Default::default() }
    }

    /// Température (°C) à la latitude `lat` (radians) et à l'altitude relative `alt` (0 = niveau
    /// de la mer, 1 = sommet du relief). `moment` : heure et saison (0.11) ; `None` = moyenne.
    ///
    /// La moyenne sur toute la sphère vaut `mean_c` : sin² de la latitude vaut 1/3 en moyenne.
    pub fn temperature(&self, lat: f32, alt: f32, moment: Option<Moment>) -> f32 {
        let tau = std::f32::consts::TAU;
        let (lat, daily) = match moment {
            // Été dans l'hémisphère nord à la saison 0 : la zone la plus chaude remonte de `tilt`
            Some(m) => (lat - self.tilt.to_radians() * (tau * m.season).cos(), self.diurnal * (tau * (m.hour - 0.5)).cos()),
            None => (lat, 0.0),
        };
        let s = lat.sin();
        self.mean_c + self.span * (1.0 / 3.0 - s * s) - self.lapse * alt.clamp(0.0, 1.0) + daily
    }

    /// Températures à l'équateur et aux pôles (niveau de la mer, moyenne).
    pub fn range(&self) -> (f32, f32) {
        (self.temperature(0.0, 0.0, None), self.temperature(std::f32::consts::FRAC_PI_2, 0.0, None))
    }
}

/// Altitude relative (0..1) d'un point de hauteur relative `rh` (hauteur du bruit − niveau de la mer).
pub fn relative_altitude(rh: f32) -> f32 {
    (rh / 0.5).clamp(0.0, 1.0)
}

/// Matière du sol émergé. `rh` : hauteur au-dessus de la mer (0..~0,6) ; `sin_lat` : |sinus| de
/// la latitude (0 à l'équateur, 1 au pôle). La neige et les glaciers demandent de l'eau
/// (`hydro.snow`) ou du givre de CO2 très froid ; l'herbe demande de l'eau liquide et de l'air.
pub fn land_material(climate: &Climate, hydro: &Hydro, airless: bool, atmosphere: bool, rh: f32, sin_lat: f32) -> VoxelType {
    if airless {
        return VoxelType::Stone;
    }
    let t = climate.temperature(sin_lat.clamp(0.0, 1.0).asin(), relative_altitude(rh), None);
    let dry = || if rh < 0.12 { VoxelType::Sand } else { VoxelType::Stone };
    if t > SCORCH_C {
        return if rh < 0.30 { VoxelType::Sand } else { VoxelType::Stone };
    }
    if t < FREEZE_C {
        return if hydro.snow || (hydro.co2_frost && t < CO2_FROST_C) { VoxelType::Snow } else { dry() };
    }
    if !atmosphere || hydro.liquid != Liquid::Water {
        // Sans air ou sans eau liquide : régolithe et roche, pas d'herbe
        return dry();
    }
    if rh < 0.02 {
        VoxelType::Sand
    } else if rh < 0.28 {
        if t > DESERT_C { VoxelType::Sand } else { VoxelType::Grass }
    } else if rh < 0.42 || t > 0.0 {
        VoxelType::Stone
    } else {
        VoxelType::Snow
    }
}

/// Givre de CO2 : sous −78 °C (calottes de Mars).
pub const CO2_FROST_C: f32 = -78.0;

/// Ce qui remplit les bassins sous le niveau de la mer : le liquide de la planète (eau, méthane,
/// ammoniac, lave), sa glace s'il gèle à cette latitude, ou rien (mer à sec : il bout, ou il n'y en
/// a pas).
pub fn sea_material(climate: &Climate, hydro: &Hydro, airless: bool, sin_lat: f32) -> Option<VoxelType> {
    if airless {
        return None;
    }
    let liquid = match hydro.liquid {
        Liquid::None => return None,
        Liquid::Water => VoxelType::Water,
        Liquid::Methane => VoxelType::Methane,
        Liquid::Ammonia => VoxelType::Ammonia,
        Liquid::Lava => VoxelType::Lava,
    };
    let t = climate.temperature(sin_lat.clamp(0.0, 1.0).asin(), 0.0, None);
    if t > hydro.boil_c {
        None
    } else if t < hydro.freeze_c {
        // Lave figée : basalte ; les autres : banquise
        Some(if hydro.liquid == Liquid::Lava { VoxelType::Stone } else { VoxelType::Ice })
    } else {
        Some(liquid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn earth() -> Climate {
        Climate { mean_c: 15.0, span: 50.0, lapse: 50.0, diurnal: 10.0, tilt: 23.4 }
    }

    #[test]
    fn the_mean_over_the_sphere_is_the_mean() {
        let c = earth();
        // Moyenne pondérée par l'aire (uniforme en sin(lat))
        let n = 2000;
        let avg: f32 = (0..n).map(|i| c.temperature((-1.0 + (i as f32 + 0.5) * 2.0 / n as f32).asin(), 0.0, None)).sum::<f32>() / n as f32;
        assert!((avg - 15.0).abs() < 0.1, "{avg}");
        let (eq, pole) = c.range();
        assert!(eq > 25.0 && pole < -15.0, "{eq} {pole}");
    }

    #[test]
    fn altitude_season_and_hour_change_the_temperature() {
        let c = earth();
        assert!(c.temperature(0.3, 1.0, None) < c.temperature(0.3, 0.0, None) - 40.0);
        let noon = c.temperature(0.0, 0.0, Some(Moment { hour: 0.5, season: 0.25 }));
        let night = c.temperature(0.0, 0.0, Some(Moment { hour: 0.0, season: 0.25 }));
        assert!(noon - night > 15.0);
        // Été au nord : plus chaud à 45° N qu'à 45° S
        let summer = Some(Moment { hour: 0.5, season: 0.0 });
        assert!(c.temperature(0.8, 0.0, summer) > c.temperature(-0.8, 0.0, summer) + 10.0);
    }

    #[test]
    fn no_grass_or_water_where_it_freezes_or_boils() {
        // Invariant de la feuille de route : pas de forêt tropicale à -150 °C
        let water = Hydro { freeze_c: FREEZE_C, boil_c: SCORCH_C, ..Hydro::default() };
        for mean in [-150.0, -60.0, -20.0, 0.0, 15.0, 40.0, 80.0, 200.0, 450.0] {
            let c = Climate { mean_c: mean, ..earth() };
            for i in 0..=20 {
                let sin_lat = i as f32 / 20.0;
                for k in 0..=12 {
                    let rh = k as f32 * 0.05;
                    let t = c.temperature(sin_lat.asin(), relative_altitude(rh), None);
                    let m = land_material(&c, &Hydro::default(), false, true, rh, sin_lat);
                    if m == VoxelType::Grass {
                        assert!((FREEZE_C..=DESERT_C).contains(&t), "herbe a {t} C");
                    }
                    // Pas de neige sur un monde sans eau
                    assert_ne!(land_material(&c, &Hydro::DRY, false, true, rh, sin_lat), VoxelType::Snow);
                }
                let t0 = c.temperature(sin_lat.asin(), 0.0, None);
                if sea_material(&c, &water, false, sin_lat) == Some(VoxelType::Water) {
                    assert!((FREEZE_C..=SCORCH_C).contains(&t0), "eau liquide a {t0} C");
                }
            }
        }
        // Sans air ni eau : ni herbe, ni mer, ni neige ; sans air du tout : rien que de la roche
        let c = earth();
        assert_eq!(sea_material(&c, &Hydro::DRY, false, 0.0), None);
        assert_ne!(land_material(&c, &Hydro::default(), false, false, 0.1, 0.0), VoxelType::Grass);
        assert_eq!(land_material(&c, &Hydro::default(), true, false, 0.1, 0.0), VoxelType::Stone);
        let frozen = Climate { mean_c: -60.0, ..earth() };
        assert_ne!(land_material(&frozen, &Hydro::DRY, false, true, 0.1, 0.9), VoxelType::Snow);
        assert_eq!(land_material(&frozen, &Hydro::default(), false, true, 0.1, 0.9), VoxelType::Snow);
    }
}

#[cfg(test)]
mod exotic_tests {
    use super::*;

    #[test]
    fn exotic_seas_use_their_own_material_and_freeze_point() {
        let titan = Climate { mean_c: -179.0, span: 3.0, lapse: 0.0, diurnal: 1.0, tilt: 0.0 };
        let methane = Hydro { liquid: Liquid::Methane, freeze_c: -182.0, boil_c: -155.0, snow: true, co2_frost: false };
        assert_eq!(sea_material(&titan, &methane, false, 0.0), Some(VoxelType::Methane));
        let lava = Hydro { liquid: Liquid::Lava, freeze_c: 1_000.0, boil_c: 3_000.0, snow: false, co2_frost: false };
        let hell = Climate { mean_c: 1_400.0, span: 200.0, ..titan };
        assert_eq!(sea_material(&hell, &lava, false, 0.0), Some(VoxelType::Lava));
        // Aux pôles d'un monde de lave plus tiède, le magma fige
        let warm = Climate { mean_c: 1_050.0, span: 300.0, ..titan };
        assert_eq!(sea_material(&warm, &lava, false, 1.0), Some(VoxelType::Stone));
        // Banquise d'eau aux pôles d'une Terre froide
        let cold = Climate { mean_c: -5.0, span: 50.0, ..titan };
        assert_eq!(sea_material(&cold, &Hydro::default(), false, 1.0), Some(VoxelType::Ice));
    }
}
