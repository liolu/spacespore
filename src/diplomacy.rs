// ─────────────────────────────────────────────────────────────────────────
//  Diplomatie : alliés, neutres, ennemis
//
//  Chaque joueur déclare sa position envers une faction : une guilde entière
//  ([TAG]) ou un joueur sans guilde. Par défaut tout le monde est neutre.
//
//  Relation effective entre deux joueurs :
//   - même guilde : toujours alliés ;
//   - ennemis dès que l'un des deux a déclaré l'autre ennemi ;
//   - alliés seulement si les deux se sont déclarés alliés ;
//   - neutres sinon.
//
//  On ne peut pas attaquer un allié (ni son vaisseau, ni ses étoiles) ;
//  les neutres et les ennemis peuvent être attaqués.
// ─────────────────────────────────────────────────────────────────────────

use bevy::prelude::*;

use serde::{Deserialize, Serialize};

use crate::net::{Peer, MAX_RELATIONS};
use crate::settings::GameSettings;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Relation {
    Ally,
    Neutral,
    Enemy,
}

impl Relation {
    pub fn label(self) -> &'static str {
        match self {
            Relation::Ally => "Allie",
            Relation::Neutral => "Neutre",
            Relation::Enemy => "Ennemi",
        }
    }

    pub fn color(self) -> Color {
        match self {
            Relation::Ally => Color::srgb(0.45, 0.9, 0.55),
            Relation::Neutral => Color::srgb(0.8, 0.8, 0.85),
            Relation::Enemy => Color::srgb(1.0, 0.4, 0.35),
        }
    }

    /// Neutre → Allié → Ennemi → Neutre.
    pub fn next(self) -> Self {
        match self {
            Relation::Neutral => Relation::Ally,
            Relation::Ally => Relation::Enemy,
            Relation::Enemy => Relation::Neutral,
        }
    }

    pub fn can_attack(self) -> bool {
        self != Relation::Ally
    }
}

/// Identifiant d'une faction : la guilde si le joueur en a une, sinon lui-même.
pub fn faction_key(gid: u64, name: &str) -> String {
    if gid == 0 { format!("p:{name}") } else { format!("g:{gid:x}") }
}

/// Nom affiché d'une faction.
pub fn faction_label(tag: &str, name: &str) -> String {
    if tag.is_empty() { name.to_string() } else { format!("la guilde [{tag}]") }
}

fn declared_in(allies: &[String], enemies: &[String], key: &str) -> Relation {
    if enemies.iter().any(|k| k == key) {
        Relation::Enemy
    } else if allies.iter().any(|k| k == key) {
        Relation::Ally
    } else {
        Relation::Neutral
    }
}

/// Identifiant de ma propre guilde (0 = sans guilde).
pub fn my_gid(settings: &GameSettings) -> u64 {
    settings.guild.as_ref().map_or(0, |g| g.id)
}

/// Mes déclarations (alliés, ennemis) : celles de ma guilde si j'en ai une
/// (décidées par son Chef et ses Sous-chefs), sinon mes déclarations personnelles.
pub fn declared_lists(settings: &GameSettings) -> (Vec<String>, Vec<String>) {
    match &settings.guild {
        Some(g) => {
            let keys = |r: Relation| {
                g.relations.iter().filter(|x| x.relation == r).map(|x| x.key.clone()).collect()
            };
            (keys(Relation::Ally), keys(Relation::Enemy))
        }
        None => (settings.allies.clone(), settings.enemies.clone()),
    }
}

/// Ce que j'ai (ou ma guilde a) déclaré envers cette faction.
pub fn my_declared(settings: &GameSettings, key: &str) -> Relation {
    let (allies, enemies) = declared_lists(settings);
    declared_in(&allies, &enemies, key)
}

/// Ce que ce joueur (ou sa guilde) a déclaré envers ma faction.
pub fn their_declared(peer: &Peer, settings: &GameSettings) -> Relation {
    let my_key = faction_key(my_gid(settings), &settings.player_name);
    declared_in(&peer.status.allies, &peer.status.enemies, &my_key)
}

pub fn same_guild(peer: &Peer, settings: &GameSettings) -> bool {
    peer.gid != 0 && peer.gid == my_gid(settings)
}

pub(crate) fn combine(same_guild: bool, mine: Relation, theirs: Relation) -> Relation {
    if same_guild {
        Relation::Ally
    } else if mine == Relation::Enemy || theirs == Relation::Enemy {
        Relation::Enemy
    } else if mine == Relation::Ally && theirs == Relation::Ally {
        Relation::Ally
    } else {
        Relation::Neutral
    }
}

/// Relation effective entre ce joueur et moi.
pub fn relation_with(peer: &Peer, settings: &GameSettings) -> Relation {
    let mine = my_declared(settings, &faction_key(peer.gid, &peer.name));
    combine(same_guild(peer, settings), mine, their_declared(peer, settings))
}

/// Change ma position personnelle envers une faction (joueur sans guilde).
pub fn set_personal(settings: &mut GameSettings, key: &str, relation: Relation) {
    settings.allies.retain(|k| k != key);
    settings.enemies.retain(|k| k != key);
    let list = match relation {
        Relation::Ally => &mut settings.allies,
        Relation::Enemy => &mut settings.enemies,
        Relation::Neutral => {
            settings.save();
            return;
        }
    };
    list.push(key.to_string());
    // Les plus anciennes déclarations sont oubliées au-delà de la limite
    if list.len() > MAX_RELATIONS {
        list.remove(0);
    }
    settings.save();
}

#[cfg(test)]
mod tests {
    use super::*;
    use Relation::*;

    #[test]
    fn alliance_needs_both_sides_and_war_only_one() {
        assert_eq!(combine(false, Neutral, Neutral), Neutral);
        assert_eq!(combine(false, Ally, Neutral), Neutral); // alliance seulement proposée
        assert_eq!(combine(false, Ally, Ally), Ally);
        assert_eq!(combine(false, Ally, Enemy), Enemy);
        assert_eq!(combine(false, Neutral, Enemy), Enemy);
        assert_eq!(combine(false, Enemy, Ally), Enemy);
        // Même guilde : alliés quoi qu'il arrive
        assert_eq!(combine(true, Enemy, Enemy), Ally);
    }

    #[test]
    fn only_allies_are_protected() {
        assert!(!Ally.can_attack());
        assert!(Neutral.can_attack());
        assert!(Enemy.can_attack());
    }

    #[test]
    fn factions_are_guilds_or_lone_players() {
        assert_eq!(faction_key(0xABC, "Bob"), "g:abc");
        assert_eq!(faction_key(0, "Bob"), "p:Bob");
        assert_eq!(declared_in(&["g:ABC".into()], &[], "g:ABC"), Ally);
        assert_eq!(declared_in(&[], &["p:Bob".into()], "p:Bob"), Enemy);
        assert_eq!(declared_in(&[], &[], "p:Bob"), Neutral);
    }
}
