// ─────────────────────────────────────────────────────────────────────────
//  Trous de ver
//
//  Très rares : une dizaine dans la galaxie principale, de 2 à 7 dans les
//  galaxies extérieures selon leur taille. Un trou de ver relie deux étoiles
//  de la même galaxie (jamais deux galaxies différentes), toujours éloignées
//  l'une de l'autre, et se prend dans les deux sens. Chaque étoile porte une
//  ouverture, posée à l'extérieur de ses orbites.
//
//  Le hasard vient de la graine du monde : les paires sont toujours les mêmes,
//  pour tous les joueurs qui ont le même monde.
//
//  Pour l'emprunter : cibler l'étoile (ou une planète de son système), être
//  sur place avec son vaisseau, puis appuyer sur T : le vaisseau s'élance en six temps (préparation,
//  accélération, bond en avant, vitesse lumière, décélération, sortie). Tant qu'il n'a pas été
//  emprunté, on ne sait pas où il mène ; ensuite les deux ouvertures sont
//  reliées par un trait, et c'est retenu sur le PC du joueur.
// ─────────────────────────────────────────────────────────────────────────

use bevy::prelude::*;

use crate::net::Net;
use crate::net_ui::NetPanel;
use crate::planet::{StarId, StarRoot};
use crate::settings::{GameSettings, StarSystemConfig, CORE_EXCLUSION, GALAXY_RADIUS};
use crate::ship::Ship;
use crate::ui::{CameraTarget, MenuState, TargetKind};
use crate::{target_system, CameraController, ZoomLevel};

/// Trous de ver dans la galaxie principale.
const MAIN_GALAXY_WORMHOLES: usize = 10;
/// Marge autour des orbites où l'on peut encore emprunter le trou de ver.
const USE_MARGIN: f32 = 30_000.0;
/// Rayon minimal du dessin (il grandit avec la distance pour rester visible).
const MIN_DRAW_RADIUS: f32 = 1_500.0;
/// Distance de la caméra au-delà de laquelle une ouverture n'est plus dessinée.
const DRAW_RANGE: f32 = 3_000_000.0;
/// Le trait entre deux ouvertures connues est dessiné de plus loin.
const LINE_RANGE: f32 = 15_000_000.0;
/// Distance de la caméra après l'arrivée (zoom « Système »).
const ARRIVAL_DISTANCE: f32 = 30_000.0;

pub struct WormholePlugin;

impl Plugin for WormholePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Wormholes>()
            .init_resource::<WormholeTravel>()
            .add_systems(Startup, (build_wormholes, setup_trip_ui))
            .add_systems(Update, (wormhole_travel, run_wormhole_trip, draw_wormholes, draw_trip_fx, update_trip_ui).chain());
    }
}

/// Un trou de ver : deux ouvertures, dans deux systèmes de la même galaxie.
#[derive(Clone, Debug, PartialEq)]
pub struct Wormhole {
    pub a: usize,
    pub b: usize,
    pub mouth_a: Vec3,
    pub mouth_b: Vec3,
    /// Distance maximale au centre du système pour pouvoir l'emprunter, par ouverture.
    pub reach_a: f32,
    pub reach_b: f32,
}

impl Wormhole {
    /// Identifiant du trou de ver (retenu dans les réglages une fois découvert).
    pub fn id(&self) -> u32 {
        self.a.min(self.b) as u32
    }

    /// Système à l'autre bout, si `sys` est l'une des deux ouvertures.
    pub fn other_end(&self, sys: usize) -> Option<usize> {
        if sys == self.a {
            Some(self.b)
        } else if sys == self.b {
            Some(self.a)
        } else {
            None
        }
    }

    pub fn reach_at(&self, sys: usize) -> f32 {
        if sys == self.a { self.reach_a } else { self.reach_b }
    }
}

#[derive(Resource, Default)]
pub struct Wormholes {
    pub list: Vec<Wormhole>,
}

impl Wormholes {
    /// Le trou de ver qui s'ouvre près de ce système.
    pub fn at(&self, sys: usize) -> Option<&Wormhole> {
        self.list.iter().find(|w| w.other_end(sys).is_some())
    }

    /// Ligne d'information quand on cible l'étoile d'un trou de ver.
    pub fn hud_line(&self, sys: usize, settings: &GameSettings) -> Option<String> {
        let w = self.at(sys)?;
        let known = settings.known_wormholes.contains(&w.id());
        Some(match w.other_end(sys).and_then(|o| settings.systems.get(o)) {
            Some(dest) if known => format!("Trou de ver ici <-> {}  (T : l'emprunter)", dest.name),
            _ => "Trou de ver ici, destination inconnue  (T : l'emprunter)".to_string(),
        })
    }
}

fn mix(a: u32, b: u32, c: u32) -> u32 {
    let mut x = a.wrapping_mul(0x9E37_79B1) ^ b.wrapping_mul(0x85EB_CA6B) ^ c.wrapping_mul(0xC2B2_AE35);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    x = x.wrapping_mul(0x297A_2D39);
    x ^= x >> 15;
    x
}

/// Distance du centre du système à sa dernière orbite.
fn system_extent(sys: &StarSystemConfig) -> f32 {
    let planets = sys.planets.iter().map(|p| p.orbit_distance);
    let stars = sys.stars.iter().map(|s| s.orbit_distance);
    planets.chain(stars).fold(0.0_f32, f32::max)
}

/// Nombre de trous de ver d'une galaxie : 10 pour la principale, proportionnel à
/// la taille pour les autres.
fn count_for(galaxy_id: usize, radius: f32) -> usize {
    if galaxy_id == 0 {
        MAIN_GALAXY_WORMHOLES
    } else {
        ((MAIN_GALAXY_WORMHOLES as f32 * radius / GALAXY_RADIUS).round() as usize).clamp(2, 7)
    }
}

/// Ouverture d'un trou de ver près d'un système : (position, portée d'emprunt).
fn mouth(seed: u32, sys_idx: usize, sys: &StarSystemConfig) -> (Vec3, f32) {
    let angle = mix(seed ^ 0xA41, sys_idx as u32, 7) as f32 / u32::MAX as f32 * std::f32::consts::TAU;
    let offset = system_extent(sys) * 1.25 + 4_000.0;
    (sys.center() + Vec3::new(angle.cos(), 0.0, angle.sin()) * offset, offset + USE_MARGIN)
}

/// Tous les trous de ver du monde, déterministes d'après sa graine.
pub fn generate(settings: &GameSettings) -> Vec<Wormhole> {
    let seed = (settings.world_seed as u32) ^ ((settings.world_seed >> 32) as u32) ^ 0x57_4F_52_4D;
    let mut out = Vec::new();
    for (gid, gal) in settings.galaxies.iter().enumerate() {
        // Systèmes de cette galaxie, hors du voisinage immédiat du trou noir central
        let candidates: Vec<usize> = settings
            .systems
            .iter()
            .enumerate()
            .filter(|(_, s)| s.galaxy_id as usize == gid && s.center().distance(gal.center) >= CORE_EXCLUSION)
            .map(|(i, _)| i)
            .collect();
        let pairs = count_for(gid, gal.radius).min(candidates.len() / 2);
        // Les deux bouts d'un trou de ver ne doivent pas être voisins : sinon il ne sert à rien
        let min_dist = gal.radius * 0.15;
        let mut used: Vec<usize> = Vec::new();
        let mut attempt = 0u32;
        for _ in 0..pairs {
            // Premier bout : un système libre tiré au hasard
            let mut a = None;
            while a.is_none() && attempt < (pairs * 60) as u32 {
                let sys = candidates[mix(seed, gid as u32, attempt) as usize % candidates.len()];
                attempt += 1;
                if !used.contains(&sys) {
                    a = Some(sys);
                }
            }
            let Some(a) = a else { break };
            used.push(a);
            let ca = settings.systems[a].center();

            // Second bout : tiré au hasard, mais assez loin ; à défaut le plus éloigné essayé
            let mut best: Option<(usize, f32)> = None;
            for _ in 0..80 {
                let sys = candidates[mix(seed ^ 0xB0B, gid as u32, attempt) as usize % candidates.len()];
                attempt += 1;
                if used.contains(&sys) {
                    continue;
                }
                let d = settings.systems[sys].center().distance(ca);
                if best.map_or(true, |(_, bd)| d > bd) {
                    best = Some((sys, d));
                }
                if d >= min_dist {
                    break;
                }
            }
            let Some((b, _)) = best else { break };
            used.push(b);
            let (mouth_a, reach_a) = mouth(seed, a, &settings.systems[a]);
            let (mouth_b, reach_b) = mouth(seed, b, &settings.systems[b]);
            out.push(Wormhole { a, b, mouth_a, mouth_b, reach_a, reach_b });
        }
    }
    out
}

fn build_wormholes(settings: Res<GameSettings>, mut wormholes: ResMut<Wormholes>) {
    wormholes.list = generate(&settings);
}

// ─────────────────────────────────────────────────────────────────────────
//  Voyage
// ─────────────────────────────────────────────────────────────────────────

/// Les six temps du voyage : (phase, durée en secondes, texte affiché).
const PHASES: [(Phase, f32, &str); 6] = [
    (Phase::Prep, 1.0, "Preparation du saut..."),
    (Phase::Accel, 1.5, "Acceleration"),
    (Phase::Leap, 0.6, "Bond en avant !"),
    (Phase::Light, 2.2, "Vitesse lumiere"),
    (Phase::Decel, 1.5, "Deceleration"),
    (Phase::Exit, 0.8, "Sortie du trou de ver"),
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    Prep,
    Accel,
    Leap,
    Light,
    Decel,
    Exit,
}

/// Un voyage en cours.
struct Trip {
    /// Temps écoulé depuis le début (secondes).
    t: f32,
    /// Étoile d'arrivée et identifiant du trou de ver.
    to: usize,
    wormhole_id: u32,
    dest_name: String,
    first_time: bool,
    /// Trajet : départ (vaisseau), ouverture d'entrée, ouverture de sortie, point d'arrivée.
    start: Vec3,
    mouth: Vec3,
    exit_mouth: Vec3,
    dest: Vec3,
    /// Étapes déjà faites (une seule fois chacune).
    arrival_set: bool,
    light_started: bool,
}

/// Voyage en trou de ver en cours. Tant qu'il dure, l'animation pilote le vaisseau
/// (voir `run_wormhole_trip`) et les commandes de déplacement sont bloquées.
#[derive(Resource, Default)]
pub struct WormholeTravel {
    trip: Option<Trip>,
}

impl WormholeTravel {
    pub fn active(&self) -> bool {
        self.trip.is_some()
    }
}

/// Phase à l'instant `t`, avancement (0 à 1) dans la phase, et son texte.
fn phase_at(t: f32) -> Option<(Phase, f32, &'static str)> {
    let mut start = 0.0;
    for (phase, duration, label) in PHASES {
        if t < start + duration {
            return Some((phase, (t - start) / duration, label));
        }
        start += duration;
    }
    None
}

fn smooth(p: f32) -> f32 {
    let p = p.clamp(0.0, 1.0);
    p * p * (3.0 - 2.0 * p)
}

impl Trip {
    /// Position du vaisseau : il s'élance vers l'ouverture en accélérant, fonce dedans,
    /// traverse à la vitesse de la lumière, puis ralentit jusqu'à l'étoile d'arrivée.
    fn ship_pos(&self, phase: Phase, p: f32) -> Vec3 {
        let before_leap = self.start + (self.mouth - self.start) * 0.7;
        match phase {
            Phase::Prep => self.start,
            Phase::Accel => self.start + (self.mouth - self.start) * (0.7 * p * p),
            Phase::Leap => before_leap.lerp(self.mouth, p * p * p),
            Phase::Light => self.mouth.lerp(self.exit_mouth, smooth(p)),
            Phase::Decel => self.exit_mouth.lerp(self.dest, 1.0 - (1.0 - p) * (1.0 - p)),
            Phase::Exit => self.dest,
        }
    }

    /// Direction du déplacement dans cette phase (axe des traits de vitesse).
    fn axis(&self, phase: Phase) -> Vec3 {
        let (a, b) = match phase {
            Phase::Prep | Phase::Accel | Phase::Leap => (self.start, self.mouth),
            Phase::Light => (self.mouth, self.exit_mouth),
            Phase::Decel | Phase::Exit => (self.exit_mouth, self.dest),
        };
        (b - a).normalize_or_zero()
    }

    /// Intensité de l'effet de vitesse (0 à 1).
    fn speed(&self, phase: Phase, p: f32) -> f32 {
        match phase {
            Phase::Prep | Phase::Exit => 0.0,
            Phase::Accel => 0.3 * p,
            Phase::Leap => 0.3 + 0.7 * p,
            Phase::Light => 1.0,
            Phase::Decel => 1.0 - p,
        }
    }

    /// Opacité du voile de couleur plaqué sur l'écran (flash au bond, tunnel, sortie).
    fn veil(&self, phase: Phase, p: f32) -> f32 {
        match phase {
            Phase::Prep => 0.0,
            Phase::Accel => 0.05 * p,
            Phase::Leap => 0.75 * p * p,
            Phase::Light => 0.3 + 0.45 * (1.0 - (p * 5.0).min(1.0)),
            Phase::Decel => 0.3 * (1.0 - p),
            Phase::Exit => 0.4 * (1.0 - p) * (1.0 - p),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn wormhole_travel(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    panel: Res<NetPanel>,
    menu: Res<MenuState>,
    wormholes: Res<Wormholes>,
    star_q: Query<&StarId, With<StarRoot>>,
    ship_q: Query<&Transform, With<Ship>>,
    target: Res<CameraTarget>,
    settings: Res<GameSettings>,
    mut travel: ResMut<WormholeTravel>,
    mut cam_q: Query<&mut CameraController>,
    mut net: ResMut<Net>,
) {
    if !keys.just_pressed(KeyCode::KeyT) || panel.focus.is_some() || menu.open || travel.active() {
        return;
    }
    let now = time.elapsed_secs_f64();
    let Some(Some(sys)) = target_system(&target.0, &star_q) else {
        net.notify("Ciblez une etoile pour chercher un trou de ver.", now);
        return;
    };
    let Some(wormhole) = wormholes.at(sys).cloned() else {
        net.notify("Il n'y a pas de trou de ver pres de cette etoile.", now);
        return;
    };
    let Some(to) = wormhole.other_end(sys) else { return };
    let Some(here) = settings.systems.get(sys) else { return };
    let Some(dest) = settings.systems.get(to) else { return };
    if net.local.hp == 0 {
        net.notify("Votre vaisseau est detruit.", now);
        return;
    }
    let Ok(ship) = ship_q.get_single() else { return };
    if ship.translation.distance(here.center()) > wormhole.reach_at(sys) {
        net.notify("Approchez-vous de l'etoile pour emprunter son trou de ver.", now);
        return;
    }

    let (mouth, exit_mouth) = if sys == wormhole.a { (wormhole.mouth_a, wormhole.mouth_b) } else { (wormhole.mouth_b, wormhole.mouth_a) };
    let hover = crate::hover_height(&CameraTarget(TargetKind::Star(to)), &settings);
    travel.trip = Some(Trip {
        t: 0.0,
        to,
        wormhole_id: wormhole.id(),
        dest_name: dest.name.clone(),
        first_time: !settings.known_wormholes.contains(&wormhole.id()),
        start: ship.translation,
        mouth,
        exit_mouth,
        dest: dest.center() + Vec3::Y * hover,
        arrival_set: false,
        light_started: false,
    });
    // Caméra en retrait : on voit le vaisseau et les effets, et le zoom reste « Système »
    if let Ok(mut ctrl) = cam_q.get_single_mut() {
        ctrl.distance = ctrl.distance.max(15_000.0);
    }
    // Un siège en cours est rompu par le départ (le vaisseau quitte le territoire)
    net.local.siege = None;
}

/// Fait avancer le voyage : place le vaisseau, change la cible à l'arrivée, et termine.
#[allow(clippy::too_many_arguments)]
fn run_wormhole_trip(
    time: Res<Time>,
    mut travel: ResMut<WormholeTravel>,
    mut ship_q: Query<&mut Transform, With<Ship>>,
    mut cam_q: Query<&mut CameraController>,
    mut target: ResMut<CameraTarget>,
    mut zoom: ResMut<ZoomLevel>,
    mut settings: ResMut<GameSettings>,
    mut net: ResMut<Net>,
) {
    let Some(trip) = travel.trip.as_mut() else { return };
    let now = time.elapsed_secs_f64();
    trip.t += time.delta_secs();
    let Ok(mut ship) = ship_q.get_single_mut() else {
        travel.trip = None;
        return;
    };
    let Some((phase, p, _)) = phase_at(trip.t) else {
        // Fin : le vaisseau est posé près de l'étoile d'arrivée
        ship.translation = trip.dest;
        if let Ok(mut ctrl) = cam_q.get_single_mut() {
            ctrl.distance = ARRIVAL_DISTANCE;
        }
        let name = trip.dest_name.clone();
        let first = trip.first_time;
        travel.trip = None;
        let text = if first {
            format!("Trou de ver decouvert : il relie les deux etoiles. Vous etes pres de {name}.")
        } else {
            format!("Trou de ver traverse : vous etes pres de {name}.")
        };
        net.notify(&text, now);
        return;
    };

    // Le vaisseau se déplace et regarde dans le sens du mouvement
    let before = ship.translation;
    ship.translation = trip.ship_pos(phase, p);
    let heading = (ship.translation - before).normalize_or_zero();
    let heading = if heading == Vec3::ZERO { trip.axis(phase) } else { heading };
    if heading != Vec3::ZERO {
        ship.look_to(heading, Vec3::Y);
    }

    // La découverte est retenue dès qu'on entre dans le trou de ver
    if phase == Phase::Light && !trip.light_started {
        trip.light_started = true;
        if trip.first_time {
            settings.known_wormholes.push(trip.wormhole_id);
            settings.save();
        }
    }
    // À la décélération : la cible devient l'étoile d'arrivée (la caméra reste en zoom « Système »)
    if phase == Phase::Decel && !trip.arrival_set {
        trip.arrival_set = true;
        target.0 = TargetKind::Star(trip.to);
        *zoom = ZoomLevel::System;
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  Effets du voyage : traits de vitesse, anneaux, voile plein écran, texte
// ─────────────────────────────────────────────────────────────────────────

#[derive(Component)]
struct TripVeil;

#[derive(Component)]
struct TripText;

fn setup_trip_ui(mut commands: Commands) {
    // Voile plein écran : ce n'est pas un bouton, il ne bloque aucun clic
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        BackgroundColor(Color::NONE),
        GlobalZIndex(40),
        TripVeil,
    ));
    commands.spawn((
        Text::new(""),
        TextFont { font_size: 30.0, ..default() },
        TextColor(Color::srgba(0.85, 0.95, 1.0, 0.0)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Percent(18.0),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        },
        TextLayout::new_with_justify(JustifyText::Center),
        GlobalZIndex(41),
        TripText,
    ));
}

fn update_trip_ui(
    travel: Res<WormholeTravel>,
    mut veil: Query<&mut BackgroundColor, With<TripVeil>>,
    mut text: Query<(&mut Text, &mut TextColor), With<TripText>>,
) {
    let (alpha, label, text_alpha) = match travel.trip.as_ref().and_then(|trip| phase_at(trip.t).map(|(ph, p, l)| (trip, ph, p, l))) {
        Some((trip, phase, p, label)) => (trip.veil(phase, p), label, 0.95),
        None => (0.0, "", 0.0),
    };
    for mut bg in &mut veil {
        let c = Color::srgba(0.78, 0.68, 1.0, alpha);
        if bg.0 != c {
            bg.0 = c;
        }
    }
    for (mut t, mut color) in &mut text {
        if t.0 != label {
            t.0 = label.to_string();
        }
        color.0 = Color::srgba(0.85, 0.95, 1.0, text_alpha);
    }
}

/// Traits de lumière autour du vaisseau, dans l'axe du déplacement, qui défilent vers l'arrière :
/// courts et lents à l'accélération, longs et rapides à la vitesse lumière, puis ils s'éteignent.
fn draw_trip_fx(
    time: Res<Time>,
    travel: Res<WormholeTravel>,
    ship_q: Query<&Transform, With<Ship>>,
    cam_q: Query<&CameraController>,
    mut gizmos: Gizmos,
) {
    let Some(trip) = travel.trip.as_ref() else { return };
    let Some((phase, p, _)) = phase_at(trip.t) else { return };
    let Ok(ship) = ship_q.get_single() else { return };
    let center = ship.translation;
    let scale = cam_q.get_single().map_or(15_000.0, |c| c.distance.max(1_000.0));
    let t = time.elapsed_secs();
    let axis = trip.axis(phase);
    if axis == Vec3::ZERO {
        return;
    }
    let mut side = axis.cross(Vec3::Y);
    if side.length_squared() < 1.0e-4 {
        side = Vec3::X;
    }
    let side = side.normalize();
    let up = side.cross(axis);
    let ring = |gizmos: &mut Gizmos, at: Vec3, radius: f32, color: Color| {
        const SEGMENTS: usize = 40;
        let point = |a: f32| at + (side * a.cos() + up * a.sin()) * radius;
        for s in 0..SEGMENTS {
            let a0 = s as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
            let a1 = (s + 1) as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
            gizmos.line(point(a0), point(a1), color);
        }
    };

    // Préparation : des anneaux d'énergie se resserrent sur le vaisseau
    if phase == Phase::Prep {
        for k in 0..3 {
            let phase_k = (p * 1.5 + k as f32 / 3.0).fract();
            ring(&mut gizmos, center, scale * 0.25 * (1.0 - phase_k), Color::srgba(0.6, 0.9, 1.0, 0.3 + 0.6 * phase_k));
        }
    }

    // Traits de vitesse
    let speed = trip.speed(phase, p);
    if speed > 0.02 {
        const STREAKS: usize = 90;
        let length = scale * (0.15 + 1.6 * speed);
        let span = scale * 3.0;
        let flow = t * (0.4 + 3.0 * speed);
        for i in 0..STREAKS {
            let h = |k: u32| ((i as u32 * 2654435761u32).wrapping_add(k.wrapping_mul(40503)) % 1000) as f32 / 1000.0;
            let angle = h(1) * std::f32::consts::TAU;
            let radius = scale * (0.08 + 0.7 * h(2));
            // Position le long de l'axe : défile vers l'arrière (les traits passent devant la caméra)
            let z = span * (1.0 - ((flow * (0.6 + 0.8 * h(3)) + h(4)).fract() * 2.0));
            let base = center + (side * angle.cos() + up * angle.sin()) * radius + axis * z;
            let alpha = (0.25 + 0.7 * speed) * (0.5 + 0.5 * h(5));
            gizmos.line(base, base - axis * length, Color::srgba(0.75 + 0.25 * h(6), 0.9, 1.0, alpha));
        }
    }

    // Sortie : des ondes s'élargissent depuis le vaisseau
    if phase == Phase::Exit {
        for k in 0..3 {
            let f = (p + k as f32 * 0.25).min(1.0);
            ring(&mut gizmos, center, scale * (0.05 + 0.9 * f), Color::srgba(0.8, 0.7, 1.0, 0.85 * (1.0 - f)));
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  Dessin
// ─────────────────────────────────────────────────────────────────────────

fn draw_mouth(gizmos: &mut Gizmos, mouth: Vec3, cam_pos: Vec3, t: f32) {
    let dist = cam_pos.distance(mouth);
    // Taille apparente à peu près constante de loin, taille réelle de près
    // Le trou de ver « respire »
    let radius = (dist * 0.012).max(MIN_DRAW_RADIUS) * (1.0 + 0.08 * (t * 3.0).sin());
    // Le dessin fait toujours face à la caméra
    let facing = (cam_pos - mouth).normalize_or_zero();
    let mut right = facing.cross(Vec3::Y);
    if right.length_squared() < 1.0e-6 {
        right = Vec3::X;
    }
    let right = right.normalize();
    let up = right.cross(facing);
    let point = |angle: f32, r: f32| mouth + (right * angle.cos() + up * angle.sin()) * r;

    let violet = Color::srgba(0.75, 0.35, 1.0, 0.9);
    let cyan = Color::srgba(0.3, 0.9, 1.0, 0.8);
    // Anneaux concentriques
    for (scale, color) in [(1.0, violet), (0.68, cyan), (0.36, violet)] {
        const SEGMENTS: usize = 40;
        for s in 0..SEGMENTS {
            let a0 = s as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
            let a1 = (s + 1) as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
            gizmos.line(point(a0, radius * scale), point(a1, radius * scale), color);
        }
    }
    // Ondes qui s'étendent depuis le centre puis s'éteignent
    for k in 0..3 {
        let phase = (t * 0.6 + k as f32 / 3.0).fract();
        let r = radius * (0.2 + 1.1 * phase);
        let c = Color::srgba(0.75, 0.6, 1.0, 0.8 * (1.0 - phase));
        const RIPPLE: usize = 32;
        for s in 0..RIPPLE {
            let a0 = s as f32 / RIPPLE as f32 * std::f32::consts::TAU;
            let a1 = (s + 1) as f32 / RIPPLE as f32 * std::f32::consts::TAU;
            gizmos.line(point(a0, r), point(a1, r), c);
        }
    }
    // Bras en spirale qui tournent vers le centre
    for arm in 0..3 {
        let base = arm as f32 * std::f32::consts::TAU / 3.0 + t * 1.4;
        let mut prev = point(base, radius);
        for step in 1..=14 {
            let f = step as f32 / 14.0;
            let next = point(base + f * 2.6, radius * (1.0 - 0.9 * f));
            gizmos.line(prev, next, cyan);
            prev = next;
        }
    }
}

fn draw_wormholes(
    time: Res<Time>,
    wormholes: Res<Wormholes>,
    settings: Res<GameSettings>,
    cam_q: Query<&GlobalTransform, With<Camera3d>>,
    mut gizmos: Gizmos,
) {
    let Ok(cam) = cam_q.get_single() else { return };
    let cam_pos = cam.translation();
    let t = time.elapsed_secs();
    for w in &wormholes.list {
        for mouth in [w.mouth_a, w.mouth_b] {
            if cam_pos.distance(mouth) <= DRAW_RANGE {
                draw_mouth(&mut gizmos, mouth, cam_pos, t);
            }
        }
        // Un trou de ver découvert : un seul trait relie ses deux ouvertures
        if settings.known_wormholes.contains(&w.id())
            && cam_pos.distance(w.mouth_a).min(cam_pos.distance(w.mouth_b)) <= LINE_RANGE
        {
            gizmos.line(w.mouth_a, w.mouth_b, Color::srgba(0.75, 0.35, 1.0, 0.85));
            // Points de lumière qui filent dans les deux sens le long du trait
            let size = (cam_pos.distance(w.mouth_a).min(cam_pos.distance(w.mouth_b)) * 0.006).max(MIN_DRAW_RADIUS * 0.6);
            for i in 0..6 {
                let f = (t * 0.18 + i as f32 / 6.0).fract();
                let f = if i % 2 == 0 { f } else { 1.0 - f };
                let p = w.mouth_a.lerp(w.mouth_b, f);
                let c = Color::srgba(0.7, 1.0, 1.0, 0.95);
                gizmos.line(p - Vec3::X * size, p + Vec3::X * size, c);
                gizmos.line(p - Vec3::Y * size, p + Vec3::Y * size, c);
                gizmos.line(p - Vec3::Z * size, p + Vec3::Z * size, c);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wormholes_are_rare_stable_two_way_and_stay_in_their_galaxy() {
        let settings = GameSettings::default();
        let list = generate(&settings);
        // Toujours les mêmes, pour tous les joueurs
        assert_eq!(list, generate(&settings));

        // Une dizaine dans la galaxie principale, quelques-uns dans les autres
        let in_galaxy = |g: u32| list.iter().filter(|w| settings.systems[w.a].galaxy_id == g).count();
        assert_eq!(in_galaxy(0), MAIN_GALAXY_WORMHOLES);
        for gid in 1..settings.galaxies.len() as u32 {
            assert!((2..=7).contains(&in_galaxy(gid)), "galaxie {gid} : {}", in_galaxy(gid));
        }

        for w in &list {
            // Jamais entre deux galaxies, jamais avec soi-même
            assert_ne!(w.a, w.b);
            assert_eq!(settings.systems[w.a].galaxy_id, settings.systems[w.b].galaxy_id);
            // Bouts éloignés l'un de l'autre
            let gal = &settings.galaxies[settings.systems[w.a].galaxy_id as usize];
            let d = settings.systems[w.a].center().distance(settings.systems[w.b].center());
            assert!(d >= gal.radius * 0.1, "{d}");
            // Double sens : chaque ouverture mène à l'autre
            assert_eq!(w.other_end(w.a), Some(w.b));
            assert_eq!(w.other_end(w.b), Some(w.a));
            assert_eq!(w.other_end(usize::MAX), None);
            // Ouvertures hors des orbites, mais à portée d'un vaisseau au centre
            for (sys, mouth, reach) in [(w.a, w.mouth_a, w.reach_a), (w.b, w.mouth_b, w.reach_b)] {
                let c = settings.systems[sys].center();
                assert!(mouth.distance(c) > system_extent(&settings.systems[sys]));
                assert!(reach > mouth.distance(c));
                assert_eq!(w.reach_at(sys), reach);
            }
        }

        // Une étoile n'a qu'une ouverture
        let mut ends: Vec<usize> = list.iter().flat_map(|w| [w.a, w.b]).collect();
        let total = ends.len();
        ends.sort();
        ends.dedup();
        assert_eq!(ends.len(), total);
    }

    #[test]
    fn a_wormhole_is_found_from_both_ends_and_named_once_known() {
        let mut settings = GameSettings::default();
        let wormholes = Wormholes { list: generate(&settings) };
        let w = wormholes.list[0].clone();
        assert_eq!(wormholes.at(w.a).map(|x| x.id()), wormholes.at(w.b).map(|x| x.id()));
        // Destination cachée tant qu'il n'a pas été emprunté, puis révélée des deux côtés
        assert!(wormholes.hud_line(w.a, &settings).unwrap().contains("inconnue"));
        settings.known_wormholes.push(w.id());
        assert!(wormholes.hud_line(w.a, &settings).unwrap().contains(&settings.systems[w.b].name));
        assert!(wormholes.hud_line(w.b, &settings).unwrap().contains(&settings.systems[w.a].name));
    }

    fn trip() -> Trip {
        Trip {
            t: 0.0,
            to: 1,
            wormhole_id: 0,
            dest_name: "X".into(),
            first_time: true,
            start: Vec3::new(1_000.0, 0.0, 0.0),
            mouth: Vec3::new(20_000.0, 0.0, 500.0),
            exit_mouth: Vec3::new(3_000_000.0, 400.0, -2_000_000.0),
            dest: Vec3::new(3_010_000.0, 500.0, -2_000_000.0),
            arrival_set: false,
            light_started: false,
        }
    }

    #[test]
    fn the_trip_has_six_phases_in_order_and_then_ends() {
        let order: Vec<Phase> = PHASES.iter().map(|p| p.0).collect();
        assert_eq!(order, [Phase::Prep, Phase::Accel, Phase::Leap, Phase::Light, Phase::Decel, Phase::Exit]);
        let total: f32 = PHASES.iter().map(|p| p.1).sum();
        // On parcourt bien les phases dans l'ordre en avançant dans le temps
        let mut seen: Vec<Phase> = Vec::new();
        let mut t = 0.0;
        while let Some((phase, p, _)) = phase_at(t) {
            assert!((0.0..=1.0).contains(&p));
            if seen.last() != Some(&phase) {
                seen.push(phase);
            }
            t += 0.01;
        }
        assert_eq!(seen, order);
        assert!(t >= total - 0.02 && t < total + 0.05, "{t} pour {total}");
        assert!(phase_at(total + 0.1).is_none());
    }

    #[test]
    fn the_ship_path_is_continuous_between_phases() {
        let trip = trip();
        let end = |ph| trip.ship_pos(ph, 1.0);
        let begin = |ph| trip.ship_pos(ph, 0.0);
        for (a, b) in [(Phase::Prep, Phase::Accel), (Phase::Accel, Phase::Leap), (Phase::Leap, Phase::Light), (Phase::Light, Phase::Decel), (Phase::Decel, Phase::Exit)] {
            assert!(end(a).distance(begin(b)) < 1.0, "saut entre {a:?} et {b:?}");
        }
        // Part du vaisseau, entre dans l'ouverture, sort à l'autre ouverture, finit à l'étoile
        assert_eq!(begin(Phase::Prep), trip.start);
        assert!(end(Phase::Leap).distance(trip.mouth) < 1.0);
        assert!(end(Phase::Light).distance(trip.exit_mouth) < 1.0);
        assert!(end(Phase::Exit).distance(trip.dest) < 1.0);
    }

    #[test]
    fn the_ship_really_accelerates_then_decelerates() {
        let trip = trip();
        // Accélération : à chaque pas régulier on avance plus que le précédent
        let steps: Vec<f32> = (0..10)
            .map(|i| trip.ship_pos(Phase::Accel, (i + 1) as f32 / 10.0).distance(trip.ship_pos(Phase::Accel, i as f32 / 10.0)))
            .collect();
        assert!(steps.windows(2).all(|w| w[1] > w[0]), "{steps:?}");
        // Décélération : à chaque pas on avance moins que le précédent
        let steps: Vec<f32> = (0..10)
            .map(|i| trip.ship_pos(Phase::Decel, (i + 1) as f32 / 10.0).distance(trip.ship_pos(Phase::Decel, i as f32 / 10.0)))
            .collect();
        assert!(steps.windows(2).all(|w| w[1] < w[0]), "{steps:?}");
        // Le bond est encore plus rapide que l'accélération qui précède
        // (en unités par seconde : les deux phases n'ont pas la même durée)
        let last_accel = trip.ship_pos(Phase::Accel, 1.0).distance(trip.ship_pos(Phase::Accel, 0.9)) / (0.1 * PHASES[1].1);
        let last_leap = trip.ship_pos(Phase::Leap, 1.0).distance(trip.ship_pos(Phase::Leap, 0.9)) / (0.1 * PHASES[2].1);
        assert!(last_leap > last_accel);
    }

    #[test]
    fn speed_lines_and_veil_follow_the_phases() {
        let trip = trip();
        assert_eq!(trip.speed(Phase::Prep, 0.5), 0.0);
        assert_eq!(trip.speed(Phase::Light, 0.5), 1.0);
        assert_eq!(trip.speed(Phase::Exit, 0.5), 0.0);
        assert!(trip.speed(Phase::Accel, 1.0) < trip.speed(Phase::Leap, 1.0));
        assert!(trip.speed(Phase::Decel, 0.0) > trip.speed(Phase::Decel, 1.0));
        for (phase, _, _) in PHASES {
            for i in 0..=10 {
                let v = trip.veil(phase, i as f32 / 10.0);
                assert!((0.0..=1.0).contains(&v), "{phase:?} {v}");
            }
        }
        // Flash blanc à la fin du bond, écran net au départ et à l'arrivée
        assert!(trip.veil(Phase::Leap, 1.0) > 0.7);
        assert_eq!(trip.veil(Phase::Prep, 0.5), 0.0);
        assert!(trip.veil(Phase::Exit, 1.0) < 0.01);
        // Les axes : vers l'ouverture, à travers le trou de ver, puis vers l'étoile
        assert!(trip.axis(Phase::Accel).dot((trip.mouth - trip.start).normalize()) > 0.999);
        assert!(trip.axis(Phase::Light).dot((trip.exit_mouth - trip.mouth).normalize()) > 0.999);
    }

    #[test]
    fn another_seed_gives_another_map() {
        let mut settings = GameSettings::default();
        let a = generate(&settings);
        settings.world_seed = 7;
        assert_ne!(a, generate(&settings));
    }
}
