// ─────────────────────────────────────────────────────────────────────────
//  Multijoueur
//
//  Réseau UDP direct, sans aucun service externe (pas de Steam, Hamachi…) :
//   - un joueur « héberge » : il ouvre le port UDP NET_PORT ;
//   - les autres le trouvent automatiquement sur le réseau local
//     (broadcast) ou tapent son adresse IP.
//
//  La carte est générée localement chez chaque joueur (même code, même
//  graine). Seuls s'échangent : position/orientation du vaisseau, pseudo et
//  couleur d'aura. L'hôte relaie l'état de tout le monde et partage son
//  horloge d'univers pour que les planètes soient au même endroit
//  sur toutes les machines.
// ─────────────────────────────────────────────────────────────────────────

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::ErrorKind;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs, UdpSocket};

use crate::settings::GameSettings;
use crate::ship::{aura_materials, LocalAura, Ship, ShipAssets};
use crate::CameraController;

pub const NET_PORT: u16 = 27777;
pub const MAX_NAME_LEN: usize = 16;
const PROTOCOL: u32 = 1;
const MAGIC: &str = "SPACESPORE";
const GAME_VERSION: &str = env!("CARGO_PKG_VERSION");
const MAX_PLAYERS: usize = 16;
const SEND_INTERVAL: f64 = 0.05;
const TIMEOUT: f64 = 6.0;
const JOIN_RETRY: f64 = 0.5;
const JOIN_TIMEOUT: f64 = 6.0;
const DISCOVER_INTERVAL: f64 = 1.5;
const LAN_EXPIRY: f64 = 5.0;
const HOST_ID: u32 = 0;

pub struct NetPlugin;

impl Plugin for NetPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UniverseClock>()
            .init_resource::<Net>()
            .add_event::<NetCommand>()
            .add_systems(
                Update,
                (
                    handle_net_commands,
                    net_update,
                    sync_remote_ships,
                    update_remote_labels,
                    update_local_aura,
                )
                    .chain(),
            )
            .add_systems(Last, send_bye_on_exit);
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  Horloge d'univers partagée (orbites synchronisées)
// ─────────────────────────────────────────────────────────────────────────

#[derive(Resource, Default)]
pub struct UniverseClock {
    offset: f64,
}

impl UniverseClock {
    pub fn secs(&self, time: &Time) -> f32 {
        self.secs_f64(time) as f32
    }

    fn secs_f64(&self, time: &Time) -> f64 {
        time.elapsed_secs_f64() + self.offset
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  Protocole
// ─────────────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug)]
struct PlayerState {
    id: u32,
    name: String,
    color: [f32; 3],
    pos: [f32; 3],
    rot: [f32; 4],
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "t")]
enum Msg {
    Discover { magic: String },
    HostInfo { magic: String, host: String, players: usize, game: String },
    Hello { magic: String, proto: u32, game: String, world: u64, name: String, color: [f32; 3] },
    Welcome { id: u32, clock: f64 },
    Reject { reason: String },
    State { name: String, color: [f32; 3], pos: [f32; 3], rot: [f32; 4] },
    Snapshot { clock: f64, players: Vec<PlayerState> },
    Bye,
}

fn send(socket: &UdpSocket, addr: SocketAddr, msg: &Msg) {
    if let Ok(bytes) = serde_json::to_vec(msg) {
        let _ = socket.send_to(&bytes, addr);
    }
}

/// Lit tous les paquets en attente sans bloquer.
fn recv_all(socket: &UdpSocket) -> Vec<(SocketAddr, Msg)> {
    let mut out = Vec::new();
    let mut buf = [0u8; 16 * 1024];
    for _ in 0..512 {
        match socket.recv_from(&mut buf) {
            Ok((n, addr)) => {
                if let Ok(msg) = serde_json::from_slice::<Msg>(&buf[..n]) {
                    out.push((addr, msg));
                }
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => break,
            // Windows renvoie ConnectionReset en UDP quand un pair a disparu : on ignore.
            Err(e) if e.kind() == ErrorKind::ConnectionReset => continue,
            Err(_) => break,
        }
    }
    out
}

fn sanitize_name(name: &str) -> String {
    let clean: String = name
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_NAME_LEN)
        .collect();
    let clean = clean.trim();
    if clean.is_empty() { "Pilote".into() } else { clean.to_string() }
}

fn sanitize_color(c: [f32; 3]) -> [f32; 3] {
    c.map(|v| if v.is_finite() { v.clamp(0.0, 1.0) } else { 1.0 })
}

fn valid_vec(v: &[f32]) -> bool {
    v.iter().all(|x| x.is_finite() && x.abs() < 1.0e9)
}

/// Empreinte de la carte générée. N'utilise que des valeurs calculées sans
/// trigonométrie (identiques au bit près sur Windows, Linux et macOS).
pub fn world_fingerprint(settings: &GameSettings) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    let mut eat = |bytes: &[u8]| {
        for b in bytes {
            h ^= *b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
    };
    eat(&(settings.systems.len() as u64).to_le_bytes());
    for sys in &settings.systems {
        eat(sys.name.as_bytes());
        eat(&(sys.stars.len() as u64).to_le_bytes());
        for st in &sys.stars {
            eat(&st.radius.to_bits().to_le_bytes());
            eat(&st.orbit_distance.to_bits().to_le_bytes());
        }
        eat(&(sys.planets.len() as u64).to_le_bytes());
        for p in &sys.planets {
            eat(&p.seed.to_le_bytes());
            eat(&p.radius.to_bits().to_le_bytes());
            eat(&p.orbit_distance.to_bits().to_le_bytes());
            eat(&(p.moons.len() as u64).to_le_bytes());
        }
    }
    h
}

/// IP locale utilisée pour sortir sur le réseau (aucun paquet n'est envoyé).
pub fn local_ip() -> Option<IpAddr> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    socket.connect((Ipv4Addr::new(8, 8, 8, 8), 80)).ok()?;
    let ip = socket.local_addr().ok()?.ip();
    if ip.is_unspecified() { None } else { Some(ip) }
}

fn parse_address(input: &str) -> Option<SocketAddr> {
    let input = input.trim();
    if input.is_empty() {
        return None;
    }
    if let Ok(addr) = input.parse::<SocketAddr>() {
        return Some(addr);
    }
    if let Ok(ip) = input.trim_matches(|c| c == '[' || c == ']').parse::<IpAddr>() {
        return Some(SocketAddr::new(ip, NET_PORT));
    }
    let resolved: Vec<SocketAddr> = if input.contains(':') {
        input.to_socket_addrs().ok()?.collect()
    } else {
        (input, NET_PORT).to_socket_addrs().ok()?.collect()
    };
    resolved
        .iter()
        .find(|a| a.is_ipv4())
        .or_else(|| resolved.first())
        .copied()
}

// ─────────────────────────────────────────────────────────────────────────
//  État réseau
// ─────────────────────────────────────────────────────────────────────────

struct ClientSlot {
    id: u32,
    last_seen: f64,
}

enum Session {
    Offline,
    Hosting {
        socket: UdpSocket,
        world: u64,
        clients: HashMap<SocketAddr, ClientSlot>,
        next_id: u32,
    },
    Joining {
        socket: UdpSocket,
        host: SocketAddr,
        world: u64,
        started: f64,
        last_hello: f64,
    },
    Connected {
        socket: UdpSocket,
        host: SocketAddr,
        my_id: u32,
        last_recv: f64,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NetMode {
    Offline,
    Hosting,
    Joining,
    Connected,
}

/// Un autre joueur, tel qu'affiché localement.
pub struct Peer {
    pub name: String,
    pub color: [f32; 3],
    pos: Vec3,
    rot: Quat,
    vel: Vec3,
    last_update: f64,
}

pub struct LanGame {
    pub addr: SocketAddr,
    pub host: String,
    pub players: usize,
    pub compatible: bool,
    seen: f64,
}

#[derive(Resource)]
pub struct Net {
    session: Session,
    pub status: String,
    pub status_is_error: bool,
    pub peers: HashMap<u32, Peer>,
    pub lan_games: Vec<LanGame>,
    /// Active la recherche de parties sur le réseau local (panneau ouvert).
    pub discovering: bool,
    discover_socket: Option<UdpSocket>,
    last_discover: f64,
    last_send: f64,
    pub local_ip: Option<IpAddr>,
}

impl Default for Net {
    fn default() -> Self {
        Self {
            session: Session::Offline,
            status: "Hors ligne".into(),
            status_is_error: false,
            peers: HashMap::new(),
            lan_games: Vec::new(),
            discovering: false,
            discover_socket: None,
            last_discover: f64::NEG_INFINITY,
            last_send: f64::NEG_INFINITY,
            local_ip: None,
        }
    }
}

impl Net {
    pub fn mode(&self) -> NetMode {
        match self.session {
            Session::Offline => NetMode::Offline,
            Session::Hosting { .. } => NetMode::Hosting,
            Session::Joining { .. } => NetMode::Joining,
            Session::Connected { .. } => NetMode::Connected,
        }
    }

    /// Identifiant du joueur local (0 = hôte ou hors ligne).
    pub fn my_id(&self) -> u32 {
        match self.session {
            Session::Connected { my_id, .. } => my_id,
            _ => HOST_ID,
        }
    }

    /// Décalage du point de stationnement du vaisseau, pour que deux joueurs
    /// posés sur le même astre ne se superposent pas.
    pub fn hover_offset(&self, zoom_distance: f32) -> Vec3 {
        let id = self.my_id();
        if id == HOST_ID {
            return Vec3::ZERO;
        }
        let angle = id as f32 * 2.399_963; // angle d'or : bonne répartition
        Vec3::new(angle.cos(), 0.0, angle.sin()) * zoom_distance.max(1.0) * 0.06
    }

    fn set_status(&mut self, text: impl Into<String>, error: bool) {
        self.status = text.into();
        self.status_is_error = error;
    }

    fn leave(&mut self) {
        match &self.session {
            Session::Hosting { socket, clients, .. } => {
                for addr in clients.keys() {
                    send(socket, *addr, &Msg::Bye);
                }
            }
            Session::Connected { socket, host, .. } | Session::Joining { socket, host, .. } => {
                send(socket, *host, &Msg::Bye);
            }
            Session::Offline => {}
        }
        self.session = Session::Offline;
        self.peers.clear();
    }
}

fn update_peer(peers: &mut HashMap<u32, Peer>, st: PlayerState, now: f64) {
    if !valid_vec(&st.pos) || !valid_vec(&st.rot) {
        return;
    }
    let pos = Vec3::from_array(st.pos);
    let q = Quat::from_array(st.rot);
    let rot = if q.length_squared() > 1.0e-6 { q.normalize() } else { Quat::IDENTITY };
    let name = sanitize_name(&st.name);
    let color = sanitize_color(st.color);
    match peers.get_mut(&st.id) {
        Some(p) => {
            let dt = (now - p.last_update) as f32;
            if dt > 0.001 {
                p.vel = (pos - p.pos) / dt;
            }
            p.pos = pos;
            p.rot = rot;
            p.name = name;
            p.color = color;
            p.last_update = now;
        }
        None => {
            peers.insert(st.id, Peer { name, color, pos, rot, vel: Vec3::ZERO, last_update: now });
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  Commandes (depuis l'interface)
// ─────────────────────────────────────────────────────────────────────────

#[derive(Event, Clone)]
pub enum NetCommand {
    Host,
    Join(String),
    Leave,
}

fn handle_net_commands(
    mut events: EventReader<NetCommand>,
    mut net: ResMut<Net>,
    mut settings: ResMut<GameSettings>,
    time: Res<Time>,
) {
    for ev in events.read() {
        let now = time.elapsed_secs_f64();
        match ev {
            NetCommand::Host => {
                net.leave();
                match UdpSocket::bind((Ipv4Addr::UNSPECIFIED, NET_PORT)) {
                    Ok(socket) => {
                        let _ = socket.set_nonblocking(true);
                        let _ = socket.set_broadcast(true);
                        net.session = Session::Hosting {
                            socket,
                            world: world_fingerprint(&settings),
                            clients: HashMap::new(),
                            next_id: 1,
                        };
                        net.local_ip = local_ip();
                        let ip = net
                            .local_ip
                            .map(|ip| ip.to_string())
                            .unwrap_or_else(|| "inconnue".into());
                        net.set_status(format!("Partie ouverte - votre IP : {ip}"), false);
                    }
                    Err(e) => {
                        net.set_status(
                            format!("Impossible d'ouvrir le port {NET_PORT} : {e}. Une autre partie est peut-etre deja ouverte sur ce PC."),
                            true,
                        );
                    }
                }
            }
            NetCommand::Join(address) => {
                net.leave();
                let Some(host) = parse_address(address) else {
                    net.set_status(format!("Adresse invalide : \"{}\"", address.trim()), true);
                    continue;
                };
                let bind: SocketAddr = if host.is_ipv4() {
                    (Ipv4Addr::UNSPECIFIED, 0).into()
                } else {
                    (std::net::Ipv6Addr::UNSPECIFIED, 0).into()
                };
                match UdpSocket::bind(bind) {
                    Ok(socket) => {
                        let _ = socket.set_nonblocking(true);
                        net.session = Session::Joining {
                            socket,
                            host,
                            world: world_fingerprint(&settings),
                            started: now,
                            last_hello: f64::NEG_INFINITY,
                        };
                        net.set_status(format!("Connexion a {host}..."), false);
                        if settings.last_join_address != address.trim() {
                            settings.last_join_address = address.trim().to_string();
                            settings.save();
                        }
                    }
                    Err(e) => net.set_status(format!("Erreur reseau : {e}"), true),
                }
            }
            NetCommand::Leave => {
                net.leave();
                net.set_status("Hors ligne", false);
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  Boucle réseau
// ─────────────────────────────────────────────────────────────────────────

fn net_update(
    time: Res<Time>,
    settings: Res<GameSettings>,
    ship_q: Query<&GlobalTransform, With<Ship>>,
    mut clock: ResMut<UniverseClock>,
    mut net: ResMut<Net>,
) {
    let now = time.elapsed_secs_f64();
    let (pos, rot) = ship_q
        .get_single()
        .map(|gt| {
            let (_, r, t) = gt.to_scale_rotation_translation();
            (t, r)
        })
        .unwrap_or((Vec3::ZERO, Quat::IDENTITY));
    let my_name = sanitize_name(&settings.player_name);
    let my_color = sanitize_color(settings.aura_color);

    discovery(&mut net, now);

    let tick = now - net.last_send >= SEND_INTERVAL;
    if tick {
        net.last_send = now;
    }

    let net = &mut *net;
    let mut next_session: Option<(Session, String, bool)> = None;

    match &mut net.session {
        Session::Offline => {}

        Session::Hosting { socket, world, clients, next_id } => {
            for (addr, msg) in recv_all(socket) {
                match msg {
                    Msg::Discover { magic } if magic == MAGIC => {
                        send(socket, addr, &Msg::HostInfo {
                            magic: MAGIC.into(),
                            host: my_name.clone(),
                            players: clients.len() + 1,
                            game: GAME_VERSION.into(),
                        });
                    }
                    Msg::Hello { magic, proto, game, world: their_world, .. } if magic == MAGIC => {
                        let clock_now = clock.secs_f64(&time);
                        if let Some(slot) = clients.get(&addr) {
                            send(socket, addr, &Msg::Welcome { id: slot.id, clock: clock_now });
                            continue;
                        }
                        let reject = if proto != PROTOCOL || game != GAME_VERSION {
                            Some(format!(
                                "Versions differentes : l'hote a la v{GAME_VERSION}, vous avez la v{game}. Mettez le jeu a jour."
                            ))
                        } else if their_world != *world {
                            Some("Votre carte est differente de celle de l'hote (modifiee dans les options ?).".into())
                        } else if clients.len() + 1 >= MAX_PLAYERS {
                            Some(format!("Partie pleine ({MAX_PLAYERS} joueurs max)."))
                        } else {
                            None
                        };
                        match reject {
                            Some(reason) => send(socket, addr, &Msg::Reject { reason }),
                            None => {
                                let id = *next_id;
                                *next_id += 1;
                                clients.insert(addr, ClientSlot { id, last_seen: now });
                                send(socket, addr, &Msg::Welcome { id, clock: clock_now });
                            }
                        }
                    }
                    Msg::State { name, color, pos, rot } => {
                        if let Some(slot) = clients.get_mut(&addr) {
                            slot.last_seen = now;
                            update_peer(&mut net.peers, PlayerState { id: slot.id, name, color, pos, rot }, now);
                        }
                    }
                    Msg::Bye => {
                        if let Some(slot) = clients.remove(&addr) {
                            net.peers.remove(&slot.id);
                        }
                    }
                    _ => {}
                }
            }

            clients.retain(|_, slot| {
                let alive = now - slot.last_seen < TIMEOUT;
                if !alive {
                    net.peers.remove(&slot.id);
                }
                alive
            });

            if tick && !clients.is_empty() {
                let mut players = vec![PlayerState {
                    id: HOST_ID,
                    name: my_name.clone(),
                    color: my_color,
                    pos: pos.to_array(),
                    rot: rot.to_array(),
                }];
                for slot in clients.values() {
                    if let Some(p) = net.peers.get(&slot.id) {
                        players.push(PlayerState {
                            id: slot.id,
                            name: p.name.clone(),
                            color: p.color,
                            pos: p.pos.to_array(),
                            rot: p.rot.to_array(),
                        });
                    }
                }
                let snapshot = Msg::Snapshot { clock: clock.secs_f64(&time), players };
                for addr in clients.keys() {
                    send(socket, *addr, &snapshot);
                }
            }
        }

        Session::Joining { socket, host, world, started, last_hello } => {
            if now - *last_hello >= JOIN_RETRY {
                *last_hello = now;
                send(socket, *host, &Msg::Hello {
                    magic: MAGIC.into(),
                    proto: PROTOCOL,
                    game: GAME_VERSION.into(),
                    world: *world,
                    name: my_name.clone(),
                    color: my_color,
                });
            }
            for (addr, msg) in recv_all(socket) {
                if addr != *host {
                    continue;
                }
                match msg {
                    Msg::Welcome { id, clock: host_clock } => {
                        clock.offset = host_clock - now;
                        let socket = socket.try_clone();
                        if let Ok(socket) = socket {
                            next_session = Some((
                                Session::Connected { socket, host: *host, my_id: id, last_recv: now },
                                format!("Connecte a {host}"),
                                false,
                            ));
                        }
                        break;
                    }
                    Msg::Reject { reason } => {
                        next_session = Some((Session::Offline, format!("Refuse : {reason}"), true));
                        break;
                    }
                    _ => {}
                }
            }
            if next_session.is_none() && now - *started > JOIN_TIMEOUT {
                next_session = Some((
                    Session::Offline,
                    format!(
                        "Aucune reponse de {host}. Verifiez l'adresse et que l'hote a bien ouvert la partie (port UDP {NET_PORT})."
                    ),
                    true,
                ));
            }
        }

        Session::Connected { socket, host, my_id, last_recv } => {
            for (addr, msg) in recv_all(socket) {
                if addr != *host {
                    continue;
                }
                match msg {
                    Msg::Snapshot { clock: host_clock, players } => {
                        *last_recv = now;
                        // Resynchronise l'horloge seulement en cas de dérive notable.
                        if (host_clock - clock.secs_f64(&time)).abs() > 0.5 {
                            clock.offset = host_clock - now;
                        }
                        let ids: Vec<u32> = players.iter().map(|p| p.id).filter(|id| id != my_id).collect();
                        net.peers.retain(|id, _| ids.contains(id));
                        for st in players.into_iter().take(MAX_PLAYERS) {
                            if st.id != *my_id {
                                update_peer(&mut net.peers, st, now);
                            }
                        }
                    }
                    Msg::Welcome { .. } => *last_recv = now,
                    Msg::Bye => {
                        next_session = Some((Session::Offline, "L'hote a ferme la partie.".into(), true));
                        break;
                    }
                    Msg::Reject { reason } => {
                        next_session = Some((Session::Offline, format!("Deconnecte : {reason}"), true));
                        break;
                    }
                    _ => {}
                }
            }
            if next_session.is_none() {
                if now - *last_recv > TIMEOUT {
                    next_session = Some((Session::Offline, "Connexion perdue avec l'hote.".into(), true));
                } else if tick {
                    send(socket, *host, &Msg::State {
                        name: my_name.clone(),
                        color: my_color,
                        pos: pos.to_array(),
                        rot: rot.to_array(),
                    });
                }
            }
        }
    }

    if let Some((session, status, error)) = next_session {
        if matches!(session, Session::Offline) {
            net.peers.clear();
        }
        net.session = session;
        net.set_status(status, error);
    }
}

/// Recherche des parties sur le réseau local par broadcast UDP.
fn discovery(net: &mut Net, now: f64) {
    let active = net.discovering && net.mode() != NetMode::Hosting;
    if !active {
        net.discover_socket = None;
        net.lan_games.clear();
        return;
    }
    if net.discover_socket.is_none() {
        if let Ok(socket) = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)) {
            let _ = socket.set_nonblocking(true);
            let _ = socket.set_broadcast(true);
            net.discover_socket = Some(socket);
            net.last_discover = f64::NEG_INFINITY;
        }
    }
    let Some(socket) = &net.discover_socket else { return };

    if now - net.last_discover >= DISCOVER_INTERVAL {
        net.last_discover = now;
        let probe = Msg::Discover { magic: MAGIC.into() };
        send(socket, (Ipv4Addr::BROADCAST, NET_PORT).into(), &probe);
        send(socket, (Ipv4Addr::LOCALHOST, NET_PORT).into(), &probe);
    }

    for (addr, msg) in recv_all(socket) {
        if let Msg::HostInfo { magic, host, players, game } = msg {
            if magic != MAGIC {
                continue;
            }
            let host = sanitize_name(&host);
            let compatible = game == GAME_VERSION;
            // Une partie sur ce PC répond à la fois via 127.0.0.1 et via l'IP locale.
            let duplicate_of_local = |a: &SocketAddr| a.ip().is_loopback() || Some(a.ip()) == local_ip();
            if let Some(g) = net.lan_games.iter_mut().find(|g| {
                g.addr == addr || (g.host == host && duplicate_of_local(&g.addr) && duplicate_of_local(&addr))
            }) {
                if !addr.ip().is_loopback() {
                    g.addr = addr;
                }
                g.host = host;
                g.players = players;
                g.compatible = compatible;
                g.seen = now;
            } else {
                net.lan_games.push(LanGame { addr, host, players, compatible, seen: now });
            }
        }
    }
    net.lan_games.retain(|g| now - g.seen < LAN_EXPIRY);
}

fn send_bye_on_exit(mut exit: EventReader<AppExit>, mut net: ResMut<Net>) {
    if exit.read().next().is_some() {
        net.leave();
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  Affichage des autres joueurs
// ─────────────────────────────────────────────────────────────────────────

#[derive(Component)]
struct RemoteShip {
    id: u32,
    color: [f32; 3],
    inner: Handle<StandardMaterial>,
    outer: Handle<StandardMaterial>,
}

#[derive(Component)]
struct RemoteLabel(u32);

#[derive(Component)]
struct RemoteLabelText;

fn label_color(c: [f32; 3]) -> Color {
    Color::srgb(0.35 + c[0] * 0.65, 0.35 + c[1] * 0.65, 0.35 + c[2] * 0.65)
}

fn sync_remote_ships(
    mut commands: Commands,
    time: Res<Time>,
    net: Res<Net>,
    assets: Option<Res<ShipAssets>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    cam_q: Query<&GlobalTransform, With<CameraController>>,
    mut ships: Query<(Entity, &mut RemoteShip, &mut Transform)>,
    labels: Query<(Entity, &RemoteLabel)>,
) {
    let Some(assets) = assets else { return };
    let now = time.elapsed_secs_f64();
    let dt = time.delta_secs();
    let cam_pos = cam_q.get_single().map(|gt| gt.translation()).unwrap_or_default();

    // Disparus
    for (e, rs, _) in &ships {
        if !net.peers.contains_key(&rs.id) {
            commands.entity(e).despawn_recursive();
        }
    }
    for (e, label) in &labels {
        if !net.peers.contains_key(&label.0) {
            commands.entity(e).despawn_recursive();
        }
    }

    // Mise à jour des existants
    let mut present = Vec::new();
    for (_, mut rs, mut tf) in &mut ships {
        let Some(peer) = net.peers.get(&rs.id) else { continue };
        present.push(rs.id);

        // Prédiction courte (≤ 0,25 s) + lissage exponentiel
        let age = (now - peer.last_update).clamp(0.0, 0.25) as f32;
        let predicted = peer.pos + peer.vel * age;
        let gap = tf.translation.distance(predicted);
        if gap > 200_000.0 || tf.translation == Vec3::ZERO {
            tf.translation = predicted;
        } else {
            let k = 1.0 - (-15.0 * dt).exp();
            tf.translation = tf.translation.lerp(predicted, k);
        }
        tf.rotation = tf.rotation.slerp(peer.rot, (1.0 - (-12.0 * dt).exp()).clamp(0.0, 1.0));
        // Même taille apparente que notre propre vaisseau
        tf.scale = Vec3::splat((cam_pos.distance(tf.translation) * 0.008).max(0.05));

        if rs.color != peer.color {
            rs.color = peer.color;
            let (inner, outer) = aura_materials(peer.color);
            if let Some(m) = materials.get_mut(&rs.inner) { *m = inner; }
            if let Some(m) = materials.get_mut(&rs.outer) { *m = outer; }
        }
    }

    // Nouveaux joueurs
    for (id, peer) in &net.peers {
        if present.contains(id) {
            continue;
        }
        let (inner, outer) = aura_materials(peer.color);
        let inner = materials.add(inner);
        let outer = materials.add(outer);
        commands
            .spawn((
                Transform::from_translation(peer.pos).with_rotation(peer.rot),
                Visibility::default(),
                RemoteShip { id: *id, color: peer.color, inner: inner.clone(), outer: outer.clone() },
            ))
            .with_children(|p| {
                assets.spawn_model(p);
                assets.spawn_aura(p, inner, outer);
            });

        commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(240.0),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                Visibility::Hidden,
                GlobalZIndex(-1),
                RemoteLabel(*id),
            ))
            .with_child((
                Node {
                    padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.45)),
                BorderRadius::all(Val::Px(4.0)),
                Text::new(peer.name.clone()),
                TextFont { font_size: 15.0, ..default() },
                TextColor(label_color(peer.color)),
                RemoteLabelText,
            ));
    }
}

/// Place le pseudo de chaque joueur au-dessus de son vaisseau, à l'écran.
fn update_remote_labels(
    net: Res<Net>,
    cam_q: Query<(&Camera, &GlobalTransform), With<CameraController>>,
    ships: Query<(&RemoteShip, &GlobalTransform)>,
    mut labels: Query<(&RemoteLabel, &mut Node, &mut Visibility, &Children)>,
    mut texts: Query<(&mut Text, &mut TextColor), With<RemoteLabelText>>,
) {
    let Ok((camera, cam_gt)) = cam_q.get_single() else { return };
    for (label, mut node, mut vis, children) in &mut labels {
        let Some(peer) = net.peers.get(&label.0) else { continue };
        let ship_pos = ships
            .iter()
            .find(|(rs, _)| rs.id == label.0)
            .map(|(_, gt)| gt.translation());
        let screen = ship_pos.and_then(|p| camera.world_to_viewport(cam_gt, p).ok());
        match screen {
            Some(sp) => {
                node.left = Val::Px(sp.x - 120.0);
                node.top = Val::Px(sp.y - 48.0);
                *vis = Visibility::Inherited;
            }
            None => *vis = Visibility::Hidden,
        }
        for child in children.iter() {
            if let Ok((mut text, mut color)) = texts.get_mut(*child) {
                if text.0 != peer.name {
                    text.0 = peer.name.clone();
                }
                let c = label_color(peer.color);
                if color.0 != c {
                    color.0 = c;
                }
            }
        }
    }
}

fn update_local_aura(
    settings: Res<GameSettings>,
    aura_q: Query<&LocalAura>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut last: Local<Option<[f32; 3]>>,
) {
    if *last == Some(settings.aura_color) {
        return;
    }
    let Ok(aura) = aura_q.get_single() else { return };
    *last = Some(settings.aura_color);
    let (inner, outer) = aura_materials(settings.aura_color);
    if let Some(m) = materials.get_mut(&aura.inner) { *m = inner; }
    if let Some(m) = materials.get_mut(&aura.outer) { *m = outer; }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_app(name: &str, color: [f32; 3], pos: Vec3) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Assets<StandardMaterial>>()
            .insert_resource(GameSettings {
                player_name: name.into(),
                aura_color: color,
                ..GameSettings::default()
            })
            .add_plugins(NetPlugin);
        app.world_mut().spawn((Transform::from_translation(pos), GlobalTransform::from_translation(pos), Ship));
        app
    }

    #[test]
    fn parse_addresses() {
        assert_eq!(parse_address("192.168.1.20"), Some(SocketAddr::from(([192, 168, 1, 20], NET_PORT))));
        assert_eq!(parse_address(" 10.0.0.1:4000 "), Some(SocketAddr::from(([10, 0, 0, 1], 4000))));
        assert_eq!(parse_address("localhost").map(|a| a.port()), Some(NET_PORT));
        assert_eq!(parse_address(""), None);
    }

    #[test]
    fn host_and_client_see_each_other() {
        let mut host = test_app("Hote", [1.0, 0.0, 0.0], Vec3::new(100.0, 0.0, 0.0));
        let mut client = test_app("Client", [0.0, 1.0, 0.0], Vec3::new(0.0, 50.0, 0.0));

        host.world_mut().send_event(NetCommand::Host);
        host.update();
        assert_eq!(host.world().resource::<Net>().mode(), NetMode::Hosting, "{}", host.world().resource::<Net>().status);

        client.world_mut().send_event(NetCommand::Join("127.0.0.1".into()));
        let start = std::time::Instant::now();
        loop {
            host.update();
            client.update();
            let ok_client = client.world().resource::<Net>().peers.values().any(|p| p.name == "Hote");
            let ok_host = host.world().resource::<Net>().peers.values().any(|p| p.name == "Client");
            if ok_client && ok_host {
                break;
            }
            assert!(start.elapsed().as_secs() < 5, "timeout: {}", client.world().resource::<Net>().status);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let cnet = client.world().resource::<Net>();
        assert_eq!(cnet.mode(), NetMode::Connected);
        let hote = cnet.peers.values().find(|p| p.name == "Hote").unwrap();
        assert_eq!(hote.color, [1.0, 0.0, 0.0]);
        assert!(hote.pos.distance(Vec3::new(100.0, 0.0, 0.0)) < 0.01);

        // Le client quitte : l'hôte le retire immédiatement
        client.world_mut().send_event(NetCommand::Leave);
        client.update();
        for _ in 0..20 {
            host.update();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(host.world().resource::<Net>().peers.is_empty());
    }
}
