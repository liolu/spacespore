//! Édition d'un modèle (E1, E3), sans Bevy : outils, symétrie miroir, annuler / rétablir par lots,
//! rayon de la souris jusqu'à la case visée ; outils de volume, remplissage, sélection, calques.
//!
//! Porté de `VoxelEditorManager.cs` (Pixel World) : outils Ajouter / Retirer / Peindre, pipette
//! (Maj+clic ou outil 4), un trait de la souris = une action annulable.
//!
//! E3 (grilles jusqu'à 1024³) : un lot d'annulation garde les **chunks** touchés avant / après (ils
//! sont partagés : rien n'est copié tant qu'on n'y écrit pas) ; les outils de volume écrivent chunk
//! par chunk (`Doc::edit_region`) ; les chunks changés sont notés pour être remaillés seuls.

use bevy::math::{IVec3, Vec3};
use bevy::render::mesh::Mesh;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;

use super::format::{Chunk, Hangar, Layer, Material, Model, ModelKind, PaletteEntry, ShipCategory, Zone, CHUNK};
use super::mesh::{Cut, Visibility};
use super::motion::{self, BlockDef, Placement};

/// Outils (touches 1 à 0 : sur AZERTY & é " ' ( - è _ ç à).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tool {
    #[default]
    Add,
    Remove,
    Paint,
    Pick,
    /// Outils de volume (E3) : la souris tire de la case de départ à celle d'arrivée.
    Box,
    Sphere,
    Cylinder,
    Line,
    /// Pot de peinture : la région de même couleur qui se touche.
    Fill,
    /// Sélection (copier, couper, coller, tourner, retourner, déplacer).
    Select,
}

impl Tool {
    pub const ALL: [Tool; 4] = [Tool::Add, Tool::Remove, Tool::Paint, Tool::Pick];
    pub const VOLUME: [Tool; 6] = [Tool::Box, Tool::Sphere, Tool::Cylinder, Tool::Line, Tool::Fill, Tool::Select];

    pub fn name(self) -> &'static str {
        match self {
            Tool::Add => "Ajouter (1)",
            Tool::Remove => "Retirer (2)",
            Tool::Paint => "Peindre (3)",
            Tool::Pick => "Pipette (4)",
            Tool::Box => "Boite (5)",
            Tool::Sphere => "Sphere (6)",
            Tool::Cylinder => "Cylindre (7)",
            Tool::Line => "Ligne (8)",
            Tool::Fill => "Remplir (9)",
            Tool::Select => "Selection (0)",
        }
    }

    /// Outil qui se tire à la souris (départ -> arrivée).
    pub fn is_shape(self) -> bool {
        matches!(self, Tool::Box | Tool::Sphere | Tool::Cylinder | Tool::Line | Tool::Select)
    }
}

/// Ce que fait un outil de volume : ajouter (dans les cases vides), retirer, peindre.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Brush {
    #[default]
    Add,
    Remove,
    Paint,
}

impl Brush {
    pub const ALL: [Brush; 3] = [Brush::Add, Brush::Remove, Brush::Paint];

    pub fn name(self) -> &'static str {
        match self {
            Brush::Add => "ajouter",
            Brush::Remove => "retirer",
            Brush::Paint => "peindre",
        }
    }

    /// Nouvelle valeur d'une case (`v` = couleur posée) ; `None` : inchangée. Les cases cachées
    /// (calque masqué, au-delà de la coupe) ne sont jamais touchées.
    fn op(self, old: u8, visible: bool, v: u8) -> Option<u8> {
        if !visible {
            return None;
        }
        match self {
            Brush::Add => (old == 0).then_some(v),
            Brush::Remove => (old != 0).then_some(0),
            Brush::Paint => (old != 0 && old != v).then_some(v),
        }
    }
}

/// Étiquettes proposées (adaptées de `TagCategories`).
pub const TAG_GROUPS: [(&str, &[&str]); 5] = [
    ("Type", &["personnage", "vaisseau", "arme", "outil", "meuble", "decor", "vehicule", "creature", "plante"]),
    ("Style", &["militaire", "civil", "pirate", "alien", "ancien", "organique", "mecanique", "magique"]),
    ("Rarete", &["commun", "peu commun", "rare", "unique"]),
    ("Taille", &["petit", "moyen", "grand", "immense"]),
    ("Usage", &["sol", "mur", "plafond", "flottant", "sous l'eau"]),
];

/// Un chunk avant / après : voxels, zones, calques.
#[derive(Clone, Debug, Default)]
struct Snap {
    old: [Option<Chunk>; 3],
    new: [Option<Chunk>; 3],
}

/// Un lot annulable : les chunks touchés, et les listes de zones et de calques si elles changent.
#[derive(Clone, Debug, Default)]
struct Batch {
    chunks: HashMap<IVec3, Snap>,
    zones: Option<(Vec<Zone>, Vec<Zone>)>,
    layers: Option<(Vec<Layer>, Vec<Layer>)>,
    hangars: Option<(Vec<Hangar>, Vec<Hangar>)>,
}

impl Batch {
    fn is_empty(&self) -> bool {
        self.chunks.is_empty() && self.zones.is_none() && self.layers.is_none() && self.hangars.is_none()
    }

    /// Mémoire gardée (octets, environ : les chunks pleins).
    fn bytes(&self) -> usize {
        let full = |c: &Option<Chunk>| if matches!(c, Some(Chunk::Full(_))) { (CHUNK * CHUNK * CHUNK) as usize } else { 16 };
        self.chunks.values().map(|s| s.old.iter().chain(s.new.iter()).map(full).sum::<usize>()).sum()
    }
}

/// Un modèle ouvert (onglet) : le modèle, son fichier, l'historique, et ce qu'il faut remailler.
#[derive(Clone, Debug)]
pub struct Doc {
    pub model: Model,
    pub path: Option<PathBuf>,
    pub dirty: bool,
    undo: Vec<Batch>,
    redo: Vec<Batch>,
    /// Trait en cours (souris enfoncée) : un seul lot annulable.
    stroke: Option<Batch>,
    /// Tout le maillage est à refaire.
    pub mesh_dirty: bool,
    /// Chunks à remailler.
    pub dirty_chunks: HashSet<IVec3>,
    /// Augmente à chaque changement (poids du fichier à recalculer).
    pub revision: u64,
    /// Calque où vont les nouveaux blocs.
    pub layer: u8,
    /// Vue en coupe.
    pub cut: Option<Cut>,
}

/// Lots gardés au plus (les plus anciens s'oublient), et leur mémoire au plus.
const MAX_UNDO: usize = 200;
const MAX_UNDO_BYTES: usize = 768 * 1024 * 1024;

/// Couleur des gabarits posés (« un bloc blanc restant est un bloc comme un autre »).
pub const BLOCK_WHITE: PaletteEntry = PaletteEntry { rgb: [255, 255, 255], material: Material::Mate };

/// Chunk d'une case.
pub fn chunk_of(p: IVec3) -> IVec3 {
    p.div_euclid(IVec3::splat(CHUNK))
}

impl Doc {
    pub fn new(model: Model, path: Option<PathBuf>) -> Self {
        Self { model, path, dirty: false, undo: Vec::new(), redo: Vec::new(), stroke: None, mesh_dirty: true, dirty_chunks: HashSet::new(), revision: 0, layer: 0, cut: None }
    }

    /// Ce qui est visible (calques, coupe).
    pub fn view(&self) -> Visibility {
        Visibility::of(&self.model, self.cut)
    }

    /// Commence un trait (clic) : tout jusqu'au relâchement sera annulé d'un coup.
    pub fn begin(&mut self) {
        self.end();
        self.stroke = Some(Batch::default());
    }

    /// Fin du trait : les chunks touchés gardent leur état d'après.
    pub fn end(&mut self) {
        if let Some(mut s) = self.stroke.take() {
            for (c, snap) in s.chunks.iter_mut() {
                snap.new = [self.model.voxels.chunk(*c).cloned(), self.model.zone_map.chunk(*c).cloned(), self.model.layer_map.chunk(*c).cloned()];
            }
            s.chunks.retain(|_, snap| snap.old != snap.new);
            self.push(s);
        }
    }

    /// Un lot à soi si aucun trait n'est en cours (à refermer par `end` si `true`).
    fn open(&mut self) -> bool {
        let own = self.stroke.is_none();
        if own {
            self.stroke = Some(Batch::default());
        }
        own
    }

    fn close(&mut self, own: bool) {
        if own {
            self.end();
        }
    }

    fn push(&mut self, b: Batch) {
        if b.is_empty() {
            return;
        }
        self.undo.push(b);
        // Les plus anciens s'oublient (nombre et mémoire)
        let mut total: usize = self.undo.iter().map(Batch::bytes).sum();
        while self.undo.len() > 1 && (self.undo.len() > MAX_UNDO || total > MAX_UNDO_BYTES) {
            total -= self.undo[0].bytes();
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// Le chunk `c` va changer : on garde son état d'avant dans le lot en cours.
    fn touch(&mut self, c: IVec3) {
        let Some(s) = self.stroke.as_mut() else { return };
        if !s.chunks.contains_key(&c) {
            let old = [self.model.voxels.chunk(c).cloned(), self.model.zone_map.chunk(c).cloned(), self.model.layer_map.chunk(c).cloned()];
            s.chunks.insert(c, Snap { old, new: Default::default() });
        }
    }

    /// Le chunk `c` (et ses voisins : leurs faces communes) est à remailler.
    fn changed(&mut self, c: IVec3) {
        self.dirty_chunks.insert(c);
        for d in [IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z] {
            self.dirty_chunks.insert(c + d);
        }
        self.dirty = true;
        self.revision += 1;
    }

    /// La case `p` a changé : son chunk, et un voisin seulement si elle est sur sa face.
    fn changed_cell(&mut self, p: IVec3) {
        let c = chunk_of(p);
        let l = p - c * CHUNK;
        self.dirty_chunks.insert(c);
        for a in 0..3 {
            let mut e = IVec3::ZERO;
            e[a] = 1;
            if l[a] == 0 {
                self.dirty_chunks.insert(c - e);
            }
            if l[a] == CHUNK - 1 {
                self.dirty_chunks.insert(c + e);
            }
        }
        self.dirty = true;
        self.revision += 1;
    }

    /// Change une case (valeur, zone) ; un nouveau bloc va dans le calque courant.
    fn put(&mut self, q: IVec3, v: u8, zone: u8) -> bool {
        if !self.model.in_bounds(q) {
            return false;
        }
        let zone = if v == 0 { 0 } else { zone };
        let (old, old_zone, old_layer) = (self.model.voxels.get(q), self.model.zone_map.get(q), self.model.layer_map.get(q));
        let layer = match (old, v) {
            (_, 0) => 0,
            (0, _) => self.layer,
            _ => old_layer,
        };
        if old == v && old_zone == zone && old_layer == layer {
            return false;
        }
        let own = self.open();
        let c = chunk_of(q);
        self.touch(c);
        self.model.voxels.set(q, v);
        self.model.zone_map.set(q, zone);
        self.model.layer_map.set(q, layer);
        self.changed_cell(q);
        self.close(own);
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
        for (c, s) in &b.chunks {
            let [v, z, l] = if forward { s.new.clone() } else { s.old.clone() };
            self.model.voxels.put_chunk(*c, v);
            self.model.zone_map.put_chunk(*c, z);
            self.model.layer_map.put_chunk(*c, l);
            self.changed(*c);
        }
        if let Some((before, after)) = &b.zones {
            self.model.zones = if forward { after.clone() } else { before.clone() };
            self.mesh_dirty = true;
        }
        if let Some((before, after)) = &b.layers {
            self.model.layers = if forward { after.clone() } else { before.clone() };
            self.mesh_dirty = true;
        }
        if let Some((before, after)) = &b.hangars {
            self.model.hangars = if forward { after.clone() } else { before.clone() };
        }
        self.dirty = true;
        self.revision += 1;
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

    /// Écrit dans la boîte `min..=max`, chunk par chunk : `f(case, ancienne valeur, visible)` donne
    /// la nouvelle valeur (`None` : inchangée). Un nouveau bloc va dans le calque courant et le corps
    /// fixe ; un bloc changé garde sa zone et son calque. `creates` : peut poser des blocs dans un
    /// chunk vide (sinon les chunks vides sont sautés). Renvoie le nombre de cases changées.
    pub fn edit_region(&mut self, min: IVec3, max: IVec3, creates: bool, mut f: impl FnMut(IVec3, u8, bool) -> Option<u8>) -> usize {
        let lo = min.max(IVec3::ZERO);
        let hi = max.min(self.model.size.as_ivec3() - IVec3::ONE);
        if lo.cmpgt(hi).any() {
            return 0;
        }
        let own = self.open();
        let view = self.view();
        let layer_now = self.layer;
        let mut total = 0;
        let (c0, c1) = (chunk_of(lo), chunk_of(hi));
        for cz in c0.z..=c1.z {
            for cy in c0.y..=c1.y {
                for cx in c0.x..=c1.x {
                    let c = IVec3::new(cx, cy, cz);
                    if !creates && self.model.voxels.chunk(c).is_none() {
                        continue;
                    }
                    let base = c * CHUNK;
                    let (a, b) = ((lo - base).max(IVec3::ZERO), (hi - base).min(IVec3::splat(CHUNK - 1)));
                    // D'abord sans rien écrire : le chunk change-t-il ?
                    let mut writes: Vec<(usize, u8)> = Vec::new();
                    {
                        let vox = self.model.voxels.chunk(c);
                        let lay = self.model.layer_map.chunk(c);
                        for z in a.z..=b.z {
                            for y in a.y..=b.y {
                                for x in a.x..=b.x {
                                    let l = IVec3::new(x, y, z);
                                    let i = super::format::local_index(l);
                                    let old = vox.map_or(0, |ch| ch.get(i));
                                    let layer = if old == 0 { layer_now } else { lay.map_or(0, |ch| ch.get(i)) };
                                    let p = base + l;
                                    if let Some(nv) = f(p, old, view.shows(p, layer)) {
                                        if nv != old {
                                            writes.push((i, nv));
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if writes.is_empty() {
                        continue;
                    }
                    self.touch(c);
                    let old_vox: Vec<u8> = writes.iter().map(|(i, _)| self.model.voxels.chunk(c).map_or(0, |ch| ch.get(*i))).collect();
                    {
                        let vox = self.model.voxels.data_mut(c);
                        for (i, nv) in &writes {
                            vox[*i] = *nv;
                        }
                    }
                    // Zones et calques : un bloc nouveau ou retiré n'a ni zone ni calque à garder
                    let fresh: Vec<(usize, u8)> = writes.iter().zip(&old_vox).filter(|((_, nv), ov)| *nv == 0 || **ov == 0).map(|((i, nv), _)| (*i, *nv)).collect();
                    if !fresh.is_empty() {
                        let zones = self.model.zone_map.data_mut(c);
                        for (i, _) in &fresh {
                            zones[*i] = 0;
                        }
                        let layers = self.model.layer_map.data_mut(c);
                        for (i, nv) in &fresh {
                            layers[*i] = if *nv == 0 { 0 } else { layer_now };
                        }
                    }
                    self.model.voxels.fold(c);
                    self.model.zone_map.fold(c);
                    self.model.layer_map.fold(c);
                    total += writes.len();
                    self.changed(c);
                }
            }
        }
        self.close(own);
        total
    }

    /// Remplace partout la couleur d'index `from` (1..=255) par `to` (un seul lot annulable).
    pub fn replace_color(&mut self, from: u8, to: PaletteEntry) -> usize {
        let Some(new) = self.model.color_index(to) else { return 0 };
        if new == from {
            return 0;
        }
        self.end();
        let max = self.model.size.as_ivec3() - IVec3::ONE;
        self.edit_region(IVec3::ZERO, max, false, |_, old, _| (old == from).then_some(new))
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
            _ => None,
        }
    }

    /// Un volume (boîte, sphère, cylindre, ligne), avec son reflet si demandé : un lot annulable.
    pub fn apply_shape(&mut self, shape: Shape, brush: Brush, color: PaletteEntry, mirror: bool) -> Result<usize, String> {
        let v = match brush {
            Brush::Remove => 0,
            _ => self.model.color_index(color).ok_or("Palette pleine (255 couleurs).")?,
        };
        self.end();
        self.stroke = Some(Batch::default());
        let mut n = 0;
        let size_x = self.model.size.x as i32;
        let mirrors: &[bool] = if mirror { &[false, true] } else { &[false] };
        for &refl in mirrors {
            let flip = |p: IVec3| if refl { IVec3::new(size_x - 1 - p.x, p.y, p.z) } else { p };
            if let Shape::Line { a, b } = shape {
                let view = self.view();
                for p in line_cells(flip(a), flip(b)) {
                    if !self.model.in_bounds(p) {
                        continue;
                    }
                    let old = self.model.voxels.get(p);
                    let layer = if old == 0 { self.layer } else { self.model.layer_map.get(p) };
                    if let Some(nv) = brush.op(old, view.shows(p, layer), v) {
                        let zone = self.model.zone_map.get(p);
                        n += self.put(p, nv, zone) as usize;
                    }
                }
                continue;
            }
            let (lo, hi) = shape.bounds();
            let (lo, hi) = if refl { (IVec3::new(size_x - 1 - hi.x, lo.y, lo.z), IVec3::new(size_x - 1 - lo.x, hi.y, hi.z)) } else { (lo, hi) };
            n += self.edit_region(lo, hi, brush == Brush::Add, |p, old, vis| if shape.contains(flip(p)) { brush.op(old, vis, v) } else { None });
        }
        self.end();
        Ok(n)
    }

    /// Pot de peinture depuis la case `start` : peindre ou retirer la région de même couleur qui se
    /// touche (6 voisins) ; ajouter : remplir la région vide dans le plan de la face visée
    /// (`normal`). Au plus `limit` cases.
    pub fn flood(&mut self, start: IVec3, normal: IVec3, brush: Brush, color: PaletteEntry, limit: usize) -> Result<usize, String> {
        let v = match brush {
            Brush::Remove => 0,
            _ => self.model.color_index(color).ok_or("Palette pleine (255 couleurs).")?,
        };
        if !self.model.in_bounds(start) {
            return Ok(0);
        }
        let view = self.view();
        let target = self.model.voxels.get(start);
        let visible = |m: &Model, p: IVec3| {
            let old = m.voxels.get(p);
            view.shows(p, if old == 0 { 0 } else { m.layer_map.get(p) })
        };
        let dirs: Vec<IVec3> = if brush == Brush::Add {
            // Dans le plan de la face : les 4 directions qui ne suivent pas la normale
            [IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z].into_iter().filter(|d| d.dot(normal) == 0).collect()
        } else {
            vec![IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z]
        };
        let mut seen = HashSet::new();
        let mut queue = VecDeque::new();
        seen.insert(start);
        queue.push_back(start);
        let mut cells = Vec::new();
        while let Some(p) = queue.pop_front() {
            if self.model.voxels.get(p) != target || !visible(&self.model, p) {
                continue;
            }
            cells.push(p);
            if cells.len() > limit {
                return Err(format!("Region trop grande (plus de {limit} cases) : rien n'est change."));
            }
            for d in &dirs {
                let q = p + *d;
                if self.model.in_bounds(q) && seen.insert(q) {
                    queue.push_back(q);
                }
            }
        }
        self.end();
        self.stroke = Some(Batch::default());
        let mut n = 0;
        for p in cells {
            let old = self.model.voxels.get(p);
            if let Some(nv) = brush.op(old, true, v) {
                let zone = self.model.zone_map.get(p);
                n += self.put(p, nv, zone) as usize;
            }
        }
        self.end();
        Ok(n)
    }

    /// Copie les blocs visibles de la boîte `min..=max`.
    pub fn copy(&self, min: IVec3, max: IVec3) -> Clip {
        let size = (max - min + IVec3::ONE).max(IVec3::ONE);
        let mut m = Model::new("presse-papiers", ModelKind::Autre, None);
        m.size = size.as_uvec3();
        m.palette = self.model.palette.clone();
        let view = self.view();
        let (c0, c1) = (chunk_of(min.max(IVec3::ZERO)), chunk_of(max.min(self.model.size.as_ivec3() - IVec3::ONE)));
        for cz in c0.z..=c1.z {
            for cy in c0.y..=c1.y {
                for cx in c0.x..=c1.x {
                    let c = IVec3::new(cx, cy, cz);
                    let Some(ch) = self.model.voxels.chunk(c) else { continue };
                    let lay = self.model.layer_map.chunk(c);
                    let base = c * CHUNK;
                    let (a, b) = ((min - base).max(IVec3::ZERO), (max - base).min(IVec3::splat(CHUNK - 1)));
                    for z in a.z..=b.z {
                        for y in a.y..=b.y {
                            for x in a.x..=b.x {
                                let l = IVec3::new(x, y, z);
                                let i = super::format::local_index(l);
                                let v = ch.get(i);
                                if v != 0 && view.shows(base + l, lay.map_or(0, |c| c.get(i))) {
                                    m.voxels.set(base + l - min, v);
                                }
                            }
                        }
                    }
                }
            }
        }
        Clip { model: m }
    }

    /// Efface les blocs visibles de la boîte.
    pub fn clear(&mut self, min: IVec3, max: IVec3) -> usize {
        self.edit_region(min, max, false, |_, old, vis| (old != 0 && vis).then_some(0))
    }

    /// Colle `clip` coin bas en `at` (ses cases vides ne changent rien).
    pub fn paste(&mut self, clip: &Clip, at: IVec3) -> Result<usize, String> {
        let mut table = [0u8; 256];
        for (i, e) in clip.model.palette.iter().enumerate() {
            table[i + 1] = self.model.color_index(*e).ok_or("Palette pleine (255 couleurs) : collage impossible.")?;
        }
        let size = clip.model.size.as_ivec3();
        let src = &clip.model.voxels;
        Ok(self.edit_region(at, at + size - IVec3::ONE, true, |p, _, vis| {
            let v = src.get(p - at);
            (v != 0 && vis).then(|| table[v as usize])
        }))
    }

    /// Tourne (quart de tour) ou retourne les blocs de la sélection sur place : un lot annulable.
    /// Renvoie la nouvelle boîte de la sélection.
    pub fn transform_selection(&mut self, min: IVec3, max: IVec3, t: ClipTransform) -> Result<(IVec3, IVec3), String> {
        let clip = self.copy(min, max).transformed(t);
        let size = clip.model.size.as_ivec3();
        // Même centre (arrondi), dans la grille
        let center2 = min + max;
        let lo = ((center2 - size + IVec3::ONE).as_vec3() / 2.0).floor().as_ivec3();
        let lo = lo.clamp(IVec3::ZERO, (self.model.size.as_ivec3() - size).max(IVec3::ZERO));
        self.end();
        self.stroke = Some(Batch::default());
        self.clear(min, max);
        let r = self.paste(&clip, lo);
        self.end();
        r.map(|_| (lo, lo + size - IVec3::ONE))
    }

    /// Ajoute un calque (il devient le calque courant).
    pub fn add_layer(&mut self) -> u8 {
        let before = self.model.layers.clone();
        let mut layers = self.model.layer_list();
        if layers.len() >= 32 {
            return self.layer;
        }
        layers.push(Layer { name: format!("Calque {}", layers.len() + 1), visible: true });
        self.model.layers = layers.clone();
        self.end();
        self.push(Batch { layers: Some((before, layers)), ..Default::default() });
        self.layer = (self.model.layers.len() - 1) as u8;
        self.dirty = true;
        self.layer
    }

    /// Montre / cache un calque (tout est remaillé).
    pub fn toggle_layer(&mut self, k: u8) {
        let mut layers = self.model.layer_list();
        if let Some(l) = layers.get_mut(k as usize) {
            l.visible = !l.visible;
        }
        self.model.layers = layers;
        self.mesh_dirty = true;
        self.dirty = true;
    }

    /// Change la coupe (seuls les chunks traversés par l'ancien ou le nouveau plan sont remaillés).
    pub fn set_cut(&mut self, cut: Option<Cut>) {
        if cut == self.cut {
            return;
        }
        let old = self.cut;
        self.cut = cut;
        match (old, cut) {
            (Some(a), Some(b)) if a.axis == b.axis => {
                let keys: Vec<IVec3> = self.model.voxels.keys().collect();
                let (p0, p1) = (a.pos.min(b.pos), a.pos.max(b.pos));
                for c in keys {
                    let (lo, hi) = (c[a.axis] * CHUNK, c[a.axis] * CHUNK + CHUNK - 1);
                    if hi >= p0 && lo <= p1 + 1 {
                        self.dirty_chunks.insert(c);
                    }
                }
            }
            _ => self.mesh_dirty = true,
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
        if let Some(h) = &def.hangar {
            hangar_allowed(self.model.category, h.category, h.cargo)?;
        }
        let mut colors = Vec::new();
        for part in &def.parts {
            let e = match part.color {
                Some(rgb) => PaletteEntry { rgb, material: part.material.unwrap_or(Material::Mate) },
                None => BLOCK_WHITE,
            };
            colors.push(self.model.color_index(e).ok_or("Palette pleine (255 couleurs) : impossible de poser le bloc.")?);
        }
        self.end();
        let before = self.model.zones.clone();
        let hangars_before = self.model.hangars.clone();
        self.stroke = Some(Batch::default());
        let mut made = 0;
        for (at, place, side) in instances {
            let base = self.model.zones.len();
            for (k, part) in def.parts.iter().enumerate() {
                let white = colors[k];
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
            // Bloc de hangar : sa place et son chemin, dans le repère du modèle
            if let Some(h) = &def.hangar {
                let door = def.parts.iter().position(|p| p.name == h.door).map(|k| (base + k) as u16);
                let n = self.model.hangars.iter().filter(|x| x.category == h.category && x.cargo == h.cargo).count() + 1;
                let what = if h.cargo { "Soute a cargos" } else { "Hangar" };
                self.model.hangars.push(Hangar {
                    name: format!("{what} {} {n}", h.category.name().to_lowercase()),
                    category: h.category,
                    cargo: h.cargo,
                    door,
                    slot: place.point(def, at, Vec3::from_array(h.slot)).to_array(),
                    facing: place.vector(Vec3::from_array(h.facing)).to_array(),
                    path: h.path.iter().map(|p| place.point(def, at, Vec3::from_array(*p)).to_array()).collect(),
                });
            }
        }
        if let Some(b) = self.stroke.as_mut() {
            b.zones = Some((before, self.model.zones.clone()));
            if def.hangar.is_some() {
                b.hangars = Some((hangars_before, self.model.hangars.clone()));
            }
        }
        self.end();
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
        let hangars_before = self.model.hangars.clone();
        for h in &mut self.model.hangars {
            h.door = match h.door {
                Some(d) if d as usize == z => None,
                Some(d) if d as usize > z => Some(d - 1),
                other => other,
            };
        }
        let hangars_after = self.model.hangars.clone();
        if let Some(b) = self.stroke.as_mut() {
            b.zones = Some((before, zones));
            if hangars_after != hangars_before {
                b.hangars = Some((hangars_before, hangars_after));
            }
        }
        self.end();
        self.mesh_dirty = true;
    }
}

impl Doc {
    /// Change les hangars (un lot annulable).
    fn edit_hangars(&mut self, f: impl FnOnce(&mut Vec<Hangar>)) {
        self.end();
        let before = self.model.hangars.clone();
        f(&mut self.model.hangars);
        if self.model.hangars != before {
            let after = self.model.hangars.clone();
            self.push(Batch { hangars: Some((before, after)), ..Default::default() });
            self.dirty = true;
            self.revision += 1;
        }
    }

    /// Retire un hangar (sa porte reste une zone de mouvement).
    pub fn remove_hangar(&mut self, i: usize) {
        self.edit_hangars(|h| {
            if i < h.len() {
                h.remove(i);
            }
        });
    }

    /// Ajoute un point au bout extérieur du chemin d'un hangar (le vaisseau arrive de plus loin).
    pub fn extend_path(&mut self, i: usize, p: Vec3) {
        self.edit_hangars(|h| {
            if let Some(h) = h.get_mut(i) {
                h.path.insert(0, p.to_array());
            }
        });
    }

    /// Retire le point extérieur du chemin (il en reste au moins deux).
    pub fn shorten_path(&mut self, i: usize) {
        self.edit_hangars(|h| {
            if let Some(h) = h.get_mut(i) {
                if h.path.len() > 2 {
                    h.path.remove(0);
                }
            }
        });
    }
}

/// Qui entre dans quel hangar (Q8, proposition) : un croiseur accueille des chasseurs ; un capital
/// des chasseurs et des corvettes, et des cargos jusqu'à la frégate dans ses soutes.
pub fn hangar_allowed(ship: Option<ShipCategory>, guest: ShipCategory, cargo: bool) -> Result<(), String> {
    use ShipCategory::*;
    let ok = match ship {
        Some(Croiseur) => guest == Chasseur && !cargo,
        Some(Capital) => {
            if cargo {
                matches!(guest, Chasseur | Corvette | Fregate)
            } else {
                matches!(guest, Chasseur | Corvette)
            }
        }
        _ => false,
    };
    if ok {
        return Ok(());
    }
    Err(match ship {
        Some(Croiseur) => "Un croiseur n'accueille que des chasseurs (pas de soute a cargos).".into(),
        Some(Capital) => "Trop grand : un capital accueille chasseurs et corvettes, et des cargos jusqu'a la fregate.".into(),
        _ => "Les hangars sont pour les croiseurs et les capitaux.".into(),
    })
}

/// Un volume tiré à la souris (E3), en cases du modèle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    /// Boîte entre deux coins (compris).
    Box { a: IVec3, b: IVec3 },
    /// Boule : centre (case) et rayon (cases).
    Sphere { c: IVec3, r: f32 },
    /// Cylindre d'axe `axis` : centre de la base, rayon, hauteur (cases, vers + si > 0).
    Cylinder { c: IVec3, axis: usize, r: f32, h: i32 },
    /// Ligne de cases.
    Line { a: IVec3, b: IVec3 },
}

impl Shape {
    /// Boîte englobante (comprise).
    pub fn bounds(&self) -> (IVec3, IVec3) {
        match *self {
            Shape::Box { a, b } | Shape::Line { a, b } => (a.min(b), a.max(b)),
            Shape::Sphere { c, r } => {
                let k = r.ceil() as i32;
                (c - IVec3::splat(k), c + IVec3::splat(k))
            }
            Shape::Cylinder { c, axis, r, h } => {
                let k = r.ceil() as i32;
                let mut lo = c - IVec3::splat(k);
                let mut hi = c + IVec3::splat(k);
                let end = c[axis] + h - h.signum();
                lo[axis] = c[axis].min(end);
                hi[axis] = c[axis].max(end);
                (lo, hi)
            }
        }
    }

    pub fn contains(&self, p: IVec3) -> bool {
        match *self {
            Shape::Box { .. } => {
                let (lo, hi) = self.bounds();
                p.cmpge(lo).all() && p.cmple(hi).all()
            }
            Shape::Sphere { c, r } => (p - c).as_vec3().length_squared() <= (r + 0.5) * (r + 0.5),
            Shape::Cylinder { c, axis, r, .. } => {
                let (lo, hi) = self.bounds();
                let mut d = (p - c).as_vec3();
                d[axis] = 0.0;
                p[axis] >= lo[axis] && p[axis] <= hi[axis] && d.length_squared() <= (r + 0.5) * (r + 0.5)
            }
            Shape::Line { a, b } => line_cells(a, b).contains(&p),
        }
    }
}

/// Cases d'une ligne (pas le plus long axe, sans trou).
pub fn line_cells(a: IVec3, b: IVec3) -> Vec<IVec3> {
    let d = b - a;
    let n = d.abs().max_element().max(1);
    (0..=n).map(|i| (a.as_vec3() + d.as_vec3() * (i as f32 / n as f32)).round().as_ivec3()).collect()
}

/// Ce qu'on fait au presse-papiers : quart de tour autour d'un axe, ou reflet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipTransform {
    /// Quart de tour autour de x, y ou z.
    Turn(usize),
    /// Reflet le long de x, y ou z.
    Mirror(usize),
}

/// Un morceau de modèle copié (sa propre grille, la palette du modèle d'origine).
#[derive(Clone, Debug)]
pub struct Clip {
    pub model: Model,
}

impl Clip {
    pub fn count(&self) -> usize {
        self.model.voxels.count()
    }

    pub fn transformed(&self, t: ClipTransform) -> Clip {
        let s = self.model.size.as_ivec3();
        let (new_size, map): (IVec3, Box<dyn Fn(IVec3) -> IVec3>) = match t {
            ClipTransform::Mirror(a) => (s, Box::new(move |p: IVec3| {
                let mut q = p;
                q[a] = s[a] - 1 - p[a];
                q
            })),
            ClipTransform::Turn(a) => {
                let (u, v) = ((a + 1) % 3, (a + 2) % 3);
                let mut ns = s;
                ns[u] = s[v];
                ns[v] = s[u];
                (ns, Box::new(move |p: IVec3| {
                    // (u, v) -> (s_v - 1 - v, u) : un quart de tour
                    let mut q = p;
                    q[u] = s[v] - 1 - p[v];
                    q[v] = p[u];
                    q
                }))
            }
        };
        let mut m = Model::new("presse-papiers", ModelKind::Autre, None);
        m.size = new_size.as_uvec3();
        m.palette = self.model.palette.clone();
        for (p, v) in self.model.voxels.iter() {
            m.voxels.set(map(p), v);
        }
        Clip { model: m }
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
/// (case pleine touchée, case vide juste devant). Les cases cachées (calque masqué, au-delà de la
/// coupe) sont traversées. Sans case pleine, le sol (y = 0) de la grille donne la case où poser.
pub fn raycast(m: &Model, view: &Visibility, origin: Vec3, dir: Vec3, max_steps: usize) -> (Option<IVec3>, Option<IVec3>) {
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
        if m.voxels.get(cell) != 0 && view.shows(cell, m.layer_map.get(cell)) {
            return (Some(cell), prev);
        }
        // Une case au-delà de la coupe n'accueille pas de bloc
        if view.cut.is_none_or(|c| c.keeps(cell)) {
            prev = Some(cell);
        }
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

/// Maillages des faces visibles de tout le modèle par zone de mouvement (0 = corps fixe) et par
/// matière : (zone, matière, maillage), dans le repère du modèle (gabarits, petits modèles).
pub fn build_parts(m: &Model) -> Vec<(u8, usize, Mesh)> {
    super::mesh::mesh_model(m, Visibility::of(m, None))
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


/// Croiseur de démonstration (512³, E3) : coque creuse ronde, ponts, passerelle vitrée, ailes,
/// réacteurs lumineux, tourelles. Sert aux mesures (`bench_editor`) et aux captures
/// (`SPACESPORE_EDITOR_DEMO=croiseur`).
pub fn demo_cruiser() -> Doc {
    use super::format::ShipCategory;
    let mut d = Doc::new(Model::new("Croiseur de demo", ModelKind::Vaisseau, Some(ShipCategory::Croiseur)), None);
    let pe = |rgb: [u8; 3], material: Material| PaletteEntry { rgb, material };
    let (hull, deck, glass, glow, wing, red) = (
        pe([120, 125, 135], Material::Metal),
        pe([70, 72, 80], Material::Mate),
        pe([150, 210, 255], Material::Verre),
        pe([80, 180, 255], Material::Lumineuse),
        pe([96, 100, 110], Material::Metal),
        pe([170, 40, 40], Material::Mate),
    );
    let (cx, cy) = (256, 200);
    // Coque : cylindres le long de z, effilés vers l'avant, puis creusés
    for z0 in (20..490).step_by(10) {
        let t = (z0 - 20) as f32 / 470.0;
        let r = 70.0 * if t > 0.7 { ((1.0 - t) / 0.3).powf(0.7) } else { 1.0 }.max(0.12);
        let _ = d.apply_shape(Shape::Cylinder { c: IVec3::new(cx, cy, z0), axis: 2, r, h: 10 }, Brush::Add, hull, false);
        if r > 8.0 {
            let _ = d.apply_shape(Shape::Cylinder { c: IVec3::new(cx, cy, z0), axis: 2, r: r - 3.0, h: 10 }, Brush::Remove, hull, false);
        }
    }
    // Ponts intérieurs
    for y in [cy - 30, cy, cy + 30] {
        let _ = d.apply_shape(Shape::Box { a: IVec3::new(cx - 52, y, 40), b: IVec3::new(cx + 52, y, 420) }, Brush::Add, deck, false);
    }
    // Bande rouge, passerelle et ses vitres
    let _ = d.apply_shape(Shape::Box { a: IVec3::new(0, cy - 4, 120), b: IVec3::new(511, cy + 4, 128) }, Brush::Paint, red, false);
    let _ = d.apply_shape(Shape::Box { a: IVec3::new(cx - 20, cy + 60, 300), b: IVec3::new(cx + 20, cy + 100, 360) }, Brush::Add, hull, false);
    let _ = d.apply_shape(Shape::Box { a: IVec3::new(cx - 16, cy + 88, 361), b: IVec3::new(cx + 16, cy + 93, 361) }, Brush::Add, glass, false);
    // Ailes (et leur reflet), réacteurs lumineux
    let _ = d.apply_shape(Shape::Box { a: IVec3::new(100, cy - 4, 250), b: IVec3::new(cx - 66, cy + 4, 400) }, Brush::Add, wing, true);
    for x in [cx - 36, cx + 36] {
        let _ = d.apply_shape(Shape::Cylinder { c: IVec3::new(x, cy, 19), axis: 2, r: 18.0, h: -15 }, Brush::Add, hull, false);
        let _ = d.apply_shape(Shape::Cylinder { c: IVec3::new(x, cy, 4), axis: 2, r: 12.0, h: 1 }, Brush::Paint, glow, false);
    }
    // Tourelles et canons
    for z in [150, 220, 420] {
        let top = cy + 70;
        let _ = d.apply_shape(Shape::Sphere { c: IVec3::new(cx, top, z), r: 10.0 }, Brush::Add, deck, false);
        let _ = d.apply_shape(Shape::Line { a: IVec3::new(cx - 3, top + 4, z), b: IVec3::new(cx - 3, top + 4, z + 30) }, Brush::Add, deck, true);
    }
    d.end();
    d
}

#[cfg(test)]
mod tests {
    use super::super::format::ShipCategory;
    use super::*;
    use bevy::math::UVec3;

    fn red() -> PaletteEntry {
        PaletteEntry { rgb: [200, 0, 0], material: Material::Mate }
    }

    fn blue() -> PaletteEntry {
        PaletteEntry { rgb: [0, 0, 200], material: Material::Metal }
    }

    fn area(m: &Model) -> f32 {
        let mut a = 0.0;
        for (_, _, mesh) in build_parts(m) {
            let Some(bevy::render::mesh::VertexAttributeValues::Float32x3(p)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else { continue };
            for q in p.chunks(4) {
                let (a0, a1, a2) = (Vec3::from(q[0]), Vec3::from(q[1]), Vec3::from(q[2]));
                a += (a1 - a0).cross(a2 - a1).length();
            }
        }
        a
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
        d.apply(Tool::Paint, Some(IVec3::new(2, 0, 0)), None, blue(), false);
        d.apply(Tool::Paint, Some(IVec3::new(5, 5, 5)), None, blue(), false);
        assert_eq!(d.model.voxels.count(), 10);
        assert_eq!(d.model.color_at(IVec3::new(2, 0, 0)), Some(blue()));
        assert_eq!(d.apply(Tool::Pick, Some(IVec3::new(2, 0, 0)), None, red(), false), Some(blue()));
        d.apply(Tool::Remove, Some(IVec3::new(2, 0, 0)), None, red(), true);
        assert_eq!(d.model.voxels.count(), 8);
        assert!(d.dirty);
        assert!(d.dirty_chunks.contains(&IVec3::ZERO));
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
        let all = Visibility::of(&m, None);
        // Vu d'en haut : la case et la case vide au-dessus
        let (hit, place) = raycast(&m, &all, Vec3::new(3.5, 20.0, 3.5), Vec3::NEG_Y, 100);
        assert_eq!((hit, place), (Some(IVec3::new(3, 2, 3)), Some(IVec3::new(3, 3, 3))));
        // De côté
        let (hit, place) = raycast(&m, &all, Vec3::new(-5.0, 2.5, 3.5), Vec3::X, 100);
        assert_eq!((hit, place), (Some(IVec3::new(3, 2, 3)), Some(IVec3::new(2, 2, 3))));
        // Rien sur le chemin : le sol
        let (hit, place) = raycast(&m, &all, Vec3::new(6.5, 10.0, 6.5), Vec3::new(0.0, -1.0, 0.01), 100);
        assert_eq!(hit, None);
        assert_eq!(place, Some(IVec3::new(6, 0, 6)));
        // Hors de la grille : rien
        assert_eq!(raycast(&m, &all, Vec3::new(50.0, 10.0, 50.0), Vec3::new(0.0, -1.0, 0.0), 100), (None, None));
        // Coupée (y <= 1) : le rayon traverse le bloc
        let cut = Visibility::of(&m, Some(Cut { axis: 1, pos: 1 }));
        assert_eq!(raycast(&m, &cut, Vec3::new(3.5, 20.0, 3.5), Vec3::NEG_Y, 100).0, None);
    }

    #[test]
    fn replace_a_color_everywhere_in_one_undo() {
        let mut d = Doc::new(Model::new("p", ModelKind::Autre, None), None);
        d.begin();
        for x in 0..6 {
            d.apply(Tool::Add, None, Some(IVec3::new(x, 0, 0)), if x % 2 == 0 { red() } else { blue() }, false);
        }
        d.end();
        let from = d.model.color_index(red()).unwrap();
        let gold = PaletteEntry { rgb: [212, 175, 55], material: Material::Metal };
        assert_eq!(d.replace_color(from, gold), 3);
        assert_eq!(d.model.color_at(IVec3::new(2, 0, 0)), Some(gold));
        assert_eq!(d.model.color_at(IVec3::new(1, 0, 0)), Some(blue()));
        assert!(d.undo());
        assert_eq!(d.model.color_at(IVec3::new(2, 0, 0)), Some(red()));
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
        let before = area(&d.model);
        d.place_block(door, IVec3::new(4, 0, 4), Placement::default(), false).unwrap();
        let parts = build_parts(&d.model);
        assert_eq!(parts.iter().map(|(z, _, _)| *z).collect::<Vec<_>>(), vec![0, 1]);
        // La face entre le corps et la porte est gardée des deux côtés
        assert_eq!(area(&d.model), before + 2.0 * 8.0 * 5.0 + 2.0 * 8.0 + 2.0 * 5.0);
    }

    #[test]
    fn volume_tools_mirror_and_undo_by_chunk() {
        let mut d = Doc::new(Model::new("v", ModelKind::Vaisseau, Some(ShipCategory::Chasseur)), None);
        // Boîte de 10 x 20 x 40 (à cheval sur plusieurs chunks) et son reflet
        let n = d.apply_shape(Shape::Box { a: IVec3::new(2, 0, 10), b: IVec3::new(11, 19, 49) }, Brush::Add, red(), true).unwrap();
        assert_eq!(n, 2 * 10 * 20 * 40);
        assert_eq!(d.model.voxels.get(IVec3::new(63 - 5, 10, 20)), d.model.voxels.get(IVec3::new(5, 10, 20)));
        // Sphère retirée au milieu de la boîte gauche
        let r = d.apply_shape(Shape::Sphere { c: IVec3::new(6, 10, 30), r: 3.0 }, Brush::Remove, red(), false).unwrap();
        assert_eq!(r, 179);
        // Cylindre peint, ligne ajoutée
        let p = d.apply_shape(Shape::Cylinder { c: IVec3::new(6, 0, 15), axis: 1, r: 2.0, h: 5 }, Brush::Paint, blue(), false).unwrap();
        assert!(p > 30, "{p}");
        assert_eq!(d.model.color_at(IVec3::new(6, 4, 15)), Some(blue()));
        assert_eq!(d.model.color_at(IVec3::new(6, 5, 15)), Some(red()));
        let l = d.apply_shape(Shape::Line { a: IVec3::new(20, 30, 0), b: IVec3::new(30, 35, 9) }, Brush::Add, blue(), false).unwrap();
        assert_eq!(l, 11);
        // Chaque outil = un annuler
        let total = d.model.voxels.count();
        assert!(d.undo());
        assert_eq!(d.model.voxels.count(), total - 11);
        assert!(d.undo());
        assert_eq!(d.model.color_at(IVec3::new(6, 4, 15)), Some(red()));
        assert!(d.undo());
        assert!(d.undo());
        assert_eq!(d.model.voxels.count(), 0);
        assert_eq!(d.model.voxels.chunk_count(), 0, "les chunks vides disparaissent");
    }

    #[test]
    fn flood_fill_paints_one_connected_region() {
        let mut d = Doc::new(Model::new("o", ModelKind::Autre, None), None);
        d.apply_shape(Shape::Box { a: IVec3::ZERO, b: IVec3::new(4, 0, 4) }, Brush::Add, red(), false).unwrap();
        d.apply_shape(Shape::Box { a: IVec3::new(10, 0, 0), b: IVec3::new(12, 0, 2) }, Brush::Add, red(), false).unwrap();
        let n = d.flood(IVec3::ZERO, IVec3::Y, Brush::Paint, blue(), 1000).unwrap();
        assert_eq!(n, 25);
        assert_eq!(d.model.color_at(IVec3::new(11, 0, 1)), Some(red()));
        // Ajouter : la région vide du plan, bornée par la grille
        let e = d.flood(IVec3::new(20, 5, 20), IVec3::Y, Brush::Add, blue(), 100_000).unwrap();
        assert_eq!(e, 32 * 32);
        assert!(d.flood(IVec3::new(20, 6, 20), IVec3::Y, Brush::Add, blue(), 100).is_err());
    }

    #[test]
    fn selections_copy_rotate_and_paste() {
        let mut d = Doc::new(Model::new("o", ModelKind::Autre, None), None);
        // Un « L » : 3 cases en x, 2 en z
        for p in [IVec3::new(2, 0, 2), IVec3::new(3, 0, 2), IVec3::new(4, 0, 2), IVec3::new(2, 0, 3)] {
            let i = d.model.color_index(red()).unwrap();
            d.set(p, i, false);
        }
        let clip = d.copy(IVec3::new(2, 0, 2), IVec3::new(4, 0, 3));
        assert_eq!((clip.count(), clip.model.size), (4, UVec3::new(3, 1, 2)));
        // Quatre quarts de tour : le même ; un quart : la boîte tourne
        let t = ClipTransform::Turn(1);
        let back = clip.transformed(t).transformed(t).transformed(t).transformed(t);
        assert_eq!(back.model.voxels, clip.model.voxels);
        assert_eq!(clip.transformed(t).model.size, UVec3::new(2, 1, 3));
        assert_eq!(clip.transformed(ClipTransform::Mirror(0)).count(), 4);
        // Coller ailleurs, tourner la sélection sur place (un annuler chacun)
        assert_eq!(d.paste(&clip, IVec3::new(10, 5, 10)).unwrap(), 4);
        assert_eq!(d.model.voxels.count(), 8);
        let (lo, hi) = d.transform_selection(IVec3::new(2, 0, 2), IVec3::new(4, 0, 3), t).unwrap();
        assert_eq!(hi - lo, IVec3::new(1, 0, 2));
        assert_eq!(d.model.voxels.count(), 8);
        assert!(d.undo());
        assert_eq!(d.model.voxels.get(IVec3::new(4, 0, 2)), d.model.color_index(red()).unwrap());
        // Effacer
        assert_eq!(d.clear(IVec3::new(10, 5, 10), IVec3::new(12, 5, 11)), 4);
    }

    #[test]
    fn layers_hide_and_protect_their_blocks() {
        let mut d = Doc::new(Model::new("o", ModelKind::Autre, None), None);
        d.apply_shape(Shape::Box { a: IVec3::ZERO, b: IVec3::new(3, 3, 3) }, Brush::Add, red(), false).unwrap();
        let k = d.add_layer();
        assert_eq!(k, 1);
        d.apply_shape(Shape::Box { a: IVec3::new(5, 0, 0), b: IVec3::new(6, 1, 1) }, Brush::Add, blue(), false).unwrap();
        assert_eq!(d.model.layer_map.get(IVec3::new(5, 0, 0)), 1);
        d.toggle_layer(0);
        // Le calque caché ne se voit plus et ne se retire pas
        assert_eq!(area(&d.model), 24.0);
        let r = d.apply_shape(Shape::Box { a: IVec3::ZERO, b: IVec3::new(10, 3, 3) }, Brush::Remove, red(), false).unwrap();
        assert_eq!(r, 8);
        assert_eq!(d.model.voxels.count(), 64);
        // Le fichier garde les calques
        let back = Model::from_bytes(&d.model.to_bytes().unwrap()).unwrap();
        assert_eq!(back.layers.len(), 2);
        assert!(!back.layers[0].visible);
    }

    /// Mesures de la règle 8 (croiseur 512³) :
    /// `cargo test --release bench_editor -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn bench_editor() {
        use super::super::mesh::ChunkJob;
        use std::time::Instant;
        let t = Instant::now();
        let mut d = demo_cruiser();
        println!("croiseur construit en {:.0} ms", t.elapsed().as_secs_f64() * 1000.0);
        let m = &d.model;
        let keys: Vec<IVec3> = m.voxels.keys().collect();
        let mem = m.voxels.memory() + m.zone_map.memory() + m.layer_map.memory();
        println!("{} voxels, {} chunks, memoire {:.1} Mo, fichier {:.2} Mo", m.voxels.count(), keys.len(), mem as f64 / 1e6, m.to_bytes().map_or(0, |b| b.len()) as f64 / 1e6);
        let view = d.view();
        for step in [1, 2, 4] {
            let t = Instant::now();
            let quads: usize = keys.iter().map(|k| ChunkJob::new(m, *k, step, view).run_buffers().values().map(|b| b.quads()).sum::<usize>()).sum();
            let ms = t.elapsed().as_secs_f64() * 1000.0;
            println!("maillage detail {step} : {quads} rectangles, {ms:.0} ms sur un fil ({:.2} ms par chunk)", ms / keys.len() as f64);
        }
        // Le même, sur tous les coeurs (comme le jeu)
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
        let t = Instant::now();
        std::thread::scope(|s| {
            for part in keys.chunks(keys.len().div_ceil(threads)) {
                s.spawn(move || part.iter().map(|k| ChunkJob::new(m, *k, 1, view).run_buffers().len()).sum::<usize>());
            }
        });
        println!("maillage complet sur {threads} fils : {:.0} ms", t.elapsed().as_secs_f64() * 1000.0);
        // Pose d'un bloc sur la coque + remaillage de ses chunks (règle 8 : < 5 ms)
        let color = PaletteEntry { rgb: [255, 0, 0], material: Material::Mate };
        let p = IVec3::new(256, 200 + 72, 200);
        d.dirty_chunks.clear();
        let t = Instant::now();
        d.apply(Tool::Add, None, Some(p), color, false);
        let dirty: Vec<IVec3> = d.dirty_chunks.drain().collect();
        for c in &dirty {
            ChunkJob::new(&d.model, *c, 1, d.view()).run();
        }
        let place_ms = t.elapsed().as_secs_f64() * 1000.0;
        println!("pose d'un bloc + remaillage de {} chunks : {place_ms:.2} ms", dirty.len());
        assert!(!dirty.is_empty() && place_ms < 5.0 * dirty.len() as f64, "{place_ms}");
        // Le plus long : le chunk seul
        let t = Instant::now();
        ChunkJob::new(&d.model, chunk_of(p), 1, d.view()).run();
        println!("remaillage d'un chunk seul : {:.2} ms", t.elapsed().as_secs_f64() * 1000.0);
        // Remplir une boîte de 256³ (règle 8 : < 1 s), puis l'annuler
        let mut e = Doc::new(Model::new("vide", ModelKind::Vaisseau, Some(super::super::format::ShipCategory::Croiseur)), None);
        let t = Instant::now();
        let n = e.apply_shape(Shape::Box { a: IVec3::splat(100), b: IVec3::splat(355) }, Brush::Add, color, false).unwrap();
        let fill_ms = t.elapsed().as_secs_f64() * 1000.0;
        let t = Instant::now();
        e.undo();
        println!("boite de 256^3 : {n} blocs en {fill_ms:.0} ms, annulee en {:.0} ms", t.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(n, 256 * 256 * 256);
        assert!(fill_ms < 1000.0, "{fill_ms}");
    }
}
