//! Climat d'un astre (phase 3) : température selon la latitude et l'altitude (et, à partir de la
//! 0.11, l'heure et la saison), et les matières de surface qui en découlent.
//!
//! Remplace l'ancienne température unique : la neige, les déserts, les mers gelées ou asséchées
//! se décident colonne par colonne avec la température locale. Le maillage lointain (`mesher.rs`)
//! et le terrain voxel (`terrain.rs`) utilisent les mêmes fonctions.

use bevy::math::Vec3;
use serde::{Deserialize, Serialize};

use super::hydrology::{Hydro, Liquid};
use crate::planet::VoxelType;

/// Instant de la journée (0.11) : heure locale, 0 = minuit, 0,5 = midi. La saison est portée
/// par le climat lui-même (`Climate::season`, voir `Climate::at`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Moment {
    pub hour: f32,
}

/// Heure la plus chaude de la journée (fraction de jour) : le sol rend la chaleur avec retard,
/// le maximum vient vers 14 h 30 et le minimum vers 2 h 30, avant l'aube.
pub const DAY_PEAK: f32 = 14.5 / 24.0;

/// Saison du moment (0.11, A3), calculée par `world_clock::Spin::season`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Season {
    /// Déclinaison de l'étoile (radians) : la zone la plus chaude remonte vers l'hémisphère d'été.
    pub decl: f32,
    /// Écart de température dû à l'excentricité (K) : plus chaud au périhélie.
    pub offset: f32,
    /// Longitude (repère fixe de l'astre) où l'étoile est au zénith : donne l'heure locale de
    /// chaque point (givre du matin). `None` : moyenne de la journée.
    pub sun_lon: Option<f32>,
}

impl Season {
    /// Arrondie pour les maillages : on ne les reconstruit que si elle change vraiment (1° de
    /// déclinaison, 1 K, une heure de la planète).
    pub fn quantized(self, with_hour: bool) -> Self {
        let step = std::f32::consts::TAU / 24.0;
        Self {
            decl: (self.decl.to_degrees().round()).to_radians(),
            offset: self.offset.round(),
            sun_lon: if with_hour { self.sun_lon.map(|l| (l / step).round() * step) } else { None },
        }
    }
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
    /// Saison du moment (jamais sauvée : elle vient de l'horloge du monde). Par défaut, la
    /// moyenne de l'année.
    #[serde(skip)]
    pub season: Season,
    /// Œil (0.14 X1, monde verrouillé) : la température suit l'angle au point sous l'étoile (+X du repère de
    /// l'astre), plus la latitude. Face jour brûlée, face nuit gelée, bande du crépuscule entre les deux.
    #[serde(default)]
    pub eye: bool,
}

impl Default for Climate {
    fn default() -> Self {
        Self { mean_c: 15.0, span: 50.0, lapse: 50.0, diurnal: 10.0, tilt: 23.0, season: Season::default(), eye: false }
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

    /// « Latitude » climatique de la direction `dir` (repère de l'astre) : la vraie latitude, ou pour un œil
    /// la moitié de l'angle au point sous l'étoile (0 sous l'étoile, 45° au crépuscule, 90° à l'opposé).
    ///
    /// Bordures (0.14) : la latitude est un peu décalée par `border_jitter`, pour que les limites de biomes, de
    /// neige et de banquise soient irrégulières et que les deux côtés se mélangent en dégradé.
    pub fn lat_of(&self, dir: Vec3) -> f32 {
        let d = dir.normalize_or(Vec3::Y);
        let lat = if self.eye { d.x.clamp(-1.0, 1.0).acos() * 0.5 } else { d.y.clamp(-1.0, 1.0).asin() };
        let j = border_jitter(d, self.mean_c.to_bits() ^ self.span.to_bits().rotate_left(7));
        // (le signe de la latitude est gardé : l'hémisphère ne change pas)
        if lat >= 0.0 { (lat + j).clamp(0.0, std::f32::consts::FRAC_PI_2) } else { (lat - j).clamp(-std::f32::consts::FRAC_PI_2, 0.0) }
    }

    /// Sinus de `lat_of` (pour les fonctions qui prennent le sinus de la latitude).
    pub fn sin_lat(&self, dir: Vec3) -> f32 {
        self.lat_of(dir).sin()
    }

    /// Le même climat à une saison donnée.
    pub fn at(mut self, season: Season) -> Self {
        self.season = season;
        self
    }

    /// Température (°C) à la latitude `lat` (radians, signée) et à l'altitude relative `alt`
    /// (0 = niveau de la mer, 1 = sommet du relief), à la saison du climat. `moment` : heure
    /// locale (0.11) ; `None` = moyenne de la journée.
    ///
    /// La moyenne sur toute la sphère (et sur l'année) vaut `mean_c` : sin² de la latitude vaut 1/3.
    pub fn temperature(&self, lat: f32, alt: f32, moment: Option<Moment>) -> f32 {
        let tau = std::f32::consts::TAU;
        // La zone la plus chaude suit l'étoile (déclinaison) ; le maximum du jour vient après midi
        // (un œil n'a pas de saison qui déplace la zone chaude : elle reste sous l'étoile)
        let lat = if self.eye { lat } else { lat - self.season.decl };
        let daily = moment.map_or(0.0, |m| self.diurnal * (tau * (m.hour - DAY_PEAK)).cos());
        let s = lat.sin();
        self.mean_c + self.span * (1.0 / 3.0 - s * s) - self.lapse * alt.clamp(0.0, 1.0) + self.season.offset + daily
    }

    /// Heure locale (0..1) dans la direction `dir` (repère fixe de l'astre), si la saison porte
    /// la position de l'étoile.
    pub fn local_hour(&self, dir: Vec3) -> Option<f32> {
        let lon = dir.x.atan2(dir.z);
        self.season.sun_lon.map(|s| (0.5 + (lon - s) / std::f32::consts::TAU).rem_euclid(1.0))
    }

    /// Givre du matin : entre l'aube et le milieu de la matinée, là où la nuit est descendue sous
    /// 0 °C et où il ne fait pas encore chaud (les zones gelées toute la journée sont déjà
    /// enneigées).
    pub fn frost_at(&self, dir: Vec3, alt: f32) -> bool {
        let Some(hour) = self.local_hour(dir) else { return false };
        if !(3.5 / 24.0..9.5 / 24.0).contains(&hour) {
            return false;
        }
        let lat = self.lat_of(dir);
        let coldest = self.temperature(lat, alt, Some(Moment { hour: DAY_PEAK - 0.5 }));
        let now = self.temperature(lat, alt, Some(Moment { hour }));
        coldest < 0.0 && now < 4.0 && self.temperature(lat, alt, None) > FREEZE_C
    }

    /// Températures à l'équateur et aux pôles (niveau de la mer, moyenne).
    pub fn range(&self) -> (f32, f32) {
        (self.temperature(0.0, 0.0, None), self.temperature(std::f32::consts::FRAC_PI_2, 0.0, None))
    }
}

fn hash3(x: i32, y: i32, z: i32, salt: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8DA6_B343) ^ (y as u32).wrapping_mul(0xD816_3841) ^ (z as u32).wrapping_mul(0xCB1A_B31F) ^ salt.wrapping_mul(0x9E37_79B9);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h >> 8) as f32 / (1u32 << 24) as f32
}

/// Bruit de valeur 3D lissé, de -1 à 1.
fn value3(p: Vec3, salt: u32) -> f32 {
    let (i, f) = (p.floor(), p - p.floor());
    let u = f * f * (Vec3::splat(3.0) - 2.0 * f);
    let (x, y, z) = (i.x as i32, i.y as i32, i.z as i32);
    let c = |a: i32, b: i32, d: i32| hash3(x + a, y + b, z + d, salt);
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let x0 = lerp(lerp(c(0, 0, 0), c(1, 0, 0), u.x), lerp(c(0, 1, 0), c(1, 1, 0), u.x), u.y);
    let x1 = lerp(lerp(c(0, 0, 1), c(1, 0, 1), u.x), lerp(c(0, 1, 1), c(1, 1, 1), u.x), u.y);
    lerp(x0, x1, u.z) * 2.0 - 1.0
}

/// Décalage des bordures (radians de latitude, 0.14) : grandes ondulations (limites irrégulières, ~±3,5°) et
/// tirage par petite case (~±1,4°), qui mêle les blocs des deux côtés sur une bande : un dégradé de blocs.
/// Même fonction pour le sol proche et la vue de l'espace (règle 16).
pub fn border_jitter(dir: Vec3, salt: u32) -> f32 {
    let wave = value3(dir * 7.0, salt) * 0.65 + value3(dir * 23.0, salt ^ 0x5151) * 0.35;
    let cell = dir * 900.0;
    let dither = hash3(cell.x.floor() as i32, cell.y.floor() as i32, cell.z.floor() as i32, salt ^ 0xD1D1) * 2.0 - 1.0;
    wave * 0.06 + dither * 0.025
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
    // Latitude signée : les saisons ne sont pas les mêmes au nord et au sud
    let t = climate.temperature(sin_lat.clamp(-1.0, 1.0).asin(), relative_altitude(rh), None);
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
    let t = climate.temperature(sin_lat.clamp(-1.0, 1.0).asin(), 0.0, None);
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
        Climate { mean_c: 15.0, span: 50.0, lapse: 50.0, diurnal: 10.0, tilt: 23.4, ..Default::default() }
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
        let afternoon = c.temperature(0.0, 0.0, Some(Moment { hour: DAY_PEAK }));
        let noon = c.temperature(0.0, 0.0, Some(Moment { hour: 0.5 }));
        let night = c.temperature(0.0, 0.0, Some(Moment { hour: 0.0 }));
        assert!(noon - night > 15.0);
        // Le plus chaud vient après midi
        assert!(afternoon > noon);
        // Été au nord : plus chaud à 45° N qu'à 45° S
        let summer = c.at(Season { decl: 23.4f32.to_radians(), ..Default::default() });
        assert!(summer.temperature(0.8, 0.0, None) > summer.temperature(-0.8, 0.0, None) + 10.0);
        // La moyenne de l'année reste la moyenne
        let winter = c.at(Season { decl: -23.4f32.to_radians(), ..Default::default() });
        let mean = (summer.temperature(0.8, 0.0, None) + winter.temperature(0.8, 0.0, None)) * 0.5;
        assert!((mean - c.temperature(0.8, 0.0, None)).abs() < 1.5);
    }

    #[test]
    fn frost_appears_in_the_morning_where_nights_freeze() {
        // Climat tempéré froid : nuits sous 0, journées douces
        let c = Climate { mean_c: 2.0, span: 10.0, lapse: 0.0, diurnal: 8.0, ..earth() };
        let dir = Vec3::new(0.0, 0.0, 1.0);
        // L'étoile au zénith à la longitude 0 : midi ici ; décalée de 6 h vers l'est : 6 h du matin
        let at = |hour: f32| c.at(Season { sun_lon: Some((0.5 - hour / 24.0) * std::f32::consts::TAU), ..Default::default() });
        assert!((at(6.0).local_hour(dir).unwrap() * 24.0 - 6.0).abs() < 0.01);
        assert!(at(6.0).frost_at(dir, 0.0), "givre a 6 h");
        assert!(!at(14.0).frost_at(dir, 0.0), "fondu l'apres-midi");
        assert!(!at(23.0).frost_at(dir, 0.0), "pas le soir");
        // Sans heure (vue de loin) : pas de givre
        assert!(!c.frost_at(dir, 0.0));
        // Nuits douces : pas de givre
        let warm = Climate { mean_c: 25.0, ..c };
        assert!(!warm.at(at(6.0).season).frost_at(dir, 0.0));
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
        let titan = Climate { mean_c: -179.0, span: 3.0, lapse: 0.0, diurnal: 1.0, tilt: 0.0, ..Default::default() };
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
