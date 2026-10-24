// ─────────────────────────────────────────────────────────────────────────
//  Décor des surfaces (phase 7 de `roadmaps/fait/ROADMAP-0.10.md`)
//
//  Arbres, buissons, roseaux, cactus, rochers, cristaux, champignons géants, cosses de spores,
//  pics de glace... posés sur les tuiles de terrain selon le biome de chaque endroit. Seules les
//  tuiles proches (les deux niveaux les plus fins) en reçoivent : le décor apparaît en
//  s'approchant et disparaît avec sa tuile. Le placement est déterministe (graine, tuile,
//  colonne) et calculé avec la tuile, en arrière-plan. Tous les objets d'une sorte partagent le
//  même maillage et le même matériau : Bevy les dessine par lots (instanciation).
//
//  La végétation n'existe que là où il y a des plantes (`planetgen::life`) : les biomes verts
//  d'un monde sans vie sont déjà nus. Les rochers et cristaux sont partout.
//
//  0.14 D3, végétation vivante : les plantes ont leur matériau (`DecorMaterial`, `decor.wgsl`) dont les
//  sommets ondulent au vent de la météo (`weather::WeatherNow`, rafales comprises), le feuillage caduc
//  prend la couleur de la saison (froid de la saison par rapport à la moyenne de l'année), les plantes
//  lumineuses (champignons géants, cosses) luisent la nuit. Trait « forêt pétrifiée » réel : des arbres de
//  pierre, même sans vie. L'alpha des sommets code la hauteur dans la plante (0..0,5) + 0,5 pour le feuillage
//  caduc.
// ─────────────────────────────────────────────────────────────────────────

use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{AsBindGroup, ShaderRef, ShaderType};
use std::collections::HashMap;

use crate::planet::{MoonId, MoonRoot, PlanetId, PlanetRoot};
use crate::planetgen::biome::Biome;
use crate::terrain::{face_dir, tile_quantum, Terrain, TileKey, TILE_CELLS};
use crate::ui::TargetKind;

pub const DECOR_SHADER: Handle<Shader> = Handle::weak_from_u128(0xDEC0_8F1A_0A7E_0001_2345_6789_ABCD);

/// Paramètres du vent et de la saison (`decor.wgsl`).
#[derive(Clone, Copy, Debug, Default, ShaderType, Reflect)]
pub struct SwayParams {
    /// xyz : déplacement du sommet des plantes (unités, repère monde), w : temps (s).
    pub wind: Vec4,
    /// rgb : couleur du feuillage de saison, a : part.
    pub tint: Vec4,
    /// x : nuit (0..1), y : lueur des plantes, z : pétrifié, w : rafale.
    pub night: Vec4,
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct Sway {
    #[uniform(100)]
    pub params: SwayParams,
}

impl MaterialExtension for Sway {
    fn vertex_shader() -> ShaderRef {
        DECOR_SHADER.into()
    }
    fn fragment_shader() -> ShaderRef {
        DECOR_SHADER.into()
    }
}

/// Matériau des plantes (vent, saison, lueur, pétrification).
pub type DecorMaterial = ExtendedMaterial<StandardMaterial, Sway>;

/// Sortes de décor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DecorKind {
    Conifer,
    Broadleaf,
    JungleTree,
    Acacia,
    Bush,
    Reed,
    Cactus,
    Rock,
    Boulder,
    RustRock,
    SaltCrystal,
    SulfurVent,
    Crystal,
    GlassShard,
    GiantMushroom,
    SporePod,
    IceSpike,
    /// Champignon lumineux des grottes (B2, s'il y a de la vie).
    GlowShroom,
    /// Lieux rares (0.14 D6, `poi.rs`) : épave de vaisseau, ruines, monolithe.
    Wreck,
    Ruin,
    Monolith,
}

impl DecorKind {
    pub const ALL: [DecorKind; 21] = [
        DecorKind::Conifer,
        DecorKind::Broadleaf,
        DecorKind::JungleTree,
        DecorKind::Acacia,
        DecorKind::Bush,
        DecorKind::Reed,
        DecorKind::Cactus,
        DecorKind::Rock,
        DecorKind::Boulder,
        DecorKind::RustRock,
        DecorKind::SaltCrystal,
        DecorKind::SulfurVent,
        DecorKind::Crystal,
        DecorKind::GlassShard,
        DecorKind::GiantMushroom,
        DecorKind::SporePod,
        DecorKind::IceSpike,
        DecorKind::GlowShroom,
        DecorKind::Wreck,
        DecorKind::Ruin,
        DecorKind::Monolith,
    ];

    /// Plante (n'apparaît qu'avec de la vie).
    pub fn plant(self) -> bool {
        matches!(
            self,
            DecorKind::Conifer
                | DecorKind::Broadleaf
                | DecorKind::JungleTree
                | DecorKind::Acacia
                | DecorKind::Bush
                | DecorKind::Reed
                | DecorKind::Cactus
                | DecorKind::GiantMushroom
                | DecorKind::SporePod
                | DecorKind::GlowShroom
        )
    }

    /// Lumineux tout le temps (cristaux, soufre, champignons des grottes).
    pub fn glows(self) -> bool {
        matches!(self, DecorKind::Crystal | DecorKind::SulfurVent | DecorKind::GlowShroom)
    }

    /// Plante qui luit la nuit (0.14 D3).
    pub fn night_glow(self) -> bool {
        matches!(self, DecorKind::GiantMushroom | DecorKind::SporePod)
    }

    /// Arbre (feuillage caduc sauf le conifère) : ondule au vent, se pétrifie.
    pub fn tree(self) -> bool {
        matches!(self, DecorKind::Conifer | DecorKind::Broadleaf | DecorKind::JungleTree | DecorKind::Acacia)
    }

    /// Feuillage qui change avec la saison.
    pub fn deciduous(self) -> bool {
        matches!(self, DecorKind::Broadleaf | DecorKind::JungleTree | DecorKind::Acacia | DecorKind::Bush)
    }

    /// Cubes (centre, demi-taille, couleur), en voxels, base au sol (y = 0).
    fn cubes(self) -> Vec<([f32; 3], [f32; 3], [f32; 3])> {
        const BARK: [f32; 3] = [0.36, 0.24, 0.14];
        const LEAF: [f32; 3] = [0.16, 0.45, 0.14];
        const PINE: [f32; 3] = [0.1, 0.3, 0.17];
        const ROCK: [f32; 3] = [0.45, 0.43, 0.42];
        match self {
            DecorKind::Conifer => vec![
                ([0.0, 1.0, 0.0], [0.25, 1.0, 0.25], BARK),
                ([0.0, 2.2, 0.0], [1.2, 0.6, 1.2], PINE),
                ([0.0, 3.2, 0.0], [0.9, 0.5, 0.9], PINE),
                ([0.0, 4.0, 0.0], [0.55, 0.45, 0.55], PINE),
                ([0.0, 4.7, 0.0], [0.25, 0.3, 0.25], PINE),
            ],
            DecorKind::Broadleaf => vec![
                ([0.0, 1.2, 0.0], [0.3, 1.2, 0.3], BARK),
                ([0.0, 3.2, 0.0], [1.5, 1.0, 1.5], LEAF),
                ([0.3, 4.3, -0.2], [1.0, 0.5, 1.0], [0.2, 0.5, 0.16]),
            ],
            DecorKind::JungleTree => vec![
                ([0.0, 2.5, 0.0], [0.35, 2.5, 0.35], BARK),
                ([0.0, 5.6, 0.0], [2.0, 0.7, 2.0], [0.06, 0.38, 0.1]),
                ([1.2, 4.6, 0.6], [0.9, 0.5, 0.9], [0.08, 0.42, 0.12]),
            ],
            DecorKind::Acacia => vec![
                ([0.0, 1.5, 0.0], [0.25, 1.5, 0.25], BARK),
                ([0.0, 3.2, 0.0], [1.8, 0.35, 1.8], [0.42, 0.5, 0.2]),
            ],
            DecorKind::Bush => vec![([0.0, 0.5, 0.0], [0.7, 0.5, 0.7], [0.2, 0.42, 0.15]), ([0.2, 0.95, 0.1], [0.4, 0.3, 0.4], [0.25, 0.48, 0.18])],
            DecorKind::Reed => vec![
                ([0.0, 1.0, 0.0], [0.1, 1.0, 0.1], [0.4, 0.5, 0.22]),
                ([0.4, 0.8, 0.2], [0.1, 0.8, 0.1], [0.45, 0.52, 0.25]),
                ([-0.3, 1.2, 0.3], [0.1, 1.2, 0.1], [0.38, 0.48, 0.2]),
            ],
            DecorKind::Cactus => vec![
                ([0.0, 1.5, 0.0], [0.3, 1.5, 0.3], [0.25, 0.5, 0.25]),
                ([0.6, 1.6, 0.0], [0.3, 0.2, 0.2], [0.25, 0.5, 0.25]),
                ([0.8, 2.1, 0.0], [0.2, 0.4, 0.2], [0.25, 0.5, 0.25]),
            ],
            DecorKind::Rock => vec![([0.0, 0.4, 0.0], [0.8, 0.5, 0.6], ROCK), ([0.3, 0.85, -0.1], [0.4, 0.3, 0.4], [0.5, 0.48, 0.46])],
            DecorKind::Boulder => vec![([0.0, 1.0, 0.0], [1.6, 1.1, 1.3], [0.4, 0.39, 0.38]), ([-0.6, 2.2, 0.3], [0.8, 0.4, 0.7], ROCK)],
            DecorKind::RustRock => vec![([0.0, 0.5, 0.0], [0.9, 0.6, 0.7], [0.58, 0.27, 0.14]), ([0.3, 1.1, 0.2], [0.4, 0.3, 0.4], [0.65, 0.32, 0.16])],
            DecorKind::SaltCrystal => vec![([0.0, 0.6, 0.0], [0.4, 0.6, 0.4], [0.95, 0.92, 0.9]), ([0.5, 0.35, 0.2], [0.3, 0.35, 0.3], [0.97, 0.94, 0.94])],
            DecorKind::SulfurVent => vec![([0.0, 0.5, 0.0], [0.9, 0.5, 0.9], [0.8, 0.72, 0.2]), ([0.0, 1.1, 0.0], [0.4, 0.3, 0.4], [0.95, 0.88, 0.3])],
            DecorKind::Crystal => vec![
                ([0.0, 1.8, 0.0], [0.3, 1.8, 0.3], [0.55, 0.85, 1.0]),
                ([0.6, 1.0, 0.3], [0.22, 1.0, 0.22], [0.65, 0.75, 1.0]),
                ([-0.4, 0.7, -0.4], [0.2, 0.7, 0.2], [0.75, 0.6, 1.0]),
            ],
            DecorKind::GlassShard => vec![([0.0, 0.9, 0.0], [0.25, 0.9, 0.5], [0.12, 0.2, 0.17]), ([0.5, 0.5, 0.3], [0.4, 0.5, 0.2], [0.15, 0.24, 0.2])],
            DecorKind::GlowShroom => vec![
                ([0.0, 0.5, 0.0], [0.12, 0.5, 0.12], [0.75, 0.85, 0.8]),
                ([0.0, 1.05, 0.0], [0.55, 0.15, 0.55], [0.3, 0.95, 0.8]),
                ([0.5, 0.35, 0.3], [0.08, 0.35, 0.08], [0.75, 0.85, 0.8]),
                ([0.5, 0.75, 0.3], [0.3, 0.1, 0.3], [0.35, 0.85, 1.0]),
            ],
            DecorKind::GiantMushroom => vec![
                ([0.0, 2.0, 0.0], [0.35, 2.0, 0.35], [0.85, 0.8, 0.7]),
                ([0.0, 4.2, 0.0], [2.0, 0.5, 2.0], [0.85, 0.35, 0.25]),
                ([0.0, 4.8, 0.0], [1.2, 0.3, 1.2], [0.9, 0.45, 0.3]),
            ],
            DecorKind::SporePod => vec![
                ([0.0, 0.7, 0.0], [0.6, 0.7, 0.6], [0.7, 0.35, 0.75]),
                ([0.7, 0.4, 0.3], [0.35, 0.4, 0.35], [0.8, 0.45, 0.85]),
                ([-0.5, 0.35, -0.4], [0.3, 0.35, 0.3], [0.62, 0.3, 0.7]),
            ],
            DecorKind::IceSpike => vec![([0.0, 1.5, 0.0], [0.35, 1.5, 0.35], [0.78, 0.9, 1.0]), ([0.4, 0.8, 0.2], [0.25, 0.8, 0.25], [0.85, 0.94, 1.0])],
            // Épave : une coque brisée en deux, de guingois, une aile plantée
            DecorKind::Wreck => vec![
                ([0.0, 1.2, 0.0], [4.0, 1.2, 1.6], [0.42, 0.44, 0.48]),
                ([5.5, 0.8, 0.8], [2.0, 0.8, 1.2], [0.38, 0.4, 0.44]),
                ([-2.0, 2.6, 0.0], [1.5, 0.4, 3.5], [0.35, 0.36, 0.4]),
                ([1.5, 2.9, 0.3], [0.8, 0.6, 0.8], [0.2, 0.5, 0.7]),
                ([3.0, 0.4, -2.5], [1.0, 0.4, 0.5], [0.55, 0.3, 0.2]),
            ],
            // Ruines : des pans de mur et deux colonnes
            DecorKind::Ruin => vec![
                ([0.0, 1.5, -3.0], [4.0, 1.5, 0.4], [0.62, 0.58, 0.5]),
                ([-3.6, 1.0, 0.0], [0.4, 1.0, 3.0], [0.6, 0.56, 0.48]),
                ([3.0, 2.5, 1.5], [0.5, 2.5, 0.5], [0.66, 0.62, 0.54]),
                ([1.0, 2.0, 2.5], [0.5, 2.0, 0.5], [0.66, 0.62, 0.54]),
                ([0.5, 0.3, 0.0], [3.5, 0.3, 3.0], [0.5, 0.47, 0.42]),
            ],
            // Monolithe : une dalle noire parfaite, haute
            DecorKind::Monolith => vec![([0.0, 4.5, 0.0], [0.5, 4.5, 2.0], [0.03, 0.03, 0.04])],
        }
    }

    /// Maillage de l'objet (cubes colorés), en voxels. Alpha des sommets (plantes) : hauteur dans la plante
    /// (0..0,5) + 0,5 pour le feuillage caduc (`decor.wgsl`).
    pub fn mesh(self) -> Mesh {
        let mut pos = Vec::new();
        let mut nor = Vec::new();
        let mut col = Vec::new();
        let mut idx = Vec::new();
        let cubes = self.cubes();
        let height = cubes.iter().map(|(c, h, _)| c[1] + h[1]).fold(0.0f32, f32::max).max(1e-3);
        const BARK: [f32; 3] = [0.36, 0.24, 0.14];
        for (c, h, color) in cubes {
            let (c, h) = (Vec3::from_array(c), Vec3::from_array(h));
            let leaf = self.deciduous() && color != BARK;
            for (n, u, v) in [
                (Vec3::X, Vec3::Y, Vec3::Z),
                (Vec3::NEG_X, Vec3::Z, Vec3::Y),
                (Vec3::Y, Vec3::Z, Vec3::X),
                (Vec3::NEG_Y, Vec3::X, Vec3::Z),
                (Vec3::Z, Vec3::X, Vec3::Y),
                (Vec3::NEG_Z, Vec3::Y, Vec3::X),
            ] {
                let base = pos.len() as u32;
                let center = c + n * h;
                let (du, dv) = (u * h, v * h);
                for corner in [center - du - dv, center + du - dv, center + du + dv, center - du + dv] {
                    pos.push(corner.to_array());
                    nor.push(n.to_array());
                    let alpha = if self.plant() { 0.5 * (corner.y / height).clamp(0.0, 0.999) + if leaf { 0.5 } else { 0.0 } } else { 1.0 };
                    col.push([color[0], color[1], color[2], alpha]);
                }
                // Face avant du côté de la normale
                let geo = (du * 2.0).cross(dv * 2.0);
                if geo.dot(n) >= 0.0 {
                    idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
                } else {
                    idx.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
                }
            }
        }
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, nor);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
        mesh.insert_indices(Indices::U32(idx));
        mesh
    }
}

/// Densité maximale (part des colonnes qui portent un objet).
const MAX_DENSITY: f32 = 0.1;

/// Décor d'un biome : (sorte, part des colonnes). Somme ≤ `MAX_DENSITY`.
pub fn decor_table(biome: Biome) -> &'static [(DecorKind, f32)] {
    use DecorKind::*;
    match biome {
        Biome::Forest => &[(Broadleaf, 0.072), (Bush, 0.022), (Rock, 0.005)],
        Biome::Taiga => &[(Conifer, 0.085), (Rock, 0.006)],
        Biome::Jungle => &[(JungleTree, 0.078), (Bush, 0.02)],
        Biome::Grassland => &[(Bush, 0.012), (Broadleaf, 0.004), (Rock, 0.004)],
        Biome::Steppe => &[(Bush, 0.008), (Rock, 0.006)],
        Biome::Savanna => &[(Acacia, 0.008), (Bush, 0.01)],
        Biome::Tundra => &[(Bush, 0.01), (Rock, 0.01)],
        Biome::Swamp => &[(Reed, 0.05), (Conifer, 0.006)],
        Biome::Desert => &[(Rock, 0.006), (Cactus, 0.002)],
        Biome::Beach => &[(Rock, 0.002)],
        Biome::Alpine => &[(Boulder, 0.008), (Rock, 0.012)],
        Biome::Regolith => &[(Rock, 0.008), (Boulder, 0.002)],
        Biome::BasaltField => &[(Boulder, 0.006), (Rock, 0.012)],
        Biome::SaltFlat => &[(SaltCrystal, 0.004)],
        Biome::RustPlain => &[(RustRock, 0.012)],
        Biome::IceSheet => &[(IceSpike, 0.003)],
        Biome::CrystalForest => &[(Crystal, 0.06), (Rock, 0.005)],
        Biome::SporePlain => &[(SporePod, 0.04)],
        Biome::FungalJungle => &[(GiantMushroom, 0.05), (SporePod, 0.02)],
        Biome::GlassDesert => &[(GlassShard, 0.01)],
        Biome::SulfurMarsh => &[(SulfurVent, 0.012)],
    }
}

/// Un objet posé : sorte et placement (repère de l'astre).
#[derive(Clone, Copy, Debug)]
pub struct DecorInstance {
    pub kind: DecorKind,
    pub transform: Transform,
    /// Arbre de pierre (trait « forêt pétrifiée », 0.14 D3).
    pub petrified: bool,
}

fn hash(a: u32, b: u32, c: u32) -> u32 {
    let mut x = a.wrapping_mul(0x9E37_79B1) ^ b.wrapping_mul(0x85EB_CA6B) ^ c.wrapping_mul(0xC2B2_AE35);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    x = x.wrapping_mul(0x297A_2D39);
    x ^ (x >> 15)
}

fn unit(h: u32) -> f32 {
    (h & 0xFF_FFFF) as f32 / 16_777_216.0
}

/// Décor d'une tuile (vide pour les tuiles lointaines, les mers, les planètes sans biomes).
pub fn tile_decor(terrain: &Terrain, key: TileKey) -> Vec<DecorInstance> {
    let layout = terrain.layout;
    let depth = key.depth as u32;
    if depth + terrain.decor_levels.max(1) <= layout.max_depth || !terrain.params.biomes.defined || terrain.params.gaseous {
        return Vec::new();
    }
    let mut out = cave_decor(terrain, key);
    let quantum = tile_quantum(layout, depth);
    // Lieux rares (0.14 D6) : l'épave, les ruines ou le monolithe dont le point tombe dans la tuile
    {
        let p = &terrain.params;
        let n_tiles = (TILE_CELLS as u32) << depth;
        let center = key.center_dir();
        for lm in crate::poi::landmarks_near(p, center, 1, &|d| terrain.vent_ground(d)) {
            let (face, s, t) = crate::terrain::dir_to_face(lm.dir);
            let (tx, ty) = ((((s + 1.0) * 0.5) as f64 * n_tiles as f64 / TILE_CELLS as f64).floor() as u32, (((t + 1.0) * 0.5) as f64 * n_tiles as f64 / TILE_CELLS as f64).floor() as u32);
            if face != key.face || tx != key.x || ty != key.y {
                continue;
            }
            let column = terrain.column(lm.dir, quantum);
            if column.kind.is_liquid() {
                continue;
            }
            let kind = match lm.kind {
                crate::poi::PoiKind::Epave => DecorKind::Wreck,
                crate::poi::PoiKind::Ruines => DecorKind::Ruin,
                _ => DecorKind::Monolith,
            };
            let yaw = (lm.seed % 628) as f32 / 100.0;
            let size = terrain.voxel() * if kind == DecorKind::Monolith { 2.0 } else { 3.0 };
            out.push(DecorInstance { kind, transform: Transform { translation: lm.dir * column.top, rotation: Quat::from_rotation_arc(Vec3::Y, lm.dir) * Quat::from_rotation_y(yaw), scale: Vec3::splat(size) }, petrified: false });
        }
    }
    let voxel = terrain.voxel();
    let lattice = (TILE_CELLS as u32) << depth;
    let seed = terrain.params.seed ^ ((key.face as u32) << 28);
    // Forêt pétrifiée sans vie (0.14 D3) : les arbres de pierre se placent d'après les biomes qu'aurait le monde
    // avec des plantes (le sol, lui, reste nu)
    let bp = terrain.params.biomes;
    let alive = (bp.petrified && !bp.flora).then(|| crate::planetgen::biome::BiomeField::new(crate::planetgen::biome::BiomeParams { flora: true, ..bp }));
    for cj in 0..TILE_CELLS as u32 {
        for ci in 0..TILE_CELLS as u32 {
            let (i, j) = (key.x * TILE_CELLS as u32 + ci, key.y * TILE_CELLS as u32 + cj);
            let h = hash(i ^ seed, j, depth);
            let roll = unit(h);
            if roll >= MAX_DENSITY {
                continue;
            }
            // Dans la cellule, un peu au hasard
            let s = -1.0 + 2.0 * (i as f32 + 0.15 + 0.7 * unit(h.rotate_left(7))) / lattice as f32;
            let t = -1.0 + 2.0 * (j as f32 + 0.15 + 0.7 * unit(h.rotate_left(14))) / lattice as f32;
            let dir = face_dir(key.face, s, t);
            let column = terrain.column(dir, quantum);
            if column.kind.is_liquid() || column.kind == crate::planet::VoxelType::Ice {
                continue;
            }
            // Rien au-dessus de l'entrée d'une grotte (le sol y manque)
            if depth == layout.max_depth && terrain.floor(dir, column.top + voxel).top < column.top - voxel {
                continue;
            }
            let Some(biome) = alive.as_ref().map_or_else(|| terrain.biome_at(dir), |f| terrain.biome_at_with(dir, f)) else { continue };
            let mut acc = 0.0;
            let Some(&(kind, _)) = decor_table(biome).iter().find(|(_, d)| {
                acc += d;
                roll < acc
            }) else {
                continue;
            };
            // Pas de plantes sans vie (un cactus dans le désert d'un monde mort)... sauf les arbres de pierre
            // d'une forêt pétrifiée (0.14 D3)
            let petrified = terrain.params.biomes.petrified && kind.tree();
            if kind.plant() && !terrain.params.biomes.flora && !petrified {
                continue;
            }
            // Forêt géante (0.14 X6) : un arbre sur quatre, six fois plus grand
            let giant = terrain.params.biomes.giant_trees && kind.tree();
            if giant && unit(h.rotate_left(27)) > 0.25 {
                continue;
            }
            let size = voxel * (0.8 + 0.5 * unit(h.rotate_left(21))) * if giant { 6.0 } else { 1.0 };
            let yaw = unit(h.rotate_left(3)) * std::f32::consts::TAU;
            let rotation = Quat::from_rotation_arc(Vec3::Y, dir) * Quat::from_rotation_y(yaw);
            out.push(DecorInstance {
                kind,
                transform: Transform { translation: dir * column.top, rotation, scale: Vec3::splat(size) },
                petrified,
            });
        }
    }
    out
}

/// Champignons lumineux sur le sol des salles de grottes dont le centre est sous la tuile
/// (tuiles du niveau le plus fin, astres avec de la vie).
fn cave_decor(terrain: &Terrain, key: TileKey) -> Vec<DecorInstance> {
    let mut out = Vec::new();
    let Some(caves) = terrain.caves.as_ref().filter(|c| c.style.glow) else { return out };
    if key.depth as u32 != terrain.layout.max_depth {
        return out;
    }
    let v = terrain.voxel();
    let n = (TILE_CELLS as i64) << terrain.layout.max_depth;
    let _ = n;
    let (x0, y0) = (key.x as i64 * TILE_CELLS as i64, key.y as i64 * TILE_CELLS as i64);
    let dirs = [key.center_dir()];
    for region in caves.for_tile(&dirs, &|d| terrain.surface_r(d), &terrain.cave_windows()) {
        let Some((rc, rr)) = region.room else { continue };
        let (face, i, j) = terrain.cell_of(rc.normalize());
        if face != key.face || !(x0..x0 + TILE_CELLS as i64).contains(&i) || !(y0..y0 + TILE_CELLS as i64).contains(&j) {
            continue;
        }
        let up = rc.normalize();
        let east = Vec3::Y.cross(up).normalize_or(Vec3::X);
        let north = up.cross(east);
        let h = hash(i as u32, j as u32, terrain.params.seed);
        for s in 0..8u32 {
            let hs = hash(h, s, 7);
            let a = unit(hs) * std::f32::consts::TAU;
            let d = unit(hs.rotate_left(9)) * rr * 0.7;
            let dir = (rc + (east * a.cos() + north * a.sin()) * d).normalize();
            let floor = terrain.floor(dir, rc.length());
            if floor.top >= rc.length() || floor.kind.is_liquid() || floor.top < rc.length() - rr * 1.2 {
                continue;
            }
            let rotation = Quat::from_rotation_arc(Vec3::Y, dir) * Quat::from_rotation_y(unit(hs.rotate_left(17)) * std::f32::consts::TAU);
            out.push(DecorInstance {
                kind: DecorKind::GlowShroom,
                transform: Transform { translation: dir * floor.top, rotation, scale: Vec3::splat(v * (0.6 + 0.6 * unit(hs.rotate_left(5)))) },
                petrified: false,
            });
        }
    }
    out
}

/// Maillages et matériaux partagés du décor.
#[derive(Resource)]
pub struct DecorAssets {
    meshes: HashMap<DecorKind, Handle<Mesh>>,
    matte: Handle<StandardMaterial>,
    glow: Handle<StandardMaterial>,
    /// Plantes (vent, saison), plantes qui luisent la nuit, arbres de pierre (0.14 D3).
    living: Handle<DecorMaterial>,
    living_glow: Handle<DecorMaterial>,
    stone: Handle<DecorMaterial>,
}

impl DecorAssets {
    /// Fait apparaître le décor comme enfants de la tuile `tile`.
    pub fn spawn(&self, commands: &mut Commands, tile: Entity, decor: &[DecorInstance]) {
        if decor.is_empty() {
            return;
        }
        let children: Vec<Entity> = decor
            .iter()
            .map(|d| {
                let mesh = Mesh3d(self.meshes[&d.kind].clone());
                if d.petrified {
                    commands.spawn((mesh, MeshMaterial3d(self.stone.clone()), d.transform)).id()
                } else if d.kind.night_glow() {
                    commands.spawn((mesh, MeshMaterial3d(self.living_glow.clone()), d.transform)).id()
                } else if d.kind.plant() {
                    commands.spawn((mesh, MeshMaterial3d(self.living.clone()), d.transform)).id()
                } else {
                    let material = if d.kind.glows() { self.glow.clone() } else { self.matte.clone() };
                    commands.spawn((mesh, MeshMaterial3d(material), d.transform)).id()
                }
            })
            .collect();
        commands.entity(tile).add_children(&children);
    }
}

pub struct DecorPlugin;

impl Plugin for DecorPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::load_internal_asset!(app, DECOR_SHADER, "decor.wgsl", Shader::from_wgsl);
        app.add_plugins(MaterialPlugin::<DecorMaterial>::default())
            .add_systems(Startup, setup_decor)
            .add_systems(Update, animate_decor.after(crate::surface::SurfaceControl));
    }
}

fn setup_decor(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, mut living_mats: ResMut<Assets<DecorMaterial>>) {
    let meshes = DecorKind::ALL.iter().map(|&k| (k, meshes.add(k.mesh()))).collect();
    let matte = materials.add(StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.9, ..default() });
    let glow = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        emissive: LinearRgba::new(0.6, 0.6, 0.8, 1.0),
        perceptual_roughness: 0.3,
        ..default()
    });
    let plant = |night: Vec4| DecorMaterial {
        base: StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.9, ..default() },
        extension: Sway { params: SwayParams { wind: Vec4::ZERO, tint: Vec4::ZERO, night } },
    };
    let living = living_mats.add(plant(Vec4::ZERO));
    let living_glow = living_mats.add(plant(Vec4::new(0.0, 1.0, 0.0, 0.0)));
    let stone = living_mats.add(plant(Vec4::new(0.0, 0.0, 1.0, 0.0)));
    commands.insert_resource(DecorAssets { meshes, matte, glow, living, living_glow, stone });
}

/// Couleur du feuillage caduc quand la saison se refroidit (roux) puis gèle (brun nu).
const AUTUMN: [f32; 3] = [0.78, 0.44, 0.12];
const BARE: [f32; 3] = [0.38, 0.3, 0.2];

/// Vent, saison et nuit des plantes (0.14 D3) : les paramètres de `decor.wgsl` chaque image, d'après la météo au
/// point du joueur (`WeatherNow`), le climat de la saison en cours et la lumière du jour.
#[allow(clippy::type_complexity)]
fn animate_decor(
    time: Res<Time>,
    surface: Res<crate::surface::Surface>,
    weather: Res<crate::weather::WeatherNow>,
    assets: Option<Res<DecorAssets>>,
    mut mats: ResMut<Assets<DecorMaterial>>,
    planets: Query<&Transform, With<PlanetRoot>>,
    moons: Query<&Transform, With<MoonRoot>>,
    planet_ids: Query<(&Transform, &PlanetId), With<PlanetRoot>>,
    moon_ids: Query<(&Transform, &MoonId), With<MoonRoot>>,
) {
    let _ = (&planets, &moons);
    let Some(assets) = assets else { return };
    let (Some(t), Some(p), Some(body)) = (surface.terrain(), surface.local_point(), surface.body()) else { return };
    let root = match body {
        TargetKind::Planet(id) => planet_ids.iter().find(|(_, p)| p.0 == id).map(|(t, _)| *t),
        TargetKind::Moon(pid, mi) => moon_ids.iter().find(|(_, m)| m.planet_idx == pid && m.moon_idx == mi).map(|(t, _)| *t),
        _ => None,
    };
    let Some(root) = root else { return };
    let voxel = t.voxel();
    let up = p.normalize_or(Vec3::Y);
    // Vent au point du joueur (m/s vers l'est et le nord) -> déplacement du sommet des plantes, repère monde
    let east = Vec3::Y.cross(up).normalize_or(Vec3::X);
    let north = up.cross(east);
    let s = &weather.sample;
    let local = east * s.east + north * s.north;
    let strength = (local.length() / 18.0).clamp(0.0, 1.3);
    let amp = voxel * (0.12 + 0.3 * strength) * (1.0 + 0.8 * s.gust);
    let wind_world = root.rotation * (local.normalize_or(east) * amp);
    let wind = wind_world.extend(time.elapsed_secs());
    // Saison : froid par rapport à la moyenne de l'année, à la latitude du joueur
    let climate = &t.params.climate;
    let lat = climate.lat_of(up);
    let now = climate.temperature(lat, 0.0, None);
    let mean = climate.at(crate::planetgen::climate::Season::default()).temperature(lat, 0.0, None);
    let autumn = ((mean - now - 2.0) / 8.0).clamp(0.0, 1.0);
    let frozen = ((2.0 - now) / 6.0).clamp(0.0, 1.0);
    let tint_rgb = [0, 1, 2].map(|i| AUTUMN[i] + (BARE[i] - AUTUMN[i]) * frozen);
    let tint = Vec4::new(tint_rgb[0], tint_rgb[1], tint_rgb[2], autumn.max(frozen));
    let night = 1.0 - surface.daylight();
    for (handle, glow, stone) in [(&assets.living, 0.0, 0.0), (&assets.living_glow, 1.0, 0.0), (&assets.stone, 0.0, 1.0)] {
        if let Some(m) = mats.get_mut(handle) {
            m.extension.params = SwayParams { wind: if stone > 0.5 { Vec4::new(0.0, 0.0, 0.0, wind.w) } else { wind }, tint: if stone > 0.5 { Vec4::ZERO } else { tint }, night: Vec4::new(night, glow, stone, s.gust) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planetgen::biome::BiomeParams;
    use crate::terrain::BodyParams;

    #[test]
    fn tables_stay_under_the_maximum_density() {
        for b in Biome::ALL {
            let total: f32 = decor_table(b).iter().map(|(_, d)| d).sum();
            assert!(total <= MAX_DENSITY + 1e-6, "{b:?} {total}");
            // Plantes seulement dans les biomes vivants
            for (k, _) in decor_table(b) {
                if k.plant() {
                    assert!(b.vegetated() || *k == DecorKind::Cactus, "{b:?} {k:?}");
                }
            }
        }
    }

    #[test]
    fn decor_meshes_are_closed_boxes_facing_out() {
        for k in DecorKind::ALL {
            let mesh = k.mesh();
            assert_eq!(mesh.count_vertices(), k.cubes().len() * 24);
        }
    }

    fn earth_like() -> BodyParams {
        let settings = crate::settings::GameSettings::default();
        // Une planète générée avec des plantes, sinon une planète faite à la main reverdie
        for sys in settings.systems.dense().iter().take(3000) {
            for p in sys.planets_uncached().iter() {
                if p.biomes.flora && !p.gaseous() && p.hydrology.ocean_fraction < 0.8 {
                    return BodyParams::planet(p);
                }
            }
        }
        panic!("aucune planete avec des plantes");
    }

    #[test]
    fn glowing_mushrooms_grow_in_cave_rooms_where_there_is_life() {
        let t = Terrain::new(earth_like());
        let caves = t.caves.clone().expect("grottes");
        assert!(caves.style.glow);
        let surf = |d: Vec3| t.surface_r(d);
        let mut shrooms = 0;
        'search: for x in -12..12 {
            for z in -12..12 {
                let d = Vec3::new(x as f32 * 0.03, 1.0, z as f32 * 0.03).normalize();
                for depth in [1.0, 2.0, 4.0] {
                    let p = d * (surf(d) - depth * caves.size);
                    let Some(r) = caves.region(caves.key_of(p), &surf) else { continue };
                    let Some((rc, _)) = r.room else { continue };
                    let (face, i, j) = t.cell_of(rc.normalize());
                    let key = TileKey { face, depth: t.layout.max_depth as u8, x: (i / TILE_CELLS as i64) as u32, y: (j / TILE_CELLS as i64) as u32 };
                    shrooms += tile_decor(&t, key).iter().filter(|d| d.kind == DecorKind::GlowShroom).count();
                    if shrooms > 3 {
                        break 'search;
                    }
                }
            }
        }
        assert!(shrooms > 3, "{shrooms} champignons");
    }

    #[test]
    fn close_tiles_get_decor_on_land_far_tiles_none() {
        let p = earth_like();
        let t = Terrain::new(p);
        let max = t.layout.max_depth;
        let mut total = 0;
        let mut plants = 0;
        // Quelques tuiles du niveau le plus fin, partout sur la planète
        for face in 0..6u8 {
            for k in 0..8u32 {
                let n = 1u32 << max;
                let key = TileKey { face, depth: max as u8, x: (k * 7919) % n, y: (k * 104_729) % n };
                let decor = tile_decor(&t, key);
                for d in &decor {
                    total += 1;
                    plants += d.kind.plant() as usize;
                    // Posé sur le sol, jamais dans l'eau
                    let dir = d.transform.translation.normalize();
                    let ground = t.column(dir, tile_quantum(t.layout, max)).top;
                    if d.kind == DecorKind::GlowShroom {
                        // Dans une grotte : sous la surface, posé sur le sol de la salle
                        assert!(d.transform.translation.length() <= ground + 0.01);
                    } else {
                        assert!((d.transform.translation.length() - ground).abs() < 1.0);
                    }
                }
                assert!(decor.len() <= (TILE_CELLS * TILE_CELLS) / 8);
            }
        }
        assert!(total > 20 && plants > 0, "{total} objets, {plants} plantes");
        // Tuile lointaine : rien
        assert!(tile_decor(&t, TileKey::root(0)).is_empty());
        // Sans biomes (planète faite à la main) : rien
        let plain = Terrain::new(BodyParams { biomes: BiomeParams::default(), ..p });
        assert!(tile_decor(&plain, TileKey { face: 0, depth: max as u8, x: 0, y: 0 }).is_empty());
    }

    /// 0.14 D3 : l'alpha des sommets d'une plante code sa hauteur (0..0,5) et le feuillage caduc (+0,5) ; les
    /// rochers restent opaques (1).
    #[test]
    fn plant_vertices_code_their_height_and_their_leaves() {
        use bevy::render::mesh::VertexAttributeValues;
        let alphas = |k: DecorKind| -> Vec<f32> {
            match k.mesh().attribute(Mesh::ATTRIBUTE_COLOR) {
                Some(VertexAttributeValues::Float32x4(c)) => c.iter().map(|c| c[3]).collect(),
                _ => panic!("couleurs"),
            }
        };
        let tree = alphas(DecorKind::Broadleaf);
        assert!(tree.iter().any(|a| *a < 0.5), "le tronc est sous 0,5");
        assert!(tree.iter().any(|a| *a >= 0.5), "le feuillage est au-dessus de 0,5");
        let top = tree.iter().cloned().fold(0.0f32, f32::max);
        assert!(top > 0.95 && top < 1.0, "{top}");
        let pine = alphas(DecorKind::Conifer);
        assert!(pine.iter().all(|a| *a < 0.5), "un conifère n'a pas de feuillage caduc");
        assert!(alphas(DecorKind::Rock).iter().all(|a| *a == 1.0));
    }

    /// 0.14 D3 : le trait « forêt pétrifiée » plante des arbres de pierre même sans vie, et seulement des arbres.
    #[test]
    fn petrified_worlds_grow_stone_trees_without_life() {
        // Une planète tempérée aux biomes terrestres (pas un monde alien de cristaux et de spores)
        let settings = crate::settings::GameSettings::default();
        let p = settings
            .systems
            .dense()
            .iter()
            .take(3000)
            .flat_map(|sys| sys.planets_uncached().into_owned())
            .find(|p| p.biomes.flora && !p.biomes.alien && !p.gaseous() && p.hydrology.ocean_fraction < 0.8 && (5.0..25.0).contains(&p.climate().mean_c) && p.geology.volcanism < 0.3)
            .map(|p| BodyParams::planet(&p))
            .expect("une planète tempérée");
        let t = Terrain::new(BodyParams { biomes: BiomeParams { flora: false, petrified: true, ..p.biomes }, ..p });
        let max = t.layout.max_depth;
        let mut trees = 0;
        let mut others = 0;
        for face in 0..6u8 {
            for k in 0..24u32 {
                let n = 1u32 << max;
                let key = TileKey { face, depth: max as u8, x: (k * 7919) % n, y: (k * 104_729) % n };
                for d in tile_decor(&t, key) {
                    if d.kind.plant() {
                        assert!(d.petrified && d.kind.tree(), "{:?}", d.kind);
                        trees += 1;
                    } else {
                        assert!(!d.petrified);
                        others += 1;
                    }
                }
            }
        }
        assert!(trees > 0 && others > 0, "{trees} arbres de pierre, {others} rochers");
    }
}

#[cfg(test)]
mod bench {
    use super::*;
    use crate::terrain::{build_tile_mesh_with, select_tiles, BodyParams};

    /// Coût du décor (`cargo test --release bench_decor -- --ignored --nocapture`).
    #[test]
    #[ignore]
    fn bench_decor() {
        let settings = crate::settings::GameSettings::default();
        let mut bodies = Vec::new();
        for sys in settings.systems.dense().iter().take(3000) {
            for p in sys.planets_uncached().iter() {
                if p.biomes.flora && !p.gaseous() && bodies.len() < 6 {
                    bodies.push(BodyParams::planet(p));
                }
            }
        }
        let (mut tiles, mut objects, mut mesh_ms, mut decor_ms, mut shown) = (0, 0, 0.0, 0.0, 0);
        for p in &bodies {
            let t = Terrain::new(*p);
            let dir = Vec3::new(0.2, 0.3, 0.9).normalize();
            let mut keys = Vec::new();
            select_tiles(t.layout, t.ground(dir).top, dir * (t.ground(dir).top + 20.0), &mut keys);
            for key in &keys {
                let a = std::time::Instant::now();
                std::hint::black_box(build_tile_mesh_with(&t, *key));
                let b = std::time::Instant::now();
                let d = tile_decor(&t, *key);
                let c = std::time::Instant::now();
                mesh_ms += (b - a).as_secs_f64() * 1000.0;
                decor_ms += (c - b).as_secs_f64() * 1000.0;
                tiles += 1;
                objects += d.len();
                shown += d.len();
            }
        }
        println!(
            "DECOR {} planetes, {tiles} tuiles visibles : tuile {:.2} ms, decor {:.3} ms par tuile ; {} objets en tout ({:.0} par planete visitee)",
            bodies.len(), mesh_ms / tiles as f64, decor_ms / tiles as f64, objects, shown as f64 / bodies.len() as f64
        );
    }
}
