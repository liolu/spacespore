//! Survie à pied (0.11, phase A4) : la combinaison du marcheur.
//!
//! Hors du vaisseau, la combinaison garde l'oxygène, la chaleur, la pression et protège des
//! radiations, dans certaines limites. Au-delà, des alertes puis des dégâts lents (Q5) : jamais
//! de mort instantanée. À zéro, le marcheur perd connaissance et le pilote automatique le ramène
//! au vaisseau. Dans le vaisseau (abri), l'oxygène se recharge et la santé revient.

use bevy::prelude::*;

use crate::net::Net;
use crate::settings::GameSettings;
use crate::surface::Surface;
use crate::world_clock::LocalWeather;

/// Réserve d'oxygène : ~8 min de marche sans air respirable.
const OXYGEN_SECS: f32 = 480.0;
/// Températures supportées par la combinaison (°C).
const SUIT_COLD: f32 = -60.0;
const SUIT_HOT: f32 = 60.0;
/// Pression au-delà de laquelle la combinaison souffre (bar).
const SUIT_PRESSURE: f32 = 10.0;
/// Radiation au sol au-delà de laquelle elle ne protège plus (0 à 1).
const SUIT_RADIATION: f32 = 0.3;

/// Ce que le marcheur subit à l'endroit où il est.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Environment {
    pub pressure: f32,
    /// Fractions d'oxygène et de CO2 de l'air.
    pub oxygen: f32,
    pub co2: f32,
    pub temperature_c: f32,
    /// Radiation au sol (0 à 1, `BiomeParams::radiation`).
    pub radiation: f32,
    /// Dans la lave.
    pub lava: bool,
    /// Sous terre : la roche arrête les radiations.
    pub underground: bool,
}

/// Effets par seconde : oxygène consommé (fraction de la réserve) et dégâts (PV), avec ce qui les
/// cause.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Rates {
    pub oxygen_use: f32,
    pub damage: f32,
    pub alerts: Vec<&'static str>,
}

impl Environment {
    /// L'air se respire sans la réserve : assez d'oxygène, pas trop de CO2, une pression humaine.
    pub fn breathable(&self) -> bool {
        (0.5..3.0).contains(&self.pressure) && self.oxygen * self.pressure >= 0.16 && self.co2 * self.pressure < 0.02
    }

    pub fn rates(&self) -> Rates {
        let mut r = Rates::default();
        if !self.breathable() {
            r.oxygen_use = 1.0 / OXYGEN_SECS;
        }
        let t = self.temperature_c;
        if t < SUIT_COLD {
            r.damage += ((SUIT_COLD - t) / 100.0).min(2.0);
            r.alerts.push("froid extreme");
        } else if t > SUIT_HOT {
            r.damage += ((t - SUIT_HOT) / 100.0).min(2.0);
            r.alerts.push("chaleur extreme");
        }
        if self.pressure > SUIT_PRESSURE {
            r.damage += ((self.pressure - SUIT_PRESSURE) / 20.0).min(3.0);
            r.alerts.push("pression ecrasante");
        }
        if !self.underground && self.radiation > SUIT_RADIATION {
            r.damage += ((self.radiation - SUIT_RADIATION) * 2.0).min(1.5);
            r.alerts.push("radiations");
        }
        if self.lava {
            r.damage += 5.0;
            r.alerts.push("lave !");
        }
        r
    }
}

/// État de la combinaison.
#[derive(Resource)]
pub struct Suit {
    /// Réserve d'oxygène (0 à 1) et santé (PV, 0 à 100).
    pub oxygen: f32,
    pub health: f32,
    /// Dernières alertes affichées (pour ne prévenir qu'au changement).
    last_alerts: Vec<&'static str>,
    pub hud: String,
}

impl Default for Suit {
    fn default() -> Self {
        Self { oxygen: 1.0, health: 100.0, last_alerts: Vec::new(), hud: String::new() }
    }
}

impl Suit {
    /// Un pas : `walking` dehors, sinon à l'abri dans le vaisseau. Renvoie vrai si le marcheur
    /// perd connaissance (santé ou oxygène à zéro).
    pub fn step(&mut self, env: Option<&Environment>, dt: f32) -> bool {
        let Some(env) = env else {
            // Abri : recharge et soins
            self.oxygen = (self.oxygen + dt * 0.1).min(1.0);
            self.health = (self.health + dt * 5.0).min(100.0);
            return false;
        };
        let r = env.rates();
        self.oxygen = (self.oxygen - r.oxygen_use * dt).max(0.0);
        let mut damage = r.damage;
        if self.oxygen <= 0.0 {
            // Plus d'oxygène : on suffoque, lentement
            damage += 2.0;
        }
        self.health = (self.health - damage * dt).max(0.0);
        self.health <= 0.0
    }
}

pub struct SuitPlugin;

impl Plugin for SuitPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Suit>().add_systems(Update, update_suit.after(crate::surface::SurfaceControl));
    }
}

/// Ce que le marcheur subit (s'il est à pied) : air de l'astre, température locale (heure et
/// saison), radiation, lave.
fn environment(settings: &GameSettings, surface: &Surface, weather: &LocalWeather) -> Option<Environment> {
    let (kind, t) = (surface.body()?, surface.terrain()?);
    let walker = surface.walking()?;
    let air = match kind {
        crate::ui::TargetKind::Planet(id) => settings.systems.get(id / 1000)?.planets().get(id % 1000)?.air.clone(),
        crate::ui::TargetKind::Moon(pid, mi) => settings.systems.get(pid / 1000)?.planets().get(pid % 1000)?.moons.get(mi)?.air.clone(),
        // Astéroïde : le vide
        crate::ui::TargetKind::Asteroid(_) => Default::default(),
        _ => return None,
    };
    let underground = surface.underground() > 0.5;
    // Sous terre, la température est celle de la roche : la moyenne, sans le jour ni la nuit
    let temperature_c = if underground {
        t.params.climate.temperature(walker.up().y.clamp(-1.0, 1.0).asin(), 0.0, None)
    } else if weather.body == Some(kind) {
        weather.temp
    } else {
        t.params.climate.mean_c
    };
    Some(Environment {
        pressure: t.params.pressure,
        oxygen: air.fraction("O2"),
        co2: air.fraction("CO2"),
        temperature_c,
        radiation: t.params.biomes.radiation,
        lava: walker.in_water && walker.liquid == crate::planet::VoxelType::Lava,
        underground,
    })
}

fn update_suit(
    time: Res<Time>,
    settings: Res<GameSettings>,
    weather: Res<LocalWeather>,
    mut surface: ResMut<Surface>,
    mut suit: ResMut<Suit>,
    mut net: ResMut<Net>,
) {
    let now = time.elapsed_secs_f64();
    let dt = time.delta_secs().min(0.1);
    let env = environment(&settings, &surface, &weather);
    let mut fainted = suit.step(env.as_ref(), dt);
    // Mode créatif : immortel
    if crate::settings::creative() {
        suit.oxygen = 1.0;
        suit.health = 100.0;
        fainted = false;
    }
    // Alertes : à chaque changement
    let alerts = env.as_ref().map(|e| {
        let mut a = e.rates().alerts;
        if !e.breathable() {
            a.insert(0, "air irrespirable");
        }
        a
    });
    let alerts = alerts.unwrap_or_default();
    let new: Vec<&'static str> = alerts.iter().copied().filter(|a| !suit.last_alerts.contains(a)).collect();
    if !new.is_empty() {
        net.notify(&format!("Combinaison : {}.", new.join(", ")), now);
    }
    if env.is_some() && suit.oxygen < 0.2 && suit.oxygen + dt / OXYGEN_SECS >= 0.2 {
        net.notify("Combinaison : reserve d'oxygene a 20 % ! Retournez au vaisseau (V).", now);
    }
    suit.last_alerts = alerts.clone();
    if fainted {
        surface.request_rescue();
        suit.health = 20.0;
        net.notify("Vous perdez connaissance... Le pilote automatique vous ramene au vaisseau.", now);
    }
    suit.hud = match env {
        Some(_) => {
            let warn = if alerts.is_empty() { String::new() } else { format!("   ! {}", alerts.join(", ")) };
            format!("O2 {:.0} %   Vie {:.0}{warn}", suit.oxygen * 100.0, suit.health)
        }
        None => String::new(),
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn earth() -> Environment {
        Environment { pressure: 1.0, oxygen: 0.21, co2: 0.0004, temperature_c: 15.0, radiation: 0.05, lava: false, underground: false }
    }

    /// Une planète tempérée : rien à craindre, l'oxygène de la réserve n'est pas entamé.
    #[test]
    fn a_temperate_planet_is_safe() {
        let r = earth().rates();
        assert_eq!((r.oxygen_use, r.damage), (0.0, 0.0));
        let mut suit = Suit::default();
        for _ in 0..600 {
            suit.step(Some(&earth()), 1.0);
        }
        assert_eq!((suit.oxygen, suit.health), (1.0, 100.0));
    }

    /// La nuit lunaire : pas d'air, -170 °C : la vie baisse, lentement (jamais d'un coup).
    #[test]
    fn the_lunar_night_hurts_slowly() {
        let night = Environment { pressure: 0.0, oxygen: 0.0, co2: 0.0, temperature_c: -170.0, radiation: 0.1, lava: false, underground: false };
        let r = night.rates();
        assert!(r.oxygen_use > 0.0 && r.damage > 0.5 && r.damage < 3.0, "{r:?}");
        let mut suit = Suit::default();
        let mut secs = 0;
        while !suit.step(Some(&night), 1.0) {
            secs += 1;
        }
        // Plus d'une minute pour s'en sortir
        assert!(secs > 60, "{secs} s");
        // Le vaisseau recharge et soigne
        for _ in 0..30 {
            suit.step(None, 1.0);
        }
        assert!(suit.health > 50.0 && suit.oxygen > 0.9);
    }

    #[test]
    fn venus_crushes_and_cooks_and_mars_cannot_be_breathed() {
        let venus = Environment { pressure: 92.0, oxygen: 0.0, co2: 0.96, temperature_c: 460.0, radiation: 0.0, lava: false, underground: false };
        let r = venus.rates();
        assert!(r.alerts.contains(&"pression ecrasante") && r.alerts.contains(&"chaleur extreme"));
        let mars = Environment { pressure: 0.006, oxygen: 0.0, co2: 0.95, temperature_c: -60.0, radiation: 0.5, lava: false, underground: false };
        assert!(!mars.breathable() && mars.rates().alerts.contains(&"radiations"));
        // Sous terre, la roche protège des radiations
        assert!(!Environment { underground: true, ..mars }.rates().alerts.contains(&"radiations"));
    }
}
