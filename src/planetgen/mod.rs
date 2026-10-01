//! Génération des étoiles et des planètes (feuille de route `ROADMAP-0.10.md`).
//!
//! Chaîne visée : étoile → orbite → physique → atmosphère → climat → eau → géologie → relief →
//! biomes → vie, déterministe, cohérente et légère en mémoire. Phase 0 (fondations) :
//!
//! - `genome`    : génome léger d'un système, planètes et lunes recalculées à la demande ;
//! - `seeds`     : une sous-graine par couche ;
//! - `units`     : la seule conversion unités réelles ↔ unités du jeu ;
//! - `profile`   : `StarProfile` et `PlanetProfile`, avec toutes leurs sections ;
//! - `live`      : valeurs vivantes (départ + delta) et identifiants d'astres ;
//! - `cache`     : cache des profils du système chargé, ménage des planètes éloignées ;
//! - `seed_code` : graine du monde en code court partageable.

// Fondations : une partie (sections des profils, conversions, tirages par couche) ne sert qu'à
// partir des phases suivantes.
#![allow(dead_code)]

pub mod cache;
pub mod genome;
pub mod live;
pub mod profile;
pub mod seed_code;
pub mod seeds;
pub mod star;
pub mod units;

#[cfg(test)]
pub mod memory;

use bevy::prelude::*;
use bevy::time::common_conditions::on_timer;
use std::time::Duration;

pub struct PlanetGenPlugin;

impl Plugin for PlanetGenPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<cache::ProfileCache>().add_systems(
            Update,
            (
                cache::refresh_profile_cache,
                cache::forget_far_planets.run_if(on_timer(Duration::from_secs(2))),
            ),
        );
    }
}

#[cfg(test)]
mod tests {
    use crate::settings::{default_galaxies, default_systems, StarSystemConfig, DEFAULT_WORLD_SEED};

    /// Empreinte complète (tous les champs, au bit près) des `n` premiers systèmes.
    pub fn world_digest(systems: &[StarSystemConfig], n: usize) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut eat = |v: u64| {
            for b in v.to_le_bytes() {
                h ^= b as u64;
                h = h.wrapping_mul(0x100_0000_01b3);
            }
        };
        let f = |x: f32| x.to_bits() as u64;
        eat(systems.len() as u64);
        for sys in systems.iter().take(n) {
            for b in sys.name.bytes() {
                eat(b as u64);
            }
            for c in sys.position {
                eat(f(c));
            }
            eat(sys.galaxy_id as u64);
            eat(sys.asteroid_belts.len() as u64);
            for st in &sys.stars {
                for v in [
                    st.orbit_distance, st.radius, st.intensity, st.light_range, st.light_color_r, st.light_color_g,
                    st.light_color_b, st.flare_height, st.flare_speed, st.flare_size, st.flare_distance,
                ] {
                    eat(f(v));
                }
                eat(st.flare_count as u64);
            }
            let planets = sys.planets_uncached();
            eat(planets.len() as u64);
            for p in planets.iter() {
                for v in [
                    p.orbit_distance, p.radius, p.sea_level, p.terrain_height, p.noise_scale, p.detail_scale,
                    p.star_radius, p.cloud_density, p.cloud_altitude, p.cloud_speed, p.eccentricity, p.inclination,
                    p.ascending_node, p.arg_periapsis, p.mean_anomaly_0,
                ] {
                    eat(f(v));
                }
                eat(p.seed as u64);
                eat(p.atmosphere as u64);
                eat(p.moons.len() as u64);
                for m in &p.moons {
                    for v in [m.orbit_distance, m.radius, m.eccentricity, m.inclination, m.ascending_node,
                              m.arg_periapsis, m.mean_anomaly_0] {
                        eat(f(v));
                    }
                    eat(m.seed as u64);
                }
            }
        }
        h
    }

    /// Empreinte des seules planètes et lunes.
    fn planets_digest(systems: &[StarSystemConfig], planets_of: impl Fn(&StarSystemConfig) -> Vec<crate::settings::PlanetConfig>) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut eat = |v: u64| {
            for b in v.to_le_bytes() {
                h ^= b as u64;
                h = h.wrapping_mul(0x100_0000_01b3);
            }
        };
        for sys in systems {
            let planets = planets_of(sys);
            eat(planets.len() as u64);
            for p in &planets {
                for v in [p.orbit_distance, p.radius, p.sea_level, p.terrain_height, p.noise_scale, p.detail_scale,
                          p.star_radius, p.cloud_altitude] {
                    eat(v.to_bits() as u64);
                }
                eat(p.seed as u64);
                eat(p.atmosphere as u64);
                for m in &p.moons {
                    for v in [m.orbit_distance, m.radius, m.mean_anomaly_0] {
                        eat(v.to_bits() as u64);
                    }
                    eat(m.seed as u64);
                }
            }
        }
        h
    }

    /// Non-régression : les étoiles ont changé (phase 1) mais les planètes et lunes restent
    /// exactement celles de la version 0.9 (empreinte relevée avant la phase 1) ; seules celles des
    /// géantes sont repoussées hors de l'étoile.
    #[test]
    fn planets_are_those_of_version_0_9() {
        let galaxies = default_galaxies(DEFAULT_WORLD_SEED);
        let systems = default_systems(&galaxies, DEFAULT_WORLD_SEED);
        assert_eq!(systems.len(), 143_892);
        assert_eq!(planets_digest(&systems, |s| s.genome_planets()), 0x5fe1_883d_9414_8c9c);
        let mut pushed = 0;
        for sys in &systems {
            let (now, before) = (sys.planets_uncached(), sys.genome_planets());
            let star = &sys.stars[0];
            let overlaps = before[0].orbit_distance - before[0].radius < star.radius * crate::settings::GIANT_CLEARANCE;
            for (a, b) in now.iter().zip(&before) {
                assert_eq!(a.radius.to_bits(), b.radius.to_bits());
                if overlaps {
                    assert!(a.orbit_distance > b.orbit_distance);
                } else {
                    assert_eq!(a.orbit_distance.to_bits(), b.orbit_distance.to_bits());
                }
            }
            pushed += overlaps as usize;
        }
        // Seules les géantes (et quelques grosses étoiles chaudes) repoussent leurs planètes
        assert!(pushed > 100 && pushed < systems.len() / 50, "{pushed} systemes repousses");
    }

    /// Le monde (étoiles comprises) est reproductible au bit près.
    #[test]
    fn the_world_is_reproducible() {
        let galaxies = default_galaxies(DEFAULT_WORLD_SEED);
        let a = default_systems(&galaxies, DEFAULT_WORLD_SEED);
        let b = default_systems(&galaxies, DEFAULT_WORLD_SEED);
        assert_eq!(world_digest(&a, usize::MAX), world_digest(&b, usize::MAX));
        let c = default_systems(&default_galaxies(7), 7);
        assert_ne!(world_digest(&a, 3000), world_digest(&c, 3000));
        // Le parcours n'a rien gardé en mémoire
        assert!(a.iter().all(|s| !s.planets_cached()));
    }

    /// Chaque étoile suit son type ; aucune ne dépasse 6,5 M de rayon affiché.
    #[test]
    fn stars_follow_their_type() {
        use crate::planetgen::star::StarClass;
        let galaxies = default_galaxies(DEFAULT_WORLD_SEED);
        let systems = default_systems(&galaxies, DEFAULT_WORLD_SEED);
        let mut count = std::collections::HashMap::new();
        for sys in &systems {
            let star = &sys.stars[0];
            *count.entry(star.class).or_insert(0usize) += 1;
            assert!(star.radius > 0.0 && star.radius <= 6_500_000.0, "{}", star.radius);
            assert!(star.temperature_k > 250.0);
            let physics = sys.star_physics().unwrap();
            assert_eq!(physics.class, star.class);
            assert!((physics.temperature_k as f32 - star.temperature_k).abs() < 1.0);
        }
        let share = |c: StarClass| *count.get(&c).unwrap_or(&0) as f64 / systems.len() as f64;
        assert!(share(StarClass::M) > 0.6 && share(StarClass::G) > 0.05 && share(StarClass::G) < 0.1);
        assert!(share(StarClass::RedGiant) > 0.003 && share(StarClass::WhiteDwarf) > 0.03);
        println!("types : {count:?}");
    }

    #[test]
    fn cached_planets_match_and_can_be_forgotten() {
        let galaxies = default_galaxies(DEFAULT_WORLD_SEED);
        let mut systems = default_systems(&galaxies, DEFAULT_WORLD_SEED);
        for sys in systems.iter_mut().step_by(997) {
            let fresh = sys.planets_uncached().into_owned();
            assert!(!sys.planets_cached());
            let cached = sys.planets();
            assert_eq!(cached.len(), fresh.len());
            for (a, b) in cached.iter().zip(&fresh) {
                assert_eq!(a.radius.to_bits(), b.radius.to_bits());
                assert_eq!(a.moons.len(), b.moons.len());
            }
            assert!(sys.planets_cached());
            sys.forget_planets();
            assert!(!sys.planets_cached());
            assert_eq!(sys.planets().len(), fresh.len());
        }
    }

    #[test]
    fn edited_planets_are_kept() {
        let galaxies = default_galaxies(DEFAULT_WORLD_SEED);
        let mut systems = default_systems(&galaxies, DEFAULT_WORLD_SEED);
        let sys = &mut systems[0];
        sys.planets_mut()[0].radius = 123.0;
        sys.forget_planets();
        assert_eq!(sys.planets()[0].radius, 123.0);
        assert!(!sys.planets_cached(), "planetes explicites : pas un cache");
    }

    /// Mémoire de la liste des systèmes : 68,9 Mo avant la phase 0 (planètes stockées).
    #[test]
    fn the_system_list_is_much_lighter() {
        use crate::planetgen::memory::retained_by;
        let galaxies = default_galaxies(DEFAULT_WORLD_SEED);
        let (systems, bytes) = retained_by(|| default_systems(&galaxies, DEFAULT_WORLD_SEED));
        let per_system = bytes as f64 / systems.len() as f64;
        println!("liste des systemes : {} octets ({per_system:.0} par systeme)", bytes);
        assert!(bytes < 68_889_502 / 2, "{bytes} octets");
        // Un système chargé (planètes + lunes) ne pèse que quelques centaines d'octets
        let (_, one) = retained_by(|| systems[10].planets().len());
        assert!(one > 0 && one < 4_000, "{one}");
    }
}
