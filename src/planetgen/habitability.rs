//! Habitabilité et dangers (phase 9).
//!
//! Score de 0 à 1, produit de facteurs (chacun de 0 à 1) : température, eau liquide, pression,
//! radiation, gravité, toxicité de l'air. Il dit si un humain pourrait y vivre, pas s'il y a de la
//! vie (la phase 7 en décide indépendamment).
//!
//! Dangers : chacun noté de 1 (gênant) à 3 (mortel) ; le niveau de danger de l'astre est le pire.

use serde::{Deserialize, Serialize};

/// Ce qu'il faut savoir d'un astre pour juger de son habitabilité.
pub struct HabInput<'a> {
    pub gaseous: bool,
    pub mean_c: f32,
    pub equator_c: f32,
    pub pole_c: f32,
    pub pressure: f32,
    pub gases: &'a [(String, f32)],
    pub liquid_water: bool,
    pub ocean_fraction: f32,
    /// Radiation au sol (0..1).
    pub radiation: f32,
    pub gravity: f32,
    pub volcanism: f32,
    pub quakes: f32,
    pub wind_ms: f32,
    /// Mers de lave, nuages d'acide.
    pub lava: bool,
    pub acid: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Danger {
    pub name: String,
    /// 1 : gênant, 2 : dangereux, 3 : mortel.
    pub level: u8,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Habitability {
    pub score: f32,
    pub label: String,
    /// Facteurs du score (0 à 1) : température, eau, pression, radiation, gravité, air.
    pub temperature: f32,
    pub water: f32,
    pub pressure: f32,
    pub radiation: f32,
    pub gravity: f32,
    pub air: f32,
    pub dangers: Vec<Danger>,
    /// Le pire danger (0 : aucun).
    pub danger_level: u8,
}

fn gauss(x: f32, center: f32, width: f32) -> f32 {
    let d = (x - center) / width;
    (-0.5 * d * d).exp()
}

fn fraction(gases: &[(String, f32)], formula: &str) -> f32 {
    gases.iter().find(|(f, _)| f == formula).map_or(0.0, |(_, x)| *x)
}

/// Respirabilité de l'air (0 : toxique ou irrespirable, 1 : comme sur Terre).
pub fn breathability(gases: &[(String, f32)], pressure: f32) -> f32 {
    if gases.is_empty() || pressure < 0.01 {
        return 0.0;
    }
    let o2 = fraction(gases, "O2") * pressure;
    let co2 = fraction(gases, "CO2") * pressure;
    let toxic = fraction(gases, "SO2") + fraction(gases, "NH3") + fraction(gases, "Cx");
    // Pression partielle d'oxygène : 0,16 à 0,5 bar (Terre 0,21)
    let oxygen = if o2 > 0.0 { gauss(o2, 0.21, 0.1).max(if (0.16..0.5).contains(&o2) { 0.8 } else { 0.0 }) } else { 0.0 };
    let co2_ok = (1.0 - (co2 / 0.05)).clamp(0.0, 1.0);
    let toxic_ok = (1.0 - toxic * 1_000.0).clamp(0.0, 1.0);
    // Sans oxygène, on peut au moins y vivre sous masque : 0,3
    (oxygen.max(0.3) * co2_ok * toxic_ok).clamp(0.0, 1.0)
}

pub fn evaluate(h: &HabInput) -> Habitability {
    if h.gaseous {
        return Habitability {
            label: "inhabitable (geante gazeuse)".into(),
            dangers: vec![
                Danger { name: "pas de sol, pression ecrasante".into(), level: 3 },
                Danger { name: "vents violents".into(), level: 2 },
            ],
            danger_level: 3,
            ..Default::default()
        };
    }
    let temperature = gauss(h.mean_c, 15.0, 25.0) * if h.equator_c > 60.0 || h.pole_c < -80.0 { 0.7 } else { 1.0 };
    let water = if h.liquid_water { (0.4 + h.ocean_fraction).min(1.0) } else { 0.0 };
    let pressure = if h.pressure < 0.01 { 0.0 } else { gauss(h.pressure.log10(), 0.0, 0.6) };
    let radiation = (1.0 - h.radiation * 1.5).clamp(0.0, 1.0);
    let gravity = gauss(h.gravity, 1.0, 0.45);
    let air = breathability(h.gases, h.pressure);
    let score = (temperature * water.max(0.05) * pressure * radiation * gravity * (0.4 + 0.6 * air)).clamp(0.0, 1.0);
    let label = match score {
        s if s > 0.6 => "habitable",
        s if s > 0.3 => "vivable avec equipement",
        s if s > 0.05 => "hostile",
        _ => "inhabitable",
    };

    let mut dangers = Vec::new();
    let mut add = |name: &str, level: u8| {
        if level > 0 {
            dangers.push(Danger { name: name.into(), level: level.min(3) });
        }
    };
    add("chaleur extreme", match h.equator_c { t if t > 300.0 => 3, t if t > 80.0 => 2, t if t > 45.0 => 1, _ => 0 });
    add("froid extreme", match h.pole_c.min(h.mean_c) { t if t < -150.0 => 3, t if t < -60.0 => 2, t if t < -25.0 => 1, _ => 0 });
    add(if h.pressure < 0.5 { "vide ou air trop mince" } else { "pression ecrasante" }, match h.pressure {
        p if p < 0.01 => 3,
        p if p < 0.3 => 2,
        p if p > 50.0 => 3,
        p if p > 5.0 => 2,
        _ => 0,
    });
    add("radiation", match h.radiation { r if r > 0.6 => 3, r if r > 0.3 => 2, r if r > 0.1 => 1, _ => 0 });
    add("gravite forte", match h.gravity { g if g > 3.0 => 3, g if g > 2.0 => 2, g if g > 1.5 => 1, _ => 0 });
    add("air toxique ou irrespirable", if h.pressure < 0.01 { 0 } else if air < 0.1 { 2 } else if air < 0.5 { 1 } else { 0 });
    add("volcans et seismes", match h.volcanism.max(h.quakes * 0.5) { v if v > 0.7 => 2, v if v > 0.4 => 1, _ => 0 });
    add("tempetes", match h.wind_ms { w if w > 60.0 => 2, w if w > 30.0 => 1, _ => 0 });
    add("mers de lave", if h.lava { 3 } else { 0 });
    add("pluies acides", if h.acid { 2 } else { 0 });
    let danger_level = dangers.iter().map(|d| d.level).max().unwrap_or(0);

    Habitability { score, label: label.into(), temperature, water, pressure, radiation, gravity, air, dangers, danger_level }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn earth_air() -> Vec<(String, f32)> {
        vec![("N2".into(), 0.78), ("O2".into(), 0.21), ("Ar".into(), 0.01), ("CO2".into(), 0.0004)]
    }

    fn input(gases: &[(String, f32)]) -> HabInput<'_> {
        HabInput {
            gaseous: false,
            mean_c: 15.0,
            equator_c: 30.0,
            pole_c: -25.0,
            pressure: 1.0,
            gases,
            liquid_water: true,
            ocean_fraction: 0.7,
            radiation: 0.002,
            gravity: 1.0,
            volcanism: 0.3,
            quakes: 1.0,
            wind_ms: 10.0,
            lava: false,
            acid: false,
        }
    }

    #[test]
    fn earth_is_habitable_and_mars_and_venus_are_not() {
        let air = earth_air();
        let earth = evaluate(&input(&air));
        assert!(earth.score > 0.7 && earth.label == "habitable", "{earth:?}");
        assert!(earth.danger_level <= 1, "{earth:?}");

        let mars_air = vec![("CO2".to_string(), 0.95f32), ("N2".to_string(), 0.03)];
        let mars = evaluate(&HabInput { mean_c: -63.0, equator_c: -20.0, pole_c: -125.0, pressure: 0.006, liquid_water: false, ocean_fraction: 0.0, radiation: 0.3, gravity: 0.38, ..input(&mars_air) });
        assert!(mars.score < 0.05 && mars.danger_level == 3, "{mars:?}");

        let venus_air = vec![("CO2".to_string(), 0.965f32), ("N2".to_string(), 0.035), ("SO2".to_string(), 0.00015)];
        let venus = evaluate(&HabInput { mean_c: 464.0, equator_c: 465.0, pole_c: 460.0, pressure: 92.0, liquid_water: false, ocean_fraction: 0.0, gravity: 0.9, acid: true, ..input(&venus_air) });
        assert!(venus.score < 0.01 && venus.danger_level == 3);
        assert!(venus.dangers.iter().any(|d| d.name == "pluies acides"));
    }

    #[test]
    fn breathing_needs_oxygen_and_no_poison() {
        assert!(breathability(&earth_air(), 1.0) > 0.9);
        let co2 = vec![("CO2".to_string(), 0.9f32), ("O2".to_string(), 0.1)];
        assert!(breathability(&co2, 1.0) < 0.1);
        let nitrogen = vec![("N2".to_string(), 1.0f32)];
        assert!((breathability(&nitrogen, 1.0) - 0.3).abs() < 1e-6, "sous masque");
        assert_eq!(breathability(&[], 0.0), 0.0);
    }

    #[test]
    fn gas_giants_are_deadly() {
        let h = evaluate(&HabInput { gaseous: true, ..input(&[]) });
        assert_eq!(h.score, 0.0);
        assert_eq!(h.danger_level, 3);
    }
}
