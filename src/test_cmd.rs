// ─────────────────────────────────────────────────────────────────────────
//  Commandes de test : aller directement à un type d'étoile, de planète ou de lune
//
//    /aller                       : la liste des types
//    /aller etoile <type>         : o, b, a, f, g, k, m, blanche, brune, sous-geante, geante
//    /aller planete <type>        : rocheuse, gazeuse, ocean, lave, titan, vie, anneaux...
//    /aller lune <type>           : volcanique, ocean-cache, air, glacee, vie, rare
//    /aller suivant               : le suivant du même type
//
//  On cherche à partir du système où l'on est, dans l'ordre des systèmes (au plus
//  `SEARCH_LIMIT` systèmes par commande, `/aller suivant` continue). Le vaisseau est téléporté
//  au bord du système trouvé ; une fois le système chargé, l'astre est ciblé et la caméra zoome.
// ─────────────────────────────────────────────────────────────────────────

use bevy::prelude::*;
use bevy::tasks::{block_on, futures_lite::future, AsyncComputeTaskPool, Task};

use crate::net::Net;
use crate::planet::{MoonId, MoonRoot, PlanetId, PlanetRoot, SpawnedSystems, StarId, StarRoot};
use crate::planetgen::hydrology::{Liquid, WaterState};
use crate::planetgen::life::LifeLevel;
use crate::planetgen::live::BodyId;
use crate::planetgen::star::StarClass;
use crate::planetgen::system::{size_class, PlanetKind};
use crate::settings::{GameSettings, MoonConfig, PlanetConfig, StarConfig, StarSystemConfig};
use crate::ship::Ship;
use crate::ui::{CameraTarget, TargetKind};
use crate::CameraController;

pub struct TestCmdPlugin;

impl Plugin for TestCmdPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<GoCommand>()
            .init_resource::<GoState>()
            .add_systems(Update, (run_go_commands, finish_search, finish_arrival).chain());
    }
}

/// `/aller ...` tapé dans le chat (l'argument, sans le mot de commande).
#[derive(Event)]
pub struct GoCommand(pub String);

/// Systèmes examinés au plus par commande (les planètes sont recalculées pour chercher).
const SEARCH_LIMIT: usize = 4000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    Star,
    Planet,
    Moon,
}

#[derive(Resource, Default)]
struct GoState {
    /// Dernière recherche et dernier système trouvé (pour `/aller suivant`).
    last: Option<(Family, String, usize)>,
    /// Recherche en arrière-plan (planètes et lunes : les recalculer prend du temps).
    search: Option<(Family, String, usize, Task<Result<Option<BodyId>, ()>>)>,
    /// Arrivée en cours : système, astre visé, instant limite.
    pending: Option<(usize, BodyId, f64)>,
}

pub const STAR_TYPES: &str = "o, b, a, f, g, k, m, blanche, brune, sous-geante, geante";
pub const PLANET_TYPES: &str = "rocheuse, mini-neptune, neptune, gazeuse, jupiter-chaud, minuscule, petite, terrestre, \
super-terre, ocean, glace, lave, methane, ammoniac, venus, titan, mars, oxygene, sans-air, vie, plantes, complexe, \
anneaux, aurores, plaques, volcans, crateres, habitable, rare, legendaire";
pub const MOON_TYPES: &str = "volcanique, ocean-cache, air, glacee, lave, vie, rare";

pub fn help() -> String {
    format!(
        "/aller etoile <type> : {STAR_TYPES}.  /aller planete <type> : {PLANET_TYPES}.  /aller lune <type> : {MOON_TYPES}.  /aller suivant : le suivant."
    )
}

fn plain(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| match c {
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'à' | 'â' => 'a',
            'î' | 'ï' => 'i',
            'ô' => 'o',
            'û' | 'ù' => 'u',
            'ç' => 'c',
            ' ' | '_' => '-',
            _ => c,
        })
        .collect()
}

/// L'étoile correspond-elle au type ? `None` : type inconnu.
pub fn star_matches(star: &StarConfig, kind: &str) -> Option<bool> {
    let class = match kind {
        "o" => StarClass::O,
        "b" => StarClass::B,
        "a" => StarClass::A,
        "f" => StarClass::F,
        "g" => StarClass::G,
        "k" => StarClass::K,
        "m" => StarClass::M,
        "blanche" | "naine-blanche" => StarClass::WhiteDwarf,
        "brune" | "naine-brune" => StarClass::BrownDwarf,
        "sous-geante" => StarClass::Subgiant,
        "geante" | "geante-rouge" => StarClass::RedGiant,
        _ => return None,
    };
    Some(star.class == class)
}

fn liquid_water(p: &PlanetConfig) -> bool {
    p.hydrology.hydro.liquid == Liquid::Water && p.hydrology.water_state == WaterState::Liquid
}

fn rolled_trait(traits: &[crate::planetgen::profile::Trait], max_rarity: f64) -> bool {
    traits.iter().any(|t| t.rarity <= max_rarity)
}

/// La planète correspond-elle au type ? `None` : type inconnu.
pub fn planet_matches(p: &PlanetConfig, kind: &str) -> Option<bool> {
    let size = size_class(p.kind, p.radius_earth as f64, p.hot);
    let gas = |f: &str| p.air.fraction(f);
    Some(match kind {
        "rocheuse" => p.kind == PlanetKind::Rocky,
        "mini-neptune" => p.kind == PlanetKind::MiniNeptune,
        "neptune" | "geante-glace" => p.kind == PlanetKind::IceGiant,
        "gazeuse" | "geante" | "jupiter" => p.kind == PlanetKind::GasGiant && !p.hot,
        "jupiter-chaud" => p.kind == PlanetKind::GasGiant && p.hot,
        "minuscule" | "petite" | "terrestre" | "super-terre" => size == kind.replace("super-terre", "super-Terre"),
        "ocean" => p.hydrology.ocean_fraction > 0.85 && liquid_water(p),
        "glace" => p.hydrology.hydro.liquid == Liquid::Water && p.hydrology.water_state == WaterState::Ice,
        "lave" => p.hydrology.hydro.liquid == Liquid::Lava,
        "methane" => p.hydrology.hydro.liquid == Liquid::Methane,
        "ammoniac" => p.hydrology.hydro.liquid == Liquid::Ammonia,
        "venus" => p.air.pressure_bar > 30.0 && !p.gaseous(),
        "titan" => gas("CH4") > 0.01 && !p.gaseous(),
        "mars" => gas("CO2") > 0.5 && p.air.pressure_bar < 0.1 && p.atmosphere,
        "oxygene" => gas("O2") > 0.05,
        "sans-air" => !p.gaseous() && !p.atmosphere,
        "vie" => p.life.level >= LifeLevel::Microbial,
        "plantes" => p.life.flora,
        "complexe" => p.life.level == LifeLevel::Complex,
        "anneaux" => p.ring.is_some(),
        "aurores" => p.aurora.is_some(),
        "plaques" => p.geology.tectonics == crate::planetgen::geology::Tectonics::Plates,
        "volcans" => p.geology.relief.volcanoes >= 10,
        "crateres" => p.geology.relief.craters > 0.6,
        "habitable" => p.habitability.score > 0.3,
        "rare" => rolled_trait(&p.traits, 0.015),
        "legendaire" => rolled_trait(&p.traits, 0.001),
        _ => return None,
    })
}

/// La lune correspond-elle au type ? `None` : type inconnu.
pub fn moon_matches(m: &MoonConfig, kind: &str) -> Option<bool> {
    Some(match kind {
        "volcanique" => m.tidal_heat > 0.2 && m.geology.volcanism > 0.3,
        "ocean-cache" => m.hydrology.subsurface_ocean,
        "air" => m.atmosphere,
        "glacee" => m.hydrology.water_state == WaterState::Ice,
        "lave" => m.hydrology.hydro.liquid == Liquid::Lava,
        "vie" => m.life.level >= LifeLevel::Microbial,
        "rare" => rolled_trait(&m.traits, 0.015),
        _ => return None,
    })
}

/// Premier astre du type dans un système.
fn match_in(sys: &StarSystemConfig, si: usize, family: Family, kind: &str) -> Result<Option<BodyId>, ()> {
    let system = si as u32;
    match family {
        Family::Star => match sys.stars.first().map(|s| star_matches(s, kind)) {
            Some(None) => Err(()),
            Some(Some(true)) => Ok(Some(BodyId::Star { system, index: 0 })),
            _ => Ok(None),
        },
        Family::Planet | Family::Moon => {
            // Type valide ? (sur une planète / lune vide)
            let valid = match family {
                Family::Planet => planet_matches(&PlanetConfig::default(), kind).is_some(),
                _ => moon_matches(&MoonConfig::default(), kind).is_some(),
            };
            if !valid {
                return Err(());
            }
            let planets = sys.planets_uncached();
            for (pi, p) in planets.iter().enumerate() {
                if family == Family::Planet {
                    if planet_matches(p, kind) == Some(true) {
                        return Ok(Some(BodyId::Planet { system, index: pi as u16 }));
                    }
                    continue;
                }
                for (mi, m) in p.moons.iter().enumerate() {
                    if moon_matches(m, kind) == Some(true) {
                        return Ok(Some(BodyId::Moon { system, planet: pi as u16, index: mi as u16 }));
                    }
                }
            }
            Ok(None)
        }
    }
}

/// Cherche le premier astre du type après le système `after` (en faisant le tour).
/// `Err` : type inconnu ; `Ok(None)` : rien dans les `limit` systèmes examinés.
pub fn find(settings: &GameSettings, family: Family, kind: &str, after: usize, limit: usize) -> Result<Option<BodyId>, ()> {
    let n = settings.systems.len();
    if n == 0 {
        return Ok(None);
    }
    // Les étoiles ne demandent pas de recalculer les planètes : on peut tout parcourir
    let limit = if family == Family::Star { n } else { limit.min(n) };
    for k in 1..=limit {
        let si = (after + k) % n;
        if let Some(id) = match_in(&settings.systems[si], si, family, kind)? {
            return Ok(Some(id));
        }
    }
    Ok(None)
}

fn describe(family: Family) -> &'static str {
    match family {
        Family::Star => "etoile",
        Family::Planet => "planete",
        Family::Moon => "lune",
    }
}

/// Cherche dans une liste de systèmes déjà copiés (tâche d'arrière-plan).
fn find_in(candidates: &[(usize, StarSystemConfig)], family: Family, kind: &str) -> Result<Option<BodyId>, ()> {
    for (si, sys) in candidates {
        if let Some(id) = match_in(sys, *si, family, kind)? {
            return Ok(Some(id));
        }
    }
    Ok(None)
}

/// Type compris pour cette famille ?
fn known_type(family: Family, kind: &str) -> bool {
    match family {
        Family::Star => star_matches(&StarConfig::default(), kind).is_some(),
        Family::Planet => planet_matches(&PlanetConfig::default(), kind).is_some(),
        Family::Moon => moon_matches(&MoonConfig::default(), kind).is_some(),
    }
}

/// Téléporte le vaisseau au bord du système de l'astre trouvé ; `finish_arrival` le ciblera.
#[allow(clippy::too_many_arguments)]
fn start_travel(
    id: BodyId,
    family: Family,
    kind: &str,
    settings: &GameSettings,
    state: &mut GoState,
    target: &mut CameraTarget,
    net: &mut Net,
    ship_q: &mut Query<&mut Transform, With<Ship>>,
    now: f64,
) {
    let si = id.system();
    let Some(sys) = settings.systems.get(si) else { return };
    state.last = Some((family, kind.to_string(), si));
    state.pending = Some((si, id, now + 30.0));
    target.0 = TargetKind::Star(si);
    if let Ok(mut ship) = ship_q.get_single_mut() {
        let scale = sys.stars.first().map_or(1_000_000.0, |s| s.scale());
        ship.translation = sys.center() + Vec3::new(scale * 2.0, scale * 0.3, 0.0);
    }
    net.local.siege = None;
    net.notify(&format!("Test : {} \"{kind}\" trouvee dans {} ({}). Arrivee...", describe(family), sys.name, id.key()), now);
}

#[allow(clippy::too_many_arguments)]
fn run_go_commands(
    time: Res<Time>,
    mut events: EventReader<GoCommand>,
    settings: Res<GameSettings>,
    spawned: Res<SpawnedSystems>,
    surface: Res<crate::surface::Surface>,
    travel: Res<crate::wormhole::WormholeTravel>,
    mut state: ResMut<GoState>,
    mut target: ResMut<CameraTarget>,
    mut net: ResMut<Net>,
    mut ship_q: Query<&mut Transform, With<Ship>>,
) {
    let now = time.elapsed_secs_f64();
    for GoCommand(arg) in events.read() {
        let words: Vec<String> = arg.split_whitespace().map(plain).collect();
        let (family, kind, after) = match words.first().map(String::as_str) {
            None => {
                net.notify(&help(), now);
                continue;
            }
            Some("suivant" | "next") => match &state.last {
                Some((f, k, si)) => (*f, k.clone(), *si),
                None => {
                    net.notify("Aucune recherche precedente : /aller planete ocean, par exemple.", now);
                    continue;
                }
            },
            Some(first) => {
                let family = match first {
                    "etoile" | "star" => Family::Star,
                    "planete" | "planet" => Family::Planet,
                    "lune" | "moon" => Family::Moon,
                    _ => {
                        net.notify(&help(), now);
                        continue;
                    }
                };
                let Some(kind) = words.get(1) else {
                    net.notify(&help(), now);
                    continue;
                };
                (family, kind.clone(), spawned.0.iter().next().copied().unwrap_or(0))
            }
        };
        if surface.active() {
            net.notify("Redecollez d'abord (V), ou revenez en orbite.", now);
            continue;
        }
        if travel.active() {
            net.notify("Impossible pendant un voyage en trou de ver.", now);
            continue;
        }
        if state.search.is_some() {
            net.notify("Une recherche est deja en cours, patientez.", now);
            continue;
        }
        if !known_type(family, &kind) {
            let types = match family {
                Family::Star => STAR_TYPES,
                Family::Planet => PLANET_TYPES,
                Family::Moon => MOON_TYPES,
            };
            net.notify(&format!("Type inconnu \"{kind}\". Types de {} : {types}.", describe(family)), now);
            continue;
        }
        let n = settings.systems.len();
        if n == 0 {
            continue;
        }
        // Étoiles : rien à recalculer, la recherche est immédiate
        if family == Family::Star {
            match find(&settings, family, &kind, after, n) {
                Ok(Some(id)) => start_travel(id, family, &kind, &settings, &mut state, &mut target, &mut net, &mut ship_q, now),
                _ => net.notify(&format!("Aucune etoile \"{kind}\" dans cet univers."), now),
            }
            continue;
        }
        // Planètes et lunes : on copie les systèmes à examiner et on cherche en arrière-plan,
        // sans figer le jeu
        let candidates: Vec<(usize, StarSystemConfig)> = (1..=SEARCH_LIMIT.min(n))
            .map(|k| (after + k) % n)
            .map(|si| (si, settings.systems[si].clone()))
            .collect();
        let task_kind = kind.clone();
        let task = AsyncComputeTaskPool::get().spawn(async move { find_in(&candidates, family, &task_kind) });
        state.search = Some((family, kind.clone(), after, task));
        net.notify(&format!("Recherche d'une {} \"{kind}\"...", describe(family)), now);
    }
}

/// Fin d'une recherche en arrière-plan : départ, ou « rien trouvé ».
#[allow(clippy::too_many_arguments)]
fn finish_search(
    time: Res<Time>,
    settings: Res<GameSettings>,
    mut state: ResMut<GoState>,
    mut target: ResMut<CameraTarget>,
    mut net: ResMut<Net>,
    mut ship_q: Query<&mut Transform, With<Ship>>,
) {
    let Some((_, _, _, task)) = state.search.as_mut() else { return };
    let Some(result) = block_on(future::poll_once(task)) else { return };
    let (family, kind, after, _) = state.search.take().unwrap();
    let now = time.elapsed_secs_f64();
    match result {
        Ok(Some(id)) => start_travel(id, family, &kind, &settings, &mut state, &mut target, &mut net, &mut ship_q, now),
        Ok(None) => {
            state.last = Some((family, kind.clone(), (after + SEARCH_LIMIT) % settings.systems.len().max(1)));
            net.notify(&format!("Aucune {} \"{kind}\" dans les {SEARCH_LIMIT} systemes suivants. /aller suivant pour continuer.", describe(family)), now);
        }
        Err(()) => net.notify(&format!("Type inconnu \"{kind}\"."), now),
    }
}

/// Une fois le système chargé, cible l'astre trouvé et zoome dessus.
#[allow(clippy::too_many_arguments)]
fn finish_arrival(
    time: Res<Time>,
    settings: Res<GameSettings>,
    spawned: Res<SpawnedSystems>,
    mut state: ResMut<GoState>,
    mut target: ResMut<CameraTarget>,
    mut net: ResMut<Net>,
    planets: Query<&PlanetId, With<PlanetRoot>>,
    moons: Query<&MoonId, With<MoonRoot>>,
    stars: Query<&StarId, With<StarRoot>>,
    mut cam_q: Query<&mut CameraController>,
) {
    let Some((si, id, until)) = state.pending else { return };
    let now = time.elapsed_secs_f64();
    if now > until {
        state.pending = None;
        net.notify("Le systeme ne s'est pas charge : rapprochez-vous de l'etoile ciblee.", now);
        return;
    }
    if !spawned.0.contains(&si) {
        return;
    }
    let Some(sys) = settings.systems.get(si) else { return };
    let (kind, radius) = match id {
        BodyId::Star { index, .. } => {
            let sid = si * 1000 + index as usize;
            if !stars.iter().any(|s| s.0 == sid) {
                return;
            }
            (TargetKind::Star(sid), sys.stars.get(index as usize).map_or(1_000_000.0, |s| s.radius))
        }
        BodyId::Planet { index, .. } => {
            let pid = si * 1000 + index as usize;
            if !planets.iter().any(|p| p.0 == pid) {
                return;
            }
            (TargetKind::Planet(pid), sys.planets().get(index as usize).map_or(10_000.0, |p| p.radius))
        }
        BodyId::Moon { planet, index, .. } => {
            let pid = si * 1000 + planet as usize;
            if !moons.iter().any(|m| m.planet_idx == pid && m.moon_idx == index as usize) {
                return;
            }
            let r = sys.planets().get(planet as usize).and_then(|p| p.moons.get(index as usize)).map_or(3_000.0, |m| m.radius);
            (TargetKind::Moon(pid, index as usize), r)
        }
    };
    target.0 = kind;
    if let Ok(mut ctrl) = cam_q.get_single_mut() {
        ctrl.zoom_goal = Some(radius * 4.0);
    }
    state.pending = None;
    net.notify(&format!("Arrive : {} ({}). Touche I : scanner.", sys.name, id.key()), now);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_listed_type_is_understood() {
        for t in STAR_TYPES.split(", ") {
            assert!(star_matches(&StarConfig::default(), t).is_some(), "etoile {t}");
        }
        for t in PLANET_TYPES.split(", ") {
            assert!(planet_matches(&PlanetConfig::default(), t).is_some(), "planete {t}");
        }
        for t in MOON_TYPES.split(", ") {
            assert!(moon_matches(&MoonConfig::default(), t).is_some(), "lune {t}");
        }
        assert!(planet_matches(&PlanetConfig::default(), "licorne").is_none());
    }

    #[test]
    fn common_types_are_found_and_match() {
        let settings = GameSettings::default();
        for (family, kind) in [
            (Family::Star, "m"),
            (Family::Star, "geante"),
            (Family::Star, "o"),
            (Family::Planet, "gazeuse"),
            (Family::Planet, "lave"),
            (Family::Planet, "anneaux"),
            (Family::Planet, "mars"),
            (Family::Moon, "volcanique"),
            (Family::Moon, "ocean-cache"),
        ] {
            let id = find(&settings, family, kind, 0, SEARCH_LIMIT).unwrap().unwrap_or_else(|| panic!("{kind} introuvable"));
            let sys = &settings.systems[id.system()];
            let ok = match id {
                BodyId::Star { index, .. } => star_matches(&sys.stars[index as usize], kind),
                BodyId::Planet { index, .. } => planet_matches(&sys.planets()[index as usize], kind),
                BodyId::Moon { planet, index, .. } => moon_matches(&sys.planets()[planet as usize].moons[index as usize], kind),
            };
            assert_eq!(ok, Some(true), "{kind} -> {}", id.key());
        }
        assert!(find(&settings, Family::Planet, "licorne", 0, 10).is_err());
        // « suivant » trouve un autre système
        let first = find(&settings, Family::Star, "g", 0, SEARCH_LIMIT).unwrap().unwrap();
        let next = find(&settings, Family::Star, "g", first.system(), SEARCH_LIMIT).unwrap().unwrap();
        assert_ne!(first.system(), next.system());
    }
}
