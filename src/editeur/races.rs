//! Races de personnage (E5, §3.2 de `ROADMAP-0.12-editeur.md`) : une famille par fichier JSON de
//! `assets/editeur/races/` (règle 4 : ajouter une race = ajouter un fichier).
//!
//! Une race = un **rig** posé sur la grille 16 × 32 × 32 (personnage tourné vers +z) : des os
//! (nom, parent, pivot, gabarit en boîtes de cases), des **parties fixes** (cornes, auréole :
//! cases d'un os sans articulation à elles) et des **membres optionnels** (cases à cocher : queue,
//! oreilles, ailes, bras en plus, jambes digitigrades...). Un os `sym` est décrit du côté gauche
//! (x bas, suffixe « _g ») et reflété à droite (« _d »). Le rig devient des zones de mouvement
//! (comme un bloc de mouvement posé), en blocs blancs ; la bibliothèque d'animations les anime par
//! leurs noms d'os.

use bevy::math::{IVec3, Vec3};
use serde::Deserialize;

use super::edit::BLOCK_WHITE;
use super::format::{Model, ModelKind, Zone};

fn one() -> f32 {
    1.0
}

fn marcher() -> String {
    "marcher".into()
}

#[derive(Clone, Debug, Deserialize)]
pub struct RaceDef {
    pub id: String,
    /// Nom de la famille (gardé dans `Model::race`).
    pub name: String,
    /// Races de la famille (« Humain, elfe, nain... »).
    #[serde(default)]
    pub races: Vec<String>,
    /// Façon de se déplacer (bipède, quadrupède, reptation...).
    #[serde(default)]
    pub locomotion: String,
    /// Vitesse des animations (golems : plus lentes).
    #[serde(default = "one")]
    pub speed: f32,
    /// Animation de l'aperçu au choix de la race.
    #[serde(default = "marcher")]
    pub preview: String,
    pub parts: Vec<RacePart>,
    #[serde(default)]
    pub options: Vec<RaceOption>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RacePart {
    pub name: String,
    #[serde(default)]
    pub parent: Option<String>,
    /// Pivot (coins des cases), inutile pour une partie fixe.
    #[serde(default)]
    pub pivot: [f32; 3],
    /// Boîtes de cases [x0, y0, z0, x1, y1, z1] (bornes comprises), dans la grille.
    pub boxes: Vec<[i32; 6]>,
    /// Décrit à gauche, reflété à droite (`nom_g`, `nom_d`).
    #[serde(default)]
    pub sym: bool,
    /// Partie fixe : ses cases suivent son parent, sans articulation.
    #[serde(default)]
    pub fixed: bool,
}

/// Membre optionnel (case à cocher au choix de la race).
#[derive(Clone, Debug, Deserialize)]
pub struct RaceOption {
    /// Identifiant (données ; unique dans la race).
    #[allow(dead_code)]
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub default: bool,
    pub parts: Vec<RacePart>,
    /// Parties de base qu'il remplace (jambes digitigrades, roues...).
    #[serde(default)]
    pub replaces: Vec<String>,
}

/// Une partie dépliée (côté choisi) : nom d'os, parent, pivot, cases.
#[derive(Clone, Debug, PartialEq)]
pub struct Bone {
    pub name: String,
    pub parent: Option<String>,
    pub pivot: Vec3,
    pub cells: Vec<IVec3>,
    pub fixed: bool,
}

/// Grille des personnages : le reflet de x est `WIDTH - 1 - x`.
const WIDTH: i32 = super::format::CHARACTER_GRID.x as i32;

impl RaceDef {
    /// Options cochées par défaut.
    pub fn default_options(&self) -> Vec<bool> {
        self.options.iter().map(|o| o.default).collect()
    }

    /// Les os du rig avec ces options, parents d'abord.
    pub fn bones(&self, options: &[bool]) -> Vec<Bone> {
        let on: Vec<&RaceOption> = self.options.iter().zip(options.iter().chain(std::iter::repeat(&false))).filter(|(_, b)| **b).map(|(o, _)| o).collect();
        let replaced: Vec<&str> = on.iter().flat_map(|o| o.replaces.iter().map(String::as_str)).collect();
        let parts = self.parts.iter().filter(|p| !replaced.contains(&p.name.as_str())).chain(on.iter().flat_map(|o| o.parts.iter()));
        let all: Vec<&RacePart> = parts.collect();
        let sym_of = |n: &str| all.iter().any(|p| p.name == n && p.sym);
        let mut out = Vec::new();
        for p in &all {
            let sides: &[&str] = if p.sym { &["_g", "_d"] } else { &[""] };
            for side in sides {
                let parent = p.parent.as_ref().map(|q| if sym_of(q) { format!("{q}{}", if side.is_empty() { "_g" } else { side }) } else { q.clone() });
                let right = *side == "_d";
                let cells = boxes_cells(&p.boxes).into_iter().map(|c| if right { IVec3::new(WIDTH - 1 - c.x, c.y, c.z) } else { c }).collect();
                let v = Vec3::from_array(p.pivot);
                let pivot = if right { Vec3::new(WIDTH as f32 - v.x, v.y, v.z) } else { v };
                out.push(Bone { name: format!("{}{side}", p.name), parent, pivot, cells, fixed: p.fixed });
            }
        }
        out
    }

    /// Le modèle de départ de la race : le rig en blocs blancs, une zone par os.
    pub fn build(&self, name: &str, options: &[bool]) -> Model {
        let mut m = Model::new(name, ModelKind::Personnage, None);
        m.race = Some(self.name.clone());
        let white = m.color_index(BLOCK_WHITE).unwrap_or(1);
        let bones = self.bones(options);
        // Zones des os articulés, dans l'ordre (parents d'abord)
        let mut zone_of: Vec<(String, u8)> = Vec::new();
        for b in bones.iter().filter(|b| !b.fixed) {
            if m.zones.len() >= 255 {
                break;
            }
            let parent = b.parent.as_ref().and_then(|p| zone_of.iter().find(|(n, _)| n == p)).map(|(_, z)| *z as u16 - 1);
            m.zones.push(Zone { name: b.name.replace('_', " "), block: "race".into(), part: b.name.clone(), parent, pivot: b.pivot.to_array(), turn: 0, mirror: false, scale: 1 });
            zone_of.push((b.name.clone(), m.zones.len() as u8));
        }
        for b in &bones {
            let zone = if b.fixed { b.parent.as_ref().and_then(|p| zone_of.iter().find(|(n, _)| n == p)).map_or(0, |(_, z)| *z) } else { zone_of.iter().find(|(n, _)| *n == b.name).map_or(0, |(_, z)| *z) };
            for c in &b.cells {
                if m.in_bounds(*c) {
                    m.voxels.set(*c, white);
                    m.zone_map.set(*c, zone);
                }
            }
        }
        m.tags.push("personnage".into());
        m
    }
}

fn boxes_cells(boxes: &[[i32; 6]]) -> Vec<IVec3> {
    let mut out = Vec::new();
    for b in boxes {
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

#[cfg(test)]
mod tests {
    use super::super::motion::{self, Library};
    use bevy::math::Quat;
    use super::*;

    fn lib() -> Library {
        let (l, errors) = Library::load(None);
        assert!(errors.is_empty(), "{errors:?}");
        l
    }

    /// Les réglages à vérifier : par défaut, puis chaque option seule en plus.
    fn configs(r: &RaceDef) -> Vec<Vec<bool>> {
        let mut out = vec![r.default_options()];
        for i in 0..r.options.len() {
            let mut c = r.default_options();
            c[i] = !c[i];
            out.push(c);
        }
        out
    }

    #[test]
    fn the_23_families_build_inside_the_grid_without_overlaps() {
        let l = lib();
        assert_eq!(l.races.len(), 23);
        let mut ids: Vec<&str> = l.races.iter().map(|r| r.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 23, "ids uniques");
        for r in &l.races {
            let mut o: Vec<&str> = r.options.iter().map(|o| o.id.as_str()).collect();
            o.sort();
            o.dedup();
            assert_eq!(o.len(), r.options.len(), "{} : options en double", r.id);
            for opts in configs(r) {
                let bones = r.bones(&opts);
                let names: Vec<&str> = bones.iter().map(|b| b.name.as_str()).collect();
                let mut seen = std::collections::HashMap::new();
                for (i, b) in bones.iter().enumerate() {
                    if let Some(p) = &b.parent {
                        let k = names.iter().position(|n| n == p).unwrap_or_else(|| panic!("{} : parent {p} de {} inconnu", r.id, b.name));
                        assert!(k < i, "{} : {} avant son parent", r.id, b.name);
                        assert!(!bones[k].fixed, "{} : {} attache a une partie fixe", r.id, b.name);
                    } else {
                        assert!(!b.fixed, "{} : partie fixe {} sans parent", r.id, b.name);
                    }
                    assert!(!b.cells.is_empty(), "{} : {} vide", r.id, b.name);
                    for c in &b.cells {
                        assert!(c.cmpge(IVec3::ZERO).all() && c.cmplt(super::super::format::CHARACTER_GRID.as_ivec3()).all(), "{} {opts:?} : {} hors de la grille en {c}", r.id, b.name);
                        if let Some(other) = seen.insert(*c, b.name.clone()).filter(|o| *o != b.name) {
                            panic!("{} {opts:?} : {} et {other} se recouvrent en {c}", r.id, b.name);
                        }
                    }
                }
                let m = r.build("x", &opts);
                assert_eq!(m.voxels.count(), seen.len());
                assert_eq!(m.zones.len(), bones.iter().filter(|b| !b.fixed).count());
                assert_eq!(m.race.as_deref(), Some(r.name.as_str()));
            }
        }
    }

    #[test]
    fn every_animation_track_finds_a_bone() {
        let l = lib();
        let mut bones = std::collections::BTreeSet::new();
        for r in &l.races {
            for opts in configs(r) {
                bones.extend(r.bones(&opts).into_iter().filter(|b| !b.fixed).map(|b| b.name));
            }
        }
        assert!(l.anims.len() >= 40, "{}", l.anims.len());
        let groups = ["Base", "Interaction", "Emotes", "Vol", "Locomotion", motion::PROCEDURAL];
        for a in &l.anims {
            assert!(groups.contains(&a.group.as_str()), "{} : groupe {}", a.id, a.group);
            for k in a.tracks.keys() {
                let probe = motion::LibAnim { tracks: [(k.clone(), a.tracks[k].clone())].into_iter().collect(), ..a.clone() };
                assert!(bones.iter().any(|b| motion::lib_pose(&probe, b, 0.3).is_some()), "{} : piste {k} sans os", a.id);
            }
        }
        // Chaque race a son repos, son aperçu, et il fait bouger quelque chose
        for r in &l.races {
            let m = r.build("x", &r.default_options());
            let anims = motion::model_anims(&m, &l);
            assert!(anims.contains(&r.preview), "{} : apercu {} absent ({anims:?})", r.id, r.preview);
            let still = motion::compose(&m, &vec![motion::Pose::default(); m.zones.len()]);
            let moving = (0..8).any(|i| {
                let mats = motion::compose(&m, &motion::zone_locals(&m, &l, &r.preview, i as f32 * 0.17, true));
                mats.iter().zip(&still).any(|(a, b)| !a.abs_diff_eq(*b, 1e-3))
            });
            assert!(moving, "{} : l'apercu ne bouge pas", r.id);
        }
    }

    #[test]
    fn the_right_side_mirrors_the_left_half_a_loop_later() {
        let l = lib();
        let walk = l.anim("marcher").unwrap();
        let shift = walk.mirror.unwrap() * walk.duration;
        assert!(!walk.tracks.contains_key("bras_d"));
        let g = motion::lib_pose(walk, "bras_g", 0.1 + shift).unwrap();
        let r = motion::lib_pose(walk, "bras_d", 0.1).unwrap();
        let tip = Vec3::new(0.3, -1.0, 0.2);
        let (a, b) = (g.rot * tip, r.rot * Vec3::new(-tip.x, tip.y, tip.z));
        assert!((a - Vec3::new(-b.x, b.y, b.z)).length() < 1e-3, "{a} {b}");
        assert!(g.rot.angle_between(Quat::IDENTITY) > 0.1);
        // Les chaînes : le motif `queue_*` donne une onde décalée à chaque maillon
        let wag = l.anims.iter().find(|a| a.tracks.keys().any(|k| k.starts_with("queue_"))).unwrap();
        let q1 = motion::lib_pose(wag, "queue_1", 0.4).unwrap();
        let q3 = motion::lib_pose(wag, "queue_3", 0.4).unwrap();
        assert!(q1.rot.angle_between(q3.rot) > 1e-3);
    }

    #[test]
    fn a_slime_squashes_and_a_golem_is_slow() {
        let l = lib();
        let slime = l.races.iter().find(|r| r.id == "slime").unwrap();
        let m = slime.build("s", &slime.default_options());
        let scales: Vec<f32> = (0..10).map(|i| motion::zone_locals(&m, &l, &slime.preview, i as f32 * 0.1, false)[0].scale.y).collect();
        assert!(scales.iter().any(|s| *s < 0.95) && scales.iter().any(|s| *s > 1.02), "{scales:?}");
        let golem = l.races.iter().find(|r| r.id == "golem").unwrap();
        assert!(golem.speed < 1.0);
    }
}
