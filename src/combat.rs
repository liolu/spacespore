// ─────────────────────────────────────────────────────────────────────────
//  Combat entre vaisseaux
//
//  Touche F : tirer sur le vaisseau attaquable le plus proche (neutre ou
//  ennemi, jamais un allié). Chaque tir retire de la coque ; à zéro le
//  vaisseau est détruit puis réapparaît réparé quelques secondes plus tard.
//  La coque se répare seule après un moment sans dégâts. La pression d'une géante gazeuse
//  l'abîme aussi (`gas.rs`) : détruit, le vaisseau réapparaît en orbite, hors de l'atmosphère.
//
//  Pas de serveur : le tireur annonce le total de ses tirs sur chaque joueur
//  (`PlayerStatus::hits`) et c'est la cible qui retire sa propre coque en
//  voyant ce total augmenter. Un paquet perdu ne fait donc perdre aucun tir.
// ─────────────────────────────────────────────────────────────────────────

use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

use crate::claims::{SiegeState, SIEGE_SECS};
use crate::gas::GasState;
use crate::diplomacy::{relation_with, same_guild, their_declared, my_declared, faction_key, Relation};
use crate::net::{display_name, Net, MAX_HP};
use crate::net_ui::NetPanel;
use crate::settings::GameSettings;
use crate::ship::Ship;
use crate::ui::MenuState;

const SHOT_DAMAGE: u8 = 10;
const SHOT_COOLDOWN: f64 = 0.6;
pub const SHOT_RANGE: f32 = 10_000.0;
/// Tirs pris en compte au plus par mise à jour (paquets en retard regroupés).
const MAX_SHOTS_PER_UPDATE: u32 = 5;
const RESPAWN_SECS: f64 = 5.0;
const REGEN_DELAY: f64 = 10.0;
const REGEN_PER_SEC: f32 = 5.0;
const BEAM_SECS: f64 = 0.18;

pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CombatState>()
            .add_systems(Startup, setup_combat_hud)
            .add_systems(
                Update,
                (reset_on_new_session, fire, receive_hits, gas_pressure, asteroid_hits, repair, announce, draw_beams, update_combat_hud)
                    .chain()
                    .after(crate::surface::SurfaceControl),
            )
            // Après le contrôleur de caméra, qui réaffiche le vaisseau à chaque frame
            .add_systems(
                PostUpdate,
                hide_destroyed_ship.before(bevy::render::view::VisibilitySystems::VisibilityPropagate),
            );
    }
}

struct Beam {
    from: u32,
    to: u32,
    until: f64,
}

#[derive(Resource, Default)]
struct CombatState {
    epoch: u64,
    last_shot: f64,
    last_damage: f64,
    dead_until: Option<f64>,
    regen: f32,
    /// Joueurs déjà vus dans cette partie (leurs tirs antérieurs ne comptent pas).
    known: HashSet<u32>,
    /// Dernier total de tirs vu, par (tireur, cible).
    seen: HashMap<(u32, u32), u32>,
    beams: Vec<Beam>,
    /// Dernier joueur sur qui j'ai tiré, et quand.
    last_target: Option<(u32, f64)>,
    peer_hp: HashMap<u32, u8>,
    /// Dégâts de pression pas encore retirés (fraction de PV).
    gas_damage: f32,
    /// Position déclarée par chaque joueur envers moi (pour prévenir des changements).
    stances: HashMap<u32, Relation>,
}

#[derive(Component)]
struct CombatHudText;

fn setup_combat_hud(mut commands: Commands) {
    commands.spawn((
        Text::new(""),
        TextFont { font_size: 16.0, ..default() },
        TextColor(Color::srgb(0.9, 0.9, 0.95)),
        TextLayout::new_with_justify(JustifyText::Right),
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(20.0),
            bottom: Val::Px(20.0),
            ..default()
        },
        CombatHudText,
    ));
}

/// Les identifiants des joueurs changent avec la partie : on repart de zéro.
fn reset_on_new_session(mut state: ResMut<CombatState>, mut net: ResMut<Net>) {
    if state.epoch == net.epoch {
        return;
    }
    state.epoch = net.epoch;
    state.known.clear();
    state.seen.clear();
    state.beams.clear();
    state.last_target = None;
    state.peer_hp.clear();
    state.stances.clear();
    net.local.hits.clear();
}

fn fire(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    panel: Res<NetPanel>,
    menu: Res<MenuState>,
    settings: Res<GameSettings>,
    ship_q: Query<&GlobalTransform, With<Ship>>,
    mut state: ResMut<CombatState>,
    mut net: ResMut<Net>,
) {
    if !keys.pressed(KeyCode::KeyF) || panel.focus.is_some() || menu.open || !net.is_enabled() {
        return;
    }
    let now = time.elapsed_secs_f64();
    let first_press = keys.just_pressed(KeyCode::KeyF);
    if net.local.hp == 0 || now - state.last_shot < SHOT_COOLDOWN {
        return;
    }
    let Ok(ship) = ship_q.get_single() else { return };
    let me = ship.translation();

    // Vaisseau attaquable le plus proche à portée
    let mut best: Option<(u32, f32)> = None;
    let mut ally_in_range: Option<String> = None;
    for (&id, peer) in &net.peers {
        let dist = peer.pos().distance(me);
        if peer.status.hp == 0 || dist > SHOT_RANGE {
            continue;
        }
        if !relation_with(peer, &settings).can_attack() {
            ally_in_range = Some(display_name(&peer.tag, &peer.name));
            continue;
        }
        if best.map_or(true, |(_, d)| dist < d) {
            best = Some((id, dist));
        }
    }
    let Some((target, _)) = best else {
        if first_press {
            let text = match ally_in_range {
                Some(who) => format!("{who} est un allie : impossible de l'attaquer."),
                None => "Aucun vaisseau a portee de tir.".to_string(),
            };
            net.notify(&text, now);
        }
        return;
    };

    state.last_shot = now;
    state.last_target = Some((target, now));
    match net.local.hits.iter_mut().find(|h| h.0 == target) {
        Some(h) => h.1 += 1,
        None => net.local.hits.push((target, 1)),
    }
    let my_id = net.my_id();
    state.beams.push(Beam { from: my_id, to: target, until: now + BEAM_SECS });
}

fn receive_hits(
    time: Res<Time>,
    settings: Res<GameSettings>,
    ship_q: Query<&GlobalTransform, With<Ship>>,
    mut state: ResMut<CombatState>,
    mut net: ResMut<Net>,
) {
    let now = time.elapsed_secs_f64();
    let my_id = net.my_id();
    let state = &mut *state;

    // (tireur, nombre de nouveaux tirs sur moi)
    let mut incoming: Vec<(u32, u32)> = Vec::new();
    for (&shooter, peer) in &net.peers {
        let first_sight = !state.known.contains(&shooter);
        for &(target, count) in &peer.status.hits {
            let prev = state.seen.get(&(shooter, target)).copied().unwrap_or(if first_sight { count } else { 0 });
            state.seen.insert((shooter, target), count);
            let new_shots = count.saturating_sub(prev);
            if new_shots == 0 {
                continue;
            }
            state.beams.push(Beam { from: shooter, to: target, until: now + BEAM_SECS });
            if target == my_id {
                incoming.push((shooter, new_shots));
            }
        }
        state.known.insert(shooter);
    }
    state.known.retain(|id| net.peers.contains_key(id));
    state.seen.retain(|(shooter, _), _| net.peers.contains_key(shooter));

    let me = ship_q.get_single().map(|gt| gt.translation()).ok();
    for (shooter, shots) in incoming {
        if net.local.hp == 0 {
            break;
        }
        let Some(peer) = net.peers.get(&shooter) else { continue };
        // Un allié ne peut pas me toucher, ni un tir venu de trop loin
        if !relation_with(peer, &settings).can_attack() {
            continue;
        }
        if me.is_some_and(|me| peer.pos().distance(me) > SHOT_RANGE * 2.0) {
            continue;
        }
        let who = display_name(&peer.tag, &peer.name);
        let damage = (shots.min(MAX_SHOTS_PER_UPDATE) as u8).saturating_mul(SHOT_DAMAGE);
        net.local.hp = net.local.hp.saturating_sub(damage);
        state.last_damage = now;
        state.regen = 0.0;
        if net.local.hp == 0 {
            state.dead_until = Some(now + RESPAWN_SECS);
            net.notify(&format!("Votre vaisseau a ete detruit par {who} !"), now);
        }
    }
}

/// La pression d'une géante gazeuse abîme la coque ; à 0 PV, le vaisseau est détruit et ramené en
/// orbite au-dessus de l'endroit survolé (il réapparaît hors de l'atmosphère).
fn gas_pressure(
    time: Res<Time>,
    gas: Res<GasState>,
    mut state: ResMut<CombatState>,
    mut net: ResMut<Net>,
    mut surface: ResMut<crate::surface::Surface>,
) {
    let Some(inside) = &gas.inside else {
        state.gas_damage = 0.0;
        return;
    };
    if net.local.hp == 0 {
        return;
    }
    let now = time.elapsed_secs_f64();
    state.gas_damage += inside.damage_per_sec * time.delta_secs();
    state.last_damage = now;
    state.regen = 0.0;
    let lost = state.gas_damage.floor();
    if lost >= 1.0 {
        state.gas_damage -= lost;
        net.local.hp = net.local.hp.saturating_sub(lost.min(255.0) as u8);
    }
    if net.local.hp == 0 {
        state.gas_damage = 0.0;
        state.dead_until = Some(now + RESPAWN_SECS);
        surface.eject();
        net.notify(&format!("Votre vaisseau a ete broye par la pression de {} !", inside.name), now);
    }
}

/// Chocs contre les astéroïdes (C1, Q6) : dégâts selon la vitesse ; à 0 PV, le vaisseau est
/// détruit et ramené en orbite.
fn asteroid_hits(
    time: Res<Time>,
    mut hits: EventReader<crate::asteroids::AsteroidHit>,
    mut state: ResMut<CombatState>,
    mut net: ResMut<Net>,
    mut surface: ResMut<crate::surface::Surface>,
) {
    for hit in hits.read() {
        if net.local.hp == 0 || hit.damage == 0 {
            continue;
        }
        let now = time.elapsed_secs_f64();
        state.last_damage = now;
        state.regen = 0.0;
        net.local.hp = net.local.hp.saturating_sub(hit.damage);
        if net.local.hp == 0 {
            state.dead_until = Some(now + RESPAWN_SECS);
            surface.eject();
            net.notify(&format!("Vaisseau detruit : choc contre un asteroide a {:.0} u/s !", hit.speed), now);
        } else {
            net.notify(&format!("Choc contre un asteroide ({:.0} u/s) : coque -{}.", hit.speed, hit.damage), now);
        }
    }
}

/// Réapparition après destruction, et réparation lente hors combat.
fn repair(time: Res<Time>, mut state: ResMut<CombatState>, mut net: ResMut<Net>) {
    let now = time.elapsed_secs_f64();
    if net.local.hp == 0 {
        match state.dead_until {
            Some(until) if now < until => {}
            _ => {
                state.dead_until = None;
                state.last_damage = now;
                net.local.hp = MAX_HP;
                net.notify("Vaisseau repare.", now);
            }
        }
        return;
    }
    if net.local.hp < MAX_HP && now - state.last_damage > REGEN_DELAY {
        state.regen += REGEN_PER_SEC * time.delta_secs();
        while state.regen >= 1.0 && net.local.hp < MAX_HP {
            state.regen -= 1.0;
            net.local.hp += 1;
        }
    }
}

/// Messages : vaisseaux que j'ai détruits, changements de diplomatie envers moi.
fn announce(
    time: Res<Time>,
    settings: Res<GameSettings>,
    mut state: ResMut<CombatState>,
    mut net: ResMut<Net>,
) {
    let now = time.elapsed_secs_f64();
    let mut messages: Vec<String> = Vec::new();
    for (&id, peer) in &net.peers {
        let who = display_name(&peer.tag, &peer.name);

        let hp = peer.status.hp;
        let before = state.peer_hp.insert(id, hp);
        let shot_by_me = state.last_target.is_some_and(|(t, when)| t == id && now - when < 3.0);
        if hp == 0 && before.is_some_and(|b| b > 0) && shot_by_me {
            messages.push(format!("Vous avez detruit le vaisseau de {who} !"));
        }

        if same_guild(peer, &settings) {
            continue;
        }
        let theirs = their_declared(peer, &settings);
        let previous = state.stances.insert(id, theirs).unwrap_or(Relation::Neutral);
        if theirs == previous {
            continue;
        }
        let mine = my_declared(&settings, &faction_key(peer.gid, &peer.name));
        messages.push(match theirs {
            Relation::Enemy => format!("{who} vous declare la guerre !"),
            Relation::Ally if mine == Relation::Ally => format!("Alliance conclue avec {who}."),
            Relation::Ally => format!("{who} vous propose une alliance : acceptez dans le panneau Guilde (G) ou la liste des joueurs (F2)."),
            Relation::Neutral => format!("{who} redevient neutre envers vous."),
        });
    }
    state.peer_hp.retain(|id, _| net.peers.contains_key(id));
    state.stances.retain(|id, _| net.peers.contains_key(id));
    for m in messages {
        net.notify(&m, now);
    }
}

fn draw_beams(
    time: Res<Time>,
    net: Res<Net>,
    ship_q: Query<&GlobalTransform, With<Ship>>,
    mut state: ResMut<CombatState>,
    mut gizmos: Gizmos,
) {
    let now = time.elapsed_secs_f64();
    state.beams.retain(|b| b.until > now);
    if state.beams.is_empty() {
        return;
    }
    let my_id = net.my_id();
    let me = ship_q.get_single().map(|gt| gt.translation()).ok();
    let position = |id: u32| if id == my_id { me } else { net.peers.get(&id).map(|p| p.pos()) };
    for beam in &state.beams {
        if let (Some(from), Some(to)) = (position(beam.from), position(beam.to)) {
            gizmos.line(from, to, Color::srgb(1.0, 0.35, 0.2));
        }
    }
}

fn update_combat_hud(
    time: Res<Time>,
    net: Res<Net>,
    settings: Res<GameSettings>,
    siege: Res<SiegeState>,
    state: Res<CombatState>,
    gas: Res<GasState>,
    mut hud: Query<(&mut Text, &mut TextColor), With<CombatHudText>>,
) {
    let now = time.elapsed_secs_f64();
    let hp = net.local.hp;
    let mut lines: Vec<String> = Vec::new();
    if let Some(sys) = net.local.siege {
        let name = settings.systems.get(sys as usize).map_or("?", |s| s.name.as_str());
        let percent = (siege.progress / SIEGE_SECS * 100.0).clamp(0.0, 100.0);
        lines.push(if siege.blocked {
            format!("Siege de {name} : {percent:.0} %  -  BLOQUE par un defenseur")
        } else {
            format!("Siege de {name} : {percent:.0} %")
        });
    }
    if hp == 0 {
        let left = state.dead_until.map_or(0.0, |u| (u - now).max(0.0));
        lines.push(format!("VAISSEAU DETRUIT  -  reparation dans {left:.0} s"));
    } else if let Some(inside) = &gas.inside {
        lines.push(format!("Coque {hp}/{MAX_HP}"));
        lines.push(format!("Pression de {} : -{:.0} PV/s  -  remontez !", inside.name, inside.damage_per_sec));
    } else if net.is_enabled() && (net.player_count() > 1 || hp < MAX_HP) {
        lines.push(format!("Coque {hp}/{MAX_HP}   (F : tirer)"));
    } else if hp < MAX_HP {
        lines.push(format!("Coque {hp}/{MAX_HP}   (reparation en cours)"));
    }
    let label = lines.join("\n");
    let color = if hp == 0 || gas.inside.is_some() {
        Relation::Enemy.color()
    } else if hp < MAX_HP / 2 {
        Color::srgb(1.0, 0.75, 0.3)
    } else {
        Color::srgb(0.9, 0.9, 0.95)
    };
    for (mut text, mut c) in &mut hud {
        if text.0 != label {
            text.0 = label.clone();
        }
        if c.0 != color {
            c.0 = color;
        }
    }
}

fn hide_destroyed_ship(net: Res<Net>, mut ship_q: Query<&mut Visibility, With<Ship>>) {
    if net.local.hp > 0 {
        return;
    }
    for mut vis in &mut ship_q {
        *vis = Visibility::Hidden;
    }
}
