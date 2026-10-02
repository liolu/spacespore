//! Conversion unités réelles ↔ unités du jeu (règle 4 de la feuille de route).
//!
//! La science se calcule en unités réelles (R⊕, M⊕, R☉, M☉, L☉, UA, K) ; le rendu travaille en
//! unités du jeu. C'est la SEULE couche de conversion entre les deux.
//!
//! Tailles : 1 R⊕ ≈ 6 000 unités et 1 R☉ = 109 R⊕ ≈ 654 000 unités (rapport réel).
//!
//! Distances : les orbites du jeu sont bien plus serrées que les vraies (une planète tourne à
//! quelques rayons de son étoile au lieu de ~200). La distance « physique » en UA, qui sert au
//! climat, est donc convertie séparément de la distance affichée : voir `orbit_game_to_au`.

/// Unités du jeu pour un rayon terrestre.
pub const GAME_PER_EARTH_RADIUS: f64 = 6_000.0;
/// Rayon solaire / rayon terrestre (réel : 109,1).
pub const EARTH_RADII_PER_SUN_RADIUS: f64 = 109.0;
/// Unités du jeu pour un rayon solaire (≈ 654 000).
pub const GAME_PER_SUN_RADIUS: f64 = GAME_PER_EARTH_RADIUS * EARTH_RADII_PER_SUN_RADIUS;

/// Rayon terrestre en km.
pub const EARTH_RADIUS_KM: f64 = 6_371.0;
/// Masse terrestre / masse solaire.
pub const EARTH_MASSES_PER_SUN_MASS: f64 = 332_946.0;
/// Masse volumique moyenne de la Terre (g/cm³).
pub const EARTH_DENSITY: f64 = 5.514;
/// Vitesse de libération terrestre (km/s).
pub const EARTH_ESCAPE_VELOCITY: f64 = 11.186;
/// Gravité terrestre (m/s²).
pub const EARTH_GRAVITY: f64 = 9.807;
/// Température de surface du Soleil (K).
pub const SUN_TEMPERATURE: f64 = 5_772.0;
/// Zéro absolu en °C.
pub const ZERO_CELSIUS: f64 = 273.15;

/// Distance d'orbite, en rayons de l'étoile, qui correspond à 1 UA dans le jeu actuel : la
/// température d'aujourd'hui (`PlanetConfig::temperature`) est tempérée vers 4 rayons d'étoile.
/// Provisoire : la phase 2 place les orbites en UA d'après la luminosité de l'étoile.
pub const GAME_STAR_RADII_PER_AU: f64 = 4.0;

pub fn earth_radii_to_game(r: f64) -> f64 {
    r * GAME_PER_EARTH_RADIUS
}

pub fn game_to_earth_radii(units: f64) -> f64 {
    units / GAME_PER_EARTH_RADIUS
}

pub fn sun_radii_to_game(r: f64) -> f64 {
    r * GAME_PER_SUN_RADIUS
}

pub fn game_to_sun_radii(units: f64) -> f64 {
    units / GAME_PER_SUN_RADIUS
}

pub fn game_to_km(units: f64) -> f64 {
    game_to_earth_radii(units) * EARTH_RADIUS_KM
}

pub fn km_to_game(km: f64) -> f64 {
    earth_radii_to_game(km / EARTH_RADIUS_KM)
}

/// Distance physique (UA) d'une orbite du jeu autour d'une étoile de rayon `star_radius` (jeu).
pub fn orbit_game_to_au(orbit: f64, star_radius: f64) -> f64 {
    orbit / (star_radius.max(1.0) * GAME_STAR_RADII_PER_AU)
}

/// Distance du jeu d'une orbite physique de `au` UA autour d'une étoile de rayon `star_radius`.
pub fn orbit_au_to_game(au: f64, star_radius: f64) -> f64 {
    au * star_radius.max(1.0) * GAME_STAR_RADII_PER_AU
}

pub fn kelvin_to_celsius(k: f64) -> f64 {
    k - ZERO_CELSIUS
}

pub fn celsius_to_kelvin(c: f64) -> f64 {
    c + ZERO_CELSIUS
}

/// Gravité de surface (en g) d'un corps de masse `mass` (M⊕) et de rayon `radius` (R⊕).
pub fn surface_gravity(mass: f64, radius: f64) -> f64 {
    if radius <= 0.0 { 0.0 } else { mass / (radius * radius) }
}

/// Vitesse de libération (km/s).
pub fn escape_velocity(mass: f64, radius: f64) -> f64 {
    if radius <= 0.0 || mass <= 0.0 { 0.0 } else { EARTH_ESCAPE_VELOCITY * (mass / radius).sqrt() }
}

/// Masse volumique moyenne (g/cm³).
pub fn density(mass: f64, radius: f64) -> f64 {
    if radius <= 0.0 { 0.0 } else { EARTH_DENSITY * mass / (radius * radius * radius) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn earth_and_sun_sizes_follow_the_roadmap() {
        assert_eq!(earth_radii_to_game(1.0), 6_000.0);
        let sun = sun_radii_to_game(1.0);
        assert!((640_000.0..=660_000.0).contains(&sun), "{sun}");
        assert!((sun / earth_radii_to_game(1.0) - 109.0).abs() < 1e-9);
        // Les étoiles du jeu actuel (600 000 à 1 500 000) font ~0,9 à 2,3 R☉
        assert!((0.9..1.0).contains(&game_to_sun_radii(600_000.0)));
    }

    #[test]
    fn conversions_round_trip() {
        for x in [0.1, 1.0, 3.7, 250.0] {
            assert!((game_to_earth_radii(earth_radii_to_game(x)) - x).abs() < 1e-9);
            assert!((game_to_sun_radii(sun_radii_to_game(x)) - x).abs() < 1e-9);
            assert!((km_to_game(game_to_km(x)) - x).abs() < 1e-9);
            assert!((orbit_au_to_game(orbit_game_to_au(x * 1e5, 9e5), 9e5) - x * 1e5).abs() < 1e-6);
            assert!((celsius_to_kelvin(kelvin_to_celsius(x)) - x).abs() < 1e-9);
        }
        assert!((game_to_km(6_000.0) - EARTH_RADIUS_KM).abs() < 1e-9);
    }

    #[test]
    fn earth_physics_is_earth() {
        assert!((surface_gravity(1.0, 1.0) - 1.0).abs() < 1e-12);
        assert!((escape_velocity(1.0, 1.0) - EARTH_ESCAPE_VELOCITY).abs() < 1e-12);
        assert!((density(1.0, 1.0) - EARTH_DENSITY).abs() < 1e-12);
        // Mars : 0,107 M⊕, 0,532 R⊕ → 0,38 g, 5,0 km/s
        assert!((surface_gravity(0.107, 0.532) - 0.378).abs() < 0.01);
        assert!((escape_velocity(0.107, 0.532) - 5.02).abs() < 0.05);
        assert_eq!(surface_gravity(1.0, 0.0), 0.0);
    }

    #[test]
    fn a_temperate_orbit_is_one_au() {
        assert!((orbit_game_to_au(4.0 * 900_000.0, 900_000.0) - 1.0).abs() < 1e-12);
    }
}
