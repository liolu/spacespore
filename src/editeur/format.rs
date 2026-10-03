//! Format des modèles voxel `.ssvox` (règles 1 et 2 de `ROADMAP-0.12-editeur.md`).
//!
//! - Un modèle : type (personnage, vaisseau, autre), race ou catégorie, taille de grille, palette de
//!   255 couleurs au plus (chacune avec sa matière : mate, métal, verre, lumineuse), voxels, zones de
//!   mouvement et étiquettes. Un voxel = un octet : 0 = vide, sinon l'entrée de la palette (comme
//!   MagicaVoxel).
//! - Stockage creux par chunks de 32³ : un chunk absent est vide, un chunk **uniforme** tient en un
//!   octet, un chunk mêlé en 32 Kio. La mémoire suit ce qui est construit, pas la taille de la grille
//!   (un vaisseau capital fait 1024³ cases).
//! - Fichier : une archive zip (deflate) avec `meta.json` (tout sauf les voxels) et `voxels.bin`
//!   (chunks codés en RLE), plus `zones.bin` (à quelle zone de mouvement appartient chaque voxel).
//!   Au plus 10 Mo (Q4).

// Fondations (E0) : la lecture voxel par voxel, les chunks pleins et l'empreinte servent à partir
// de E1 (édition), E3 (grands vaisseaux) et E7 (réseau).
#![allow(dead_code)]

use bevy::math::{IVec3, UVec3};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Cursor, Read, Write};

/// Version du format (à augmenter si `meta.json` ou les chunks changent de sens).
pub const FORMAT_VERSION: u32 = 1;
/// Côté d'un chunk.
pub const CHUNK: i32 = 32;
const CHUNK_VOLUME: usize = (CHUNK * CHUNK * CHUNK) as usize;
/// Poids maximal d'un fichier (Q4) : c'est aussi ce qu'on envoie aux autres joueurs.
pub const MAX_FILE_BYTES: usize = 10 * 1024 * 1024;
/// Entrées de palette au plus (l'index 0 est le vide).
pub const MAX_COLORS: usize = 255;

/// Type de modèle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModelKind {
    #[default]
    Personnage,
    Vaisseau,
    Autre,
}

impl ModelKind {
    pub const ALL: [ModelKind; 3] = [ModelKind::Personnage, ModelKind::Vaisseau, ModelKind::Autre];

    pub fn name(self) -> &'static str {
        match self {
            ModelKind::Personnage => "Personnage",
            ModelKind::Vaisseau => "Vaisseau",
            ModelKind::Autre => "Autre",
        }
    }
}

/// Catégorie de vaisseau : elle fixe la taille de la grille (§1.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ShipCategory {
    Chasseur,
    Corvette,
    Fregate,
    Croiseur,
    Capital,
}

impl ShipCategory {
    pub const ALL: [ShipCategory; 5] = [ShipCategory::Chasseur, ShipCategory::Corvette, ShipCategory::Fregate, ShipCategory::Croiseur, ShipCategory::Capital];

    pub fn name(self) -> &'static str {
        match self {
            ShipCategory::Chasseur => "Chasseur",
            ShipCategory::Corvette => "Corvette",
            ShipCategory::Fregate => "Fregate",
            ShipCategory::Croiseur => "Croiseur",
            ShipCategory::Capital => "Capital",
        }
    }

    /// Côté de la grille (cube).
    pub fn grid(self) -> u32 {
        match self {
            ShipCategory::Chasseur => 64,
            ShipCategory::Corvette => 128,
            ShipCategory::Fregate => 256,
            ShipCategory::Croiseur => 512,
            ShipCategory::Capital => 1024,
        }
    }
}

/// Grille d'un personnage (Q6) : 16 de large, 32 de haut, 32 de long.
pub const CHARACTER_GRID: UVec3 = UVec3::new(16, 32, 32);
/// Grille libre d'un objet « Autre » : jusqu'à 64³.
pub const OTHER_MAX_GRID: u32 = 64;

/// Matière d'une couleur (§2.2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Material {
    #[default]
    Mate,
    Metal,
    Verre,
    Lumineuse,
}

impl Material {
    pub fn name(self) -> &'static str {
        match self {
            Material::Mate => "mate",
            Material::Metal => "metal",
            Material::Verre => "verre",
            Material::Lumineuse => "lumineuse",
        }
    }
}

/// Une entrée de la palette du modèle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PaletteEntry {
    pub rgb: [u8; 3],
    pub material: Material,
}

/// Zone de mouvement (blocs de mouvement, E4, `motion.rs`) : une partie d'un bloc posé, avec son
/// parent (zones emboîtées), son pivot (repère du modèle) et l'orientation du bloc (ses animations
/// sont tournées et reflétées de même).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Zone {
    /// Nom affiché.
    pub name: String,
    /// Bloc de mouvement d'origine (« bras », « porte_pivotante »...).
    pub block: String,
    /// Partie du bloc (ses pistes d'animation).
    #[serde(default)]
    pub part: String,
    /// Zone parente (index dans `zones`).
    pub parent: Option<u16>,
    pub pivot: [f32; 3],
    /// Quarts de tour autour de y.
    #[serde(default)]
    pub turn: u8,
    #[serde(default)]
    pub mirror: bool,
    /// Taille (1 = celle du gabarit ; 0 lu comme 1).
    #[serde(default)]
    pub scale: u8,
}

/// Un chunk de 32³ voxels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Chunk {
    /// Toutes les cases ont la même valeur (non nulle).
    Uniform(u8),
    /// Cases mêlées, indexées x + 32·(y + 32·z).
    Full(Box<[u8]>),
}

impl Chunk {
    fn get(&self, i: usize) -> u8 {
        match self {
            Chunk::Uniform(v) => *v,
            Chunk::Full(d) => d[i],
        }
    }

    fn count(&self) -> usize {
        match self {
            Chunk::Uniform(_) => CHUNK_VOLUME,
            Chunk::Full(d) => d.iter().filter(|v| **v != 0).count(),
        }
    }
}

/// Une grille creuse d'octets par chunks (voxels, ou numéros de zone).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Sparse {
    chunks: HashMap<IVec3, Chunk>,
}

fn split(p: IVec3) -> (IVec3, usize) {
    let c = p.div_euclid(IVec3::splat(CHUNK));
    let l = p.rem_euclid(IVec3::splat(CHUNK));
    (c, (l.x + CHUNK * (l.y + CHUNK * l.z)) as usize)
}

impl Sparse {
    pub fn get(&self, p: IVec3) -> u8 {
        let (c, i) = split(p);
        self.chunks.get(&c).map_or(0, |ch| ch.get(i))
    }

    pub fn set(&mut self, p: IVec3, v: u8) {
        let (c, i) = split(p);
        match self.chunks.get_mut(&c) {
            None if v == 0 => {}
            None => {
                let mut d = vec![0u8; CHUNK_VOLUME].into_boxed_slice();
                d[i] = v;
                self.chunks.insert(c, Chunk::Full(d));
            }
            Some(Chunk::Uniform(u)) if *u == v => {}
            Some(ch @ Chunk::Uniform(_)) => {
                let Chunk::Uniform(u) = *ch else { unreachable!() };
                let mut d = vec![u; CHUNK_VOLUME].into_boxed_slice();
                d[i] = v;
                *ch = Chunk::Full(d);
            }
            Some(Chunk::Full(d)) => {
                d[i] = v;
                // Chunk devenu vide ou uniforme : il se replie
                let first = d[0];
                if d[i] == first && d.iter().all(|x| *x == first) {
                    if first == 0 {
                        self.chunks.remove(&c);
                    } else {
                        self.chunks.insert(c, Chunk::Uniform(first));
                    }
                }
            }
        }
    }

    /// Remplit tout un chunk d'un coup (outils de volume).
    pub fn fill_chunk(&mut self, c: IVec3, v: u8) {
        if v == 0 {
            self.chunks.remove(&c);
        } else {
            self.chunks.insert(c, Chunk::Uniform(v));
        }
    }

    /// Nombre de voxels non vides.
    pub fn count(&self) -> usize {
        self.chunks.values().map(Chunk::count).sum()
    }

    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// Mémoire occupée (octets, environ).
    pub fn memory(&self) -> usize {
        self.chunks.values().map(|c| match c {
            Chunk::Uniform(_) => 16,
            Chunk::Full(_) => CHUNK_VOLUME + 16,
        }).sum::<usize>() + self.chunks.capacity() * 24
    }

    /// Tous les voxels non vides (position, valeur).
    pub fn iter(&self) -> impl Iterator<Item = (IVec3, u8)> + '_ {
        self.chunks.iter().flat_map(|(c, ch)| {
            let base = *c * CHUNK;
            (0..CHUNK_VOLUME).filter_map(move |i| {
                let v = ch.get(i);
                (v != 0).then(|| {
                    let i = i as i32;
                    (base + IVec3::new(i % CHUNK, (i / CHUNK) % CHUNK, i / (CHUNK * CHUNK)), v)
                })
            })
        })
    }

    /// Chunks codés : (position, 1 = uniforme + valeur | 2 = RLE (valeur, longueur u16)...).
    fn encode(&self) -> Vec<u8> {
        let mut keys: Vec<&IVec3> = self.chunks.keys().collect();
        keys.sort_by_key(|k| (k.z, k.y, k.x));
        let mut out = Vec::new();
        out.extend_from_slice(&(keys.len() as u32).to_le_bytes());
        for k in keys {
            for c in [k.x, k.y, k.z] {
                out.extend_from_slice(&c.to_le_bytes());
            }
            match &self.chunks[k] {
                Chunk::Uniform(v) => {
                    out.push(1);
                    out.push(*v);
                }
                Chunk::Full(d) => {
                    out.push(2);
                    let runs = rle(d);
                    out.extend_from_slice(&(runs.len() as u32).to_le_bytes());
                    for (v, n) in runs {
                        out.push(v);
                        out.extend_from_slice(&n.to_le_bytes());
                    }
                }
            }
        }
        out
    }

    fn decode(bytes: &[u8]) -> Result<Self, String> {
        let mut r = Reader { b: bytes, at: 0 };
        let n = r.u32()? as usize;
        let mut chunks = HashMap::with_capacity(n);
        for _ in 0..n {
            let k = IVec3::new(r.i32()?, r.i32()?, r.i32()?);
            let ch = match r.u8()? {
                1 => Chunk::Uniform(r.u8()?),
                2 => {
                    let runs = r.u32()? as usize;
                    let mut d = Vec::with_capacity(CHUNK_VOLUME);
                    for _ in 0..runs {
                        let v = r.u8()?;
                        let len = r.u16()? as usize;
                        if d.len() + len > CHUNK_VOLUME {
                            return Err("chunk trop long".into());
                        }
                        d.extend(std::iter::repeat(v).take(len));
                    }
                    if d.len() != CHUNK_VOLUME {
                        return Err("chunk incomplet".into());
                    }
                    Chunk::Full(d.into_boxed_slice())
                }
                t => return Err(format!("chunk de type inconnu {t}")),
            };
            chunks.insert(k, ch);
        }
        Ok(Self { chunks })
    }
}

/// Plages de valeurs identiques (au plus 65 535 par plage).
fn rle(d: &[u8]) -> Vec<(u8, u16)> {
    let mut out: Vec<(u8, u16)> = Vec::new();
    for &v in d {
        match out.last_mut() {
            Some((pv, n)) if *pv == v && *n < u16::MAX => *n += 1,
            _ => out.push((v, 1)),
        }
    }
    out
}

struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], String> {
        let s = self.b.get(self.at..self.at + n).ok_or("fichier tronque")?;
        self.at += n;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, String> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn i32(&mut self) -> Result<i32, String> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
}

/// Tout sauf les voxels (`meta.json`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Meta {
    version: u32,
    name: String,
    kind: ModelKind,
    #[serde(default)]
    race: Option<String>,
    #[serde(default)]
    category: Option<ShipCategory>,
    size: [u32; 3],
    palette: Vec<PaletteEntry>,
    #[serde(default)]
    zones: Vec<Zone>,
    #[serde(default)]
    tags: Vec<String>,
}

/// Un modèle voxel.
#[derive(Clone, Debug, PartialEq)]
pub struct Model {
    pub name: String,
    pub kind: ModelKind,
    pub race: Option<String>,
    pub category: Option<ShipCategory>,
    pub size: UVec3,
    /// Couleurs (index 1..=255 dans les voxels : l'entrée `i - 1`).
    pub palette: Vec<PaletteEntry>,
    pub voxels: Sparse,
    pub zones: Vec<Zone>,
    /// Zone de chaque voxel (0 = corps fixe, sinon l'entrée `i - 1` de `zones`).
    pub zone_map: Sparse,
    pub tags: Vec<String>,
}

impl Model {
    /// Modèle vide d'un type (grille à sa taille).
    pub fn new(name: &str, kind: ModelKind, category: Option<ShipCategory>) -> Self {
        let size = match (kind, category) {
            (ModelKind::Personnage, _) => CHARACTER_GRID,
            (ModelKind::Vaisseau, Some(c)) => UVec3::splat(c.grid()),
            (ModelKind::Vaisseau, None) => UVec3::splat(ShipCategory::Chasseur.grid()),
            (ModelKind::Autre, _) => UVec3::splat(32),
        };
        Self { name: name.to_string(), kind, race: None, category, size, palette: Vec::new(), voxels: Sparse::default(), zones: Vec::new(), zone_map: Sparse::default(), tags: Vec::new() }
    }

    pub fn in_bounds(&self, p: IVec3) -> bool {
        p.cmpge(IVec3::ZERO).all() && p.cmplt(self.size.as_ivec3()).all()
    }

    /// Index de palette d'une couleur (ajoutée si elle manque) ; `None` : palette pleine.
    pub fn color_index(&mut self, e: PaletteEntry) -> Option<u8> {
        if let Some(i) = self.palette.iter().position(|p| *p == e) {
            return Some(i as u8 + 1);
        }
        if self.palette.len() >= MAX_COLORS {
            return None;
        }
        self.palette.push(e);
        Some(self.palette.len() as u8)
    }

    /// Retire de la palette les couleurs qui ne servent plus (à l'enregistrement) ; les voxels
    /// suivent leur nouvel index.
    pub fn compact_palette(&mut self) {
        let mut used = [false; 256];
        for (_, v) in self.voxels.iter() {
            used[v as usize] = true;
        }
        let mut remap = [0u8; 256];
        let mut kept = Vec::new();
        for (i, e) in self.palette.iter().enumerate() {
            if used[i + 1] {
                kept.push(*e);
                remap[i + 1] = kept.len() as u8;
            }
        }
        if kept.len() == self.palette.len() {
            return;
        }
        let cells: Vec<(IVec3, u8)> = self.voxels.iter().collect();
        for (p, v) in cells {
            self.voxels.set(p, remap[v as usize]);
        }
        self.palette = kept;
    }

    /// Couleur du voxel `p` (`None` : vide).
    pub fn color_at(&self, p: IVec3) -> Option<PaletteEntry> {
        match self.voxels.get(p) {
            0 => None,
            v => self.palette.get(v as usize - 1).copied(),
        }
    }

    /// Fichier `.ssvox` (archive zip, deflate).
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        let meta = Meta {
            version: FORMAT_VERSION,
            name: self.name.clone(),
            kind: self.kind,
            race: self.race.clone(),
            category: self.category,
            size: self.size.to_array(),
            palette: self.palette.clone(),
            zones: self.zones.clone(),
            tags: self.tags.clone(),
        };
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        let err = |e: &dyn std::fmt::Display| format!("ecriture du modele : {e}");
        for (name, data) in [
            ("meta.json", serde_json::to_vec_pretty(&meta).map_err(|e| err(&e))?),
            ("voxels.bin", self.voxels.encode()),
            ("zones.bin", self.zone_map.encode()),
        ] {
            zip.start_file(name, opts).map_err(|e| err(&e))?;
            zip.write_all(&data).map_err(|e| err(&e))?;
        }
        let bytes = zip.finish().map_err(|e| err(&e))?.into_inner();
        if bytes.len() > MAX_FILE_BYTES {
            return Err(format!("modele trop lourd : {:.1} / 10 Mo", bytes.len() as f64 / (1024.0 * 1024.0)));
        }
        Ok(bytes)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_FILE_BYTES {
            return Err("fichier de plus de 10 Mo".into());
        }
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| format!("pas un modele .ssvox : {e}"))?;
        let mut read = |name: &str| -> Result<Vec<u8>, String> {
            let mut f = zip.by_name(name).map_err(|e| format!("{name} manquant : {e}"))?;
            let mut out = Vec::new();
            // Jamais plus que 64 fois le fichier (archive piégée)
            (&mut f).take((MAX_FILE_BYTES * 64) as u64).read_to_end(&mut out).map_err(|e| format!("{name} illisible : {e}"))?;
            Ok(out)
        };
        let meta: Meta = serde_json::from_slice(&read("meta.json")?).map_err(|e| format!("meta.json : {e}"))?;
        if meta.version > FORMAT_VERSION {
            return Err(format!("modele d'une version plus recente ({}) : mettez le jeu a jour", meta.version));
        }
        if meta.palette.len() > MAX_COLORS {
            return Err("palette de plus de 255 couleurs".into());
        }
        let voxels = Sparse::decode(&read("voxels.bin")?)?;
        let zone_map = match read("zones.bin") {
            Ok(b) => Sparse::decode(&b)?,
            Err(_) => Sparse::default(),
        };
        Ok(Self {
            name: meta.name,
            kind: meta.kind,
            race: meta.race,
            category: meta.category,
            size: UVec3::from_array(meta.size),
            palette: meta.palette,
            voxels,
            zones: meta.zones,
            zone_map,
            tags: meta.tags,
        })
    }

    /// Empreinte du fichier (réseau, règle 7) : FNV-1a 64 bits.
    pub fn fingerprint(bytes: &[u8]) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in bytes {
            h ^= *b as u64;
            h = h.wrapping_mul(0x100_0000_01b3);
        }
        h
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Model {
        let mut m = Model::new("Essai", ModelKind::Vaisseau, Some(ShipCategory::Chasseur));
        let red = m.color_index(PaletteEntry { rgb: [200, 30, 30], material: Material::Mate }).unwrap();
        let glass = m.color_index(PaletteEntry { rgb: [150, 200, 255], material: Material::Verre }).unwrap();
        // Un chunk plein d'une couleur, un chunk mêlé, des voxels aux coins
        m.voxels.fill_chunk(IVec3::new(1, 0, 0), red);
        for x in 0..20 {
            m.voxels.set(IVec3::new(x, 3, 5), if x % 3 == 0 { glass } else { red });
        }
        m.voxels.set(IVec3::new(63, 63, 63), glass);
        m.zones.push(Zone { name: "Aile gauche".into(), block: "aile".into(), part: "aile".into(), parent: None, pivot: [10.0, 3.0, 5.0], turn: 1, mirror: true, scale: 2 });
        m.zone_map.set(IVec3::new(4, 3, 5), 1);
        m.tags = vec!["chasseur".into(), "rouge".into()];
        m
    }

    #[test]
    fn models_round_trip() {
        let m = sample();
        let bytes = m.to_bytes().unwrap();
        let back = Model::from_bytes(&bytes).unwrap();
        assert_eq!(back, m);
        assert_eq!(back.voxels.count(), 32 * 32 * 32 + 20 + 1);
        assert_eq!(back.color_at(IVec3::new(3, 3, 5)).unwrap().material, Material::Verre);
        assert_eq!(back.zone_map.get(IVec3::new(4, 3, 5)), 1);
        // Même modèle, même fichier, même empreinte
        assert_eq!(Model::fingerprint(&bytes), Model::fingerprint(&m.to_bytes().unwrap()));
    }

    #[test]
    fn sparse_chunks_fold_and_unfold() {
        let mut s = Sparse::default();
        s.fill_chunk(IVec3::ZERO, 7);
        assert_eq!(s.memory() < 200, true);
        s.set(IVec3::new(1, 2, 3), 9);
        assert_eq!(s.get(IVec3::new(1, 2, 3)), 9);
        assert_eq!(s.get(IVec3::new(4, 4, 4)), 7);
        // Revient uniforme, puis vide
        s.set(IVec3::new(1, 2, 3), 7);
        assert_eq!(s.chunks[&IVec3::ZERO], Chunk::Uniform(7));
        s.fill_chunk(IVec3::ZERO, 0);
        s.set(IVec3::new(-1, -1, -1), 3);
        assert_eq!(s.get(IVec3::new(-1, -1, -1)), 3, "coordonnees negatives");
        s.set(IVec3::new(-1, -1, -1), 0);
        assert_eq!(s.chunk_count(), 0);
    }

    #[test]
    fn a_full_capital_ship_is_cheap_when_uniform() {
        // Une coque pleine de 512³ : 4 096 chunks uniformes, quelques Ko seulement
        let mut m = Model::new("Bloc", ModelKind::Vaisseau, Some(ShipCategory::Capital));
        let c = m.color_index(PaletteEntry { rgb: [90, 90, 100], material: Material::Metal }).unwrap();
        for z in 0..16 {
            for y in 0..16 {
                for x in 0..16 {
                    m.voxels.fill_chunk(IVec3::new(x, y, z), c);
                }
            }
        }
        assert!(m.voxels.memory() < 1_000_000, "{}", m.voxels.memory());
        let bytes = m.to_bytes().unwrap();
        assert!(bytes.len() < 200_000, "{}", bytes.len());
        assert_eq!(Model::from_bytes(&bytes).unwrap().voxels.count(), 512 * 512 * 512);
    }

    #[test]
    fn unused_colors_are_dropped_on_save() {
        let mut m = Model::new("P", ModelKind::Autre, None);
        let a = m.color_index(PaletteEntry { rgb: [1, 1, 1], material: Material::Mate }).unwrap();
        let b = m.color_index(PaletteEntry { rgb: [2, 2, 2], material: Material::Mate }).unwrap();
        let c = m.color_index(PaletteEntry { rgb: [3, 3, 3], material: Material::Verre }).unwrap();
        m.voxels.set(IVec3::ZERO, a);
        m.voxels.set(IVec3::X, c);
        let _ = b;
        m.compact_palette();
        assert_eq!(m.palette.len(), 2);
        assert_eq!(m.color_at(IVec3::X).unwrap().material, Material::Verre);
        assert_eq!(m.color_at(IVec3::ZERO).unwrap().rgb, [1, 1, 1]);
    }

    #[test]
    fn palette_is_limited_to_255() {
        let mut m = Model::new("P", ModelKind::Autre, None);
        for i in 0..255u32 {
            assert!(m.color_index(PaletteEntry { rgb: [i as u8, (i / 2) as u8, 3], material: Material::Mate }).is_some());
        }
        assert!(m.color_index(PaletteEntry { rgb: [1, 2, 250], material: Material::Mate }).is_none());
        // Une couleur déjà là reste trouvée
        assert_eq!(m.color_index(PaletteEntry { rgb: [0, 0, 3], material: Material::Mate }), Some(1));
    }

    #[test]
    fn broken_files_are_refused() {
        assert!(Model::from_bytes(b"pas un zip").is_err());
        let mut bytes = sample().to_bytes().unwrap();
        bytes.truncate(bytes.len() / 2);
        assert!(Model::from_bytes(&bytes).is_err());
        assert!(Sparse::decode(&[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 9]).is_err());
    }

    #[test]
    fn grids_follow_the_model_type() {
        assert_eq!(Model::new("a", ModelKind::Personnage, None).size, UVec3::new(16, 32, 32));
        assert_eq!(Model::new("a", ModelKind::Vaisseau, Some(ShipCategory::Capital)).size, UVec3::splat(1024));
        let m = Model::new("a", ModelKind::Personnage, None);
        assert!(m.in_bounds(IVec3::new(15, 31, 31)) && !m.in_bounds(IVec3::new(16, 0, 0)));
    }
}
