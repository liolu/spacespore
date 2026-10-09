//! Terrain voxel des planètes et des lunes, utilisé pour les atterrissages.
//!
//! Chaque corps est une sphère-cube : 6 faces, chacune découpée en un quadtree de tuiles de
//! 32 × 32 colonnes. Plus la caméra est proche d'une tuile, plus elle est subdivisée, jusqu'au
//! voxel le plus fin (une colonne d'environ 5 à 11 unités). La hauteur d'une colonne ne dépend que
//! de sa direction : le maillage affiché et le sol sous les pieds du joueur sont donc identiques.
//!
//! Près du joueur (tuiles du niveau le plus fin), le terrain est en voxels 3D (0.11, B1) : une
//! cellule est pleine selon une seule fonction (`Terrain::kind_at`) = champ de hauteur + formes 3D
//! (surplomb de test) + deltas (`voxel.rs`, minage en 0.14). Le champ de hauteur lointain en est la
//! vue de dessus (`Terrain::column`) : pas de saut entre loin et près.
//!
//! Tout ce fichier est du calcul pur (aucune ressource Bevy), ce qui permet de le tester.

use bevy::math::Vec3;
use bevy::render::mesh::{Indices, Mesh, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use noise::{Fbm, NoiseFn, Perlin};
use std::f32::consts::{FRAC_PI_2, FRAC_PI_4};
use std::sync::Arc;

use crate::planet::VoxelType;
use crate::planetgen::biome::{Biome, BiomeField, BiomeParams};
use crate::planetgen::climate::{sea_material, Climate};
use crate::planetgen::geology::{moon_relief, Relief, ReliefField, ReliefSample};
use crate::planetgen::hydrology::Hydro;
use crate::settings::{MoonConfig, PlanetConfig};
use crate::caves::{eval_pieces, ore_chance, CaveCell, CaveStyle, Caves, Piece, Region};
#[cfg(test)]
use crate::caves::Shape;
use crate::rocks::Rocks;
use crate::voxel::{BodyVoxels, Cell, BLOCK};

/// Colonnes par côté d'une tuile.
pub const TILE_CELLS: usize = 32;

/// Période (colonnes) des coordonnées de vague des mailles d'eau : les nombres d'onde des vagues
/// sont des multiples entiers de 2π / WAVE_PERIOD (`water.wgsl`).
pub const WAVE_PERIOD: i64 = 4096;

/// Taille maximale d'un voxel au niveau le plus fin.
pub const MAX_VOXEL: f32 = 11.0;

/// Une tuile est subdivisée tant que la caméra est plus proche que ce multiple de sa taille.
pub const SPLIT_FACTOR: f32 = 1.8;


// ─────────────────────────────────────────────────────────────────────────
//  Paramètres d'un corps
// ─────────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug)]
pub struct BodyParams {
    /// Lune : pas d'océan ni d'atmosphère, sol gris.
    pub airless: bool,
    pub atmosphere: bool,
    pub radius: f32,
    pub sea_level: f32,
    pub terrain_height: f32,
    pub seed: u32,
    pub noise_scale: f32,
    pub detail_scale: f32,
    pub temperature: f32,
    /// Gravité de surface (g) : la marche et le saut en dépendent.
    pub gravity: f32,
    /// Géante gazeuse ou neptunienne : pas de sol, on y vole jusqu'au cœur (`GAS_CORE`).
    pub gaseous: bool,
    /// Température selon la latitude et l'altitude (phase 3) : neige, déserts, mers gelées.
    pub climate: Climate,
    /// Ciel de jour, coucher de soleil et brume de l'horizon (sRGB), pression au sol (bar).
    pub sky: [f32; 3],
    pub sunset: [f32; 3],
    pub haze: [f32; 3],
    pub pressure: f32,
    /// Liquide des mers et glaces possibles (phase 4).
    pub hydro: Hydro,
    /// Formes du relief issues de la géologie (phase 5).
    pub relief: Relief,
    /// Sols et biomes (phase 6).
    pub biomes: BiomeParams,
    /// Astéroïde (C1) : sa forme remplace le relief (même fonction que son maillage lointain).
    pub asteroid: Option<crate::asteroids::AsteroidShape>,
    /// Marées (C4) : le niveau de la mer monte et descend (mis à jour pendant un séjour).
    pub tide: Tide,
    /// Géologie active (0.13 T5) : geysers, fumerolles, lave, séismes (`geoactive.rs`).
    pub geo: crate::geoactive::GeoActivity,
    /// Couleur des flammes de rentrée selon l'air (0.13 P3 : azote orange, CO2 rose, méthane vert-bleu...).
    pub plasma: [f32; 3],
}

/// Marées (C4) : un renflement de la mer vers chaque astre qui la tire (et à l'opposé), en
/// unités, dans le repère fixe de l'astre. Figées au moment de leur calcul : `surface.rs` les
/// recalcule quand le niveau change d'un voxel sous le joueur.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Tide {
    pub dirs: [Vec3; 4],
    pub amps: [f32; 4],
}

impl Tide {
    /// Hauteur de la mer (unités) dans la direction `dir` : haute vers l'astre et à l'opposé, basse
    /// à 90° (marée d'équilibre, polynôme de Legendre P2).
    pub fn at(&self, dir: Vec3) -> f32 {
        self.dirs.iter().zip(self.amps).map(|(d, a)| {
            let c = dir.dot(*d);
            a * (1.5 * c * c - 0.5)
        }).sum()
    }

    pub fn is_calm(&self) -> bool {
        self.amps.iter().all(|a| *a == 0.0)
    }
}

/// Ciel d'une planète faite à la main (sans atmosphère calculée) : celui de la Terre.
pub const EARTH_SKY: [f32; 3] = [0.36, 0.58, 0.92];
pub const EARTH_SUNSET: [f32; 3] = [1.0, 0.5, 0.2];

/// Une géante gazeuse n'a pas de sol : le vol s'arrête à cette fraction de son rayon (le cœur).
pub const GAS_CORE: f32 = 0.3;

impl BodyParams {
    pub fn planet(p: &PlanetConfig) -> Self {
        Self {
            airless: false,
            atmosphere: p.atmosphere,
            radius: p.radius,
            sea_level: p.sea_level,
            terrain_height: p.terrain_height,
            seed: p.seed,
            noise_scale: p.noise_scale,
            detail_scale: p.detail_scale,
            temperature: p.temperature(),
            gravity: p.gravity_g,
            gaseous: p.gaseous(),
            climate: p.climate(),
            sky: if p.air.present() { p.air.sky } else { EARTH_SKY },
            sunset: if p.air.present() { p.air.sunset } else { EARTH_SUNSET },
            haze: if p.air.present() { p.air.haze } else { EARTH_SKY },
            pressure: if p.air.present() { p.air.pressure_bar } else if p.atmosphere { 1.0 } else { 0.0 },
            hydro: if p.gaseous() { Hydro::DRY } else { p.hydrology.hydro },
            relief: p.geology.relief,
            biomes: p.biomes,
            asteroid: None,
            tide: Default::default(),
            geo: crate::geoactive::GeoActivity {
                volcanism: p.geology.volcanism,
                quakes: p.geology.quakes,
                cryo: p.hydrology.subsurface_ocean && p.geology.activity > 0.05,
            },
            plasma: if p.air.present() { crate::approche::plasma_color(&p.air.gases) } else { crate::approche::PLASMA_DEFAULT },
        }
    }

    pub fn moon(m: &MoonConfig, parent: &PlanetConfig) -> Self {
        // Lune générée (phase 9) : un monde comme une planète (air, mers, biomes possibles)
        if m.generated() {
            return Self::planet(&m.as_planet(parent));
        }
        Self {
            airless: true,
            atmosphere: false,
            radius: m.radius,
            sea_level: 0.5,
            terrain_height: m.radius * 0.045,
            seed: m.seed,
            noise_scale: 2.0,
            detail_scale: 4.0,
            temperature: m.climate.map_or_else(|| parent.temperature(), |c| c.mean_c),
            gravity: m.gravity_g,
            gaseous: false,
            climate: m.climate.unwrap_or_else(|| Climate::from_mean(parent.temperature(), false)),
            sky: [0.0; 3],
            sunset: [0.0; 3],
            haze: [0.0; 3],
            pressure: 0.0,
            hydro: Hydro::DRY,
            relief: m.relief.unwrap_or_else(|| moon_relief(m.seed)),
            biomes: BiomeParams::default(),
            asteroid: None,
            tide: Default::default(),
            geo: Default::default(),
            plasma: crate::approche::PLASMA_DEFAULT,
        }
    }

    pub fn layout(&self) -> Layout {
        layout_for(self.radius)
    }
}

/// Finesse du quadtree d'un corps.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    /// Profondeur du niveau le plus fin (les tuiles racines sont à la profondeur 0).
    pub max_depth: u32,
    /// Taille d'un voxel au niveau le plus fin (unités).
    pub voxel: f32,
}

/// Échelle du sol (0.13, règle 14) : le voxel est `GROUND_SCALE` fois plus petit qu'en 0.12 (la
/// planète 16 fois plus grande en voxels, ses rayons en unités inchangés : règle 15). Changer
/// l'échelle = changer ce nombre (décision Q1 après l'étude E1, `roadmaps/fait/RAPPORT-echelle-E1.md`).
pub const GROUND_SCALE: u32 = 16;

/// Échelle en cours : `GROUND_SCALE`, ou un autre `k` pour les tests (`/echelle k`,
/// `SPACESPORE_SCALE`, jamais sauvegardé, au prochain atterrissage).
static VOXEL_SCALE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(GROUND_SCALE);

pub fn voxel_scale() -> u32 {
    VOXEL_SCALE.load(std::sync::atomic::Ordering::Relaxed)
}

pub fn set_voxel_scale(k: u32) {
    VOXEL_SCALE.store(k.clamp(1, 64), std::sync::atomic::Ordering::Relaxed);
}

/// Profondeur maximale du quadtree (k = 64 sur la plus grande planète : 13).
const MAX_TREE_DEPTH: u32 = 18;

/// Relief en voxels d'un astre (partagé avec le maillage vu de l'espace).
pub fn landforms_of(p: &BodyParams) -> crate::planetgen::landforms::Landforms {
    let wet = p.atmosphere && !p.airless && p.hydro.liquid == crate::planetgen::hydrology::Liquid::Water;
    crate::planetgen::landforms::Landforms::new(p.seed, p.radius, layout_for(p.radius).voxel, &p.relief, p.airless || !p.atmosphere, wet)
}

pub fn layout_for(radius: f32) -> Layout {
    layout_scaled(radius, voxel_scale())
}

/// Le quadtree d'un corps de rayon `radius` avec un voxel `k` fois plus petit.
pub fn layout_scaled(radius: f32, k: u32) -> Layout {
    let arc = FRAC_PI_2 * radius;
    let max_voxel = MAX_VOXEL / k.max(1) as f32;
    let mut depth = 0u32;
    while depth < MAX_TREE_DEPTH && arc / ((TILE_CELLS << depth) as f32) > max_voxel {
        depth += 1;
    }
    Layout { max_depth: depth, voxel: arc / (TILE_CELLS << depth) as f32 }
}

// ─────────────────────────────────────────────────────────────────────────
//  Faces de la sphère-cube (projection équiangulaire : cellules presque uniformes)
// ─────────────────────────────────────────────────────────────────────────

/// Direction (unitaire) du point (s, t) ∈ [-1, 1]² de la face `face` (0..6).
/// Accepte des valeurs un peu hors de [-1, 1] (colonnes voisines au bord d'une face).
pub fn face_dir(face: u8, s: f32, t: f32) -> Vec3 {
    let a = (s * FRAC_PI_4).tan();
    let b = (t * FRAC_PI_4).tan();
    let v = match face {
        0 => Vec3::new(1.0, b, -a),
        1 => Vec3::new(-1.0, b, a),
        2 => Vec3::new(a, 1.0, -b),
        3 => Vec3::new(a, -1.0, b),
        4 => Vec3::new(a, b, 1.0),
        _ => Vec3::new(-a, b, -1.0),
    };
    v.normalize()
}

/// Inverse de `face_dir` : (face, s, t).
pub fn dir_to_face(dir: Vec3) -> (u8, f32, f32) {
    let (ax, ay, az) = (dir.x.abs(), dir.y.abs(), dir.z.abs());
    let (face, a, b) = if ax >= ay && ax >= az {
        if dir.x > 0.0 { (0u8, -dir.z / ax, dir.y / ax) } else { (1, dir.z / ax, dir.y / ax) }
    } else if ay >= az {
        if dir.y > 0.0 { (2, dir.x / ay, -dir.z / ay) } else { (3, dir.x / ay, dir.z / ay) }
    } else if dir.z > 0.0 {
        (4, dir.x / az, dir.y / az)
    } else {
        (5, -dir.x / az, dir.y / az)
    };
    (face, a.atan() / FRAC_PI_4, b.atan() / FRAC_PI_4)
}

// ─────────────────────────────────────────────────────────────────────────
//  Tuiles du quadtree
// ─────────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TileKey {
    pub face: u8,
    pub depth: u8,
    pub x: u32,
    pub y: u32,
}

impl TileKey {
    pub fn root(face: u8) -> Self {
        Self { face, depth: 0, x: 0, y: 0 }
    }

    pub fn parent(self) -> Option<Self> {
        (self.depth > 0).then(|| Self { face: self.face, depth: self.depth - 1, x: self.x >> 1, y: self.y >> 1 })
    }

    pub fn children(self) -> [Self; 4] {
        let d = self.depth + 1;
        let (x, y) = (self.x << 1, self.y << 1);
        [
            Self { face: self.face, depth: d, x, y },
            Self { face: self.face, depth: d, x: x + 1, y },
            Self { face: self.face, depth: d, x, y: y + 1 },
            Self { face: self.face, depth: d, x: x + 1, y: y + 1 },
        ]
    }

    /// `self` contient strictement `other`.
    #[cfg(test)]
    pub fn is_ancestor_of(self, other: Self) -> bool {
        if self.face != other.face || other.depth <= self.depth {
            return false;
        }
        let shift = other.depth - self.depth;
        (other.x >> shift) == self.x && (other.y >> shift) == self.y
    }

    pub fn center_dir(self) -> Vec3 {
        let n = (1u32 << self.depth) as f32;
        face_dir(self.face, -1.0 + 2.0 * (self.x as f32 + 0.5) / n, -1.0 + 2.0 * (self.y as f32 + 0.5) / n)
    }

    /// Longueur d'arc d'un côté de la tuile.
    pub fn arc(self, radius: f32) -> f32 {
        FRAC_PI_2 * radius / (1u32 << self.depth) as f32
    }
}

/// Tuiles (feuilles du quadtree) à afficher pour une caméra à `cam_local` (repère du corps) ;
/// `ground_r` = rayon du sol sous la caméra : les distances se mesurent au sol, pas à la sphère de
/// base (à la nouvelle échelle, un plateau à 60 voxels au-dessus n'aurait jamais de tuiles fines).
#[cfg(test)]
pub fn select_tiles(layout: Layout, ground_r: f32, cam_local: Vec3, out: &mut Vec<TileKey>) {
    select_tiles_with(layout, ground_r, cam_local, SPLIT_FACTOR, f32::INFINITY, out);
}

/// Comme `select_tiles`, avec un facteur de découpe (`split` : distance de détail, 0.13 T4) et
/// la hauteur du relief (`relief`, unités) : une tuile tout entière derrière l'horizon (cachée par
/// la courbure, même ses sommets) n'est pas découpée.
pub fn select_tiles_with(layout: Layout, ground_r: f32, cam_local: Vec3, split: f32, relief: f32, out: &mut Vec<TileKey>) {
    // Distance au-delà de laquelle rien n'est visible : horizon de la caméra (sur la sphère du
    // sol le plus bas) plus l'horizon des plus hauts sommets
    let low = (ground_r - relief).max(ground_r * 0.5);
    let cam_r = cam_local.length();
    let hidden = if relief.is_finite() {
        (cam_r * cam_r - low * low).max(0.0).sqrt() + ((low + 2.0 * relief).powi(2) - low * low).max(0.0).sqrt()
    } else {
        f32::INFINITY
    };
    fn visit(key: TileKey, layout: Layout, radius: f32, cam: Vec3, split: f32, hidden: f32, out: &mut Vec<TileKey>) {
        if (key.depth as u32) < layout.max_depth {
            let dist = (cam - key.center_dir() * radius).length();
            let arc = key.arc(radius);
            if dist < split * arc && dist - 0.75 * arc < hidden {
                for child in key.children() {
                    visit(child, layout, radius, cam, split, hidden, out);
                }
                return;
            }
        }
        out.push(key);
    }
    for face in 0..6 {
        visit(TileKey::root(face), layout, ground_r, cam_local, split, hidden, out);
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  Champ de hauteur
// ─────────────────────────────────────────────────────────────────────────

/// Une colonne de voxels : hauteur de sa face supérieure et apparence.
#[derive(Clone, Copy, Debug)]
pub struct Column {
    pub dir: Vec3,
    /// Rayon de la face supérieure.
    pub top: f32,
    pub kind: VoxelType,
    pub color: [f32; 4],
    /// Rayon du sol avant l'arrondi aux couches (pente du sol : couleurs, 0.13 T3).
    pub raw: f32,
    /// Fond sous un liquide transparent (0.13 O1) : rayon, matière et couleur du dessus du fond
    /// marin. Sans liquide transparent, ce sont ceux de la colonne.
    pub bed: f32,
    pub bed_kind: VoxelType,
    pub bed_color: [f32; 4],
}

impl Column {
    /// Colonne sans liquide transparent : le fond est le dessus.
    pub fn solid(dir: Vec3, top: f32, kind: VoxelType, color: [f32; 4], raw: f32) -> Self {
        Self { dir, top, kind, color, raw, bed: top, bed_kind: kind, bed_color: color }
    }

    /// Mer, lac... d'un liquide transparent : le dessus est la surface de l'eau.
    pub fn is_sea(&self) -> bool {
        self.kind.is_clear_liquid()
    }

    /// Profondeur de l'eau (unités) : 0 sur la terre ferme.
    pub fn depth(&self) -> f32 {
        if self.is_sea() { (self.top - self.bed).max(0.0) } else { 0.0 }
    }

    /// Le dessus solide (le fond marin sous l'eau) : matière, hauteur, couleur du dessus (avec la
    /// teinte de pente de la tuile pour la terre ferme).
    pub fn ground_view(&self) -> Column {
        if self.is_sea() {
            Column { top: self.bed, kind: self.bed_kind, color: self.bed_color, ..*self }
        } else {
            *self
        }
    }
}

/// Couleur d'un dessus immergé : l'alpha (0,5) sert de drapeau aux sommets (le matériau du sol
/// y met la lumière du fond marin : absorption et caustiques, `caustics.wgsl`) ; ailleurs 1.
pub fn submerged(color: [f32; 4], under: bool) -> [f32; 4] {
    [color[0], color[1], color[2], if under { SUBMERGED_ALPHA } else { 1.0 }]
}

/// Alpha des sommets sous l'eau (voir `submerged`).
pub const SUBMERGED_ALPHA: f32 = 0.5;

/// Teinte de la roche nue d'un sol de couleur `color` : la pierre, un peu de la couleur du lieu.
pub fn rock_of(color: [f32; 4]) -> [f32; 4] {
    let s = VoxelType::Stone.color();
    [s[0] * 0.7 + color[0] * 0.3, s[1] * 0.7 + color[1] * 0.3, s[2] * 0.7 + color[2] * 0.3, 1.0]
}

fn mix(a: [f32; 4], b: [f32; 4], f: f32) -> [f32; 4] {
    [a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f, a[2] + (b[2] - a[2]) * f, 1.0]
}

/// Bruit de valeurs (-1 à 1) lissé, bien moins cher que Perlin : taches de couleur du sol (T3).
fn value_noise(p: [f64; 3], seed: u32) -> f32 {
    let (fx, fy, fz) = (p[0].floor(), p[1].floor(), p[2].floor());
    let (x, y, z) = (fx as i64, fy as i64, fz as i64);
    let s = |t: f64| -> f32 {
        let t = t as f32;
        t * t * (3.0 - 2.0 * t)
    };
    let (u, v, w) = (s(p[0] - fx), s(p[1] - fy), s(p[2] - fz));
    let h = |i: i64, j: i64, k: i64| -> f32 {
        let mut z = (seed as u64) ^ (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (j as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ (k as u64).wrapping_mul(0x1656_67B1_9E37_79F9);
        z = (z ^ (z >> 31)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        ((z ^ (z >> 29)) >> 40) as f32 / (1u64 << 23) as f32 - 1.0
    };
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let plane = |k: i64| lerp(lerp(h(x, y, k), h(x + 1, y, k), u), lerp(h(x, y + 1, k), h(x + 1, y + 1, k), u), v);
    lerp(plane(z), plane(z + 1), w)
}

/// Couleur de la mousse au pied des parois humides.
const MOSS: [f32; 4] = [0.16, 0.34, 0.12, 1.0];

/// Couleur du dessus d'une colonne selon la pente du sol (`slope` = dénivelé / distance) et la
/// paroi qui la domine (`wall`, voxels) (0.13 T3) : roche nue sur les pentes fortes (la neige
/// tient plus longtemps, le sable des plages seulement en pente douce), mousse au pied des parois
/// sur un monde humide. Mêmes règles pour les tuiles et le maillage vu de l'espace (règle 16).
pub fn ground_tint(kind: VoxelType, color: [f32; 4], slope: f32, wall: f32, wet: bool) -> [f32; 4] {
    if kind.is_liquid() {
        return color;
    }
    let (a, b) = match kind {
        VoxelType::Snow | VoxelType::Ice => (1.4, 1.8),
        VoxelType::Sand => (0.35, 0.6),
        _ => (0.7, 1.0),
    };
    let f = ((slope - a) / (b - a)).clamp(0.0, 1.0);
    let f = f * f * (3.0 - 2.0 * f);
    let c = mix(color, rock_of(color), f);
    let green = matches!(kind, VoxelType::Grass | VoxelType::Forest | VoxelType::Jungle | VoxelType::Taiga | VoxelType::Swamp | VoxelType::Tundra);
    if wet && green && wall >= 3.0 && f < 0.5 {
        return mix(c, MOSS, 0.55);
    }
    c
}

/// Couleur d'une paroi à la couche `k` : strates de 2 à 4 voxels, plus ou moins claires, ocre
/// sur les mondes qui ont de l'air (0.13 T3).
pub fn strata_color(color: [f32; 4], k: i32, seed: u32, warm: bool) -> [f32; 4] {
    let h = |x: i64| -> f32 {
        let mut z = (x as u64 ^ ((seed as u64) << 32)).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        z = (z ^ (z >> 29)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        ((z ^ (z >> 32)) >> 40) as f32 / (1u64 << 24) as f32
    };
    // Bandes d'épaisseur variable : la limite suivante tombe 2 à 4 couches plus haut
    let band = (k as i64).div_euclid(3);
    let cut = band * 3 + (h(band * 7 + 1) * 3.0) as i64;
    let band = if (k as i64) < cut { band * 2 } else { band * 2 + 1 };
    let rock = rock_of(color);
    let lum = 0.8 + 0.32 * h(band);
    let c = [rock[0] * lum, rock[1] * lum, rock[2] * lum, 1.0];
    if warm {
        mix(c, [0.70 * lum, 0.50 * lum, 0.36 * lum, 1.0], 0.3 * h(band + 99))
    } else {
        c
    }
}

/// Pente du sol et hauteur de la paroi voisine (voxels) de la colonne `c`, d'après ses quatre
/// voisines (est, ouest, nord, sud).
fn slope_of(c: &Column, e: &Column, w: &Column, n: &Column, s: &Column, voxel: f32) -> (f32, f32) {
    let dx = ((e.dir - w.dir).length() * c.raw).max(1e-3);
    let dy = ((n.dir - s.dir).length() * c.raw).max(1e-3);
    let slope = ((e.raw - w.raw) / dx).hypot((n.raw - s.raw) / dy);
    let wall = (e.top.max(w.top).max(n.top).max(s.top) - c.top) / voxel;
    (slope, wall)
}

pub struct Terrain {
    pub params: BodyParams,
    pub layout: Layout,
    continent: Fbm<Perlin>,
    detail: Fbm<Perlin>,
    /// Relief en voxels (0.13 T1) : collines, massifs, bosses, vallées (`planetgen::landforms`).
    forms: crate::planetgen::landforms::Landforms,
    color: Perlin,
    relief: ReliefField,
    biomes: BiomeField,
    /// Surplomb de test (B1), s'il a trouvé une terre ferme où se poser.
    pub overhang: Option<Overhang>,
    /// Cellules modifiées (minage, 0.14) : vides pour l'instant.
    voxels: Option<Arc<BodyVoxels>>,
    /// Grottes (B2), partagées entre les tuiles d'un même astre (cache des régions).
    pub caves: Option<Arc<Caves>>,
    /// Arches et cheminées de fée (B3), partagées de même.
    pub rocks: Option<Arc<Rocks>>,
    /// Plus petit cratère calculé (rayon angulaire, bits d'un f32) : plus grand pendant la
    /// construction d'une tuile lointaine (chaque tâche a son propre terrain).
    min_crater: std::sync::atomic::AtomicU32,
    /// Grottes maillées dans les tuiles (0.13 E2) : jusqu'à `NEAR_CAVE_VOXELS` sous la surface, plus
    /// une tranche autour du joueur quand il descend plus bas (`with_cave_window`, unités sous la
    /// surface). Les grottes vont jusqu'à `caves::MAX_DEPTH` (2 000 unités, décision Q2).
    pub cave_window: Option<(f32, f32)>,
    /// Niveaux de tuiles (les plus fins) qui portent du décor.
    pub decor_levels: u32,
}

/// Profondeur (voxels) des grottes toujours maillées sous la surface.
pub const NEAR_CAVE_VOXELS: f32 = 300.0;
/// Demi-hauteur (voxels) de la tranche de grottes maillée autour du joueur sous terre.
pub const CAVE_WINDOW_VOXELS: f32 = 150.0;

impl Terrain {
    pub fn new(params: BodyParams) -> Self {
        let mut continent: Fbm<Perlin> = Fbm::new(params.seed);
        continent.octaves = 6;
        let mut detail: Fbm<Perlin> = Fbm::new(params.seed.wrapping_add(81));
        detail.octaves = 4;
        Self {
            params,
            layout: params.layout(),
            continent,
            detail,
            forms: landforms_of(&params),
            color: Perlin::new(params.seed.wrapping_add(200)),
            relief: ReliefField::new(params.relief).with_sea(params.sea_level).with_min_crater(2.0 * params.layout().voxel / params.radius.max(1.0)),
            biomes: BiomeField::new(params.biomes),
            overhang: None,
            voxels: None,
            caves: CaveStyle::of(&params).filter(|_| params.asteroid.is_none()).map(|s| Arc::new(Caves::new(s, params.seed, params.layout().voxel))),
            min_crater: std::sync::atomic::AtomicU32::new((2.0 * params.layout().voxel / params.radius.max(1.0)).to_bits()),
            // Formes 3D (T2) sur tout astre solide ; arches et gorges seulement là où le vent et
            // l'eau sculptent la roche
            rocks: (!params.gaseous && params.asteroid.is_none()).then(|| {
                let air = params.atmosphere && !params.airless && params.pressure >= 0.05;
                let wet = air && params.hydro.liquid == crate::planetgen::hydrology::Liquid::Water;
                Arc::new(Rocks::new(params.seed, params.radius, params.layout().voxel, air, wet))
            }),
            cave_window: None,
            decor_levels: 2,
        }
        .with_overhang()
    }

    /// Tranche de grottes en plus à mailler (profondeurs sous la surface, unités).
    /// Hauteur du relief (unités) : des mers aux plus hauts sommets (horizon des tuiles, T4).
    pub fn relief_span(&self) -> f32 {
        self.params.terrain_height + 2.0 * self.forms.max_height() + self.params.relief.tier_step.max(self.params.relief.cubic_step) * layout_for(self.params.radius).voxel
    }

    /// Décor sur les tuiles des `levels` niveaux les plus fins (0.13 T4 : plus loin en Ultra).
    pub fn with_decor_levels(mut self, levels: u32) -> Self {
        self.decor_levels = levels;
        self
    }

    pub fn with_cave_window(mut self, window: Option<(f32, f32)>) -> Self {
        self.cave_window = window;
        self
    }

    /// Tranches de profondeur des grottes à mailler (unités sous la surface).
    pub fn cave_windows(&self) -> Vec<(f32, f32)> {
        let mut w = vec![(0.0, NEAR_CAVE_VOXELS * self.layout.voxel)];
        w.extend(self.cave_window);
        w
    }

    /// Le même terrain avec les grottes (et leur cache) d'un autre terrain du même astre.
    pub fn with_caves(mut self, caves: Option<Arc<Caves>>) -> Self {
        if caves.is_some() {
            self.caves = caves;
        }
        self
    }

    /// Le même terrain avec les arches (et leur cache) d'un autre terrain du même astre.
    pub fn with_rocks(mut self, rocks: Option<Arc<Rocks>>) -> Self {
        if rocks.is_some() {
            self.rocks = rocks;
        }
        self
    }

    /// Ce que les arches demandent au relief : rayon du sol, terre ferme, relief sculpté.
    pub fn rock_ground(&self, dir: Vec3) -> (f32, bool, f32) {
        let (h, _, s) = self.raw_height_full(dir);
        let sculpted = s.mountain.max(s.cliff).max(self.params.relief.terraces * 0.5).max(self.params.relief.canyons * 4.0).min(1.0);
        (h, h > self.params.radius + self.layout.voxel, sculpted)
    }

    /// Rayon du sol (champ de hauteur brut) dans la direction `dir` : la surface des grottes.
    pub fn surface_r(&self, dir: Vec3) -> f32 {
        self.raw_height(dir).0.max(self.params.radius)
    }

    /// Remplace les cellules modifiées (un impact, un autre joueur a creusé).
    pub fn set_voxels(&mut self, voxels: Option<Arc<BodyVoxels>>) {
        self.voxels = voxels.filter(|v| !v.blocks.is_empty());
    }

    /// Le même terrain avec les cellules modifiées de l'astre.
    pub fn with_voxels(mut self, voxels: Option<Arc<BodyVoxels>>) -> Self {
        self.voxels = voxels.filter(|v| !v.blocks.is_empty());
        self
    }

    fn with_overhang(mut self) -> Self {
        if self.params.asteroid.is_some() {
            return self;
        }
        self.overhang = Overhang::find(&self);
        self
    }

    pub fn voxel(&self) -> f32 {
        self.layout.voxel
    }

    /// Rayon brut (non quantifié) du sol dans la direction `dir`, et sa « hauteur relative » 0..1.
    fn raw_height(&self, dir: Vec3) -> (f32, f32) {
        let (h, hv, _) = self.raw_height_full(dir);
        (h, hv)
    }

    /// Comme `raw_height`, avec la nature du relief (éboulis, coulées, falaises).
    fn raw_height_full(&self, dir: Vec3) -> (f32, f32, ReliefSample) {
        let p = &self.params;
        // Astéroïde : sa forme (bosses et cratères compris)
        if let Some(shape) = &p.asteroid {
            let h = shape.radius_at(dir);
            return (h, ((h / p.radius.max(1e-3) - 0.6) / 0.8).clamp(0.0, 1.0), ReliefSample::default());
        }
        // Monde cubique (0.14 X6) : le relief se lit au centre d'une case (règle 16, comme `mesher.rs`)
        let voxel = layout_for(p.radius).voxel;
        let dir = crate::planetgen::landforms::sculpt_dir(dir, &p.relief, p.radius, voxel);
        let s = dir * p.noise_scale;
        let continent = self.continent.get([s.x as f64, s.y as f64, s.z as f64]) as f32;
        let ds = p.detail_scale as f64;
        let det = self.detail.get([s.x as f64 * ds, s.y as f64 * ds, s.z as f64 * ds]) as f32 * 0.15;
        let base = ((continent + det + 1.0) * 0.5).clamp(0.0, 1.0);
        // Montagnes, rifts, volcans, canyons, plateaux, cratères (`planetgen::geology`)
        let min = f32::from_bits(self.min_crater.load(std::sync::atomic::Ordering::Relaxed));
        let sample = self.relief.sample_min(dir, base, min);
        let hv = (base + sample.h).clamp(0.0, 1.2);

        // Relief à l'échelle du marcheur (en voxels) : collines, massifs le long des plaques,
        // bosses, vallées ; le même que vu de l'espace (règle 16)
        let land = crate::planetgen::landforms::land_mask(hv, p.sea_level);
        let forms = self.forms.offset(dir, land, sample.mountain, true);
        let rel = crate::planetgen::landforms::sculpt_height((hv - p.sea_level) * p.terrain_height + forms, &p.relief, voxel);
        let h = p.radius + rel;
        (h, hv, sample)
    }

    /// Matière du sol : d'après la température locale (latitude, altitude), voir `planetgen::climate`.
    fn surface_type(&self, rh: f32, dir: Vec3) -> VoxelType {
        let p = &self.params;
        self.biomes.material(&p.climate, &p.hydro, p.airless, p.atmosphere, rh, dir)
    }

    /// Biome dans la direction `dir` (`None` : sous la mer, ou planète sans biomes calculés).
    pub fn biome_at(&self, dir: Vec3) -> Option<Biome> {
        let p = &self.params;
        if !p.biomes.defined || p.gaseous {
            return None;
        }
        let (h, _) = self.raw_height(dir);
        if h < p.radius && sea_material(&p.climate, &p.hydro, p.airless, p.climate.sin_lat(dir)).is_some() {
            return None;
        }
        Some(self.biomes.biome(&p.climate, &p.hydro, p.airless, p.atmosphere, (h - p.radius) / p.terrain_height.max(1.0), dir))
    }

    /// Colonne vue de dessus dans la direction `dir` (champ de hauteur des tuiles lointaines et
    /// du décor) : le sol, ou le dessus d'une forme 3D (surplomb) s'il y en a une.
    pub fn column(&self, dir: Vec3, quantum: f32) -> Column {
        let mut c = self.base_column(dir, quantum);
        if let Some(top) = self.overhang.as_ref().and_then(|o| o.top(dir)) {
            if top > c.top {
                c.top = self.params.radius + ((top - self.params.radius) / quantum).round() * quantum;
                c.kind = VoxelType::Stone;
                c.color = OVERHANG_COLOR;
            }
        }
        c
    }

    /// Colonne du champ de hauteur seul (sans les formes 3D), hauteur arrondie au multiple de
    /// `quantum` au-dessus du niveau de la mer.
    pub fn base_column(&self, dir: Vec3, quantum: f32) -> Column {
        let p = &self.params;
        // Géante gazeuse : pas de relief, seulement le cœur où le vol s'arrête
        if p.gaseous {
            return Column::solid(dir, p.radius * GAS_CORE, VoxelType::Stone, [0.3, 0.25, 0.2, 1.0], p.radius * GAS_CORE);
        }
        // Astéroïde : roche de son type, couleurs de son maillage
        if let Some(shape) = &p.asteroid {
            use crate::planetgen::belts::AsteroidClass;
            let h = shape.radius_at(dir);
            let top = p.radius + ((h - p.radius) / quantum).round() * quantum;
            let kind = match shape.class {
                AsteroidClass::C => VoxelType::Basalt,
                AsteroidClass::S => VoxelType::Stone,
                AsteroidClass::M => VoxelType::Ore,
                AsteroidClass::Ice => VoxelType::Ice,
            };
            return Column::solid(dir, top, kind, shape.color_at(dir), h);
        }
        let (h, hv, relief) = self.raw_height_full(dir);
        let rel = ((h - p.radius) / quantum).round();
        // Marée (C4) : la mer monte ou descend de quelques voxels près du joueur
        let tide_q = if p.tide.is_calm() { 0.0 } else { (p.tide.at(dir) / quantum).round() };
        // Sous le niveau de la mer : eau, banquise, ou bassin à sec (trop chaud, ou sans air)
        let sea = if rel < tide_q { sea_material(&p.climate, &p.hydro, p.airless, p.climate.sin_lat(dir)) } else { None };
        let water = sea.is_some();

        // Taches de couleur sur plusieurs échelles (0.13 T3) : 200, 30 et 5 voxels, seulement
        // celles que la tuile peut montrer (pas de moiré sur les tuiles lointaines)
        let mut var = self.color.get([dir.x as f64 * 12.0, dir.y as f64 * 12.0, dir.z as f64 * 12.0]) as f32 * 0.10;
        for (k, (wave, amp)) in [(200.0, 0.06), (30.0, 0.045), (5.0, 0.03)].into_iter().enumerate() {
            let wave = wave * self.layout.voxel;
            if wave < 3.0 * quantum {
                break;
            }
            let f = (p.radius / wave) as f64;
            let o = 37.1 * (k + 1) as f64;
            var += value_noise([dir.x as f64 * f + o, dir.y as f64 * f - o, dir.z as f64 * f + 0.5 * o], p.seed.wrapping_add(k as u32)) * amp;
        }
        let jitter = ((dir.x * 127.1 + dir.y * 311.7 + dir.z * 74.7).sin() * 43758.547).fract().abs() * 0.05 - 0.025;

        let mut bed = None;
        let (top, kind, color) = if water {
            let kind = sea.unwrap_or(VoxelType::Water);
            let base = kind.color();
            let depth = ((p.radius - h) / (p.terrain_height * 0.5)).clamp(0.0, 0.45);
            let color = [
                (base[0] * (1.0 - depth) + var * 0.7 + jitter).clamp(0.03, 1.0),
                (base[1] * (1.0 - depth * 0.4) + var * 0.9 + jitter).clamp(0.03, 1.0),
                (base[2] * (1.0 - depth * 0.2) + var * 0.4 + jitter).clamp(0.03, 1.0),
                1.0,
            ];
            // Fond marin gardé sous un liquide transparent (0.13 O1) : sable près du rivage, vase
            // et roche sombre vers le large
            if kind.is_clear_liquid() {
                let bed_top = p.radius + rel * quantum;
                let deep = ((p.radius + tide_q * quantum - bed_top) / (self.layout.voxel * 300.0)).clamp(0.0, 1.0);
                let shore = ((p.radius + tide_q * quantum - bed_top) / (self.layout.voxel * 24.0)).clamp(0.0, 1.0);
                let bed_kind = if shore < 0.85 { VoxelType::Sand } else { VoxelType::Stone };
                let sand = VoxelType::Sand.color();
                let floor = [0.34, 0.33, 0.34, 1.0];
                let c = mix(mix(sand, [0.52, 0.46, 0.34, 1.0], shore), floor, deep);
                let bed_color = [
                    (c[0] + var * 0.7 + jitter).clamp(0.03, 1.0),
                    (c[1] + var * 0.9 + jitter).clamp(0.03, 1.0),
                    (c[2] + var * 0.4 + jitter).clamp(0.03, 1.0),
                    1.0,
                ];
                bed = Some((bed_top, bed_kind, bed_color));
            }
            (p.radius + tide_q * quantum, kind, color)
        } else {
            // Hauteur réelle (collines comprises) : une colline au bord de l'eau n'est pas une plage
            let mut kind = self.surface_type((h - p.radius) / p.terrain_height.max(1.0), dir);
            // Marée basse : le fond découvert est une grève de sable
            if rel < 0.0 && sea_material(&p.climate, &p.hydro, p.airless, p.climate.sin_lat(dir)).is_some() {
                kind = VoxelType::Sand;
            }
            // Coulées de lave figées (basalte), éboulis au pied des pentes et des falaises (sauf
            // sous la neige éternelle des sommets)
            // Fond de cratère rempli : glace sur un monde froid et humide, lave figée ailleurs
            if relief.flooded && !kind.is_liquid() {
                kind = if p.hydro.snow && p.climate.mean_c < -20.0 { VoxelType::Ice } else { VoxelType::Basalt };
            }
            if !p.airless && kind != VoxelType::Snow && kind != VoxelType::Ice {
                if relief.lava {
                    kind = VoxelType::Basalt;
                } else if relief.scree && relief.cliff < 0.5 {
                    kind = VoxelType::Stone;
                }
            }
            let color = if p.airless {
                let (lo, hi) = ([0.45, 0.44, 0.42], [0.70, 0.68, 0.65]);
                [
                    (lo[0] + (hi[0] - lo[0]) * hv + var + jitter).clamp(0.03, 1.0),
                    (lo[1] + (hi[1] - lo[1]) * hv + var * 0.8 + jitter).clamp(0.03, 1.0),
                    (lo[2] + (hi[2] - lo[2]) * hv + var * 0.4 + jitter).clamp(0.03, 1.0),
                    1.0,
                ]
            } else {
                let base = kind.color();
                [
                    (base[0] + var * 0.7 + jitter).clamp(0.03, 1.0),
                    (base[1] + var * 0.9 + jitter).clamp(0.03, 1.0),
                    (base[2] + var * 0.4 + jitter).clamp(0.03, 1.0),
                    1.0,
                ]
            };
            // Éjectas et rayons clairs des cratères récents ; mers de lave des bassins remplis
            let b = relief.bright;
            let mut color = [
                color[0] + (0.86 - color[0]) * b * 0.6,
                color[1] + (0.85 - color[1]) * b * 0.6,
                color[2] + (0.82 - color[2]) * b * 0.6,
                1.0,
            ];
            if relief.flooded && p.airless {
                color = [color[0] * 0.55, color[1] * 0.55, color[2] * 0.58, 1.0];
            }
            (p.radius + rel * quantum, kind, color)
        };
        match bed {
            Some((bed, bed_kind, bed_color)) => Column { dir, top, kind, color, raw: h, bed, bed_kind, bed_color },
            None => Column::solid(dir, top, kind, color, h),
        }
    }

    /// Surface la plus haute de la colonne du niveau le plus fin qui contient `dir` (dessus d'un
    /// surplomb compris).
    pub fn ground(&self, dir: Vec3) -> Column {
        self.floor(dir, f32::INFINITY)
    }

    // ── Voxels 3D ────────────────────────────────────────────────────────

    /// Colonnes par côté d'une face au niveau le plus fin.
    fn lattice(&self) -> i64 {
        (TILE_CELLS as i64) << self.layout.max_depth
    }

    /// Colonne (face, i, j) du niveau le plus fin qui contient `dir`.
    pub fn cell_of(&self, dir: Vec3) -> (u8, i64, i64) {
        let n = self.lattice();
        let (face, s, t) = dir_to_face(dir);
        let i = ((((s + 1.0) * 0.5) as f64 * n as f64).floor() as i64).clamp(0, n - 1);
        let j = ((((t + 1.0) * 0.5) as f64 * n as f64).floor() as i64).clamp(0, n - 1);
        (face, i, j)
    }

    /// Direction du centre de la colonne (i, j) de la face `face` (i, j peuvent déborder d'une
    /// colonne au bord de la face).
    pub fn cell_dir(&self, face: u8, i: i64, j: i64) -> Vec3 {
        // Même calcul (f32) que les tuiles en champ de hauteur : mêmes colonnes au bit près
        let n = self.lattice() as f32;
        face_dir(face, -1.0 + 2.0 * (i as f32 + 0.5) / n, -1.0 + 2.0 * (j as f32 + 0.5) / n)
    }

    /// Couche radiale qui contient le rayon `r` (0 = juste au-dessus du niveau de la mer).
    /// (un centième de voxel de tolérance : à 16 000 unités du centre, l'écart entre deux `f32`
    /// voisins fait ~0,004 voxel à l'échelle 0.13, et un rayon pile sur une limite de couche,
    /// calculé par `layer_radius`, tomberait dans la couche du dessous ; règle 19)
    pub fn layer(&self, r: f32) -> i32 {
        ((r - self.params.radius) / self.layout.voxel + 0.01).floor() as i32
    }

    /// Rayon du bas de la couche `k`.
    pub fn layer_radius(&self, k: i32) -> f32 {
        self.params.radius + k as f32 * self.layout.voxel
    }

    /// Colonne de base au niveau le plus fin et sa première couche vide.
    fn base_cell_column(&self, dir: Vec3) -> (Column, i32) {
        let c = self.base_column(dir, self.layout.voxel);
        let top_k = ((c.top - self.params.radius) / self.layout.voxel).round() as i32;
        (c, top_k)
    }

    /// Formes 3D ou cellules modifiées près de cette colonne : sinon le champ de hauteur suffit.
    fn has_3d(&self, face: u8, i: i64, j: i64, dir: Vec3) -> bool {
        self.caves.is_some()
            || self.rocks.is_some()
            || self.overhang.as_ref().is_some_and(|o| o.near(dir))
            || self.voxels.as_ref().is_some_and(|v| v.layers_in(face, i.div_euclid(BLOCK), j.div_euclid(BLOCK)).is_some())
    }

    /// LA fonction des voxels (règle 11) : matière de la cellule (couche `k`) de la colonne
    /// (face, i, j), dont la colonne de base est `base` (première couche vide `top_k`).
    /// Delta d'abord, puis le champ de hauteur creusé par les grottes, puis les formes 3D.
    pub fn kind_at(&self, face: u8, i: i64, j: i64, k: i32, dir: Vec3, base: &Column, top_k: i32) -> VoxelType {
        self.kind_in(face, i, j, k, dir, base, top_k, None)
    }

    /// Comme `kind_at`, avec les pièces de grottes qui croisent la colonne déjà rassemblées, et
    /// les couches (k0..=k1) où chacune peut compter (maillage d'une tuile).
    #[allow(clippy::too_many_arguments)]
    pub fn kind_in(&self, face: u8, i: i64, j: i64, k: i32, dir: Vec3, base: &Column, top_k: i32, pieces: Option<&[(i32, i32, Piece)]>) -> VoxelType {
        if let Some(v) = &self.voxels {
            if let Some(kind) = v.get(Cell { face, i, j, k }) {
                return kind;
            }
        }
        if k < top_k {
            let v = self.layout.voxel;
            // Mer : le fond (sable puis roche) sous une eau transparente (0.13 O1)
            if base.kind.is_clear_liquid() {
                let bed_k = ((base.bed - self.params.radius) / v).round() as i32;
                return if k >= bed_k { base.kind } else { base.bed_kind };
            }
            let r = self.layer_radius(k) + v * 0.5;
            let depth = base.top - r;
            // Sous la mer, pas de grotte (elle se remplirait) ; près de la surface, le sol.
            // Grottes, puis formes creusées du relief (gorges, corniches, strates : T2)
            if !base.kind.is_liquid() {
                let p = dir * r;
                let cell = match pieces {
                    Some(list) => eval_pieces(list.iter().filter(|(k0, k1, _)| (*k0..=*k1).contains(&k)).map(|(_, _, piece)| piece), p, v),
                    None => {
                        let caves = self.caves.as_ref().filter(|c| depth < crate::caves::MAX_DEPTH + c.size).map(|c| c.pieces_near(p, &|d| self.surface_r(d)));
                        let rocks = self.rocks.as_ref().map(|r| r.pieces_near(dir, &|d| self.rock_ground(d)));
                        eval_pieces(
                            caves.iter().flat_map(|a| a.iter()).chain(rocks.iter().flat_map(|a| a.iter())).filter(|piece| {
                                let (c, r) = piece.bound();
                                c.distance_squared(p) <= r * r
                            }),
                            p,
                            v,
                        )
                    }
                };
                match cell {
                    CaveCell::Air => return VoxelType::Air,
                    CaveCell::Water => return VoxelType::Water,
                    CaveCell::Crystal => return VoxelType::Crystal,
                    CaveCell::Rock => {}
                }
            }
            if depth < 2.5 * v || base.kind.is_liquid() {
                return base.kind;
            }
            // Roche profonde, avec des filons plus fréquents en profondeur (phase 8)
            let h = (((face as u64) << 58) ^ (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (j as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ (k as u64).wrapping_mul(0x1656_67B1_9E37_79F9))
                .wrapping_mul(0xBF58_476D_1CE4_E5B9);
            if ((h >> 40) as f32 / (1u64 << 24) as f32) < ore_chance(depth) {
                return VoxelType::Ore;
            }
            return self.caves.as_ref().map_or(VoxelType::Stone, |c| c.style.rock);
        }
        let p = dir * (self.layer_radius(k) + self.layout.voxel * 0.5);
        if let Some(o) = &self.overhang {
            if o.solid(p) {
                return VoxelType::Stone;
            }
        }
        // Arches et cheminées de fée posées sur le sol
        let added = |piece: &Piece| matches!(piece, Piece::Add(s) if s.contains(p));
        let hit = match pieces {
            Some(list) => list.iter().any(|(k0, k1, piece)| (*k0..=*k1).contains(&k) && added(piece)),
            None => self.rocks.as_ref().is_some_and(|r| r.pieces_near(dir, &|d| self.rock_ground(d)).iter().any(added)),
        };
        if hit {
            return VoxelType::Stone;
        }
        VoxelType::Air
    }

    /// Sol sous le point (`dir`, `r`) : la plus haute surface pleine dont le dessus est au plus à
    /// `r` (le dessus d'un surplomb si l'on est dessus, le sol si l'on est dessous).
    pub fn floor(&self, dir: Vec3, r: f32) -> Column {
        self.floor_in(dir, r, false)
    }

    /// Comme `floor`, mais le fond sous l'eau (0.13 O2 : nage, plongée) : les liquides
    /// transparents ne comptent pas.
    pub fn seabed(&self, dir: Vec3, r: f32) -> Column {
        self.floor_in(dir, r, true)
    }

    /// Le liquide transparent des mers de l'astre à la latitude de `dir` (sans lui : pas de mer, ou
    /// gelée / de lave).
    pub fn sea_kind(&self, dir: Vec3) -> Option<VoxelType> {
        let p = &self.params;
        if p.gaseous || p.asteroid.is_some() || !p.atmosphere {
            return None;
        }
        sea_material(&p.climate, &p.hydro, p.airless, p.climate.sin_lat(dir)).filter(|k| k.is_clear_liquid())
    }

    /// Surface du liquide transparent au-dessus de `dir` : (rayon, matière), si la colonne en a.
    pub fn sea_surface(&self, dir: Vec3) -> Option<(f32, VoxelType)> {
        if self.params.gaseous || self.params.asteroid.is_some() {
            return None;
        }
        let (face, i, j) = self.cell_of(dir);
        let (base, _) = self.base_cell_column(self.cell_dir(face, i, j));
        base.is_sea().then_some((base.top, base.kind))
    }

    fn floor_in(&self, dir: Vec3, r: f32, dry: bool) -> Column {
        let p = &self.params;
        if p.gaseous {
            return Column::solid(dir, p.radius * GAS_CORE, VoxelType::Stone, [0.3, 0.25, 0.2, 1.0], p.radius * GAS_CORE);
        }
        let (face, i, j) = self.cell_of(dir);
        let center = self.cell_dir(face, i, j);
        let (base, top_k) = self.base_cell_column(center);
        if !self.has_3d(face, i, j, center) {
            return if dry { base.ground_view() } else { base };
        }
        let highest = self.highest_layer(face, i, j, top_k);
        // (marges en fraction de voxel : à 10 000 unités du centre, 0,001 est sous la précision
        // d'un f32)
        let start = if r.is_finite() { (self.layer(r + 0.05 * self.layout.voxel) - 1).min(highest) } else { highest };
        for k in (start - 512..=start).rev() {
            let kind = self.kind_at(face, i, j, k, center, &base, top_k);
            if kind != VoxelType::Air && !(dry && kind.is_clear_liquid()) {
                let color = if k < top_k { base.ground_view().color } else { OVERHANG_COLOR };
                return Column { dir: center, top: self.layer_radius(k + 1), kind, color, raw: base.raw, bed: base.bed, bed_kind: base.bed_kind, bed_color: base.bed_color };
            }
        }
        if dry { base.ground_view() } else { base }
    }

    /// Plafond au-dessus du point (`dir`, `r`) : bas de la première cellule pleine au-dessus de
    /// `r` (l'infini s'il n'y en a pas).
    pub fn ceiling(&self, dir: Vec3, r: f32) -> f32 {
        let (face, i, j) = self.cell_of(dir);
        let center = self.cell_dir(face, i, j);
        if self.params.gaseous || !self.has_3d(face, i, j, center) {
            return f32::INFINITY;
        }
        let (base, top_k) = self.base_cell_column(center);
        let highest = self.highest_layer(face, i, j, top_k);
        let k0 = ((r - self.params.radius) / self.layout.voxel - 0.05).ceil() as i32;
        for k in k0..=highest {
            if self.kind_at(face, i, j, k, center, &base, top_k) != VoxelType::Air {
                return self.layer_radius(k);
            }
        }
        f32::INFINITY
    }

    /// Plus haute couche qui peut être pleine dans cette colonne.
    fn highest_layer(&self, face: u8, i: i64, j: i64, top_k: i32) -> i32 {
        let mut k = top_k;
        if let Some(o) = &self.overhang {
            k = k.max(o.layers(self).1);
        }
        if let Some((_, hi)) = self.voxels.as_ref().and_then(|v| v.layers_in(face, i.div_euclid(BLOCK), j.div_euclid(BLOCK))) {
            k = k.max(hi);
        }
        if let Some(rocks) = &self.rocks {
            let dir = self.cell_dir(face, i, j);
            for piece in rocks.pieces_near(dir, &|d| self.rock_ground(d)).iter().filter(|p| matches!(p, Piece::Add(_))) {
                let (c, r) = piece.bound();
                k = k.max(self.layer(c.length() + r) + 1);
            }
        }
        k
    }
}

/// Couleur des formes 3D de test (roche claire).
const OVERHANG_COLOR: [f32; 4] = [0.62, 0.58, 0.54, 1.0];

/// Surplomb artificiel de test (B1) : une arche dont le tablier s'avance en auvent, sur la terre
/// ferme, à un endroit fixé par la graine de l'astre (commande `/surplomb` pour y aller).
#[derive(Clone, Copy, Debug)]
pub struct Overhang {
    pub dir: Vec3,
    east: Vec3,
    north: Vec3,
    /// Rayon du sol au pied de l'arche, taille d'un voxel.
    pub base: f32,
    v: f32,
}

/// Boîtes de l'arche (min, max) en voxels : vers l'est, vers le nord, vers le haut.
const ARCH: [([f32; 3], [f32; 3]); 3] = [
    ([-12.0, -3.0, -6.0], [-8.0, 3.0, 9.0]),
    ([8.0, -3.0, -6.0], [12.0, 3.0, 9.0]),
    ([-12.0, -3.0, 9.0], [22.0, 3.0, 12.0]),
];

impl Overhang {
    /// Cherche une terre ferme (pas sous la mer) dans des directions tirées de la graine.
    fn find(t: &Terrain) -> Option<Self> {
        let p = &t.params;
        if p.gaseous {
            return None;
        }
        let v = t.layout.voxel;
        let mut x = (p.seed as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0x5851_F42D_4C95_7F2D;
        let mut next = || {
            x ^= x >> 33;
            x = x.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
            x ^= x >> 29;
            (x >> 11) as f32 / (1u64 << 53) as f32
        };
        for _ in 0..48 {
            let z = 1.6 * next() - 0.8;
            let a = std::f32::consts::TAU * next();
            let r = (1.0 - z * z).sqrt();
            let dir = Vec3::new(r * a.cos(), z, r * a.sin());
            let c = t.base_column(dir, v);
            if c.kind.is_liquid() {
                continue;
            }
            let east = Vec3::Y.cross(dir).normalize_or(Vec3::X);
            let north = dir.cross(east).normalize();
            return Some(Self { dir, east, north, base: c.top, v });
        }
        None
    }

    /// Coordonnées locales (voxels) d'un point du repère de l'astre.
    fn local(&self, p: Vec3) -> [f32; 3] {
        let d = p - self.dir * self.base;
        [d.dot(self.east) / self.v, d.dot(self.north) / self.v, (p.length() - self.base) / self.v]
    }

    pub fn solid(&self, p: Vec3) -> bool {
        // Seulement près de l'arche (à l'antipode, les coordonnées locales retomberaient dedans)
        if (p - self.dir * self.base).length_squared() > (40.0 * self.v).powi(2) {
            return false;
        }
        let l = self.local(p);
        ARCH.iter().any(|(lo, hi)| (0..3).all(|a| l[a] >= lo[a] && l[a] < hi[a]))
    }

    /// La colonne `dir` passe près de l'arche.
    pub fn near(&self, dir: Vec3) -> bool {
        dir.dot(self.dir) > (40.0 * self.v / self.base).cos()
    }

    /// Dessus de l'arche dans la direction `dir` (vue de dessus), s'il y en a.
    pub fn top(&self, dir: Vec3) -> Option<f32> {
        if !self.near(dir) {
            return None;
        }
        let l = self.local(dir * self.base);
        ARCH.iter()
            .filter(|(lo, hi)| l[0] >= lo[0] && l[0] < hi[0] && l[1] >= lo[1] && l[1] < hi[1])
            .map(|(_, hi)| self.base + hi[2] * self.v)
            .reduce(f32::max)
    }

    /// Couches (k) occupées par l'arche.
    pub fn layers(&self, t: &Terrain) -> (i32, i32) {
        (t.layer(self.base - 7.0 * self.v), t.layer(self.base + 13.0 * self.v))
    }

    /// Un point sous l'auvent, au sol (pour `/surplomb`) : à l'est des piliers.
    pub fn visit_dir(&self) -> Vec3 {
        (self.dir * self.base + self.east * 17.0 * self.v).normalize()
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  Maillage d'une tuile
// ─────────────────────────────────────────────────────────────────────────

#[derive(Default)]
struct MeshBuf {
    pos: Vec<[f32; 3]>,
    nor: Vec<[f32; 3]>,
    col: Vec<[f32; 4]>,
    idx: Vec<u32>,
}

impl MeshBuf {
    /// Quad plat dont la face avant regarde du côté de `normal`.
    fn quad(&mut self, c: [Vec3; 4], normal: Vec3, color: [f32; 4]) {
        let base = self.pos.len() as u32;
        for p in &c {
            self.pos.push(p.to_array());
            self.nor.push(normal.to_array());
            self.col.push(color);
        }
        let geo = (c[1] - c[0]).cross(c[2] - c[0]);
        if geo.dot(normal) >= 0.0 {
            self.idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        } else {
            self.idx.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
        }
    }

    fn into_mesh(self) -> Mesh {
        // Seulement sur la carte graphique : la copie en mémoire est libérée après l'envoi (T4)
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.pos);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.nor);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.col);
        mesh.insert_indices(Indices::U32(self.idx));
        mesh
    }
}

/// Quantum vertical (pas de hauteur) d'une tuile de profondeur `depth` : les cellules grossissent avec
/// la distance, mais pas les marches, sinon le lointain serait fait de falaises de plusieurs
/// centaines d'unités.
pub fn tile_quantum(layout: Layout, depth: u32) -> f32 {
    let steps = layout.max_depth - depth.min(layout.max_depth);
    layout.voxel * (1u32 << steps.min(MAX_STEP_DOUBLINGS)) as f32
}

/// Les marches ne dépassent pas 4 voxels (2 doublements).
const MAX_STEP_DOUBLINGS: u32 = 2;

/// Construit le maillage d'une tuile, dans le repère du corps (centre à l'origine).
///
/// Chaque colonne donne une face supérieure ; entre deux colonnes de hauteurs différentes, la plus
/// haute dessine la paroi qui les sépare. Une « jupe » descend le long des bords de la tuile pour
/// cacher les fentes avec les tuiles voisines de profondeur différente.
#[cfg(test)]
pub fn build_tile_mesh(params: &BodyParams, key: TileKey) -> Mesh {
    build_tile_mesh_with(&Terrain::new(*params), key)
}

/// Comme `build_tile_mesh`, avec un `Terrain` déjà construit (partagé avec le décor). Au niveau le
/// plus fin (près du joueur), la tuile est en voxels 3D.
#[allow(dead_code)]
pub fn build_tile_mesh_with(terrain: &Terrain, key: TileKey) -> Mesh {
    build_tile_meshes_with(terrain, key).0
}

/// La tuile et, s'il y a de l'eau (mer, lac), sa surface transparente à part (0.13 O1).
pub fn build_tile_meshes_with(terrain: &Terrain, key: TileKey) -> (Mesh, Option<Mesh>) {
    if key.depth as u32 >= terrain.layout.max_depth && !terrain.params.gaseous {
        return build_voxel_tile_mesh(terrain, key);
    }
    build_height_tile_mesh(terrain, key)
}

/// Tuile en champ de hauteur (tuiles lointaines).
pub fn build_height_tile_mesh(terrain: &Terrain, key: TileKey) -> (Mesh, Option<Mesh>) {
    // Pas de cratère plus petit qu'une colonne et demie de cette tuile (on ne le verrait pas)
    use std::sync::atomic::Ordering;
    let before = terrain.min_crater.load(Ordering::Relaxed);
    let column = FRAC_PI_2 / ((TILE_CELLS as u32) << key.depth.min(30)) as f32;
    terrain.min_crater.store(f32::from_bits(before).max(1.5 * column).to_bits(), Ordering::Relaxed);
    let mesh = build_height_tile_inner(terrain, key);
    terrain.min_crater.store(before, Ordering::Relaxed);
    mesh
}

fn build_height_tile_inner(terrain: &Terrain, key: TileKey) -> (Mesh, Option<Mesh>) {
    let layout = terrain.layout;
    let depth = (key.depth as u32).min(layout.max_depth);
    let lattice = (TILE_CELLS as u32) << depth;
    let quantum = tile_quantum(layout, depth);
    let skirt = quantum * 8.0;
    let (i0, j0) = (key.x as i64 * TILE_CELLS as i64, key.y as i64 * TILE_CELLS as i64);

    let line = |i: i64| -> f32 { -1.0 + 2.0 * i as f32 / lattice as f32 };
    let mid = |i: i64| -> f32 { -1.0 + 2.0 * (i as f32 + 0.5) / lattice as f32 };

    let n1 = TILE_CELLS + 1;
    let mut corners = Vec::with_capacity(n1 * n1);
    for cj in 0..n1 as i64 {
        for ci in 0..n1 as i64 {
            corners.push(face_dir(key.face, line(i0 + ci), line(j0 + cj)));
        }
    }
    let corner = |ci: usize, cj: usize| corners[cj * n1 + ci];

    // Colonnes de la tuile et une rangée de colonnes voisines tout autour
    let nc = TILE_CELLS + 2;
    let mut cols = Vec::with_capacity(nc * nc);
    for cj in -1..=TILE_CELLS as i64 {
        for ci in -1..=TILE_CELLS as i64 {
            cols.push(terrain.column(face_dir(key.face, mid(i0 + ci), mid(j0 + cj)), quantum));
        }
    }
    tint_columns(terrain, &mut cols, nc, |c| c, |c| c);
    let col = |ci: i32, cj: i32| &cols[(cj + 1) as usize * nc + (ci + 1) as usize];

    let mut buf = MeshBuf::default();
    buf.pos.reserve(TILE_CELLS * TILE_CELLS * 6);
    let last = TILE_CELLS as i32 - 1;

    for cj in 0..TILE_CELLS as i32 {
        for ci in 0..TILE_CELLS as i32 {
            let c0 = col(ci, cj);
            let c = &c0.ground_view();
            let (u, v) = (ci as usize, cj as usize);

            buf.quad(
                [
                    corner(u, v) * c.top,
                    corner(u + 1, v) * c.top,
                    corner(u + 1, v + 1) * c.top,
                    corner(u, v + 1) * c.top,
                ],
                c.dir,
                submerged(c.color, c0.is_sea()),
            );

            // (voisin, extrémités de l'arête partagée)
            let sides: [((i32, i32), (usize, usize), (usize, usize)); 4] = [
                ((1, 0), (u + 1, v), (u + 1, v + 1)),
                ((-1, 0), (u, v), (u, v + 1)),
                ((0, 1), (u, v + 1), (u + 1, v + 1)),
                ((0, -1), (u, v), (u + 1, v)),
            ];
            for ((dx, dy), ea, eb) in sides {
                let (ni, nj) = (ci + dx, cj + dy);
                let nb0 = col(ni, nj);
                let nb = &nb0.ground_view();
                let boundary = ni < 0 || ni > last || nj < 0 || nj > last;
                let hi = c.top;
                let lo = if boundary {
                    c.top.min(nb.top) - skirt
                } else if c.top > nb.top + 1e-3 {
                    nb.top
                } else {
                    continue;
                };
                let (a, b) = (corner(ea.0, ea.1), corner(eb.0, eb.1));
                let mut n = (b - a).cross(c.dir).normalize_or_zero();
                if n.dot(nb.dir - c.dir) < 0.0 {
                    n = -n;
                }
                // Parois : la roche nue (le sol seulement au bord des marches)
                let wall = if c.top - lo > 1.5 * quantum && !c.kind.is_liquid() { rock_of(c.color) } else { c.color };
                let shade = submerged([wall[0] * 0.82, wall[1] * 0.82, wall[2] * 0.82, 1.0], c0.is_sea() || nb0.is_sea());
                buf.quad([a * lo, b * lo, b * hi, a * hi], n, shade);
            }
        }
    }
    let water = water_mesh(&cols, nc, &corners, n1, (i0, j0, 1i64 << (layout.max_depth - depth)), |c| c);
    (buf.into_mesh(), water)
}

/// Surface de l'eau d'une tuile (0.13 O1) : une grille de sommets (un par coin de colonne) au
/// niveau de la mer, dont la couleur est celle du liquide et l'alpha la PROFONDEUR (unités, moyenne
/// des quatre colonnes autour du sommet : 0 au rivage). `None` sans eau.
///
/// `lattice` = (i0, j0, pas) : coordonnées du coin (0, 0) de la tuile dans le réseau de colonnes
/// le plus fin de la face, et pas entre deux colonnes (UV = ces coordonnées modulo `WAVE_PERIOD`,
/// pour que les vagues, qui ne dépendent que d'elles, se raccordent d'une tuile à l'autre).
fn water_mesh<T>(cols: &[T], nc: usize, corners: &[Vec3], n1: usize, lattice: (i64, i64, i64), get: impl Fn(&T) -> &Column) -> Option<Mesh> {
    let at = |ci: usize, cj: usize| get(&cols[cj * nc + ci]);
    let cells = TILE_CELLS;
    // Colonnes de la tuile : indices 1..=cells dans `cols` (une rangée de voisines autour)
    let wet = |i: usize, j: usize| at(i + 1, j + 1).is_sea();
    if !(0..cells).any(|j| (0..cells).any(|i| wet(i, j))) {
        return None;
    }
    let mut pos = Vec::with_capacity(n1 * n1);
    let mut nor = Vec::with_capacity(n1 * n1);
    let mut col = Vec::with_capacity(n1 * n1);
    let mut uv = Vec::with_capacity(n1 * n1);
    for cj in 0..n1 {
        for ci in 0..n1 {
            uv.push([((lattice.0 + ci as i64 * lattice.2).rem_euclid(WAVE_PERIOD)) as f32, ((lattice.1 + cj as i64 * lattice.2).rem_euclid(WAVE_PERIOD)) as f32]);
            // Les quatre colonnes autour du coin (ci, cj) : (ci-1, cj-1) .. (ci, cj) de la tuile
            let mut level = 0.0;
            let mut count = 0.0;
            let mut depth = 0.0;
            let mut tint = [0.0f32; 3];
            for (di, dj) in [(0usize, 0usize), (1, 0), (0, 1), (1, 1)] {
                let c = at(ci + di, cj + dj);
                if c.is_sea() {
                    level += c.top;
                    count += 1.0;
                    depth += c.depth();
                    let k = c.kind.color();
                    tint = [tint[0] + k[0], tint[1] + k[1], tint[2] + k[2]];
                }
            }
            let dir = corners[cj * n1 + ci];
            if count == 0.0 {
                pos.push((dir * at(ci + 1, cj + 1).top).to_array());
                col.push([0.0, 0.0, 0.0, 0.0]);
            } else {
                pos.push((dir * (level / count)).to_array());
                col.push([tint[0] / count, tint[1] / count, tint[2] / count, depth / 4.0]);
            }
            nor.push(dir.to_array());
        }
    }
    let mut idx = Vec::new();
    let v = |i: usize, j: usize| (j * n1 + i) as u32;
    for j in 0..cells {
        for i in 0..cells {
            if wet(i, j) {
                idx.extend_from_slice(&[v(i, j), v(i + 1, j), v(i + 1, j + 1), v(i, j), v(i + 1, j + 1), v(i, j + 1)]);
            }
        }
    }
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, nor);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uv);
    mesh.insert_indices(Indices::U32(idx));
    Some(mesh)
}

/// Couleurs des dessus d'une tuile d'après la pente et les parois (`ground_tint`) ; `cols` a une
/// rangée de voisines tout autour (`nc` par côté).
fn tint_columns<T>(t: &Terrain, cols: &mut [T], nc: usize, get: impl Fn(&T) -> &Column, get_mut: impl Fn(&mut T) -> &mut Column) {
    let p = &t.params;
    let wet = p.atmosphere && !p.airless && p.hydro.liquid == crate::planetgen::hydrology::Liquid::Water;
    let at = |ci: usize, cj: usize| cj * nc + ci;
    let tinted: Vec<(usize, [f32; 4])> = (1..nc - 1)
        .flat_map(|cj| (1..nc - 1).map(move |ci| (ci, cj)))
        .map(|(ci, cj)| {
            let c = get(&cols[at(ci, cj)]);
            let (slope, wall) = slope_of(c, get(&cols[at(ci + 1, cj)]), get(&cols[at(ci - 1, cj)]), get(&cols[at(ci, cj + 1)]), get(&cols[at(ci, cj - 1)]), t.layout.voxel);
            (at(ci, cj), ground_tint(c.kind, c.color, slope, wall, wet))
        })
        .collect();
    for (k, color) in tinted {
        get_mut(&mut cols[k]).color = color;
    }
}

/// Colonne d'une tuile 3D (avec une rangée de voisines tout autour).
struct Col3 {
    base: Column,
    top_k: i32,
    bed_k: i32,
    /// Colonne canonique (une voisine au-delà du bord de la face est rapportée à sa vraie face).
    face: u8,
    i: i64,
    j: i64,
}

/// Couleur d'une cellule : celle de la colonne pour le sol, celle de sa matière pour la roche, les
/// filons, les cristaux et l'eau des grottes (avec un peu de variation).
fn cell_color(kind: VoxelType, k: i32, c: &Col3) -> [f32; 4] {
    let ground = c.base.ground_view();
    if kind == ground.kind && k < c.bed_k {
        return ground.color;
    }
    if k >= c.bed_k && kind == VoxelType::Stone {
        return OVERHANG_COLOR;
    }
    let base = kind.color();
    let jitter = (((c.i * 31 + c.j * 17 + k as i64 * 7) & 15) as f32 / 15.0 - 0.5) * 0.08;
    [(base[0] + jitter).clamp(0.02, 1.0), (base[1] + jitter).clamp(0.02, 1.0), (base[2] + jitter).clamp(0.02, 1.0), 1.0]
}

/// Tuile en voxels 3D (niveau le plus fin) : chaque cellule pleine montre ses faces tournées vers
/// une cellule vide (dessus, dessous des surplombs, côtés). Sans forme 3D, le résultat a les mêmes
/// dessus que le champ de hauteur. Une jupe descend le long des bords (raccord avec les tuiles
/// plus grossières voisines).
pub fn build_voxel_tile_mesh(t: &Terrain, key: TileKey) -> (Mesh, Option<Mesh>) {
    let layout = t.layout;
    let v = layout.voxel;
    let n = t.lattice();
    let (i0, j0) = (key.x as i64 * TILE_CELLS as i64, key.y as i64 * TILE_CELLS as i64);
    let line = |i: i64| -> f32 { -1.0 + 2.0 * i as f32 / n as f32 };

    let n1 = TILE_CELLS + 1;
    let mut corners = Vec::with_capacity(n1 * n1);
    for cj in 0..n1 as i64 {
        for ci in 0..n1 as i64 {
            corners.push(face_dir(key.face, line(i0 + ci), line(j0 + cj)));
        }
    }
    let corner = |ci: usize, cj: usize| corners[cj * n1 + ci];

    let nc = TILE_CELLS + 2;
    let mut cols: Vec<Col3> = Vec::with_capacity(nc * nc);
    for cj in -1..=TILE_CELLS as i64 {
        for ci in -1..=TILE_CELLS as i64 {
            let (gi, gj) = (i0 + ci, j0 + cj);
            let dir = t.cell_dir(key.face, gi, gj);
            let (face, i, j) = if (0..n).contains(&gi) && (0..n).contains(&gj) { (key.face, gi, gj) } else { t.cell_of(dir) };
            let center = t.cell_dir(face, i, j);
            let (base, top_k) = t.base_cell_column(center);
            // Première couche vide au-dessus du fond (sous l'eau, le fond marin)
            let bed_k = ((base.bed - t.params.radius) / v).round() as i32;
            cols.push(Col3 { base, top_k, bed_k, face, i, j });
        }
    }
    tint_columns(t, &mut cols, nc, |c| &c.base, |c| &mut c.base);
    let col = |ci: i32, cj: i32| &cols[(cj + 1) as usize * nc + (ci + 1) as usize];

    // Couches à examiner : autour du sol, plus les formes 3D et les cellules modifiées
    let tile_dir = key.center_dir();
    let any_3d = t.overhang.as_ref().is_some_and(|o| tile_dir.dot(o.dir) > (key.arc(t.params.radius) * 0.8 / o.base + 40.0 * v / o.base).cos())
        || t.voxels.as_ref().is_some_and(|vx| cols.iter().any(|c| vx.layers_in(c.face, c.i.div_euclid(BLOCK), c.j.div_euclid(BLOCK)).is_some()));
    let mut kmin = cols.iter().map(|c| c.bed_k).min().unwrap_or(0) - 1;
    let mut kmax = cols.iter().map(|c| c.top_k).max().unwrap_or(0) + 1;
    // Sans forme 3D, chaque colonne n'examine que les couches de ses voisines (et pas toute la
    // hauteur de la tuile : une mer profonde aurait des centaines de couches d'eau)
    let near = |ci: i32, cj: i32| -> (i32, i32) {
        let (mut lo, mut hi) = (i32::MAX, i32::MIN);
        for dj in -1..=1 {
            for di in -1..=1 {
                let (a, b) = (ci + di, cj + dj);
                if (-1..=TILE_CELLS as i32).contains(&a) && (-1..=TILE_CELLS as i32).contains(&b) {
                    let c = &cols[(b + 1) as usize * nc + (a + 1) as usize];
                    lo = lo.min(c.bed_k);
                    hi = hi.max(c.top_k);
                }
            }
        }
        (lo - 1, hi + 1)
    };
    if any_3d {
        if let Some(o) = &t.overhang {
            let (lo, hi) = o.layers(t);
            kmin = kmin.min(lo - 1);
            kmax = kmax.max(hi + 1);
        }
        if let Some(vx) = &t.voxels {
            for c in &cols {
                if let Some((lo, hi)) = vx.layers_in(c.face, c.i.div_euclid(BLOCK), c.j.div_euclid(BLOCK)) {
                    kmin = kmin.min(lo - 1);
                    kmax = kmax.max(hi + 1);
                }
            }
        }
    }
    // Grottes : régions qui touchent la tuile, rassemblées une fois
    let regions: Vec<Arc<Region>> = match &t.caves {
        Some(caves) => {
            let dirs = [corner(0, 0), corner(TILE_CELLS, 0), corner(0, TILE_CELLS), corner(TILE_CELLS, TILE_CELLS), tile_dir];
            caves.for_tile(&dirs, &|d| t.surface_r(d), &t.cave_windows())
        }
        None => Vec::new(),
    };
    let dirs = [corner(0, 0), corner(TILE_CELLS, 0), corner(0, TILE_CELLS), corner(TILE_CELLS, TILE_CELLS), tile_dir];
    let rock_pieces: Vec<Piece> = t.rocks.as_ref().map_or(Vec::new(), |r| r.for_tile(&dirs, &|d| t.rock_ground(d)));
    // Pour chaque colonne : les régions dont la sphère englobante croise la colonne, et les
    // couches concernées (le reste de la colonne est de la roche pleine)
    // Seulement les pièces dans le cône de la tuile
    let cone = key.arc(t.params.radius) * 0.75 / t.params.radius + 2.0 * v / t.params.radius;
    let pieces: Vec<Piece> = regions
        .iter()
        .filter(|r| {
            let (bc, br) = r.bound();
            bc.angle_between(tile_dir) < cone + (br + 2.0 * v) / bc.length().max(1.0)
        })
        .flat_map(|r| r.pieces().collect::<Vec<_>>())
        .filter(|piece| {
            let (bc, br) = piece.bound();
            let len = bc.length().max(1.0);
            bc.angle_between(tile_dir) < cone + (br + 2.0 * v) / len
        })
        .chain(rock_pieces)
        .collect();
    let mut col_pieces: Vec<Vec<(i32, i32, Piece)>> = Vec::with_capacity(cols.len());
    let mut col_spans: Vec<Vec<(i32, i32)>> = Vec::with_capacity(cols.len());
    let margin = v * 1.5;
    for c in &cols {
        let mut list = Vec::new();
        let mut spans = Vec::new();
        for piece in &pieces {
            let (bc, br) = piece.bound();
            let br = br + margin;
            let along = bc.dot(c.base.dir);
            let perp2 = bc.length_squared() - along * along;
            if along > 0.0 && perp2 < br * br {
                let half = (br * br - perp2).sqrt();
                // (les arches dépassent du sol ; les grottes restent dessous)
                let top = if matches!(piece, Piece::Add(_)) { t.layer(along + half) + 1 } else { (t.layer(along + half) + 1).min(c.top_k) };
                let span = (t.layer(along - half) - 1, top);
                if span.0 <= span.1 {
                    list.push((span.0, span.1, *piece));
                    // Seules les pièces creusées ouvrent des faces à dessiner
                    if !matches!(piece, Piece::Fill(_)) {
                        spans.push(span);
                    }
                }
            }
        }
        col_pieces.push(list);
        col_spans.push(spans);
    }
    let index = |ci: i32, cj: i32| (cj + 1) as usize * nc + (ci + 1) as usize;
    let compute = |ci: i32, cj: i32, k: i32| -> VoxelType {
        let c = col(ci, cj);
        let list = &col_pieces[index(ci, cj)];
        if !any_3d && list.is_empty() && k >= c.top_k {
            return VoxelType::Air;
        }
        t.kind_in(c.face, c.i, c.j, k, c.base.dir, &c.base, c.top_k, Some(list))
    };
    // Couches à examiner dans une colonne : autour du sol, plus là où passent les grottes
    let ranges = |ci: i32, cj: i32| -> Vec<(i32, i32)> {
        let c = col(ci, cj);
        let mut out = if any_3d {
            vec![(kmin.min(c.bed_k - 1), kmax)]
        } else if c.base.is_sea() {
            // Eau profonde : le fond et la surface, pas les couches d'eau entre les deux
            let (lo, _) = near(ci, cj);
            vec![(lo, c.bed_k + 1), (c.top_k - 1, c.top_k + 1)]
        } else {
            vec![near(ci, cj)]
        };
        out.extend(col_spans[index(ci, cj)].iter().copied());
        out.sort();
        let mut merged: Vec<(i32, i32)> = Vec::new();
        for (a, b) in out {
            match merged.last_mut() {
                Some(last) if a <= last.1 + 1 => last.1 = last.1.max(b),
                _ => merged.push((a, b)),
            }
        }
        merged
    };
    // Chaque cellule n'est calculée qu'une fois : matières de toutes les colonnes (bordure
    // comprise) sur leurs couches examinées, un voxel de marge
    let mut memo: Vec<Vec<(i32, Vec<VoxelType>)>> = Vec::with_capacity(cols.len());
    for cj in -1..=TILE_CELLS as i32 {
        for ci in -1..=TILE_CELLS as i32 {
            memo.push(ranges(ci, cj).into_iter().map(|(a, b)| (a - 1, (a - 1..=b + 1).map(|k| compute(ci, cj, k)).collect())).collect());
        }
    }
    let kind = |ci: i32, cj: i32, k: i32| -> VoxelType {
        for (k0, list) in &memo[index(ci, cj)] {
            if k >= *k0 && ((k - k0) as usize) < list.len() {
                return list[(k - k0) as usize];
            }
        }
        compute(ci, cj, k)
    };

    let mut buf = MeshBuf::default();
    buf.pos.reserve(TILE_CELLS * TILE_CELLS * 8);
    let last = TILE_CELLS as i32 - 1;
    let skirt = v * 8.0;
    let shade = |c: [f32; 4], k: f32| [c[0] * k, c[1] * k, c[2] * k, c[3]];
    // Vide pour le maillage solide : l'air et les liquides transparents (leur surface est à part)
    let open = |ci: i32, cj: i32, k: i32| {
        let kind = kind(ci, cj, k);
        kind == VoxelType::Air || (kind.is_clear_liquid() && col(ci, cj).base.is_sea())
    };

    for cj in 0..TILE_CELLS as i32 {
        for ci in 0..TILE_CELLS as i32 {
            let c = col(ci, cj);
            let (u, w) = (ci as usize, cj as usize);
            let up = c.base.dir;
            for k in ranges(ci, cj).into_iter().flat_map(|(a, b)| a..=b) {
                let here = kind(ci, cj, k);
                if open(ci, cj, k) {
                    continue;
                }
                let color = submerged(cell_color(here, k, c), c.base.is_sea());
                let (r0, r1) = (t.layer_radius(k), t.layer_radius(k + 1));
                if open(ci, cj, k + 1) {
                    buf.quad([corner(u, w) * r1, corner(u + 1, w) * r1, corner(u + 1, w + 1) * r1, corner(u, w + 1) * r1], up, color);
                }
                if open(ci, cj, k - 1) {
                    buf.quad([corner(u, w) * r0, corner(u + 1, w) * r0, corner(u + 1, w + 1) * r0, corner(u, w + 1) * r0], -up, shade(color, 0.55));
                }
                let sides: [((i32, i32), (usize, usize), (usize, usize)); 4] = [
                    ((1, 0), (u + 1, w), (u + 1, w + 1)),
                    ((-1, 0), (u, w), (u, w + 1)),
                    ((0, 1), (u, w + 1), (u + 1, w + 1)),
                    ((0, -1), (u, w), (u + 1, w)),
                ];
                for ((dx, dy), ea, eb) in sides {
                    if !open(ci + dx, cj + dy, k) {
                        continue;
                    }
                    let (a, b) = (corner(ea.0, ea.1), corner(eb.0, eb.1));
                    let nb = col(ci + dx, cj + dy);
                    let mut nrm = (b - a).cross(up).normalize_or_zero();
                    if nrm.dot(nb.base.dir - up) < 0.0 {
                        nrm = -nrm;
                    }
                    // Paroi sous la couche du dessus : strates de la roche (0.13 T3)
                    let ground = c.base.ground_view();
                    let side = if k < c.bed_k - 1 && matches!(here, VoxelType::Stone | VoxelType::Basalt) || (k < c.bed_k - 1 && here == ground.kind) {
                        strata_color(ground.color, k, t.params.seed, !t.params.airless && t.params.atmosphere)
                    } else {
                        color
                    };
                    buf.quad([a * r0, b * r0, b * r1, a * r1], nrm, shade(submerged(side, c.base.is_sea() || nb.base.is_sea()), 0.82));
                }
            }
            // Jupes au bord de la tuile (raccord avec une voisine plus grossière)
            let sides: [((i32, i32), (usize, usize), (usize, usize)); 4] = [
                ((1, 0), (u + 1, w), (u + 1, w + 1)),
                ((-1, 0), (u, w), (u, w + 1)),
                ((0, 1), (u, w + 1), (u + 1, w + 1)),
                ((0, -1), (u, w), (u + 1, w)),
            ];
            for ((dx, dy), ea, eb) in sides {
                let (ni, nj) = (ci + dx, cj + dy);
                if !(ni < 0 || ni > last || nj < 0 || nj > last) {
                    continue;
                }
                let nb = col(ni, nj);
                let hi = c.base.bed.min(nb.base.bed);
                let lo = hi - skirt;
                let (a, b) = (corner(ea.0, ea.1), corner(eb.0, eb.1));
                let mut nrm = (b - a).cross(up).normalize_or_zero();
                if nrm.dot(nb.base.dir - up) < 0.0 {
                    nrm = -nrm;
                }
                buf.quad([a * lo, b * lo, b * hi, a * hi], nrm, shade(submerged(c.base.ground_view().color, c.base.is_sea()), 0.82));
            }
        }
    }
    // Surface de l'eau : colonnes de mer (le niveau est celui de la colonne), plus l'eau des
    // grottes dont le dessus touche l'air
    let water = water_mesh(&cols, nc, &corners, n1, (i0, j0, 1), |c| &c.base);
    (buf.into_mesh(), water)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use bevy::render::mesh::VertexAttributeValues;

    pub(crate) fn earth_like() -> BodyParams {
        BodyParams {
            airless: false,
            atmosphere: true,
            radius: 9000.0,
            sea_level: 0.4,
            terrain_height: 360.0,
            seed: 1234,
            noise_scale: 2.0,
            detail_scale: 4.0,
            temperature: 15.0,
            gravity: 1.0,
            gaseous: false,
            climate: Climate::default(),
            sky: EARTH_SKY,
            sunset: EARTH_SUNSET,
            haze: EARTH_SKY,
            pressure: 1.0,
            hydro: Hydro::default(),
            relief: Relief::default(),
            biomes: BiomeParams::default(),
            asteroid: None,
            tide: Default::default(),
            geo: Default::default(),
            plasma: crate::approche::PLASMA_DEFAULT,
        }
    }

    #[test]
    fn faces_round_trip() {
        for face in 0..6u8 {
            for &(s, t) in &[(0.0, 0.0), (0.7, -0.3), (-0.95, 0.95), (0.2, 0.9)] {
                let d = face_dir(face, s, t);
                let (f2, s2, t2) = dir_to_face(d);
                assert_eq!(face, f2, "face {face} ({s},{t})");
                assert!((s - s2).abs() < 1e-4 && (t - t2).abs() < 1e-4, "face {face}: ({s},{t}) -> ({s2},{t2})");
            }
        }
    }

    #[test]
    fn voxel_size_stays_in_range() {
        let max = MAX_VOXEL / GROUND_SCALE as f32;
        for r in [120.0, 300.0, 500.0, 1_000.0, 1_500.0, 3_000.0, 6_000.0, 13_000.0] {
            let l = layout_scaled(r, GROUND_SCALE);
            assert!(l.voxel > max * 0.4 && l.voxel <= max, "rayon {r} : voxel {}", l.voxel);
        }
    }

    #[test]
    fn selection_refines_around_the_camera() {
        let p = earth_like();
        let t = Terrain::new(p);
        let dir = Vec3::new(0.3, 0.8, 0.5).normalize();
        let cam = dir * (t.ground(dir).top + 15.0);
        let mut tiles = Vec::new();
        select_tiles(t.layout, t.ground(dir).top, cam, &mut tiles);
        assert!(tiles.len() < 600, "{} tuiles", tiles.len());
        // Le niveau le plus fin existe sous la caméra...
        let (face, s, tt) = dir_to_face(dir);
        let finest = tiles.iter().filter(|k| k.depth as u32 == t.layout.max_depth && k.face == face).count();
        assert!(finest > 0);
        let n = (1u32 << t.layout.max_depth) as f32;
        let under = TileKey {
            face,
            depth: t.layout.max_depth as u8,
            x: (((s + 1.0) * 0.5 * n) as u32).min(n as u32 - 1),
            y: (((tt + 1.0) * 0.5 * n) as u32).min(n as u32 - 1),
        };
        assert!(tiles.contains(&under));
        // ...et les tuiles ne se chevauchent pas
        for a in &tiles {
            for b in &tiles {
                assert!(!a.is_ancestor_of(*b));
            }
        }
    }

    #[test]
    fn far_camera_uses_only_coarse_tiles() {
        let p = earth_like();
        let l = p.layout();
        let mut tiles = Vec::new();
        select_tiles(l, p.radius, Vec3::new(0.0, 4.0 * p.radius, 0.0), &mut tiles);
        assert!(tiles.len() <= 24, "{} tuiles", tiles.len());
    }

    #[test]
    fn tile_mesh_is_well_formed() {
        let p = earth_like();
        let t = Terrain::new(p);
        let dir = Vec3::new(0.1, 0.9, 0.2).normalize();
        let (face, s, tt) = dir_to_face(dir);
        for depth in [0u32, 2, t.layout.max_depth] {
            let n = (1u32 << depth) as f32;
            let key = TileKey {
                face,
                depth: depth as u8,
                x: (((s + 1.0) * 0.5 * n) as u32).min(n as u32 - 1),
                y: (((tt + 1.0) * 0.5 * n) as u32).min(n as u32 - 1),
            };
            let mesh = build_tile_mesh(&p, key);
            let Some(VertexAttributeValues::Float32x3(pos)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else { panic!() };
            let Some(VertexAttributeValues::Float32x3(nor)) = mesh.attribute(Mesh::ATTRIBUTE_NORMAL) else { panic!() };
            let Some(Indices::U32(idx)) = mesh.indices() else { panic!() };
            assert!(pos.len() >= TILE_CELLS * TILE_CELLS * 4);
            assert_eq!(idx.len() % 3, 0);
            let mut up_facing = 0;
            for tri in idx.chunks(3) {
                let v = |i: u32| Vec3::from_array(pos[i as usize]);
                let n = Vec3::from_array(nor[tri[0] as usize]);
                let geo = (v(tri[1]) - v(tri[0])).cross(v(tri[2]) - v(tri[0]));
                assert!(geo.is_finite() && geo.length() > 0.0, "triangle dégénéré");
                // Face avant du côté de la normale : sinon la surface serait invisible
                assert!(geo.dot(n) > 0.0, "triangle à l'envers (profondeur {depth})");
                if n.dot(v(tri[0]).normalize()) > 0.99 {
                    up_facing += 1;
                }
            }
            assert!(up_facing >= TILE_CELLS * TILE_CELLS * 2, "faces supérieures : {up_facing}");
            for v in pos {
                let r = Vec3::from_array(*v).length();
                assert!(r > p.radius * 0.5 && r < p.radius + 2000.0, "sommet hors de la planète : {r}");
            }
        }
    }

    #[test]
    fn ground_matches_the_finest_mesh() {
        let p = earth_like();
        let mut t = Terrain::new(p);
        t.caves = None;
        t.overhang = None;
        let dir = Vec3::new(-0.4, 0.3, 0.86).normalize();
        let (face, s, tt) = dir_to_face(dir);
        let n = (1u32 << t.layout.max_depth) as f32;
        let key = TileKey {
            face,
            depth: t.layout.max_depth as u8,
            x: (((s + 1.0) * 0.5 * n) as u32).min(n as u32 - 1),
            y: (((tt + 1.0) * 0.5 * n) as u32).min(n as u32 - 1),
        };
        let mesh = build_tile_mesh_with(&t, key);
        let Some(VertexAttributeValues::Float32x3(pos)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else { panic!() };
        // Le sommet de face supérieure le plus proche de `dir` est à la hauteur de `ground`
        let ground = t.ground(dir).top;
        let best = pos
            .iter()
            .map(|p| Vec3::from_array(*p))
            .min_by(|a, b| a.normalize().distance(dir).total_cmp(&b.normalize().distance(dir)))
            .unwrap();
        assert!((best.length() - ground).abs() <= t.voxel() * 8.0 + 1.0, "{} vs {}", best.length(), ground);
        assert!(ground > p.radius - 1.0 && ground < p.radius + p.terrain_height * 3.0);
    }

    /// Feuille la plus fine qui contient `dir`.
    fn finest(t: &Terrain, dir: Vec3) -> TileKey {
        let (face, s, tt) = dir_to_face(dir);
        let n = (1u32 << t.layout.max_depth) as f32;
        TileKey {
            face,
            depth: t.layout.max_depth as u8,
            x: (((s + 1.0) * 0.5 * n) as u32).min(n as u32 - 1),
            y: (((tt + 1.0) * 0.5 * n) as u32).min(n as u32 - 1),
        }
    }

    /// Dessus (rayons des sommets tournés vers le haut, arrondis) d'un maillage.
    fn tops(mesh: &Mesh) -> Vec<i64> {
        let Some(VertexAttributeValues::Float32x3(pos)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else { panic!() };
        let Some(VertexAttributeValues::Float32x3(nor)) = mesh.attribute(Mesh::ATTRIBUTE_NORMAL) else { panic!() };
        let mut out: Vec<i64> = pos
            .iter()
            .zip(nor)
            .filter(|(p, n)| Vec3::from_array(**n).dot(Vec3::from_array(**p).normalize()) > 0.999)
            .map(|(p, _)| (Vec3::from_array(*p).length() * 100.0).round() as i64)
            .collect();
        out.sort();
        out
    }

    /// Règle 11, raccord sur les 6 faces : sans forme 3D, une tuile en voxels 3D a exactement les
    /// mêmes dessus que le champ de hauteur (tuiles lointaines), et ses triangles sont à l'endroit.
    #[test]
    fn voxel_tiles_match_the_height_field_on_all_six_faces() {
        let mut p = earth_like();
        p.seed = 77;
        let mut t = Terrain::new(p);
        t.overhang = None;
        t.caves = None;
        t.rocks = None;
        for face in 0..6u8 {
            for (s, tt) in [(0.1, -0.2), (0.999, 0.3), (-0.999, -0.999)] {
                let key = finest(&t, face_dir(face, s, tt));
                let height = build_height_tile_mesh(&t, key).0;
                let voxel = build_voxel_tile_mesh(&t, key).0;
                assert_eq!(tops(&height), tops(&voxel), "face {face} ({s}, {tt})");
                let Some(VertexAttributeValues::Float32x3(pos)) = voxel.attribute(Mesh::ATTRIBUTE_POSITION) else { panic!() };
                let Some(VertexAttributeValues::Float32x3(nor)) = voxel.attribute(Mesh::ATTRIBUTE_NORMAL) else { panic!() };
                let Some(Indices::U32(idx)) = voxel.indices() else { panic!() };
                for tri in idx.chunks(3) {
                    let v = |i: u32| Vec3::from_array(pos[i as usize]);
                    let geo = (v(tri[1]) - v(tri[0])).cross(v(tri[2]) - v(tri[0]));
                    assert!(geo.dot(Vec3::from_array(nor[tri[0] as usize])) > 0.0, "triangle a l'envers");
                }
            }
        }
    }

    /// Les colonnes de part et d'autre d'une arête du cube sont les mêmes pour les deux faces
    /// (une voisine au-delà du bord est rapportée à sa vraie face) : pas de fissure entre faces.
    #[test]
    fn cells_agree_across_the_cube_edges() {
        let t = Terrain::new(earth_like());
        let n = t.lattice();
        for face in 0..6u8 {
            for &(gi, gj) in &[(-1i64, n / 3), (n, n / 2), (n / 4, -1), (n / 5, n)] {
                let dir = t.cell_dir(face, gi, gj);
                let (f2, i2, j2) = t.cell_of(dir);
                assert_ne!(f2, face, "face {face} ({gi}, {gj})");
                // La colonne voisine canonique est bien à un voxel du bord
                let back = t.cell_dir(f2, i2, j2);
                // (distance des points : `angle_between` passe par acos, imprécis pour les petits angles)
                assert!((back - dir).length() * t.params.radius < t.voxel() * 0.75, "face {face} ({gi}, {gj})");
            }
        }
    }

    /// La même graine donne les mêmes voxels (deux machines, deux constructions).
    #[test]
    fn the_density_is_deterministic() {
        let a = Terrain::new(earth_like());
        let b = Terrain::new(earth_like());
        let o = a.overhang.expect("une arche sur la terre ferme");
        let key = finest(&a, o.dir);
        assert_eq!(tops(&build_voxel_tile_mesh(&a, key).0), tops(&build_voxel_tile_mesh(&b, key).0));
        let mut h = 0u64;
        let (face, i, j) = a.cell_of(o.dir);
        for di in -15..15 {
            for k in -10..30 {
                let dir = a.cell_dir(face, i + di, j);
                let (base, top_k) = a.base_cell_column(dir);
                let ka = a.kind_at(face, i + di, j, k, dir, &base, top_k);
                let kb = b.kind_at(face, i + di, j, k, dir, &base, top_k);
                assert_eq!(ka, kb);
                h = h.wrapping_mul(31).wrapping_add(ka as u64);
            }
        }
        assert_ne!(h, 0);
    }

    /// Le surplomb : de l'air sous une roche (vraie 3D), un plafond pour qui est dessous, un sol
    /// pour qui est dessus, et une face du dessous dans le maillage.
    #[test]
    fn the_test_overhang_has_air_under_rock() {
        let t = Terrain::new(earth_like());
        let o = t.overhang.expect("arche");
        let under = o.visit_dir();
        let floor = t.floor(under, o.base + t.voxel() * 3.0);
        let roof = t.ceiling(under, floor.top + 0.01);
        assert!(roof.is_finite() && roof > floor.top + t.voxel() * 5.0, "plafond {roof}, sol {}", floor.top);
        // Dessus de l'auvent : on y tient debout
        let top = t.ground(under);
        assert!(top.top > roof, "dessus {} plafond {roof}", top.top);
        assert_eq!(top.kind, VoxelType::Stone);
        // Le maillage a des faces tournées vers le bas (dessous de l'auvent)
        let mesh = build_voxel_tile_mesh(&t, finest(&t, under)).0;
        let Some(VertexAttributeValues::Float32x3(pos)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else { panic!() };
        let Some(VertexAttributeValues::Float32x3(nor)) = mesh.attribute(Mesh::ATTRIBUTE_NORMAL) else { panic!() };
        let down = pos.iter().zip(nor).filter(|(p, n)| Vec3::from_array(**n).dot(Vec3::from_array(**p).normalize()) < -0.999).count();
        assert!(down > 0, "pas de dessous");
        // Loin de l'arche, pas de plafond
        assert_eq!(t.ceiling(-o.dir, t.ground(-o.dir).top + 1.0), f32::INFINITY);
    }

    /// B2 : des grottes creusées dans la roche (air sous la surface), avec des entrées, et rien
    /// au-delà de 2 000 unités de profondeur.
    #[test]
    fn caves_are_carved_under_the_surface() {
        let t = Terrain::new(earth_like());
        let caves = t.caves.clone().expect("grottes");
        let surf = |d: Vec3| t.surface_r(d);
        let mut carved = 0;
        let mut entrance = None;
        for x in -8..8 {
            for z in -8..8 {
                let d = Vec3::new(x as f32 * 0.02, 1.0, z as f32 * 0.02).normalize();
                for depth in [1.0, 3.0, 6.0] {
                    let p = d * (surf(d) - depth * caves.size);
                    if let Some(r) = caves.region(caves.key_of(p), &surf) {
                        if let Some((rc, _)) = r.room {
                            let (face, i, j) = t.cell_of(rc.normalize());
                            let center = t.cell_dir(face, i, j);
                            let (base, top_k) = t.base_cell_column(center);
                            if !base.kind.is_liquid() && t.kind_at(face, i, j, t.layer(rc.length()), center, &base, top_k) == VoxelType::Air {
                                carved += 1;
                            }
                        }
                        entrance = entrance.or(r.entrance);
                    }
                }
            }
        }
        assert!(carved > 0, "aucune salle creusee");
        // Une entrée : au bord du puits, le sol descend bien plus bas que la surface
        if let Some(e) = entrance {
            let dir = e.normalize();
            let top = t.base_column(dir, t.voxel()).top;
            if !t.base_column(dir, t.voxel()).kind.is_liquid() {
                assert!(t.ground(dir).top < top - 2.0 * t.voxel(), "entree bouchee : {} vs {top}", t.ground(dir).top);
            }
        }
        // Très profond : de la roche
        let d = Vec3::Y;
        let (face, i, j) = t.cell_of(d);
        let center = t.cell_dir(face, i, j);
        let (base, top_k) = t.base_cell_column(center);
        let deep = t.layer(base.top - crate::caves::MAX_DEPTH - 2.0 * caves.size);
        assert_ne!(t.kind_at(face, i, j, deep, center, &base, top_k), VoxelType::Air);
    }

    /// B3 : des arches et des cheminées de fée posées sur le sol, avec de l'air dessous (vraie 3D),
    /// et leur dessus pris pour le sol.
    #[test]
    fn natural_arches_stand_on_the_ground() {
        let mut p = earth_like();
        p.relief = Relief { seed: 5, plates: 8, mountains: 0.25, terraces: 0.8, ..Default::default() };
        let t = Terrain::new(p);
        let rocks = t.rocks.clone().expect("arches (planete avec de l'air)");
        let mut checked = 0;
        for k in 0..400 {
            let a = k as f32 * 0.37;
            let dir = Vec3::new(a.cos() * 0.6, 0.5 + 0.3 * (a * 0.7).sin(), a.sin() * 0.6).normalize();
            for piece in rocks.pieces_near(dir, &|d| t.rock_ground(d)).iter() {
                let Piece::Add(Shape::Capsule(a, b, _)) = piece else { continue };
                // Le point le plus haut d'une capsule au-dessus du sol : de la roche, et le sol
                // de la colonne est au moins à sa hauteur
                let top = if a.length() > b.length() { *a } else { *b };
                let d = top.normalize();
                let base = t.base_column(d, t.voxel()).top;
                if top.length() < base + 4.0 * t.voxel() {
                    continue;
                }
                assert!(t.ground(d).top >= top.length() - 2.0 * t.voxel(), "arche non prise pour le sol");
                checked += 1;
            }
            if checked > 3 {
                break;
            }
        }
        assert!(checked > 0, "aucune arche trouvee");
    }

    /// T2 : les formes du relief sont de vraies cellules 3D. Les pièces posées (corniches, pitons,
    /// blocs, ponts) sont de la roche au-dessus du sol, les pièces creusées (gorges, grottes des
    /// falaises, strates, dessous des corniches) de l'air ou de l'eau sous le sol ; et le sol pris
    /// pour les collisions (`floor`) est le dessus de la roche posée.
    #[test]
    fn cliff_forms_are_real_voxels() {
        let mut p = earth_like();
        p.relief = Relief { seed: 5, plates: 8, mountains: 0.4, terraces: 0.6, canyons: 0.1, ..Default::default() };
        let t = Terrain::new(p);
        let rocks = t.rocks.clone().expect("formes du relief");
        let v = t.voxel();
        let (mut added, mut carved, mut floors) = (0, 0, 0);
        let mut seen = std::collections::HashSet::new();
        for n in 0..3000 {
            let a = n as f32 * 0.37;
            let dir = Vec3::new(a.cos() * 0.6, 0.5 + 0.4 * (a * 0.13).sin(), a.sin() * 0.6).normalize();
            let pieces = rocks.pieces_near(dir, &|d| t.rock_ground(d));
            for piece in pieces.iter() {
                let (c, _) = piece.bound();
                if !seen.insert(c.to_array().map(f32::to_bits)) {
                    continue;
                }
                let d = c.normalize();
                let (face, i, j) = t.cell_of(d);
                let center = t.cell_dir(face, i, j);
                let (base, top_k) = t.base_cell_column(center);
                let p = center * c.length();
                let k = t.layer(c.length());
                let inside = |q: &Piece| match q {
                    Piece::Add(s) | Piece::Carve(s, _) => s.contains(p),
                    _ => false,
                };
                if !inside(piece) || base.kind.is_liquid() {
                    continue;
                }
                let kind = t.kind_at(face, i, j, k, center, &base, top_k);
                match piece {
                    Piece::Add(_) if k >= top_k => {
                        assert_eq!(kind, VoxelType::Stone, "roche posee absente");
                        added += 1;
                        // Juste au-dessus de la roche posée, le sol est au moins à sa hauteur
                        if floors < 20 && t.kind_at(face, i, j, k + 1, center, &base, top_k) == VoxelType::Air {
                            let f = t.floor(center, t.layer_radius(k + 1) + 0.5 * v);
                            assert!(f.top >= t.layer_radius(k + 1) - 0.01 * v, "sol sous la roche posee {} < {}", f.top, t.layer_radius(k + 1));
                            floors += 1;
                        }
                    }
                    Piece::Carve(..) if k < top_k && !pieces.iter().any(|q| matches!(q, Piece::Add(s) if s.contains(p))) => {
                        assert!(matches!(kind, VoxelType::Air | VoxelType::Water), "roche non creusee : {kind:?}");
                        carved += 1;
                    }
                    _ => {}
                }
            }
            if added > 30 && carved > 30 {
                break;
            }
        }
        eprintln!("pose {added}, creuse {carved}, sols {floors}");
        assert!(added > 5 && carved > 5 && floors > 0, "pose {added}, creuse {carved}, sols {floors}");
    }

    /// `/relief` : chaque forme repérée existe là où on la montre (roche posée au point visé ou
    /// air creusé), sur un monde avec air et sur un monde nu.
    #[test]
    fn relief_marks_point_at_real_forms() {
        use crate::rocks::Feature;
        for airless in [false, true] {
            let mut p = earth_like();
            p.airless = airless;
            p.atmosphere = !airless;
            p.relief = Relief { seed: 5, plates: 8, mountains: 0.4, terraces: 0.6, canyons: 0.1, ..Default::default() };
            let t = Terrain::new(p);
            let rocks = t.rocks.clone().expect("formes du relief");
            let mut found = Vec::new();
            for f in Feature::ALL {
                let Some((_, _, look)) = rocks.nearest(Vec3::new(0.3, 0.8, 0.2).normalize(), Some(f), 20, &|d| t.rock_ground(d)) else { continue };
                let d = look.normalize();
                let (face, i, j) = t.cell_of(d);
                let center = t.cell_dir(face, i, j);
                let (base, top_k) = t.base_cell_column(center);
                let k = t.layer(look.length());
                let kind = t.kind_at(face, i, j, k, center, &base, top_k);
                let solid = matches!(f, Feature::Pinnacle | Feature::Boulders | Feature::Bridge | Feature::Arch | Feature::Hoodoo);
                if f != Feature::Ledge && f != Feature::Strata && f != Feature::Boulders {
                    assert_eq!(kind != VoxelType::Air && kind != VoxelType::Water, solid, "{} ({airless}) : {kind:?}", f.name());
                }
                found.push(f.name());
            }
            eprintln!("air {}: {found:?}", !airless);
            assert!(found.contains(&"piton") && found.contains(&"blocs") && found.contains(&"corniche"));
            assert_eq!(found.contains(&"gorge"), !airless);
        }
    }

    /// T3 : roche nue sur les pentes fortes (plus tôt pour le sable, plus tard pour la neige),
    /// mousse au pied des parois humides, strates par couche, taches de quelques voxels au sol.
    #[test]
    fn ground_colors_follow_slope_and_scale() {
        let dist = |a: [f32; 4], b: [f32; 4]| (0..3).map(|i| (a[i] - b[i]).abs()).sum::<f32>();
        let g = VoxelType::Grass.color();
        assert_eq!(ground_tint(VoxelType::Grass, g, 0.3, 0.0, true), g);
        assert!(dist(ground_tint(VoxelType::Grass, g, 1.2, 0.0, true), rock_of(g)) < 1e-4);
        let sand = VoxelType::Sand.color();
        assert!(dist(ground_tint(VoxelType::Sand, sand, 0.7, 0.0, true), rock_of(sand)) < 1e-4, "plage en pente = roche");
        let snow = VoxelType::Snow.color();
        assert_eq!(ground_tint(VoxelType::Snow, snow, 1.2, 0.0, true), snow, "neige sur les pentes moyennes");
        assert!(dist(ground_tint(VoxelType::Grass, g, 0.2, 5.0, true), g) > 0.05, "mousse");
        assert_eq!(ground_tint(VoxelType::Grass, g, 0.2, 5.0, false), g);
        let w = VoxelType::Water.color();
        assert_eq!(ground_tint(VoxelType::Water, w, 3.0, 9.0, true), w);
        // Strates : des bandes différentes d'une couche à l'autre
        let bands: std::collections::HashSet<_> = (0..30).map(|k| strata_color(g, k, 7, true).map(f32::to_bits)).collect();
        assert!(bands.len() > 4, "{}", bands.len());
        // Taches de 5 voxels : deux colonnes à 3 voxels l'une de l'autre n'ont pas la même couleur
        let t = Terrain::new(earth_like());
        let v = t.voxel();
        let mut differ = 0;
        for k in 0..40 {
            let d = Vec3::new(0.3 + k as f32 * 0.01, 0.8, 0.2).normalize();
            let e = (d + Vec3::Y.cross(d).normalize() * 3.0 * v / t.params.radius).normalize();
            let (a, b) = (t.base_column(d, v), t.base_column(e, v));
            if a.kind == b.kind && dist(a.color, b.color) > 0.01 {
                differ += 1;
            }
        }
        assert!(differ > 20, "{differ}");
    }

    /// T4 : en Ultra, plus de tuiles fines autour de la caméra ; rien n'est découpé derrière
    /// l'horizon (moins de tuiles qu'en découpant tout), et la surface reste couverte.
    #[test]
    fn detail_reaches_the_horizon_but_not_beyond() {
        let t = Terrain::new(earth_like());
        let dir = Vec3::new(0.2, 0.9, 0.3).normalize();
        let g = t.ground(dir).top;
        let cam = dir * (g + 3.0 * t.voxel());
        let count = |split: f32, relief: f32| {
            let mut out = Vec::new();
            select_tiles_with(t.layout, g, cam, split, relief, &mut out);
            let fine = out.iter().filter(|k| k.depth as u32 == t.layout.max_depth).count();
            // Couverture : l'aire des feuilles fait les 6 faces
            let area: f64 = out.iter().map(|k| 0.25f64.powi(k.depth as i32)).sum();
            assert!((area - 6.0).abs() < 1e-6, "{area}");
            (out.len(), fine)
        };
        let (n_low, fine_low) = count(SPLIT_FACTOR, t.relief_span());
        let (n_ultra, fine_ultra) = count(4.5, t.relief_span());
        let (n_all, _) = count(4.5, f32::INFINITY);
        eprintln!("bas {n_low} ({fine_low} fines), ultra {n_ultra} ({fine_ultra} fines), sans horizon {n_all}");
        assert!(fine_ultra > fine_low * 3, "{fine_low} {fine_ultra}");
        assert!(n_ultra < n_all, "{n_ultra} {n_all}");
    }

    /// Un delta (minage, 0.14) creuse bien une cellule : le sol descend d'un voxel.
    #[test]
    fn a_delta_digs_a_cell() {
        let t = Terrain::new(earth_like());
        let dir = -t.overhang.unwrap().dir;
        let g = t.ground(dir);
        let (face, i, j) = t.cell_of(dir);
        let mut body = BodyVoxels::default();
        body.set(Cell { face, i, j, k: t.layer(g.top) - 1 }, VoxelType::Air);
        let dug = Terrain::new(earth_like()).with_voxels(Some(Arc::new(body)));
        assert!((dug.ground(dir).top - (g.top - t.voxel())).abs() < 1e-3, "{} vs {}", dug.ground(dir).top, g.top);
    }

    #[test]
    fn gas_giants_have_no_ground_until_the_core() {
        let mut p = earth_like();
        p.gaseous = true;
        p.radius = 80_000.0;
        let t = Terrain::new(p);
        for dir in [Vec3::Y, Vec3::X, Vec3::new(1.0, -2.0, 0.5).normalize()] {
            assert_eq!(t.ground(dir).top, 80_000.0 * GAS_CORE);
        }
    }

    #[test]
    fn moons_are_gray_and_dry() {
        let mut p = earth_like();
        p.airless = true;
        p.atmosphere = false;
        p.radius = 3000.0;
        p.terrain_height = 135.0;
        let t = Terrain::new(p);
        for k in 0..50 {
            let a = k as f32 * 0.37;
            let c = t.ground(Vec3::new(a.cos(), (a * 0.7).sin(), a.sin()).normalize());
            assert_ne!(c.kind, VoxelType::Water);
            assert!((c.color[0] - c.color[2]).abs() < 0.2, "pas gris : {:?}", c.color);
        }
    }
}

#[cfg(test)]
mod sea_level_tests {
    use super::*;
    use crate::planetgen::hydrology::sea_level_for;

    /// Le relief suit N(0,5 ; 0,09) : le niveau de la mer de `hydrology` donne bien la couverture
    /// océanique voulue, quelle que soit la graine ou l'échelle du bruit.
    #[test]
    fn the_sea_level_gives_the_wanted_ocean_fraction() {
        for (k, ns) in [1.5f32, 2.5, 3.5].into_iter().enumerate() {
            let mut heights = Vec::new();
            for seed in 0..6u32 {
                let t = Terrain::new(BodyParams { seed: seed * 7919 + k as u32, noise_scale: ns, ..params() });
                for i in 0..1500 {
                    let z = 1.0 - 2.0 * (i as f32 + 0.5) / 1500.0;
                    let a = i as f32 * 2.399_963;
                    let r = (1.0 - z * z).sqrt();
                    heights.push(t.raw_height(Vec3::new(r * a.cos(), z, r * a.sin())).1);
                }
            }
            for f in [0.1f32, 0.3, 0.5, 0.71, 0.9] {
                let level = sea_level_for(f as f64);
                let under = heights.iter().filter(|&&h| h < level).count() as f32 / heights.len() as f32;
                assert!((under - f).abs() < 0.05, "echelle {ns} : {f} voulu, {under} obtenu");
            }
        }
    }

    fn params() -> BodyParams {
        BodyParams {
            airless: false, atmosphere: true, radius: 9000.0, sea_level: 0.4, terrain_height: 360.0, seed: 1,
            noise_scale: 2.0, detail_scale: 4.0, temperature: 15.0, gravity: 1.0, gaseous: false,
            climate: crate::planetgen::climate::Climate::default(), sky: EARTH_SKY, sunset: EARTH_SUNSET, haze: EARTH_SKY, pressure: 1.0,
            hydro: Hydro::default(),
            relief: Relief::default(),
            biomes: BiomeParams::default(),
            asteroid: None,
            tide: Default::default(),
            geo: Default::default(),
            plasma: crate::approche::PLASMA_DEFAULT,
        }
    }
}

#[cfg(test)]
mod bench {
    use super::*;

    /// Temps de construction des tuiles (ignoré : `cargo test --release bench_tiles -- --ignored --nocapture`).
    #[test]
    #[ignore]
    fn bench_tiles() {
        let settings = crate::settings::GameSettings::default();
        let mut bodies = Vec::new();
        for sys in settings.systems.dense().iter().take(40) {
            for p in sys.planets() {
                if !p.gaseous() {
                    bodies.push(BodyParams::planet(p));
                }
            }
        }
        let start = std::time::Instant::now();
        let mut tiles = 0;
        for p in bodies.iter().take(30) {
            let t = Terrain::new(*p);
            let dir = Vec3::new(0.3, 0.7, -0.4).normalize();
            let mut keys = Vec::new();
            select_tiles(t.layout, t.ground(dir).top, dir * (t.ground(dir).top + 20.0), &mut keys);
            for key in keys.iter().take(40) {
                std::hint::black_box(build_tile_mesh(p, *key));
                tiles += 1;
            }
        }
        let ms = start.elapsed().as_secs_f64() * 1000.0;
        println!("BENCH {tiles} tuiles en {ms:.0} ms : {:.2} ms par tuile", ms / tiles as f64);
    }

    /// Tuiles du niveau le plus fin : champ de hauteur contre voxels 3D (`bench_tiles` aussi).
    #[test]
    #[ignore]
    fn bench_voxel_tiles() {
        let settings = crate::settings::GameSettings::default();
        let bodies: Vec<BodyParams> = settings.systems.dense().iter().take(40).flat_map(|s| s.planets().iter().filter(|p| !p.gaseous()).map(BodyParams::planet).collect::<Vec<_>>()).take(20).collect();
        let mut keys = Vec::new();
        for p in &bodies {
            let t = Terrain::new(*p);
            let dir = t.overhang.map_or(Vec3::Y, |o| o.dir);
            let mut sel = Vec::new();
            select_tiles(t.layout, t.ground(dir).top, dir * (t.ground(dir).top + 20.0), &mut sel);
            keys.extend(sel.into_iter().filter(|k| k.depth as u32 == t.layout.max_depth).take(20).map(|k| (t.params, k)));
        }
        // Un terrain par astre, comme dans le jeu (les tuiles partagent le cache des grottes)
        let terrains: Vec<Terrain> = keys.iter().map(|(p, _)| *p).collect::<Vec<_>>().chunks(20).map(|c| Terrain::new(c[0])).collect();
        for (name, voxel) in [("champ de hauteur", false), ("voxels 3D (1re fois)", true), ("voxels 3D", true)] {
            let start = std::time::Instant::now();
            for (n, (_, key)) in keys.iter().enumerate() {
                let t = &terrains[(n / 20).min(terrains.len() - 1)];
                std::hint::black_box(if voxel { build_voxel_tile_mesh(t, *key) } else { build_height_tile_mesh(t, *key) });
            }
            let ms = start.elapsed().as_secs_f64() * 1000.0;
            println!("BENCH {name} : {} tuiles fines, {:.2} ms par tuile", keys.len(), ms / keys.len() as f64);
        }
    }
}

#[cfg(test)]
mod geology_tests {
    use super::*;

    /// Le relief géologique (montagnes, rifts, cratères) ne déplace guère la couverture océanique
    /// voulue par l'hydrologie.
    #[test]
    fn relief_keeps_the_ocean_fraction() {
        let settings = crate::settings::GameSettings::default();
        let mut checked = 0;
        for sys in settings.systems.dense().iter().take(3000) {
            for p in sys.planets_uncached().iter().filter(|p| !p.gaseous() && p.hydrology.ocean_fraction > 0.05) {
                if checked >= 40 {
                    return;
                }
                checked += 1;
                let t = Terrain::new(BodyParams::planet(p));
                let n = 2000;
                let under = (0..n)
                    .filter(|&i| {
                        let z = 1.0 - 2.0 * (i as f32 + 0.5) / n as f32;
                        let a = i as f32 * 2.399_963;
                        let r = (1.0 - z * z).sqrt();
                        t.raw_height(Vec3::new(r * a.cos(), z, r * a.sin())).1 < p.sea_level
                    })
                    .count() as f32
                    / n as f32;
                assert!((under - p.hydrology.ocean_fraction).abs() < 0.1, "{under} au lieu de {}", p.hydrology.ocean_fraction);
            }
        }
        assert!(checked > 10);
    }
}

#[cfg(test)]
mod scale_study {
    use super::*;

    /// T1 : pentes réelles du sol (terres de planètes rocheuses), en degrés : part au-dessus de 15,
    /// 30, 45° (`cargo test --release slope_distribution -- --nocapture`).
    #[test]
    fn slope_distribution() {
        let settings = crate::settings::GameSettings::default();
        let bodies: Vec<BodyParams> = settings.systems.dense().iter().take(80).flat_map(|s| s.planets().iter().filter(|p| !p.gaseous()).map(BodyParams::planet).collect::<Vec<_>>()).take(12).collect();
        let mut slopes = Vec::new();
        for p in &bodies {
            let t = Terrain::new(*p);
            let v = t.voxel();
            for i in 0..400 {
                let a = i as f32 * 2.399;
                let z = 1.0 - 2.0 * (i as f32 + 0.5) / 400.0;
                let dir = Vec3::new(a.cos() * (1.0 - z * z).sqrt(), z, a.sin() * (1.0 - z * z).sqrt());
                let g = t.ground(dir);
                if g.kind.is_liquid() {
                    continue;
                }
                let east = Vec3::Y.cross(dir).normalize_or(Vec3::X);
                // Pente sur 4 voxels (les cubes font des marches d'un voxel)
                let g2 = t.ground((dir * p.radius + east * 4.0 * v).normalize());
                slopes.push(((g2.top - g.top) / (4.0 * v)).atan().to_degrees().abs());
            }
        }
        let share = |d: f32| slopes.iter().filter(|s| **s > d).count() as f32 / slopes.len().max(1) as f32 * 100.0;
        println!("PENTES {} points de terre : > 5 : {:.0} %, > 15 : {:.0} %, > 30 : {:.0} %, > 45 : {:.0} %, > 60 : {:.1} %", slopes.len(), share(5.0), share(15.0), share(30.0), share(45.0), share(60.0));
        assert!(share(15.0) > 10.0 && share(60.0) < 15.0);
    }

    /// Cache disque des tuiles (0.13 E3) : générer une tuile contre la relire d'un fichier
    /// (`cargo test --release bench_tile_cache -- --ignored --nocapture`).
    #[test]
    #[ignore]
    fn bench_tile_cache() {
        use bevy::render::mesh::VertexAttributeValues;
        let settings = crate::settings::GameSettings::default();
        let p = settings.systems.dense().iter().take(40).flat_map(|s| s.planets().iter().filter(|p| !p.gaseous()).map(BodyParams::planet).collect::<Vec<_>>()).next().unwrap();
        let t = Terrain::new(p);
        let dir = Vec3::new(0.3, 0.7, -0.4).normalize();
        let mut keys = Vec::new();
        select_tiles(t.layout, t.ground(dir).top, dir * (t.ground(dir).top + 2.0 * t.voxel()), &mut keys);
        let keys: Vec<TileKey> = keys.into_iter().filter(|k| k.depth as u32 >= t.layout.max_depth - 1).take(40).collect();
        let dir_tmp = std::env::temp_dir().join("spacespore_tile_cache_bench");
        let _ = std::fs::create_dir_all(&dir_tmp);
        let (mut gen_ms, mut write_ms, mut read_ms, mut bytes) = (0.0, 0.0, 0.0, 0usize);
        for (n, key) in keys.iter().enumerate() {
            let s0 = std::time::Instant::now();
            let mesh = build_tile_mesh_with(&t, *key);
            gen_ms += s0.elapsed().as_secs_f64() * 1000.0;
            // Encodage brut : positions, normales, couleurs, indices
            let mut buf: Vec<u8> = Vec::new();
            for attr in [Mesh::ATTRIBUTE_POSITION, Mesh::ATTRIBUTE_NORMAL, Mesh::ATTRIBUTE_COLOR] {
                match mesh.attribute(attr) {
                    Some(VertexAttributeValues::Float32x3(v)) => buf.extend(v.iter().flatten().flat_map(|f| f.to_le_bytes())),
                    Some(VertexAttributeValues::Float32x4(v)) => buf.extend(v.iter().flatten().flat_map(|f| f.to_le_bytes())),
                    _ => {}
                }
            }
            if let Some(bevy::render::mesh::Indices::U32(i)) = mesh.indices() {
                buf.extend(i.iter().flat_map(|x| x.to_le_bytes()));
            }
            bytes += buf.len();
            let path = dir_tmp.join(format!("{n}.bin"));
            let s1 = std::time::Instant::now();
            std::fs::write(&path, &buf).unwrap();
            write_ms += s1.elapsed().as_secs_f64() * 1000.0;
            let s2 = std::time::Instant::now();
            let back = std::fs::read(&path).unwrap();
            let floats: Vec<f32> = back.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
            std::hint::black_box(floats);
            read_ms += s2.elapsed().as_secs_f64() * 1000.0;
        }
        let n = keys.len() as f64;
        println!("CACHE {} tuiles fines : generer {:.2} ms, ecrire {:.2} ms, relire et decoder {:.2} ms par tuile, {:.0} Ko par tuile", keys.len(), gen_ms / n, write_ms / n, read_ms / n, bytes as f64 / n / 1024.0);
        let _ = std::fs::remove_dir_all(&dir_tmp);
    }

    /// Étude d'échelle (0.13 E1) : pour k = 1, 8, 16, 32, 64, sur des planètes rocheuses de taille
    /// différente, ce que coûte un atterrissage (`cargo test --release bench_scale -- --ignored
    /// --nocapture --test-threads=1`) : quadtree, tuiles à générer autour du marcheur, temps, mémoire
    /// des maillages, portée des tuiles fines, horizon, précision.
    #[test]
    #[ignore]
    fn bench_scale() {
        let settings = crate::settings::GameSettings::default();
        let mut bodies: Vec<BodyParams> = Vec::new();
        for sys in settings.systems.dense().iter().take(60) {
            for p in sys.planets() {
                if !p.gaseous() && p.radius_earth > 0.5 && p.radius_earth < 2.0 && bodies.len() < 6 {
                    bodies.push(BodyParams::planet(p));
                }
            }
        }
        println!("ETUDE {} planetes rocheuses, rayons {:?}", bodies.len(), bodies.iter().map(|b| b.radius.round()).collect::<Vec<_>>());
        for k in [1u32, 8, 16, 32, 64] {
            set_voxel_scale(k);
            let (mut tiles, mut fine, mut verts, mut ms, mut reach, mut horizon, mut rvox, mut voxel, mut depth, mut prec) = (0usize, 0usize, 0usize, 0.0f64, 0.0f32, 0.0f32, 0.0f32, 0.0f32, 0u32, 0.0f32);
            for p in &bodies {
                let t = Terrain::new(*p);
                let v = t.layout.voxel;
                let dir = Vec3::new(0.3, 0.7, -0.4).normalize();
                let eye = dir * (t.ground(dir).top + 1.8 * v);
                let mut keys = Vec::new();
                select_tiles(t.layout, t.ground(dir).top, eye, &mut keys);
                let start = std::time::Instant::now();
                for key in &keys {
                    let mesh = build_tile_mesh_with(&t, *key);
                    verts += mesh.count_vertices();
                }
                ms += start.elapsed().as_secs_f64() * 1000.0;
                tiles += keys.len();
                let finest: Vec<&TileKey> = keys.iter().filter(|k| k.depth as u32 == t.layout.max_depth).collect();
                fine += finest.len();
                reach += finest.iter().map(|k| (k.center_dir() * p.radius - eye).length() / v).fold(0.0, f32::max);
                let rv = p.radius / v;
                horizon += (2.0 * rv * 1.8).sqrt();
                rvox += rv;
                voxel += v;
                depth = depth.max(t.layout.max_depth);
                // Précision : écart entre deux f32 voisins au rayon de la planète, en voxels
                let r = p.radius;
                prec = prec.max((f32::from_bits(r.to_bits() + 1) - r) / v);
            }
            let n = bodies.len() as f32;
            println!(
                "ETUDE k={k:>2} | voxel {:.3} u | rayon {:>6.0} voxels | profondeur {depth:>2} | horizon {:>4.0} voxels | tuiles fines jusqu'a {:>4.0} voxels | {:>4.0} tuiles ({:.0} fines) par atterrissage | {:>6.0} ms ({:.2} ms/tuile) | maillages {:.0} Mo | precision f32 {:.4} voxel",
                voxel / n, rvox / n, horizon / n, reach / n, tiles as f32 / n, fine as f32 / n, ms / n as f64, ms / tiles as f64, verts as f64 * 40.0 / n as f64 / 1.0e6, prec
            );
        }
        set_voxel_scale(GROUND_SCALE);
    }
}
