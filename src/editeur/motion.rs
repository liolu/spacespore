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
//! - E5 : bibliothèque d'animations de personnage (`assets/editeur/anims/`, §3.3) par noms d'os,
//!   chaînes (`queue_*`), côté droit reflété du gauche, couche procédurale toujours active, taille
//!   animée ; `Library` réunit blocs, animations et races (`races.rs`).

use bevy::math::{IVec3, Mat4, Quat, Vec3};
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
    },
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
    match track {
        Track::Wave { axis, amplitude, period, phase, step, amp_step } => {
            let k = index.max(1) as f32 - 1.0;
            let a = Vec3::from_array(*axis).normalize_or(Vec3::Y);
            let x = (t / period.max(0.05) - phase - step * k) * std::f32::consts::TAU;
            Pose { rot: Quat::from_axis_angle(a, (amplitude + amp_step * k).to_radians() * x.sin()), ..Pose::default() }
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
    /// Une pose du repère du bloc dans celui du modèle.
    pub fn pose(&self, p: Pose) -> Pose {
        // Un quart de tour échange les tailles en x et en z
        let s = if self.turn % 2 == 1 { Vec3::new(p.scale.z, p.scale.y, p.scale.x) } else { p.scale };
        Pose { rot: self.rotate(p.rot), pos: self.vector(p.pos) * self.scale(), scale: s }
    }
}

/// Pose d'une partie posée (repère du modèle) pour l'animation `anim` du bloc à l'instant `t`.
pub fn part_pose(def: &BlockDef, part: &str, anim: &str, t: f32, place: &Placement) -> Option<Pose> {
    let a = def.anims.get(anim)?;
    let track = a.tracks.get(part)?;
    Some(place.pose(sample(track, t, a.duration, 1)))
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
    let bones: Vec<String> = m.zones.iter().map(bone).collect();
    for a in &lib.anims {
        let fits = a.requires.is_empty() || bones.iter().any(|b| a.requires.iter().any(|r| b.starts_with(r.as_str())));
        if a.group != PROCEDURAL && fits && !out.contains(&a.id) && bones.iter().any(|b| resolve(a, b).is_some()) {
            out.push(a.id.clone());
        }
    }
    for z in &m.zones {
        if let Some(def) = lib.block(&z.block) {
            for a in def.anims.keys() {
                if !out.contains(a) {
                    out.push(a.clone());
                }
            }
        }
    }
    out
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
    let library = lib.anim(anim).map_or(0.0, |a| a.duration);
    (blocks.max(library) / model_speed(m, lib)).max(0.5)
}

/// Mouvement propre de chaque zone (autour de son pivot, sans ses parents). Pour chaque zone :
/// l'animation de son bloc, sinon celle de la bibliothèque, sinon le repos (du bloc, puis de la
/// bibliothèque) ; avec `procedural`, les animations toujours actives s'ajoutent par-dessus.
pub fn zone_locals(m: &Model, lib: &Library, anim: &str, t: f32, procedural: bool) -> Vec<Pose> {
    let t = t * model_speed(m, lib);
    let wanted = lib.anim(anim);
    let rest = lib.anim("repos");
    let layers: Vec<&LibAnim> = if procedural { lib.anims.iter().filter(|a| a.group == PROCEDURAL).collect() } else { Vec::new() };
    m.zones
        .iter()
        .map(|z| {
            let b = bone(z);
            let def = lib.block(&z.block);
            let place = zone_placement(z);
            let base = def
                .and_then(|d| part_pose(d, &z.part, anim, t, &place))
                .or_else(|| wanted.and_then(|a| lib_pose(a, &b, t)))
                .or_else(|| def.and_then(|d| part_pose(d, &z.part, "repos", t, &place)))
                .or_else(|| rest.and_then(|a| lib_pose(a, &b, t)))
                .unwrap_or_default();
            layers.iter().filter_map(|a| lib_pose(a, &b, t)).fold(base, |p, q| p.then(&q))
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
        let wave = |phase| Track::Wave { axis: [0.0, 1.0, 0.0], amplitude: 30.0, period: 2.0, phase, step: 0.25, amp_step: 0.0 };
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
}
