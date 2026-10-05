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
            .add_event::<DescendCommand>()
            .init_resource::<Essai>()
            .add_systems(Update, (run_go_commands, finish_search, finish_arrival, run_descend, run_essai).chain())
            .add_systems(Startup, |mut started: Local<bool>| {
                if !*started {
                    *started = true;
                    if let Some(k) = std::env::var("SPACESPORE_SCALE").ok().and_then(|s| s.parse::<u32>().ok()) {
                        crate::terrain::set_voxel_scale(k);
                    }
                }
            })
            .add_systems(Update, dev_script)
            .add_systems(Last, perf_log);
    }
}

/// Tests (développement) : `SPACESPORE_TEST_CMD` = une commande du chat lancée à 6 s
/// (`SPACESPORE_TEST_CMD_SECS`) ;
/// `SPACESPORE_TEST_STAR=k` cible l'étoile k du système chargé (`sys` : son indice, comme une
/// étoile cliquée de loin) à `SPACESPORE_TEST_STAR_SECS`
/// (35 par défaut) ; les positions des étoiles et du vaisseau sont écrites dans le journal.
#[allow(clippy::too_many_arguments)]
fn dev_script(
    time: Res<Time>,
    spawned: Res<SpawnedSystems>,
    settings: Res<GameSettings>,
    mut target: ResMut<CameraTarget>,
    mut chat: EventWriter<crate::chat_cmd::ChatCommand>,
    stars: Query<(&GlobalTransform, &StarId), With<StarRoot>>,
    ship: Query<&GlobalTransform, With<Ship>>,
    mut step: Local<u8>,
    mut last_log: Local<f32>,
    mut flying: Local<bool>,
    mut second: Local<bool>,
    mut cams: Query<&mut crate::CameraController>,
    mut surface: ResMut<crate::surface::Surface>,
) {
    let t = time.elapsed_secs();
    let cmd_at: f32 = std::env::var("SPACESPORE_TEST_CMD_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(6.0);
    if *step == 0 && t > cmd_at {
        *step = 1;
        // Plusieurs commandes séparées par « ; » (dans l'ordre)
        if let Ok(cmds) = std::env::var("SPACESPORE_TEST_CMD") {
            for cmd in cmds.split(';').map(str::trim).filter(|c| !c.is_empty()) {
                chat.send(crate::chat_cmd::ChatCommand(cmd.to_string()));
            }
        }
    }
    // Seconde commande, plus tard (après un atterrissage) : `SPACESPORE_TEST_CMD2` à
    // `SPACESPORE_TEST_CMD2_SECS` (30 s par défaut)
    let cmd2_at: f32 = std::env::var("SPACESPORE_TEST_CMD2_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(30.0);
    if !*second && t > cmd2_at {
        *second = true;
        if let Ok(cmds) = std::env::var("SPACESPORE_TEST_CMD2") {
            for cmd in cmds.split(';').map(str::trim).filter(|c| !c.is_empty()) {
                chat.send(crate::chat_cmd::ChatCommand(cmd.to_string()));
            }
        }
    }
    // Mesures (0.13 E3) : `SPACESPORE_TEST_FLY` = descendre en vol bas à 8 s (puis plein gaz)
    let fly_at: f32 = std::env::var("SPACESPORE_TEST_FLY_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(8.0);
    if std::env::var("SPACESPORE_TEST_FLY").is_ok() && t > fly_at && !*flying {
        for mut c in &mut cams {
            c.zoom_goal = Some(200.0);
        }
        // Comme un coup de molette, à chaque image jusqu'au passage en vol bas
        surface.test_zoom_in(time.elapsed_secs_f64());
        *flying = surface.active();
    }
    let Ok(k) = std::env::var("SPACESPORE_TEST_STAR") else { return };
    let at: f32 = std::env::var("SPACESPORE_TEST_STAR_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(35.0);
    if *step == 1 && t > at {
        *step = 2;
        // « sys » : comme un clic sur l'étoile lointaine (indice du système)
        if let Some(&si) = spawned.0.iter().next() {
            let id = if k == "sys" { si } else { si * 1000 + k.parse::<usize>().unwrap_or(0) };
            target.0 = TargetKind::Star(id);
            info!("TEST cible etoile {id}");
        }
    }
    if t - *last_log > 3.0 && *step >= 1 {
        *last_log = t;
        let s = ship.get_single().map(|g| g.translation()).unwrap_or_default();
        for (g, id) in &stars {
            let r = settings.systems.get(id.0 / 1000).and_then(|sys| sys.stars.get(id.0 % 1000)).map_or(0.0, |c| c.radius);
            info!("TEST t={t:.0} etoile {} pos {:.0} rayon {r:.0} dist vaisseau {:.0}", id.0, g.translation(), g.translation().distance(s));
        }
        let tgt = match target.0 { TargetKind::Star(i) => format!("Star({i})"), _ => "autre".into() };
        info!("TEST t={t:.0} vaisseau {s:.0} cible {tgt} systemes {:?}", spawned.0);
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

pub const STAR_TYPES: &str = "o, b, a, f, g, k, m, blanche, brune, sous-geante, geante, double, triple";
pub const PLANET_TYPES: &str = "mer, rocheuse, mini-neptune, neptune, gazeuse, jupiter-chaud, minuscule, petite, terrestre, \
super-terre, ocean, glace, lave, methane, ammoniac, venus, titan, mars, oxygene, sans-air, vie, plantes, complexe, \
anneaux, aurores, errante, plaques, volcans, crateres, habitable, rare, legendaire";
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
        // Mer d'eau liquide, air, et une planète qui tourne (il y fait jour à un moment)
        "mer" => liquid_water(p) && p.hydrology.ocean_fraction > 0.05 && p.atmosphere && !p.tidally_locked && !p.gaseous(),
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
        "errante" => p.rogue,
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
        // Étoiles doubles et triples (C3)
        Family::Star if matches!(kind, "double" | "triple") => {
            let n = if kind == "double" { 2 } else { 3 };
            Ok((sys.stars.len() == n).then_some(BodyId::Star { system, index: 0 }))
        }
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
    // Dans notre galaxie et les extérieures (les lointaines ne sont générées qu'à l'approche)
    let n = settings.systems.dense().len();
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
        Family::Star => matches!(kind, "double" | "triple") || star_matches(&StarConfig::default(), kind).is_some(),
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
        ship.translation = sys.center() + Vec3::new(scale * 2.0, scale * 0.3, 0.0) * crate::settings::SPACE_STRETCH;
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
    mut essai: ResMut<Essai>,
) {
    let now = time.elapsed_secs_f64();
    for GoCommand(arg) in events.read() {
        let words: Vec<String> = arg.split_whitespace().map(plain).collect();
        // `/essai mer` : enchaîne /aller planete mer, /jour, /vol, /mer (voir `run_essai`)
        if words.first().map(String::as_str) == Some("essai") {
            match words.get(1).map(String::as_str) {
                Some("mer") => {
                    essai.stage = 1;
                    essai.sent = false;
                    essai.since = now;
                    net.notify("Essai \"mer\" : recherche d'une planete avec mer qui tourne, puis jour, vol bas, au-dessus de la mer.", now);
                }
                _ => net.notify("/essai mer : va de jour au-dessus d'une mer (tests de l'eau).", now),
            }
            continue;
        }
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
        let n = settings.systems.dense().len();
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

/// `/vol` : descendre en vol bas sur l'astre ciblé (comme un coup de molette, jusqu'au passage).
#[derive(Event)]
pub struct DescendCommand;

fn run_descend(
    time: Res<Time>,
    mut events: EventReader<DescendCommand>,
    mut surface: ResMut<crate::surface::Surface>,
    mut cams: Query<&mut CameraController>,
    mut net: ResMut<Net>,
    mut until: Local<f64>,
) {
    let now = time.elapsed_secs_f64();
    if events.read().next().is_some() {
        *until = now + 20.0;
        net.notify("Descente en vol bas...", now);
    }
    if now < *until {
        if surface.active() {
            *until = 0.0;
            return;
        }
        for mut c in &mut cams {
            c.zoom_goal = Some(200.0);
        }
        surface.test_zoom_in(now);
    }
}

/// `/essai mer` : étapes automatiques (1 : recherche et arrivée, 2 : jour, 3 : vol bas, 4 : mer).
#[derive(Resource, Default)]
struct Essai {
    stage: u8,
    since: f64,
    /// La recherche de l'étape 1 est lancée.
    sent: bool,
}

fn run_essai(
    time: Res<Time>,
    mut essai: ResMut<Essai>,
    state: Res<GoState>,
    target: Res<CameraTarget>,
    surface: Res<crate::surface::Surface>,
    mut chat: EventWriter<crate::chat_cmd::ChatCommand>,
    mut net: ResMut<Net>,
) {
    let now = time.elapsed_secs_f64();
    if essai.stage == 0 {
        return;
    }
    let waited = now - essai.since;
    if waited > 90.0 {
        essai.stage = 0;
        net.notify("Essai abandonne (trop long). Regardez les messages ci-dessus.", now);
        return;
    }
    match essai.stage {
        1 => {
            // Lance la recherche, puis attend l'arrivée (astre ciblé, plus de recherche en cours)
            if !essai.sent {
                essai.sent = true;
                chat.send(crate::chat_cmd::ChatCommand("/aller planete mer".to_string()));
            } else if waited > 1.0 && state.search.is_none() && state.pending.is_none() && matches!(target.0, TargetKind::Planet(_)) {
                essai.stage = 2;
                essai.since = now;
            }
        }
        2 => {
            if waited > 0.5 {
                chat.send(crate::chat_cmd::ChatCommand("/jour".to_string()));
                chat.send(crate::chat_cmd::ChatCommand("/vol".to_string()));
                essai.stage = 3;
                essai.since = now;
            }
        }
        3 => {
            if surface.active() && waited > 3.0 {
                chat.send(crate::chat_cmd::ChatCommand("/mer".to_string()));
                essai.stage = 0;
            }
        }
        _ => essai.stage = 0,
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
            state.last = Some((family, kind.clone(), (after + SEARCH_LIMIT) % settings.systems.dense().len().max(1)));
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
            assert!(known_type(Family::Star, t), "etoile {t}");
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

/// Mesure (0.13, règle 18) : avec `SPACESPORE_PERF=<fichier>`, les temps d'image entre
/// `SPACESPORE_PERF_FROM` et `SPACESPORE_PERF_TO` secondes (14 et 30 par défaut) : images/s
/// médianes, 1 % bas, images de plus de 33 ms, pire image. Écrit dans le fichier à la fin.
fn perf_log(time: Res<Time>, tiles: Res<crate::surface::TileStats>, mut frames: Local<Vec<f32>>, mut detail: Local<Vec<f32>>, mut done: Local<bool>) {
    let Ok(path) = std::env::var("SPACESPORE_PERF") else { return };
    if *done {
        return;
    }
    let num = |k: &str, d: f32| std::env::var(k).ok().and_then(|s| s.parse().ok()).unwrap_or(d);
    let (from, to) = (num("SPACESPORE_PERF_FROM", 14.0), num("SPACESPORE_PERF_TO", 30.0));
    let t = time.elapsed_secs();
    if t >= from && t <= to {
        frames.push(time.delta_secs());
        if tiles.leaves > 0 {
            detail.push(tiles.ready as f32 / tiles.leaves as f32);
        }
    }
    if t > to && !frames.is_empty() {
        *done = true;
        let mut f = frames.clone();
        f.sort_by(|a, b| a.total_cmp(b));
        let median = f[f.len() / 2];
        let worst: Vec<f32> = f.iter().rev().take((f.len() / 100).max(1)).copied().collect();
        let low = worst.iter().sum::<f32>() / worst.len() as f32;
        let spikes = f.iter().filter(|x| **x > 0.033).count();
        let text = format!(
            "images {} | median {:.1} images/s | 1 % bas {:.1} images/s | > 33 ms : {} | pire {:.1} ms | echelle x{} | tuiles a leur finesse : moyenne {:.0} %, pire {:.0} %\n",
            f.len(),
            1.0 / median.max(1e-6),
            1.0 / low.max(1e-6),
            spikes,
            f.last().copied().unwrap_or(0.0) * 1000.0,
            crate::terrain::voxel_scale(),
            100.0 * detail.iter().sum::<f32>() / detail.len().max(1) as f32,
            100.0 * detail.iter().copied().fold(1.0f32, f32::min)
        );
        info!("PERF {text}");
        let _ = std::fs::write(path, text);
    }
}
