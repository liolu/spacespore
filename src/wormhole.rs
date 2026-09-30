// ─────────────────────────────────────────────────────────────────────────
//  Trous de ver
//
//  Très rares : une dizaine dans la galaxie principale, de 2 à 7 dans les
//  galaxies extérieures selon leur taille. Chacun est posé près d'une étoile,
//  à l'extérieur de ses orbites, et mène à une étoile tirée au hasard dans
//  tout l'univers (n'importe quelle galaxie). Le hasard vient de la graine du
//  monde : la destination est donc toujours la même, pour tous les joueurs
//  qui ont le même monde. Un trou de ver est à sens unique.
//
//  Pour l'emprunter : cibler l'étoile (ou une planète de son système), être
//  sur place avec son vaisseau, puis appuyer sur T. La destination n'est
//  révélée qu'après un premier passage, et retenue ensuite.
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
/// Distance de la caméra au-delà de laquelle un trou de ver n'est plus dessiné.
const DRAW_RANGE: f32 = 3_000_000.0;
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

#[derive(Clone, Debug, PartialEq)]
pub struct Wormhole {
    /// Système près duquel s'ouvre le trou de ver.
    pub from: usize,
    /// Système où il mène.
    pub to: usize,
    /// Position de l'ouverture.
    pub mouth: Vec3,
    /// Distance maximale au centre du système pour pouvoir l'emprunter.
    pub reach: f32,
}

#[derive(Resource, Default)]
pub struct Wormholes {
    pub list: Vec<Wormhole>,
}

impl Wormholes {
    pub fn at(&self, sys: usize) -> Option<&Wormhole> {
        self.list.iter().find(|w| w.from == sys)
    }

    /// Ligne d'information quand on cible l'étoile d'un trou de ver.
    pub fn hud_line(&self, sys: usize, settings: &GameSettings) -> Option<String> {
        let w = self.at(sys)?;
        let known = settings.known_wormholes.contains(&(sys as u32));
        Some(match settings.systems.get(w.to) {
            Some(dest) if known => {
                let galaxy = if dest.galaxy_id == 0 { String::new() } else { format!(", galaxie {}", dest.galaxy_id) };
                format!("Trou de ver ici -> {}{galaxy}  (T : l'emprunter)", dest.name)
            }
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

/// Tous les trous de ver du monde, déterministes d'après sa graine.
pub fn generate(settings: &GameSettings) -> Vec<Wormhole> {
    let seed = (settings.world_seed as u32) ^ ((settings.world_seed >> 32) as u32) ^ 0x57_4F_52_4D;
    let total = settings.systems.len();
    let mut out = Vec::new();
    if total < 2 {
        return out;
    }
    for (gid, gal) in settings.galaxies.iter().enumerate() {
        // Systèmes de cette galaxie, hors du voisinage immédiat du trou noir central
        let candidates: Vec<usize> = settings
            .systems
            .iter()
            .enumerate()
            .filter(|(_, s)| s.galaxy_id as usize == gid && s.center().distance(gal.center) >= CORE_EXCLUSION)
            .map(|(i, _)| i)
            .collect();
        if candidates.is_empty() {
            continue;
        }
        let wanted = count_for(gid, gal.radius).min(candidates.len());
        let mut chosen: Vec<usize> = Vec::new();
        for attempt in 0..(wanted * 20) as u32 {
            if chosen.len() >= wanted {
                break;
            }
            let from = candidates[mix(seed, gid as u32, attempt) as usize % candidates.len()];
            if !chosen.contains(&from) {
                chosen.push(from);
            }
        }
        for (k, from) in chosen.into_iter().enumerate() {
            // Destination : n'importe quel autre système de l'univers
            let mut to = mix(seed ^ 0xD35_7, from as u32, k as u32) as usize % total;
            if to == from {
                to = (to + 1) % total;
            }
            let sys = &settings.systems[from];
            let angle = mix(seed ^ 0xA41, from as u32, 7) as f32 / u32::MAX as f32 * std::f32::consts::TAU;
            let extent = system_extent(sys);
            let offset = extent * 1.25 + 4_000.0;
            out.push(Wormhole {
                from,
                to,
                mouth: sys.center() + Vec3::new(angle.cos(), 0.0, angle.sin()) * offset,
                reach: offset + USE_MARGIN,
            });
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
    let Some(here) = settings.systems.get(sys) else { return };
    let Some(dest) = settings.systems.get(wormhole.to) else { return };
    if net.local.hp == 0 {
        net.notify("Votre vaisseau est detruit.", now);
        return;
    }
    let Ok(mut ship) = ship_q.get_single_mut() else { return };
    if ship.translation.distance(here.center()) > wormhole.reach {
        net.notify("Approchez-vous de l'etoile pour emprunter son trou de ver.", now);
        return;
    }

    let name = dest.name.clone();
    let galaxy = dest.galaxy_id;
    // Le vaisseau réapparaît près de l'étoile d'arrivée, et la caméra le suit
    ship.translation = dest.center() + Vec3::Y * 80.0;
    target.0 = TargetKind::Star(wormhole.to);
    if let Ok(mut ctrl) = cam_q.get_single_mut() {
        ctrl.distance = ARRIVAL_DISTANCE;
    }
    // Au zoom « Planète », une cible hors du système courant serait annulée
    *zoom = ZoomLevel::System;
    // Un siège en cours est rompu par le départ (le vaisseau quitte le territoire)
    net.local.siege = None;

    let first_time = !settings.known_wormholes.contains(&(sys as u32));
    if first_time {
        settings.known_wormholes.push(sys as u32);
        settings.save();
    }
    let place = if galaxy == 0 { name } else { format!("{name} (galaxie {galaxy})") };
    net.notify(&format!("Trou de ver traverse : vous arrivez pres de {place}."), now);
}

// ─────────────────────────────────────────────────────────────────────────
//  Dessin
// ─────────────────────────────────────────────────────────────────────────

fn draw_wormholes(
    time: Res<Time>,
    wormholes: Res<Wormholes>,
    cam_q: Query<&GlobalTransform, With<Camera3d>>,
    mut gizmos: Gizmos,
) {
    let Ok(cam) = cam_q.get_single() else { return };
    let cam_pos = cam.translation();
    let t = time.elapsed_secs();
    for w in &wormholes.list {
        let dist = cam_pos.distance(w.mouth);
        if dist > DRAW_RANGE {
            continue;
        }
        // Taille apparente à peu près constante de loin, taille réelle de près
        let radius = (dist * 0.012).max(MIN_DRAW_RADIUS);
        // Le dessin fait toujours face à la caméra
        let facing = (cam_pos - w.mouth).normalize_or_zero();
        let mut right = facing.cross(Vec3::Y);
        if right.length_squared() < 1.0e-6 {
            right = Vec3::X;
        }
        let right = right.normalize();
        let up = right.cross(facing);
        let point = |angle: f32, r: f32| w.mouth + (right * angle.cos() + up * angle.sin()) * r;

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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wormholes_are_rare_stable_and_lead_elsewhere() {
        let settings = GameSettings::default();
        let a = generate(&settings);
        let b = generate(&settings);
        // Toujours les mêmes, pour tous les joueurs
        assert_eq!(a, b);
        // Une dizaine dans la galaxie principale, quelques-uns dans les autres
        let in_galaxy = |g: u32| a.iter().filter(|w| settings.systems[w.from].galaxy_id == g).count();
        assert_eq!(in_galaxy(0), MAIN_GALAXY_WORMHOLES);
        for gid in 1..settings.galaxies.len() as u32 {
            assert!((2..=7).contains(&in_galaxy(gid)), "galaxie {gid} : {}", in_galaxy(gid));
        }
        // Un seul trou de ver par étoile, destination toujours ailleurs
        let mut froms: Vec<usize> = a.iter().map(|w| w.from).collect();
        froms.sort();
        froms.dedup();
        assert_eq!(froms.len(), a.len());
        assert!(a.iter().all(|w| w.to != w.from && w.to < settings.systems.len()));
        // Le hasard mène bien dans d'autres galaxies aussi
        let cross = a.iter().filter(|w| settings.systems[w.to].galaxy_id != settings.systems[w.from].galaxy_id).count();
        assert!(cross * 2 > a.len(), "{cross} sur {}", a.len());
        // L'ouverture est hors des orbites, mais à portée d'un vaisseau au centre
        for w in &a {
            let c = settings.systems[w.from].center();
            assert!(w.mouth.distance(c) > system_extent(&settings.systems[w.from]));
            assert!(w.reach > w.mouth.distance(c));
        }
    }

    #[test]
    fn another_seed_gives_another_map() {
        let mut settings = GameSettings::default();
        let a = generate(&settings);
        settings.world_seed = 7;
        assert_ne!(a, generate(&settings));
    }
}
