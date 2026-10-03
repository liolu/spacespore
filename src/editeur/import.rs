//! Import des modèles de Pixel World (éditeur Unity) : JSON `{ name, tags, size, scale, voxels:
//! [[x, y, z, "Bloc"], ...] }`, ou un catalogue `{ catalog, models: [...] }`
//! (`VoxelModelSerializer.cs`). Chaque `BlockType` devient une couleur de la palette (celles de
//! `BlockData.GetColor`) avec sa matière (verre, métal, lumineuse ou mate).

use bevy::math::{IVec3, UVec3};
use serde_json::Value;

use super::format::{Material, Model, ModelKind, PaletteEntry, OTHER_MAX_GRID};

/// Couleur et matière d'un bloc de Pixel World (`BlockType`, noms sans casse).
pub fn block_color(name: &str) -> Option<PaletteEntry> {
    use Material::*;
    let (rgb, material) = match name.to_ascii_lowercase().as_str() {
        "grass" => ([95, 159, 53], Mate),
        "dirt" => ([134, 96, 67], Mate),
        "stone" => ([136, 136, 136], Mate),
        "sand" => ([215, 203, 140], Mate),
        "snow" => ([242, 242, 252], Mate),
        "water" => ([28, 100, 210], Verre),
        "coral" => ([255, 127, 140], Mate),
        "flesh" => ([180, 50, 60], Mate),
        "bone" => ([235, 225, 210], Mate),
        "root" => ([80, 50, 30], Mate),
        "mushroom" => ([140, 80, 160], Mate),
        "mycelium" => ([180, 160, 200], Mate),
        "glass" => ([180, 220, 240], Verre),
        "crystal" => ([100, 200, 220], Verre),
        "silicon" => ([40, 60, 100], Metal),
        "metal" => ([170, 175, 180], Metal),
        "ash" => ([80, 80, 85], Mate),
        "obsidian" => ([20, 15, 30], Mate),
        "mist" => ([220, 230, 245], Verre),
        "paper" => ([240, 230, 210], Mate),
        "darkvoid" => ([30, 10, 50], Mate),
        "mirror" => ([210, 215, 225], Metal),
        "cloud" => ([200, 220, 255], Mate),
        "glow" => ([50, 200, 170], Lumineuse),
        "mercury" => ([190, 195, 200], Metal),
        "petrified" => ([120, 105, 90], Mate),
        "rust" => ([180, 90, 40], Mate),
        "marble" => ([230, 225, 220], Mate),
        "moss" => ([70, 120, 50], Mate),
        "brick" => ([155, 85, 60], Mate),
        "flower" => ([240, 210, 50], Mate),
        "redflower" => ([210, 45, 55], Mate),
        "tallgrass" => ([55, 130, 40], Mate),
        "ice" => ([170, 210, 235], Verre),
        "deadgrass" => ([150, 135, 80], Mate),
        "wood" => ([100, 70, 45], Mate),
        "leaves" => ([50, 160, 50], Mate),
        "orangeleaves" => ([210, 140, 35], Mate),
        "redleaves" => ([175, 55, 35], Mate),
        "pinkleaves" => ([185, 95, 155], Mate),
        _ => return None,
    };
    Some(PaletteEntry { rgb, material })
}

/// Bilan d'un import.
#[derive(Debug, Default, PartialEq)]
pub struct ImportReport {
    pub models: usize,
    pub voxels: usize,
    /// Voxels ignorés : bloc inconnu (ou air), hors de la grille.
    pub unknown: usize,
    pub outside: usize,
}

/// Lit un JSON de Pixel World (un modèle ou un catalogue) : les modèles deviennent des objets
/// « Autre » (grille d'origine, au plus 64³).
pub fn from_pixel_world(json: &str) -> Result<(Vec<Model>, ImportReport), String> {
    let v: Value = serde_json::from_str(json).map_err(|e| format!("JSON illisible : {e}"))?;
    let list: Vec<&Value> = match v.get("models").and_then(Value::as_array) {
        Some(models) => models.iter().collect(),
        None => vec![&v],
    };
    let mut report = ImportReport::default();
    let mut out = Vec::new();
    for m in list {
        let name = m.get("name").and_then(Value::as_str).unwrap_or("Modele importe");
        let mut model = Model::new(name, ModelKind::Autre, None);
        if let Some(s) = m.get("size").and_then(Value::as_array) {
            let d: Vec<u32> = s.iter().filter_map(Value::as_u64).map(|x| (x as u32).clamp(1, OTHER_MAX_GRID)).collect();
            if d.len() == 3 {
                model.size = UVec3::new(d[0], d[1], d[2]);
            }
        }
        model.tags = m.get("tags").and_then(Value::as_array).map_or_else(Vec::new, |t| t.iter().filter_map(Value::as_str).map(String::from).collect());
        model.tags.push("pixel world".into());
        for vox in m.get("voxels").and_then(Value::as_array).into_iter().flatten() {
            let Some(a) = vox.as_array().filter(|a| a.len() == 4) else {
                report.unknown += 1;
                continue;
            };
            let (Some(x), Some(y), Some(z), Some(block)) = (a[0].as_i64(), a[1].as_i64(), a[2].as_i64(), a[3].as_str()) else {
                report.unknown += 1;
                continue;
            };
            let p = IVec3::new(x as i32, y as i32, z as i32);
            if !model.in_bounds(p) {
                report.outside += 1;
                continue;
            }
            let Some(color) = block_color(block) else {
                report.unknown += 1;
                continue;
            };
            let Some(i) = model.color_index(color) else { continue };
            model.voxels.set(p, i);
            report.voxels += 1;
        }
        report.models += 1;
        out.push(model);
    }
    Ok((out, report))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imports_a_model_and_a_catalog() {
        let one = r#"{
  "name": "Lanterne",
  "tags": ["decor", "lumiere"],
  "size": [4, 6, 4],
  "voxels": [
    [1, 0, 1, "Metal"],
    [1, 1, 1, "Glow"],
    [1, 2, 1, "glass"],
    [9, 9, 9, "Metal"],
    [0, 0, 0, "Air"],
    [0, 1, 0, "Inconnu"]
  ]
}"#;
        let (models, report) = from_pixel_world(one).unwrap();
        assert_eq!(report, ImportReport { models: 1, voxels: 3, unknown: 2, outside: 1 });
        let m = &models[0];
        assert_eq!((m.name.as_str(), m.size, m.kind), ("Lanterne", UVec3::new(4, 6, 4), ModelKind::Autre));
        assert_eq!(m.palette.len(), 3);
        assert_eq!(m.color_at(IVec3::new(1, 1, 1)).unwrap(), PaletteEntry { rgb: [50, 200, 170], material: Material::Lumineuse });
        assert_eq!(m.color_at(IVec3::new(1, 2, 1)).unwrap().material, Material::Verre);
        assert!(m.tags.contains(&"pixel world".to_string()) && m.tags.contains(&"decor".to_string()));

        let catalog = format!(r#"{{ "catalog": "Mes modeles", "models": [ {one}, {{ "name": "Cube", "size": [2, 2, 2], "voxels": [[0, 0, 0, "Brick"]] }} ] }}"#);
        let (models, report) = from_pixel_world(&catalog).unwrap();
        assert_eq!(report.models, 2);
        assert_eq!(models[1].color_at(IVec3::ZERO).unwrap().rgb, [155, 85, 60]);
        // Et le modèle importé s'enregistre au format du jeu
        let bytes = models[1].to_bytes().unwrap();
        assert_eq!(Model::from_bytes(&bytes).unwrap(), models[1]);
    }

    #[test]
    fn every_pixel_world_block_has_a_color() {
        let blocks = "Grass Dirt Stone Sand Snow Water Coral Flesh Bone Root Mushroom Mycelium Glass Crystal Silicon Metal Ash Obsidian \
Mist Paper DarkVoid Mirror Cloud Glow Mercury Petrified Rust Marble Moss Brick Flower RedFlower TallGrass Ice DeadGrass Wood Leaves \
OrangeLeaves RedLeaves PinkLeaves";
        for b in blocks.split_whitespace() {
            assert!(block_color(b).is_some(), "{b}");
        }
        assert!(block_color("Air").is_none());
        assert!(from_pixel_world("pas du json").is_err());
    }
}
