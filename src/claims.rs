// ─────────────────────────────────────────────────────────────────────────
//  Revendication d'étoiles
//
//  Touche C sur une étoile (ou un astre de son système) : la revendiquer,
//  ou l'abandonner si elle est déjà à soi. 5 étoiles au plus par joueur.
//
//  Un territoire appartient à la guilde du joueur s'il a un tag, sinon au
//  joueur seul. Il est dessiné comme une frontière autour de l'étoile ; les
//  frontières d'étoiles proches d'un même propriétaire fusionnent en un seul
//  contour.
//
//  Chaque joueur garde ses revendications dans ses réglages et les annonce
//  aux autres avec son état réseau (pas de serveur central).
// ─────────────────────────────────────────────────────────────────────────

use bevy::prelude::*;

use crate::diplomacy::relation_with;
use crate::guild::Guilds;
use crate::net::{display_name, sanitize_tag, Net, MAX_CLAIMS};
use crate::ship::Ship;
use crate::net_ui::NetPanel;
use crate::planet::{StarId, StarRoot};
use crate::settings::GameSettings;
use crate::ui::MenuState;
use crate::{target_system, CameraTarget};

/// Rayon de la frontière autour d'une étoile revendiquée. Deux étoiles d'un
/// même propriétaire à moins de 2 rayons ont une frontière commune.
pub const CLAIM_RADIUS: f32 = 25_000.0 * crate::settings::GALAXY_SCALE;
const CIRCLE_SEGMENTS: usize = 96;

pub struct ClaimsPlugin;

impl Plugin for ClaimsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SiegeState>()
            .add_systems(Update, (claim_input, siege_tick, defend_claims, draw_claims).chain());
    }
}

/// Propriétaire d'un territoire : une guilde entière, ou un joueur sans guilde.
#[derive(Clone, PartialEq, Eq, Debug)]
enum Owner {
    Guild(String),
    Me,
    Peer(u32),
}

struct Claim {
    sys: usize,
    owner: Owner,
    /// Nom affiché du joueur qui a posé la revendication.
    player: String,
    color: Color,
}

/// Couleur d'une guilde, déduite de son tag (la même chez tous les joueurs).
fn guild_color(tag: &str) -> Color {
    let hash = tag.bytes().fold(2166136261u32, |h, b| (h ^ b as u32).wrapping_mul(16777619));
    Color::hsl((hash % 360) as f32, 0.85, 0.6)
}

fn owner_of(guilds: &Guilds, tag: &str, color: [f32; 3], fallback: Owner) -> (Owner, Color) {
    if tag.is_empty() {
        (fallback, Color::srgb(color[0], color[1], color[2]))
    } else {
        // Couleur choisie par la guilde (à défaut, une couleur déduite du tag)
        let color = guilds.by_tag(tag).map_or_else(|| guild_color(tag), |g| Color::srgb(g.color[0], g.color[1], g.color[2]));
        (Owner::Guild(tag.to_string()), color)
    }
}

/// Toutes les revendications connues : les miennes et celles des joueurs connectés.
fn all_claims(net: &Net, settings: &GameSettings, guilds: &Guilds) -> Vec<Claim> {
    let mut out = Vec::new();
    let my_tag = sanitize_tag(&settings.clan_tag);
    let (owner, color) = owner_of(guilds, &my_tag, settings.aura_color, Owner::Me);
    let player = display_name(&my_tag, &settings.player_name);
    for &sys in settings.claims.iter().take(MAX_CLAIMS) {
        out.push(Claim { sys: sys as usize, owner: owner.clone(), player: player.clone(), color });
    }
    for (&id, peer) in &net.peers {
        let (owner, color) = owner_of(guilds, &peer.tag, peer.color, Owner::Peer(id));
        let player = display_name(&peer.tag, &peer.name);
        for &sys in peer.claims.iter().take(MAX_CLAIMS) {
            out.push(Claim { sys: sys as usize, owner: owner.clone(), player: player.clone(), color });
        }
    }
    out.retain(|c| c.sys < settings.systems.len());
    out
}

/// Étoiles revendiquées regroupées par propriétaire (une guilde forme un seul
/// groupe), avec la couleur du territoire. Sert à tracer les liaisons.
pub fn owner_groups(net: &Net, settings: &GameSettings, guilds: &Guilds) -> Vec<(Color, Vec<usize>)> {
    let mut groups: Vec<(Owner, Color, Vec<usize>)> = Vec::new();
    for claim in all_claims(net, settings, guilds) {
        match groups.iter_mut().find(|g| g.0 == claim.owner) {
            Some(group) => {
                if !group.2.contains(&claim.sys) {
                    group.2.push(claim.sys);
                }
            }
            None => groups.push((claim.owner, claim.color, vec![claim.sys])),
        }
    }
    groups.into_iter().map(|(_, color, stars)| (color, stars)).collect()
}

/// Qui a revendiqué ce système (pour l'affichage), s'il l'est.
pub fn claim_owner_label(sys: usize, net: &Net, settings: &GameSettings, guilds: &Guilds) -> Option<String> {
    all_claims(net, settings, guilds).into_iter().find(|c| c.sys == sys).map(|c| match c.owner {
        Owner::Guild(tag) => format!("la guilde [{tag}] ({})", c.player),
        _ => c.player,
    })
}

fn claim_input(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    target: Res<CameraTarget>,
    star_q: Query<&StarId, With<StarRoot>>,
    panel: Res<NetPanel>,
    menu: Res<MenuState>,
    ship_q: Query<&GlobalTransform, With<Ship>>,
    mut siege: ResMut<SiegeState>,
    npcs: Res<crate::galaxy_fx::NpcTerritories>,
    mut settings: ResMut<GameSettings>,
    mut net: ResMut<Net>,
    travel: Res<crate::wormhole::WormholeTravel>,
) {
    if !keys.just_pressed(KeyCode::KeyC) || panel.focus.is_some() || menu.open {
        return;
    }
    let now = time.elapsed_secs_f64();
    // Pendant un voyage en trou de ver : ni revendication, ni abandon, ni siège
    if travel.active() {
        net.notify("Impossible de revendiquer pendant un voyage en trou de ver.", now);
        return;
    }
    let Some(Some(sys)) = target_system(&target.0, &star_q) else {
        net.notify("Ciblez une etoile pour la revendiquer.", now);
        return;
    };
    let Some(name) = settings.systems.get(sys).map(|s| s.name.clone()) else { return };
    let sys_id = sys as u32;

    if let Some(i) = settings.claims.iter().position(|&s| s == sys_id) {
        settings.claims.remove(i);
        settings.save();
        net.notify(&format!("Vous abandonnez {name} ({}/{MAX_CLAIMS}).", settings.claims.len()), now);
        return;
    }
    // Territoire d'une faction PNJ : ni revendication ni siège
    if let Some(faction) = npcs.faction_of(sys) {
        net.notify(&format!("{name} fait partie du territoire de {} (PNJ) : impossible de la revendiquer.", faction.name), now);
        return;
    }
    // Étoile d'un autre joueur : C lance (ou annule) un siège pour la lui prendre
    if let Some(peer) = net.peers.values().find(|p| p.claims.contains(&sys_id)) {
        let who = display_name(&peer.tag, &peer.name);
        if net.local.siege == Some(sys_id) {
            net.local.siege = None;
            net.notify(&format!("Siege de {name} annule."), now);
            return;
        }
        let text = if !relation_with(peer, &settings).can_attack() {
            format!("{name} appartient a {who}, votre allie : impossible de l'attaquer.")
        } else if net.local.hp == 0 {
            "Votre vaisseau est detruit.".to_string()
        } else if !ship_in_territory(&ship_q, &settings, sys) {
            format!("{name} appartient a {who}. Allez-y avec votre vaisseau puis appuyez sur C pour l'assieger.")
        } else {
            net.local.siege = Some(sys_id);
            siege.progress = 0.0;
            siege.blocked = false;
            format!("Vous assiegez {name} ({who}) : tenez {SIEGE_SECS:.0} s sans defenseur sur place. C pour annuler.")
        };
        net.notify(&text, now);
        return;
    }
    if settings.claims.len() >= MAX_CLAIMS {
        net.notify(
            &format!("Maximum {MAX_CLAIMS} etoiles : abandonnez-en une (C sur une etoile a vous)."),
            now,
        );
        return;
    }
    settings.claims.push(sys_id);
    settings.save();
    net.notify(&format!("Vous revendiquez {name} ({}/{MAX_CLAIMS}).", settings.claims.len()), now);
}

// ─────────────────────────────────────────────────────────────────────────
//  Sièges : prendre l'étoile d'un joueur neutre ou ennemi
//
//  L'assiégeant doit rester dans le territoire de l'étoile pendant
//  SIEGE_SECS. Le siège n'avance pas tant qu'un défenseur (le propriétaire
//  ou un membre de sa guilde, vaisseau intact) s'y trouve aussi. À la fin,
//  l'assiégeant annonce la prise (`PlayerStatus::taken`) et le propriétaire
//  retire lui-même sa revendication.
// ─────────────────────────────────────────────────────────────────────────

/// Durée d'un siège sans défenseur (secondes).
pub const SIEGE_SECS: f32 = 20.0;
/// Durée pendant laquelle une prise est annoncée aux autres joueurs. Plus court
/// qu'un siège : une reprise ne peut pas être annulée par une vieille annonce.
const TAKEN_ANNOUNCE_SECS: f64 = 10.0;

/// Avancement du siège en cours (l'étoile visée est dans `Net::local.siege`).
#[derive(Resource, Default)]
pub struct SiegeState {
    pub progress: f32,
    /// Un défenseur est sur place : le siège n'avance pas.
    pub blocked: bool,
}

fn in_territory(pos: Vec3, settings: &GameSettings, sys: usize) -> bool {
    settings.systems.get(sys).is_some_and(|s| s.center().distance(pos) < CLAIM_RADIUS)
}

fn ship_in_territory(ship_q: &Query<&GlobalTransform, With<Ship>>, settings: &GameSettings, sys: usize) -> bool {
    ship_q.get_single().is_ok_and(|gt| in_territory(gt.translation(), settings, sys))
}

fn siege_tick(
    time: Res<Time>,
    ship_q: Query<&GlobalTransform, With<Ship>>,
    mut siege: ResMut<SiegeState>,
    mut settings: ResMut<GameSettings>,
    mut net: ResMut<Net>,
) {
    let now = time.elapsed_secs_f64();
    net.local.taken.retain(|t| t.1 > now);
    let Some(sys_id) = net.local.siege else { return };
    let sys = sys_id as usize;
    let name = settings.systems.get(sys).map_or_else(|| "?".to_string(), |s| s.name.clone());

    // Propriétaire actuel (il peut avoir abandonné l'étoile ou s'être déconnecté)
    let owner = net.peers.values().find(|p| p.claims.contains(&sys_id));
    let stop = match owner {
        None => Some(format!("Siege de {name} termine : l'etoile n'est plus revendiquee.")),
        Some(p) if !relation_with(p, &settings).can_attack() => {
            Some(format!("Siege de {name} annule : son proprietaire est votre allie."))
        }
        _ if net.local.hp == 0 => Some(format!("Siege de {name} rompu : votre vaisseau est detruit.")),
        _ if !ship_in_territory(&ship_q, &settings, sys) => {
            Some(format!("Siege de {name} rompu : vous avez quitte son territoire."))
        }
        _ => None,
    };
    if let Some(text) = stop {
        net.local.siege = None;
        siege.progress = 0.0;
        net.notify(&text, now);
        return;
    }
    let Some(owner) = owner else { return };
    let who = display_name(&owner.tag, &owner.name);

    // Défenseurs sur place : le propriétaire ou sa guilde
    siege.blocked = net.peers.values().any(|p| {
        let defends = p.claims.contains(&sys_id) || (!owner.tag.is_empty() && p.tag == owner.tag);
        defends && p.status.hp > 0 && in_territory(p.pos(), &settings, sys)
    });
    if siege.blocked {
        return;
    }
    siege.progress += time.delta_secs();
    if siege.progress < SIEGE_SECS {
        return;
    }

    // Prise de l'étoile
    net.local.siege = None;
    siege.progress = 0.0;
    net.local.taken.push((sys_id, now + TAKEN_ANNOUNCE_SECS));
    if settings.claims.len() < MAX_CLAIMS {
        settings.claims.push(sys_id);
        settings.save();
        net.notify(&format!("Vous avez pris {name} a {who} ! ({}/{MAX_CLAIMS})", settings.claims.len()), now);
    } else {
        net.notify(&format!("{name} n'appartient plus a {who} (vous avez deja {MAX_CLAIMS} etoiles)."), now);
    }
}

/// Côté défenseur : alerte quand une de mes étoiles est assiégée, et perte
/// de l'étoile quand un assiégeant annonce l'avoir prise.
fn defend_claims(
    time: Res<Time>,
    mut settings: ResMut<GameSettings>,
    mut net: ResMut<Net>,
    mut warned: Local<Vec<(u32, u32)>>,
) {
    if settings.claims.is_empty() {
        warned.clear();
        return;
    }
    let now = time.elapsed_secs_f64();
    let mut messages: Vec<String> = Vec::new();
    let mut lost: Vec<u32> = Vec::new();
    let mut sieges: Vec<(u32, u32)> = Vec::new();
    for (&id, peer) in &net.peers {
        // Un allié ne peut pas me prendre d'étoile
        if !relation_with(peer, &settings).can_attack() {
            continue;
        }
        let who = display_name(&peer.tag, &peer.name);
        let star = |sys: u32| settings.systems.get(sys as usize).map_or("?", |s| s.name.as_str());
        if let Some(sys) = peer.status.siege.filter(|s| settings.claims.contains(s)) {
            sieges.push((id, sys));
            if !warned.contains(&(id, sys)) {
                messages.push(format!("{who} assiege votre etoile {} ! Allez la defendre.", star(sys)));
            }
        }
        for &sys in peer.status.taken.iter().filter(|s| settings.claims.contains(s)) {
            if !lost.contains(&sys) {
                lost.push(sys);
                messages.push(format!("{who} vous a pris l'etoile {} !", star(sys)));
            }
        }
    }
    *warned = sieges;
    if !lost.is_empty() {
        settings.claims.retain(|s| !lost.contains(s));
        settings.save();
    }
    for m in messages {
        net.notify(&m, now);
    }
}

/// Cercle de frontière dans le plan de sa galaxie.
pub(crate) struct Border {
    center: Vec3,
    u: Vec3,
    v: Vec3,
    normal: Vec3,
}

impl Border {
    #[cfg(test)]
    fn point(&self, angle: f32, radius: f32) -> Vec3 {
        self.center + (self.u * angle.cos() + self.v * angle.sin()) * radius
    }

    /// Le point est-il à l'intérieur de ce cercle (distance mesurée dans son plan) ?
    #[cfg(test)]
    fn contains(&self, p: Vec3, radius: f32) -> bool {
        let d = p - self.center;
        let flat = d - self.normal * d.dot(self.normal);
        flat.length_squared() < radius * radius
    }
}

/// Regroupe les cercles qui se touchent (centres à moins de 2 rayons).
fn touching_groups(borders: &[Border], radius: f32) -> Vec<Vec<usize>> {
    let mut group_of: Vec<Option<usize>> = vec![None; borders.len()];
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for start in 0..borders.len() {
        if group_of[start].is_some() {
            continue;
        }
        let id = groups.len();
        let mut members = vec![start];
        group_of[start] = Some(id);
        let mut next = 0;
        while next < members.len() {
            let a = members[next];
            next += 1;
            for b in 0..borders.len() {
                if group_of[b].is_none() && borders[a].center.distance(borders[b].center) < 2.0 * radius {
                    group_of[b] = Some(id);
                    members.push(b);
                }
            }
        }
        groups.push(members);
    }
    groups
}

/// Arcs (angles début, fin) d'un cercle qui ne sont recouverts par aucun autre.
fn free_arcs(covered: &mut Vec<(f32, f32)>) -> Vec<(f32, f32)> {
    use std::f32::consts::TAU;
    // Recouvrements ramenés dans [0, TAU), en coupant ceux qui passent par 0
    let mut spans: Vec<(f32, f32)> = Vec::new();
    for &(a, b) in covered.iter() {
        let a0 = a.rem_euclid(TAU);
        let b0 = a0 + (b - a);
        if b0 > TAU {
            spans.push((a0, TAU));
            spans.push((0.0, b0 - TAU));
        } else {
            spans.push((a0, b0));
        }
    }
    spans.sort_by(|x, y| x.0.total_cmp(&y.0));
    let mut free = Vec::new();
    let mut cursor = 0.0;
    for (a, b) in spans {
        if a > cursor {
            free.push((cursor, a));
        }
        cursor = cursor.max(b);
    }
    if cursor < TAU {
        free.push((cursor, TAU));
    }
    // Un arc libre qui touche 0 des deux côtés ne fait qu'un
    if free.len() > 1 && free[0].0 == 0.0 && free[free.len() - 1].1 == TAU {
        let last = free.pop().unwrap();
        free[0].0 = last.0 - TAU;
    }
    free
}

/// Segments du contour extérieur d'un groupe de cercles de même rayon.
///
/// Les cercles qui se touchent sont ramenés dans un plan commun (à la hauteur
/// moyenne du groupe), puis on calcule pour chacun les arcs recouverts par ses
/// voisins : ils commencent et finissent exactement aux points d'intersection,
/// si bien que le contour de l'ensemble est continu.
pub(crate) fn outline_segments(borders: &[Border], radius: f32) -> Vec<(Vec3, Vec3)> {
    use std::f32::consts::TAU;
    let step = TAU / CIRCLE_SEGMENTS as f32;
    let mut out = Vec::new();
    for group in touching_groups(borders, radius) {
        let first = &borders[group[0]];
        let (u, v, normal) = (first.u, first.v, first.normal);
        let height = group.iter().map(|&i| borders[i].center.dot(normal)).sum::<f32>() / group.len() as f32;
        // Centres ramenés dans le plan commun
        let centers: Vec<Vec3> = group
            .iter()
            .map(|&i| borders[i].center - normal * (borders[i].center.dot(normal) - height))
            .collect();
        for (a, &ca) in centers.iter().enumerate() {
            let mut covered: Vec<(f32, f32)> = Vec::new();
            let mut hidden = false;
            for (b, &cb) in centers.iter().enumerate() {
                if a == b {
                    continue;
                }
                let d = cb - ca;
                let (dx, dy) = (d.dot(u), d.dot(v));
                let dist = (dx * dx + dy * dy).sqrt();
                if dist < 1.0 {
                    // Même position : un seul des deux est dessiné
                    hidden |= b < a;
                    continue;
                }
                if dist >= 2.0 * radius {
                    continue;
                }
                let half = (dist / (2.0 * radius)).clamp(-1.0, 1.0).acos();
                let mid = dy.atan2(dx);
                covered.push((mid - half, mid + half));
            }
            if hidden {
                continue;
            }
            for (start, end) in free_arcs(&mut covered) {
                let n = (((end - start) / step - 1.0e-3).ceil() as usize).max(1);
                let point = |t: f32| ca + (u * t.cos() + v * t.sin()) * radius;
                for s in 0..n {
                    let t0 = start + (end - start) * s as f32 / n as f32;
                    let t1 = start + (end - start) * (s + 1) as f32 / n as f32;
                    out.push((point(t0), point(t1)));
                }
            }
        }
    }
    out
}

pub(crate) fn border_of(settings: &GameSettings, sys: usize) -> Option<Border> {
    let sys = settings.systems.get(sys)?;
    let tilt = settings.galaxies.get(sys.galaxy_id as usize).map_or(Quat::IDENTITY, |g| g.tilt);
    Some(Border { center: sys.center(), u: tilt * Vec3::X, v: tilt * Vec3::Z, normal: tilt * Vec3::Y })
}

fn draw_claims(time: Res<Time>, net: Res<Net>, settings: Res<GameSettings>, guilds: Res<Guilds>, mut gizmos: Gizmos) {
    let claims = all_claims(&net, &settings, &guilds);
    if claims.is_empty() {
        return;
    }

    // Étoiles assiégées : anneau rouge clignotant autour de la frontière
    let blink = (time.elapsed_secs() * 6.0).sin() * 0.5 + 0.5;
    let sieged = net.peers.values().filter_map(|p| p.status.siege).chain(net.local.siege);
    for sys in sieged {
        if !claims.iter().any(|c| c.sys == sys as usize) {
            continue;
        }
        if let Some(border) = border_of(&settings, sys as usize) {
            let red = Color::srgba(1.0, 0.2, 0.15, 0.3 + 0.7 * blink);
            for (a, b) in outline_segments(std::slice::from_ref(&border), CLAIM_RADIUS * 1.08) {
                gizmos.line(a, b, red);
            }
        }
    }
    let mut owners: Vec<&Owner> = Vec::new();
    for c in &claims {
        if !owners.contains(&&c.owner) {
            owners.push(&c.owner);
        }
    }
    for owner in owners {
        let mut color = Color::WHITE;
        let mut borders: Vec<Border> = Vec::new();
        for c in claims.iter().filter(|c| &c.owner == owner) {
            let Some(sys) = settings.systems.get(c.sys) else { continue };
            let center = sys.center();
            // Deux joueurs d'une même guilde sur la même étoile : un seul cercle
            if borders.iter().any(|b| b.center == center) {
                continue;
            }
            borders.extend(border_of(&settings, c.sys));
            color = c.color;
        }
        // Frontière nette + liseré intérieur plus discret
        for (scale, alpha) in [(1.0, 1.0), (0.94, 0.35)] {
            for (a, b) in outline_segments(&borders, CLAIM_RADIUS * scale) {
                gizmos.line(a, b, color.with_alpha(alpha));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn border(x: f32) -> Border {
        Border { center: Vec3::new(x, 0.0, 0.0), u: Vec3::X, v: Vec3::Z, normal: Vec3::Y }
    }

    #[test]
    fn close_stars_share_one_outline() {
        let full = CIRCLE_SEGMENTS;
        // Une étoile seule : cercle complet
        assert_eq!(outline_segments(&[border(0.0)], 10.0).len(), full);
        // Deux étoiles éloignées : deux cercles complets
        assert_eq!(outline_segments(&[border(0.0), border(100.0)], 10.0).len(), 2 * full);
        // Deux étoiles proches : les arcs intérieurs disparaissent, aucun segment
        // restant n'est à l'intérieur de l'autre cercle
        let borders = [border(0.0), border(12.0)];
        let segs = outline_segments(&borders, 10.0);
        assert!(segs.len() < 2 * full && segs.len() > full);
        for (a, b) in &segs {
            let mid = (*a + *b) * 0.5;
            let inside = borders.iter().filter(|o| o.contains(mid, 9.9)).count();
            assert_eq!(inside, 0);
        }
    }

    #[test]
    fn merged_outline_is_one_closed_loop_even_at_different_heights() {
        // Trois étoiles proches à des hauteurs différentes, disposées en chaîne
        let at = |x: f32, y: f32, z: f32| Border { center: Vec3::new(x, y, z), u: Vec3::X, v: Vec3::Z, normal: Vec3::Y };
        let borders = [at(0.0, 0.0, 0.0), at(12.0, 3.0, 5.0), at(22.0, -2.0, -4.0)];
        let segs = outline_segments(&borders, 10.0);
        let key = |p: Vec3| ((p.x * 100.0).round() as i32, (p.y * 100.0).round() as i32, (p.z * 100.0).round() as i32);
        let mut degree: std::collections::HashMap<(i32, i32, i32), u32> = std::collections::HashMap::new();
        for (a, b) in &segs {
            *degree.entry(key(*a)).or_default() += 1;
            *degree.entry(key(*b)).or_default() += 1;
        }
        // Contour fermé et continu : chaque extrémité est partagée par exactement deux segments
        assert!(degree.values().all(|&n| n == 2), "{:?}", degree.iter().filter(|(_, n)| **n != 2).collect::<Vec<_>>());
        // Tout le contour est dans un seul plan
        let y0 = segs[0].0.y;
        assert!(segs.iter().all(|(a, b)| (a.y - y0).abs() < 1.0e-3 && (b.y - y0).abs() < 1.0e-3));
    }

    #[test]
    fn guild_color_is_stable_per_tag() {
        assert_eq!(guild_color("ABC"), guild_color("ABC"));
        assert_ne!(guild_color("ABC"), guild_color("XYZ"));
    }
}
