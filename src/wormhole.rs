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
//  sur place avec son vaisseau, puis appuyer sur T. Tant qu'il n'a pas été
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
const LINE_RANGE: f32 = 30_000_000.0;
/// Distance de la caméra après l'arrivée (zoom « Système »).
const ARRIVAL_DISTANCE: f32 = 30_000.0;

pub struct WormholePlugin;

impl Plugin for WormholePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Wormholes>()
            .add_systems(Startup, build_wormholes)
            .add_systems(Update, (wormhole_travel, draw_wormholes));
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

#[allow(clippy::too_many_arguments)]
fn wormhole_travel(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    panel: Res<NetPanel>,
    menu: Res<MenuState>,
    wormholes: Res<Wormholes>,
    star_q: Query<&StarId, With<StarRoot>>,
    mut ship_q: Query<&mut Transform, With<Ship>>,
    mut cam_q: Query<&mut CameraController>,
    mut target: ResMut<CameraTarget>,
    mut zoom: ResMut<ZoomLevel>,
    mut settings: ResMut<GameSettings>,
    mut net: ResMut<Net>,
) {
    if !keys.just_pressed(KeyCode::KeyT) || panel.focus.is_some() || menu.open {
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
    let Ok(mut ship) = ship_q.get_single_mut() else { return };
    if ship.translation.distance(here.center()) > wormhole.reach_at(sys) {
        net.notify("Approchez-vous de l'etoile pour emprunter son trou de ver.", now);
        return;
    }

    let name = dest.name.clone();
    // Le vaisseau réapparaît près de l'étoile d'arrivée, et la caméra le suit
    ship.translation = dest.center() + Vec3::Y * 80.0;
    target.0 = TargetKind::Star(to);
    if let Ok(mut ctrl) = cam_q.get_single_mut() {
        ctrl.distance = ARRIVAL_DISTANCE;
    }
    // Au zoom « Planète », une cible hors du système courant serait annulée
    *zoom = ZoomLevel::System;
    // Un siège en cours est rompu par le départ (le vaisseau quitte le territoire)
    net.local.siege = None;

    if !settings.known_wormholes.contains(&wormhole.id()) {
        settings.known_wormholes.push(wormhole.id());
        settings.save();
        net.notify(&format!("Trou de ver decouvert : il relie les deux etoiles. Vous arrivez pres de {name}."), now);
    } else {
        net.notify(&format!("Trou de ver traverse : vous arrivez pres de {name}."), now);
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

    #[test]
    fn another_seed_gives_another_map() {
        let mut settings = GameSettings::default();
        let a = generate(&settings);
        settings.world_seed = 7;
        assert_ne!(a, generate(&settings));
    }
}
