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

use bevy::math::{IVec3, Mat4, Quat, Vec3};
use super::format::{Model, Zone};
use serde::Deserialize;
use std::collections::BTreeMap;

include!(concat!(env!("OUT_DIR"), "/blocs.rs"));

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
    /// Images clés : [t, rx, ry, rz] (degrés, angles d'Euler XYZ) et [t, x, y, z] (cases).
    #[serde(rename = "cles")]
    Keys {
        #[serde(default)]
        rot: Vec<[f32; 4]>,
        #[serde(default)]
        pos: Vec<[f32; 4]>,
    },
    /// Onde : rotation autour de `axis`, amplitude (degrés), période (s), déphasage (fraction).
    #[serde(rename = "onde")]
    Wave {
        axis: [f32; 3],
        amplitude: f32,
        period: f32,
        #[serde(default)]
        phase: f32,
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

    fn scale(&self) -> f32 {
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

/// Pose d'une piste à l'instant `t` (boucle de `duration` s) : rotation et déplacement (cases),
/// dans le repère du bloc.
pub fn sample(track: &Track, t: f32, duration: f32) -> (Quat, Vec3) {
    match track {
        Track::Wave { axis, amplitude, period, phase } => {
            let a = Vec3::from_array(*axis).normalize_or(Vec3::Y);
            let x = (t / period.max(0.05) - phase) * std::f32::consts::TAU;
            (Quat::from_axis_angle(a, amplitude.to_radians() * x.sin()), Vec3::ZERO)
        }
        Track::Keys { rot, pos } => {
            let d = duration.max(0.01);
            let u = t.rem_euclid(d);
            let r = keyed(rot, u).map_or(Quat::IDENTITY, |(a, b, f)| euler([a[1], a[2], a[3]]).slerp(euler([b[1], b[2], b[3]]), f));
            let p = keyed(pos, u).map_or(Vec3::ZERO, |(a, b, f)| Vec3::new(a[1], a[2], a[3]).lerp(Vec3::new(b[1], b[2], b[3]), f));
            (r, p)
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

/// Pose d'une partie posée (repère du modèle) pour l'animation `anim` à l'instant `t`.
pub fn part_pose(def: &BlockDef, part: &str, anim: &str, t: f32, place: &Placement) -> (Quat, Vec3) {
    let Some(a) = def.anims.get(anim) else { return (Quat::IDENTITY, Vec3::ZERO) };
    let Some(track) = a.tracks.get(part) else { return (Quat::IDENTITY, Vec3::ZERO) };
    let (q, p) = sample(track, t, a.duration);
    (place.rotate(q), place.vector(p) * place.scale())
}

// ─────────────────────────────────────────────────────────────────────────
//  Lecteur : poses des zones d'un modèle (repère du modèle)
// ─────────────────────────────────────────────────────────────────────────

/// Placement d'une zone posée.
pub fn zone_placement(z: &Zone) -> Placement {
    Placement { turn: z.turn % 4, mirror: z.mirror, scale: z.scale.max(1) }
}

/// Animations jouables du modèle : « repos » d'abord, puis celles de ses blocs.
pub fn model_anims(m: &Model, blocks: &[BlockDef]) -> Vec<String> {
    let mut out = vec!["repos".to_string()];
    for z in &m.zones {
        if let Some(def) = blocks.iter().find(|b| b.id == z.block) {
            for a in def.anims.keys() {
                if !out.contains(a) {
                    out.push(a.clone());
                }
            }
        }
    }
    out
}

/// Durée d'une boucle de l'animation dans ce modèle (la plus longue de ses blocs).
pub fn anim_duration(m: &Model, blocks: &[BlockDef], anim: &str) -> f32 {
    m.zones
        .iter()
        .filter_map(|z| blocks.iter().find(|b| b.id == z.block))
        .filter_map(|d| d.anims.get(anim).or_else(|| d.anims.get("repos")))
        .map(|a| a.duration)
        .fold(0.0, f32::max)
        .max(0.5)
}

/// Mouvement propre de chaque zone (rotation autour de son pivot, déplacement), sans ses
/// parents. Un bloc sans cette animation joue son repos ; un bloc inconnu ne bouge pas.
pub fn zone_locals(m: &Model, blocks: &[BlockDef], anim: &str, t: f32) -> Vec<(Quat, Vec3)> {
    m.zones
        .iter()
        .map(|z| {
            let Some(def) = blocks.iter().find(|b| b.id == z.block) else { return (Quat::IDENTITY, Vec3::ZERO) };
            let a = if def.anims.contains_key(anim) { anim } else { "repos" };
            part_pose(def, &z.part, a, t, &zone_placement(z))
        })
        .collect()
}

/// Mélange de deux poses (changement d'animation en douceur) : 0 = `a`, 1 = `b`.
pub fn blend(a: &[(Quat, Vec3)], b: &[(Quat, Vec3)], f: f32) -> Vec<(Quat, Vec3)> {
    a.iter().zip(b).map(|((qa, pa), (qb, pb))| (qa.slerp(*qb, f), pa.lerp(*pb, f))).collect()
}

/// Matrices des zones dans le repère du modèle (index 0 = corps fixe, `i + 1` = zone `i`) :
/// chaque zone tourne autour de son pivot, emportée par sa parente.
pub fn compose(m: &Model, locals: &[(Quat, Vec3)]) -> Vec<Mat4> {
    let mut out = vec![Mat4::IDENTITY; m.zones.len() + 1];
    for (i, z) in m.zones.iter().enumerate() {
        let (q, d) = locals.get(i).copied().unwrap_or((Quat::IDENTITY, Vec3::ZERO));
        let p = Vec3::from_array(z.pivot);
        let local = Mat4::from_translation(p + d) * Mat4::from_quat(q) * Mat4::from_translation(-p);
        // Parents d'abord (les blocs sont posés ainsi) ; un parent après est ignoré
        let parent = z.parent.map(|k| k as usize).filter(|k| *k < i).map_or(Mat4::IDENTITY, |k| out[k + 1]);
        out[i + 1] = parent * local;
    }
    out
}

/// Zones qui traversent le corps fixe pendant l'animation (16 instants d'une boucle) ; les cases
/// proches de l'articulation (pivot) ne comptent pas.
pub fn collisions(m: &Model, blocks: &[BlockDef], anim: &str) -> Vec<usize> {
    let mut hit = vec![false; m.zones.len()];
    let cells: Vec<(IVec3, usize)> = m.zone_map.iter().map(|(p, z)| (p, z as usize)).filter(|(_, z)| *z > 0 && *z <= m.zones.len()).collect();
    let d = anim_duration(m, blocks, anim);
    for i in 0..16 {
        let mats = compose(m, &zone_locals(m, blocks, anim, d * i as f32 / 16.0));
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
        let keys = Track::Keys { rot: vec![[0.0, 0.0, 0.0, 0.0], [1.0, 90.0, 0.0, 0.0], [2.0, 0.0, 0.0, 0.0]], pos: vec![] };
        let (q, _) = sample(&keys, 1.0, 2.0);
        assert!((q.angle_between(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2))).abs() < 1e-3);
        let (q0, _) = sample(&keys, 0.0, 2.0);
        let (q2, _) = sample(&keys, 2.0, 2.0);
        assert!(q0.angle_between(q2) < 1e-3, "boucle");
        let wave = |phase| Track::Wave { axis: [0.0, 1.0, 0.0], amplitude: 30.0, period: 2.0, phase };
        let (a, _) = sample(&wave(0.0), 0.5, 1.0);
        let (b, _) = sample(&wave(0.25), 0.5, 1.0);
        assert!(a.angle_between(Quat::IDENTITY) > 0.4 && b.angle_between(Quat::IDENTITY) < 1e-3, "dephasage");
    }
    fn block(id: &str) -> BlockDef {
        defs().into_iter().find(|b| b.id == id).unwrap()
    }

    #[test]
    fn a_door_that_sweeps_through_the_hull_is_reported() {
        use super::super::edit::Doc;
        use super::super::format::{Material, ModelKind, PaletteEntry};
        let blocks = defs();
        let door = block("porte_pivotante");
        let mut d = Doc::new(Model::new("v", ModelKind::Autre, None), None);
        let at = IVec3::new(10, 2, 10);
        d.place_block(&door, at, Placement { turn: 0, mirror: false, scale: 1 }, false).unwrap();
        assert_eq!(d.model.zones.len(), 1);
        assert!(collisions(&d.model, &blocks, "ouverture").is_empty());
        // Au repos, rien ne bouge
        let rest = compose(&d.model, &zone_locals(&d.model, &blocks, "repos", 0.3));
        assert!(rest[1].abs_diff_eq(Mat4::IDENTITY, 1e-5));
        // Une coque là où passe le bord de la porte ouverte
        let open = compose(&d.model, &zone_locals(&d.model, &blocks, "ouverture", 2.0));
        let edge = open[1].transform_point3(at.as_vec3() + Vec3::new(0.5, 3.5, 4.5));
        let hull = d.model.color_index(PaletteEntry { rgb: [90, 90, 90], material: Material::Metal }).unwrap();
        d.set(edge.floor().as_ivec3(), hull, false);
        assert_eq!(collisions(&d.model, &blocks, "ouverture"), vec![0]);
    }

    #[test]
    fn nested_zones_follow_their_parent() {
        use super::super::edit::Doc;
        use super::super::format::ModelKind;
        let blocks = defs();
        let arm = block("bras");
        let mut d = Doc::new(Model::new("p", ModelKind::Personnage, None), None);
        d.place_block(&arm, IVec3::new(3, 20, 8), Placement { turn: 0, mirror: false, scale: 1 }, true).unwrap();
        // Bras, avant-bras, main, des deux côtés
        assert_eq!(d.model.zones.len(), 6);
        assert_eq!(d.model.zones[2].parent, Some(1));
        assert_eq!(d.model.zones[5].parent, Some(4));
        let mats = compose(&d.model, &zone_locals(&d.model, &blocks, "saluer", 0.8));
        // La main suit le bras levé : elle monte
        let hand = Vec3::from_array(d.model.zones[2].pivot);
        assert!(mats[3].transform_point3(hand).y > hand.y + 5.0);
        // Le bras reflété salue de l'autre côté
        let a = mats[3].transform_point3(hand) - hand;
        let hand2 = Vec3::from_array(d.model.zones[5].pivot);
        let b = mats[6].transform_point3(hand2) - hand2;
        assert!((a.x + b.x).abs() < 1e-3 && (a.y - b.y).abs() < 1e-3, "{a} {b}");
        assert!(model_anims(&d.model, &blocks).starts_with(&["repos".to_string()]));
    }
}
