//! Les modèles de l'éditeur dans le jeu (E7, `ROADMAP-0.12-editeur.md`).
//!
//! - Un modèle se désigne par une `ModelKey` : un modèle par défaut (`perso:humanoide`,
//!   `vaisseau:chasseur`, fabriqués par le code, voir `editeur::defaults`) ou l'**empreinte** d'un
//!   fichier `.ssvox` (règle 7 : les autres joueurs le reçoivent une fois, puis le gardent dans
//!   `saves/cache_modeles/`).
//! - `GameModels` charge et maille les modèles hors du fil principal, une fois chacun ; un `Rig`
//!   (composant) affiche un modèle (une entité par zone de mouvement et matière, `RigPart`) et
//!   l'anime : animations des blocs, de la bibliothèque, états du vaisseau, avec ce que donne le
//!   jeu (`motion::Inputs`).
//! - Tailles (décisions du 03/10/2026) : un personnage fait la taille du marcheur (~2 blocs) ; un
//!   vaisseau fait sa vraie taille posé et en vol bas (4 voxels = 1 bloc, Q2), et dans l'espace une
//!   taille d'icône qui suit la caméra.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use bevy::prelude::*;
use bevy::tasks::{block_on, futures_lite::future, AsyncComputeTaskPool, Task};
use serde::{Deserialize, Serialize};

use crate::editeur::defaults;
use crate::editeur::format::{Model, ModelKind, MAX_FILE_BYTES};
use crate::editeur::motion::{self, Inputs, Library, Pose};
use crate::settings::GameSettings;

pub struct ModelsPlugin;

impl Plugin for ModelsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameModels>()
            .add_systems(Startup, (setup_models, spawn_local_walker))
            .add_systems(Update, (resolve_local_models, drive_local.after(crate::surface::SurfaceControl), sync_remote_walkers, load_models, build_rigs, animate_rigs).chain());
    }
}

/// Un modèle : par défaut (nom) ou fichier (empreinte).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModelKey {
    Default(String),
    Print(u64),
}

impl Default for ModelKey {
    fn default() -> Self {
        Self::default_ship()
    }
}

impl ModelKey {
    pub fn default_ship() -> Self {
        ModelKey::Default(defaults::DEFAULT_SHIP.into())
    }

    pub fn default_character() -> Self {
        ModelKey::Default(defaults::DEFAULT_CHARACTER.into())
    }
}

/// Un modèle chargé et maillé.
pub struct Loaded {
    pub model: Model,
    /// (zone de mouvement, matière, maillage), dans le repère du modèle (voxels).
    pub parts: Vec<(u8, usize, Handle<Mesh>)>,
    /// Boîte des blocs (voxels).
    pub lo: Vec3,
    pub hi: Vec3,
    /// Boîtes de collision (une par chunk occupé, au plus juste ; voxels).
    pub boxes: Vec<(Vec3, Vec3)>,
}

impl Loaded {
    pub fn size(&self) -> Vec3 {
        (self.hi - self.lo).max(Vec3::ONE)
    }

    pub fn center(&self) -> Vec3 {
        (self.lo + self.hi) * 0.5
    }
}

/// Tous les modèles connus du jeu.
#[derive(Resource, Default)]
pub struct GameModels {
    pub lib: Library,
    loaded: HashMap<ModelKey, Arc<Loaded>>,
    tasks: HashMap<ModelKey, Task<Option<(Model, Vec<(u8, usize, Mesh)>, Vec<(Vec3, Vec3)>)>>>,
    /// Modèles demandés (chargés dès que possible).
    wanted: HashSet<ModelKey>,
    /// Fichiers connus par empreinte (les nôtres et ceux reçus) : envoyés à qui les demande.
    pub files: HashMap<u64, Arc<Vec<u8>>>,
    /// Introuvables pour l'instant (empreinte reçue d'un autre joueur, fichier en route).
    missing: HashSet<ModelKey>,
    mats: Vec<Handle<StandardMaterial>>,
    /// Modèles du joueur local.
    pub ship: ModelKey,
    pub character: ModelKey,
    /// Les choix du joueur ont changé (éditeur : « Utiliser en jeu »).
    pub reload: bool,
}

/// Dossier des modèles reçus des autres joueurs.
pub fn cache_dir() -> PathBuf {
    crate::settings::data_dir().join("cache_modeles")
}

impl GameModels {
    /// Le modèle, s'il est prêt (sinon il est demandé).
    pub fn get(&mut self, key: &ModelKey) -> Option<Arc<Loaded>> {
        if let Some(l) = self.loaded.get(key) {
            return Some(l.clone());
        }
        if let ModelKey::Print(fp) = key {
            if self.file(*fp).is_none() {
                self.missing.insert(key.clone());
                return None;
            }
        }
        self.wanted.insert(key.clone());
        None
    }

    pub fn peek(&self, key: &ModelKey) -> Option<&Arc<Loaded>> {
        self.loaded.get(key)
    }

    /// Les fichiers des autres joueurs qui nous manquent (à demander au réseau).
    pub fn missing_prints(&self) -> Vec<u64> {
        self.missing.iter().filter_map(|k| if let ModelKey::Print(fp) = k { Some(*fp) } else { None }).collect()
    }

    /// Un fichier reçu (ou choisi) : gardé, et chargé s'il était attendu.
    pub fn add_file(&mut self, bytes: Vec<u8>) -> Option<u64> {
        if bytes.len() > MAX_FILE_BYTES {
            return None;
        }
        let fp = Model::fingerprint(&bytes);
        Model::from_bytes(&bytes).ok()?;
        let dir = cache_dir();
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join(format!("{fp:016x}.ssvox")), &bytes);
        self.files.insert(fp, Arc::new(bytes));
        self.missing.remove(&ModelKey::Print(fp));
        Some(fp)
    }

    /// Les octets d'un modèle (fichier connu ou en cache).
    pub fn file(&mut self, fp: u64) -> Option<Arc<Vec<u8>>> {
        if let Some(b) = self.files.get(&fp) {
            return Some(b.clone());
        }
        let b = std::fs::read(cache_dir().join(format!("{fp:016x}.ssvox"))).ok()?;
        (Model::fingerprint(&b) == fp).then(|| {
            let b = Arc::new(b);
            self.files.insert(fp, b.clone());
            b
        })
    }

    /// Matériau de rendu d'une matière (mate, métal, verre, lumineuse).
    pub fn material(&self, k: usize) -> Handle<StandardMaterial> {
        self.mats.get(k).cloned().unwrap_or_default()
    }
}

fn setup_models(mut models: ResMut<GameModels>, mut materials: ResMut<Assets<StandardMaterial>>) {
    models.lib = Library::load(Some(&crate::editeur::editor_dir())).0;
    models.mats = (0..4).map(|k| materials.add(crate::editeur::view::material_of(k))).collect();
    models.ship = ModelKey::default_ship();
    models.character = ModelKey::default_character();
    models.reload = true;
}

/// Le fichier d'un modèle choisi par le joueur (chemin relatif au dossier des sauvegardes, ou
/// absolu) -> sa clé ; un fichier illisible : le modèle par défaut.
fn choice(models: &mut GameModels, path: &Option<String>, fallback: ModelKey) -> ModelKey {
    let Some(p) = path else { return fallback };
    let full = {
        let p = PathBuf::from(p);
        if p.is_absolute() { p } else { crate::settings::data_dir().join(p) }
    };
    match std::fs::read(&full).ok().and_then(|b| models.add_file(b)) {
        Some(fp) => ModelKey::Print(fp),
        None => {
            warn!("Modele {} illisible : modele par defaut", full.display());
            fallback
        }
    }
}

/// Les modèles du joueur local (au départ, et quand il en choisit un autre).
fn resolve_local_models(mut models: ResMut<GameModels>, settings: Res<GameSettings>) {
    if !models.reload {
        return;
    }
    models.reload = false;
    let ship = choice(&mut models, &settings.ship_model, ModelKey::default_ship());
    let character = choice(&mut models, &settings.character_model, ModelKey::default_character());
    models.ship = ship;
    models.character = character;
}

/// Charge et maille les modèles demandés, hors du fil principal.
fn load_models(mut models: ResMut<GameModels>, mut meshes: ResMut<Assets<Mesh>>) {
    let wanted: Vec<ModelKey> = models.wanted.drain().collect();
    let pool = AsyncComputeTaskPool::get();
    for key in wanted {
        if models.loaded.contains_key(&key) || models.tasks.contains_key(&key) {
            continue;
        }
        let source: Option<Box<dyn FnOnce() -> Option<Model> + Send>> = match &key {
            ModelKey::Default(id) => {
                let (id, lib) = (id.clone(), models.lib.clone());
                Some(Box::new(move || defaults::build(&id, &lib)))
            }
            ModelKey::Print(fp) => match models.file(*fp) {
                Some(b) => Some(Box::new(move || Model::from_bytes(&b).ok())),
                None => {
                    models.missing.insert(key.clone());
                    None
                }
            },
        };
        let Some(source) = source else { continue };
        models.tasks.insert(
            key,
            pool.spawn(async move {
                let m = source()?;
                let parts = crate::editeur::edit::build_parts(&m);
                let boxes = collision_boxes(&m);
                Some((m, parts, boxes))
            }),
        );
    }
    let models = &mut *models;
    let finished: Vec<(ModelKey, _)> = models.tasks.iter_mut().filter_map(|(k, t)| block_on(future::poll_once(t)).map(|r| (k.clone(), r))).collect();
    let done: Vec<ModelKey> = finished.into_iter().map(|(k, r)| {
        if let Some((model, parts, boxes)) = r {
            let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
            for (a, b) in &boxes {
                lo = lo.min(*a);
                hi = hi.max(*b);
            }
            if lo.x > hi.x {
                (lo, hi) = (Vec3::ZERO, Vec3::ONE);
            }
            let parts = parts.into_iter().map(|(z, k, m)| (z, k, meshes.add(m))).collect();
            models.loaded.insert(k.clone(), Arc::new(Loaded { model, parts, lo, hi, boxes }));
        } else {
            warn!("Modele {k:?} illisible");
        }
        k
    }).collect();
    for k in done {
        models.tasks.remove(&k);
    }
}

/// Boîtes de collision : la boîte des blocs de chaque chunk occupé (voxels).
pub fn collision_boxes(m: &Model) -> Vec<(Vec3, Vec3)> {
    use crate::editeur::format::CHUNK;
    let mut boxes: HashMap<IVec3, (IVec3, IVec3)> = HashMap::new();
    for (p, _) in m.voxels.iter() {
        let c = p.div_euclid(IVec3::splat(CHUNK));
        let e = boxes.entry(c).or_insert((p, p));
        e.0 = e.0.min(p);
        e.1 = e.1.max(p);
    }
    let mut v: Vec<(Vec3, Vec3)> = boxes.into_values().map(|(a, b)| (a.as_vec3(), (b + IVec3::ONE).as_vec3())).collect();
    v.sort_by(|a, b| a.0.to_array().partial_cmp(&b.0.to_array()).unwrap_or(std::cmp::Ordering::Equal));
    v
}

// ─────────────────────────────────────────────────────────────────────────
//  Affichage et animation
// ─────────────────────────────────────────────────────────────────────────

/// Comment un rig se place sous son parent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fit {
    /// Vaisseau : nez (+z du modèle) vers l'avant du parent (-Z), centré ; la plus grande dimension
    /// fait `icon_length` (le parent porte l'échelle).
    Ship,
    /// Personnage : pieds à l'origine, regard vers l'avant du parent (-Z), hauteur `height` (unités
    /// du monde : la taille du marcheur).
    Character { height: f32 },
}

/// Longueur d'un vaisseau à l'échelle 1 (unités) : celle de l'ancien vaisseau (5) pour un chasseur,
/// un peu plus grand pour les grandes catégories (sans écraser l'écran).
pub fn icon_length(l: &Loaded) -> f32 {
    5.0 * (l.size().max_element() / 64.0).max(0.25).sqrt()
}

/// Placement du modèle sous son parent.
pub fn fit_transform(l: &Loaded, fit: Fit) -> Transform {
    let turn = Quat::from_rotation_y(std::f32::consts::PI);
    match fit {
        Fit::Ship => {
            let k = icon_length(l) / l.size().max_element();
            Transform { translation: turn * (-l.center() * k), rotation: turn, scale: Vec3::splat(k) }
        }
        Fit::Character { height } => {
            let k = height / l.size().y.max(1.0);
            let foot = Vec3::new(l.center().x, l.lo.y, l.center().z);
            Transform { translation: turn * (-foot * k), rotation: turn, scale: Vec3::splat(k) }
        }
    }
}

/// Un modèle affiché et animé (enfant d'un vaisseau, d'un marcheur...).
#[derive(Component)]
pub struct Rig {
    pub key: ModelKey,
    pub fit: Fit,
    /// Animation jouée (bloc, bibliothèque, « etat:vol »...) et depuis quand.
    pub anim: String,
    pub t: f32,
    pub inputs: Inputs,
    /// Mélange depuis la pose d'avant (changement d'animation).
    from: Vec<Pose>,
    blend: f32,
    blend_secs: f32,
    last: Vec<Pose>,
    built: Option<ModelKey>,
}

impl Rig {
    pub fn new(key: ModelKey, fit: Fit, anim: &str) -> Self {
        Self { key, fit, anim: anim.into(), t: 0.0, inputs: Inputs::default(), from: Vec::new(), blend: 1.0, blend_secs: 0.3, last: Vec::new(), built: None }
    }

    /// Change d'animation (en douceur ; plus lentement entre deux états du vaisseau).
    pub fn play(&mut self, anim: &str) {
        if self.anim == anim {
            return;
        }
        self.blend_secs = if anim.starts_with(motion::STATE_PREFIX) { 1.5 } else { 0.25 };
        self.anim = anim.into();
        self.t = 0.0;
        self.from = self.last.clone();
        self.blend = 0.0;
    }

}

/// Une pièce d'un rig (zone de mouvement, 0 = corps fixe).
#[derive(Component)]
pub struct RigPart(pub u8);

/// (Re)construit les rigs dont le modèle est prêt ou a changé.
fn build_rigs(mut commands: Commands, mut models: ResMut<GameModels>, mut rigs: Query<(Entity, &mut Rig, &mut Transform, Option<&Children>)>, parts: Query<(), With<RigPart>>) {
    for (e, mut rig, mut tf, children) in &mut rigs {
        if rig.built.as_ref() == Some(&rig.key) {
            continue;
        }
        let Some(l) = models.get(&rig.key.clone()) else { continue };
        for c in children.into_iter().flatten() {
            if parts.contains(*c) {
                commands.entity(*c).despawn_recursive();
            }
        }
        *tf = fit_transform(&l, rig.fit);
        commands.entity(e).with_children(|p| {
            for (zone, k, mesh) in &l.parts {
                p.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(models.material(*k)), Transform::IDENTITY, RigPart(*zone)));
            }
        });
        rig.built = Some(rig.key.clone());
        rig.last.clear();
        rig.from.clear();
    }
}

/// Anime les rigs : pose de chaque zone, mélangée avec la pose d'avant.
fn animate_rigs(time: Res<Time>, models: Res<GameModels>, mut rigs: Query<(&mut Rig, &Children)>, mut parts: Query<(&RigPart, &mut Transform)>) {
    let dt = time.delta_secs();
    for (mut rig, children) in &mut rigs {
        let Some(l) = rig.built.as_ref().and_then(|k| models.peek(k)).cloned() else { continue };
        if l.model.zones.is_empty() {
            continue;
        }
        rig.t += dt;
        rig.blend = (rig.blend + dt / rig.blend_secs.max(0.05)).min(1.0);
        let now = motion::zone_locals_with(&l.model, &models.lib, &rig.anim, rig.t, l.model.kind == ModelKind::Personnage, &rig.inputs);
        let pose = if rig.blend < 1.0 && rig.from.len() == now.len() {
            let f = rig.blend * rig.blend * (3.0 - 2.0 * rig.blend);
            motion::blend(&rig.from, &now, f)
        } else {
            now
        };
        let mats = motion::compose(&l.model, &pose);
        rig.last = pose;
        for c in children {
            if let Ok((part, mut tf)) = parts.get_mut(*c) {
                let t = Transform::from_matrix(mats.get(part.0 as usize).copied().unwrap_or(Mat4::IDENTITY));
                if *tf != t {
                    *tf = t;
                }
            }
        }
    }
}

/// Le modèle du vaisseau du joueur (enfant du vaisseau).
#[derive(Component)]
pub struct ShipRig;

/// Le personnage du joueur, à pied (vue à la troisième personne), et son modèle.
#[derive(Component)]
pub struct LocalWalker;

#[derive(Component)]
pub struct WalkerRig;

fn spawn_local_walker(mut commands: Commands) {
    commands.spawn((Transform::IDENTITY, Visibility::Hidden, LocalWalker)).with_child((
        Rig::new(ModelKey::default_character(), Fit::Character { height: 2.0 }, "repos"),
        Transform::IDENTITY,
        Visibility::default(),
        WalkerRig,
    ));
}

/// Le vaisseau et le personnage du joueur suivent le jeu : modèles choisis, état du vaisseau
/// (posé, vol, combat, détruit...), poussée, cible des tourelles, animation du marcheur.
#[allow(clippy::too_many_arguments)]
fn drive_local(
    time: Res<Time>,
    models: Res<GameModels>,
    mut surface: ResMut<crate::surface::Surface>,
    combat: Option<Res<crate::combat::CombatState>>,
    net: Res<crate::net::Net>,
    ship_q: Query<&GlobalTransform, With<crate::ship::Ship>>,
    mut ship_rig: Query<(&GlobalTransform, &mut Rig), (With<ShipRig>, Without<WalkerRig>)>,
    mut walker: Query<(&mut Transform, &mut Visibility), With<LocalWalker>>,
    mut walker_rig: Query<&mut Rig, (With<WalkerRig>, Without<ShipRig>)>,
    thrust_q: Query<&crate::ship::ShipThrust>,
    mut last: Local<Option<(Quat, f32)>>,
) {
    let now = time.elapsed_secs_f64();
    let dt = time.delta_secs().max(1e-4);
    if let Some(l) = models.peek(&models.ship) {
        let len = icon_length(l);
        surface.ship_dims = crate::surface::ShipDims { icon_len: len, half_h: len * l.size().y / l.size().max_element() * 0.5, voxels: l.size().max_element() };
    }
    if let (Ok(ship), Ok((rig_gt, mut rig))) = (ship_q.get_single(), ship_rig.get_single_mut()) {
        if rig.key != models.ship {
            rig.key = models.ship.clone();
        }
        let in_combat = combat.as_ref().is_some_and(|c| now - c.last_shot.max(c.last_damage) < 6.0 && c.last_shot + c.last_damage > 0.0);
        let state = if net.local.hp == 0 {
            "detruit"
        } else if in_combat && surface.ship_state() == "vol" {
            "combat"
        } else {
            surface.ship_state()
        };
        // Un autre joueur entre ou sort d'un de mes hangars : les portes s'ouvrent
        match crate::dock::carrier_sequence(&net, net.my_id()) {
            Some((seq, t)) => {
                rig.play(seq);
                rig.t = t;
            }
            None => rig.play(&format!("{}{state}", motion::STATE_PREFIX)),
        }
        // Poussée : les commandes (vol bas, décollage, atterrissage) ou le pilote automatique
        // (croisière), jamais le déplacement dans le monde (orbite de l'astre, origine flottante)
        let pos = ship.translation();
        let (push, turn) = if surface.active() { (surface.pilot, surface.pilot_turn) } else { (thrust_q.get_single().map_or(Vec3::ZERO, |t| t.push), 0.0) };
        let rot = ship.compute_transform().rotation;
        let (prev, thrust) = last.unwrap_or((rot, 0.0));
        let mag = push.length().min(1.0);
        let thrust = thrust + (mag - thrust) * (1.0 - (-4.0 * dt).exp());
        *last = Some((rot, thrust));
        rig.inputs.thrust = thrust;
        rig.inputs.speed = thrust;
        // Manœuvre : virage demandé ou mesuré (le vaisseau tourne), poussée de côté ou verticale
        let spin = (prev.angle_between(rot) / dt / 0.8).min(1.0);
        let side = (push.x.abs() + push.y.abs()).min(1.0);
        rig.inputs.maneuver = turn.abs().max(spin).max(side * 0.8).clamp(0.0, 1.0);
        // Tuyères : la direction de poussée dans le repère du modèle (son avant +z = -Z du vaisseau)
        rig.inputs.steer = if mag > 0.01 { Vec3::new(-push.x, push.y, -push.z) / push.length() } else { Vec3::Z };
        // Tourelles : le vaisseau le plus proche (dans le repère du modèle)
        let to_model = rig_gt.affine().inverse();
        let target = net.peers.values().filter(|p| p.status.hp > 0).map(|p| p.pos()).min_by(|a, b| a.distance(pos).total_cmp(&b.distance(pos)));
        rig.inputs.target = match target {
            Some(t) => to_model.transform_point3(t),
            None => Inputs::default().target,
        };
    }
    if let (Ok((mut tf, mut vis)), Ok(mut rig)) = (walker.get_single_mut(), walker_rig.get_single_mut()) {
        if rig.key != models.character {
            rig.key = models.character.clone();
        }
        match surface.walker_view() {
            Some((t, anim)) => {
                *tf = t;
                *vis = Visibility::Inherited;
                rig.play(anim);
            }
            None => *vis = Visibility::Hidden,
        }
    }
}

/// Le personnage d'un autre joueur, à pied sur un astre que nous avons chargé.
#[derive(Component)]
pub struct RemoteWalker(pub u32);

/// Affiche les autres joueurs à pied (leur personnage, animé) sur les astres chargés ici.
#[allow(clippy::too_many_arguments)]
fn sync_remote_walkers(
    mut commands: Commands,
    net: Res<crate::net::Net>,
    planets: Query<(&Transform, &crate::planet::PlanetId), (With<crate::planet::PlanetRoot>, Without<RemoteWalker>)>,
    moons: Query<(&Transform, &crate::planet::MoonId), (With<crate::planet::MoonRoot>, Without<RemoteWalker>)>,
    mut walkers: Query<(Entity, &RemoteWalker, &mut Transform, &Children), (Without<crate::planet::PlanetRoot>, Without<crate::planet::MoonRoot>)>,
    mut rigs: Query<&mut Rig>,
) {
    use crate::ui::TargetKind;
    let frame = |k: TargetKind| -> Option<crate::surface::Frame> {
        let tf = match k {
            TargetKind::Planet(id) => planets.iter().find(|(_, p)| p.0 == id).map(|(t, _)| *t),
            TargetKind::Moon(a, b) => moons.iter().find(|(_, m)| m.planet_idx == a && m.moon_idx == b).map(|(t, _)| *t),
            _ => None,
        }?;
        Some(crate::surface::Frame { center: tf.translation, rot: tf.rotation })
    };
    let mut shown = HashSet::new();
    for (e, w, mut tf, children) in &mut walkers {
        let place = net.peers.get(&w.0).and_then(|p| p.look.walk.as_ref().map(|s| (p, s))).and_then(|(p, s)| Some((p, s, frame(s.body()?)?)));
        let Some((peer, state, f)) = place else {
            commands.entity(e).despawn_recursive();
            continue;
        };
        shown.insert(w.0);
        *tf = f.to_world(state.local());
        for c in children {
            if let Ok(mut rig) = rigs.get_mut(*c) {
                let key = peer.look.character_key();
                if rig.key != key {
                    rig.key = key;
                }
                rig.play(&state.anim);
            }
        }
    }
    for (id, peer) in &net.peers {
        let Some(state) = peer.look.walk.as_ref() else { continue };
        if shown.contains(id) {
            continue;
        }
        let Some(f) = state.body().and_then(frame) else { continue };
        commands.spawn((f.to_world(state.local()), Visibility::default(), RemoteWalker(*id))).with_child((
            Rig::new(peer.look.character_key(), Fit::Character { height: 2.0 }, &state.anim),
            Transform::IDENTITY,
            Visibility::default(),
        ));
    }
}

/// Repousse le point `p` hors des boîtes de collision du modèle `l` (marcheur contre un vaisseau
/// posé) : `to_model` passe du repère de `p` à celui du modèle (voxels) ; seulement à l'horizontale
/// du modèle. Renvoie le déplacement à appliquer.
pub fn push_out(l: &Loaded, to_model: Mat4, p: Vec3, radius: f32) -> Option<Vec3> {
    let q = to_model.transform_point3(p);
    let r = to_model.transform_vector3(Vec3::X * radius).length();
    for (a, b) in &l.boxes {
        if q.y < a.y || q.y > b.y {
            continue;
        }
        let (lo, hi) = (*a - Vec3::new(r, 0.0, r), *b + Vec3::new(r, 0.0, r));
        if q.x > lo.x && q.x < hi.x && q.z > lo.z && q.z < hi.z {
            let d = [q.x - lo.x, hi.x - q.x, q.z - lo.z, hi.z - q.z];
            let i = (0..4).min_by(|x, y| d[*x].total_cmp(&d[*y])).unwrap_or(0);
            let mut n = q;
            match i {
                0 => n.x = lo.x,
                1 => n.x = hi.x,
                2 => n.z = lo.z,
                _ => n.z = hi.z,
            }
            return Some(to_model.inverse().transform_point3(n) - p);
        }
    }
    None
}

/// Animation d'un personnage d'après ce que fait le marcheur.
pub fn walker_anim(on_ground: bool, in_water: bool, speed: f32, sprint: bool, microgravity: bool, vr: f32, voxel: f32) -> &'static str {
    if in_water {
        "nager"
    } else if !on_ground {
        if microgravity {
            "flotter"
        } else if vr > 0.0 {
            "sauter"
        } else {
            "tomber"
        }
    } else if speed > voxel * 0.5 {
        if sprint { "courir" } else { "marcher" }
    } else {
        "repos"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collision_boxes_cover_every_block() {
        let lib = Library::load(None).0;
        let m = defaults::build("vaisseau:chasseur", &lib).unwrap();
        let boxes = collision_boxes(&m);
        assert!(!boxes.is_empty());
        for (p, _) in m.voxels.iter() {
            let c = p.as_vec3() + Vec3::splat(0.5);
            assert!(boxes.iter().any(|(a, b)| c.cmpge(*a).all() && c.cmple(*b).all()));
        }
    }

    #[test]
    fn walkers_are_pushed_out_of_a_landed_ship() {
        let lib = Library::load(None).0;
        let m = defaults::build("vaisseau:chasseur", &lib).unwrap();
        let boxes = collision_boxes(&m);
        let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for (a, b) in &boxes {
            lo = lo.min(*a);
            hi = hi.max(*b);
        }
        let l = Loaded { model: m, parts: Vec::new(), lo, hi, boxes };
        // Le vaisseau posé, deux fois plus grand, déplacé ; un marcheur au milieu de sa coque
        let ship = Transform::from_translation(Vec3::new(100.0, 0.0, 50.0)).with_scale(Vec3::splat(2.0));
        let to_ship = (ship.compute_matrix() * fit_transform(&l, Fit::Ship).compute_matrix()).inverse();
        let inside = ship.transform_point(Vec3::ZERO);
        let push = push_out(&l, to_ship, inside, 0.5).expect("dedans");
        assert!(push.length() > 0.1 && push.y.abs() < 1e-3, "{push}");
        // Loin du vaisseau : rien
        assert!(push_out(&l, to_ship, inside + Vec3::new(200.0, 0.0, 0.0), 0.5).is_none());
    }

    #[test]
    fn walker_animations() {
        assert_eq!(walker_anim(true, false, 0.0, false, false, 0.0, 1.0), "repos");
        assert_eq!(walker_anim(true, false, 5.0, true, false, 0.0, 1.0), "courir");
        assert_eq!(walker_anim(false, false, 0.0, false, true, 0.0, 1.0), "flotter");
        assert_eq!(walker_anim(false, true, 3.0, false, false, 0.0, 1.0), "nager");
    }
}
