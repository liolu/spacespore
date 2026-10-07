//! Maillage glouton par chunk (E3, règle 3 de `roadmaps/fait/ROADMAP-0.12-editeur.md`).
//!
//! Un chunk 32³ est maillé seul, hors du fil principal : `ChunkJob` emporte le chunk et ses 26
//! voisins (données partagées : rien n'est copié), la palette, les calques cachés et la coupe ;
//! `ChunkJob::run` fusionne les faces visibles de même couleur en grands rectangles. Une face n'est
//! cachée que par un voisin de la même zone de mouvement (la pièce bouge : ce qu'elle cache peut se
//! voir) ; le verre laisse voir ce qui est derrière lui. `step` = niveau de détail (1, 2 ou 4 : une
//! case de maillage pour 2³ ou 4³ voxels, quand le chunk est loin).

use bevy::math::{IVec3, Vec3};
use bevy::render::mesh::{Indices, Mesh, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use std::collections::HashMap;

use super::format::{local_index, Chunk, Material, Model, PaletteEntry, Sparse, CHUNK};

/// Coupe (vue de l'intérieur) : on ne garde que les cases dont la coordonnée `axis` est ≤ `pos`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cut {
    pub axis: usize,
    pub pos: i32,
}

impl Cut {
    pub fn keeps(&self, p: IVec3) -> bool {
        p[self.axis] <= self.pos
    }
}

/// Ce qui est visible : calques cachés et coupe.
#[derive(Clone, Copy, Debug)]
pub struct Visibility {
    pub hidden: [bool; 256],
    pub cut: Option<Cut>,
}

impl Visibility {
    pub fn of(m: &Model, cut: Option<Cut>) -> Self {
        let mut hidden = [false; 256];
        for (i, l) in m.layers.iter().enumerate().take(256) {
            hidden[i] = !l.visible;
        }
        Self { hidden, cut }
    }

    pub fn shows(&self, p: IVec3, layer: u8) -> bool {
        !self.hidden[layer as usize] && self.cut.is_none_or(|c| c.keeps(p))
    }
}

/// Ordre des faces : +X, -X, +Y, -Y, +Z, -Z (coins dans le sens de `edit::FACES`).
const FACES: [(IVec3, [[f32; 3]; 4]); 6] = [
    (IVec3::X, [[1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [1.0, 1.0, 1.0], [1.0, 0.0, 1.0]]),
    (IVec3::NEG_X, [[0.0, 0.0, 1.0], [0.0, 1.0, 1.0], [0.0, 1.0, 0.0], [0.0, 0.0, 0.0]]),
    (IVec3::Y, [[0.0, 1.0, 0.0], [0.0, 1.0, 1.0], [1.0, 1.0, 1.0], [1.0, 1.0, 0.0]]),
    (IVec3::NEG_Y, [[0.0, 0.0, 1.0], [0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 0.0, 1.0]]),
    (IVec3::Z, [[1.0, 0.0, 1.0], [1.0, 1.0, 1.0], [0.0, 1.0, 1.0], [0.0, 0.0, 1.0]]),
    (IVec3::NEG_Z, [[0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 1.0, 0.0], [1.0, 0.0, 0.0]]),
];

/// Ombrage simple par face : le dessus plus clair, le dessous plus sombre.
const SHADE: [f32; 6] = [0.86, 0.86, 1.0, 0.6, 0.93, 0.93];

pub fn material_index(mat: Material) -> usize {
    match mat {
        Material::Mate => 0,
        Material::Metal => 1,
        Material::Verre => 2,
        Material::Lumineuse => 3,
    }
}

/// Couleur linéaire (sommets) d'une entrée de palette sRGB.
pub fn srgb(e: &PaletteEntry) -> [f32; 3] {
    e.rgb.map(|c| (c as f32 / 255.0).powf(2.2))
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
    /// Une face `k` (ordre de `FACES`) : rectangle dont les coins du gabarit unité sont envoyés par
    /// `corner` (gabarit -> position).
    fn quad(&mut self, k: usize, c: [f32; 3], corner: impl Fn([f32; 3]) -> [f32; 3]) {
        let (n, quad) = &FACES[k];
        let base = self.pos.len() as u32;
        let s = SHADE[k];
        for q in quad {
            self.pos.push(corner(*q));
            self.nor.push(n.as_vec3().to_array());
            self.col.push([c[0] * s, c[1] * s, c[2] * s, 1.0]);
        }
        self.idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    pub fn append(&mut self, b: Buffers) {
        let base = self.pos.len() as u32;
        self.pos.extend(b.pos);
        self.nor.extend(b.nor);
        self.col.extend(b.col);
        self.idx.extend(b.idx.into_iter().map(|i| i + base));
    }

    /// Rectangles (faces fusionnées).
    #[cfg(test)]
    pub fn quads(&self) -> usize {
        self.idx.len() / 6
    }

    pub fn is_empty(&self) -> bool {
        self.idx.is_empty()
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

/// Ce qu'il faut pour mailler un chunk ailleurs que sur le fil principal.
pub struct ChunkJob {
    pub key: IVec3,
    pub step: i32,
    /// Le chunk et ses 26 voisins (index (dx+1) + 3·((dy+1) + 3·(dz+1))) : voxels, zones, calques.
    vox: [Option<Chunk>; 27],
    zone: [Option<Chunk>; 27],
    layer: [Option<Chunk>; 27],
    palette: Vec<PaletteEntry>,
    view: Visibility,
    zoned: bool,
}

fn neighbours(s: &Sparse, key: IVec3) -> [Option<Chunk>; 27] {
    std::array::from_fn(|i| {
        let d = IVec3::new(i as i32 % 3, (i as i32 / 3) % 3, i as i32 / 9) - IVec3::ONE;
        s.chunk(key + d).cloned()
    })
}

impl ChunkJob {
    pub fn new(m: &Model, key: IVec3, step: i32, view: Visibility) -> Self {
        Self {
            key,
            step: step.clamp(1, 4),
            vox: neighbours(&m.voxels, key),
            zone: neighbours(&m.zone_map, key),
            layer: neighbours(&m.layer_map, key),
            palette: m.palette.clone(),
            view,
            zoned: !m.zones.is_empty(),
        }
    }

    /// (valeur, zone) d'un voxel, coordonnées locales au chunk (-32..64) ; invisible = vide.
    fn voxel(&self, l: IVec3) -> (u8, u8) {
        let d = l.div_euclid(IVec3::splat(CHUNK));
        let i = ((d.x + 1) + 3 * ((d.y + 1) + 3 * (d.z + 1))) as usize;
        let Some(ch) = &self.vox[i] else { return (0, 0) };
        let li = local_index(l.rem_euclid(IVec3::splat(CHUNK)));
        let v = ch.get(li);
        if v == 0 {
            return (0, 0);
        }
        let layer = self.layer[i].as_ref().map_or(0, |c| c.get(li));
        if !self.view.shows(self.key * CHUNK + l, layer) {
            return (0, 0);
        }
        (v, if self.zoned { self.zone[i].as_ref().map_or(0, |c| c.get(li)) } else { 0 })
    }

    /// Une case de maillage (taille `step`) : le premier voxel visible qu'elle contient.
    fn cell(&self, c: IVec3) -> (u8, u8) {
        let s = self.step;
        if s == 1 {
            return self.voxel(c);
        }
        for z in 0..s {
            for y in 0..s {
                for x in 0..s {
                    let r = self.voxel(c * s + IVec3::new(x, y, z));
                    if r.0 != 0 {
                        return r;
                    }
                }
            }
        }
        (0, 0)
    }

    /// Faces visibles du chunk, fusionnées, par (zone, matière).
    pub fn run_buffers(&self) -> HashMap<(u8, usize), Buffers> {
        if self.vox[13].is_none() {
            return HashMap::new();
        }
        let n = CHUNK / self.step;
        let w = (n + 2) as usize;
        // Cases avec une bordure d'une case (les voisins)
        let mut grid = vec![(0u8, 0u8); w * w * w];
        let at = |p: IVec3| ((p.x + 1) as usize) + w * (((p.y + 1) as usize) + w * (p.z + 1) as usize);
        let mut any = false;
        for z in -1..=n {
            for y in -1..=n {
                for x in -1..=n {
                    let p = IVec3::new(x, y, z);
                    let inside = p.cmpge(IVec3::ZERO).all() && p.cmplt(IVec3::splat(n)).all();
                    // Bordure : seulement les cases qui touchent une face du chunk
                    let edge = [x, y, z].iter().filter(|c| **c == -1 || **c == n).count();
                    if !inside && edge > 1 {
                        continue;
                    }
                    let c = self.cell(p);
                    any |= inside && c.0 != 0;
                    grid[at(p)] = c;
                }
            }
        }
        let mut out: HashMap<(u8, usize), Buffers> = HashMap::new();
        if !any {
            return out;
        }
        let mat = |v: u8| self.palette.get(v as usize - 1).map_or(Material::Mate, |e| e.material);
        let origin = (self.key * CHUNK).as_vec3();
        let s = self.step as f32;
        let mut mask = vec![0u16; (n * n) as usize];
        for k in 0..6 {
            let (normal, _) = FACES[k];
            let a = k / 2;
            let (u, v) = ((a + 1) % 3, (a + 2) % 3);
            for d in 0..n {
                // Masque des faces visibles de la tranche : clé = valeur + zone
                for j in 0..n {
                    for i in 0..n {
                        let mut p = IVec3::ZERO;
                        p[a] = d;
                        p[u] = i;
                        p[v] = j;
                        let (val, zone) = grid[at(p)];
                        let mut key = 0;
                        if val != 0 {
                            let (nv, nz) = grid[at(p + normal)];
                            let hidden = nv != 0 && nz == zone && (mat(nv) != Material::Verre || mat(val) == Material::Verre);
                            if !hidden {
                                key = val as u16 | (zone as u16) << 8;
                            }
                        }
                        mask[(i + j * n) as usize] = key;
                    }
                }
                // Rectangles gloutons
                for j in 0..n {
                    let mut i = 0;
                    while i < n {
                        let key = mask[(i + j * n) as usize];
                        if key == 0 {
                            i += 1;
                            continue;
                        }
                        let mut wd = 1;
                        while i + wd < n && mask[(i + wd + j * n) as usize] == key {
                            wd += 1;
                        }
                        let mut ht = 1;
                        'grow: while j + ht < n {
                            for x in i..i + wd {
                                if mask[(x + (j + ht) * n) as usize] != key {
                                    break 'grow;
                                }
                            }
                            ht += 1;
                        }
                        for y in j..j + ht {
                            for x in i..i + wd {
                                mask[(x + y * n) as usize] = 0;
                            }
                        }
                        let val = (key & 0xff) as u8;
                        let zone = (key >> 8) as u8;
                        let Some(e) = self.palette.get(val as usize - 1) else {
                            i += wd;
                            continue;
                        };
                        let c = srgb(e);
                        let (i0, j0, wf, hf, df) = (i as f32, j as f32, wd as f32, ht as f32, d as f32);
                        out.entry((zone, material_index(e.material))).or_default().quad(k, c, |q| {
                            let mut r = Vec3::ZERO;
                            r[a] = df + q[a];
                            r[u] = i0 + q[u] * wf;
                            r[v] = j0 + q[v] * hf;
                            (origin + r * s).to_array()
                        });
                        i += wd;
                    }
                }
            }
        }
        out
    }

    pub fn run(&self) -> Vec<(u8, usize, Mesh)> {
        let mut v: Vec<(u8, usize, Mesh)> = self.run_buffers().into_iter().filter(|(_, b)| !b.is_empty()).map(|((z, k), b)| (z, k, b.mesh())).collect();
        v.sort_by_key(|(z, k, _)| (*z, *k));
        v
    }
}

/// Tout le modèle d'un coup (petits modèles : gabarits, collage), par (zone, matière).
pub fn mesh_model(m: &Model, view: Visibility) -> Vec<(u8, usize, Mesh)> {
    let mut all: HashMap<(u8, usize), Buffers> = HashMap::new();
    let mut keys: Vec<IVec3> = m.voxels.keys().collect();
    keys.sort_by_key(|k| (k.z, k.y, k.x));
    for key in keys {
        for (g, b) in ChunkJob::new(m, key, 1, view).run_buffers() {
            all.entry(g).or_default().append(b);
        }
    }
    let mut v: Vec<(u8, usize, Mesh)> = all.into_iter().filter(|(_, b)| !b.is_empty()).map(|((z, k), b)| (z, k, b.mesh())).collect();
    v.sort_by_key(|(z, k, _)| (*z, *k));
    v
}

#[cfg(test)]
mod tests {
    use super::super::format::{ModelKind, ShipCategory};
    use super::*;

    /// Aire totale des faces (en faces unité) : ne dépend pas de la fusion.
    pub fn area(meshes: &[(u8, usize, Mesh)]) -> f32 {
        let mut a = 0.0;
        for (_, _, m) in meshes {
            let Some(bevy::render::mesh::VertexAttributeValues::Float32x3(p)) = m.attribute(Mesh::ATTRIBUTE_POSITION) else { continue };
            for q in p.chunks(4) {
                let (a0, a1, a2) = (Vec3::from(q[0]), Vec3::from(q[1]), Vec3::from(q[2]));
                a += (a1 - a0).cross(a2 - a1).length();
            }
        }
        a
    }

    fn red() -> PaletteEntry {
        PaletteEntry { rgb: [200, 0, 0], material: Material::Mate }
    }

    #[test]
    fn faces_merge_and_hidden_faces_vanish() {
        let mut m = Model::new("p", ModelKind::Autre, None);
        let i = m.color_index(red()).unwrap();
        let all = Visibility::of(&m, None);
        m.voxels.set(IVec3::ZERO, i);
        assert_eq!(area(&mesh_model(&m, all)), 6.0);
        // Une barre de 10 : 42 faces, mais 6 rectangles seulement
        for x in 0..10 {
            m.voxels.set(IVec3::new(x, 0, 0), i);
        }
        let mm = mesh_model(&m, all);
        assert_eq!(area(&mm), 42.0);
        assert_eq!(mm.iter().map(|(_, _, m)| m.count_vertices()).sum::<usize>(), 24);
        // Le verre laisse voir la face rouge derrière lui
        let g = m.color_index(PaletteEntry { rgb: [100, 200, 255], material: Material::Verre }).unwrap();
        m.voxels.set(IVec3::new(10, 0, 0), g);
        assert_eq!(area(&mesh_model(&m, all)), 42.0 + 5.0);
    }

    #[test]
    fn chunk_borders_cut_layers_and_lod() {
        let mut m = Model::new("v", ModelKind::Vaisseau, Some(ShipCategory::Chasseur));
        let i = m.color_index(red()).unwrap();
        // Un cube de 40 à cheval sur 8 chunks : seule sa surface est maillée
        for z in 10..50 {
            for y in 10..50 {
                for x in 10..50 {
                    m.voxels.set(IVec3::new(x, y, z), i);
                }
            }
        }
        let all = Visibility::of(&m, None);
        assert_eq!(area(&mesh_model(&m, all)), 6.0 * 40.0 * 40.0);
        // Coupe à y = 29 : on voit l'intérieur (le dessus de la tranche)
        let cut = Visibility::of(&m, Some(Cut { axis: 1, pos: 29 }));
        assert_eq!(area(&mesh_model(&m, cut)), 40.0 * 40.0 * 2.0 + 4.0 * 40.0 * 20.0);
        // Calque caché : rien
        m.layers = vec![super::super::format::Layer { name: "a".into(), visible: false }];
        assert_eq!(area(&mesh_model(&m, Visibility::of(&m, None))), 0.0);
        m.layers.clear();
        // Moins de détail : la même boîte en cases de 2 (aire identique, cube aligné sur 2)
        let job = ChunkJob::new(&m, IVec3::ZERO, 2, all);
        let full = ChunkJob::new(&m, IVec3::ZERO, 1, all);
        let a2: f32 = job.run_buffers().into_values().map(|b| area(&[(0, 0, b.mesh())])).sum();
        let a1: f32 = full.run_buffers().into_values().map(|b| area(&[(0, 0, b.mesh())])).sum();
        assert_eq!(a1, a2);
    }
}
