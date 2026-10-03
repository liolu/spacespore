//! Modèles fournis par défaut (E7) : un personnage par famille de race (le rig de la race, peint),
//! un vaisseau par catégorie. Ils servent quand un joueur n'a pas choisi de modèle, et en attendant
//! le modèle d'un autre joueur. Ils sont fabriqués par le code (toujours les mêmes) : un nom suffit
//! pour les désigner sur le réseau (`perso:humanoide`, `vaisseau:chasseur`).

use bevy::math::{IVec3, Vec3};

use super::edit::{Brush, Doc, Shape};
use super::format::{Material, Model, ModelKind, PaletteEntry, ShipCategory};
use super::motion::{Library, Placement};
use super::races::RaceDef;

pub const SHIP_PREFIX: &str = "vaisseau:";
pub const CHARACTER_PREFIX: &str = "perso:";

/// Vaisseau par défaut du joueur.
pub const DEFAULT_SHIP: &str = "vaisseau:chasseur";
/// Personnage par défaut du joueur.
pub const DEFAULT_CHARACTER: &str = "perso:humanoide";

/// Identifiant d'une catégorie (`vaisseau:fregate`).
#[cfg(test)]
pub fn ship_id(c: ShipCategory) -> String {
    format!("{SHIP_PREFIX}{}", c.name().to_lowercase())
}

/// Tous les modèles par défaut.
#[cfg(test)]
pub fn all_ids(lib: &Library) -> Vec<String> {
    let mut out: Vec<String> = ShipCategory::ALL.iter().map(|c| ship_id(*c)).collect();
    out.extend(lib.races.iter().map(|r| format!("{CHARACTER_PREFIX}{}", r.id)));
    out
}

/// Fabrique un modèle par défaut (`None` : nom inconnu).
pub fn build(id: &str, lib: &Library) -> Option<Model> {
    if let Some(c) = id.strip_prefix(SHIP_PREFIX) {
        let cat = ShipCategory::ALL.into_iter().find(|x| x.name().to_lowercase() == c)?;
        return Some(ship(cat, lib));
    }
    let r = id.strip_prefix(CHARACTER_PREFIX)?;
    let (k, race) = lib.races.iter().enumerate().find(|(_, x)| x.id == r)?;
    Some(character(race, k))
}

fn pe(rgb: [u8; 3], material: Material) -> PaletteEntry {
    PaletteEntry { rgb, material }
}

// ─────────────────────────────────────────────────────────────────────────
//  Personnages
// ─────────────────────────────────────────────────────────────────────────

const SKINS: [[u8; 3]; 6] = [[236, 188, 150], [198, 134, 90], [120, 78, 52], [170, 200, 140], [150, 160, 200], [210, 120, 110]];
const CLOTHES: [[u8; 3]; 6] = [[60, 90, 150], [140, 50, 50], [60, 110, 70], [110, 80, 140], [180, 140, 60], [70, 70, 80]];

/// Le rig de la race (membres par défaut), peint : peau, habits, pantalon, chaussures, yeux.
fn character(race: &RaceDef, k: usize) -> Model {
    let name = race.races.first().cloned().unwrap_or_else(|| race.name.clone());
    let mut m = race.build(&name, &race.default_options());
    let skin = SKINS[k % SKINS.len()];
    let cloth = CLOTHES[(k * 5 + 1) % CLOTHES.len()];
    let pants = CLOTHES[(k * 3 + 2) % CLOTHES.len()].map(|c| (c as f32 * 0.6) as u8);
    let dark = [30, 28, 34];
    let slime = race.id == "slime";
    let color_of = |bone: &str| -> PaletteEntry {
        let b = bone.trim_end_matches("_g").trim_end_matches("_d");
        let starts = |p: &[&str]| p.iter().any(|x| b.starts_with(x));
        if slime {
            pe([90, 220, 120], Material::Verre)
        } else if starts(&["tronc", "bras", "avant_bras", "cape", "abdomen"]) {
            pe(cloth, Material::Mate)
        } else if starts(&["bassin", "cuisse", "tibia"]) {
            pe(pants, Material::Mate)
        } else if starts(&["pied", "roue", "bout_"]) {
            pe(dark, Material::Mate)
        } else if starts(&["aile", "pointe"]) {
            pe([235, 235, 240], Material::Mate)
        } else if starts(&["antenne"]) && race.id == "mecha" {
            pe([90, 180, 255], Material::Lumineuse)
        } else {
            pe(skin, Material::Mate)
        }
    };
    let bones: Vec<String> = m.zones.iter().map(super::motion::bone).collect();
    let mut index = [0u8; 256];
    for (z, b) in bones.iter().enumerate() {
        index[z + 1] = m.color_index(color_of(b)).unwrap_or(1);
    }
    index[0] = m.color_index(pe(skin, Material::Mate)).unwrap_or(1);
    let cells: Vec<(IVec3, u8)> = m.voxels.iter().collect();
    for (p, _) in cells {
        let z = m.zone_map.get(p);
        m.voxels.set(p, index[z as usize]);
    }
    // Les yeux : deux cases sombres sur le devant de la tête
    if let Some(t) = bones.iter().position(|b| b == "tete") {
        let (mut lo, mut hi) = (IVec3::MAX, IVec3::MIN);
        for (p, z) in m.zone_map.iter() {
            if z as usize == t + 1 {
                lo = lo.min(p);
                hi = hi.max(p);
            }
        }
        if lo.x <= hi.x {
            let eye = m.color_index(pe(dark, Material::Mate)).unwrap_or(1);
            let y = lo.y + (hi.y - lo.y) * 3 / 5;
            let cx = (lo.x + hi.x) / 2;
            for x in [cx - 1, hi.x + lo.x - (cx - 1)] {
                let p = IVec3::new(x, y, hi.z);
                if m.voxels.get(p) != 0 {
                    m.voxels.set(p, eye);
                }
            }
        }
    }
    m.compact_palette();
    m
}

// ─────────────────────────────────────────────────────────────────────────
//  Vaisseaux
// ─────────────────────────────────────────────────────────────────────────

/// Un vaisseau simple de la catégorie (nez vers +z) : coque effilée, ailes, passerelle, blocs de
/// mouvement (réacteurs, tourelles, feux, train ou hangars selon la taille).
fn ship(cat: ShipCategory, lib: &Library) -> Model {
    let g = cat.grid() as i32;
    let gf = g as f32;
    let mut d = Doc::new(Model::new(&format!("{} par defaut", cat.name()), ModelKind::Vaisseau, Some(cat)), None);
    let hull = pe([126, 132, 142], Material::Metal);
    let dark = pe([70, 74, 84], Material::Metal);
    let glass = pe([150, 210, 255], Material::Verre);
    let stripe = pe(match cat {
        ShipCategory::Chasseur => [200, 60, 50],
        ShipCategory::Corvette => [60, 120, 200],
        ShipCategory::Fregate => [220, 170, 50],
        ShipCategory::Croiseur => [90, 160, 90],
        ShipCategory::Capital => [150, 70, 170],
    }, Material::Mate);
    let (cx, cy) = (g / 2, (gf * 0.42) as i32);
    let (z0, z1) = ((gf * 0.12) as i32, (gf * 0.88) as i32);
    let r = gf * 0.10;
    // Coque : cylindres le long de z, effilés vers le nez
    let seg = (g / 24).max(2);
    let mut z = z0;
    while z < z1 {
        let t = (z - z0) as f32 / (z1 - z0) as f32;
        let rr = r * if t > 0.65 { ((1.0 - t) / 0.35).powf(0.6).max(0.15) } else { 1.0 };
        let _ = d.apply_shape(Shape::Cylinder { c: IVec3::new(cx, cy, z), axis: 2, r: rr, h: seg }, Brush::Add, hull, false);
        z += seg;
    }
    // Bande de couleur, passerelle vitrée
    let band = (gf * 0.03).max(1.0) as i32;
    let _ = d.apply_shape(Shape::Box { a: IVec3::new(0, 0, (gf * 0.5) as i32), b: IVec3::new(g - 1, g - 1, (gf * 0.5) as i32 + band) }, Brush::Paint, stripe, false);
    let top = cy + r as i32;
    let (bw, bh) = ((gf * 0.05).max(2.0) as i32, (gf * 0.05).max(2.0) as i32);
    let bz = (gf * 0.62) as i32;
    let _ = d.apply_shape(Shape::Box { a: IVec3::new(cx - bw, top, bz - bw * 2), b: IVec3::new(cx + bw - 1, top + bh, bz + bw) }, Brush::Add, dark, false);
    let _ = d.apply_shape(Shape::Box { a: IVec3::new(cx - bw + 1, top + bh - 1, bz + bw + 1), b: IVec3::new(cx + bw - 2, top + bh - 1, bz + bw + 1) }, Brush::Add, glass, false);
    // Ailes (et leur reflet)
    let wing = (gf * 0.02).max(1.0) as i32;
    let _ = d.apply_shape(
        Shape::Box { a: IVec3::new((gf * 0.12) as i32, cy - wing, (gf * 0.3) as i32), b: IVec3::new(cx - r as i32, cy + wing, (gf * 0.55) as i32) },
        Brush::Add,
        dark,
        true,
    );
    // Blocs de mouvement, à l'échelle de la catégorie
    let scale = ((gf / 64.0).round() as u8).clamp(1, 8);
    let p = |turn: u8| Placement { turn, mirror: false, scale };
    let mut put = |id: &str, at: IVec3, place: Placement, mirror: bool| {
        if let Some(def) = lib.block(id) {
            let _ = d.place_block(def, at, place, mirror);
        }
    };
    let ex = (r * 0.45) as i32;
    put("propulseur", IVec3::new(cx - ex, cy, z0 - 1), p(0), true);
    put("feu", IVec3::new((gf * 0.12) as i32, cy + wing + 1, (gf * 0.42) as i32), Placement { scale: 1, ..p(0) }, true);
    put("tourelle", IVec3::new(cx, top + 1, (gf * 0.4) as i32), p(0), false);
    match cat {
        ShipCategory::Chasseur => {
            put("train", IVec3::new(cx - ex, cy - r as i32 - 1, (gf * 0.35) as i32), p(0), true);
            put("ailes_x", IVec3::new(cx + r as i32 + 1, cy, (gf * 0.25) as i32), Placement { scale: 1, ..p(0) }, true);
        }
        ShipCategory::Corvette | ShipCategory::Fregate => {
            put("radar", IVec3::new(cx, top + bh + 1, bz), p(0), false);
            put("train", IVec3::new(cx - ex, cy - r as i32 - 1, (gf * 0.35) as i32), p(0), true);
        }
        ShipCategory::Croiseur | ShipCategory::Capital => {
            put("radar", IVec3::new(cx, top + bh + 1, bz), p(0), false);
            put("anneau", IVec3::new(cx, cy, (gf * 0.2) as i32), p(0), false);
            // Hangars sur les flancs, et la baie creusée derrière la porte
            let side = cx + r as i32 + 1;
            let mut bay = |id: &str, z: i32, y: i32| {
                let Some(def) = lib.block(id) else { return };
                if d.place_block(def, IVec3::new(side, y, z), Placement { turn: 1, mirror: false, scale: 1 }, false).is_ok() {
                    if let Some(h) = d.model.hangars.last().cloned() {
                        let s = Vec3::from_array(h.slot).as_ivec3();
                        let half = def.parts[0].boxes[0][3] + 1;
                        let height = def.parts[0].boxes[0][4];
                        let _ = d.apply_shape(Shape::Box { a: IVec3::new(s.x - 4, y, z - half + 1), b: IVec3::new(side - 1, y + height, z + half - 1) }, Brush::Remove, hull, false);
                    }
                }
            };
            if cat == ShipCategory::Croiseur {
                bay("hangar_chasseur", (gf * 0.45) as i32, cy - 20);
            } else {
                bay("hangar_corvette", (gf * 0.40) as i32, cy - 40);
                bay("hangar_chasseur", (gf * 0.62) as i32, cy - 20);
                bay("soute_cargo", (gf * 0.24) as i32, cy - 75);
            }
        }
    }
    d.end();
    let mut m = d.model;
    m.compact_palette();
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_default_model_builds() {
        let (lib, _) = Library::load(None);
        let ids = all_ids(&lib);
        assert_eq!(ids.len(), 5 + 23);
        for id in &ids {
            if id == "vaisseau:capital" || id == "vaisseau:croiseur" {
                continue;
            }
            let m = build(id, &lib).unwrap_or_else(|| panic!("{id}"));
            assert!(m.voxels.count() > 100, "{id}");
            assert!(m.to_bytes().is_ok(), "{id} : trop lourd");
        }
        // Les mêmes à chaque fois (désignés par leur nom sur le réseau)
        let a = build("perso:centaure", &lib).unwrap();
        let b = build("perso:centaure", &lib).unwrap();
        assert_eq!(a.voxels, b.voxels);
        assert!(build("vaisseau:inconnu", &lib).is_none());
    }

    /// Les gros vaisseaux par défaut (long) : `cargo test --release big_default_ships -- --ignored`.
    #[test]
    #[ignore]
    fn big_default_ships_have_hangars() {
        let (lib, _) = Library::load(None);
        let c = build("vaisseau:croiseur", &lib).unwrap();
        assert_eq!(c.hangars.len(), 1);
        let k = build("vaisseau:capital", &lib).unwrap();
        assert_eq!(k.hangars.len(), 3);
        assert!(k.hangars.iter().any(|h| h.cargo));
        assert!(k.to_bytes().is_ok());
    }
}
