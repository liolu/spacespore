// ─────────────────────────────────────────────────────────────────────────
//  Commandes du chat (locales : elles ne sont jamais envoyées aux autres joueurs)
//
//    /tp <n>         : va au trou noir de la galaxie n (0 = notre galaxie)
//    /tp <type>      : va à la première galaxie de ce type (ex. /tp annulaire)
//    /tp liste       : les types de galaxies et leurs numéros
//    /aide           : la liste des commandes
// ─────────────────────────────────────────────────────────────────────────

use bevy::prelude::*;

use crate::galaxy_shape::GalaxyKind;
use crate::net::Net;
use crate::settings::GameSettings;
use crate::ui::{CameraTarget, TargetKind};
use crate::wormhole::WormholeTravel;
use crate::CameraController;

pub struct ChatCmdPlugin;

impl Plugin for ChatCmdPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<ChatCommand>().add_systems(Update, run_chat_commands);
    }
}

/// Une ligne tapée dans le chat qui reste sur cette machine.
#[derive(Event)]
pub struct ChatCommand(pub String);

const COMMANDS: [&str; 4] = ["/tp", "/galaxie", "/aide", "/help"];

/// La ligne est une commande du jeu (et non un message à envoyer).
pub fn is_local(line: &str) -> bool {
    let word = line.trim_start().split_whitespace().next().unwrap_or("");
    COMMANDS.contains(&word.to_lowercase().as_str())
}

/// Minuscules sans accents, pour comparer « Spirale barrée » et « spirale barree ».
fn plain(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| match c {
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'à' | 'â' | 'ä' => 'a',
            'î' | 'ï' => 'i',
            'ô' | 'ö' => 'o',
            'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            _ => c,
        })
        .collect()
}

/// Galaxie visée par l'argument de `/tp` : un numéro, « maison », ou un type de galaxie.
fn find_galaxy(arg: &str, settings: &GameSettings, current: usize) -> Result<usize, String> {
    let count = settings.galaxies.len();
    let arg = plain(arg.trim());
    if let Ok(n) = arg.parse::<usize>() {
        return if n < count {
            Ok(n)
        } else {
            Err(format!("Il n'y a pas de galaxie {n} : choisissez de 0 a {}.", count - 1))
        };
    }
    if matches!(arg.as_str(), "maison" | "home" | "nous" | "ici") {
        return Ok(0);
    }
    let matches: Vec<usize> = settings
        .galaxies
        .iter()
        .enumerate()
        .filter(|(_, g)| plain(g.kind.name()).contains(&arg))
        .map(|(i, _)| i)
        .collect();
    // Première galaxie de ce type, autre que celle où l'on se trouve
    matches
        .iter()
        .copied()
        .find(|&i| i != current)
        .or_else(|| matches.first().copied())
        .ok_or_else(|| format!("Aucune galaxie ne correspond a \"{arg}\". Essayez /tp liste."))
}

fn help() -> &'static str {
    "Commandes : /tp <n> (trou noir de la galaxie n, 0 = la notre), /tp <type> (ex. /tp annulaire), /tp liste, /aide. /g message : chat de guilde."
}

fn list_kinds(settings: &GameSettings) -> String {
    let mut parts = Vec::new();
    for kind in GalaxyKind::ALL {
        let ids: Vec<String> = settings
            .galaxies
            .iter()
            .enumerate()
            .filter(|(_, g)| g.kind == kind)
            .take(4)
            .map(|(i, _)| i.to_string())
            .collect();
        if !ids.is_empty() {
            parts.push(format!("{} ({})", kind.name(), ids.join(", ")));
        }
    }
    format!("Galaxies 0 a {} : {}", settings.galaxies.len() - 1, parts.join(" - "))
}

#[allow(clippy::too_many_arguments)]
fn run_chat_commands(
    time: Res<Time>,
    mut events: EventReader<ChatCommand>,
    settings: Res<GameSettings>,
    travel: Res<WormholeTravel>,
    mut target: ResMut<CameraTarget>,
    mut cam_q: Query<&mut CameraController>,
    mut net: ResMut<Net>,
) {
    let now = time.elapsed_secs_f64();
    for ChatCommand(line) in events.read() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if !line.starts_with('/') {
            net.notify("Le chat demande le multijoueur : ouvrez le panneau F2. Tapez /aide pour les commandes.", now);
            continue;
        }
        let mut words = line.splitn(2, char::is_whitespace);
        let command = words.next().unwrap_or("").to_lowercase();
        let arg = words.next().unwrap_or("").trim();
        match command.as_str() {
            "/aide" | "/help" => net.notify(help(), now),
            "/tp" | "/galaxie" => {
                if arg.is_empty() {
                    net.notify(help(), now);
                    continue;
                }
                if plain(arg) == "liste" || plain(arg) == "list" {
                    net.notify(&list_kinds(&settings), now);
                    continue;
                }
                if travel.active() {
                    net.notify("Impossible de se teleporter pendant un voyage en trou de ver.", now);
                    continue;
                }
                let current = match target.0 {
                    TargetKind::DistantGalaxyCore(id) => id as usize,
                    _ => 0,
                };
                match find_galaxy(arg, &settings, current) {
                    Ok(id) => {
                        // Même cible que le clic sur le trou noir : le vaisseau saute jusque-là
                        target.0 = if id == 0 { TargetKind::GalacticCore } else { TargetKind::DistantGalaxyCore(id as u32) };
                        if let Ok(mut ctrl) = cam_q.get_single_mut() {
                            ctrl.zoom_goal = Some(crate::galaxy_view_distance(&target.0, &settings));
                        }
                        net.local.siege = None;
                        let what = settings.galaxies.get(id).map_or("", |g| g.kind.name());
                        net.notify(&format!("Teleportation au trou noir de la galaxie {id} ({what})."), now);
                    }
                    Err(e) => net.notify(&e, now),
                }
            }
            _ => net.notify("Commande inconnue. Tapez /aide.", now),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_game_commands_stay_local() {
        assert!(is_local("/tp 3"));
        assert!(is_local("  /TP annulaire"));
        assert!(is_local("/aide"));
        assert!(!is_local("/g salut"));
        assert!(!is_local("bonjour /tp"));
    }

    #[test]
    fn galaxies_are_found_by_number_home_or_type() {
        let settings = GameSettings::default();
        assert_eq!(find_galaxy("0", &settings, 5), Ok(0));
        assert_eq!(find_galaxy("maison", &settings, 5), Ok(0));
        assert!(find_galaxy("9999", &settings, 0).is_err());
        assert!(find_galaxy("zzz", &settings, 0).is_err());
        let id = find_galaxy("spirale barree", &settings, 0).unwrap();
        assert_eq!(settings.galaxies[id].kind, GalaxyKind::Barred);
        // Sans accent ou avec : pareil
        assert_eq!(find_galaxy("Spirale barrée", &settings, 0), Ok(id));
    }
}
