//! Tunnels du sub-espace (0.13 V4) : on les creuse avec une foreuse, on y vole dedans.
//!
//! - Une foreuse = un bien de l'économie (`economy::FIRST_TUNNEL_GOOD`) : I (100 u), II (500 u),
//!   III (5 000 u) et la clandestine (250 u, pour les routes clandestines). On la garde (outil).
//! - Énergie : 1 cellule de carburant (`economy::FUEL`) par unité de distance (1 u = `UNIT` unités du monde).
//! - Un tunnel relie deux points absolus (f64) : le vaisseau là où l'on creuse, et l'astre ciblé.
//!   Il a 1, 2 ou 3 voies (vitesses `LANE_SPEED`) ; la 3e est à péage (`TOLL_PER_U`). Clandestin : une
//!   seule voie, ouvertures cachées (visibles seulement de très près).
//! - Dedans : un cylindre qu'on ne voit pas de l'extérieur (`cinematic::CineKind::Ride`) ; Z / S =
//!   avancer / reculer, Q / D = changer de voie, Échap = sortir. Pas de gravité ni de combat dedans.
//! - Sauvé dans `tunnels.json`. Pas encore partagé en multijoueur (chaque joueur a les siens).
//!
//! Chat : `/tunnel creuser <1|2|3|c> [voies]`, `/tunnel liste`, `/tunnel entrer [n]`, `/tunnel sortir`.

use bevy::math::DVec3;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::cinematic::{CineKind, Cinematic, DIG_SECS};
use crate::economy::{Economy, FIRST_TUNNEL_GOOD, FUEL, GOODS};
use crate::net::Net;
use crate::settings::{data_dir, origin, GameSettings};
use crate::ship::Ship;
use crate::wormhole::WormholeTravel;
use crate::{CameraController, CameraTarget};

/// Une unité de distance de tunnel (u), en unités du monde.
pub const UNIT: f64 = 1_000_000.0;
/// Portée maximale (u) de chaque foreuse : I, II, III, clandestine.
pub const DRILL_RANGE: [f64; 4] = [100.0, 500.0, 5000.0, 250.0];
/// Vitesse (u par seconde) des voies 1, 2 et 3.
pub const LANE_SPEED: [f64; 3] = [5.0, 15.0, 40.0];
/// Péage de la voie 3 (crédits par u parcourue).
pub const TOLL_PER_U: f64 = 3.0;
/// Distance (unités du monde) à laquelle on peut entrer dans une ouverture.
pub const ENTER_RANGE: f64 = 200_000.0;
/// Les ouvertures clandestines ne se voient que de ce près (unités du monde).
const SECRET_SEEN: f64 = 3_000_000.0;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Tunnel {
    pub a: [f64; 3],
    pub b: [f64; 3],
    pub lanes: u8,
    pub secret: bool,
    pub length_u: f64,
}

impl Tunnel {
    pub fn a(&self) -> DVec3 {
        DVec3::from_array(self.a)
    }

    pub fn b(&self) -> DVec3 {
        DVec3::from_array(self.b)
    }

    pub fn len(&self) -> f64 {
        (self.b() - self.a()).length()
    }

    /// Position (absolue) à la distance `s` (unités du monde) depuis l'ouverture A.
    pub fn at(&self, s: f64) -> DVec3 {
        let len = self.len().max(1.0);
        self.a() + (self.b() - self.a()) * (s / len)
    }
}

#[derive(Resource, Default, Serialize, Deserialize)]
pub struct Tunnels {
    pub list: Vec<Tunnel>,
    /// Les 4 foreuses et 300 cellules d'energie ont ete donnees (une seule fois, pour essayer sans marchand).
    #[serde(default)]
    kit: bool,
    #[serde(skip)]
    dig: Option<Tunnel>,
    #[serde(skip)]
    dig_started: bool,
    #[serde(skip)]
    ride: Option<Ride>,
}

/// Un trajet dans un tunnel.
struct Ride {
    idx: usize,
    /// Distance depuis l'ouverture d'entrée (unités du monde).
    p: f64,
    /// Entré par B : on va de B vers A.
    from_b: bool,
    lane: usize,
    /// Vitesse relative, de -1 (marche arrière) à 1.
    vel: f64,
    toll_due: f64,
    paid: f64,
}

impl Tunnels {
    fn path() -> std::path::PathBuf {
        data_dir().join("tunnels.json")
    }

    fn load() -> Self {
        std::fs::read_to_string(Self::path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }

    fn save(&self) {
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::create_dir_all(data_dir());
            let _ = std::fs::write(Self::path(), json);
        }
    }

    /// Dans un tunnel (ou en train d'en creuser un) : le vaisseau n'est plus piloté par les commandes.
    pub fn busy(&self) -> bool {
        self.dig.is_some() || self.ride.is_some()
    }
}

#[derive(Event)]
pub struct TunnelCommand(pub String);

pub struct TunnelPlugin;

impl Plugin for TunnelPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Tunnels::load())
            .add_event::<TunnelCommand>()
            .add_systems(Startup, give_kit)
            .add_systems(Update, (run_tunnel_commands, run_dig, run_ride, sync_controls, draw_mouths).chain());
    }
}

/// Au premier lancement avec les tunnels : une de chaque foreuse et 300 cellules d'energie.
fn give_kit(mut tunnels: ResMut<Tunnels>, mut eco: ResMut<Economy>) {
    if tunnels.kit {
        return;
    }
    tunnels.kit = true;
    for t in 0..4 {
        if eco.count(good_of(t)) == 0 {
            eco.give(good_of(t), 1);
        }
    }
    eco.give(FUEL, 300);
    tunnels.save();
}

/// Foreuse (indice 0..3) pour le mot donné.
fn tier_of(word: &str) -> Option<usize> {
    match word {
        "1" | "i" => Some(0),
        "2" | "ii" => Some(1),
        "3" | "iii" => Some(2),
        "c" | "clandestin" | "clandestine" | "s" | "speciale" => Some(3),
        _ => None,
    }
}

fn good_of(tier: usize) -> usize {
    FIRST_TUNNEL_GOOD + tier
}

/// Lecture d'une position de monde (f32) en absolu (f64).
fn abs_of(local: Vec3) -> DVec3 {
    local.as_dvec3() + origin()
}

fn local_of(abs: DVec3) -> Vec3 {
    (abs - origin()).as_vec3()
}

/// Coût en énergie (cellules de carburant) d'un tunnel de `length_u` : 1 par unité de distance.
pub fn energy_cost(length_u: f64) -> u32 {
    (length_u - 1e-3).ceil().max(1.0) as u32
}

/// Prix du péage (crédits) pour `s` unités du monde parcourues sur la voie 3.
pub fn toll_for(s: f64) -> f64 {
    s / UNIT * TOLL_PER_U
}

#[allow(clippy::too_many_arguments)]
fn run_tunnel_commands(
    time: Res<Time>,
    mut events: EventReader<TunnelCommand>,
    mut tunnels: ResMut<Tunnels>,
    mut eco: ResMut<Economy>,
    mut net: ResMut<Net>,
    mut cine: ResMut<Cinematic>,
    target: Res<CameraTarget>,
    settings: Res<GameSettings>,
    ship_q: Query<&Transform, With<Ship>>,
    cam_q: Query<&CameraController>,
    sky: Res<crate::skybox::SkyState>,
) {
    let now = time.elapsed_secs_f64();
    for TunnelCommand(arg) in events.read() {
        let words: Vec<String> = arg.split_whitespace().map(|w| w.to_lowercase()).collect();
        let sub = words.first().map(String::as_str).unwrap_or("");
        let Ok(ship) = ship_q.get_single() else { continue };
        match sub {
            "creuser" | "dig" => {
                if tunnels.busy() || cine.active() {
                    net.notify("Une sequence est deja en cours.", now);
                    continue;
                }
                let Some(tier) = words.get(1).and_then(|w| tier_of(w)) else {
                    net.notify("/tunnel creuser <1 | 2 | 3 | c> [voies 1-3] : foreuse I (100 u), II (500 u), III (5000 u), c = clandestine (250 u).", now);
                    continue;
                };
                if eco.count(good_of(tier)) == 0 {
                    net.notify(&format!("Il vous faut : {} (rayon Modules de vaisseau chez un marchand).", GOODS[good_of(tier)].0), now);
                    continue;
                }
                let secret = tier == 3;
                let lanes = if secret { 1 } else { words.get(2).and_then(|w| w.parse::<u8>().ok()).unwrap_or(2).clamp(1, 3) };
                let Ok(ctrl) = cam_q.get_single() else { continue };
                if ctrl.last_target_pos == Vec3::ZERO && !matches!(target.0, crate::TargetKind::GalacticCore) {
                    net.notify("Ciblez d'abord la destination du tunnel (etoile, planete...).", now);
                    continue;
                }
                // Sortie : au point de stationnement de la cible
                let hover = crate::hover_height(&target, &settings);
                let a = abs_of(ship.translation);
                let b = abs_of(ctrl.last_target_pos + Vec3::Y * hover);
                // Test : `SPACESPORE_TEST_TUNNEL=<u>` creuse droit devant, sans cible
                let b = match std::env::var("SPACESPORE_TEST_TUNNEL").ok().and_then(|v| v.parse::<f64>().ok()) {
                    Some(u) => a + (ship.rotation * Vec3::NEG_Z).as_dvec3() * u * UNIT,
                    None => b,
                };
                let length_u = (b - a).length() / UNIT;
                if length_u < 0.5 {
                    net.notify("La destination est trop proche pour un tunnel (0,5 u au moins).", now);
                    continue;
                }
                if length_u > DRILL_RANGE[tier] {
                    net.notify(&format!("Tunnel de {length_u:.0} u : trop long pour cette foreuse (max {:.0} u).", DRILL_RANGE[tier]), now);
                    continue;
                }
                let cells = energy_cost(length_u);
                if eco.count(FUEL) < cells {
                    net.notify(&format!("Energie insuffisante : {cells} cellules de carburant pour {length_u:.0} u (vous en avez {}).", eco.count(FUEL)), now);
                    continue;
                }
                eco.take(FUEL, cells);
                let tunnel = Tunnel { a: a.to_array(), b: b.to_array(), lanes, secret, length_u };
                tunnels.dig = Some(tunnel);
                tunnels.dig_started = false;
                cine.dig_len = length_u as f32;
                cine.start(CineKind::Dig, DIG_SECS);
                // Le fond = le vrai ciel d'ici, la foreuse creuse vers la destination
                cine.use_sky(&sky, &settings, (b - a).as_vec3());
                tunnels.dig_started = true;
                let kind = if secret { "clandestin" } else { "normal" };
                net.notify(&format!("Creusement : {length_u:.0} u, {lanes} voie(s), tunnel {kind}, {cells} cellules d'energie."), now);
            }
            "liste" | "list" => {
                if tunnels.list.is_empty() {
                    net.notify("Aucun tunnel. /tunnel creuser <1|2|3|c> [voies] (il faut une foreuse et du carburant).", now);
                    continue;
                }
                let here = abs_of(ship.translation);
                let lines: Vec<String> = tunnels
                    .list
                    .iter()
                    .enumerate()
                    .map(|(i, t)| {
                        let near = (here - t.a()).length().min((here - t.b()).length()) / UNIT;
                        format!("{}: {:.0} u, {} voie(s){}, ouverture a {:.0} u", i + 1, t.length_u, t.lanes, if t.secret { ", clandestin" } else { "" }, near)
                    })
                    .collect();
                net.notify(&lines.join(" | "), now);
            }
            "entrer" | "enter" => {
                if tunnels.busy() || cine.active() {
                    continue;
                }
                let here = abs_of(ship.translation);
                let wanted = words.get(1).and_then(|w| w.parse::<usize>().ok()).map(|n| n.saturating_sub(1));
                let best = tunnels
                    .list
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| wanted.is_none_or(|w| w == *i))
                    .flat_map(|(i, t)| [(i, false, (here - t.a()).length()), (i, true, (here - t.b()).length())])
                    .filter(|(_, _, d)| *d <= ENTER_RANGE)
                    .min_by(|x, y| x.2.total_cmp(&y.2));
                let Some((idx, from_b, _)) = best else {
                    net.notify("Aucune ouverture de tunnel a portee : approchez-vous d'une ouverture (cercle bleu).", now);
                    continue;
                };
                let lane = (tunnels.list[idx].lanes as usize - 1).min(1);
                tunnels.ride = Some(Ride { idx, p: 0.0, from_b, lane, vel: 0.0, toll_due: 0.0, paid: 0.0 });
                cine.start(CineKind::Ride, f32::MAX);
                net.notify("Vous entrez dans le tunnel : Z / S avancer, reculer ; Q / D changer de voie ; Echap sortir.", now);
            }
            "sortir" | "exit" => {
                cine.leave = true;
            }
            _ => {
                let owned: Vec<String> = (0..4).filter(|&t| eco.count(good_of(t)) > 0).map(|t| format!("{} ({:.0} u)", GOODS[good_of(t)].0, DRILL_RANGE[t])).collect();
                let text = format!(
                    "/tunnel creuser <1|2|3|c> [voies] | liste | entrer [n] | sortir. 1 cellule de carburant = 1 u. Foreuses : {}.",
                    if owned.is_empty() { "aucune (a acheter chez un marchand)".to_string() } else { owned.join(", ") }
                );
                net.notify(&text, now);
            }
        }
    }
}

/// Fin du creusement : le tunnel existe.
fn run_dig(time: Res<Time>, mut tunnels: ResMut<Tunnels>, cine: Res<Cinematic>, mut net: ResMut<Net>) {
    if !tunnels.dig_started || cine.active() {
        return;
    }
    let Some(tunnel) = tunnels.dig.take() else { return };
    tunnels.dig_started = false;
    let now = time.elapsed_secs_f64();
    let text = format!("Tunnel creuse : {:.0} u, {} voie(s). Entrez par ses ouvertures avec /tunnel entrer.", tunnel.length_u, tunnel.lanes);
    tunnels.list.push(tunnel);
    tunnels.save();
    net.notify(&text, now);
}

/// Vol dans un tunnel : avancer / reculer, changer de voie, péage, sortie aux ouvertures.
#[allow(clippy::too_many_arguments)]
fn run_ride(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut tunnels: ResMut<Tunnels>,
    mut cine: ResMut<Cinematic>,
    mut eco: ResMut<Economy>,
    mut net: ResMut<Net>,
    mut ship_q: Query<(&mut Transform, &mut Visibility), With<Ship>>,
    panel: Res<crate::net_ui::NetPanel>,
) {
    let tunnels = &mut *tunnels;
    let Some(ride) = tunnels.ride.as_mut() else { return };
    let now = time.elapsed_secs_f64();
    let dt = time.delta_secs().min(0.1) as f64;
    let Some(tunnel) = tunnels.list.get(ride.idx).cloned() else {
        tunnels.ride = None;
        cine.stop();
        return;
    };
    let typing = panel.focus.is_some();
    // Commandes
    let mut want = 0.0;
    if !typing {
        if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
            want += 1.0;
        }
        if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
            want -= 1.0;
        }
        let lanes = tunnel.lanes as usize;
        if (keys.just_pressed(KeyCode::KeyA) || keys.just_pressed(KeyCode::ArrowLeft)) && ride.lane > 0 {
            ride.lane -= 1;
        }
        if (keys.just_pressed(KeyCode::KeyD) || keys.just_pressed(KeyCode::ArrowRight)) && ride.lane + 1 < lanes {
            ride.lane += 1;
        }
    }
    // Voie à péage : si on ne peut plus payer, on revient sur la voie 2 (ou 1)
    let toll_lane = tunnel.lanes == 3 && ride.lane == 2;
    if toll_lane && eco.credits < 1 {
        ride.lane = 1;
        net.notify("Plus de credits pour le peage : retour sur la voie 2.", now);
    }
    let speed = LANE_SPEED[ride.lane.min(2)] * UNIT;
    // La vitesse suit la commande en douceur (marche arrière à mi-vitesse)
    let target = if want > 0.0 { 1.0 } else if want < 0.0 { -0.5 } else { 0.0 };
    ride.vel += (target - ride.vel) * (1.0 - (-2.5 * dt).exp());
    let step = ride.vel * speed * dt;
    ride.p += step;
    // Péage : la voie 3 se paie à la distance parcourue (dans les deux sens)
    if tunnel.lanes == 3 && ride.lane == 2 && !crate::settings::creative() {
        ride.toll_due += toll_for(step.abs());
        let whole = ride.toll_due.floor();
        if whole >= 1.0 {
            ride.toll_due -= whole;
            let pay = (whole as i64).min(eco.credits);
            eco.credits -= pay;
            ride.paid += pay as f64;
            cine.ride.flash = 1.0;
        }
    }
    cine.ride.flash = (cine.ride.flash - dt as f32 * 3.0).max(0.0);

    let len = tunnel.len();
    let leaving = cine.leave || ride.p >= len || ride.p <= -1.0;
    let sign = if ride.from_b { -1.0 } else { 1.0 };
    let s_from_a = if ride.from_b { len - ride.p } else { ride.p };
    if let Ok((mut tf, mut vis)) = ship_q.get_single_mut() {
        let pos = local_of(tunnel.at(s_from_a.clamp(0.0, len)));
        tf.translation = pos;
        let axis = ((tunnel.b() - tunnel.a()).normalize_or_zero() * sign).as_vec3();
        if axis != Vec3::ZERO {
            tf.rotation = Transform::IDENTITY.looking_to(axis, Vec3::Y).rotation;
        }
        *vis = Visibility::Hidden;
    }
    // Ce que montre l'intérieur
    let lanes = tunnel.lanes as f32;
    cine.ride.lane = if lanes > 1.0 { (ride.lane as f32 - (lanes - 1.0) / 2.0) / ((lanes - 1.0) / 2.0) } else { 0.0 };
    cine.ride.speed = (ride.vel.abs() * (LANE_SPEED[ride.lane.min(2)] / LANE_SPEED[2])) as f32 * 1.0 + 0.1 * ride.vel.abs() as f32;
    cine.ride.s = (ride.p / UNIT) as f32 * 6.0;
    cine.ride.dir = if ride.vel < -0.02 { -1.0 } else { 1.0 };
    let left = if ride.vel < -0.02 { ride.p.max(0.0) } else { (len - ride.p).max(0.0) } / UNIT;
    cine.caption = format!(
        "Voie {}/{}{}   {:.0} u/s   reste {:.0} u{}",
        ride.lane + 1,
        tunnel.lanes,
        if toll_lane { " (peage)" } else { "" },
        ride.vel.abs() * LANE_SPEED[ride.lane.min(2)],
        left,
        if ride.paid > 0.0 { format!("   peage paye {:.0} cr", ride.paid) } else { String::new() }
    );

    if leaving {
        let at_b = if ride.from_b { ride.p <= -1.0 || (cine.leave && ride.p < len * 0.5) } else { ride.p >= len || (cine.leave && ride.p > len * 0.5) };
        // Sortie par l'ouverture la plus proche de la position
        let exit_abs = if ride.from_b {
            if at_b { tunnel.a() } else { tunnel.b() }
        } else if at_b {
            tunnel.b()
        } else {
            tunnel.a()
        };
        let exit_abs = if cine.leave { tunnel.at(s_from_a.clamp(0.0, len)) } else { exit_abs };
        let paid = ride.paid;
        tunnels.ride = None;
        cine.stop();
        if let Ok((mut tf, _)) = ship_q.get_single_mut() {
            tf.translation = local_of(exit_abs);
        }
        let text = if paid > 0.0 { format!("Vous sortez du tunnel (peage : {paid:.0} cr).") } else { "Vous sortez du tunnel.".to_string() };
        net.notify(&text, now);
    }
}

/// Les commandes de déplacement sont bloquées pendant le creusement et dans le tunnel.
fn sync_controls(tunnels: Res<Tunnels>, mut travel: ResMut<WormholeTravel>) {
    let busy = tunnels.busy();
    if travel.external != busy {
        travel.external = busy;
    }
}

/// Les ouvertures : deux anneaux bleus (clandestines : cachées, visibles de tout près).
fn draw_mouths(tunnels: Res<Tunnels>, mut gizmos: Gizmos, cam_q: Query<&Transform, With<Camera3d>>) {
    if tunnels.ride.is_some() || tunnels.list.is_empty() {
        return;
    }
    let Ok(cam) = cam_q.get_single() else { return };
    let cam_abs = abs_of(cam.translation);
    for t in &tunnels.list {
        let axis = (t.b() - t.a()).normalize_or_zero().as_vec3();
        if axis == Vec3::ZERO {
            continue;
        }
        for (end, color) in [(t.a(), Color::srgb(0.3, 0.7, 1.0)), (t.b(), Color::srgb(0.5, 0.9, 1.0))] {
            let d = (end - cam_abs).length();
            if t.secret && d > SECRET_SEEN {
                continue;
            }
            // Taille apparente à peu près constante
            let radius = (d * 0.01).max(30_000.0) as f32;
            let iso = Isometry3d::new(local_of(end), Quat::from_rotation_arc(Vec3::Z, axis));
            gizmos.circle(iso, radius, color);
            gizmos.circle(iso, radius * 0.6, color.with_alpha(0.6));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn energy_is_one_cell_per_unit() {
        assert_eq!(energy_cost(100.0), 100);
        assert_eq!(energy_cost(12.2), 13);
        assert_eq!(energy_cost(0.1), 1);
    }

    #[test]
    fn drills_have_their_ranges() {
        assert_eq!(DRILL_RANGE, [100.0, 500.0, 5000.0, 250.0]);
        assert_eq!(tier_of("c"), Some(3));
        assert_eq!(tier_of("3"), Some(2));
        assert_eq!(tier_of("x"), None);
        // La clandestine est plus chere que la II, et toutes sont en vente partout
        assert!(GOODS[good_of(3)].2 > GOODS[good_of(1)].2);
        for t in 0..4 {
            assert!(crate::economy::sold_by(7, good_of(t)));
        }
    }

    #[test]
    fn tunnel_geometry_and_toll() {
        let t = Tunnel { a: [0.0, 0.0, 0.0], b: [3.0 * UNIT, 4.0 * UNIT, 0.0], lanes: 3, secret: false, length_u: 5.0 };
        assert!((t.len() - 5.0 * UNIT).abs() < 1.0);
        let mid = t.at(2.5 * UNIT);
        assert!((mid - DVec3::new(1.5 * UNIT, 2.0 * UNIT, 0.0)).length() < 1.0);
        assert!((toll_for(10.0 * UNIT) - 30.0).abs() < 1e-9);
        let back: Tunnel = serde_json::from_str(&serde_json::to_string(&t).unwrap()).unwrap();
        assert_eq!(back, t);
    }
}
