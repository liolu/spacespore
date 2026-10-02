//! Atmosphère (phase 3) : rétention des gaz, composition, pression, albédo, effet de serre,
//! température d'équilibre, nuages, vents, couleur du ciel, du coucher de soleil et de la brume.
//!
//! - Rétention (fuite de Jeans) : un gaz reste si la vitesse de libération dépasse 5 fois la
//!   vitesse de ses molécules dans la haute atmosphère, chauffée par les rayons X de l'étoile
//!   (les planètes proches d'une naine rouge active perdent leur air).
//! - T_eq = [L (1 − A) / (16 π σ d²)]^¼ (Soleil, UA : 278,6 K × L^¼ (1 − A)^¼ / √d), puis effet
//!   de serre d'une atmosphère grise d'épaisseur optique τ : T = T_eq (1 + ¾ τ)^¼. τ est calé sur
//!   la Terre (+33 K), Vénus (+500 K), Mars (+~0) et Titan (+~12 K). La vapeur d'eau suit la
//!   température (Clausius-Clapeyron) : quelques itérations.
//! - Ciel : diffusion de Rayleigh (∝ 1/λ⁴) de la lumière de l'étoile, blanchie par une atmosphère
//!   épaisse, teintée par les brumes (méthane, soufre, poussière, gaz fictifs).
//!
//! Gaz réels et fictifs sont étiquetés (règle 5).

use serde::{Deserialize, Serialize};

use super::climate::Climate;
use super::profile::Realism;
use super::seeds::LayerRng;
use super::system::PlanetKind;

/// Un gaz : masse molaire, pouvoir de serre (par bar, voir `optical_depth`), brume.
pub struct Gas {
    pub formula: &'static str,
    pub name: &'static str,
    pub molar_mass: f64,
    pub greenhouse: f64,
    /// Couleur de la brume qu'il forme, et sa force (0 = transparent).
    pub haze: [f32; 3],
    pub haze_strength: f32,
    pub realism: Realism,
}

const fn gas(formula: &'static str, name: &'static str, molar_mass: f64, greenhouse: f64, haze: [f32; 3], haze_strength: f32, realism: Realism) -> Gas {
    Gas { formula, name, molar_mass, greenhouse, haze, haze_strength, realism }
}

const R: Realism = Realism::Realistic;
const F: Realism = Realism::Fictional;
const CLEAR: [f32; 3] = [1.0, 1.0, 1.0];

pub const GASES: [Gas; 13] = [
    gas("N2", "diazote", 28.0, 0.1, CLEAR, 0.0, R),
    gas("O2", "dioxygene", 32.0, 0.02, CLEAR, 0.0, R),
    gas("CO2", "dioxyde de carbone", 44.0, 0.39, CLEAR, 0.0, R),
    gas("CH4", "methane", 16.0, 10.0, [0.95, 0.6, 0.3], 0.7, R),
    gas("H2", "dihydrogene", 2.0, 0.5, CLEAR, 0.0, R),
    gas("He", "helium", 4.0, 0.0, CLEAR, 0.0, R),
    gas("Ar", "argon", 40.0, 0.0, CLEAR, 0.0, R),
    gas("H2O", "vapeur d'eau", 18.0, 100.0, [0.95, 0.95, 0.97], 0.1, R),
    gas("NH3", "ammoniac", 17.0, 20.0, [0.95, 0.9, 0.75], 0.4, R),
    gas("SO2", "dioxyde de soufre", 64.0, 5.0, [0.95, 0.85, 0.45], 0.8, R),
    gas("Ae", "aetherion (lumineux)", 60.0, 2.0, [0.7, 0.45, 1.0], 0.9, F),
    gas("Sp", "sporogaz", 35.0, 1.0, [0.55, 0.9, 0.5], 0.8, F),
    gas("Cx", "chromex", 90.0, 8.0, [1.0, 0.45, 0.6], 0.9, F),
];

pub fn gas_info(formula: &str) -> Option<&'static Gas> {
    GASES.iter().find(|g| g.formula == formula)
}

/// Type de nuages.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CloudKind {
    #[default]
    None,
    /// Eau : blancs.
    Water,
    /// Glace carbonique : fins, pâles.
    CarbonDioxide,
    /// Acide sulfurique (Vénus) : voile jaune continu.
    Sulfuric,
    /// Méthane et brume organique (Titan) : orangés.
    Methane,
    /// Ammoniac (géantes) : crème.
    Ammonia,
    /// Gaz fictifs : colorés.
    Exotic,
}

impl CloudKind {
    pub fn name(self) -> &'static str {
        match self {
            CloudKind::None => "aucun",
            CloudKind::Water => "eau",
            CloudKind::CarbonDioxide => "glace carbonique",
            CloudKind::Sulfuric => "acide sulfurique",
            CloudKind::Methane => "methane",
            CloudKind::Ammonia => "ammoniac",
            CloudKind::Exotic => "exotiques",
        }
    }
}

/// Atmosphère calculée d'une planète.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Air {
    /// Pression au sol (bar) ; pour une géante, au niveau des nuages (1 bar).
    pub pressure_bar: f32,
    /// (formule, fraction) par ordre décroissant ; vide = pas d'atmosphère.
    pub gases: Vec<(String, f32)>,
    pub albedo: f32,
    /// Température d'équilibre sans effet de serre (K).
    pub t_eq_k: f32,
    pub greenhouse_k: f32,
    pub clouds: CloudKind,
    pub cloud_cover: f32,
    pub cloud_color: [f32; 3],
    /// Vent moyen (m/s) et circulation générale.
    pub wind_ms: f32,
    pub winds: String,
    /// Ciel de jour, coucher de soleil, brume de l'horizon (sRGB).
    pub sky: [f32; 3],
    pub sunset: [f32; 3],
    pub haze: [f32; 3],
}

impl Air {
    /// Une atmosphère qui compte (ciel, nuages, eau liquide possible).
    pub fn present(&self) -> bool {
        self.pressure_bar >= 0.01 && !self.gases.is_empty()
    }

    pub fn fraction(&self, formula: &str) -> f32 {
        self.gases.iter().find(|(f, _)| f == formula).map_or(0.0, |(_, x)| *x)
    }
}

/// Données d'entrée : la planète (phase 2) et son étoile (phase 1).
pub struct AirInput {
    pub kind: PlanetKind,
    pub mass: f64,
    pub radius: f64,
    pub au: f64,
    pub luminosity: f64,
    /// Rayons X de l'étoile (Soleil = 1).
    pub xray: f64,
    pub star_color: [f32; 3],
    pub locked: bool,
    pub rotation_h: f64,
    pub axial_tilt: f64,
}

/// Vitesse de libération (km/s).
pub fn escape_velocity(mass: f64, radius: f64) -> f64 {
    11.186 * (mass / radius.max(1e-6)).max(0.0).sqrt()
}

/// T_eq (K) à `au` UA d'une étoile de luminosité `l` (L☉), albédo `a`.
pub fn equilibrium_temperature(l: f64, au: f64, a: f64) -> f64 {
    278.6 * l.max(1e-9).powf(0.25) * (1.0 - a).clamp(0.0, 1.0).powf(0.25) / au.max(1e-4).sqrt()
}

/// Température de la haute atmosphère (K) : ~3 fois T_eq, bien plus sous les rayons X d'une étoile
/// active proche (`xray` : flux reçu, Terre = 1). Terre : ~900 K.
pub fn exosphere_temperature(t_eq: f64, xray: f64) -> f64 {
    t_eq * 3.0 * (1.0 + xray.max(0.0)).powf(0.25)
}

/// Le gaz de masse molaire `m` reste, avec une haute atmosphère à `t_exo` K ?
pub fn retained(v_esc: f64, t_exo: f64, m: f64) -> bool {
    v_esc > 5.0 * 0.158 * (t_exo / m).sqrt()
}

/// Épaisseur optique infrarouge (atmosphère grise) d'un mélange à `pressure` bar.
pub fn optical_depth(gases: &[(String, f32)], pressure: f64) -> f64 {
    let p = pressure.max(0.0).powf(1.3);
    gases.iter().map(|(f, x)| gas_info(f).map_or(0.0, |g| g.greenhouse) * *x as f64 * p).sum()
}

/// Fraction de vapeur d'eau à la température `t` (K) sous `pressure` bar (humidité 50 %), plafonnée
/// à 4 % : au-delà, la vapeur monte et se perd (serre humide) au lieu d'emballer le climat.
fn vapour_fraction(t: f64, pressure: f64) -> f64 {
    let p_h2o = 0.017 * 0.5 * (0.0625 * (t.min(450.0) - 288.0)).exp();
    (p_h2o / pressure.max(1e-6)).min(0.04)
}

fn normalize(mut gases: Vec<(String, f32)>) -> Vec<(String, f32)> {
    gases.retain(|(_, x)| *x > 0.0);
    let total: f32 = gases.iter().map(|(_, x)| x).sum();
    if total > 0.0 {
        for g in &mut gases {
            g.1 /= total;
        }
    }
    gases.sort_by(|a, b| b.1.total_cmp(&a.1));
    gases
}

fn mix3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

fn scaled(c: [f32; 3]) -> [f32; 3] {
    let m = c[0].max(c[1]).max(c[2]).max(1e-4);
    [c[0] / m, c[1] / m, c[2] / m]
}

/// Poids de Rayleigh (1/λ⁴) pour le rouge, le vert et le bleu (680, 550, 440 nm).
const RAYLEIGH: [f32; 3] = [0.177, 0.405, 1.0];

/// Couleurs du ciel, du coucher de soleil et de la brume.
pub fn sky_colors(star: [f32; 3], gases: &[(String, f32)], pressure: f64, dusty: bool) -> ([f32; 3], [f32; 3], [f32; 3]) {
    let p = pressure.max(0.0) as f32;
    // Diffusion de Rayleigh de la lumière de l'étoile
    let rayleigh = scaled([star[0] * RAYLEIGH[0], star[1] * RAYLEIGH[1], star[2] * RAYLEIGH[2]]);
    // La diffusion multiple blanchit le ciel d'une atmosphère épaisse ; une atmosphère ténue est sombre
    let white = (0.25 + 0.3 * (p.max(1e-3).log10() + 0.5)).clamp(0.1, 0.9);
    let mut sky = mix3(rayleigh, scaled(star), white);
    let brightness = (p / 0.3).sqrt().clamp(0.15, 1.0);
    // Brumes : méthane orangé, soufre jaune, poussière (air ténu et sec), gaz fictifs
    let (mut haze, mut strength) = ([0.0f32; 3], 0.0f32);
    for (f, x) in gases {
        if let Some(g) = gas_info(f) {
            // Quelques % suffisent (la brume orangée de Titan vient de 5 % de méthane)
            let w = g.haze_strength * (*x * 20.0).min(1.0);
            haze = [haze[0] + g.haze[0] * w, haze[1] + g.haze[1] * w, haze[2] + g.haze[2] * w];
            strength += w;
        }
    }
    if dusty {
        let w = 0.8;
        haze = [haze[0] + 0.78 * w, haze[1] + 0.56 * w, haze[2] + 0.4 * w];
        strength += w;
    }
    let haze_color = if strength > 0.0 { scaled([haze[0] / strength, haze[1] / strength, haze[2] / strength]) } else { sky };
    sky = mix3(sky, haze_color, (strength * 0.8).min(0.85));
    let sky = [sky[0] * brightness * 0.92, sky[1] * brightness * 0.92, sky[2] * brightness * 0.92];
    // Coucher de soleil : la lumière traverse beaucoup d'air, le bleu est diffusé en route
    let path = 3.0 * p.max(0.01).powf(0.5);
    let sunset = scaled([
        star[0] * (-path * RAYLEIGH[0]).exp(),
        star[1] * (-path * RAYLEIGH[1]).exp(),
        star[2] * (-path * RAYLEIGH[2]).exp(),
    ]);
    let sunset = mix3(sunset, haze_color, (strength * 0.4).min(0.5));
    let horizon = mix3(sky, haze_color, 0.5);
    (sky, sunset, horizon)
}

/// Atmosphère et climat d'une planète.
/// Écart jour / nuit (demi-amplitude, K) : grand sans air (Lune ~ ±150 K, Mars ~ ±40 K), amorti
/// par l'atmosphère (Terre ~ ±5 K, Vénus ~ rien), plus fort quand les jours sont longs.
pub fn diurnal_amplitude(t_surface_k: f32, pressure_bar: f32, rotation_h: f32) -> f32 {
    t_surface_k * 0.24 * (rotation_h.max(0.24) / 24.0).powf(0.25).clamp(0.6, 3.0) / (1.0 + 11.6 * pressure_bar.max(0.0).powf(0.75))
}

pub fn generate(input: &AirInput, rng: &mut LayerRng) -> (Air, Climate) {
    let v_esc = escape_velocity(input.mass, input.radius);
    let au = input.au.max(1e-4);
    let gaseous = input.kind.gaseous();
    let t_eq_raw = equilibrium_temperature(input.luminosity, au, 0.3);
    // Haute atmosphère chauffée par les rayons X reçus (Terre : ~1 000 K)
    let xray_here = input.xray / (au * au);
    let t_exo = exosphere_temperature(t_eq_raw, xray_here);

    // ── Composition et pression de départ selon le type de monde ─────────
    let roll = rng.unit();
    let (mut gases, mut pressure): (Vec<(String, f32)>, f64) = if gaseous {
        let cold = t_eq_raw < 150.0;
        let mut g = vec![("H2".to_string(), 0.86), ("He".to_string(), 0.13)];
        if t_eq_raw < 400.0 {
            g.push(("CH4".to_string(), if input.kind == PlanetKind::GasGiant { 0.003 } else { 0.02 }));
        }
        if cold {
            g.push(("NH3".to_string(), 0.0005));
        }
        g.push(("H2O".to_string(), 0.001));
        (g, 1.0)
    } else if input.mass < 0.02 || roll < 0.25 {
        // Trop petite, ou pas assez de gaz rejetés par ses volcans
        (Vec::new(), 0.0)
    } else if t_eq_raw > 400.0 {
        if rng.unit() < 0.5 {
            // Serre emballée (Vénus) : CO2 épais, nuages d'acide sulfurique
            (vec![("CO2".into(), 0.965), ("N2".into(), 0.035), ("SO2".into(), 0.000_15)], 30.0 + 120.0 * rng.unit())
        } else {
            (vec![("CO2".into(), 0.6), ("SO2".into(), 0.3), ("N2".into(), 0.1)], 10f64.powf(rng.range(-3.0, -0.5)))
        }
    } else if t_eq_raw > 180.0 {
        // Tempérée : azote, un peu de CO2 et d'argon, vapeur d'eau ; parfois de l'oxygène (vie ?)
        let mut g = vec![("N2".to_string(), 0.78), ("Ar".to_string(), 0.01), ("CO2".to_string(), 0.0004 + 0.02 * rng.unit().powi(3) as f32)];
        if rng.unit() < 0.25 {
            g.push(("O2".to_string(), 0.21));
        }
        (g, 10f64.powf(rng.range(-1.3, 0.7)) * input.mass.powf(0.6))
    } else if rng.unit() < 0.5 {
        // Froide et épaisse (Titan) : azote et méthane
        (vec![("N2".into(), 0.95), ("CH4".into(), 0.05)], 0.5 + 2.5 * rng.unit())
    } else {
        // Froide et ténue (Mars) : CO2
        (vec![("CO2".into(), 0.95), ("N2".into(), 0.027), ("Ar".into(), 0.02)], 10f64.powf(rng.range(-2.5, -1.3)))
    };
    // Gaz fictifs, rares (règle 5 : étiquetés)
    if !gaseous && !gases.is_empty() && rng.unit() < 0.03 {
        let fictional = ["Ae", "Sp", "Cx"][(rng.unit() * 2.999) as usize];
        gases.push((fictional.to_string(), rng.range(0.01, 0.15) as f32));
    }

    // ── Rétention : les gaz trop légers s'échappent, la pression baisse d'autant ──
    if !gaseous {
        let before: f32 = gases.iter().map(|(_, x)| x).sum();
        gases.retain(|(f, _)| gas_info(f).is_some_and(|g| retained(v_esc, t_exo, g.molar_mass)));
        let after: f32 = gases.iter().map(|(_, x)| x).sum();
        pressure *= if before > 0.0 { (after / before) as f64 } else { 0.0 };
        if pressure < 1e-4 {
            gases.clear();
            pressure = 0.0;
        }
    }
    let wet = !gaseous && pressure >= 0.01 && gases.iter().any(|(f, _)| f == "N2" || f == "CO2") && t_eq_raw > 180.0 && t_eq_raw < 400.0;

    // ── Effet de serre (avec la vapeur d'eau, qui suit la température) et albédo ──
    let sulfuric = gases.iter().any(|(f, _)| f == "SO2") && pressure > 5.0;
    let mut t_surface = t_eq_raw;
    let mut cover = 0.0f64;
    let mut albedo = 0.12;
    let mut t_eq = t_eq_raw;
    for _ in 0..8 {
        let mut mix = gases.clone();
        if wet {
            let x = vapour_fraction(t_surface, pressure);
            mix.retain(|(f, _)| f != "H2O");
            mix.push(("H2O".into(), x as f32));
        }
        cover = if gaseous {
            0.9
        } else if sulfuric {
            1.0
        } else if wet {
            (0.25 + 0.5 * (t_surface - 250.0) / 80.0).clamp(0.1, 0.75)
        } else if pressure > 0.3 {
            0.3
        } else {
            0.05 * (pressure / 0.01).min(1.0)
        };
        let icy = !gaseous && pressure > 0.0 && t_surface < 250.0 && wet;
        albedo = if gaseous { 0.35 } else { 0.12 + 0.3 * cover + if sulfuric { 0.3 } else { 0.0 } + if icy { 0.15 } else { 0.0 } };
        t_eq = equilibrium_temperature(input.luminosity, au, albedo);
        let tau = if gaseous { 0.0 } else { optical_depth(&mix, pressure) };
        t_surface = (t_eq * (1.0 + 0.75 * tau).powf(0.25)).min(1_500.0);
        if wet {
            gases = normalize(mix);
        }
    }
    // Une géante rayonne aussi sa chaleur interne (Jupiter : ~+40 % au niveau 1 bar)
    if gaseous {
        t_surface = t_eq * 1.3;
    }
    let gases = normalize(gases);
    let greenhouse = t_surface - t_eq;

    // ── Nuages ──────────────────────────────────────────────────────────
    let has = |f: &str| gases.iter().any(|(g, x)| g == f && *x > 0.0005);
    let fictional = gases.iter().find(|(f, x)| *x > 0.01 && gas_info(f).is_some_and(|g| g.realism == Realism::Fictional));
    let clouds = if pressure < 0.005 && !gaseous {
        CloudKind::None
    } else if fictional.is_some() {
        CloudKind::Exotic
    } else if sulfuric {
        CloudKind::Sulfuric
    } else if gaseous && has("NH3") {
        CloudKind::Ammonia
    } else if has("CH4") && t_surface < 200.0 {
        CloudKind::Methane
    } else if wet && (230.0..400.0).contains(&t_surface) {
        CloudKind::Water
    } else if has("CO2") && t_surface < 220.0 {
        CloudKind::CarbonDioxide
    } else {
        CloudKind::None
    };
    let cloud_color = match clouds {
        CloudKind::Water | CloudKind::None => [0.95, 0.95, 0.97],
        CloudKind::CarbonDioxide => [0.9, 0.92, 0.98],
        CloudKind::Sulfuric => [0.95, 0.88, 0.6],
        CloudKind::Methane => [0.9, 0.65, 0.35],
        CloudKind::Ammonia => [0.95, 0.9, 0.78],
        CloudKind::Exotic => fictional.and_then(|(f, _)| gas_info(f)).map_or([0.8, 0.6, 1.0], |g| g.haze),
    };
    if clouds == CloudKind::None {
        cover = 0.0;
    }

    // ── Climat : écarts de température, vents ──────────────────────────
    let p = pressure as f32;
    let mean_c = (t_surface - 273.15) as f32;
    // Une atmosphère épaisse répartit la chaleur ; une face toujours éclairée creuse l'écart
    let span = (t_surface as f32 * 0.35 / (1.0 + p)) * if input.locked { 1.6 } else { 1.0 };
    let climate = Climate {
        mean_c,
        span,
        lapse: if p >= 0.01 && !gaseous { 50.0 * (p.min(1.0)).powf(0.3) * (input.mass / input.radius.powi(2)).sqrt() as f32 } else { 0.0 },
        diurnal: diurnal_amplitude(t_surface as f32, p, input.rotation_h as f32),
        tilt: input.axial_tilt as f32,
        season: Default::default(),
    };
    let wind_ms = if pressure < 0.001 {
        0.0
    } else if gaseous {
        100.0 + 50.0 * rng.unit() as f32
    } else {
        (8.0 * (1.0 + span / 40.0) * (1.0 + p.min(100.0).powf(0.3)) * 0.5).min(120.0)
    };
    let winds = if wind_ms == 0.0 {
        "aucun".to_string()
    } else if gaseous {
        "jets alternes en bandes (est-ouest)".to_string()
    } else if input.locked {
        "du cote jour vers le cote nuit".to_string()
    } else if p > 30.0 {
        "superrotation : toute l'atmosphere tourne plus vite que le sol".to_string()
    } else if input.rotation_h < 30.0 {
        "trois cellules par hemisphere (Hadley, Ferrel, polaire), alizes et vents d'ouest".to_string()
    } else {
        "une grande cellule de Hadley de l'equateur aux poles".to_string()
    };

    let dusty = !gaseous && pressure > 0.0 && pressure < 0.1 && !wet;
    let (sky, sunset, haze) = sky_colors(input.star_color, &gases, if gaseous { 1.0 } else { pressure }, dusty);
    let air = Air {
        pressure_bar: pressure as f32,
        gases: gases.iter().map(|(f, x)| (f.clone(), *x)).collect(),
        albedo: albedo as f32,
        t_eq_k: t_eq as f32,
        greenhouse_k: greenhouse as f32,
        clouds,
        cloud_cover: cover as f32,
        cloud_color,
        wind_ms,
        winds,
        sky,
        sunset,
        haze,
    };
    (air, climate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planetgen::seeds::Layer;

    fn input(kind: PlanetKind, mass: f64, radius: f64, au: f64) -> AirInput {
        AirInput {
            kind,
            mass,
            radius,
            au,
            luminosity: 1.0,
            xray: 1.0,
            star_color: [1.0, 0.95, 0.9],
            locked: false,
            rotation_h: 24.0,
            axial_tilt: 23.0,
        }
    }

    #[test]
    fn the_equilibrium_temperature_formula() {
        // Terre : 255 K avec un albédo de 0,3 ; Vénus (A = 0,75, 0,72 UA) : ~232 K
        assert!((equilibrium_temperature(1.0, 1.0, 0.3) - 255.0).abs() < 1.0);
        assert!((equilibrium_temperature(1.0, 0.723, 0.75) - 232.0).abs() < 2.0);
    }

    #[test]
    fn earth_keeps_nitrogen_and_loses_hydrogen() {
        let t_exo = exosphere_temperature(255.0, 1.0);
        let v = escape_velocity(1.0, 1.0);
        assert!(retained(v, t_exo, 28.0) && retained(v, t_exo, 18.0) && retained(v, t_exo, 44.0));
        assert!(!retained(v, t_exo, 2.0));
        // La Lune et Mercure ne gardent rien de courant, Mars garde son CO2, Titan son azote
        assert!(!retained(escape_velocity(0.0123, 0.273), 255.0 * 3.5, 44.0));
        assert!(!retained(escape_velocity(0.055, 0.383), 440.0 * 3.0, 44.0));
        assert!(retained(escape_velocity(0.107, 0.532), 210.0 * 3.5, 44.0));
        assert!(retained(escape_velocity(0.0225, 0.404), 82.0 * 3.0, 28.0));
    }

    #[test]
    fn greenhouse_is_calibrated_on_venus_earth_and_mars() {
        let venus = vec![("CO2".to_string(), 0.965f32), ("N2".to_string(), 0.035)];
        let t = 232.0 * (1.0 + 0.75 * optical_depth(&venus, 92.0)).powf(0.25);
        assert!((t - 737.0).abs() < 60.0, "venus {t}");
        let earth = vec![("N2".to_string(), 0.78f32), ("O2".to_string(), 0.21), ("H2O".to_string(), 0.0085), ("CO2".to_string(), 0.0004)];
        let t = 255.0 * (1.0 + 0.75 * optical_depth(&earth, 1.0)).powf(0.25);
        assert!((t - 288.0).abs() < 6.0, "terre {t}");
        let mars = vec![("CO2".to_string(), 0.95f32)];
        let t = 210.0 * (1.0 + 0.75 * optical_depth(&mars, 0.006)).powf(0.25);
        assert!(t - 210.0 < 3.0, "mars {t}");
    }

    #[test]
    fn earth_like_worlds_get_a_blue_sky_and_red_sunsets() {
        let earth = vec![("N2".to_string(), 0.78f32), ("O2".to_string(), 0.21)];
        let (sky, sunset, _) = sky_colors([1.0, 0.95, 0.9], &earth, 1.0, false);
        assert!(sky[2] > sky[1] && sky[1] > sky[0], "ciel {sky:?}");
        assert!(sunset[0] > sunset[1] && sunset[1] > sunset[2], "coucher {sunset:?}");
        // Titan : brume orangée
        let titan = vec![("N2".to_string(), 0.95f32), ("CH4".to_string(), 0.05)];
        let (sky, _, _) = sky_colors([1.0, 0.95, 0.9], &titan, 1.5, false);
        assert!(sky[0] > sky[2], "titan {sky:?}");
        // Mars : poussière ocre, ciel sombre
        let (sky, _, _) = sky_colors([1.0, 0.95, 0.9], &[("CO2".to_string(), 0.95)], 0.006, true);
        assert!(sky[0] > sky[2] && sky[0] < 0.5, "mars {sky:?}");
    }

    #[test]
    fn generated_atmospheres_are_coherent() {
        let mut seen_venus = false;
        let mut seen_earth = false;
        for i in 0..3000u64 {
            let mut rng = LayerRng::new(i, Layer::Atmosphere);
            let au = 0.3 + (i % 40) as f64 * 0.1;
            let mass = 0.05 + (i % 13) as f64 * 0.4;
            let radius = mass.powf(0.279);
            let (air, climate) = generate(&input(PlanetKind::Rocky, mass, radius, au), &mut rng);
            let sum: f32 = air.gases.iter().map(|(_, x)| x).sum();
            if air.gases.is_empty() {
                assert_eq!(air.pressure_bar, 0.0);
                assert_eq!(air.clouds, CloudKind::None);
            } else {
                assert!((sum - 1.0).abs() < 1e-3, "{air:?}");
            }
            assert!(air.greenhouse_k >= -0.01 && air.albedo > 0.0 && air.albedo < 0.9);
            assert!(climate.mean_c > -273.0 && climate.mean_c < 1_300.0);
            // Plus d'air, moins d'écart entre l'équateur et les pôles
            if air.pressure_bar > 30.0 {
                assert!(climate.span < 10.0);
                seen_venus |= air.clouds == CloudKind::Sulfuric && climate.mean_c > 300.0;
            }
            if air.clouds == CloudKind::Water {
                seen_earth = true;
                assert!((-60.0..130.0).contains(&climate.mean_c), "{climate:?}");
            }
        }
        assert!(seen_venus && seen_earth);
    }

    #[test]
    fn giants_are_hydrogen_and_helium() {
        let mut rng = LayerRng::new(5, Layer::Atmosphere);
        let (air, _) = generate(&input(PlanetKind::GasGiant, 318.0, 11.2, 5.2), &mut rng);
        assert_eq!(air.gases[0].0, "H2");
        assert!(air.fraction("He") > 0.1);
        assert_eq!(air.clouds, CloudKind::Ammonia);
    }

    #[test]
    fn red_dwarf_flares_strip_close_planets() {
        // Planète terrestre à 0,03 UA d'une naine rouge très active : elle perd son azote
        let t_eq = equilibrium_temperature(0.001, 0.03, 0.3);
        let t_exo = exosphere_temperature(t_eq, 30.0 / (0.03 * 0.03));
        assert!(!retained(escape_velocity(1.0, 1.0), t_exo, 28.0));
    }
}
