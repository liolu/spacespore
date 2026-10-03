//! Édition d'un modèle (E1), sans Bevy : outils, symétrie miroir, annuler / rétablir par lots,
//! rayon de la souris jusqu'à la case visée, maillage des faces visibles.
//!
//! Porté de `VoxelEditorManager.cs` (Pixel World) : outils Ajouter / Retirer / Peindre, pipette
//! (Maj+clic ou outil 4), un trait de la souris = une action annulable.

use bevy::math::{IVec3, Vec3};
use bevy::render::mesh::{Indices, Mesh, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use std::path::PathBuf;

use super::format::{Material, Model, ModelKind, PaletteEntry};

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

/// Une case changée : position, ancien et nouvel index de palette.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Change {
    pub pos: IVec3,
    pub old: u8,
    pub new: u8,
}

/// Un modèle ouvert (onglet) : le modèle, son fichier, et l'historique.
#[derive(Clone, Debug)]
pub struct Doc {
    pub model: Model,
    pub path: Option<PathBuf>,
    pub dirty: bool,
    undo: Vec<Vec<Change>>,
    redo: Vec<Vec<Change>>,
    /// Trait en cours (souris enfoncée) : un seul lot annulable.
    stroke: Option<Vec<Change>>,
    /// Le maillage est à refaire.
    pub mesh_dirty: bool,
}

/// Lots gardés au plus (les plus anciens s'oublient).
const MAX_UNDO: usize = 200;

impl Doc {
    pub fn new(model: Model, path: Option<PathBuf>) -> Self {
        Self { model, path, dirty: false, undo: Vec::new(), redo: Vec::new(), stroke: None, mesh_dirty: true }
    }

    /// Commence un trait (clic) : tout jusqu'au relâchement sera annulé d'un coup.
    pub fn begin(&mut self) {
        self.end();
        self.stroke = Some(Vec::new());
    }

    /// Fin du trait.
    pub fn end(&mut self) {
        if let Some(s) = self.stroke.take() {
            if !s.is_empty() {
                self.undo.push(s);
                if self.undo.len() > MAX_UNDO {
                    self.undo.remove(0);
                }
                self.redo.clear();
            }
        }
    }

    /// Pose la valeur `v` (0 = vide) en `p` (et en miroir si demandé), dans le trait en cours.
    pub fn set(&mut self, p: IVec3, v: u8, mirror: bool) -> bool {
        let mut changed = false;
        let targets = if mirror { mirrored(&self.model, p) } else { vec![p] };
        for q in targets {
            if !self.model.in_bounds(q) {
                continue;
            }
            let old = self.model.voxels.get(q);
            if old == v {
                continue;
            }
            self.model.voxels.set(q, v);
            let c = Change { pos: q, old, new: v };
            match &mut self.stroke {
                Some(s) => s.push(c),
                None => {
                    self.undo.push(vec![c]);
                    self.redo.clear();
                }
            }
            changed = true;
        }
        if changed {
            self.dirty = true;
            self.mesh_dirty = true;
        }
        changed
    }

    pub fn undo(&mut self) -> bool {
        self.end();
        let Some(batch) = self.undo.pop() else { return false };
        for c in batch.iter().rev() {
            self.model.voxels.set(c.pos, c.old);
        }
        self.redo.push(batch);
        self.dirty = true;
        self.mesh_dirty = true;
        true
    }

    pub fn redo(&mut self) -> bool {
        self.end();
        let Some(batch) = self.redo.pop() else { return false };
        for c in &batch {
            self.model.voxels.set(c.pos, c.new);
        }
        self.undo.push(batch);
        self.dirty = true;
        self.mesh_dirty = true;
        true
    }

    /// Applique un outil à la case visée `hit` (case pleine) et `place` (case vide devant elle).
    /// Renvoie la couleur prise par la pipette.
    pub fn apply(&mut self, tool: Tool, hit: Option<IVec3>, place: Option<IVec3>, color: PaletteEntry, mirror: bool) -> Option<PaletteEntry> {
        match tool {
            Tool::Add => {
                if let (Some(p), Some(i)) = (place, self.model.color_index(color)) {
                    self.set(p, i, mirror);
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
}

/// La case et son reflet par rapport au milieu de la grille (axe x : gauche / droite).
pub fn mirrored(m: &Model, p: IVec3) -> Vec<IVec3> {
    let q = IVec3::new(m.size.x as i32 - 1 - p.x, p.y, p.z);
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

/// Maillage des faces visibles du modèle (une case = un cube unité), couleurs aux sommets.
/// (Maillage glouton par chunk, hors du fil principal : E3.)
pub fn build_mesh(m: &Model) -> Mesh {
    const FACES: [(IVec3, [[f32; 3]; 4]); 6] = [
        (IVec3::X, [[1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [1.0, 1.0, 1.0], [1.0, 0.0, 1.0]]),
        (IVec3::NEG_X, [[0.0, 0.0, 1.0], [0.0, 1.0, 1.0], [0.0, 1.0, 0.0], [0.0, 0.0, 0.0]]),
        (IVec3::Y, [[0.0, 1.0, 0.0], [0.0, 1.0, 1.0], [1.0, 1.0, 1.0], [1.0, 1.0, 0.0]]),
        (IVec3::NEG_Y, [[0.0, 0.0, 1.0], [0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 0.0, 1.0]]),
        (IVec3::Z, [[1.0, 0.0, 1.0], [1.0, 1.0, 1.0], [0.0, 1.0, 1.0], [0.0, 0.0, 1.0]]),
        (IVec3::NEG_Z, [[0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 1.0, 0.0], [1.0, 0.0, 0.0]]),
    ];
    let (mut pos, mut nor, mut col, mut idx) = (Vec::new(), Vec::new(), Vec::new(), Vec::<u32>::new());
    for (p, v) in m.voxels.iter() {
        let Some(e) = m.palette.get(v as usize - 1) else { continue };
        let c = srgb(e);
        for (k, (n, quad)) in FACES.iter().enumerate() {
            // Face cachée par un voisin opaque (le verre laisse voir ce qui est derrière)
            let nb = m.voxels.get(p + *n);
            if nb != 0 && m.palette.get(nb as usize - 1).is_some_and(|x| x.material != Material::Verre || e.material == Material::Verre) {
                continue;
            }
            // Ombrage simple par face : le dessus plus clair, le dessous plus sombre
            let shade = [0.86, 0.86, 1.0, 0.6, 0.93, 0.93][k];
            let base = pos.len() as u32;
            for q in quad {
                pos.push([p.x as f32 + q[0], p.y as f32 + q[1], p.z as f32 + q[2]]);
                nor.push(n.as_vec3().to_array());
                col.push([c[0] * shade, c[1] * shade, c[2] * shade, 1.0]);
            }
            idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, nor);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
    mesh.insert_indices(Indices::U32(idx));
    mesh
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
    fn starter_palette_has_every_material() {
        let p = starter_palette();
        assert!(p.len() > 40 && p.len() <= 255);
        for mat in [Material::Mate, Material::Metal, Material::Verre, Material::Lumineuse] {
            assert!(p.iter().any(|e| e.material == mat));
        }
    }
}
