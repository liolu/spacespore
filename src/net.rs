// ─────────────────────────────────────────────────────────────────────────
//  Multijoueur
//
//  Réseau UDP direct, sans aucun service externe (pas de Steam, Hamachi…).
//  Tout est automatique :
//   - au lancement, chaque jeu ouvre une partie (port UDP NET_PORT) ;
//   - les jeux d'un même réseau local se trouvent par broadcast et se
//     regroupent tout seuls dans une seule partie ;
//   - pour Internet, la box est ouverte automatiquement (UPnP) et l'hôte
//     reçoit un petit code d'invitation (son adresse encodée) à donner
//     à ses amis.
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
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::settings::GameSettings;
use crate::ship::{aura_materials, LocalAura, Ship, ShipAssets};
use crate::CameraController;

pub const NET_PORT: u16 = 27777;
pub const MAX_NAME_LEN: usize = 16;
const PROTOCOL: u32 = 2;
const MAGIC: &str = "SPACESPORE";
const GAME_VERSION: &str = env!("CARGO_PKG_VERSION");
const MAX_PLAYERS: usize = 16;
const SEND_INTERVAL: f64 = 0.05;
const TIMEOUT: f64 = 6.0;
const JOIN_RETRY: f64 = 0.5;
const JOIN_TIMEOUT: f64 = 6.0;
const DISCOVER_INTERVAL: f64 = 1.5;
const LAN_EXPIRY: f64 = 5.0;
const HOST_RETRY: f64 = 3.0;
const AVOID_DURATION: f64 = 30.0;
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
            .add_systems(Last, cleanup_on_exit);
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
    HostInfo { magic: String, sid: u64, players: usize, game: String, world: u64 },
    Hello { magic: String, proto: u32, game: String, world: u64, name: String, color: [f32; 3] },
    Welcome { id: u32, clock: f64 },
    Reject { reason: String },
    State { name: String, color: [f32; 3], pos: [f32; 3], rot: [f32; 4] },
    Snapshot { clock: f64, code: Option<String>, players: Vec<PlayerState> },
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
fn local_ip() -> Option<IpAddr> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    socket.connect((Ipv4Addr::new(8, 8, 8, 8), 80)).ok()?;
    let ip = socket.local_addr().ok()?.ip();
    if ip.is_unspecified() { None } else { Some(ip) }
}

// ─────────────────────────────────────────────────────────────────────────
//  Code d'invitation : adresse Internet de l'hôte encodée en base 32
//  (alphabet de Crockford, sans I, L, O, U pour éviter les confusions).
//  Port standard : 7 caractères « ABC-DEFG » ; autre port : 10 caractères.
// ─────────────────────────────────────────────────────────────────────────

const CODE_ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

pub fn encode_invite(ip: Ipv4Addr, port: u16) -> String {
    let (value, len) = if port == NET_PORT {
        (u32::from(ip) as u64, 7)
    } else {
        (((u32::from(ip) as u64) << 16) | port as u64, 10)
    };
    let mut chars: Vec<u8> = (0..len)
        .map(|i| CODE_ALPHABET[((value >> (5 * (len - 1 - i))) & 31) as usize])
        .collect();
    let split = if len == 7 { 3 } else { 5 };
    chars.insert(split, b'-');
    String::from_utf8(chars).unwrap_or_default()
}

pub fn decode_invite(code: &str) -> Option<SocketAddr> {
    let mut value: u64 = 0;
    let mut len = 0;
    for c in code.chars() {
        if c == '-' || c == ' ' {
            continue;
        }
        let c = match c.to_ascii_uppercase() {
            'O' => '0',
            'I' | 'L' => '1',
            c => c,
        };
        let digit = CODE_ALPHABET.iter().position(|&a| a as char == c)? as u64;
        value = (value << 5) | digit;
        len += 1;
        if len > 10 {
            return None;
        }
    }
    match len {
        7 if value <= u32::MAX as u64 => Some(SocketAddr::new(Ipv4Addr::from(value as u32).into(), NET_PORT)),
        10 if value < (1 << 48) => {
            let port = (value & 0xFFFF) as u16;
            let ip = Ipv4Addr::from((value >> 16) as u32);
            (port != 0).then(|| SocketAddr::new(ip.into(), port))
        }
        _ => None,
    }
}

/// Code d'invitation, ou à défaut une adresse IP / un nom de machine.
fn parse_target(input: &str) -> Option<SocketAddr> {
    let input = input.trim();
    if input.is_empty() {
        return None;
    }
    if let Some(addr) = decode_invite(input) {
        return Some(addr);
    }
    if let Ok(addr) = input.parse::<SocketAddr>() {
        return Some(addr);
    }
    if let Ok(ip) = input.trim_matches(|c| c == '[' || c == ']').parse::<IpAddr>() {
        return Some(SocketAddr::new(ip, NET_PORT));
    }
    if !input.contains('.') && !input.contains(':') {
        return None; // ni code valide, ni adresse : faute de frappe probable
    }
    let resolved: Vec<SocketAddr> = if input.contains(':') {
        input.to_socket_addrs().ok()?.collect()
    } else {
        (input, NET_PORT).to_socket_addrs().ok()?.collect()
    };
    resolved.iter().find(|a| a.is_ipv4()).or_else(|| resolved.first()).copied()
}

// ─────────────────────────────────────────────────────────────────────────
//  Ouverture automatique du port sur la box (UPnP), en arrière-plan
// ─────────────────────────────────────────────────────────────────────────

enum Upnp {
    Pending,
    Ready { ip: Ipv4Addr, port: u16, gateway: igd_next::Gateway },
    Failed(String),
}

fn is_non_public(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_unspecified()
        || (o[0] == 100 && (64..128).contains(&o[1])) // CGNAT 100.64.0.0/10
}

fn add_mapping(gw: &igd_next::Gateway, port: u16, local: SocketAddr) -> Result<(), igd_next::AddPortError> {
    use igd_next::{AddPortError, PortMappingProtocol};
    match gw.add_port(PortMappingProtocol::UDP, port, local, 3600, "SpaceSpore") {
        Err(AddPortError::OnlyPermanentLeasesSupported) => {
            gw.add_port(PortMappingProtocol::UDP, port, local, 0, "SpaceSpore")
        }
        r => r,
    }
}

fn start_upnp(state: Arc<Mutex<Upnp>>) {
    std::thread::spawn(move || {
        let result = (|| -> Result<(Ipv4Addr, u16, igd_next::Gateway, SocketAddr), String> {
            let local = match local_ip() {
                Some(IpAddr::V4(ip)) => ip,
                _ => return Err("Pas de connexion reseau.".into()),
            };
            let gw = igd_next::search_gateway(igd_next::SearchOptions {
                timeout: Some(Duration::from_secs(5)),
                ..Default::default()
            })
            .map_err(|_| "Votre box n'accepte pas l'ouverture automatique (UPnP desactive).".to_string())?;
            let ext = match gw.get_external_ip() {
                Ok(IpAddr::V4(ip)) => ip,
                _ => return Err("Adresse Internet introuvable.".into()),
            };
            if is_non_public(ext) {
                return Err("Votre connexion Internet est partagee (CGNAT) : vos amis ne peuvent pas vous rejoindre, mais vous pouvez rejoindre les leurs.".into());
            }
            let local_addr = SocketAddr::new(IpAddr::V4(local), NET_PORT);
            // Si un autre PC de la maison utilise déjà le port, on en prend un voisin.
            for port in NET_PORT..NET_PORT + 10 {
                if add_mapping(&gw, port, local_addr).is_ok() {
                    return Ok((ext, port, gw, local_addr));
                }
            }
            Err("La box refuse d'ouvrir le port.".into())
        })();

        match result {
            Ok((ip, port, gateway, local_addr)) => {
                if let Ok(mut s) = state.lock() {
                    *s = Upnp::Ready { ip, port, gateway: gateway.clone() };
                }
                // Renouvelle l'ouverture tant que le jeu tourne
                loop {
                    std::thread::sleep(Duration::from_secs(30 * 60));
                    let _ = add_mapping(&gateway, port, local_addr);
                }
            }
            Err(reason) => {
                if let Ok(mut s) = state.lock() {
                    *s = Upnp::Failed(reason);
                }
            }
        }
    });
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
        clients: HashMap<SocketAddr, ClientSlot>,
        next_id: u32,
    },
    Joining {
        socket: UdpSocket,
        host: SocketAddr,
        started: f64,
        last_hello: f64,
        manual: bool,
    },
    Connected {
        socket: UdpSocket,
        host: SocketAddr,
        my_id: u32,
        last_recv: f64,
        manual: bool,
        host_code: Option<String>,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NetMode {
    Offline,
    Hosting,
    Joining,
    Connected,
}

/// État du code d'invitation affiché au joueur.
pub enum Invite {
    Pending,
    Ready(String),
    Unavailable(String),
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

/// Partie trouvée sur le réseau local.
#[derive(Clone)]
struct LanHost {
    addr: SocketAddr,
    sid: u64,
    players: usize,
    game: String,
    world: u64,
    seen: f64,
}

#[derive(Resource)]
pub struct Net {
    session: Session,
    /// Dernier message à afficher (erreur de connexion, départ de l'hôte…).
    pub notice: String,
    pub notice_is_error: bool,
    pub peers: HashMap<u32, Peer>,
    /// Identifiant aléatoire de cette instance du jeu.
    sid: u64,
    world: Option<u64>,
    lan: Vec<LanHost>,
    avoid: HashMap<SocketAddr, f64>,
    discover_socket: Option<UdpSocket>,
    last_discover: f64,
    next_host_try: f64,
    last_send: f64,
    upnp: Arc<Mutex<Upnp>>,
    upnp_started: bool,
}

impl Default for Net {
    fn default() -> Self {
        Self {
            session: Session::Offline,
            notice: String::new(),
            notice_is_error: false,
            peers: HashMap::new(),
            sid: rand::random::<u64>(),
            world: None,
            lan: Vec::new(),
            avoid: HashMap::new(),
            discover_socket: None,
            last_discover: f64::NEG_INFINITY,
            next_host_try: 0.0,
            last_send: f64::NEG_INFINITY,
            upnp: Arc::new(Mutex::new(Upnp::Pending)),
            upnp_started: false,
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

    /// Nombre de joueurs dans la partie, soi compris.
    pub fn player_count(&self) -> usize {
        self.peers.len() + 1
    }

    /// Vrai si on a rejoint un ami avec son code (pas le regroupement automatique).
    pub fn joined_by_code(&self) -> bool {
        matches!(
            self.session,
            Session::Joining { manual: true, .. } | Session::Connected { manual: true, .. }
        )
    }

    /// Code à donner à un ami pour qu'il rejoigne la partie en cours.
    pub fn invite(&self) -> Invite {
        match &self.session {
            Session::Connected { host_code: Some(code), .. } => Invite::Ready(code.clone()),
            Session::Connected { .. } => {
                Invite::Unavailable("La box de l'hote n'accepte pas les joueurs venant d'Internet.".into())
            }
            Session::Hosting { .. } => match self.upnp.lock().as_deref() {
                Ok(Upnp::Ready { ip, port, .. }) => Invite::Ready(encode_invite(*ip, *port)),
                Ok(Upnp::Failed(reason)) => Invite::Unavailable(reason.clone()),
                _ => Invite::Pending,
            },
            _ => Invite::Pending,
        }
    }

    fn own_code(&self) -> Option<String> {
        match self.upnp.lock().as_deref() {
            Ok(Upnp::Ready { ip, port, .. }) => Some(encode_invite(*ip, *port)),
            _ => None,
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

    fn set_notice(&mut self, text: impl Into<String>, error: bool) {
        self.notice = text.into();
        self.notice_is_error = error;
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

    fn try_host(&mut self, now: f64) {
        match UdpSocket::bind((Ipv4Addr::UNSPECIFIED, NET_PORT)) {
            Ok(socket) => {
                let _ = socket.set_nonblocking(true);
                self.session = Session::Hosting { socket, clients: HashMap::new(), next_id: 1 };
            }
            // Port déjà pris (ex. deuxième jeu sur ce PC) : on rejoindra via la recherche locale.
            Err(_) => self.next_host_try = now + HOST_RETRY,
        }
    }

    fn start_join(&mut self, host: SocketAddr, manual: bool, now: f64) -> bool {
        self.leave();
        let bind: SocketAddr = if host.is_ipv4() {
            (Ipv4Addr::UNSPECIFIED, 0).into()
        } else {
            (std::net::Ipv6Addr::UNSPECIFIED, 0).into()
        };
        match UdpSocket::bind(bind) {
            Ok(socket) => {
                let _ = socket.set_nonblocking(true);
                self.session = Session::Joining {
                    socket,
                    host,
                    started: now,
                    last_hello: f64::NEG_INFINITY,
                    manual,
                };
                true
            }
            Err(_) => false,
        }
    }
}

/// Choisit la partie locale à rejoindre automatiquement.
/// Hors ligne : la plus peuplée. En hébergeant seul : seulement une partie
/// déjà peuplée, ou à égalité celle de plus petit identifiant (les deux jeux
/// font le même choix, donc ils finissent toujours regroupés).
fn pick_lan_host(lan: &[LanHost], my_sid: u64, hosting_alone: bool, world: u64, avoid: &HashMap<SocketAddr, f64>, now: f64) -> Option<SocketAddr> {
    lan.iter()
        .filter(|h| h.sid != my_sid && h.game == GAME_VERSION && h.world == world && h.players < MAX_PLAYERS)
        .filter(|h| avoid.get(&h.addr).map_or(true, |until| now >= *until))
        .filter(|h| !hosting_alone || h.players > 1 || h.sid < my_sid)
        .max_by(|a, b| a.players.cmp(&b.players).then(b.sid.cmp(&a.sid)))
        .map(|h| h.addr)
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
    /// Rejoindre un ami avec son code d'invitation.
    JoinCode(String),
    /// Quitter la partie de l'ami et revenir au mode automatique.
    Leave,
}

fn handle_net_commands(
    mut events: EventReader<NetCommand>,
    mut net: ResMut<Net>,
    time: Res<Time>,
) {
    for ev in events.read() {
        let now = time.elapsed_secs_f64();
        match ev {
            NetCommand::JoinCode(code) => {
                let Some(host) = parse_target(code) else {
                    net.set_notice(format!("Code invalide : \"{}\"", code.trim()), true);
                    continue;
                };
                if net.start_join(host, true, now) {
                    net.set_notice("Connexion a votre ami...", false);
                } else {
                    net.set_notice("Erreur reseau.", true);
                }
            }
            NetCommand::Leave => {
                net.leave();
                net.next_host_try = now;
                net.set_notice("", false);
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

    let net = &mut *net;
    let world = *net.world.get_or_insert_with(|| world_fingerprint(&settings));
    if !net.upnp_started {
        net.upnp_started = true;
        start_upnp(net.upnp.clone());
    }

    // ── Mode automatique : toujours dans une partie ─────────────────────
    if net.mode() == NetMode::Offline && now >= net.next_host_try {
        net.try_host(now);
    }
    if let Some(target) = discovery(net, now, world) {
        net.start_join(target, false, now);
    }

    let tick = now - net.last_send >= SEND_INTERVAL;
    if tick {
        net.last_send = now;
    }

    let own_code = net.own_code();
    let sid = net.sid;
    // (nouvelle session, message, erreur ?, adresse à éviter)
    let mut next: Option<(Session, String, bool, Option<SocketAddr>)> = None;

    match &mut net.session {
        Session::Offline => {}

        Session::Hosting { socket, clients, next_id } => {
            for (addr, msg) in recv_all(socket) {
                match msg {
                    Msg::Discover { magic } if magic == MAGIC => {
                        send(socket, addr, &Msg::HostInfo {
                            magic: MAGIC.into(),
                            sid,
                            players: clients.len() + 1,
                            game: GAME_VERSION.into(),
                            world,
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
                                "Versions differentes : votre ami a la v{GAME_VERSION}, vous avez la v{game}. Mettez le jeu a jour."
                            ))
                        } else if their_world != world {
                            Some("Votre carte est differente de celle de votre ami (modifiee dans les options ?).".into())
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
                let snapshot = Msg::Snapshot { clock: clock.secs_f64(&time), code: own_code, players };
                for addr in clients.keys() {
                    send(socket, *addr, &snapshot);
                }
            }
        }

        Session::Joining { socket, host, started, last_hello, manual } => {
            if now - *last_hello >= JOIN_RETRY {
                *last_hello = now;
                send(socket, *host, &Msg::Hello {
                    magic: MAGIC.into(),
                    proto: PROTOCOL,
                    game: GAME_VERSION.into(),
                    world,
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
                        if let Ok(socket) = socket.try_clone() {
                            let text = if *manual { "Vous avez rejoint votre ami !" } else { "" };
                            next = Some((
                                Session::Connected {
                                    socket,
                                    host: *host,
                                    my_id: id,
                                    last_recv: now,
                                    manual: *manual,
                                    host_code: None,
                                },
                                text.into(),
                                false,
                                None,
                            ));
                        }
                        break;
                    }
                    Msg::Reject { reason } => {
                        let text = if *manual { format!("Refuse : {reason}") } else { String::new() };
                        next = Some((Session::Offline, text, *manual, Some(*host)));
                        break;
                    }
                    _ => {}
                }
            }
            if next.is_none() && now - *started > JOIN_TIMEOUT {
                let text = if *manual {
                    "Aucune reponse de votre ami. Verifiez le code et que son jeu est bien lance.".into()
                } else {
                    String::new()
                };
                next = Some((Session::Offline, text, *manual, Some(*host)));
            }
        }

        Session::Connected { socket, host, my_id, last_recv, host_code, .. } => {
            for (addr, msg) in recv_all(socket) {
                if addr != *host {
                    continue;
                }
                match msg {
                    Msg::Snapshot { clock: host_clock, code, players } => {
                        *last_recv = now;
                        *host_code = code.filter(|c| decode_invite(c).is_some());
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
                        next = Some((Session::Offline, "L'hote a quitte la partie.".into(), false, None));
                        break;
                    }
                    Msg::Reject { reason } => {
                        next = Some((Session::Offline, format!("Deconnecte : {reason}"), true, None));
                        break;
                    }
                    _ => {}
                }
            }
            if next.is_none() {
                if now - *last_recv > TIMEOUT {
                    next = Some((Session::Offline, "Connexion perdue avec l'hote.".into(), true, None));
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

    if let Some((session, text, error, avoid)) = next {
        let offline = matches!(session, Session::Offline);
        net.session = session;
        if offline {
            net.peers.clear();
            net.next_host_try = now; // on rouvre aussitôt sa propre partie
        }
        if let Some(addr) = avoid {
            net.avoid.insert(addr, now + AVOID_DURATION);
        }
        if !text.is_empty() || !error {
            net.set_notice(text, error);
        }
    }
}

/// Recherche des parties sur le réseau local par broadcast UDP.
/// Renvoie la partie à rejoindre automatiquement, s'il y en a une.
fn discovery(net: &mut Net, now: f64, world: u64) -> Option<SocketAddr> {
    let active = match &net.session {
        Session::Offline => true,
        Session::Hosting { clients, .. } => clients.is_empty(),
        _ => false,
    };
    if !active {
        net.discover_socket = None;
        net.lan.clear();
        return None;
    }
    if net.discover_socket.is_none() {
        if let Ok(socket) = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)) {
            let _ = socket.set_nonblocking(true);
            let _ = socket.set_broadcast(true);
            net.discover_socket = Some(socket);
            net.last_discover = f64::NEG_INFINITY;
        }
    }
    let socket = net.discover_socket.as_ref()?;

    if now - net.last_discover >= DISCOVER_INTERVAL {
        net.last_discover = now;
        let probe = Msg::Discover { magic: MAGIC.into() };
        send(socket, (Ipv4Addr::BROADCAST, NET_PORT).into(), &probe);
        // Un autre jeu lancé sur ce même PC
        send(socket, (Ipv4Addr::LOCALHOST, NET_PORT).into(), &probe);
    }

    for (addr, msg) in recv_all(socket) {
        if let Msg::HostInfo { magic, sid, players, game, world } = msg {
            if magic != MAGIC || sid == net.sid {
                continue;
            }
            // Un même jeu répond parfois via 127.0.0.1 et via son IP : on garde une entrée par jeu.
            if let Some(h) = net.lan.iter_mut().find(|h| h.sid == sid) {
                if !addr.ip().is_loopback() || h.addr.ip().is_loopback() {
                    h.addr = addr;
                }
                h.players = players;
                h.game = game;
                h.world = world;
                h.seen = now;
            } else {
                net.lan.push(LanHost { addr, sid, players, game, world, seen: now });
            }
        }
    }
    net.lan.retain(|h| now - h.seen < LAN_EXPIRY);
    net.avoid.retain(|_, until| now < *until);

    let hosting_alone = net.mode() == NetMode::Hosting;
    pick_lan_host(&net.lan, net.sid, hosting_alone, world, &net.avoid, now)
}

/// À la fermeture du jeu : prévient les autres joueurs et referme le port de la box.
fn cleanup_on_exit(mut exit: EventReader<AppExit>, mut net: ResMut<Net>) {
    if exit.read().next().is_none() {
        return;
    }
    net.leave();
    if let Ok(Upnp::Ready { port, gateway, .. }) = net.upnp.lock().as_deref() {
        let _ = gateway.remove_port(igd_next::PortMappingProtocol::UDP, *port);
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
        // Pas de recherche UPnP pendant les tests
        app.world_mut().resource_mut::<Net>().upnp_started = true;
        app.world_mut().spawn((Transform::from_translation(pos), GlobalTransform::from_translation(pos), Ship));
        app
    }

    #[test]
    fn invite_codes_roundtrip() {
        let ip = Ipv4Addr::new(86, 245, 12, 201);
        let code = encode_invite(ip, NET_PORT);
        assert_eq!(code.len(), 8, "{code}");
        assert_eq!(decode_invite(&code), Some(SocketAddr::new(ip.into(), NET_PORT)));
        assert_eq!(decode_invite(&code.to_lowercase().replace('-', " ")), Some(SocketAddr::new(ip.into(), NET_PORT)));
        let code = encode_invite(ip, 27779);
        assert_eq!(code.len(), 11, "{code}");
        assert_eq!(decode_invite(&code), Some(SocketAddr::new(ip.into(), 27779)));
        assert_eq!(decode_invite("ABC"), None);
        assert_eq!(parse_target("192.168.1.20"), Some(SocketAddr::from(([192, 168, 1, 20], NET_PORT))));
        assert_eq!(parse_target("n'importe quoi"), None);
    }

    #[test]
    fn two_lone_hosts_pick_the_same_winner() {
        let a = LanHost { addr: ([10, 0, 0, 1], NET_PORT).into(), sid: 5, players: 1, game: GAME_VERSION.into(), world: 1, seen: 0.0 };
        let b = LanHost { addr: ([10, 0, 0, 2], NET_PORT).into(), sid: 9, players: 1, game: GAME_VERSION.into(), world: 1, seen: 0.0 };
        let avoid = HashMap::new();
        // b voit a (sid plus petit) : b rejoint a. a voit b : a reste hôte.
        assert_eq!(pick_lan_host(&[a.clone()], 9, true, 1, &avoid, 0.0), Some(a.addr));
        assert_eq!(pick_lan_host(&[b.clone()], 5, true, 1, &avoid, 0.0), None);
        // Une partie déjà peuplée attire toujours
        let big = LanHost { players: 3, sid: 99, ..b.clone() };
        assert_eq!(pick_lan_host(&[big.clone()], 5, true, 1, &avoid, 0.0), Some(big.addr));
        // Carte différente : ignorée
        assert_eq!(pick_lan_host(&[big], 5, true, 2, &avoid, 0.0), None);
    }

    #[test]
    fn second_game_on_same_pc_joins_automatically() {
        let mut host = test_app("Hote", [1.0, 0.0, 0.0], Vec3::new(100.0, 0.0, 0.0));
        host.update();
        assert_eq!(host.world().resource::<Net>().mode(), NetMode::Hosting);

        // Le port est pris : ce jeu trouve l'autre via la recherche locale et le rejoint seul.
        let mut client = test_app("Client", [0.0, 1.0, 0.0], Vec3::new(0.0, 50.0, 0.0));
        let start = std::time::Instant::now();
        loop {
            host.update();
            client.update();
            let ok_client = client.world().resource::<Net>().peers.values().any(|p| p.name == "Hote");
            let ok_host = host.world().resource::<Net>().peers.values().any(|p| p.name == "Client");
            if ok_client && ok_host {
                break;
            }
            assert!(start.elapsed().as_secs() < 8, "timeout");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let cnet = client.world().resource::<Net>();
        assert_eq!(cnet.mode(), NetMode::Connected);
        assert_eq!(cnet.player_count(), 2);
        let hote = cnet.peers.values().find(|p| p.name == "Hote").unwrap();
        assert_eq!(hote.color, [1.0, 0.0, 0.0]);
        assert!(hote.pos.distance(Vec3::new(100.0, 0.0, 0.0)) < 0.01);

        // L'hôte ferme : le client revient à sa propre partie (ou hors ligne si le port est encore pris)
        host.world_mut().resource_mut::<Net>().leave();
        drop(host);
        for _ in 0..20 {
            client.update();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let cnet = client.world().resource::<Net>();
        assert!(cnet.peers.is_empty());
        assert_ne!(cnet.mode(), NetMode::Connected);
    }
}
