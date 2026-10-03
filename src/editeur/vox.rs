//! Import / export MagicaVoxel `.vox` (E8).
//!
//! Format (version 150) : `VOX ` + `MAIN` qui contient des paires `SIZE` / `XYZI` (un modèle de
//! 256³ au plus chacun), la palette `RGBA` (256 couleurs, l'index 0 est vide), les matières `MATL`
//! (verre, métal, lumineux) et un graphe de scène (`nTRN` / `nGRP` / `nSHP`) qui place les modèles.
//! MagicaVoxel a z vers le haut : (x, y, z) du `.vox` = (x, z, y) ici. Un modèle plus grand que 256
//! est coupé en morceaux de 256³ placés par le graphe de scène.

use std::collections::HashMap;

use bevy::math::{IVec3, UVec3};

use super::format::{Material, Model, ModelKind, PaletteEntry, ShipCategory};

/// Côté d'un morceau.
const PIECE: i32 = 256;

fn chunk(id: &[u8; 4], content: &[u8], children: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(12 + content.len() + children.len());
    out.extend_from_slice(id);
    out.extend_from_slice(&(content.len() as u32).to_le_bytes());
    out.extend_from_slice(&(children.len() as u32).to_le_bytes());
    out.extend_from_slice(content);
    out.extend_from_slice(children);
    out
}

fn dict(pairs: &[(&str, String)]) -> Vec<u8> {
    let mut out = (pairs.len() as u32).to_le_bytes().to_vec();
    for (k, v) in pairs {
        for s in [k.as_bytes(), v.as_bytes()] {
            out.extend_from_slice(&(s.len() as u32).to_le_bytes());
            out.extend_from_slice(s);
        }
    }
    out
}

/// Le modèle en `.vox`.
pub fn export(m: &Model) -> Vec<u8> {
    // Morceaux de 256³ (repère du .vox : z vers le haut)
    let mut pieces: HashMap<IVec3, Vec<[u8; 4]>> = HashMap::new();
    for (p, v) in m.voxels.iter() {
        let q = IVec3::new(p.x, p.z, p.y);
        let k = q.div_euclid(IVec3::splat(PIECE));
        let l = q - k * PIECE;
        pieces.entry(k).or_default().push([l.x as u8, l.y as u8, l.z as u8, v]);
    }
    let mut keys: Vec<IVec3> = pieces.keys().copied().collect();
    keys.sort_by_key(|k| (k.z, k.y, k.x));
    if keys.is_empty() {
        keys.push(IVec3::ZERO);
        pieces.insert(IVec3::ZERO, Vec::new());
    }
    let size = IVec3::new(m.size.x as i32, m.size.z as i32, m.size.y as i32);
    let mut body = Vec::new();
    for k in &keys {
        let s = (size - *k * PIECE).clamp(IVec3::ONE, IVec3::splat(PIECE));
        let mut c = Vec::new();
        for x in [s.x, s.y, s.z] {
            c.extend_from_slice(&(x as u32).to_le_bytes());
        }
        body.extend(chunk(b"SIZE", &c, &[]));
        let vox = &pieces[k];
        let mut c = (vox.len() as u32).to_le_bytes().to_vec();
        for v in vox {
            c.extend_from_slice(v);
        }
        body.extend(chunk(b"XYZI", &c, &[]));
    }
    // Graphe de scène : transformation racine -> groupe -> (transformation -> forme) par morceau
    let mut scene = Vec::new();
    let n = keys.len() as u32;
    let mut node = |content: Vec<u8>, id: &[u8; 4]| scene.extend(chunk(id, &content, &[]));
    let ntrn = |id: u32, child: u32, t: Option<IVec3>| {
        let mut c = id.to_le_bytes().to_vec();
        c.extend(dict(&[]));
        c.extend_from_slice(&child.to_le_bytes());
        c.extend_from_slice(&(-1i32).to_le_bytes());
        c.extend_from_slice(&0u32.to_le_bytes());
        c.extend_from_slice(&1u32.to_le_bytes());
        c.extend(dict(&t.map(|t| vec![("_t", format!("{} {} {}", t.x, t.y, t.z))]).unwrap_or_default()));
        c
    };
    node(ntrn(0, 1, None), b"nTRN");
    // Un calque (MagicaVoxel en attend au moins un)
    let mut layr = 0u32.to_le_bytes().to_vec();
    layr.extend(dict(&[("_name", "spacespore".to_string())]));
    layr.extend_from_slice(&(-1i32).to_le_bytes());
    node(layr, b"LAYR");
    let mut grp = 1u32.to_le_bytes().to_vec();
    grp.extend(dict(&[]));
    grp.extend_from_slice(&n.to_le_bytes());
    for i in 0..n {
        grp.extend_from_slice(&(2 + i * 2).to_le_bytes());
    }
    node(grp, b"nGRP");
    for (i, k) in keys.iter().enumerate() {
        let i = i as u32;
        // MagicaVoxel place le centre d'un modèle à sa translation
        let s = (size - *k * PIECE).clamp(IVec3::ONE, IVec3::splat(PIECE));
        let t = *k * PIECE + s / 2;
        node(ntrn(2 + i * 2, 3 + i * 2, Some(t)), b"nTRN");
        let mut shp = (3 + i * 2).to_le_bytes().to_vec();
        shp.extend(dict(&[]));
        shp.extend_from_slice(&1u32.to_le_bytes());
        shp.extend_from_slice(&i.to_le_bytes());
        shp.extend(dict(&[]));
        node(shp, b"nSHP");
    }
    body.extend(scene);
    // Palette (index i du modèle = couleur i du .vox ; l'entrée i - 1 du chunk RGBA)
    let mut rgba = Vec::with_capacity(1024);
    for i in 0..256 {
        let e = m.palette.get(i).copied().unwrap_or(PaletteEntry { rgb: [0, 0, 0], material: Material::Mate });
        rgba.extend_from_slice(&[e.rgb[0], e.rgb[1], e.rgb[2], 255]);
    }
    body.extend(chunk(b"RGBA", &rgba, &[]));
    // Matières
    for (i, e) in m.palette.iter().enumerate() {
        let kind = match e.material {
            Material::Mate => continue,
            Material::Metal => vec![("_type", "_metal".to_string()), ("_metal", "0.8".to_string()), ("_rough", "0.3".to_string())],
            Material::Verre => vec![("_type", "_glass".to_string()), ("_trans", "0.6".to_string())],
            Material::Lumineuse => vec![("_type", "_emit".to_string()), ("_emit", "1".to_string())],
        };
        let mut c = ((i + 1) as u32).to_le_bytes().to_vec();
        c.extend(dict(&kind));
        body.extend(chunk(b"MATL", &c, &[]));
    }
    let mut out = b"VOX ".to_vec();
    out.extend_from_slice(&150u32.to_le_bytes());
    out.extend(chunk(b"MAIN", &[], &body));
    out
}

struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let s = self.b.get(self.at..self.at + n).ok_or("fichier .vox tronque")?;
        self.at += n;
        Ok(s)
    }

    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn i32(&mut self) -> Result<i32, String> {
        Ok(self.u32()? as i32)
    }

    fn string(&mut self) -> Result<String, String> {
        let n = self.u32()? as usize;
        if n > 4096 {
            return Err("chaine trop longue".into());
        }
        Ok(String::from_utf8_lossy(self.take(n)?).into_owned())
    }

    fn dict(&mut self) -> Result<HashMap<String, String>, String> {
        let n = self.u32()?;
        if n > 256 {
            return Err("dictionnaire trop long".into());
        }
        (0..n).map(|_| Ok((self.string()?, self.string()?))).collect()
    }
}

/// Palette par défaut de MagicaVoxel (quand le fichier n'a pas de RGBA) : un dégradé simple.
fn default_palette() -> Vec<[u8; 4]> {
    (0..256u32).map(|i| { let v = (i * 37 % 256) as u8; [v, 255 - v, (i * 91 % 256) as u8, 255] }).collect()
}

/// Un `.vox` -> un modèle (type « Autre », ou vaisseau s'il est grand).
pub fn import(bytes: &[u8], name: &str) -> Result<Model, String> {
    let mut r = Reader { b: bytes, at: 0 };
    if r.take(4)? != b"VOX " {
        return Err("pas un fichier MagicaVoxel (.vox)".into());
    }
    let _version = r.u32()?;
    if r.take(4)? != b"MAIN" {
        return Err("chunk MAIN manquant".into());
    }
    let (n, _) = (r.u32()?, r.u32()?);
    r.take(n as usize)?;
    let mut sizes: Vec<IVec3> = Vec::new();
    let mut models: Vec<Vec<[u8; 4]>> = Vec::new();
    let mut palette = default_palette();
    let mut mats: HashMap<u32, Material> = HashMap::new();
    // Graphe : nTRN (id -> enfant, translation), nGRP (id -> enfants), nSHP (id -> modèle)
    let mut trn: HashMap<i32, (i32, IVec3)> = HashMap::new();
    let mut grp: HashMap<i32, Vec<i32>> = HashMap::new();
    let mut shp: HashMap<i32, Vec<i32>> = HashMap::new();
    while r.at + 12 <= bytes.len() {
        let id: [u8; 4] = r.take(4)?.try_into().unwrap();
        let (n, m) = (r.u32()? as usize, r.u32()? as usize);
        let content = r.take(n)?;
        let mut c = Reader { b: content, at: 0 };
        match &id {
            b"SIZE" => sizes.push(IVec3::new(c.i32()?, c.i32()?, c.i32()?)),
            b"XYZI" => {
                let k = c.u32()? as usize;
                if k > 16_777_216 {
                    return Err("trop de voxels".into());
                }
                models.push((0..k).map(|_| Ok(c.take(4)?.try_into().unwrap())).collect::<Result<_, String>>()?);
            }
            b"RGBA" => {
                palette = (0..256).map(|_| Ok(c.take(4)?.try_into().unwrap())).collect::<Result<_, String>>()?;
            }
            b"MATL" => {
                let i = c.u32()?;
                let d = c.dict()?;
                let mat = match d.get("_type").map(String::as_str) {
                    Some("_glass") => Material::Verre,
                    Some("_metal") => Material::Metal,
                    Some("_emit") => Material::Lumineuse,
                    _ => Material::Mate,
                };
                mats.insert(i, mat);
            }
            b"nTRN" => {
                let node = c.i32()?;
                c.dict()?;
                let child = c.i32()?;
                c.i32()?;
                c.i32()?;
                let frames = c.u32()?;
                let mut t = IVec3::ZERO;
                for _ in 0..frames.min(1) {
                    if let Some(s) = c.dict()?.get("_t") {
                        let v: Vec<i32> = s.split_whitespace().filter_map(|x| x.parse().ok()).collect();
                        if v.len() == 3 {
                            t = IVec3::new(v[0], v[1], v[2]);
                        }
                    }
                }
                trn.insert(node, (child, t));
            }
            b"nGRP" => {
                let node = c.i32()?;
                c.dict()?;
                let k = c.u32()?.min(65_536);
                grp.insert(node, (0..k).map(|_| c.i32()).collect::<Result<_, String>>()?);
            }
            b"nSHP" => {
                let node = c.i32()?;
                c.dict()?;
                let k = c.u32()?.min(65_536);
                let mut ids = Vec::new();
                for _ in 0..k {
                    ids.push(c.i32()?);
                    c.dict()?;
                }
                shp.insert(node, ids);
            }
            _ => {}
        }
        r.take(m)?;
    }
    if sizes.len() != models.len() || models.is_empty() {
        return Err("fichier .vox sans modele".into());
    }
    // Où va chaque modèle : la somme des translations du graphe (coin = centre - taille / 2)
    let mut place: Vec<IVec3> = vec![IVec3::ZERO; models.len()];
    let mut placed = vec![false; models.len()];
    fn walk(node: i32, t: IVec3, trn: &HashMap<i32, (i32, IVec3)>, grp: &HashMap<i32, Vec<i32>>, shp: &HashMap<i32, Vec<i32>>, sizes: &[IVec3], place: &mut [IVec3], placed: &mut [bool], depth: u32) {
        if depth > 64 {
            return;
        }
        if let Some((child, dt)) = trn.get(&node) {
            walk(*child, t + *dt, trn, grp, shp, sizes, place, placed, depth + 1);
        } else if let Some(children) = grp.get(&node) {
            for c in children {
                walk(*c, t, trn, grp, shp, sizes, place, placed, depth + 1);
            }
        } else if let Some(ids) = shp.get(&node) {
            for id in ids {
                if let (Some(s), Some(p)) = (sizes.get(*id as usize), place.get_mut(*id as usize)) {
                    *p = t - *s / 2;
                    placed[*id as usize] = true;
                }
            }
        }
    }
    if trn.contains_key(&0) {
        walk(0, IVec3::ZERO, &trn, &grp, &shp, &sizes, &mut place, &mut placed, 0);
    }
    // Boîte de tous les modèles, ramenée à l'origine
    let (mut lo, mut hi) = (IVec3::MAX, IVec3::MIN);
    for (i, s) in sizes.iter().enumerate() {
        let p = if placed[i] { place[i] } else { IVec3::ZERO };
        lo = lo.min(p);
        hi = hi.max(p + *s);
    }
    let span = (hi - lo).max(IVec3::ONE);
    // Repère du jeu : y vers le haut
    let size = UVec3::new(span.x as u32, span.z as u32, span.y as u32);
    if size.max_element() > ShipCategory::Capital.grid() {
        return Err(format!("modele trop grand ({} x {} x {}, 1024 au plus)", size.x, size.y, size.z));
    }
    let kind = if size.max_element() > super::format::OTHER_MAX_GRID { ModelKind::Vaisseau } else { ModelKind::Autre };
    let category = (kind == ModelKind::Vaisseau).then(|| ShipCategory::ALL.into_iter().find(|c| c.grid() >= size.max_element()).unwrap_or(ShipCategory::Capital));
    let mut m = Model::new(name, kind, category);
    m.size = match category {
        Some(c) => UVec3::splat(c.grid()),
        None => size.max(UVec3::splat(8)),
    };
    // Couleurs utilisées seulement (255 au plus : les index du .vox se suivent)
    let mut index = [0u8; 256];
    for (i, vox) in models.iter().enumerate() {
        let p = if placed[i] { place[i] } else { IVec3::ZERO } - lo;
        for v in vox {
            let c = v[3];
            if c == 0 {
                continue;
            }
            if index[c as usize] == 0 {
                let rgba = palette[(c as usize + 255) % 256];
                let e = PaletteEntry { rgb: [rgba[0], rgba[1], rgba[2]], material: mats.get(&(c as u32)).copied().unwrap_or(Material::Mate) };
                index[c as usize] = m.color_index(e).ok_or("plus de 255 couleurs")?;
            }
            let q = p + IVec3::new(v[0] as i32, v[1] as i32, v[2] as i32);
            m.voxels.set(IVec3::new(q.x, q.z, q.y), index[c as usize]);
        }
    }
    Ok(m)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(size: u32) -> Model {
        let mut m = Model::new("V", ModelKind::Autre, None);
        m.size = UVec3::splat(size);
        let a = m.color_index(PaletteEntry { rgb: [200, 30, 30], material: Material::Mate }).unwrap();
        let g = m.color_index(PaletteEntry { rgb: [120, 200, 250], material: Material::Verre }).unwrap();
        let l = m.color_index(PaletteEntry { rgb: [255, 200, 40], material: Material::Lumineuse }).unwrap();
        for x in 0..size as i32 {
            m.voxels.set(IVec3::new(x, 0, 0), a);
            m.voxels.set(IVec3::new(x, (x * 3) % size as i32, size as i32 - 1), if x % 2 == 0 { g } else { l });
        }
        m
    }

    #[test]
    fn vox_round_trip_keeps_voxels_colors_and_materials() {
        let m = sample(20);
        let back = import(&export(&m), "V").unwrap();
        assert_eq!(back.voxels.count(), m.voxels.count());
        for (p, _) in m.voxels.iter() {
            assert_eq!(back.color_at(p), m.color_at(p), "{p}");
        }
    }

    #[test]
    fn big_models_are_split_in_256_pieces() {
        let mut m = sample(300);
        m.kind = ModelKind::Vaisseau;
        m.category = Some(ShipCategory::Croiseur);
        m.size = UVec3::splat(512);
        m.voxels.set(IVec3::new(511, 511, 511), 1);
        let bytes = export(&m);
        let back = import(&bytes, "Gros").unwrap();
        assert_eq!(back.kind, ModelKind::Vaisseau);
        assert_eq!(back.voxels.count(), m.voxels.count());
        assert_eq!(back.color_at(IVec3::new(511, 511, 511)), m.color_at(IVec3::new(511, 511, 511)));
        assert_eq!(back.color_at(IVec3::new(299, 0, 0)), m.color_at(IVec3::new(299, 0, 0)));
        assert!(import(b"PNG ....", "x").is_err());
    }
}
