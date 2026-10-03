//! Édition d'un modèle (E1), sans Bevy : outils, symétrie miroir, annuler / rétablir par lots,
//! rayon de la souris jusqu'à la case visée, maillage des faces visibles.
//!
//! Porté de `VoxelEditorManager.cs` (Pixel World) : outils Ajouter / Retirer / Peindre, pipette
//! (Maj+clic ou outil 4), un trait de la souris = une action annulable.

use bevy::math::{IVec3, Vec3};
use bevy::render::mesh::{Indices, Mesh, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use std::path::PathBuf;

use super::format::{Material, Model, ModelKind, PaletteEntry, Zone};
use super::motion::{self, BlockDef, Placement};

/// Outils de base (touches 1 à 4, les mêmes sur AZERTY : & é " ').
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tool {
    #[default]
    Add,
    Remove,
    Paint,
    Pick,
}

impl Tool {
    pub const ALL: [Tool; 4] = [Tool::Add, Tool::Remove, Tool::Paint, Tool::Pick];

    pub fn name(self) -> &'static str {
        match self {
            Tool::Add => "Ajouter (1)",
            Tool::Remove => "Retirer (2)",
            Tool::Paint => "Peindre (3)",
            Tool::Pick => "Pipette (4)",
        }
    }
}

/// Les 23 familles de races jouables (§3.2) : leur squelette et leurs animations arrivent en E5.
pub const RACES: [&str; 23] = [
    "Humanoide",
    "Humanoide animal",
    "Reptilien",
    "Aile celeste",
    "Demon",
    "Fee / insectoide aile",
    "Harpie / homme-oiseau",
    "Dragonoide",
    "Drakeide sans ailes",
    "Centaure",
    "Drider",
    "Lamia / naga",
    "Sirene / triton",
    "Slime",
    "Spectre",
    "Satyre / faune",
    "Minotaure",
    "Quadrupede animal",
    "Insectoide",
    "Cephalopode",
    "Golem / geant",
    "Dryade / sylvain",
    "Mecha / robot",
];

/// Étiquettes proposées (adaptées de `TagCategories`).
pub const TAG_GROUPS: [(&str, &[&str]); 5] = [
    ("Type", &["personnage", "vaisseau", "arme", "outil", "meuble", "decor", "vehicule", "creature", "plante"]),
    ("Style", &["militaire", "civil", "pirate", "alien", "ancien", "organique", "mecanique", "magique"]),
    ("Rarete", &["commun", "peu commun", "rare", "unique"]),
    ("Taille", &["petit", "moyen", "grand", "immense"]),
    ("Usage", &["sol", "mur", "plafond", "flottant", "sous l'eau"]),
];

/// Une case changée : position, ancien et nouvel index de palette, ancienne et nouvelle zone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Change {
    pub pos: IVec3,
    pub old: u8,
    pub new: u8,
    pub old_zone: u8,
    pub new_zone: u8,
}

/// Un lot annulable : des cases, et la liste des zones avant / après si elle a changé.
#[derive(Clone, Debug, Default)]
struct Batch {
    changes: Vec<Change>,
    zones: Option<(Vec<Zone>, Vec<Zone>)>,
}

impl Batch {
    fn is_empty(&self) -> bool {
        self.changes.is_empty() && self.zones.is_none()
    }
}

/// Un modèle ouvert (onglet) : le modèle, son fichier, et l'historique.
#[derive(Clone, Debug)]
pub struct Doc {
    pub model: Model,
    pub path: Option<PathBuf>,
    pub dirty: bool,
    undo: Vec<Batch>,
    redo: Vec<Batch>,
    /// Trait en cours (souris enfoncée) : un seul lot annulable.
    stroke: Option<Batch>,
    /// Le maillage est à refaire.
    pub mesh_dirty: bool,
}

/// Lots gardés au plus (les plus anciens s'oublient).
const MAX_UNDO: usize = 200;

/// Couleur des gabarits posés (« un bloc blanc restant est un bloc comme un autre »).
pub const BLOCK_WHITE: PaletteEntry = PaletteEntry { rgb: [255, 255, 255], material: Material::Mate };

impl Doc {
    pub fn new(model: Model, path: Option<PathBuf>) -> Self {
        Self { model, path, dirty: false, undo: Vec::new(), redo: Vec::new(), stroke: None, mesh_dirty: true }
    }

    /// Commence un trait (clic) : tout jusqu'au relâchement sera annulé d'un coup.
    pub fn begin(&mut self) {
        self.end();
        self.stroke = Some(Batch::default());
    }

    /// Fin du trait.
    pub fn end(&mut self) {
        if let Some(s) = self.stroke.take() {
            self.push(s);
        }
    }

    fn push(&mut self, b: Batch) {
        if b.is_empty() {
            return;
        }
        self.undo.push(b);
        if self.undo.len() > MAX_UNDO {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// Change une case (valeur et zone), dans le trait en cours.
    fn put(&mut self, q: IVec3, v: u8, zone: u8) -> bool {
        if !self.model.in_bounds(q) {
            return false;
        }
        let zone = if v == 0 { 0 } else { zone };
        let (old, old_zone) = (self.model.voxels.get(q), self.model.zone_map.get(q));
        if old == v && old_zone == zone {
            return false;
        }
        self.model.voxels.set(q, v);
        self.model.zone_map.set(q, zone);
        let c = Change { pos: q, old, new: v, old_zone, new_zone: zone };
        match &mut self.stroke {
            Some(s) => s.changes.push(c),
            None => self.push(Batch { changes: vec![c], zones: None }),
        }
        self.dirty = true;
        self.mesh_dirty = true;
        true
    }

    /// Pose la valeur `v` (0 = vide) en `p` (et en miroir si demandé), dans le trait en cours ;
    /// une case déjà pleine garde sa zone.
    pub fn set(&mut self, p: IVec3, v: u8, mirror: bool) -> bool {
        let mut changed = false;
        let targets = if mirror { mirrored(&self.model, p) } else { vec![p] };
        for q in targets {
            let zone = self.model.zone_map.get(q);
            changed |= self.put(q, v, zone);
        }
        changed
    }

    fn apply_batch(&mut self, b: &Batch, forward: bool) {
        let it: Box<dyn Iterator<Item = &Change>> = if forward { Box::new(b.changes.iter()) } else { Box::new(b.changes.iter().rev()) };
        for c in it {
            let (v, z) = if forward { (c.new, c.new_zone) } else { (c.old, c.old_zone) };
            self.model.voxels.set(c.pos, v);
            self.model.zone_map.set(c.pos, z);
        }
        if let Some((before, after)) = &b.zones {
            self.model.zones = if forward { after.clone() } else { before.clone() };
        }
        self.dirty = true;
        self.mesh_dirty = true;
    }

    pub fn undo(&mut self) -> bool {
        self.end();
        let Some(batch) = self.undo.pop() else { return false };
        self.apply_batch(&batch, false);
        self.redo.push(batch);
        true
    }

    pub fn redo(&mut self) -> bool {
        self.end();
        let Some(batch) = self.redo.pop() else { return false };
        self.apply_batch(&batch, true);
        self.undo.push(batch);
        true
    }

    /// Remplace partout la couleur d'index `from` (1..=255) par `to` (un seul lot annulable).
    pub fn replace_color(&mut self, from: u8, to: PaletteEntry) -> usize {
        let Some(new) = self.model.color_index(to) else { return 0 };
        if new == from {
            return 0;
        }
        let cells: Vec<IVec3> = self.model.voxels.iter().filter(|(_, v)| *v == from).map(|(p, _)| p).collect();
        self.begin();
        for p in &cells {
            self.set(*p, new, false);
        }
        self.end();
        cells.len()
    }

    /// Applique un outil à la case visée `hit` (case pleine) et `place` (case vide devant elle).
    /// Renvoie la couleur prise par la pipette. Un bloc ajouté contre une zone de mouvement la
    /// rejoint (il bougera avec elle).
    pub fn apply(&mut self, tool: Tool, hit: Option<IVec3>, place: Option<IVec3>, color: PaletteEntry, mirror: bool) -> Option<PaletteEntry> {
        match tool {
            Tool::Add => {
                if let (Some(p), Some(i)) = (place, self.model.color_index(color)) {
                    let mut pairs = vec![(p, hit)];
                    if mirror {
                        let q = mirror_cell(&self.model, p);
                        if q != p {
                            pairs.push((q, hit.map(|h| mirror_cell(&self.model, h))));
                        }
                    }
                    for (q, h) in pairs {
                        let zone = h.map_or(0, |h| self.model.zone_map.get(h));
                        if self.model.voxels.get(q) == 0 {
                            self.put(q, i, zone);
                        }
                    }
                }
                None
            }
            Tool::Remove => {
                if let Some(p) = hit {
                    self.set(p, 0, mirror);
                }
                None
            }
            Tool::Paint => {
                if let (Some(p), Some(i)) = (hit, self.model.color_index(color)) {
                    // Peindre ne pose pas de bloc là où il n'y en a pas (miroir compris)
                    let targets = if mirror { mirrored(&self.model, p) } else { vec![p] };
                    for q in targets {
                        if self.model.voxels.get(q) != 0 {
                            self.set(q, i, false);
                        }
                    }
                }
                None
            }
            Tool::Pick => hit.and_then(|p| self.model.color_at(p)),
        }
    }

    /// Pose un bloc de mouvement (son ancre sur `at`) : ses cases deviennent des blocs blancs et
    /// chaque partie une zone. Avec la symétrie, le bloc reflété est posé aussi de l'autre côté.
    /// Un seul lot annulable ; renvoie le nombre de zones créées.
    pub fn place_block(&mut self, def: &BlockDef, at: IVec3, place: Placement, mirror: bool) -> Result<usize, String> {
        let mut instances = vec![(at, place, "")];
        if mirror {
            let m = mirror_cell(&self.model, at);
            // Un bloc au milieu (sur le plan du miroir) n'a pas de reflet
            if m != at {
                let refl = Placement { turn: (4 - place.turn % 4) % 4, mirror: !place.mirror, scale: place.scale };
                instances[0].2 = if at.x < m.x { " g" } else { " d" };
                instances.push((m, refl, if at.x < m.x { " d" } else { " g" }));
            }
        }
        if self.model.zones.len() + def.parts.len() * instances.len() > 255 {
            return Err("Trop de zones de mouvement (255 au plus).".into());
        }
        let white = self.model.color_index(BLOCK_WHITE).ok_or("Palette pleine (255 couleurs) : impossible de poser des blocs blancs.")?;
        self.end();
        let before = self.model.zones.clone();
        self.stroke = Some(Batch::default());
        let mut made = 0;
        for (at, place, side) in instances {
            let base = self.model.zones.len();
            for part in &def.parts {
                let parent = part.parent.as_ref().and_then(|n| def.parts.iter().position(|p| &p.name == n)).map(|k| (base + k) as u16);
                let pivot = place.point(def, at, Vec3::from_array(part.pivot));
                self.model.zones.push(Zone {
                    name: format!("{}{side}", part.name.replace('_', " ")),
                    block: def.id.clone(),
                    part: part.name.clone(),
                    parent,
                    pivot: pivot.to_array(),
                    turn: place.turn,
                    mirror: place.mirror,
                    scale: place.scale,
                });
                let zone = self.model.zones.len() as u8;
                for c in motion::part_cells(part) {
                    for q in place.cells(def, at, c) {
                        self.put(q, white, zone);
                    }
                }
                made += 1;
            }
        }
        let mut b = self.stroke.take().unwrap_or_default();
        b.zones = Some((before, self.model.zones.clone()));
        self.push(b);
        self.dirty = true;
        self.mesh_dirty = true;
        Ok(made)
    }

    /// Retire la zone `z` (index dans `zones`) : ses blocs restent et rejoignent la zone parente
    /// (ou le corps fixe), ses zones filles aussi. Annulable.
    pub fn remove_zone(&mut self, z: usize) {
        if z >= self.model.zones.len() {
            return;
        }
        self.end();
        let before = self.model.zones.clone();
        let parent = before[z].parent;
        // Nouveaux numéros : la zone retirée -> sa parente, celles d'après reculent d'un
        let renumber = |n: u8| -> u8 {
            match (n as usize).cmp(&(z + 1)) {
                std::cmp::Ordering::Less => n,
                std::cmp::Ordering::Equal => parent.map_or(0, |p| p as u8 + 1),
                std::cmp::Ordering::Greater => n - 1,
            }
        };
        let mut zones = before.clone();
        zones.remove(z);
        for zone in &mut zones {
            zone.parent = match zone.parent {
                Some(p) if p as usize == z => parent,
                Some(p) if p as usize > z => Some(p - 1),
                other => other,
            };
        }
        // La parente garde son numéro sauf si elle vient après (impossible : parents d'abord)
        self.stroke = Some(Batch::default());
        let cells: Vec<(IVec3, u8)> = self.model.zone_map.iter().collect();
        for (p, n) in cells {
            let m = renumber(n);
            if m != n {
                let v = self.model.voxels.get(p);
                self.put(p, v, m);
            }
        }
        self.model.zones = zones.clone();
        let mut b = self.stroke.take().unwrap_or_default();
        b.zones = Some((before, zones));
        self.push(b);
        self.dirty = true;
        self.mesh_dirty = true;
    }
}

/// Reflet d'une case par rapport au milieu de la grille (axe x).
pub fn mirror_cell(m: &Model, p: IVec3) -> IVec3 {
    IVec3::new(m.size.x as i32 - 1 - p.x, p.y, p.z)
}

/// La case et son reflet par rapport au milieu de la grille (axe x : gauche / droite).
pub fn mirrored(m: &Model, p: IVec3) -> Vec<IVec3> {
    let q = mirror_cell(m, p);
    if q == p { vec![p] } else { vec![p, q] }
}

/// La symétrie miroir est active par défaut pour les personnages.
pub fn mirror_default(kind: ModelKind) -> bool {
    kind == ModelKind::Personnage
}

/// Case visée par un rayon (repère du modèle : une case = un cube unité, coin à l'origine) :
/// (case pleine touchée, case vide juste devant). Sans case pleine, le sol (y = 0) de la grille
/// donne la case où poser.
pub fn raycast(m: &Model, origin: Vec3, dir: Vec3, max_steps: usize) -> (Option<IVec3>, Option<IVec3>) {
    let dir = dir.normalize_or_zero();
    if dir == Vec3::ZERO {
        return (None, None);
    }
    let size = m.size.as_vec3();
    // Entrée dans la boîte de la grille
    let inv = Vec3::ONE / dir;
    let t0 = (Vec3::ZERO - origin) * inv;
    let t1 = (size - origin) * inv;
    let tmin = t0.min(t1).max_element().max(0.0);
    let tmax = t0.max(t1).min_element();
    let mut ground = None;
    if dir.y < 0.0 {
        let t = -origin.y / dir.y;
        let p = origin + dir * t;
        let c = IVec3::new(p.x.floor() as i32, 0, p.z.floor() as i32);
        if t > 0.0 && m.in_bounds(c) {
            ground = Some(c);
        }
    }
    if tmin > tmax {
        return (None, ground);
    }
    // Parcours case par case (DDA)
    let start = origin + dir * (tmin + 1e-4);
    let mut cell = start.floor().as_ivec3().clamp(IVec3::ZERO, m.size.as_ivec3() - IVec3::ONE);
    let step = dir.signum().as_ivec3();
    let next = (cell.as_vec3() + Vec3::new((step.x > 0) as u8 as f32, (step.y > 0) as u8 as f32, (step.z > 0) as u8 as f32) - origin) * inv;
    let mut tnext = Vec3::new(if dir.x == 0.0 { f32::INFINITY } else { next.x }, if dir.y == 0.0 { f32::INFINITY } else { next.y }, if dir.z == 0.0 { f32::INFINITY } else { next.z });
    let delta = inv.abs();
    let mut prev: Option<IVec3> = None;
    for _ in 0..max_steps {
        if !m.in_bounds(cell) {
            break;
        }
        if m.voxels.get(cell) != 0 {
            return (Some(cell), prev);
        }
        prev = Some(cell);
        if tnext.x < tnext.y && tnext.x < tnext.z {
            cell.x += step.x;
            tnext.x += delta.x;
        } else if tnext.y < tnext.z {
            cell.y += step.y;
            tnext.y += delta.y;
        } else {
            cell.z += step.z;
            tnext.z += delta.z;
        }
    }
    (None, ground)
}

/// Maillages des faces visibles, un par matière (mate, métal, verre, lumineuse), dans l'ordre de
/// `Material` : chacun a son rendu (E2). (L'éditeur maille par zone : `build_parts`.)
#[cfg(test)]
pub fn build_meshes(m: &Model) -> [Mesh; 4] {
    let mut g = build_groups(m, false);
    [0, 1, 2, 3].map(|k| g.remove(&(0, k)).unwrap_or_else(|| Buffers::default().mesh()))
}

/// Maillage des faces visibles du modèle (une case = un cube unité), couleurs aux sommets.
/// (Maillage glouton par chunk, hors du fil principal : E3.)
#[cfg(test)]
pub fn build_mesh(m: &Model) -> Mesh {
    let mut all = Buffers::default();
    let mut g = build_buffers(m, false);
    for k in 0..4 {
        if let Some(b) = g.remove(&(0, k)) {
            all.append(b);
        }
    }
    all.mesh()
}

/// Maillages par zone de mouvement (0 = corps fixe) et par matière : (zone, matière, maillage),
/// dans le repère du modèle. Une face n'est cachée que par un voisin de la même zone (la pièce
/// bouge : ce qu'elle cache peut se voir).
pub fn build_parts(m: &Model) -> Vec<(u8, usize, Mesh)> {
    let mut v: Vec<(u8, usize, Mesh)> = build_groups(m, true).into_iter().map(|((z, k), mesh)| (z, k, mesh)).collect();
    v.sort_by_key(|(z, k, _)| (*z, *k));
    v
}

fn material_index(mat: Material) -> usize {
    match mat {
        Material::Mate => 0,
        Material::Metal => 1,
        Material::Verre => 2,
        Material::Lumineuse => 3,
    }
}

/// Sommets d'un maillage en construction.
#[derive(Default)]
pub struct Buffers {
    pos: Vec<[f32; 3]>,
    nor: Vec<[f32; 3]>,
    col: Vec<[f32; 4]>,
    idx: Vec<u32>,
}

impl Buffers {
    /// Une face `k` (ordre de `FACES`) de la case `p`.
    pub fn face(&mut self, p: Vec3, k: usize, c: [f32; 4]) {
        let (n, quad) = &FACES[k];
        // Ombrage simple par face : le dessus plus clair, le dessous plus sombre
        let shade = [0.86, 0.86, 1.0, 0.6, 0.93, 0.93][k];
        let base = self.pos.len() as u32;
        for q in quad {
            self.pos.push([p.x + q[0], p.y + q[1], p.z + q[2]]);
            self.nor.push(n.as_vec3().to_array());
            self.col.push([c[0] * shade, c[1] * shade, c[2] * shade, c[3]]);
        }
        self.idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    #[cfg(test)]
    fn append(&mut self, b: Buffers) {
        let base = self.pos.len() as u32;
        self.pos.extend(b.pos);
        self.nor.extend(b.nor);
        self.col.extend(b.col);
        self.idx.extend(b.idx.into_iter().map(|i| i + base));
    }

    pub fn mesh(self) -> Mesh {
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.pos);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.nor);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.col);
        mesh.insert_indices(Indices::U32(self.idx));
        mesh
    }
}

pub const FACES: [(IVec3, [[f32; 3]; 4]); 6] = [
    (IVec3::X, [[1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [1.0, 1.0, 1.0], [1.0, 0.0, 1.0]]),
    (IVec3::NEG_X, [[0.0, 0.0, 1.0], [0.0, 1.0, 1.0], [0.0, 1.0, 0.0], [0.0, 0.0, 0.0]]),
    (IVec3::Y, [[0.0, 1.0, 0.0], [0.0, 1.0, 1.0], [1.0, 1.0, 1.0], [1.0, 1.0, 0.0]]),
    (IVec3::NEG_Y, [[0.0, 0.0, 1.0], [0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 0.0, 1.0]]),
    (IVec3::Z, [[1.0, 0.0, 1.0], [1.0, 1.0, 1.0], [0.0, 1.0, 1.0], [0.0, 0.0, 1.0]]),
    (IVec3::NEG_Z, [[0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 1.0, 0.0], [1.0, 0.0, 0.0]]),
];

fn build_groups(m: &Model, by_zone: bool) -> std::collections::HashMap<(u8, usize), Mesh> {
    build_buffers(m, by_zone).into_iter().map(|(k, b)| (k, b.mesh())).collect()
}

/// Faces visibles groupées par (zone, matière) ; sans `by_zone`, tout est dans la zone 0.
fn build_buffers(m: &Model, by_zone: bool) -> std::collections::HashMap<(u8, usize), Buffers> {
    let mut out: std::collections::HashMap<(u8, usize), Buffers> = Default::default();
    let zoned = by_zone && !m.zones.is_empty();
    for (p, v) in m.voxels.iter() {
        let Some(e) = m.palette.get(v as usize - 1) else { continue };
        let zone = if zoned { m.zone_map.get(p) } else { 0 };
        let c = srgb(e);
        let buf = out.entry((zone, material_index(e.material))).or_default();
        for (k, (n, _)) in FACES.iter().enumerate() {
            // Face cachée par un voisin opaque (le verre laisse voir ce qui est derrière)
            let nb = m.voxels.get(p + *n);
            if nb != 0
                && (!zoned || m.zone_map.get(p + *n) == zone)
                && m.palette.get(nb as usize - 1).is_some_and(|x| x.material != Material::Verre || e.material == Material::Verre)
            {
                continue;
            }
            buf.face(p.as_vec3(), k, [c[0], c[1], c[2], 1.0]);
        }
    }
    out
}

/// Couleur linéaire (sommets) d'une entrée de palette sRGB.
pub fn srgb(e: &PaletteEntry) -> [f32; 3] {
    e.rgb.map(|c| (c as f32 / 255.0).powf(2.2))
}

/// Couleurs de départ (E2 apporte la grande palette OKLCH) : celles des blocs de Pixel World,
/// plus du blanc, du noir et quelques métaux, verres et lumières.
pub fn starter_palette() -> Vec<PaletteEntry> {
    let mut out: Vec<PaletteEntry> = [
        "snow", "paper", "marble", "bone", "stone", "ash", "obsidian", "darkvoid", "brick", "rust", "flesh", "redflower", "redleaves", "coral",
        "pinkleaves", "mushroom", "mycelium", "orangeleaves", "flower", "sand", "deadgrass", "wood", "dirt", "root", "petrified", "moss",
        "tallgrass", "grass", "leaves", "silicon", "water", "cloud", "metal", "mercury", "mirror", "glass", "crystal", "ice", "glow",
    ]
    .iter()
    .filter_map(|b| super::import::block_color(b))
    .collect();
    out.extend([
        PaletteEntry { rgb: [255, 255, 255], material: Material::Mate },
        PaletteEntry { rgb: [10, 10, 12], material: Material::Mate },
        PaletteEntry { rgb: [212, 175, 55], material: Material::Metal },
        PaletteEntry { rgb: [255, 80, 40], material: Material::Lumineuse },
        PaletteEntry { rgb: [80, 160, 255], material: Material::Lumineuse },
    ]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::math::UVec3;

    fn red() -> PaletteEntry {
        PaletteEntry { rgb: [200, 0, 0], material: Material::Mate }
    }

    #[test]
    fn a_stroke_is_one_undo_and_mirror_doubles_it() {
        let mut d = Doc::new(Model::new("p", ModelKind::Personnage, None), None);
        d.begin();
        for z in 0..5 {
            d.apply(Tool::Add, None, Some(IVec3::new(2, 0, z)), red(), true);
        }
        d.end();
        // Miroir : x = 2 et x = 13 (grille de 16)
        assert_eq!(d.model.voxels.count(), 10);
        assert_ne!(d.model.voxels.get(IVec3::new(13, 0, 4)), 0);
        assert!(d.undo());
        assert_eq!(d.model.voxels.count(), 0);
        assert!(d.redo());
        assert_eq!(d.model.voxels.count(), 10);
        // Peindre ne crée pas de bloc ; retirer, si
        let blue = PaletteEntry { rgb: [0, 0, 200], material: Material::Metal };
        d.apply(Tool::Paint, Some(IVec3::new(2, 0, 0)), None, blue, false);
        d.apply(Tool::Paint, Some(IVec3::new(5, 5, 5)), None, blue, false);
        assert_eq!(d.model.voxels.count(), 10);
        assert_eq!(d.model.color_at(IVec3::new(2, 0, 0)), Some(blue));
        assert_eq!(d.apply(Tool::Pick, Some(IVec3::new(2, 0, 0)), None, red(), false), Some(blue));
        d.apply(Tool::Remove, Some(IVec3::new(2, 0, 0)), None, red(), true);
        assert_eq!(d.model.voxels.count(), 8);
        assert!(d.dirty);
    }

    #[test]
    fn nothing_is_placed_outside_the_grid() {
        let mut d = Doc::new(Model::new("p", ModelKind::Autre, None), None);
        assert!(!d.set(IVec3::new(-1, 0, 0), 1, false));
        assert!(!d.set(IVec3::new(0, 32, 0), 1, false));
        assert!(!d.undo());
    }

    #[test]
    fn the_ray_finds_the_face_or_the_ground() {
        let mut m = Model::new("p", ModelKind::Autre, None);
        m.size = UVec3::splat(8);
        let i = m.color_index(red()).unwrap();
        m.voxels.set(IVec3::new(3, 2, 3), i);
        // Vu d'en haut : la case et la case vide au-dessus
        let (hit, place) = raycast(&m, Vec3::new(3.5, 20.0, 3.5), Vec3::NEG_Y, 100);
        assert_eq!((hit, place), (Some(IVec3::new(3, 2, 3)), Some(IVec3::new(3, 3, 3))));
        // De côté
        let (hit, place) = raycast(&m, Vec3::new(-5.0, 2.5, 3.5), Vec3::X, 100);
        assert_eq!((hit, place), (Some(IVec3::new(3, 2, 3)), Some(IVec3::new(2, 2, 3))));
        // Rien sur le chemin : le sol
        let (hit, place) = raycast(&m, Vec3::new(6.5, 10.0, 6.5), Vec3::new(0.0, -1.0, 0.01), 100);
        assert_eq!(hit, None);
        assert_eq!(place, Some(IVec3::new(6, 0, 6)));
        // Hors de la grille : rien
        assert_eq!(raycast(&m, Vec3::new(50.0, 10.0, 50.0), Vec3::new(0.0, -1.0, 0.0), 100), (None, None));
    }

    #[test]
    fn hidden_faces_are_not_meshed() {
        let mut m = Model::new("p", ModelKind::Autre, None);
        let i = m.color_index(red()).unwrap();
        m.voxels.set(IVec3::ZERO, i);
        assert_eq!(build_mesh(&m).count_vertices(), 24);
        m.voxels.set(IVec3::X, i);
        assert_eq!(build_mesh(&m).count_vertices(), 40, "la face commune disparait");
        // Le verre laisse voir la face derrière lui
        let g = m.color_index(PaletteEntry { rgb: [100, 200, 255], material: Material::Verre }).unwrap();
        m.voxels.set(IVec3::X, g);
        assert_eq!(build_mesh(&m).count_vertices(), 44);
    }

    #[test]
    fn replace_a_color_everywhere_in_one_undo() {
        let mut d = Doc::new(Model::new("p", ModelKind::Autre, None), None);
        let blue = PaletteEntry { rgb: [0, 0, 200], material: Material::Metal };
        d.begin();
        for x in 0..6 {
            d.apply(Tool::Add, None, Some(IVec3::new(x, 0, 0)), if x % 2 == 0 { red() } else { blue }, false);
        }
        d.end();
        let from = d.model.color_index(red()).unwrap();
        let gold = PaletteEntry { rgb: [212, 175, 55], material: Material::Metal };
        assert_eq!(d.replace_color(from, gold), 3);
        assert_eq!(d.model.color_at(IVec3::new(2, 0, 0)), Some(gold));
        assert_eq!(d.model.color_at(IVec3::new(1, 0, 0)), Some(blue));
        assert!(d.undo());
        assert_eq!(d.model.color_at(IVec3::new(2, 0, 0)), Some(red()));
        // Les maillages par matière se partagent les faces
        let total: usize = build_meshes(&d.model).iter().map(|m| m.count_vertices()).sum();
        assert_eq!(total, build_mesh(&d.model).count_vertices());
    }

    #[test]
    fn starter_palette_has_every_material() {
        let p = starter_palette();
        assert!(p.len() > 40 && p.len() <= 255);
        for mat in [Material::Mate, Material::Metal, Material::Verre, Material::Lumineuse] {
            assert!(p.iter().any(|e| e.material == mat));
        }
    }
    #[test]
    fn a_placed_block_is_one_undo_and_added_cells_join_its_zone() {
        let (blocks, _) = motion::load_blocks(None);
        let door = blocks.iter().find(|b| b.id == "porte_pivotante").unwrap();
        let mut d = Doc::new(Model::new("v", ModelKind::Autre, None), None);
        let n = d.place_block(door, IVec3::new(4, 0, 4), Placement { turn: 1, mirror: false, scale: 2 }, true).unwrap();
        assert_eq!(n, 2, "la porte et son reflet");
        // 1 x 8 x 5 cases, taille 2 : 8 fois plus
        assert_eq!(d.model.voxels.count(), 2 * 40 * 8);
        assert_eq!(d.model.color_at(IVec3::new(4, 0, 4)), Some(BLOCK_WHITE));
        assert_eq!(d.model.zone_map.get(IVec3::new(4, 0, 4)), 1);
        // Un bloc ajouté contre la porte la rejoint ; peindre garde la zone ; retirer la quitte
        let side = [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z].map(|n| IVec3::new(4, 0, 4) + n).into_iter().find(|p| d.model.in_bounds(*p) && d.model.voxels.get(*p) == 0).unwrap();
        d.apply(Tool::Add, Some(IVec3::new(4, 0, 4)), Some(side), red(), false);
        assert_eq!(d.model.zone_map.get(side), 1);
        d.apply(Tool::Paint, Some(IVec3::new(4, 0, 4)), None, red(), false);
        assert_eq!(d.model.zone_map.get(IVec3::new(4, 0, 4)), 1);
        d.apply(Tool::Remove, Some(IVec3::new(4, 0, 4)), None, red(), false);
        assert_eq!(d.model.zone_map.get(IVec3::new(4, 0, 4)), 0);
        // Retirer la zone 1 : ses blocs restent, la zone 2 devient la 1
        let kept = d.model.voxels.count();
        d.remove_zone(0);
        assert_eq!(d.model.zones.len(), 1);
        assert_eq!(d.model.voxels.count(), kept);
        assert_eq!(d.model.zone_map.get(side), 0);
        assert!(d.model.zone_map.iter().all(|(_, z)| z == 1));
        // Tout s'annule
        for _ in 0..5 {
            assert!(d.undo());
        }
        assert_eq!(d.model.voxels.count(), 0);
        assert!(d.model.zones.is_empty());
        assert_eq!(d.model.zone_map.count(), 0);
        assert!(d.redo());
        assert_eq!(d.model.zones.len(), 2);
    }

    #[test]
    fn moving_parts_are_meshed_apart() {
        let (blocks, _) = motion::load_blocks(None);
        let door = blocks.iter().find(|b| b.id == "porte_pivotante").unwrap();
        let mut d = Doc::new(Model::new("v", ModelKind::Autre, None), None);
        let i = d.model.color_index(red()).unwrap();
        d.set(IVec3::new(3, 0, 4), i, false);
        d.place_block(door, IVec3::new(4, 0, 4), Placement::default(), false).unwrap();
        let parts = build_parts(&d.model);
        assert_eq!(parts.iter().map(|(z, _, _)| *z).collect::<Vec<_>>(), vec![0, 1]);
        // La face entre le corps et la porte est gardée des deux côtés
        let total: usize = parts.iter().map(|(_, _, m)| m.count_vertices()).sum();
        assert_eq!(total, build_mesh(&d.model).count_vertices() + 8);
    }
}
