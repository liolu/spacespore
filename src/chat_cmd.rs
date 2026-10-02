// ─────────────────────────────────────────────────────────────────────────
//  Commandes du chat (locales : elles ne sont jamais envoyées aux autres joueurs)
//
//    /tp <n>         : va au trou noir de la galaxie n (0 = notre galaxie)
//    /tp <type>      : va à la première galaxie de ce type (ex. /tp annulaire)
//    /tp liste       : les types de galaxies et leurs numéros
//    /profil         : exporte en JSON le profil de l'astre ciblé (étoile, planète, lune)
//    /graine         : la graine du monde en code court (copiée dans le presse-papiers)
//    /graine <code>  : la graine qui correspond à un code
//    /aller ...      : tests, aller à un type d'étoile, de planète ou de lune (`test_cmd.rs`)
//    /stats [n|tout] : statistiques de tous les astres d'une galaxie (`stats.rs`), F3 : masquer
//    /aide           : la liste des commandes
// ─────────────────────────────────────────────────────────────────────────

use bevy::prelude::*;

use crate::galaxy_shape::GalaxyKind;
use crate::net::Net;
use crate::planet::{StarId, StarRoot};
use crate::planetgen::cache::{profile_of, ProfileCache};
use crate::planetgen::live::BodyId;
use crate::planetgen::seed_code;
use crate::settings::{data_dir, GameSettings};
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

const COMMANDS: [&str; 11] = ["/tp", "/galaxie", "/profil", "/profile", "/graine", "/seed", "/aller", "/go", "/stats", "/aide", "/help"];

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
    "Commandes : /tp <n> (trou noir de la galaxie n, 0 = la notre), /tp <type> (ex. /tp annulaire), /tp liste, /profil (exporte l'astre cible en JSON), /graine (code du monde), /aller (tests : /aller planete ocean, /aller etoile geante...), /stats (statistiques de la galaxie, /stats tout, F3), /aide. /g message : chat de guilde."
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

/// Astre du monde visé par la caméra. `star_loaded` : l'étoile ciblée est celle du système chargé
/// (id = système × 1000 + n) et non une étoile lointaine (id = indice du système).
pub(crate) fn target_body(kind: &TargetKind, star_loaded: bool) -> Option<BodyId> {
    Some(match *kind {
        TargetKind::Planet(id) => BodyId::Planet { system: (id / 1000) as u32, index: (id % 1000) as u16 },
        TargetKind::Moon(id, m) => BodyId::Moon { system: (id / 1000) as u32, planet: (id % 1000) as u16, index: m as u16 },
        TargetKind::Star(id) if star_loaded => BodyId::Star { system: (id / 1000) as u32, index: (id % 1000) as u16 },
        TargetKind::Star(id) => BodyId::Star { system: id as u32, index: 0 },
        _ => return None,
    })
}

fn copy_to_clipboard(text: &str) -> bool {
    arboard::Clipboard::new().and_then(|mut c| c.set_text(text.to_string())).is_ok()
}

/// Écrit le profil JSON de l'astre ciblé dans `saves/vX.Y.Z/profils/` et le copie.
fn export_profile(settings: &GameSettings, cache: &ProfileCache, kind: &TargetKind, star_loaded: bool) -> String {
    let Some(id) = target_body(kind, star_loaded) else {
        return "Ciblez une etoile, une planete ou une lune, puis tapez /profil.".into();
    };
    let Some(profile) = profile_of(settings, cache, id) else {
        return "Cet astre n'existe pas (ou plus).".into();
    };
    let name = profile.name().to_string();
    let Ok(json) = serde_json::to_string_pretty(&profile) else { return "Export impossible.".into() };
    let dir = data_dir().join("profils");
    let path = dir.join(format!("{}.json", id.key()));
    let written = std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&path, &json)).is_ok();
    let copied = copy_to_clipboard(&json);
    match (written, copied) {
        (true, true) => format!("Profil de {name} exporte : {} (copie dans le presse-papiers).", path.display()),
        (true, false) => format!("Profil de {name} exporte : {}.", path.display()),
        (false, true) => format!("Profil de {name} copie dans le presse-papiers (ecriture du fichier impossible)."),
        (false, false) => "Export impossible : ni fichier ni presse-papiers.".into(),
    }
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
    mut go: EventWriter<crate::test_cmd::GoCommand>,
    mut stats: EventWriter<crate::stats::StatsCommand>,
    profiles: Res<ProfileCache>,
    star_q: Query<&StarId, With<StarRoot>>,
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
            // Tests : aller à un type d'étoile, de planète ou de lune (`test_cmd.rs`)
            "/aller" | "/go" => {
                go.send(crate::test_cmd::GoCommand(arg.to_string()));
            }
            // Statistiques de tous les astres d'une galaxie (`stats.rs`)
            "/stats" => {
                stats.send(crate::stats::StatsCommand(arg.to_string()));
            }
            "/profil" | "/profile" => {
                let loaded = matches!(target.0, TargetKind::Star(id) if star_q.iter().any(|s| s.0 == id));
                net.notify(&export_profile(&settings, &profiles, &target.0, loaded), now);
            }
            "/graine" | "/seed" if !arg.is_empty() => match seed_code::decode(arg) {
                Some(seed) => net.notify(&format!("Le code {} est celui de la graine {seed}.", seed_code::encode(seed)), now),
                None => net.notify("Code de graine invalide (ex. K7Q2-M9XA).", now),
            },
            "/graine" | "/seed" => {
                let code = seed_code::encode(settings.world_seed);
                let copied = if copy_to_clipboard(&code) { " (copie dans le presse-papiers)" } else { "" };
                net.notify(&format!("Graine du monde : {code}{copied}."), now);
            }
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
        assert!(is_local("/profil"));
        assert!(is_local("/graine"));
        assert!(is_local("/aller planete lave"));
        assert!(is_local("/stats tout"));
        assert!(!is_local("/g salut"));
        assert!(!is_local("bonjour /tp"));
    }

    #[test]
    fn targets_become_world_bodies() {
        assert_eq!(target_body(&TargetKind::Planet(12_002), false), Some(BodyId::Planet { system: 12, index: 2 }));
        assert_eq!(target_body(&TargetKind::Moon(5_001, 1), false), Some(BodyId::Moon { system: 5, planet: 1, index: 1 }));
        assert_eq!(target_body(&TargetKind::Star(7_000), true), Some(BodyId::Star { system: 7, index: 0 }));
        assert_eq!(target_body(&TargetKind::Star(7_000), false), Some(BodyId::Star { system: 7_000, index: 0 }));
        assert_eq!(target_body(&TargetKind::GalacticCore, false), None);
    }

    #[test]
    fn every_target_kind_exports_a_profile() {
        let settings = GameSettings::default();
        // Un système dont la première planète a une lune
        let si = (0..).find(|&s| !settings.systems[s].planets()[0].moons.is_empty()).unwrap();
        let s = si as u32;
        let cache = ProfileCache::build(&settings, si);
        for id in [
            BodyId::Star { system: s, index: 0 },
            BodyId::Planet { system: s, index: 0 },
            BodyId::Moon { system: s, planet: 0, index: 0 },
            // Hors du système chargé : calculé à la demande
            BodyId::Planet { system: 4_000, index: 0 },
        ] {
            let profile = profile_of(&settings, &cache, id).unwrap_or_else(|| panic!("{}", id.key()));
            assert_eq!(profile.id(), id.key());
            // Les sections gardent leur ordre : identité d'abord
            let json = serde_json::to_string_pretty(&profile).unwrap();
            assert!(json.trim_start().starts_with("{
  \"id\""), "{json}");
        }
        assert!(profile_of(&settings, &cache, BodyId::Planet { system: s, index: 999 }).is_none());
        assert!(profile_of(&settings, &cache, BodyId::Planet { system: u32::MAX, index: 0 }).is_none());
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
