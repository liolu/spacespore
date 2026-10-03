//! Mode avancé (E8) : images clés des animations du modèle, et blocs de mouvement personnels (une
//! zone et ses zones filles enregistrées en bloc dans `saves/editeur/blocs/`, à poser ensuite
//! comme les blocs du jeu).

use std::collections::HashMap;

use bevy::math::{EulerRot, IVec3, Vec3};
use serde_json::{json, Value};

use super::format::{Model, ModelAnim};
use super::motion::{self, Track};

/// Écart sous lequel deux images clés sont au même instant (s).
const SAME_TIME: f32 = 0.02;

/// Angles (degrés, Euler XYZ) d'un os à l'instant `t` d'une animation du modèle.
pub fn angles_at(a: &ModelAnim, bone: &str, t: f32) -> [f32; 3] {
    let Some(keys) = a.keys.get(bone).filter(|k| !k.is_empty()) else { return [0.0; 3] };
    let q = motion::sample(&Track::Keys { rot: keys.clone(), pos: Vec::new(), scale: Vec::new() }, t, a.duration, 1).rot;
    let (x, y, z) = q.to_euler(EulerRot::XYZ);
    [x.to_degrees(), y.to_degrees(), z.to_degrees()]
}

/// Pose (ou remplace) l'image clé de `bone` à l'instant `t`.
pub fn set_key(a: &mut ModelAnim, bone: &str, t: f32, angles: [f32; 3]) {
    let keys = a.keys.entry(bone.to_string()).or_default();
    let t = t.clamp(0.0, a.duration);
    match keys.iter_mut().find(|k| (k[0] - t).abs() < SAME_TIME) {
        Some(k) => *k = [k[0], angles[0], angles[1], angles[2]],
        None => {
            keys.push([t, angles[0], angles[1], angles[2]]);
            keys.sort_by(|x, y| x[0].total_cmp(&y[0]));
        }
    }
}

/// Retire l'image clé de `bone` à l'instant `t` (s'il y en a une).
pub fn remove_key(a: &mut ModelAnim, bone: &str, t: f32) -> bool {
    let Some(keys) = a.keys.get_mut(bone) else { return false };
    let n = keys.len();
    keys.retain(|k| (k[0] - t).abs() >= SAME_TIME);
    let removed = keys.len() != n;
    if keys.is_empty() {
        a.keys.remove(bone);
    }
    removed
}

/// Instants des images clés d'un os.
pub fn key_times(a: &ModelAnim, bone: &str) -> Vec<f32> {
    a.keys.get(bone).map_or(Vec::new(), |k| k.iter().map(|x| x[0]).collect())
}

/// Une nouvelle animation au nom libre (« animation 1 », « animation 2 »...).
pub fn fresh_anim_name(m: &Model) -> String {
    (1..).map(|k| format!("animation {k}")).find(|n| !m.anims.contains_key(n)).unwrap()
}

/// La zone `z` et ses zones filles (toute la descendance), parents d'abord.
pub fn subtree(m: &Model, z: usize) -> Vec<usize> {
    let mut out = vec![z];
    let mut i = 0;
    while i < out.len() {
        let p = out[i];
        for (k, zone) in m.zones.iter().enumerate() {
            if zone.parent == Some(p as u16) && !out.contains(&k) {
                out.push(k);
            }
        }
        i += 1;
    }
    out.sort();
    out
}

/// Un bloc de mouvement (JSON, format de `assets/editeur/blocs/`) fait de la zone `z` et de ses
/// filles : leurs cases (en boîtes : une ligne en x par boîte), leurs pivots, la couleur la plus
/// fréquente de chacune, et les animations du modèle qui les touchent.
pub fn block_json(m: &Model, z: usize, id: &str, name: &str) -> Value {
    let zones = subtree(m, z);
    // Cases de chaque zone, et l'ancre : le coin bas de l'ensemble
    let mut cells: HashMap<usize, Vec<IVec3>> = HashMap::new();
    for (p, n) in m.zone_map.iter() {
        let k = n as usize - 1;
        if zones.contains(&k) {
            cells.entry(k).or_default().push(p);
        }
    }
    let anchor = cells.values().flatten().fold(IVec3::MAX, |a, p| a.min(*p));
    let anchor = if anchor.x == i32::MAX { IVec3::ZERO } else { anchor };
    let names: HashMap<usize, String> = zones.iter().map(|k| (*k, motion::bone(&m.zones[*k]))).collect();
    let parts: Vec<Value> = zones
        .iter()
        .map(|k| {
            let zone = &m.zones[*k];
            let mut c = cells.get(k).cloned().unwrap_or_default();
            c.sort_by_key(|p| (p.z, p.y, p.x));
            // Lignes en x : une boîte par suite de cases
            let mut boxes: Vec<[i32; 6]> = Vec::new();
            for p in &c {
                let q = *p - anchor;
                match boxes.last_mut() {
                    Some(b) if b[1] == q.y && b[2] == q.z && b[3] + 1 == q.x => b[3] = q.x,
                    _ => boxes.push([q.x, q.y, q.z, q.x, q.y, q.z]),
                }
            }
            // Couleur la plus fréquente
            let mut count: HashMap<u8, usize> = HashMap::new();
            for p in &c {
                *count.entry(m.voxels.get(*p)).or_default() += 1;
            }
            let color = count.into_iter().max_by_key(|(_, n)| *n).and_then(|(v, _)| m.palette.get(v as usize - 1).copied());
            let pivot = Vec3::from_array(zone.pivot) - anchor.as_vec3();
            let mut part = json!({ "name": names[k], "pivot": pivot.to_array(), "boxes": boxes });
            if let Some(parent) = zone.parent.map(|p| p as usize).filter(|p| zones.contains(p)) {
                part["parent"] = json!(names[&parent]);
            }
            if let Some(e) = color {
                part["color"] = json!(e.rgb);
                part["material"] = json!(e.material);
            }
            part
        })
        .collect();
    let mut anims = serde_json::Map::new();
    anims.insert("repos".into(), json!({ "duration": 1.0, "tracks": {} }));
    for (n, a) in &m.anims {
        let tracks: serde_json::Map<String, Value> = zones
            .iter()
            .filter_map(|k| a.keys.get(&names[k]).map(|keys| (names[k].clone(), json!({ "type": "cles", "rot": keys }))))
            .collect();
        if !tracks.is_empty() {
            anims.insert(n.clone(), json!({ "duration": a.duration, "tracks": tracks }));
        }
    }
    let kind = match m.kind {
        super::format::ModelKind::Personnage => "personnage",
        super::format::ModelKind::Vaisseau => "vaisseau",
        super::format::ModelKind::Autre => "autre",
    };
    json!({ "id": id, "name": name, "kinds": [kind], "parts": parts, "anims": anims })
}

#[cfg(test)]
mod tests {
    use super::super::edit::Doc;
    use super::super::format::ModelKind;
    use super::super::motion::{BlockDef, Library, Placement};
    use super::*;
    use bevy::math::Quat;

    #[test]
    fn keys_are_set_interpolated_and_removed() {
        let mut a = ModelAnim { duration: 2.0, keys: Default::default() };
        set_key(&mut a, "bras_g", 0.0, [0.0; 3]);
        set_key(&mut a, "bras_g", 1.0, [-90.0, 0.0, 0.0]);
        set_key(&mut a, "bras_g", 2.0, [0.0; 3]);
        assert_eq!(key_times(&a, "bras_g"), vec![0.0, 1.0, 2.0]);
        assert!((angles_at(&a, "bras_g", 1.0)[0] + 90.0).abs() < 0.1);
        let mid = angles_at(&a, "bras_g", 0.5)[0];
        assert!(mid < -10.0 && mid > -80.0, "{mid}");
        // La même clé : remplacée
        set_key(&mut a, "bras_g", 1.01, [-45.0, 0.0, 0.0]);
        assert_eq!(key_times(&a, "bras_g").len(), 3);
        assert!(remove_key(&mut a, "bras_g", 1.0));
        assert_eq!(key_times(&a, "bras_g").len(), 2);
    }

    #[test]
    fn a_zone_becomes_a_reusable_motion_block() {
        let lib = Library::load(None).0;
        let mut d = Doc::new(Model::new("p", ModelKind::Personnage, None), None);
        d.place_block(lib.block("bras").unwrap(), IVec3::new(3, 20, 8), Placement::default(), false).unwrap();
        // Une animation du modèle : le bras se lève
        d.edit_anims(|anims| {
            let mut a = ModelAnim { duration: 2.0, keys: Default::default() };
            set_key(&mut a, "bras", 0.0, [0.0; 3]);
            set_key(&mut a, "bras", 1.0, [0.0, 0.0, -120.0]);
            anims.insert("lever".into(), a);
        });
        assert!(motion::model_anims(&d.model, &lib).contains(&"lever".to_string()));
        let v = block_json(&d.model, 0, "mon_bras", "Mon bras");
        let def: BlockDef = serde_json::from_value(v).unwrap();
        assert_eq!(def.parts.len(), 3);
        assert!(def.anims.contains_key("lever"));
        // Posé ailleurs : les mêmes cases, les mêmes zones emboîtées
        let mut e = Doc::new(Model::new("q", ModelKind::Personnage, None), None);
        e.place_block(&def, IVec3::new(8, 10, 8), Placement::default(), false).unwrap();
        assert_eq!(e.model.voxels.count(), d.model.voxels.count());
        assert_eq!(e.model.zones[2].parent, Some(1));
        // L'animation du bloc lève le bras
        let lib2 = Library { blocks: vec![def], ..lib };
        let up = motion::zone_locals(&e.model, &lib2, "lever", 1.0, false)[0].rot;
        assert!(up.angle_between(Quat::IDENTITY) > 1.5);
    }
}
