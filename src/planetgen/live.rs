//! Valeurs vivantes (règle 7 de la feuille de route), en préparation du minage (0.14).
//!
//! Masse, rayon, composition et orbite calculés depuis la graine ne sont que la valeur de DÉPART.
//! Toute modification (minage, destruction) est un delta enregistré dans la sauvegarde du monde
//! (`world.json`, champ `body_deltas`) et, plus tard, partagé en réseau. Le code lit toujours
//! « départ + delta » (`Live::current`), jamais le départ seul ; la gravité et l'orbite se
//! recalculent depuis la masse courante. Pour l'instant, aucun delta n'est jamais écrit.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Identifiant stable d'un astre du monde : `s<système>.e<étoile>`, `s<système>.p<planète>` ou
/// `s<système>.p<planète>.m<lune>` (indices à partir de 0). Il sert de clé aux deltas.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BodyId {
    Star { system: u32, index: u16 },
    Planet { system: u32, index: u16 },
    Moon { system: u32, planet: u16, index: u16 },
}

impl BodyId {
    pub fn system(self) -> usize {
        match self {
            BodyId::Star { system, .. } | BodyId::Planet { system, .. } | BodyId::Moon { system, .. } => system as usize,
        }
    }

    pub fn key(self) -> String {
        match self {
            BodyId::Star { system, index } => format!("s{system}.e{index}"),
            BodyId::Planet { system, index } => format!("s{system}.p{index}"),
            BodyId::Moon { system, planet, index } => format!("s{system}.p{planet}.m{index}"),
        }
    }

    pub fn parse(key: &str) -> Option<Self> {
        let mut parts = key.split('.');
        let system = parts.next()?.strip_prefix('s')?.parse().ok()?;
        let second = parts.next()?;
        let third = parts.next();
        if parts.next().is_some() {
            return None;
        }
        let num = |s: &str, prefix: char| s.strip_prefix(prefix)?.parse::<u16>().ok();
        match (second.chars().next()?, third) {
            ('e', None) => Some(BodyId::Star { system, index: num(second, 'e')? }),
            ('p', None) => Some(BodyId::Planet { system, index: num(second, 'p')? }),
            ('p', Some(m)) => Some(BodyId::Moon { system, planet: num(second, 'p')?, index: num(m, 'm')? }),
            _ => None,
        }
    }
}

/// Une valeur vivante : départ (généré) + delta (modifications du monde).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct Live {
    pub base: f64,
    pub delta: f64,
}

impl Live {
    pub fn new(base: f64, delta: f64) -> Self {
        Self { base, delta }
    }

    /// La valeur à utiliser partout.
    pub fn current(&self) -> f64 {
        self.base + self.delta
    }
}

fn is_zero(x: &f64) -> bool {
    *x == 0.0
}

/// Modifications d'un astre depuis sa génération. Tout à zéro = astre intact.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BodyDelta {
    /// Masse ajoutée (négative : retirée), en M⊕.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub mass_earth: f64,
    /// Rayon ajouté, en R⊕.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub radius_earth: f64,
    /// Demi-grand axe ajouté, en UA.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub orbit_au: f64,
    /// Fraction de masse ajoutée par matière (ex. "fer": -0.01).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub composition: BTreeMap<String, f64>,
}

impl BodyDelta {
    pub fn is_empty(&self) -> bool {
        self.mass_earth == 0.0 && self.radius_earth == 0.0 && self.orbit_au == 0.0 && self.composition.is_empty()
    }
}

/// Tous les deltas du monde, par clé d'astre (`BodyId::key`). Vide tant que le minage n'existe pas.
pub type WorldDeltas = BTreeMap<String, BodyDelta>;

/// Delta d'un astre (zéro s'il n'en a pas).
pub fn delta_of<'a>(deltas: &'a WorldDeltas, id: BodyId) -> std::borrow::Cow<'a, BodyDelta> {
    match deltas.get(&id.key()) {
        Some(d) => std::borrow::Cow::Borrowed(d),
        None => std::borrow::Cow::Owned(BodyDelta::default()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_keys_round_trip() {
        for id in [
            BodyId::Star { system: 0, index: 0 },
            BodyId::Planet { system: 1234, index: 2 },
            BodyId::Moon { system: 99_999, planet: 1, index: 1 },
        ] {
            assert_eq!(BodyId::parse(&id.key()), Some(id), "{}", id.key());
        }
        assert_eq!(BodyId::Moon { system: 3, planet: 1, index: 0 }.key(), "s3.p1.m0");
        for bad in ["", "s", "p1", "s1", "s1.x2", "s1.p2.m3.m4", "s-1.p0", "s1.e1.m0"] {
            assert_eq!(BodyId::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn live_values_read_base_plus_delta() {
        let mut deltas = WorldDeltas::new();
        let id = BodyId::Planet { system: 5, index: 0 };
        assert!(delta_of(&deltas, id).is_empty());
        deltas.insert(id.key(), BodyDelta { mass_earth: -0.25, ..Default::default() });
        let mass = Live::new(1.0, delta_of(&deltas, id).mass_earth);
        assert_eq!(mass.current(), 0.75);
        // Un delta vide ne s'écrit pas dans world.json
        assert_eq!(serde_json::to_string(&BodyDelta::default()).unwrap(), "{}");
        let back: BodyDelta = serde_json::from_str(r#"{"mass_earth":-0.25}"#).unwrap();
        assert_eq!(back.mass_earth, -0.25);
    }
}
