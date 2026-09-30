// Contient du code hérité pas (ou plus) branché : sélecteurs d'astres désactivés, ancienne mise à jour
// intégrée au jeu (remplacée par le launcher)... Gardé, sans avertissements.
#![allow(dead_code)]

use bevy::prelude::*;

use crate::settings::{GameSettings, AsteroidBeltConfig};
use crate::ui::AstresMutableResources;
use crate::astre::etoile::main_sequence_star;
use crate::astre::etoile::dwarf_star;
use crate::astre::etoile::star as voxel_star;
use crate::astre::etoile::giant_star;
use crate::astre::etoile::supergiant_star;
use crate::astre::etoile::hypergiant_star;
use crate::astre::planete::gas_planet;
use crate::astre::planete::comet;

// ─────────────────────────────────────────────────────────────────────────
//  Deterministic PRNG (xorshift64)
// ─────────────────────────────────────────────────────────────────────────

pub struct SeedRng(u64);

impl SeedRng {
    pub fn new(seed: u64) -> Self {
        Self(seed.wrapping_add(1) | 1)
    }
    fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    pub fn f32(&mut self) -> f32 {
        (self.next_u64() & 0xFF_FFFF) as f32 / 16_777_216.0
    }
    pub fn u32(&mut self) -> u32 {
        self.next_u64() as u32
    }
    pub fn range_f32(&mut self, min: f32, max: f32) -> f32 {
        min + self.f32() * (max - min)
    }
    pub fn range_u32(&mut self, min: u32, max: u32) -> u32 {
        if min >= max { return min; }
        min + (self.next_u64() % (max - min + 1) as u64) as u32
    }
    #[allow(dead_code)]
    pub fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        let i = self.next_u64() as usize % items.len();
        items[i]
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  Spawn descriptors — each astre declares its own
// ─────────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AstreCategory {
    Star,
    GasPlanet,
    Comet,
}

#[derive(Clone, Copy, Debug)]
pub struct SpawnProps {
    pub category:  AstreCategory,
    pub weight:    f32,
    pub orbit_min: f32,
    pub orbit_max: f32,
}

// ─────────────────────────────────────────────────────────────────────────
//  Main generation system — runs at PreStartup before load_saved_astres
// ─────────────────────────────────────────────────────────────────────────

pub fn populate_from_seed(
    mut settings: ResMut<GameSettings>,
    mut res: AstresMutableResources,
) {
    let seed = settings.world_seed;
    if seed == 0 {
        return;
    }

    let mut rng = SeedRng::new(seed);

    // ── Clear all hardcoded defaults ────────────────────────────────────
    res.gas_res.planets.clear();
    res.comet_res.comets.clear();
    res.meteor_res.meteoroids.clear();
    res.vstar_res.stars.clear();
    res.proto_res.stars.clear();
    res.dwarf_res.stars.clear();
    res.ms_res.stars.clear();
    res.giant_res.stars.clear();
    res.sg_res.stars.clear();
    res.hg_res.stars.clear();
    res.black_res.holes.clear();
    res.nebula_res.enabled = false;
    res.pulsar_res.pulsars.clear();
    res.magnetar_res.magnetars.clear();
    res.neutron_res.stars.clear();
    res.sn_res.supernovae.clear();

    // ── Generate central star (weighted selection from SPAWN_PROPS) ─────
    let star_weights: &[(f32, fn(&mut SeedRng, &mut AstresMutableResources))] = &[
        (main_sequence_star::SPAWN_PROPS.weight, |rng, res| {
            main_sequence_star::generate_random(rng, &mut res.ms_res.stars);
        }),
        (dwarf_star::SPAWN_PROPS.weight, |rng, res| {
            dwarf_star::generate_random(rng, &mut res.dwarf_res.stars);
        }),
        (voxel_star::SPAWN_PROPS.weight, |rng, res| {
            voxel_star::generate_random(rng, &mut res.vstar_res.stars);
        }),
        (giant_star::SPAWN_PROPS.weight, |rng, res| {
            giant_star::generate_random(rng, &mut res.giant_res.stars);
        }),
        (supergiant_star::SPAWN_PROPS.weight, |rng, res| {
            supergiant_star::generate_random(rng, &mut res.sg_res.stars);
        }),
        (hypergiant_star::SPAWN_PROPS.weight, |rng, res| {
            hypergiant_star::generate_random(rng, &mut res.hg_res.stars);
        }),
    ];

    let total_w: f32 = star_weights.iter().map(|(w, _)| w).sum();
    let roll = rng.f32() * total_w;
    let mut acc = 0.0;
    for &(w, gen_fn) in star_weights {
        acc += w;
        if roll < acc {
            gen_fn(&mut rng, &mut res);
            break;
        }
    }

    // ── Generate planets ────────────────────────────────────────────────
    let num_planets = rng.range_u32(3, 7);
    let mut orbit = rng.range_f32(1500.0, 2500.0);

    let sys = settings.systems.first_mut().expect("at least one system");
    sys.planets.clear();
    sys.stars.clear();
    sys.asteroid_belts.clear();

    let gas_props = gas_planet::SPAWN_PROPS;

    for _pi in 0..num_planets {
        let is_gas = orbit >= gas_props.orbit_min && rng.f32() < gas_props.weight;

        if is_gas {
            gas_planet::generate_random(&mut rng, &mut res.gas_res.planets, orbit);
        } else {
            generate_rocky_planet(&mut rng, sys, orbit);
        }

        orbit += rng.range_f32(1200.0, 2800.0);
    }

    // ── Optional comet ──────────────────────────────────────────────────
    let comet_props = comet::SPAWN_PROPS;
    if rng.f32() < comet_props.weight {
        comet::generate_random(&mut rng, &mut res.comet_res.comets, orbit);
    }

    // ── Asteroid belt ──────────────────────────────────────────────────
    sys.asteroid_belts.push(AsteroidBeltConfig {
        distance: orbit + rng.range_f32(500.0, 1500.0),
        width: rng.range_f32(800.0, 2000.0),
        min_size: rng.range_f32(20.0, 50.0),
        max_size: rng.range_f32(80.0, 180.0),
        count: rng.range_u32(200, 400),
    });

    info!(
        "System generated from seed {} — {} rocky, {} gas, {} comets, {} belts",
        seed,
        sys.planets.len(),
        res.gas_res.planets.len(),
        res.comet_res.comets.len(),
        sys.asteroid_belts.len(),
    );
}

// ─────────────────────────────────────────────────────────────────────────
//  Rocky planet generator (uses settings, stays here)
// ─────────────────────────────────────────────────────────────────────────

fn generate_rocky_planet(
    rng: &mut SeedRng,
    sys: &mut crate::settings::StarSystemConfig,
    orbit: f32,
) {
    use crate::settings::{PlanetConfig, MoonConfig};

    let planet_seed = rng.u32();
    let radius = rng.range_f32(40.0, 300.0);
    let has_atmo = rng.f32() < 0.35;
    let sea = if has_atmo { rng.range_f32(0.15, 0.6) } else { rng.range_f32(0.0, 0.12) };

    let num_moons = if rng.f32() < 0.40 { rng.range_u32(1, 3) } else { 0 };
    let mut moons = Vec::new();
    let mut moon_orbit = radius * 2.5 + 100.0;
    for _ in 0..num_moons {
        moons.push(MoonConfig {
            orbit_distance: moon_orbit,
            radius: rng.range_f32(15.0, (radius * 0.3).max(20.0)),
            seed: rng.u32(),
            eccentricity: rng.range_f32(0.0, 0.05),
            inclination: rng.range_f32(-0.15, 0.15),
            ascending_node: rng.range_f32(0.0, std::f32::consts::TAU),
            arg_periapsis: rng.range_f32(0.0, std::f32::consts::TAU),
            mean_anomaly_0: rng.range_f32(0.0, std::f32::consts::TAU),
        });
        moon_orbit += rng.range_f32(150.0, 400.0);
    }

    sys.planets.push(PlanetConfig {
        orbit_distance: orbit,
        radius,
        sea_level: sea,
        terrain_height: rng.range_f32(20.0, 120.0),
        seed: planet_seed,
        noise_scale: rng.range_f32(1.5, 4.0),
        detail_scale: rng.range_f32(3.0, 6.0),
        atmosphere: has_atmo,
        cloud_density: if has_atmo { rng.range_f32(0.3, 1.0) } else { 0.0 },
        cloud_altitude: rng.range_f32(50.0, 120.0),
        cloud_speed: rng.range_f32(0.01, 0.05),
        eccentricity: rng.range_f32(0.0, 0.06),
        inclination: rng.range_f32(-0.08, 0.08),
        ascending_node: rng.range_f32(0.0, std::f32::consts::TAU),
        arg_periapsis: rng.range_f32(0.0, std::f32::consts::TAU),
        mean_anomaly_0: rng.range_f32(0.0, std::f32::consts::TAU),
        moons,
    });
}
