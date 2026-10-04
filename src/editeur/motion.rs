//! Blocs de mouvement (E4, §4 de `ROADMAP-0.12-editeur.md`) : des pièces déjà animées que le joueur
//! pose comme des blocs, puis repeint et complète.
//!
//! - Données, pas code (règle 4) : chaque bloc est un fichier JSON de `assets/editeur/blocs/`
//!   (intégré au jeu à la compilation par `build.rs`), plus ceux que le joueur dépose dans
//!   `saves/editeur/blocs/`.
//! - Un bloc = des **parties** (os) : nom, parent (parties emboîtées : main dans avant-bras dans
//!   bras), pivot, gabarit en boîtes de cases ; et des **animations** (repos, marche, ouverture…) :
//!   images clés (rotation, déplacement) ou **ondes** (queues, tentacules : sinusoïde déphasée).
//! - Posé dans un modèle : chaque partie devient une **zone** (`format::Zone`) qui garde son bloc,
//!   son orientation (quarts de tour), son miroir et sa taille ; les animations sont tournées et
//!   reflétées de même.
//! - E6 : blocs de vaisseau (§5) : pistes pilotées par le jeu (`entree` : poussée, vitesse,
//!   manœuvre ; `vise` : tourelle ou tuyère vers une cible ; `clignote` : feux), **états du
//!   vaisseau** (`BlockDef::states` : posé, décollage, vol, combat, atterrissage, détruit -> une
//!   animation par état ; on passe de l'une à l'autre en douceur) et **hangars** (§5.1 :
//!   `BlockDef::hangar`, séquences d'entrée et de sortie).
//! - E5 : bibliothèque d'animations de personnage (`assets/editeur/anims/`, §3.3) par noms d'os,
//!   chaînes (`queue_*`), côté droit reflété du gauche, couche procédurale toujours active, taille
//!   animée ; `Library` réunit blocs, animations et races (`races.rs`).

use bevy::math::{IVec3, Mat4, Quat, Vec2, Vec3};
use super::format::{Model, Zone};
use serde::Deserialize;
use std::collections::BTreeMap;

include!(concat!(env!("OUT_DIR"), "/donnees.rs"));

#[derive(Clone, Debug, Deserialize)]
pub struct BlockDef {
    pub id: String,
    pub name: String,
    /// Types de modèles où il est proposé (« personnage », « vaisseau », « autre ») ; vide : tous.
    #[serde(default)]
    pub kinds: Vec<String>,
    /// La taille peut changer (Maj+molette).
    #[serde(default)]
    pub scalable: bool,
    /// Case du gabarit posée sur la case visée.
    #[serde(default)]
    pub anchor: [i32; 3],
    pub parts: Vec<PartDef>,
    #[serde(default)]
    pub anims: BTreeMap<String, AnimDef>,
    /// Animation de chaque état du vaisseau (`SHIP_STATES`) ; un état absent : « repos ».
    #[serde(default)]
    pub states: BTreeMap<String, String>,
    /// Bloc de hangar (§5.1) : une place d'amarrage et son chemin.
    #[serde(default)]
    pub hangar: Option<HangarDef>,
}

/// Place d'amarrage d'un bloc de hangar, dans le repère du bloc (coins des cases).
#[derive(Clone, Debug, Deserialize)]
pub struct HangarDef {
    /// Catégorie de vaisseau la plus grande qui y entre.
    pub category: super::format::ShipCategory,
    /// Soute à cargos (vaisseaux chargés de ressources, capitaux seulement).
    #[serde(default)]
    pub cargo: bool,
    /// Partie qui sert de porte (elle joue « ouverture » pendant l'entrée et la sortie).
    pub door: String,
    /// Place du vaisseau amarré et direction de son nez.
    pub slot: [f32; 3],
    pub facing: [f32; 3],
    /// Chemin d'entrée, du dehors jusqu'à la place (la sortie le parcourt à l'envers).
    pub path: Vec<[f32; 3]>,
}

/// États du vaisseau (§5) : (identifiant, nom). Le jeu change d'état (E7), les zones suivent.
pub const SHIP_STATES: [(&str, &str); 6] = [("pose", "Pose"), ("decollage", "Decollage"), ("vol", "Vol"), ("combat", "Combat"), ("atterrissage", "Atterrissage"), ("detruit", "Detruit")];

/// Préfixes des animations d'état et des séquences de hangar dans l'aperçu.
pub const STATE_PREFIX: &str = "etat:";
pub const HANGAR_IN: &str = "hangar:entree";
pub const HANGAR_OUT: &str = "hangar:sortie";
/// Durée d'une entrée ou d'une sortie de hangar (s) : porte 1 s, trajet, porte 1 s.
pub const HANGAR_SECS: f32 = 8.0;

/// Ce que le jeu donne aux pistes pilotées (E7 ; l'aperçu de l'éditeur les simule).
#[derive(Clone, Copy, Debug)]
pub struct Inputs {
    /// Poussée, vitesse, manœuvre (0 à 1).
    pub thrust: f32,
    pub speed: f32,
    pub maneuver: f32,
    /// Cible des tourelles (point du repère du modèle).
    pub target: Vec3,
    /// Direction de poussée voulue (repère du modèle) : les tuyères soufflent à l'opposé.
    pub steer: Vec3,
}

impl Default for Inputs {
    fn default() -> Self {
        Self { thrust: 0.0, speed: 0.0, maneuver: 0.0, target: Vec3::new(0.0, 0.0, 1.0e5), steer: Vec3::Z }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct PartDef {
    pub name: String,
    #[serde(default)]
    pub parent: Option<String>,
    /// Pivot (coin des cases : la case (0, 0, 0) va de 0 à 1).
    pub pivot: [f32; 3],
    /// Boîtes de cases [x0, y0, z0, x1, y1, z1] (bornes comprises).
    pub boxes: Vec<[i32; 6]>,
    /// Couleur de départ (flammes, feux) ; sans : blanc mate.
    #[serde(default)]
    pub color: Option<[u8; 3]>,
    #[serde(default)]
    pub material: Option<super::format::Material>,
}

fn one() -> f32 {
    1.0
}

#[derive(Clone, Debug, Deserialize)]
pub struct AnimDef {
    /// Durée d'une boucle (s).
    #[serde(default = "one")]
    pub duration: f32,
    #[serde(default)]
    pub tracks: BTreeMap<String, Track>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type")]
pub enum Track {
    /// Images clés : [t, rx, ry, rz] (degrés, angles d'Euler XYZ), [t, x, y, z] (cases) et
    /// [t, sx, sy, sz] (taille : slime qui s'écrase et s'étire).
    #[serde(rename = "cles")]
    Keys {
        #[serde(default)]
        rot: Vec<[f32; 4]>,
        #[serde(default)]
        pos: Vec<[f32; 4]>,
        #[serde(default)]
        scale: Vec<[f32; 4]>,
    },
    /// Onde : rotation autour de `axis`, amplitude (degrés), période (s), déphasage (fraction) ;
    /// dans une chaîne, chaque maillon est décalé de `step` et son amplitude grandit de `amp_step`.
    #[serde(rename = "onde")]
    Wave {
        axis: [f32; 3],
        amplitude: f32,
        period: f32,
        #[serde(default)]
        phase: f32,
        #[serde(default)]
        step: f32,
        #[serde(default)]
        amp_step: f32,
        /// Position autour de laquelle on oscille (degrés, Euler XYZ) : aile dépliée, cape
        /// toujours derrière le dos.
        #[serde(default)]
        base: [f32; 3],
    },
    /// Piloté par le jeu (E6) : `input` = « poussee », « vitesse » ou « manoeuvre » (0 à 1) fait
    /// passer de `rot[0]` à `rot[1]` (degrés) et de `scale[0]` à `scale[1]` ; `flicker` = flamme qui
    /// vacille.
    #[serde(rename = "entree")]
    Input {
        input: String,
        #[serde(default)]
        rot: [[f32; 3]; 2],
        #[serde(default = "unit_scales")]
        scale: [[f32; 3]; 2],
        #[serde(default)]
        flicker: f32,
    },
    /// Clignote (feux de position) : allumé une fraction `on` de chaque période.
    #[serde(rename = "clignote")]
    Blink {
        period: f32,
        #[serde(default = "half")]
        on: f32,
        #[serde(default)]
        phase: f32,
    },
    /// Vise (tourelle, tuyère) : tourne autour de `axis` pour amener `forward` (repère du bloc) vers
    /// la cible (`input` = « cible ») ou à l'opposé de la poussée (« poussee »), entre `limits`
    /// (degrés).
    #[serde(rename = "vise")]
    Aim {
        axis: [f32; 3],
        #[serde(default = "forward_z")]
        forward: [f32; 3],
        #[serde(default = "no_limits")]
        limits: [f32; 2],
        #[serde(default = "cible")]
        input: String,
    },
}

fn unit_scales() -> [[f32; 3]; 2] {
    [[1.0; 3]; 2]
}

fn half() -> f32 {
    0.5
}

fn forward_z() -> [f32; 3] {
    [0.0, 0.0, 1.0]
}

fn no_limits() -> [f32; 2] {
    [-180.0, 180.0]
}

fn cible() -> String {
    "cible".into()
}

/// Tous les blocs : ceux du jeu, puis ceux du joueur (un fichier illisible est signalé, pas fatal).
pub fn load_blocks(extra_dir: Option<&std::path::Path>) -> (Vec<BlockDef>, Vec<String>) {
    let mut out = Vec::new();
    let mut errors = Vec::new();
    for (file, text) in BUILTIN_BLOCKS {
        match serde_json::from_str::<BlockDef>(text) {
            Ok(b) => out.push(b),
            Err(e) => errors.push(format!("{file} : {e}")),
        }
    }
    if let Some(Ok(rd)) = extra_dir.map(std::fs::read_dir) {
        for p in rd.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|x| x == "json")) {
            match std::fs::read_to_string(&p).map_err(|e| e.to_string()).and_then(|t| serde_json::from_str::<BlockDef>(&t).map_err(|e| e.to_string())) {
                Ok(b) => {
                    // Un bloc du joueur du même nom remplace celui du jeu
                    out.retain(|x: &BlockDef| x.id != b.id);
                    out.push(b);
                }
                Err(e) => errors.push(format!("{} : {e}", p.display())),
            }
        }
    }
    (out, errors)
}

// ─────────────────────────────────────────────────────────────────────────
//  Placement : quarts de tour autour de y, miroir (x), taille
// ─────────────────────────────────────────────────────────────────────────

/// Orientation d'un bloc posé.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Placement {
    /// Quarts de tour autour de y (0 à 3).
    pub turn: u8,
    /// Reflet gauche / droite (x -> -x), appliqué avant le tour.
    pub mirror: bool,
    /// Taille (1 = celle du gabarit).
    pub scale: u8,
}

impl Placement {
    pub fn rotation(&self) -> Quat {
        Quat::from_rotation_y(self.turn as f32 * std::f32::consts::FRAC_PI_2)
    }

    /// Un vecteur du repère du bloc dans celui du modèle (sans la taille).
    pub fn vector(&self, v: Vec3) -> Vec3 {
        let v = if self.mirror { Vec3::new(-v.x, v.y, v.z) } else { v };
        self.rotation() * v
    }

    /// Une rotation du repère du bloc dans celui du modèle : le reflet change le sens des
    /// rotations (quaternion (w, x, y, z) -> (w, x, -y, -z)), puis le tour la fait pivoter.
    pub fn rotate(&self, q: Quat) -> Quat {
        let q = if self.mirror { Quat::from_xyzw(q.x, -q.y, -q.z, q.w) } else { q };
        let r = self.rotation();
        r * q * r.inverse()
    }

    pub fn scale(&self) -> f32 {
        self.scale.max(1) as f32
    }

    /// Point du bloc (coins des cases) -> point du modèle, l'ancre du bloc posée sur `at`.
    pub fn point(&self, def: &BlockDef, at: IVec3, p: Vec3) -> Vec3 {
        let a = IVec3::from_array(def.anchor).as_vec3();
        let s = self.scale();
        // L'ancre (centre de sa première sous-case) tombe au centre de la case visée
        let t = |q: Vec3| self.vector((q - a) * s);
        let offset = Vec3::splat(0.5) - t(a + Vec3::splat(0.5 / s));
        at.as_vec3() + offset + t(p)
    }

    /// Cases du modèle occupées par la case `c` du bloc (taille³ cases).
    pub fn cells(&self, def: &BlockDef, at: IVec3, c: IVec3) -> Vec<IVec3> {
        let s = self.scale.max(1) as i32;
        let mut out = Vec::with_capacity((s * s * s) as usize);
        for i in 0..s {
            for j in 0..s {
                for k in 0..s {
                    let sub = c.as_vec3() + (Vec3::new(i as f32, j as f32, k as f32) + Vec3::splat(0.5)) / s as f32;
                    out.push(self.point(def, at, sub).floor().as_ivec3());
                }
            }
        }
        out
    }
}

/// Cases du gabarit d'une partie.
pub fn part_cells(part: &PartDef) -> Vec<IVec3> {
    let mut out = Vec::new();
    for b in &part.boxes {
        for x in b[0].min(b[3])..=b[0].max(b[3]) {
            for y in b[1].min(b[4])..=b[1].max(b[4]) {
                for z in b[2].min(b[5])..=b[2].max(b[5]) {
                    out.push(IVec3::new(x, y, z));
                }
            }
        }
    }
    out
}

// ─────────────────────────────────────────────────────────────────────────
//  Animations
// ─────────────────────────────────────────────────────────────────────────

fn euler(d: [f32; 3]) -> Quat {
    Quat::from_euler(bevy::math::EulerRot::XYZ, d[0].to_radians(), d[1].to_radians(), d[2].to_radians())
}

/// Pose d'une partie autour de son pivot : rotation, déplacement (cases), taille (slime).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub rot: Quat,
    pub pos: Vec3,
    pub scale: Vec3,
}

impl Default for Pose {
    fn default() -> Self {
        Self { rot: Quat::IDENTITY, pos: Vec3::ZERO, scale: Vec3::ONE }
    }
}

impl Pose {
    /// `self` puis `other` par-dessus (couche procédurale).
    pub fn then(&self, other: &Pose) -> Pose {
        Pose { rot: self.rot * other.rot, pos: self.pos + other.pos, scale: self.scale * other.scale }
    }

    pub fn lerp(&self, other: &Pose, f: f32) -> Pose {
        Pose { rot: self.rot.slerp(other.rot, f), pos: self.pos.lerp(other.pos, f), scale: self.scale.lerp(other.scale, f) }
    }
}

/// Pose d'une piste à l'instant `t` (boucle de `duration` s), dans le repère de la piste ;
/// `index` = rang dans une chaîne (1 = le premier maillon : l'onde s'y décale de `step`).
pub fn sample(track: &Track, t: f32, duration: f32, index: u32) -> Pose {
    sample_with(track, t, duration, index, &Inputs::default())
}

/// Comme `sample`, avec ce que donne le jeu (les pistes `vise` se calculent dans `part_pose`).
pub fn sample_with(track: &Track, t: f32, duration: f32, index: u32, inputs: &Inputs) -> Pose {
    match track {
        Track::Input { input, rot, scale, flicker } => {
            let v = match input.as_str() {
                "poussee" => inputs.thrust,
                "vitesse" => inputs.speed,
                "manoeuvre" => inputs.maneuver,
                _ => 0.0,
            }
            .clamp(0.0, 1.0);
            let r = euler(rot[0]).slerp(euler(rot[1]), v);
            let mut s = Vec3::from_array(scale[0]).lerp(Vec3::from_array(scale[1]), v);
            if *flicker > 0.0 && v > 0.0 {
                let n = (t * 31.0).sin() * 0.5 + (t * 17.3 + 1.0).sin() * 0.5;
                s *= 1.0 + flicker * n;
            }
            Pose { rot: r, scale: s, ..Pose::default() }
        }
        Track::Blink { period, on, phase } => {
            let lit = (t / period.max(0.05) - phase).rem_euclid(1.0) < *on;
            Pose { scale: if lit { Vec3::ONE } else { Vec3::splat(0.001) }, ..Pose::default() }
        }
        Track::Aim { .. } => Pose::default(),
        Track::Wave { axis, amplitude, period, phase, step, amp_step, base } => {
            let k = index.max(1) as f32 - 1.0;
            let a = Vec3::from_array(*axis).normalize_or(Vec3::Y);
            let x = (t / period.max(0.05) - phase - step * k) * std::f32::consts::TAU;
            Pose { rot: Quat::from_axis_angle(a, (amplitude + amp_step * k).to_radians() * x.sin()) * euler(*base), ..Pose::default() }
        }
        Track::Keys { rot, pos, scale } => {
            let d = duration.max(0.01);
            let u = t.rem_euclid(d);
            let r = keyed(rot, u).map_or(Quat::IDENTITY, |(a, b, f)| euler([a[1], a[2], a[3]]).slerp(euler([b[1], b[2], b[3]]), f));
            let v = |k: &[f32; 4]| Vec3::new(k[1], k[2], k[3]);
            let p = keyed(pos, u).map_or(Vec3::ZERO, |(a, b, f)| v(a).lerp(v(b), f));
            let s = keyed(scale, u).map_or(Vec3::ONE, |(a, b, f)| v(a).lerp(v(b), f));
            Pose { rot: r, pos: p, scale: s }
        }
    }
}

/// Les deux images clés autour de `u` et l'avancement entre elles (avant la première et après
/// la dernière : la plus proche).
fn keyed(keys: &[[f32; 4]], u: f32) -> Option<(&[f32; 4], &[f32; 4], f32)> {
    let first = keys.first()?;
    if u <= first[0] {
        return Some((first, first, 0.0));
    }
    for w in keys.windows(2) {
        if u <= w[1][0] {
            let f = (u - w[0][0]) / (w[1][0] - w[0][0]).max(1e-6);
            // Lissage : départ et arrivée en douceur
            return Some((&w[0], &w[1], f * f * (3.0 - 2.0 * f)));
        }
    }
    let last = keys.last()?;
    Some((last, last, 0.0))
}

impl Placement {
    /// Un vecteur du modèle dans le repère du bloc (l'inverse de `vector`).
    pub fn unvector(&self, v: Vec3) -> Vec3 {
        let v = self.rotation().inverse() * v;
        if self.mirror { Vec3::new(-v.x, v.y, v.z) } else { v }
    }

    /// Une pose du repère du bloc dans celui du modèle.
    pub fn pose(&self, p: Pose) -> Pose {
        // Un quart de tour échange les tailles en x et en z
        let s = if self.turn % 2 == 1 { Vec3::new(p.scale.z, p.scale.y, p.scale.x) } else { p.scale };
        Pose { rot: self.rotate(p.rot), pos: self.vector(p.pos) * self.scale(), scale: s }
    }
}

/// Pose d'une partie posée (repère du modèle) pour l'animation `anim` du bloc à l'instant `t` ;
/// `pivot` (repère du modèle) sert aux pistes qui visent.
pub fn part_pose(def: &BlockDef, part: &str, anim: &str, t: f32, place: &Placement, pivot: Vec3, inputs: &Inputs) -> Option<Pose> {
    let a = def.anims.get(anim)?;
    let track = a.tracks.get(part)?;
    if let Track::Aim { axis, forward, limits, input } = track {
        let v = if input == "poussee" { -inputs.steer } else { inputs.target - pivot };
        let angle = aim_angle(place.unvector(v), Vec3::from_array(*axis), Vec3::from_array(*forward)).clamp(limits[0].to_radians(), limits[1].to_radians());
        let a = Vec3::from_array(*axis).normalize_or(Vec3::Y);
        return Some(place.pose(Pose { rot: Quat::from_axis_angle(a, angle), ..Pose::default() }));
    }
    Some(place.pose(sample_with(track, t, a.duration, 1, inputs)))
}

/// Angle (radians) dont il faut tourner `forward` autour de `axis` pour viser `v` (repère du
/// bloc). Autour de x (tangage) : d'après la hauteur de la cible, quelle que soit sa direction.
pub fn aim_angle(v: Vec3, axis: Vec3, forward: Vec3) -> f32 {
    let a = axis.normalize_or(Vec3::Y);
    let (f, v) = if a.x.abs() > 0.9 {
        let flat = |x: Vec3| Vec3::new(0.0, x.y, Vec2::new(x.x, x.z).length());
        (flat(forward), flat(v))
    } else {
        (forward - a * a.dot(forward), v - a * a.dot(v))
    };
    if f.length_squared() < 1e-9 || v.length_squared() < 1e-9 {
        return 0.0;
    }
    // Tangage d'une pièce tournée vers l'arrière (tuyère) : le sens de rotation s'inverse
    let flip = if a.x.abs() > 0.9 && forward.z < 0.0 { -1.0 } else { 1.0 };
    flip * a.dot(f.cross(v)).atan2(f.dot(v))
}

// ─────────────────────────────────────────────────────────────────────────
//  Bibliothèque d'animations de personnage (§3.3) : par noms d'os standard
// ─────────────────────────────────────────────────────────────────────────

/// Une animation de la bibliothèque (`assets/editeur/anims/*.json`), dans le repère du modèle
/// (personnage tourné vers +z, côté gauche « _g » en x bas). Les pistes visent des **os** par
/// nom (`tete`, `bras_g`...) ou par motif de chaîne (`queue_*` : `queue_1`, `queue_2`...) ; un os
/// absent est ignoré, la même « marcher » sert donc à toutes les races.
#[derive(Clone, Debug, Deserialize)]
pub struct LibAnim {
    pub id: String,
    pub name: String,
    pub group: String,
    #[serde(default = "one")]
    pub duration: f32,
    /// Côté droit (« _d ») sans piste à lui : celle du côté gauche reflétée, décalée de cette
    /// fraction de boucle (0,5 : en opposition, comme les bras qui marchent).
    #[serde(default)]
    pub mirror: Option<f32>,
    /// Proposée seulement aux modèles qui ont un os commençant ainsi (« aile » pour voler).
    #[serde(default)]
    pub requires: Vec<String>,
    #[serde(default)]
    pub tracks: BTreeMap<String, Track>,
}

/// Groupe des animations toujours actives (respiration, queues, oreilles...).
pub const PROCEDURAL: &str = "Procedurales";

/// Groupe des variantes d'une famille (C2) : jouées à la place d'une animation par les races qui
/// les nomment (`RaceDef::anims`), jamais proposées seules.
pub const VARIANTS: &str = "Variantes";

/// L'animation `id` telle que la joue ce modèle : la variante de sa race s'il y en a une.
pub fn anim_for<'a>(lib: &'a Library, m: &Model, id: &str) -> Option<&'a LibAnim> {
    lib.race_of(m).and_then(|r| r.anims.get(id)).and_then(|v| lib.anim(v)).or_else(|| lib.anim(id))
}

/// Pose d'un os d'une race : par son nom, sinon par l'os qu'il remplace (`RaceDef::alias`).
fn race_pose(anim: &LibAnim, race: Option<&super::races::RaceDef>, bone: &str, t: f32) -> Option<Pose> {
    lib_pose(anim, bone, t).or_else(|| race?.alias.iter().find(|(_, v)| v.as_str() == bone).and_then(|(k, _)| lib_pose(anim, k, t)))
}

/// L'animation touche-t-elle cet os (directement ou par un alias) ?
fn touches(anim: &LibAnim, race: Option<&super::races::RaceDef>, bone: &str) -> bool {
    resolve(anim, bone).is_some() || race.is_some_and(|r| r.alias.iter().any(|(k, v)| v == bone && resolve(anim, k).is_some()))
}

/// Nom d'os d'une zone : son nom, espaces en « _ » (« avant bras g » -> `avant_bras_g`).
pub fn bone(z: &Zone) -> String {
    z.name.trim().replace(' ', "_")
}

/// Un motif `pre*post` correspond-il à l'os ? Renvoie le rang lu à la place de `*`.
fn pattern_index(pattern: &str, bone: &str) -> Option<u32> {
    let (pre, post) = pattern.split_once('*')?;
    let mid = bone.strip_prefix(pre)?.strip_suffix(post)?;
    if mid.is_empty() || !mid.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    mid.parse().ok()
}

/// La piste d'une animation pour un os : (piste, rang dans la chaîne, reflétée, décalage en
/// fraction de boucle). Ordre : nom exact, motif, puis le côté gauche reflété.
fn resolve<'a>(anim: &'a LibAnim, bone: &str) -> Option<(&'a Track, u32, bool, f32)> {
    let direct = |b: &str| -> Option<(&'a Track, u32)> {
        if let Some(t) = anim.tracks.get(b) {
            return Some((t, 1));
        }
        anim.tracks.iter().find_map(|(k, t)| pattern_index(k, b).map(|i| (t, i)))
    };
    if let Some((t, i)) = direct(bone) {
        return Some((t, i, false, 0.0));
    }
    let shift = anim.mirror?;
    let left = format!("{}_g", bone.strip_suffix("_d")?);
    direct(&left).map(|(t, i)| (t, i, true, shift))
}

/// Pose d'un os pour une animation de la bibliothèque (`None` : l'animation ne le touche pas).
pub fn lib_pose(anim: &LibAnim, bone: &str, t: f32) -> Option<Pose> {
    let (track, index, mirrored, shift) = resolve(anim, bone)?;
    let p = sample(track, t + shift * anim.duration, anim.duration, index);
    Some(if mirrored { Placement { turn: 0, mirror: true, scale: 1 }.pose(p) } else { p })
}

/// Tout ce qui anime : blocs de mouvement, bibliothèque d'animations, races.
#[derive(Clone, Debug, Default)]
pub struct Library {
    pub blocks: Vec<BlockDef>,
    pub anims: Vec<LibAnim>,
    pub races: Vec<super::races::RaceDef>,
}

impl Library {
    /// Ceux du jeu, puis ceux du joueur (`saves/editeur/{blocs,anims,races}/`) ; un fichier
    /// illisible est signalé, pas fatal.
    pub fn load(extra: Option<&std::path::Path>) -> (Self, Vec<String>) {
        let (blocks, mut errors) = load_blocks(extra.map(|d| d.join("blocs")).as_deref());
        let (anims, e2) = load_json::<LibAnim>(BUILTIN_ANIMS, extra.map(|d| d.join("anims")).as_deref(), |a| a.id.clone());
        let (races, e3) = load_json::<super::races::RaceDef>(BUILTIN_RACES, extra.map(|d| d.join("races")).as_deref(), |r| r.id.clone());
        errors.extend(e2);
        errors.extend(e3);
        (Self { blocks, anims, races }, errors)
    }

    pub fn anim(&self, id: &str) -> Option<&LibAnim> {
        self.anims.iter().find(|a| a.id == id)
    }

    pub fn block(&self, id: &str) -> Option<&BlockDef> {
        self.blocks.iter().find(|b| b.id == id)
    }

    /// La race d'un modèle (par son nom de famille).
    pub fn race_of(&self, m: &Model) -> Option<&super::races::RaceDef> {
        let r = m.race.as_deref()?;
        self.races.iter().find(|x| x.name == r || x.id == r)
    }
}

/// Fichiers JSON intégrés puis ceux d'un dossier (même id = remplace).
fn load_json<T: serde::de::DeserializeOwned>(builtin: &[(&str, &str)], extra: Option<&std::path::Path>, id: impl Fn(&T) -> String) -> (Vec<T>, Vec<String>) {
    let mut out: Vec<T> = Vec::new();
    let mut errors = Vec::new();
    for (file, text) in builtin {
        match serde_json::from_str::<T>(text) {
            Ok(x) => out.push(x),
            Err(e) => errors.push(format!("{file} : {e}")),
        }
    }
    if let Some(Ok(rd)) = extra.map(std::fs::read_dir) {
        let mut files: Vec<_> = rd.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|x| x == "json")).collect();
        files.sort();
        for p in files {
            match std::fs::read_to_string(&p).map_err(|e| e.to_string()).and_then(|t| serde_json::from_str::<T>(&t).map_err(|e| e.to_string())) {
                Ok(x) => {
                    let k = id(&x);
                    out.retain(|y| id(y) != k);
                    out.push(x);
                }
                Err(e) => errors.push(format!("{} : {e}", p.display())),
            }
        }
    }
    (out, errors)
}

// ─────────────────────────────────────────────────────────────────────────
//  Lecteur : poses des zones d'un modèle (repère du modèle)
// ─────────────────────────────────────────────────────────────────────────

/// Placement d'une zone posée.
pub fn zone_placement(z: &Zone) -> Placement {
    Placement { turn: z.turn % 4, mirror: z.mirror, scale: z.scale.max(1) }
}

/// Animations jouables du modèle : « repos » d'abord, puis celles de la bibliothèque qui touchent
/// un de ses os (dans l'ordre des groupes), puis celles de ses blocs.
pub fn model_anims(m: &Model, lib: &Library) -> Vec<String> {
    let mut out = vec!["repos".to_string()];
    // Les animations créées dans l'éditeur (E8)
    out.extend(m.anims.keys().filter(|k| k.as_str() != "repos").cloned());
    let bones: Vec<String> = m.zones.iter().map(bone).collect();
    let race = lib.race_of(m);
    for a in lib.anims.iter().filter(|a| a.group != PROCEDURAL && a.group != VARIANTS) {
        // La variante de la famille, s'il y en a une ; une animation qui ne touche aucun os n'est
        // pas proposée
        let Some(eff) = anim_for(lib, m, &a.id) else { continue };
        let fits = eff.requires.is_empty() || bones.iter().any(|b| eff.requires.iter().any(|r| b.starts_with(r.as_str())));
        if fits && !out.contains(&a.id) && bones.iter().any(|b| touches(eff, race, b)) {
            out.push(a.id.clone());
        }
    }
    // États du vaisseau, si un bloc en a ; entrée et sortie des hangars
    if m.zones.iter().filter_map(|z| lib.block(&z.block)).any(|d| !d.states.is_empty()) {
        out.extend(SHIP_STATES.iter().map(|(s, _)| format!("{STATE_PREFIX}{s}")));
    }
    if !m.hangars.is_empty() {
        out.push(HANGAR_IN.into());
        out.push(HANGAR_OUT.into());
    }
    for z in &m.zones {
        if let Some(def) = lib.block(&z.block) {
            // Les poses des états (« rentre », « eteint »...) se voient par les états
            for a in def.anims.keys().filter(|a| !def.states.values().any(|s| s == *a)) {
                if !out.contains(a) {
                    out.push(a.clone());
                }
            }
        }
    }
    out
}

/// Nom affiché d'une animation de l'aperçu.
pub fn anim_label(lib: &Library, id: &str) -> String {
    if let Some(s) = id.strip_prefix(STATE_PREFIX) {
        return format!("Etat : {}", SHIP_STATES.iter().find(|(k, _)| *k == s).map_or(s, |(_, n)| n));
    }
    match id {
        HANGAR_IN => "Hangar : entree".into(),
        HANGAR_OUT => "Hangar : sortie".into(),
        _ => lib.anim(id).map_or(id.to_string(), |a| a.name.clone()),
    }
}

/// L'animation que joue le bloc d'une zone : celle de l'état demandé (`etat:vol`), sinon `anim`.
fn block_anim<'a>(def: &'a BlockDef, anim: &'a str) -> &'a str {
    match anim.strip_prefix(STATE_PREFIX) {
        Some(s) => def.states.get(s).map_or("repos", String::as_str),
        None => anim,
    }
}

/// Temps de la porte d'un hangar pendant une entrée ou une sortie de `HANGAR_SECS` : elle joue
/// « ouverture » (4 s : s'ouvre en 1 s, reste ouverte, se ferme en 1 s) étirée sur la séquence.
fn hangar_door_time(s: f32) -> f32 {
    let d = HANGAR_SECS;
    let s = s.rem_euclid(d);
    if s < 1.0 {
        s
    } else if s < d - 1.0 {
        1.0 + (s - 1.0) / (d - 2.0) * 2.0
    } else {
        3.0 + (s - (d - 1.0))
    }
}

/// Le vaisseau d'un hangar pendant une entrée (`entering`) ou une sortie, à l'instant `s` :
/// (position, direction du nez), dans le repère du modèle. Il attend dehors (entrée) ou à sa place
/// (sortie) pendant que la porte s'ouvre.
pub fn hangar_ship(h: &super::format::Hangar, entering: bool, s: f32) -> Option<(Vec3, Vec3)> {
    let pts: Vec<Vec3> = h.path.iter().map(|p| Vec3::from_array(*p)).collect();
    if pts.len() < 2 {
        return None;
    }
    let s = s.rem_euclid(HANGAR_SECS);
    let f = ((s - 1.0) / (HANGAR_SECS - 2.0)).clamp(0.0, 1.0);
    let f = f * f * (3.0 - 2.0 * f);
    let f = if entering { f } else { 1.0 - f };
    let lens: Vec<f32> = pts.windows(2).map(|w| w[0].distance(w[1])).collect();
    let total: f32 = lens.iter().sum::<f32>().max(1e-3);
    let mut at = f * total;
    for (i, l) in lens.iter().enumerate() {
        if at <= *l || i == lens.len() - 1 {
            let k = (at / l.max(1e-3)).clamp(0.0, 1.0);
            let pos = pts[i].lerp(pts[i + 1], k);
            // Le nez suit le chemin, et finit tourné comme la place
            let along = (pts[i + 1] - pts[i]).normalize_or(Vec3::Z);
            let facing = Vec3::from_array(h.facing).normalize_or(along);
            let dir = along.lerp(facing, (f * 1.5 - 0.5).clamp(0.0, 1.0)).normalize_or(along);
            return Some((pos, dir));
        }
        at -= l;
    }
    None
}

/// Vitesse des animations du modèle (golems : plus lentes).
pub fn model_speed(m: &Model, lib: &Library) -> f32 {
    lib.race_of(m).map_or(1.0, |r| r.speed)
}

/// Durée d'une boucle de l'animation dans ce modèle (la plus longue de ses sources).
pub fn anim_duration(m: &Model, lib: &Library, anim: &str) -> f32 {
    let blocks = m
        .zones
        .iter()
        .filter_map(|z| lib.block(&z.block))
        .filter_map(|d| d.anims.get(anim).or_else(|| d.anims.get("repos")))
        .map(|a| a.duration)
        .fold(0.0, f32::max);
    if anim == HANGAR_IN || anim == HANGAR_OUT {
        return HANGAR_SECS;
    }
    if let Some(a) = m.anims.get(anim) {
        return (a.duration / model_speed(m, lib)).max(0.1);
    }
    let library = anim_for(lib, m, anim).map_or(0.0, |a| a.duration);
    let states = m.zones.iter().filter_map(|z| lib.block(&z.block)).filter_map(|d| d.anims.get(block_anim(d, anim))).map(|a| a.duration).fold(0.0, f32::max);
    (blocks.max(library).max(states) / model_speed(m, lib)).max(0.5)
}

/// Mouvement propre de chaque zone (autour de son pivot, sans ses parents). Pour chaque zone :
/// l'animation de son bloc, sinon celle de la bibliothèque, sinon le repos (du bloc, puis de la
/// bibliothèque) ; avec `procedural`, les animations toujours actives s'ajoutent par-dessus.
pub fn zone_locals(m: &Model, lib: &Library, anim: &str, t: f32, procedural: bool) -> Vec<Pose> {
    zone_locals_with(m, lib, anim, t, procedural, &Inputs::default())
}

/// Comme `zone_locals`, avec ce que donne le jeu (poussée, cible...). Pendant une entrée ou une
/// sortie de hangar, les portes des hangars s'ouvrent puis se ferment.
pub fn zone_locals_with(m: &Model, lib: &Library, anim: &str, t: f32, procedural: bool, inputs: &Inputs) -> Vec<Pose> {
    let doors: Vec<usize> = if anim == HANGAR_IN || anim == HANGAR_OUT { m.hangars.iter().filter_map(|h| h.door.map(|d| d as usize)).collect() } else { Vec::new() };
    let t = t * model_speed(m, lib);
    let own = m.anims.get(anim);
    let race = lib.race_of(m);
    let wanted = anim_for(lib, m, anim);
    let rest = anim_for(lib, m, "repos");
    let layers: Vec<&LibAnim> = if procedural { lib.anims.iter().filter(|a| a.group == PROCEDURAL).collect() } else { Vec::new() };
    m.zones
        .iter()
        .enumerate()
        .map(|(i, z)| {
            let b = bone(z);
            let def = lib.block(&z.block);
            let place = zone_placement(z);
            let pivot = Vec3::from_array(z.pivot);
            let (anim, t) = if doors.contains(&i) { ("ouverture", hangar_door_time(t)) } else { (anim, t) };
            // Animation du modèle (E8) : elle seule compte ; un os sans clés ne bouge pas
            if let Some(a) = own {
                let base = a.keys.get(&b).map_or(Pose::default(), |k| sample(&Track::Keys { rot: k.clone(), pos: Vec::new(), scale: Vec::new() }, t, a.duration, 1));
                return layers.iter().filter_map(|a| race_pose(a, race, &b, t)).fold(base, |p, q| p.then(&q));
            }
            // Le bloc a cette animation : elle seule compte (une partie sans piste ne bouge pas)
            if let Some(d) = def.filter(|d| d.anims.contains_key(block_anim(d, anim))) {
                let base = part_pose(d, &z.part, block_anim(d, anim), t, &place, pivot, inputs).unwrap_or_default();
                return layers.iter().filter_map(|a| race_pose(a, race, &b, t)).fold(base, |p, q| p.then(&q));
            }
            let base = def
                .and_then(|d| part_pose(d, &z.part, block_anim(d, anim), t, &place, pivot, inputs))
                .or_else(|| wanted.and_then(|a| race_pose(a, race, &b, t)))
                .or_else(|| def.and_then(|d| part_pose(d, &z.part, "repos", t, &place, pivot, inputs)))
                .or_else(|| rest.and_then(|a| race_pose(a, race, &b, t)))
                .unwrap_or_default();
            layers.iter().filter_map(|a| race_pose(a, race, &b, t)).fold(base, |p, q| p.then(&q))
        })
        .collect()
}

/// Mélange de deux poses (changement d'animation en douceur) : 0 = `a`, 1 = `b`.
pub fn blend(a: &[Pose], b: &[Pose], f: f32) -> Vec<Pose> {
    a.iter().zip(b).map(|(x, y)| x.lerp(y, f)).collect()
}

/// Matrices des zones dans le repère du modèle (index 0 = corps fixe, `i + 1` = zone `i`) :
/// chaque zone tourne autour de son pivot, emportée par sa parente.
pub fn compose(m: &Model, locals: &[Pose]) -> Vec<Mat4> {
    let mut out = vec![Mat4::IDENTITY; m.zones.len() + 1];
    for (i, z) in m.zones.iter().enumerate() {
        let p = locals.get(i).copied().unwrap_or_default();
        let c = Vec3::from_array(z.pivot);
        let local = Mat4::from_translation(c + p.pos) * Mat4::from_quat(p.rot) * Mat4::from_scale(p.scale) * Mat4::from_translation(-c);
        // Parents d'abord (les blocs et les races sont posés ainsi) ; un parent après est ignoré
        let parent = z.parent.map(|k| k as usize).filter(|k| *k < i).map_or(Mat4::IDENTITY, |k| out[k + 1]);
        out[i + 1] = parent * local;
    }
    out
}

/// Zones qui traversent le corps fixe pendant l'animation (16 instants d'une boucle) ; les cases
/// proches de l'articulation (pivot) ne comptent pas.
pub fn collisions(m: &Model, lib: &Library, anim: &str) -> Vec<usize> {
    let mut hit = vec![false; m.zones.len()];
    let cells: Vec<(IVec3, usize)> = m.zone_map.iter().map(|(p, z)| (p, z as usize)).filter(|(_, z)| *z > 0 && *z <= m.zones.len()).collect();
    let d = anim_duration(m, lib, anim);
    for i in 0..16 {
        let mats = compose(m, &zone_locals(m, lib, anim, d * i as f32 / 16.0, false));
        for (p, z) in &cells {
            if hit[z - 1] {
                continue;
            }
            let zone = &m.zones[z - 1];
            let mat = mats[*z];
            let c = mat.transform_point3(p.as_vec3() + Vec3::splat(0.5));
            let pivot = mat.transform_point3(Vec3::from_array(zone.pivot));
            if c.distance(pivot) < 1.5 * zone.scale.max(1) as f32 {
                continue;
            }
            let q = c.floor().as_ivec3();
            if m.voxels.get(q) != 0 && m.zone_map.get(q) == 0 {
                hit[z - 1] = true;
            }
        }
    }
    hit.iter().enumerate().filter(|(_, h)| **h).map(|(i, _)| i).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defs() -> Vec<BlockDef> {
        let (b, errors) = load_blocks(None);
        assert!(errors.is_empty(), "{errors:?}");
        b
    }

    #[test]
    fn every_builtin_block_is_valid() {
        let blocks = defs();
        assert!(blocks.len() >= 10, "{}", blocks.len());
        for b in &blocks {
            assert!(!b.parts.is_empty(), "{}", b.id);
            let names: Vec<&str> = b.parts.iter().map(|p| p.name.as_str()).collect();
            for (i, p) in b.parts.iter().enumerate() {
                // Parent connu et défini avant (les parents d'abord)
                if let Some(parent) = &p.parent {
                    let k = names.iter().position(|n| n == parent).unwrap_or_else(|| panic!("{} : parent {parent} inconnu", b.id));
                    assert!(k < i, "{} : {} avant son parent", b.id, p.name);
                }
                assert!(!part_cells(p).is_empty(), "{} : {} vide", b.id, p.name);
            }
            // Chaque piste vise une partie qui existe
            for (anim, a) in &b.anims {
                for part in a.tracks.keys() {
                    assert!(names.contains(&part.as_str()), "{} / {anim} : partie {part} inconnue", b.id);
                }
            }
            assert!(b.anims.contains_key("repos"), "{} sans animation de repos", b.id);
            // États du vaisseau : connus, vers des animations qui existent
            for (s, a) in &b.states {
                assert!(SHIP_STATES.iter().any(|(k, _)| k == s), "{} : etat {s} inconnu", b.id);
                assert!(b.anims.contains_key(a), "{} : etat {s} -> {a} absente", b.id);
            }
            if let Some(h) = &b.hangar {
                assert!(names.contains(&h.door.as_str()), "{} : porte {} absente", b.id, h.door);
                assert!(h.path.len() >= 2 && b.anims.contains_key("ouverture"), "{}", b.id);
            }
        }
        // Les ids sont uniques
        let mut ids: Vec<&str> = blocks.iter().map(|b| b.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), blocks.len());
    }

    #[test]
    fn the_anchor_lands_on_the_aimed_cell_in_every_orientation() {
        let b = &defs()[0];
        let at = IVec3::new(5, 6, 7);
        let a = IVec3::from_array(b.anchor);
        for turn in 0..4 {
            for mirror in [false, true] {
                for scale in 1..=3 {
                    let p = Placement { turn, mirror, scale };
                    let cells = p.cells(b, at, a);
                    assert!(cells.contains(&at), "{p:?} : {cells:?}");
                    assert_eq!(cells.len(), (scale as usize).pow(3));
                }
            }
        }
        // Quatre quarts de tour : retour au départ
        let p = Placement { turn: 1, mirror: false, scale: 1 };
        let v = Vec3::new(1.0, 2.0, 3.0);
        let r = p.rotation() * p.rotation() * p.rotation() * p.rotation() * v;
        assert!((r - v).length() < 1e-5);
    }

    #[test]
    fn mirrored_rotations_mirror_the_motion() {
        // Un bras qui se lève vers +x devient un bras qui se lève vers -x
        let lift = Quat::from_rotation_z(0.8);
        let tip = Vec3::new(0.0, -1.0, 0.0);
        let p = Placement { turn: 0, mirror: true, scale: 1 };
        let a = lift * tip;
        let b = p.rotate(lift) * p.vector(tip);
        assert!((b - Vec3::new(-a.x, a.y, a.z)).length() < 1e-5, "{a} {b}");
        // Tourné d'un quart : la même chose tournée
        let t = Placement { turn: 1, mirror: false, scale: 1 };
        let c = t.rotate(lift) * t.vector(tip);
        assert!((c - t.rotation() * a).length() < 1e-5);
    }

    #[test]
    fn animations_loop_and_waves_ripple() {
        let keys = Track::Keys { rot: vec![[0.0, 0.0, 0.0, 0.0], [1.0, 90.0, 0.0, 0.0], [2.0, 0.0, 0.0, 0.0]], pos: vec![], scale: vec![] };
        let q = sample(&keys, 1.0, 2.0, 1).rot;
        assert!((q.angle_between(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2))).abs() < 1e-3);
        let q0 = sample(&keys, 0.0, 2.0, 1).rot;
        let q2 = sample(&keys, 2.0, 2.0, 1).rot;
        assert!(q0.angle_between(q2) < 1e-3, "boucle");
        let wave = |phase| Track::Wave { axis: [0.0, 1.0, 0.0], amplitude: 30.0, period: 2.0, phase, step: 0.25, amp_step: 0.0, base: [0.0; 3] };
        let a = sample(&wave(0.0), 0.5, 1.0, 1).rot;
        let b = sample(&wave(0.25), 0.5, 1.0, 1).rot;
        // Le 2e maillon d'une chaîne : décalé de `step`
        let c = sample(&wave(0.0), 0.5, 1.0, 2).rot;
        assert!(c.angle_between(b) < 1e-3, "chaine");
        assert!(a.angle_between(Quat::IDENTITY) > 0.4 && b.angle_between(Quat::IDENTITY) < 1e-3, "dephasage");
    }
    fn block(id: &str) -> BlockDef {
        defs().into_iter().find(|b| b.id == id).unwrap()
    }

    #[test]
    fn a_door_that_sweeps_through_the_hull_is_reported() {
        use super::super::edit::Doc;
        use super::super::format::{Material, ModelKind, PaletteEntry};
        let lib = Library { blocks: defs(), ..Default::default() };
        let door = block("porte_pivotante");
        let mut d = Doc::new(Model::new("v", ModelKind::Autre, None), None);
        let at = IVec3::new(10, 2, 10);
        d.place_block(&door, at, Placement { turn: 0, mirror: false, scale: 1 }, false).unwrap();
        assert_eq!(d.model.zones.len(), 1);
        assert!(collisions(&d.model, &lib, "ouverture").is_empty());
        // Au repos, rien ne bouge
        let rest = compose(&d.model, &zone_locals(&d.model, &lib, "repos", 0.3, false));
        assert!(rest[1].abs_diff_eq(Mat4::IDENTITY, 1e-5));
        // Une coque là où passe le bord de la porte ouverte
        let open = compose(&d.model, &zone_locals(&d.model, &lib, "ouverture", 2.0, false));
        let edge = open[1].transform_point3(at.as_vec3() + Vec3::new(0.5, 3.5, 4.5));
        let hull = d.model.color_index(PaletteEntry { rgb: [90, 90, 90], material: Material::Metal }).unwrap();
        d.set(edge.floor().as_ivec3(), hull, false);
        assert_eq!(collisions(&d.model, &lib, "ouverture"), vec![0]);
    }

    #[test]
    fn nested_zones_follow_their_parent() {
        use super::super::edit::Doc;
        use super::super::format::ModelKind;
        let lib = Library { blocks: defs(), ..Default::default() };
        let arm = block("bras");
        let mut d = Doc::new(Model::new("p", ModelKind::Personnage, None), None);
        d.place_block(&arm, IVec3::new(3, 20, 8), Placement { turn: 0, mirror: false, scale: 1 }, true).unwrap();
        // Bras, avant-bras, main, des deux côtés
        assert_eq!(d.model.zones.len(), 6);
        assert_eq!(d.model.zones[2].parent, Some(1));
        assert_eq!(d.model.zones[5].parent, Some(4));
        let mats = compose(&d.model, &zone_locals(&d.model, &lib, "saluer", 0.8, false));
        // La main suit le bras levé : elle monte
        let hand = Vec3::from_array(d.model.zones[2].pivot);
        assert!(mats[3].transform_point3(hand).y > hand.y + 5.0);
        // Le bras reflété salue de l'autre côté
        let a = mats[3].transform_point3(hand) - hand;
        let hand2 = Vec3::from_array(d.model.zones[5].pivot);
        let b = mats[6].transform_point3(hand2) - hand2;
        assert!((a.x + b.x).abs() < 1e-3 && (a.y - b.y).abs() < 1e-3, "{a} {b}");
        assert!(model_anims(&d.model, &lib).starts_with(&["repos".to_string()]));
    }

    fn ship(cat: super::super::format::ShipCategory) -> super::super::edit::Doc {
        use super::super::format::ModelKind;
        super::super::edit::Doc::new(Model::new("v", ModelKind::Vaisseau, Some(cat)), None)
    }

    fn lib_blocks() -> Library {
        Library { blocks: defs(), ..Default::default() }
    }

    /// Direction d'un vecteur d'une zone après sa pose.
    fn turned(m: &Model, mats: &[Mat4], zone: usize, v: Vec3) -> Vec3 {
        let _ = m;
        mats[zone + 1].transform_vector3(v).normalize()
    }

    #[test]
    fn turrets_aim_and_nozzles_follow_the_thrust() {
        use super::super::format::ShipCategory;
        let lib = lib_blocks();
        let mut d = ship(ShipCategory::Chasseur);
        d.place_block(&block("tourelle"), IVec3::new(20, 10, 20), Placement { turn: 1, mirror: false, scale: 1 }, false).unwrap();
        let m = &d.model;
        let pivot = Vec3::from_array(m.zones[1].pivot);
        // Cible à droite et en hauteur : le canon (vers +z dans son bloc, tourné d'un quart) la vise
        let target = pivot + Vec3::new(30.0, 20.0, -5.0);
        let inputs = Inputs { target, ..Inputs::default() };
        let mats = compose(m, &zone_locals_with(m, &lib, "repos", 0.0, false, &inputs));
        let gun = turned(m, &mats, 1, Placement { turn: 1, mirror: false, scale: 1 }.vector(Vec3::Z));
        let want = (target - pivot).normalize();
        assert!(gun.dot(want) > 0.97, "{gun} {want}");
        // Rangée (vol) : elle ne vise plus
        let still = compose(m, &zone_locals_with(m, &lib, "etat:vol", 0.0, false, &inputs));
        assert!(still[1].abs_diff_eq(Mat4::IDENTITY, 1e-4) && still[2].abs_diff_eq(Mat4::IDENTITY, 1e-4));
        // Tuyère : on pousse vers le haut, elle souffle vers le bas (dans ses limites)
        let mut d = ship(ShipCategory::Chasseur);
        d.place_block(&block("tuyere_orientable"), IVec3::new(30, 30, 30), Placement::default(), false).unwrap();
        let m = &d.model;
        let up = Inputs { steer: Vec3::new(0.0, 1.0, 1.0).normalize(), thrust: 1.0, ..Inputs::default() };
        let mats = compose(m, &zone_locals_with(m, &lib, "repos", 0.0, false, &up));
        let exhaust = turned(m, &mats, 1, Vec3::NEG_Z);
        assert!(exhaust.y < -0.3 && exhaust.y > -0.5, "{exhaust}");
    }

    #[test]
    fn flames_follow_the_thrust_and_lights_blink() {
        use super::super::format::ShipCategory;
        let lib = lib_blocks();
        let mut d = ship(ShipCategory::Chasseur);
        d.place_block(&block("propulseur"), IVec3::new(30, 30, 30), Placement::default(), false).unwrap();
        d.place_block(&block("feu"), IVec3::new(10, 30, 30), Placement::default(), false).unwrap();
        let m = &d.model;
        let flame = |thrust: f32, anim: &str, t: f32| zone_locals_with(m, &lib, anim, t, false, &Inputs { thrust, ..Inputs::default() });
        assert!(flame(0.0, "repos", 0.0)[1].scale.z < 0.1);
        assert!(flame(1.0, "repos", 0.0)[1].scale.z > 0.8);
        // Posé : éteint, même à pleine poussée
        assert!(flame(1.0, "etat:pose", 0.0)[1].scale.z < 0.01);
        // Le feu : allumé un instant, puis éteint
        assert!(flame(0.0, "repos", 0.05)[2].scale.x > 0.9);
        assert!(flame(0.0, "repos", 0.6)[2].scale.x < 0.01);
        // Les flammes ont leur couleur lumineuse
        assert!(m.palette.iter().any(|e| e.material == super::super::format::Material::Lumineuse));
    }

    #[test]
    fn ship_states_move_the_blocks() {
        use super::super::format::ShipCategory;
        let lib = lib_blocks();
        let mut d = ship(ShipCategory::Chasseur);
        d.place_block(&block("train"), IVec3::new(20, 10, 20), Placement::default(), false).unwrap();
        d.place_block(&block("ailes_repliables"), IVec3::new(40, 20, 20), Placement::default(), false).unwrap();
        let m = &d.model;
        let anims = model_anims(m, &lib);
        assert!(anims.contains(&"etat:combat".to_string()) && !anims.contains(&HANGAR_IN.to_string()));
        let pose = |s: &str| zone_locals_with(m, &lib, &format!("etat:{s}"), 0.0, false, &Inputs::default());
        // Posé : train sorti, ailes repliées ; en vol : train rentré, ailes dépliées
        assert!(pose("pose")[0].rot.angle_between(Quat::IDENTITY) < 1e-3);
        assert!(pose("pose")[2].rot.angle_between(Quat::IDENTITY) > 1.4);
        assert!(pose("vol")[0].rot.angle_between(Quat::IDENTITY) > 1.5);
        assert!(pose("vol")[2].rot.angle_between(Quat::IDENTITY) < 1e-3);
        assert_eq!(anim_label(&lib, "etat:atterrissage"), "Etat : Atterrissage");
    }

    #[test]
    fn hangars_take_the_right_ships_and_play_entry_and_exit() {
        use super::super::format::ShipCategory;
        let lib = lib_blocks();
        // Un chasseur n'a pas de hangar ; un croiseur pas de soute à cargos
        assert!(ship(ShipCategory::Chasseur).place_block(&block("hangar_chasseur"), IVec3::new(32, 0, 32), Placement::default(), false).is_err());
        assert!(ship(ShipCategory::Croiseur).place_block(&block("soute_cargo"), IVec3::new(256, 0, 256), Placement::default(), false).is_err());
        assert!(ship(ShipCategory::Croiseur).place_block(&block("hangar_corvette"), IVec3::new(256, 0, 256), Placement::default(), false).is_err());
        let mut d = ship(ShipCategory::Capital);
        d.place_block(&block("soute_cargo"), IVec3::new(512, 100, 700), Placement { turn: 2, mirror: false, scale: 1 }, false).unwrap();
        let h = d.model.hangars[0].clone();
        assert!(h.cargo && h.category == ShipCategory::Fregate && h.door == Some(0));
        // Tourné d'un demi-tour : la place est derrière la porte (vers +z)
        assert!(h.slot[2] > 700.0 + 150.0, "{:?}", h.slot);
        // Entrée : dehors pendant que la porte s'ouvre, à sa place à la fin ; la porte est ouverte au milieu
        let (start, _) = hangar_ship(&h, true, 0.5).unwrap();
        let (end, nose) = hangar_ship(&h, true, HANGAR_SECS - 0.2).unwrap();
        assert!(start.distance(Vec3::from_array(h.path[0])) < 1.0);
        assert!(end.distance(Vec3::from_array(h.slot)) < 1.0);
        assert!(nose.dot(Vec3::from_array(h.facing)) > 0.99);
        let (out, _) = hangar_ship(&h, false, HANGAR_SECS - 0.2).unwrap();
        assert!(out.distance(Vec3::from_array(h.path[0])) < 1.0);
        let m = &d.model;
        let door = |s: f32| zone_locals_with(m, &lib, HANGAR_IN, s, false, &Inputs::default())[0].pos.y;
        assert!(door(0.0).abs() < 1e-3 && door(4.0) > 100.0 && door(HANGAR_SECS - 0.01).abs() < 2.0);
        assert!(model_anims(m, &lib).contains(&HANGAR_OUT.to_string()));
        // Chemin prolongé, gardé dans le fichier, annulable
        d.extend_path(0, Vec3::new(512.0, 400.0, 100.0));
        assert_eq!(d.model.hangars[0].path.len(), h.path.len() + 1);
        let back = Model::from_bytes(&d.model.to_bytes().unwrap()).unwrap();
        assert_eq!(back.hangars, d.model.hangars);
        assert!(d.undo() && d.undo());
        assert!(d.model.hangars.is_empty());
    }
}
