// ─────────────────────────────────────────────────────────────────────────
//  Guildes
//
//  Tout joueur peut créer une guilde (nom, tag, emblème, couleur) : il en
//  devient le Chef. Rangs, du plus haut au plus bas :
//   - Chef      : tous les droits ; transfert du rôle de Chef ; dissolution.
//   - Sous-chef : gère membres, rôles et relations, mais pas les autres
//                 Sous-chefs ni le Chef ; ne peut pas dissoudre.
//   - Officier  : ajoute / exclut des membres, accepte ou refuse les
//                 demandes ; ne change aucun rôle.
//   - Membre    : peut seulement demander une promotion.
//  On ne modifie jamais que des joueurs d'un rang strictement inférieur au
//  sien, et on ne donne jamais un rang égal ou supérieur au sien.
//
//  Il n'y a pas de serveur : la guilde est une fiche (`GuildRecord`) que
//  chaque membre garde dans ses réglages et annonce régulièrement. Chaque
//  modification augmente son numéro de révision ; la fiche la plus récente
//  l'emporte partout. Les règles de droits sont vérifiées par le jeu de
//  celui qui agit.
// ─────────────────────────────────────────────────────────────────────────

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::diplomacy::Relation;
use crate::net::{sanitize_tag, Net, MAX_NAME_LEN, MAX_RELATIONS};
use crate::settings::GameSettings;

pub const MAX_GUILD_NAME: usize = 24;
pub const MAX_MEMBERS: usize = 20;
pub const EMBLEM_COUNT: u8 = 8;
const MAX_REFUSED: usize = 8;
/// Fiches gardées après un départ ou une dissolution, pour corriger les
/// joueurs qui auraient encore une vieille version.
const ARCHIVE_MAX: usize = 4;
/// Le cache sur disque est réécrit au plus à cet intervalle (secondes).
const CACHE_SAVE_SECS: f64 = 5.0;
const CACHE_MAX_GUILDS: usize = 32;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Role {
    Member,
    Officer,
    Deputy,
    Chief,
}

impl Role {
    pub fn label(self) -> &'static str {
        match self {
            Role::Member => "Membre",
            Role::Officer => "Officier",
            Role::Deputy => "Sous-chef",
            Role::Chief => "Chef",
        }
    }

    /// Rang juste au-dessus (sans aller jusqu'à Chef : ce rôle se transfère).
    pub fn up(self) -> Option<Role> {
        match self {
            Role::Member => Some(Role::Officer),
            Role::Officer => Some(Role::Deputy),
            _ => None,
        }
    }

    pub fn down(self) -> Option<Role> {
        match self {
            Role::Deputy => Some(Role::Officer),
            Role::Officer => Some(Role::Member),
            _ => None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Member {
    pub id: u64,
    pub name: String,
    pub role: Role,
    #[serde(default)]
    pub wants_promotion: bool,
}

/// Position de la guilde envers une autre faction (guilde ou joueur seul).
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct GuildRelation {
    /// Voir `diplomacy::faction_key`.
    pub key: String,
    /// Nom affiché de la faction (elle peut être hors ligne).
    pub label: String,
    pub relation: Relation,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct GuildRecord {
    pub id: u64,
    /// Numéro de révision : augmente à chaque modification.
    pub rev: u64,
    pub name: String,
    pub tag: String,
    pub emblem: u8,
    pub color: [f32; 3],
    pub members: Vec<Member>,
    /// Joueurs dont la demande a été refusée récemment.
    #[serde(default)]
    pub refused: Vec<u64>,
    #[serde(default)]
    pub relations: Vec<GuildRelation>,
    #[serde(default)]
    pub dissolved: bool,
}

type Check = Result<(), String>;

fn deny(text: &str) -> Check {
    Err(text.to_string())
}

impl GuildRecord {
    pub fn create(name: &str, tag: &str, emblem: u8, color: [f32; 3], founder: u64, founder_name: &str) -> Result<Self, String> {
        let name: String = name.trim().chars().filter(|c| !c.is_control()).take(MAX_GUILD_NAME).collect();
        let tag = sanitize_tag(tag);
        if name.is_empty() {
            return Err("Donnez un nom a la guilde.".into());
        }
        if tag.is_empty() {
            return Err("Donnez un tag a la guilde (1 a 5 lettres ou chiffres).".into());
        }
        Ok(Self {
            id: rand::random::<u64>() | 1,
            rev: 1,
            name,
            tag,
            emblem: emblem % EMBLEM_COUNT,
            color,
            members: vec![Member { id: founder, name: founder_name.to_string(), role: Role::Chief, wants_promotion: false }],
            refused: Vec::new(),
            relations: Vec::new(),
            dissolved: false,
        })
    }

    pub fn member(&self, pid: u64) -> Option<&Member> {
        self.members.iter().find(|m| m.id == pid)
    }

    pub fn role_of(&self, pid: u64) -> Option<Role> {
        self.member(pid).map(|m| m.role)
    }

    pub fn has_member(&self, pid: u64) -> bool {
        self.member(pid).is_some()
    }

    fn member_mut(&mut self, pid: u64) -> Option<&mut Member> {
        self.members.iter_mut().find(|m| m.id == pid)
    }

    /// Rang de celui qui agit, s'il a au moins le rang demandé.
    fn require(&self, actor: u64, min: Role, error: &str) -> Result<Role, String> {
        match self.role_of(actor) {
            Some(role) if role >= min => Ok(role),
            _ => Err(error.to_string()),
        }
    }

    /// Ajoute un joueur comme Membre (demande acceptée, ou ajout direct).
    pub fn add_member(&mut self, actor: u64, pid: u64, name: &str) -> Check {
        self.require(actor, Role::Officer, "Il faut etre Officier ou plus pour ajouter des membres.")?;
        if self.has_member(pid) {
            return deny("Ce joueur est deja dans la guilde.");
        }
        if self.members.len() >= MAX_MEMBERS {
            return deny("La guilde est pleine.");
        }
        self.members.push(Member { id: pid, name: name.to_string(), role: Role::Member, wants_promotion: false });
        self.refused.retain(|r| *r != pid);
        self.rev += 1;
        Ok(())
    }

    pub fn refuse(&mut self, actor: u64, pid: u64) -> Check {
        self.require(actor, Role::Officer, "Il faut etre Officier ou plus pour refuser une demande.")?;
        self.refused.retain(|r| *r != pid);
        self.refused.push(pid);
        if self.refused.len() > MAX_REFUSED {
            self.refused.remove(0);
        }
        self.rev += 1;
        Ok(())
    }

    pub fn kick(&mut self, actor: u64, target: u64) -> Check {
        let rank = self.require(actor, Role::Officer, "Il faut etre Officier ou plus pour exclure un membre.")?;
        let Some(target_rank) = self.role_of(target) else { return deny("Ce joueur n'est pas dans la guilde.") };
        if rank <= target_rank {
            return deny("Vous ne pouvez exclure que des joueurs d'un rang inferieur au votre.");
        }
        self.members.retain(|m| m.id != target);
        self.rev += 1;
        Ok(())
    }

    /// Promotion ou rétrogradation.
    pub fn set_role(&mut self, actor: u64, target: u64, role: Role) -> Check {
        if actor == target {
            return deny("Vous ne pouvez pas changer votre propre role.");
        }
        let rank = self.require(actor, Role::Deputy, "Seuls le Chef et les Sous-chefs peuvent changer les roles.")?;
        let Some(target_rank) = self.role_of(target) else { return deny("Ce joueur n'est pas dans la guilde.") };
        if rank <= target_rank {
            return deny("Vous ne pouvez modifier que des joueurs d'un rang inferieur au votre.");
        }
        if role >= rank {
            return deny("Vous ne pouvez pas donner un rang egal ou superieur au votre.");
        }
        if let Some(m) = self.member_mut(target) {
            m.role = role;
            m.wants_promotion = false;
        }
        self.rev += 1;
        Ok(())
    }

    /// Le Chef donne son rôle à un autre membre et devient Sous-chef.
    pub fn transfer(&mut self, actor: u64, target: u64) -> Check {
        if self.role_of(actor) != Some(Role::Chief) {
            return deny("Seul le Chef peut transferer son role.");
        }
        if actor == target || !self.has_member(target) {
            return deny("Choisissez un autre membre de la guilde.");
        }
        for m in &mut self.members {
            if m.id == target {
                m.role = Role::Chief;
                m.wants_promotion = false;
            } else if m.id == actor {
                m.role = Role::Deputy;
            }
        }
        self.rev += 1;
        Ok(())
    }

    /// Départ d'un membre. Si c'était le Chef, le rôle passe à un membre du
    /// plus haut rang restant (`pick(n)` choisit parmi les n à égalité) ; sans
    /// membre restant, la guilde est dissoute. Renvoie le nom du nouveau Chef.
    pub fn leave(&mut self, pid: u64, pick: impl FnOnce(usize) -> usize) -> Result<Option<String>, String> {
        let Some(role) = self.role_of(pid) else { return Err("Vous n'etes pas dans cette guilde.".into()) };
        self.members.retain(|m| m.id != pid);
        self.rev += 1;
        if role != Role::Chief {
            return Ok(None);
        }
        let Some(top) = self.members.iter().map(|m| m.role).max() else {
            self.dissolved = true;
            self.relations.clear();
            return Ok(None);
        };
        let candidates: Vec<usize> = (0..self.members.len()).filter(|&i| self.members[i].role == top).collect();
        let heir = &mut self.members[candidates[pick(candidates.len()) % candidates.len()]];
        heir.role = Role::Chief;
        heir.wants_promotion = false;
        Ok(Some(heir.name.clone()))
    }

    pub fn dissolve(&mut self, actor: u64) -> Check {
        if self.role_of(actor) != Some(Role::Chief) {
            return deny("Seul le Chef peut dissoudre la guilde.");
        }
        self.dissolved = true;
        self.members.clear();
        self.relations.clear();
        self.refused.clear();
        self.rev += 1;
        Ok(())
    }

    pub fn ask_promotion(&mut self, pid: u64, wants: bool) -> Check {
        if self.role_of(pid) != Some(Role::Member) {
            return deny("Seuls les Membres peuvent demander une promotion.");
        }
        if let Some(m) = self.member_mut(pid) {
            m.wants_promotion = wants;
        }
        self.rev += 1;
        Ok(())
    }

    pub fn set_relation(&mut self, actor: u64, key: &str, label: &str, relation: Relation) -> Check {
        self.require(actor, Role::Deputy, "Seuls le Chef et les Sous-chefs peuvent changer les relations de la guilde.")?;
        self.relations.retain(|r| r.key != key);
        if relation != Relation::Neutral {
            self.relations.push(GuildRelation { key: key.to_string(), label: label.to_string(), relation });
            if self.relations.len() > MAX_RELATIONS * 2 {
                self.relations.remove(0);
            }
        }
        self.rev += 1;
        Ok(())
    }

    pub fn declared(&self, key: &str) -> Relation {
        self.relations.iter().find(|r| r.key == key).map_or(Relation::Neutral, |r| r.relation)
    }

    /// Un membre a changé de pseudo.
    fn rename_member(&mut self, pid: u64, name: &str) -> bool {
        match self.member_mut(pid) {
            Some(m) if m.name != name => {
                m.name = name.to_string();
                self.rev += 1;
                true
            }
            _ => false,
        }
    }

    /// Fiche reçue du réseau : tailles bornées, texte nettoyé.
    fn sanitized(mut self) -> Self {
        let clean = |s: &str, max: usize| -> String { s.chars().filter(|c| !c.is_control()).take(max).collect() };
        self.name = clean(&self.name, MAX_GUILD_NAME);
        self.tag = sanitize_tag(&self.tag);
        self.emblem %= EMBLEM_COUNT;
        self.color = self.color.map(|c| if c.is_finite() { c.clamp(0.0, 1.0) } else { 1.0 });
        self.members.truncate(MAX_MEMBERS);
        for m in &mut self.members {
            m.name = clean(&m.name, MAX_NAME_LEN);
        }
        self.refused.truncate(MAX_REFUSED);
        self.relations.truncate(MAX_RELATIONS * 2);
        for r in &mut self.relations {
            r.key = clean(&r.key, MAX_NAME_LEN + 2);
            r.label = clean(&r.label, MAX_GUILD_NAME + 8);
        }
        self
    }

    /// Cette fiche doit-elle remplacer l'autre ? La révision la plus haute
    /// gagne ; à égalité (deux modifications en même temps), un ordre
    /// arbitraire mais identique chez tous départage.
    fn newer_than(&self, other: &Self) -> bool {
        if self.rev != other.rev {
            return self.rev > other.rev;
        }
        self != other
            && serde_json::to_string(self).unwrap_or_default() > serde_json::to_string(other).unwrap_or_default()
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  Annuaire et synchronisation
// ─────────────────────────────────────────────────────────────────────────

/// Guildes connues : la mienne et celles annoncées par les joueurs connectés.
#[derive(Resource, Default)]
pub struct Guilds {
    pub known: HashMap<u64, GuildRecord>,
    /// Demande d'adhésion en cours : (guilde, sa révision au moment de la demande).
    pub request: Option<(u64, u64)>,
    primed: bool,
    /// Une fiche a changé : le cache sur disque est à réécrire.
    dirty: bool,
    last_save: f64,
}

impl Guilds {
    /// Guilde existante (non dissoute).
    pub fn active(&self, gid: u64) -> Option<&GuildRecord> {
        self.known.get(&gid).filter(|g| !g.dissolved)
    }

    pub fn by_tag(&self, tag: &str) -> Option<&GuildRecord> {
        self.known.values().find(|g| !g.dissolved && g.tag == tag)
    }
}

pub struct GuildPlugin;

impl Plugin for GuildPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Guilds>()
            .add_systems(Update, guild_sync.after(crate::net::net_update));
    }
}

pub fn my_role(settings: &GameSettings) -> Option<Role> {
    settings.guild.as_ref().and_then(|g| g.role_of(settings.player_id))
}

/// Ai-je au moins ce rang dans ma guilde ?
pub fn has_rank(settings: &GameSettings, min: Role) -> bool {
    my_role(settings).is_some_and(|r| r >= min)
}

/// Range la nouvelle version d'une fiche dont je suis (ou étais) membre.
fn store(settings: &mut GameSettings, guilds: &mut Guilds, record: GuildRecord) {
    guilds.known.insert(record.id, record.clone());
    guilds.dirty = true;
    if record.dissolved || !record.has_member(settings.player_id) {
        // Je n'en fais plus partie : la fiche est gardée pour corriger les
        // joueurs qui auraient encore l'ancienne version
        settings.guild = None;
        settings.guild_archive.retain(|g| g.id != record.id);
        settings.guild_archive.push(record);
        if settings.guild_archive.len() > ARCHIVE_MAX {
            settings.guild_archive.remove(0);
        }
    } else {
        settings.guild = Some(record);
    }
    settings.clan_tag = settings.guild.as_ref().map_or(String::new(), |g| g.tag.clone());
    settings.save();
}

/// Enregistre une modification faite par moi et l'annonce tout de suite.
fn commit(settings: &mut GameSettings, guilds: &mut Guilds, net: &mut Net, record: GuildRecord) {
    net.guild_outbox.push(record.clone());
    store(settings, guilds, record);
}

/// Applique une action sur ma guilde ; en cas de refus, le motif est affiché.
pub fn edit(
    settings: &mut GameSettings,
    guilds: &mut Guilds,
    net: &mut Net,
    now: f64,
    action: impl FnOnce(&mut GuildRecord, u64) -> Result<Option<String>, String>,
) -> bool {
    let Some(mut record) = settings.guild.clone() else {
        net.notify("Vous n'etes dans aucune guilde.", now);
        return false;
    };
    match action(&mut record, settings.player_id) {
        Ok(message) => {
            commit(settings, guilds, net, record);
            if let Some(m) = message {
                net.notify(&m, now);
            }
            true
        }
        Err(reason) => {
            net.notify(&reason, now);
            false
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn create(
    settings: &mut GameSettings,
    guilds: &mut Guilds,
    net: &mut Net,
    now: f64,
    name: &str,
    tag: &str,
    emblem: u8,
    color: [f32; 3],
) -> bool {
    if settings.guild.is_some() {
        net.notify("Quittez d'abord votre guilde actuelle.", now);
        return false;
    }
    let result = if guilds.by_tag(&sanitize_tag(tag)).is_some() {
        Err("Ce tag est deja utilise par une autre guilde.".to_string())
    } else {
        GuildRecord::create(name, tag, emblem, color, settings.player_id, &settings.player_name)
    };
    match result {
        Ok(record) => {
            net.notify(&format!("Guilde [{}] {} creee : vous en etes le Chef.", record.tag, record.name), now);
            guilds.request = None;
            commit(settings, guilds, net, record);
            true
        }
        Err(reason) => {
            net.notify(&reason, now);
            false
        }
    }
}

/// Quitte ma guilde (le rôle de Chef passe à un autre membre si besoin).
pub fn leave(settings: &mut GameSettings, guilds: &mut Guilds, net: &mut Net, now: f64) {
    edit(settings, guilds, net, now, |g, me| {
        let name = g.name.clone();
        let heir = g.leave(me, |n| rand::random::<usize>() % n.max(1))?;
        Ok(Some(match heir {
            Some(heir) => format!("Vous quittez {name}. {heir} devient Chef."),
            None if g.dissolved => format!("Vous quittez {name} : la guilde est dissoute (plus aucun membre)."),
            None => format!("Vous quittez {name}."),
        }))
    });
}

/// Fiche reçue d'un autre joueur.
fn receive(settings: &mut GameSettings, guilds: &mut Guilds, net: &mut Net, now: f64, record: GuildRecord) {
    let record = record.sanitized();
    if record.id == 0 {
        return;
    }
    if guilds.known.get(&record.id).is_some_and(|mine| !record.newer_than(mine)) {
        return;
    }
    let me = settings.player_id;
    let label = format!("[{}] {}", record.tag, record.name);

    if let Some(old) = settings.guild.as_ref().filter(|g| g.id == record.id) {
        // Nouvelle version de ma guilde
        if record.dissolved {
            net.notify(&format!("La guilde {label} a ete dissoute."), now);
        } else if !record.has_member(me) {
            net.notify(&format!("Vous ne faites plus partie de la guilde {label}."), now);
        } else if let (Some(before), Some(after)) = (old.role_of(me), record.role_of(me)) {
            if before != after {
                net.notify(&format!("Vous etes maintenant {} de la guilde {label}.", after.label()), now);
            }
        }
        store(settings, guilds, record);
        return;
    }

    if settings.guild.is_none() && !record.dissolved && record.has_member(me) {
        // Demande acceptée, ou ajout direct par un Officier
        net.notify(&format!("Vous faites maintenant partie de la guilde {label}."), now);
        guilds.request = None;
        store(settings, guilds, record);
        return;
    }

    if let Some((gid, rev)) = guilds.request {
        if gid == record.id && (record.dissolved || (record.rev > rev && record.refused.contains(&me))) {
            net.notify(&format!("Votre demande pour rejoindre {label} a ete refusee."), now);
            guilds.request = None;
        }
    }
    guilds.known.insert(record.id, record);
    guilds.dirty = true;
}

fn guild_sync(
    time: Res<Time>,
    panel: Res<crate::net_ui::NetPanel>,
    mut settings: ResMut<GameSettings>,
    mut guilds: ResMut<Guilds>,
    mut net: ResMut<Net>,
) {
    let now = time.elapsed_secs_f64();
    let settings = settings.bypass_change_detection();
    let guilds = &mut *guilds;
    let net = &mut *net;

    if !guilds.primed {
        guilds.primed = true;
        if settings.player_id == 0 {
            settings.player_id = rand::random::<u64>() | 1;
            settings.save();
        }
        for g in settings.guild.iter().chain(settings.guild_archive.iter()) {
            guilds.known.insert(g.id, g.clone());
        }
        load_cache(guilds, net);
    }

    for record in std::mem::take(&mut net.guild_inbox) {
        receive(settings, guilds, net, now, record);
    }

    // Mon pseudo a changé : la fiche suit (hors saisie en cours)
    if panel.focus.is_none() {
        if let Some(mut g) = settings.guild.clone() {
            if g.rename_member(settings.player_id, &settings.player_name) {
                commit(settings, guilds, net, g);
            }
        }
    }

    // Un autre joueur connecté a le même identifiant que moi : deux jeux qui lisent la
    // même sauvegarde (même PC, ou dossier copié). Celui dont l'identifiant réseau est
    // le plus élevé prend une identité neuve ; l'autre garde le compte.
    let my_net_id = net.my_id();
    let duplicate = net.peers.iter().any(|(&id, p)| p.status.pid == settings.player_id && settings.player_id != 0 && id < my_net_id);
    if duplicate {
        become_new_player(settings, guilds, net, now);
        return;
    }

    // Le tag affiché vient toujours de la guilde
    let tag = settings.guild.as_ref().map_or("", |g| g.tag.as_str());
    if settings.clan_tag != tag {
        settings.clan_tag = tag.to_string();
    }

    // Fiches qu'un autre joueur me demande
    for id in std::mem::take(&mut net.guild_asked) {
        if let Some(record) = guilds.known.get(&id) {
            net.guild_outbox.push(record.clone());
        }
    }

    // Identité annoncée aux autres
    net.local.pid = settings.player_id;
    if settings.guild.is_some() {
        guilds.request = None;
    }
    net.local.req = guilds.request.map_or(0, |r| r.0);

    // Les fiches ne circulent pas en continu : chaque joueur annonce seulement
    // quelle révision il a, et on ne demande que celles qui nous manquent.
    net.guild_want.clear();
    for peer in net.peers.values_mut() {
        let status = &peer.status;
        let behind = |gid: u64, rev: u64| guilds.known.get(&gid).map(|g| g.rev < rev);
        // Sa guilde : à demander si inconnue ou plus ancienne chez moi
        if status.gid != 0 && behind(status.gid, status.grev).unwrap_or(true) && !net.guild_want.contains(&status.gid) {
            net.guild_want.push(status.gid);
        }
        // Guildes qu'il a quittées ou dissoutes : seulement si je les connais en plus ancien
        for &(gid, rev) in &status.archive {
            if behind(gid, rev) == Some(true) && !net.guild_want.contains(&gid) {
                net.guild_want.push(gid);
            }
        }

        // Guilde de chaque joueur connecté : seulement si la fiche le confirme
        let record = guilds.active(status.gid).filter(|g| g.has_member(status.pid));
        let (gid, tag) = record.map_or((0, ""), |g| (g.id, g.tag.as_str()));
        if peer.gid != gid || peer.tag != tag {
            peer.gid = gid;
            peer.tag = tag.to_string();
        }
    }

    // Ce qui a été reçu reste sur le disque : inutile de le redemander la prochaine fois
    if (net.cache_dirty || guilds.dirty) && now - guilds.last_save >= CACHE_SAVE_SECS {
        guilds.last_save = now;
        guilds.dirty = false;
        net.cache_dirty = false;
        save_cache(guilds, net);
    }
}

/// Repart d'une identité vierge : nouvel identifiant, sans guilde, sans étoiles ni
/// relations, avec un pseudo distinct. Rien n'est écrit sur disque (voir `temp_identity`).
fn become_new_player(settings: &mut GameSettings, guilds: &mut Guilds, net: &mut Net, now: f64) {
    settings.temp_identity = true;
    settings.player_id = rand::random::<u64>() | 1;
    settings.guild = None;
    settings.guild_archive.clear();
    settings.clan_tag.clear();
    settings.claims.clear();
    settings.allies.clear();
    settings.enemies.clear();
    let suffix = rand::random::<u32>() % 90 + 10;
    let base: String = settings.player_name.chars().take(MAX_NAME_LEN - 3).collect();
    settings.player_name = format!("{base} {suffix}");
    guilds.request = None;
    net.local.pid = settings.player_id;
    net.local.req = 0;
    net.notify(
        &format!("Un autre jeu utilise deja ce compte : vous jouez en tant que {} (sans guilde).", settings.player_name),
        now,
    );
}

/// Fiches de guilde et profils de joueurs déjà reçus, gardés entre deux parties.
#[derive(Serialize, Deserialize, Default)]
struct NetCache {
    #[serde(default)]
    guilds: Vec<GuildRecord>,
    #[serde(default)]
    profiles: Vec<crate::net::Profile>,
}

fn cache_path() -> std::path::PathBuf {
    crate::settings::data_dir().join("net_cache.json")
}

fn load_cache(guilds: &mut Guilds, net: &mut Net) {
    // Les tests ne touchent pas aux fichiers du joueur
    if cfg!(test) {
        return;
    }
    let Ok(text) = std::fs::read_to_string(cache_path()) else { return };
    let cache: NetCache = serde_json::from_str(&text).unwrap_or_default();
    for record in cache.guilds.into_iter().take(CACHE_MAX_GUILDS) {
        let record = record.sanitized();
        if guilds.known.get(&record.id).map_or(true, |mine| record.newer_than(mine)) {
            guilds.known.insert(record.id, record);
        }
    }
    for profile in cache.profiles {
        net.profiles.insert(profile);
    }
}

fn save_cache(guilds: &Guilds, net: &Net) {
    if cfg!(test) {
        return;
    }
    let mut records: Vec<GuildRecord> = guilds.known.values().cloned().collect();
    records.sort_by_key(|g| std::cmp::Reverse(g.rev));
    records.truncate(CACHE_MAX_GUILDS);
    let cache = NetCache { guilds: records, profiles: net.profiles.all() };
    if let Ok(json) = serde_json::to_string(&cache) {
        std::fs::write(cache_path(), json).ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHIEF: u64 = 1;
    const DEPUTY: u64 = 2;
    const DEPUTY2: u64 = 3;
    const OFFICER: u64 = 4;
    const OFFICER2: u64 = 5;
    const MEMBER: u64 = 6;
    const MEMBER2: u64 = 7;
    const OUTSIDER: u64 = 99;

    fn guild() -> GuildRecord {
        let mut g = GuildRecord::create("Les Spores", "spo", 2, [1.0, 0.0, 0.0], CHIEF, "Chef").unwrap();
        for (id, role) in [
            (DEPUTY, Role::Deputy), (DEPUTY2, Role::Deputy),
            (OFFICER, Role::Officer), (OFFICER2, Role::Officer),
            (MEMBER, Role::Member), (MEMBER2, Role::Member),
        ] {
            g.add_member(CHIEF, id, &format!("J{id}")).unwrap();
            if role != Role::Member {
                g.set_role(CHIEF, id, role).unwrap();
            }
        }
        g
    }

    #[test]
    fn creator_becomes_chief() {
        let g = GuildRecord::create("  Les Spores ", "spo", 2, [1.0; 3], CHIEF, "Chef").unwrap();
        assert_eq!((g.name.as_str(), g.tag.as_str()), ("Les Spores", "SPO"));
        assert_eq!(g.role_of(CHIEF), Some(Role::Chief));
        assert!(GuildRecord::create("", "spo", 0, [1.0; 3], CHIEF, "Chef").is_err());
        assert!(GuildRecord::create("Nom", "!!", 0, [1.0; 3], CHIEF, "Chef").is_err());
    }

    #[test]
    fn officers_manage_members_but_not_roles() {
        let mut g = guild();
        // Ajouter / refuser : Officier et plus
        assert!(g.add_member(MEMBER, OUTSIDER, "X").is_err());
        assert!(g.refuse(MEMBER, OUTSIDER).is_err());
        assert!(g.refuse(OFFICER, OUTSIDER).is_ok());
        assert!(g.refused.contains(&OUTSIDER));
        assert!(g.add_member(OFFICER, OUTSIDER, "X").is_ok());
        assert_eq!(g.role_of(OUTSIDER), Some(Role::Member));
        assert!(!g.refused.contains(&OUTSIDER));
        // Exclure : seulement un rang strictement inférieur
        assert!(g.kick(OFFICER, OFFICER2).is_err());
        assert!(g.kick(OFFICER, DEPUTY).is_err());
        assert!(g.kick(MEMBER, MEMBER2).is_err());
        assert!(g.kick(OFFICER, OUTSIDER).is_ok());
        // Un Officier ne change aucun rôle
        assert!(g.set_role(OFFICER, MEMBER, Role::Officer).is_err());
        assert!(g.set_role(MEMBER, MEMBER2, Role::Officer).is_err());
    }

    #[test]
    fn roles_change_only_below_own_rank() {
        let mut g = guild();
        // Sous-chef : promeut / rétrograde Membres et Officiers, jusqu'à Officier
        assert!(g.set_role(DEPUTY, MEMBER, Role::Officer).is_ok());
        assert!(g.set_role(DEPUTY, MEMBER, Role::Deputy).is_err());
        assert!(g.set_role(DEPUTY, OFFICER, Role::Member).is_ok());
        // ... mais pas un autre Sous-chef, ni le Chef, ni lui-même
        assert!(g.set_role(DEPUTY, DEPUTY2, Role::Officer).is_err());
        assert!(g.set_role(DEPUTY, CHIEF, Role::Member).is_err());
        assert!(g.set_role(DEPUTY, DEPUTY, Role::Member).is_err());
        // Chef : tout sauf donner son propre rang (le rôle de Chef se transfère)
        assert!(g.set_role(CHIEF, DEPUTY2, Role::Officer).is_ok());
        assert!(g.set_role(CHIEF, MEMBER2, Role::Deputy).is_ok());
        assert!(g.set_role(CHIEF, MEMBER2, Role::Chief).is_err());
        assert!(g.set_role(CHIEF, CHIEF, Role::Deputy).is_err());
    }

    #[test]
    fn only_members_ask_for_promotion_and_it_clears_when_granted() {
        let mut g = guild();
        assert!(g.ask_promotion(OFFICER, true).is_err());
        assert!(g.ask_promotion(OUTSIDER, true).is_err());
        assert!(g.ask_promotion(MEMBER, true).is_ok());
        assert!(g.member(MEMBER).unwrap().wants_promotion);
        g.set_role(CHIEF, MEMBER, Role::Officer).unwrap();
        assert!(!g.member(MEMBER).unwrap().wants_promotion);
    }

    #[test]
    fn only_chief_transfers_or_dissolves() {
        let mut g = guild();
        assert!(g.transfer(DEPUTY, MEMBER).is_err());
        assert!(g.dissolve(DEPUTY).is_err());
        assert!(g.transfer(CHIEF, CHIEF).is_err());
        assert!(g.transfer(CHIEF, OUTSIDER).is_err());
        assert!(g.transfer(CHIEF, MEMBER).is_ok());
        assert_eq!((g.role_of(MEMBER), g.role_of(CHIEF)), (Some(Role::Chief), Some(Role::Deputy)));
        // L'ancien Chef ne peut plus dissoudre, le nouveau oui
        assert!(g.dissolve(CHIEF).is_err());
        assert!(g.dissolve(MEMBER).is_ok());
        assert!(g.dissolved && g.members.is_empty());
    }

    #[test]
    fn chief_leaving_hands_over_to_highest_rank() {
        // Deux Sous-chefs à égalité : le tirage choisit entre eux seulement
        for (pick, heir) in [(0, DEPUTY), (1, DEPUTY2)] {
            let mut g = guild();
            let name = g.leave(CHIEF, |n| { assert_eq!(n, 2); pick }).unwrap();
            assert_eq!(g.role_of(heir), Some(Role::Chief));
            assert_eq!(name, Some(format!("J{heir}")));
            assert_eq!(g.members.iter().filter(|m| m.role == Role::Chief).count(), 1);
        }
        // Un membre ordinaire qui part ne change rien aux rôles
        let mut g = guild();
        assert_eq!(g.leave(MEMBER, |_| 0).unwrap(), None);
        assert_eq!(g.role_of(CHIEF), Some(Role::Chief));
        // Dernier membre : la guilde est dissoute
        let mut solo = GuildRecord::create("Solo", "S", 0, [1.0; 3], CHIEF, "Chef").unwrap();
        solo.leave(CHIEF, |_| 0).unwrap();
        assert!(solo.dissolved);
    }

    #[test]
    fn relations_are_set_by_chief_and_deputies() {
        let mut g = guild();
        assert!(g.set_relation(OFFICER, "g:1", "Autre", Relation::Enemy).is_err());
        assert!(g.set_relation(DEPUTY, "g:1", "Autre", Relation::Enemy).is_ok());
        assert_eq!(g.declared("g:1"), Relation::Enemy);
        assert!(g.set_relation(CHIEF, "g:1", "Autre", Relation::Neutral).is_ok());
        assert_eq!(g.declared("g:1"), Relation::Neutral);
        assert!(g.relations.is_empty());
    }

    #[test]
    fn newest_revision_wins_everywhere() {
        let old = guild();
        let mut new = old.clone();
        new.kick(CHIEF, MEMBER).unwrap();
        assert!(new.newer_than(&old) && !old.newer_than(&new) && !old.newer_than(&old));
        // Deux modifications simultanées : les deux joueurs gardent la même
        let mut other = old.clone();
        other.kick(CHIEF, MEMBER2).unwrap();
        assert_ne!(new.newer_than(&other), other.newer_than(&new));
    }
}
