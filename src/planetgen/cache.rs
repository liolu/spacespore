//! Caches de la génération.
//!
//! - Planètes : `StarSystemConfig::planets()` les recalcule à la demande et les garde ; ce module
//!   libère, toutes les deux secondes, celles des systèmes éloignés du vaisseau (seuls le système
//!   chargé et ses voisins restent en mémoire).
//! - Profils : `ProfileCache` garde les profils complets (étoile, planètes, lunes) du système
//!   chargé ; les autres astres sont profilés à la demande (`profile_of`).

use bevy::prelude::*;
use serde::Serialize;
use std::collections::HashSet;

use super::live::BodyId;
use super::profile::{PlanetProfile, StarProfile};
use crate::planet::SpawnedSystems;
use crate::settings::{GameSettings, SystemSpatialIndex};
use crate::ship::Ship;

/// Profils du système chargé.
#[derive(Resource, Default)]
pub struct ProfileCache {
    pub system: Option<usize>,
    pub stars: Vec<StarProfile>,
    pub planets: Vec<PlanetProfile>,
    /// Lunes de chaque planète.
    pub moons: Vec<Vec<PlanetProfile>>,
}

impl ProfileCache {
    /// Profils de tout un système.
    pub fn build(settings: &GameSettings, si: usize) -> Self {
        let Some(sys) = settings.systems.get(si) else { return Self::default() };
        let deltas = &settings.body_deltas;
        let planets = sys.planets();
        Self {
            system: Some(si),
            stars: sys.stars.iter().enumerate().map(|(i, st)| StarProfile::build(si, i, sys, st)).collect(),
            planets: planets.iter().enumerate().map(|(pi, p)| PlanetProfile::planet(si, pi, sys, p, deltas)).collect(),
            moons: planets
                .iter()
                .enumerate()
                .map(|(pi, p)| {
                    p.moons.iter().enumerate().map(|(mi, m)| PlanetProfile::moon(si, pi, mi, sys, p, m, deltas)).collect()
                })
                .collect(),
        }
    }

    fn get(&self, id: BodyId) -> Option<Profile> {
        if self.system != Some(id.system()) {
            return None;
        }
        Some(match id {
            BodyId::Star { index, .. } => Profile::Star(self.stars.get(index as usize)?.clone()),
            BodyId::Planet { index, .. } => Profile::Body(self.planets.get(index as usize)?.clone()),
            BodyId::Moon { planet, index, .. } => Profile::Body(self.moons.get(planet as usize)?.get(index as usize)?.clone()),
        })
    }
}

/// Profil d'un astre, quel qu'il soit (s'exporte en JSON tel quel).
#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum Profile {
    Star(StarProfile),
    Body(PlanetProfile),
}

impl Profile {
    pub fn name(&self) -> &str {
        match self {
            Profile::Star(s) => &s.name,
            Profile::Body(b) => &b.name,
        }
    }

    pub fn id(&self) -> &str {
        match self {
            Profile::Star(s) => &s.id,
            Profile::Body(b) => &b.id,
        }
    }
}

/// Profil d'un astre : depuis le cache s'il est dans le système chargé, sinon calculé.
pub fn profile_of(settings: &GameSettings, cache: &ProfileCache, id: BodyId) -> Option<Profile> {
    cache.get(id).or_else(|| ProfileCache::build(settings, id.system()).get(id))
}

pub(super) fn refresh_profile_cache(settings: Res<GameSettings>, spawned: Res<SpawnedSystems>, mut cache: ResMut<ProfileCache>) {
    if !spawned.is_changed() && !settings.is_changed() {
        return;
    }
    let loaded = spawned.0.iter().next().copied();
    if loaded.is_none() {
        if cache.system.is_some() {
            *cache = ProfileCache::default();
        }
        return;
    }
    if cache.system != loaded || settings.is_changed() {
        *cache = loaded.map_or_else(ProfileCache::default, |si| ProfileCache::build(&settings, si));
    }
}

/// Libère les planètes des systèmes loin du vaisseau.
pub(super) fn forget_far_planets(
    mut settings: ResMut<GameSettings>,
    spatial: Res<SystemSpatialIndex>,
    spawned: Res<SpawnedSystems>,
    ship_q: Query<&GlobalTransform, With<Ship>>,
) {
    let Ok(ship) = ship_q.get_single() else { return };
    // Les mêmes systèmes que ceux que `planet.rs` examine pour savoir dans lequel on se trouve
    let near: HashSet<usize> = spatial
        .systems_in_radius(ship.translation(), crate::planet::MAX_SYSTEM_REACH)
        .into_iter()
        .chain(spawned.0.iter().copied())
        .collect();
    // Pas de « changement » des réglages : rien d'autre ne doit réagir à ce ménage
    let settings = settings.bypass_change_detection();
    let mut freed = 0;
    for (si, sys) in settings.systems.iter_mut() {
        if sys.planets_cached() && !near.contains(&si) {
            sys.forget_planets();
            freed += 1;
        }
    }
    if freed > 0 {
        debug!("planetgen : planetes de {freed} systemes liberees");
    }
}

/// Nombre de systèmes dont les planètes sont en mémoire.
pub fn cached_systems(settings: &GameSettings) -> usize {
    settings.systems.iter().filter(|(_, s)| s.planets_cached()).count()
}
