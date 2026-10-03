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

/// Taille maximale d'un voxel au niveau le plus fin.
const MAX_VOXEL: f32 = 11.0;

/// Une tuile est subdivisée tant que la caméra est plus proche que ce multiple de sa taille.
pub const SPLIT_FACTOR: f32 = 1.8;

/// Longueurs d'onde maximales (unités) des collines moyennes et du relief fin ajoutés au relief du
/// corps ; elles rétrécissent avec le rayon pour les petites lunes.
const MID_WAVE: f32 = 1800.0;
const FINE_WAVE: f32 = 200.0;

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

pub fn layout_for(radius: f32) -> Layout {
    let arc = FRAC_PI_2 * radius;
    let mut depth = 0u32;
    while depth < 14 && arc / ((TILE_CELLS << depth) as f32) > MAX_VOXEL {
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

/// Tuiles (feuilles du quadtree) à afficher pour une caméra à `cam_local` (repère du corps).
pub fn select_tiles(layout: Layout, radius: f32, cam_local: Vec3, out: &mut Vec<TileKey>) {
    fn visit(key: TileKey, layout: Layout, radius: f32, cam: Vec3, out: &mut Vec<TileKey>) {
        if (key.depth as u32) < layout.max_depth {
            let dist = (cam - key.center_dir() * radius).length();
            if dist < SPLIT_FACTOR * key.arc(radius) {
                for child in key.children() {
                    visit(child, layout, radius, cam, out);
                }
                return;
            }
        }
        out.push(key);
    }
    for face in 0..6 {
        visit(TileKey::root(face), layout, radius, cam_local, out);
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
}

pub struct Terrain {
    pub params: BodyParams,
    pub layout: Layout,
    continent: Fbm<Perlin>,
    detail: Fbm<Perlin>,
    mid: Fbm<Perlin>,
    fine: Fbm<Perlin>,
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
}

impl Terrain {
    pub fn new(params: BodyParams) -> Self {
        let mut continent: Fbm<Perlin> = Fbm::new(params.seed);
        continent.octaves = 6;
        let mut detail: Fbm<Perlin> = Fbm::new(params.seed.wrapping_add(81));
        detail.octaves = 4;
        let mut mid: Fbm<Perlin> = Fbm::new(params.seed.wrapping_add(131));
        mid.octaves = 3;
        let mut fine: Fbm<Perlin> = Fbm::new(params.seed.wrapping_add(171));
        fine.octaves = 4;
        Self {
            params,
            layout: params.layout(),
            continent,
            detail,
            mid,
            fine,
            color: Perlin::new(params.seed.wrapping_add(200)),
            relief: ReliefField::new(params.relief).with_sea(params.sea_level).with_min_crater(2.0 * params.layout().voxel / params.radius.max(1.0)),
            biomes: BiomeField::new(params.biomes),
            overhang: None,
            voxels: None,
            caves: CaveStyle::of(&params).map(|s| Arc::new(Caves::new(s, params.seed, params.layout().voxel))),
            // Seulement là où le vent et l'eau sculptent la roche
            min_crater: std::sync::atomic::AtomicU32::new((2.0 * params.layout().voxel / params.radius.max(1.0)).to_bits()),
            rocks: (params.atmosphere && !params.airless && !params.gaseous && params.pressure >= 0.05)
                .then(|| Arc::new(Rocks::new(params.seed, params.radius, params.layout().voxel))),
        }
        .with_overhang()
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
        let s = dir * p.noise_scale;
        let continent = self.continent.get([s.x as f64, s.y as f64, s.z as f64]) as f32;
        let ds = p.detail_scale as f64;
        let det = self.detail.get([s.x as f64 * ds, s.y as f64 * ds, s.z as f64 * ds]) as f32 * 0.15;
        let base = ((continent + det + 1.0) * 0.5).clamp(0.0, 1.0);
        // Montagnes, rifts, volcans, canyons, plateaux, cratères (`planetgen::geology`)
        let min = f32::from_bits(self.min_crater.load(std::sync::atomic::Ordering::Relaxed));
        let sample = self.relief.sample_min(dir, base, min);
        let hv = (base + sample.h).clamp(0.0, 1.2);

        // L'érosion adoucit aussi les collines et le relief fin
        let rugged = (if p.airless { 1.5 } else { 1.0 }) * (1.0 - 0.5 * p.relief.erosion);
        let mid_amp = (p.terrain_height * 0.5).clamp(self.layout.voxel * 3.0, self.layout.voxel * 36.0) * rugged;
        let fine_amp = self.layout.voxel * 2.5 * rugged;
        let mf = (p.radius / (p.radius * 0.3).clamp(250.0, MID_WAVE)) as f64;
        let ff = (p.radius / (p.radius * 0.1).clamp(80.0, FINE_WAVE)) as f64;
        let mid = self.mid.get([dir.x as f64 * mf, dir.y as f64 * mf, dir.z as f64 * mf]) as f32 * mid_amp;
        let fine = self.fine.get([dir.x as f64 * ff, dir.y as f64 * ff, dir.z as f64 * ff]) as f32 * fine_amp;

        let h = p.radius + (hv - p.sea_level) * p.terrain_height + mid + fine;
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
        if h < p.radius && sea_material(&p.climate, &p.hydro, p.airless, dir.y).is_some() {
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
            return Column { dir, top: p.radius * GAS_CORE, kind: VoxelType::Stone, color: [0.3, 0.25, 0.2, 1.0] };
        }
        let (h, hv, relief) = self.raw_height_full(dir);
        let rel = ((h - p.radius) / quantum).round();
        // Sous le niveau de la mer : eau, banquise, ou bassin à sec (trop chaud, ou sans air)
        let sea = if rel < 0.0 { sea_material(&p.climate, &p.hydro, p.airless, dir.y) } else { None };
        let water = sea.is_some();

        let var = self.color.get([dir.x as f64 * 12.0, dir.y as f64 * 12.0, dir.z as f64 * 12.0]) as f32 * 0.10;
        let jitter = ((dir.x * 127.1 + dir.y * 311.7 + dir.z * 74.7).sin() * 43758.547).fract().abs() * 0.05 - 0.025;

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
            (p.radius, kind, color)
        } else {
            // Hauteur réelle (collines comprises) : une colline au bord de l'eau n'est pas une plage
            let mut kind = self.surface_type((h - p.radius) / p.terrain_height.max(1.0), dir);
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
        Column { dir, top, kind, color }
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
    pub fn layer(&self, r: f32) -> i32 {
        ((r - self.params.radius) / self.layout.voxel).floor() as i32
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
            let r = self.layer_radius(k) + v * 0.5;
            let depth = base.top - r;
            // Sous la mer, pas de grotte (elle se remplirait) ; près de la surface, le sol
            if !base.kind.is_liquid() {
                if let Some(caves) = &self.caves {
                    if depth < crate::caves::MAX_DEPTH + caves.size {
                        let p = dir * r;
                        let cell = match pieces {
                            Some(list) => eval_pieces(list.iter().filter(|(k0, k1, _)| (*k0..=*k1).contains(&k)).map(|(_, _, piece)| piece), p, v),
                            None => {
                                let near = caves.pieces_near(p, &|d| self.surface_r(d));
                                eval_pieces(
                                    near.iter().filter(|piece| {
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
        let p = &self.params;
        if p.gaseous {
            return Column { dir, top: p.radius * GAS_CORE, kind: VoxelType::Stone, color: [0.3, 0.25, 0.2, 1.0] };
        }
        let (face, i, j) = self.cell_of(dir);
        let center = self.cell_dir(face, i, j);
        let (base, top_k) = self.base_cell_column(center);
        if !self.has_3d(face, i, j, center) {
            return base;
        }
        let highest = self.highest_layer(face, i, j, top_k);
        // (marges en fraction de voxel : à 10 000 unités du centre, 0,001 est sous la précision
        // d'un f32)
        let start = if r.is_finite() { (self.layer(r + 0.05 * self.layout.voxel) - 1).min(highest) } else { highest };
        for k in (start - 512..=start).rev() {
            let kind = self.kind_at(face, i, j, k, center, &base, top_k);
            if kind != VoxelType::Air {
                let color = if k < top_k { base.color } else { OVERHANG_COLOR };
                return Column { dir: center, top: self.layer_radius(k + 1), kind, color };
            }
        }
        base
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
            for piece in rocks.pieces_near(dir, &|d| self.rock_ground(d)).iter() {
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
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
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
pub fn build_tile_mesh_with(terrain: &Terrain, key: TileKey) -> Mesh {
    if key.depth as u32 >= terrain.layout.max_depth && !terrain.params.gaseous {
        return build_voxel_tile_mesh(terrain, key);
    }
    build_height_tile_mesh(terrain, key)
}

/// Tuile en champ de hauteur (tuiles lointaines).
pub fn build_height_tile_mesh(terrain: &Terrain, key: TileKey) -> Mesh {
    // Pas de cratère plus petit qu'une colonne et demie de cette tuile (on ne le verrait pas)
    use std::sync::atomic::Ordering;
    let before = terrain.min_crater.load(Ordering::Relaxed);
    let column = FRAC_PI_2 / ((TILE_CELLS as u32) << key.depth.min(30)) as f32;
    terrain.min_crater.store(f32::from_bits(before).max(1.5 * column).to_bits(), Ordering::Relaxed);
    let mesh = build_height_tile_inner(terrain, key);
    terrain.min_crater.store(before, Ordering::Relaxed);
    mesh
}

fn build_height_tile_inner(terrain: &Terrain, key: TileKey) -> Mesh {
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
    let col = |ci: i32, cj: i32| &cols[(cj + 1) as usize * nc + (ci + 1) as usize];

    let mut buf = MeshBuf::default();
    buf.pos.reserve(TILE_CELLS * TILE_CELLS * 6);
    let last = TILE_CELLS as i32 - 1;

    for cj in 0..TILE_CELLS as i32 {
        for ci in 0..TILE_CELLS as i32 {
            let c = col(ci, cj);
            let (u, v) = (ci as usize, cj as usize);

            buf.quad(
                [
                    corner(u, v) * c.top,
                    corner(u + 1, v) * c.top,
                    corner(u + 1, v + 1) * c.top,
                    corner(u, v + 1) * c.top,
                ],
                c.dir,
                c.color,
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
                let nb = col(ni, nj);
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
                let shade = [c.color[0] * 0.82, c.color[1] * 0.82, c.color[2] * 0.82, 1.0];
                buf.quad([a * lo, b * lo, b * hi, a * hi], n, shade);
            }
        }
    }
    buf.into_mesh()
}

/// Colonne d'une tuile 3D (avec une rangée de voisines tout autour).
struct Col3 {
    base: Column,
    top_k: i32,
    /// Colonne canonique (une voisine au-delà du bord de la face est rapportée à sa vraie face).
    face: u8,
    i: i64,
    j: i64,
}

/// Couleur d'une cellule : celle de la colonne pour le sol, celle de sa matière pour la roche, les
/// filons, les cristaux et l'eau des grottes (avec un peu de variation).
fn cell_color(kind: VoxelType, k: i32, c: &Col3) -> [f32; 4] {
    if kind == c.base.kind && k < c.top_k {
        return c.base.color;
    }
    if k >= c.top_k && kind == VoxelType::Stone {
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
pub fn build_voxel_tile_mesh(t: &Terrain, key: TileKey) -> Mesh {
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
            cols.push(Col3 { base, top_k, face, i, j });
        }
    }
    let col = |ci: i32, cj: i32| &cols[(cj + 1) as usize * nc + (ci + 1) as usize];

    // Couches à examiner : autour du sol, plus les formes 3D et les cellules modifiées
    let tile_dir = key.center_dir();
    let any_3d = t.overhang.as_ref().is_some_and(|o| tile_dir.dot(o.dir) > (key.arc(t.params.radius) * 0.8 / o.base + 40.0 * v / o.base).cos())
        || t.voxels.as_ref().is_some_and(|vx| cols.iter().any(|c| vx.layers_in(c.face, c.i.div_euclid(BLOCK), c.j.div_euclid(BLOCK)).is_some()));
    let mut kmin = cols.iter().map(|c| c.top_k).min().unwrap_or(0) - 1;
    let mut kmax = cols.iter().map(|c| c.top_k).max().unwrap_or(0) + 1;
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
            caves.for_tile(&dirs, &|d| t.surface_r(d))
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
        let mut out = vec![(kmin.min(c.top_k - 1), kmax)];
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
    let shade = |c: [f32; 4], k: f32| [c[0] * k, c[1] * k, c[2] * k, 1.0];

    for cj in 0..TILE_CELLS as i32 {
        for ci in 0..TILE_CELLS as i32 {
            let c = col(ci, cj);
            let (u, w) = (ci as usize, cj as usize);
            let up = c.base.dir;
            for k in ranges(ci, cj).into_iter().flat_map(|(a, b)| a..=b) {
                let here = kind(ci, cj, k);
                if here == VoxelType::Air {
                    continue;
                }
                let color = cell_color(here, k, c);
                let (r0, r1) = (t.layer_radius(k), t.layer_radius(k + 1));
                if kind(ci, cj, k + 1) == VoxelType::Air {
                    buf.quad([corner(u, w) * r1, corner(u + 1, w) * r1, corner(u + 1, w + 1) * r1, corner(u, w + 1) * r1], up, color);
                }
                if kind(ci, cj, k - 1) == VoxelType::Air {
                    buf.quad([corner(u, w) * r0, corner(u + 1, w) * r0, corner(u + 1, w + 1) * r0, corner(u, w + 1) * r0], -up, shade(color, 0.55));
                }
                let sides: [((i32, i32), (usize, usize), (usize, usize)); 4] = [
                    ((1, 0), (u + 1, w), (u + 1, w + 1)),
                    ((-1, 0), (u, w), (u, w + 1)),
                    ((0, 1), (u, w + 1), (u + 1, w + 1)),
                    ((0, -1), (u, w), (u + 1, w)),
                ];
                for ((dx, dy), ea, eb) in sides {
                    if kind(ci + dx, cj + dy, k) != VoxelType::Air {
                        continue;
                    }
                    let (a, b) = (corner(ea.0, ea.1), corner(eb.0, eb.1));
                    let nb = col(ci + dx, cj + dy);
                    let mut nrm = (b - a).cross(up).normalize_or_zero();
                    if nrm.dot(nb.base.dir - up) < 0.0 {
                        nrm = -nrm;
                    }
                    buf.quad([a * r0, b * r0, b * r1, a * r1], nrm, shade(color, 0.82));
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
                let hi = c.base.top.min(nb.base.top);
                let lo = hi - skirt;
                let (a, b) = (corner(ea.0, ea.1), corner(eb.0, eb.1));
                let mut nrm = (b - a).cross(up).normalize_or_zero();
                if nrm.dot(nb.base.dir - up) < 0.0 {
                    nrm = -nrm;
                }
                buf.quad([a * lo, b * lo, b * hi, a * hi], nrm, shade(c.base.color, 0.82));
            }
        }
    }
    buf.into_mesh()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::render::mesh::VertexAttributeValues;

    fn earth_like() -> BodyParams {
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
        for r in [120.0, 300.0, 500.0, 1_000.0, 1_500.0, 3_000.0, 6_000.0, 13_000.0] {
            let l = layout_for(r);
            assert!(l.voxel > MAX_VOXEL * 0.4 && l.voxel <= MAX_VOXEL, "rayon {r} : voxel {}", l.voxel);
        }
    }

    #[test]
    fn selection_refines_around_the_camera() {
        let p = earth_like();
        let t = Terrain::new(p);
        let dir = Vec3::new(0.3, 0.8, 0.5).normalize();
        let cam = dir * (t.ground(dir).top + 15.0);
        let mut tiles = Vec::new();
        select_tiles(t.layout, p.radius, cam, &mut tiles);
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
                let height = build_height_tile_mesh(&t, key);
                let voxel = build_voxel_tile_mesh(&t, key);
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
                assert!(back.angle_between(dir) * t.params.radius < t.voxel() * 0.75, "face {face} ({gi}, {gj})");
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
        assert_eq!(tops(&build_voxel_tile_mesh(&a, key)), tops(&build_voxel_tile_mesh(&b, key)));
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
        let mesh = build_voxel_tile_mesh(&t, finest(&t, under));
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
            select_tiles(t.layout, p.radius, dir * (t.ground(dir).top + 20.0), &mut keys);
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
            select_tiles(t.layout, p.radius, dir * (t.ground(dir).top + 20.0), &mut sel);
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
