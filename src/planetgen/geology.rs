//! Géologie et relief (phase 5) : âge de la planète et de sa surface, activité interne,
//! tectonique, volcanisme, séismes, champ magnétique, érosion ; et le relief qui en découle.
//!
//! - Chaleur interne : plus forte pour une planète massive et jeune (refroidissement en
//!   exp(−âge / τ), τ ∝ √masse). Terre ≈ 0,6 (plaques), Vénus ≈ 0,4, Mars et la Lune : éteintes.
//! - Tectonique des plaques s'il y a assez de chaleur et de l'eau liquide (elle lubrifie les
//!   failles), sinon couvercle stagnant (volcans géants, peu de montagnes), sinon inactive.
//! - Âge de la surface : jeune sur une planète active (renouvelée), aussi vieux que la planète sur
//!   un monde éteint, qui accumule les cratères. L'atmosphère, la pluie, le vent et la glace les
//!   effacent (érosion).
//! - Champ magnétique : dynamo d'un noyau liquide (activité) entretenue par la rotation.
//!
//! Le relief (`Relief`, `ReliefField`) s'ajoute au bruit des continents, dans les mêmes unités
//! (hauteur relative), pour le terrain voxel (`terrain.rs`) comme pour le maillage lointain
//! (`mesher.rs`).

use bevy::math::Vec3;
use noise::{NoiseFn, Perlin};
use serde::{Deserialize, Serialize};

use super::seeds::LayerRng;

/// Régime tectonique.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tectonics {
    /// Plus de chaleur interne : surface figée.
    #[default]
    Inactive,
    /// Une seule plaque : volcans géants, failles, peu de chaînes de montagnes (Vénus, Mars jeune).
    StagnantLid,
    /// Plaques mobiles : chaînes de montagnes, rifts, dorsales (Terre).
    Plates,
}

impl Tectonics {
    pub fn name(self) -> &'static str {
        match self {
            Tectonics::Inactive => "inactive",
            Tectonics::StagnantLid => "couvercle stagnant (une seule plaque)",
            Tectonics::Plates => "tectonique des plaques",
        }
    }
}

/// Paramètres du relief, en hauteur relative (le bruit des continents varie de ~0,3 à ~0,7).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Relief {
    pub seed: u32,
    /// Nombre de plaques (0 : pas de chaînes de montagnes ni de rifts).
    pub plates: u8,
    /// Hauteur des chaînes de montagnes (convergence) et profondeur des rifts (divergence).
    pub mountains: f32,
    pub rifts: f32,
    pub volcanoes: u8,
    pub volcano_height: f32,
    /// Profondeur des canyons et failles.
    pub canyons: f32,
    /// Densité des cratères (0 à 1) et leur profondeur.
    pub craters: f32,
    pub crater_depth: f32,
    /// Plateaux en marches (0 à 1).
    pub terraces: f32,
    /// Érosion (0 : relief vif, 1 : très adouci).
    pub erosion: f32,
    /// Monde vertical (0.14 X6) : hauteur des étages (voxels ; 0 = aucun) séparés par des parois.
    #[serde(default)]
    pub tier_step: f32,
    /// Monde cubique (0.14 X6) : arête des gros cubes du relief (voxels ; 0 = aucun).
    #[serde(default)]
    pub cubic_step: f32,
}

/// Géologie d'une planète (profil).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Geology {
    pub age_gyr: f32,
    pub surface_age_gyr: f32,
    /// Chaleur interne (0 à 1).
    pub activity: f32,
    pub tectonics: Tectonics,
    /// Volcanisme (0 à 1).
    pub volcanism: f32,
    /// Séismes de magnitude 5 et plus, par rapport à la Terre (1).
    pub quakes: f32,
    /// Champ magnétique de surface, Terre = 1.
    pub magnetic_field: f32,
    /// Érosion par la pluie, le vent et la glace (0 à 1 chacune).
    pub rain_erosion: f32,
    pub wind_erosion: f32,
    pub ice_erosion: f32,
    pub relief: Relief,
}

impl Geology {
    /// Formes du relief, pour le profil.
    pub fn features(&self) -> Vec<String> {
        let r = &self.relief;
        let mut out = Vec::new();
        if r.plates > 0 && r.mountains > 0.0 {
            out.push(format!("chaines de montagnes ({} plaques)", r.plates));
        }
        if r.rifts > 0.0 {
            out.push("rifts et dorsales".into());
        }
        if r.volcanoes > 0 {
            out.push(format!("{} volcans (boucliers, cones, caldeiras)", r.volcanoes));
        }
        if r.canyons > 0.0 {
            out.push("canyons et failles".into());
        }
        if r.terraces > 0.0 {
            out.push("plateaux".into());
        }
        if r.craters > 0.05 {
            out.push(if r.craters > 0.5 { "nombreux crateres".into() } else { "crateres".into() });
        }
        out
    }
}

/// Données d'entrée.
pub struct GeoInput {
    pub mass: f64,
    pub age_gyr: f64,
    pub gravity: f64,
    /// Eau liquide en surface (pluie, mers).
    pub liquid_water: bool,
    pub pressure: f64,
    pub ice: bool,
    pub wind_ms: f64,
    pub rotation_h: f64,
    pub locked: bool,
    /// Chauffage par les marées d'une planète géante proche (lunes, comme Io) : 0 à 1.
    pub tidal: f64,
}

/// Chaleur interne (0 à 1) d'une planète de `mass` M⊕ âgée de `age` Gyr.
pub fn internal_activity(mass: f64, age: f64) -> f64 {
    let m = mass.max(0.001).sqrt();
    (2.7 * m * (-age / (3.0 * m)).exp()).clamp(0.0, 1.0)
}

/// Géologie et relief d'une planète rocheuse.
pub fn generate(input: &GeoInput, seed: u32, rng: &mut LayerRng) -> Geology {
    let age = input.age_gyr.max(0.05);
    // Les marées d'une géante proche entretiennent la chaleur d'une petite lune (Io, Europe)
    let activity = internal_activity(input.mass, age).max(input.tidal.clamp(0.0, 1.0));
    let tectonics = if activity > 0.35 && input.liquid_water && input.mass > 0.3 {
        Tectonics::Plates
    } else if activity > 0.08 {
        Tectonics::StagnantLid
    } else {
        Tectonics::Inactive
    };
    let volcanism = (activity * rng.range(0.6, 1.4)).clamp(0.0, 1.0);
    let surface_age = match tectonics {
        Tectonics::Plates => rng.range(0.1, 0.5),
        // Les coulées de lave renouvellent la surface (Vénus : 0,3 à 0,7 Gyr ; Io : quelques millions d'années)
        Tectonics::StagnantLid => rng.range(0.3, 1.0) * age.min(3.0) * (1.0 - activity).powi(2),
        Tectonics::Inactive => age * rng.range(0.85, 1.0),
    };
    let quakes = match tectonics {
        Tectonics::Plates => activity / 0.58,
        Tectonics::StagnantLid => 0.05 * activity / 0.3,
        Tectonics::Inactive => 0.0,
    };
    // Dynamo : noyau encore liquide et rotation assez rapide
    let spin = if input.locked { 0.15 } else { (24.0 / input.rotation_h.max(1.0)).clamp(0.1, 2.0).sqrt() };
    let magnetic = if activity > 0.12 { (activity / 0.58).sqrt() * input.mass.sqrt().min(2.0) * spin } else { 0.0 };

    // Érosion : pluie (eau liquide), vent (air épais), glace, d'autant plus forte que la surface est vieille
    let rain = if input.liquid_water { 0.8 } else { 0.0 };
    let wind = (input.pressure.max(0.0).powf(0.3) * (input.wind_ms / 30.0).min(1.0)).min(1.0) * if input.pressure > 0.01 { 1.0 } else { 0.0 };
    let ice = if input.ice { 0.5 } else { 0.0 };
    let erosion = ((rain + 0.4 * wind + 0.5 * ice) * (surface_age / 2.0).min(1.0).max(0.4)).clamp(0.0, 0.9);

    // ── Relief ──────────────────────────────────────────────────────────
    let g = input.gravity.clamp(0.1, 3.0);
    let plates = if tectonics == Tectonics::Plates { 6 + (rng.unit() * 8.0) as u8 } else { 0 };
    // Les volcans montent plus haut sous une faible gravité (Olympus Mons sur Mars)
    let volcanoes = match tectonics {
        Tectonics::Plates => (volcanism * 18.0) as u8,
        Tectonics::StagnantLid => (4.0 + volcanism * 26.0) as u8,
        Tectonics::Inactive => (rng.unit() * 3.0 * (1.0 - age / 13.0).max(0.0)) as u8,
    };
    let volcano_height = ((0.12 + 0.18 * rng.unit()) / g.sqrt()).min(0.45);
    // Cratères : surface vieille, pas d'air pour freiner les météores, peu d'érosion
    let shield = (1.0 - (input.pressure / 1.0).min(1.0) * 0.8).max(0.0);
    let craters = ((surface_age / 3.5).min(1.0) * shield * (1.0 - erosion)).clamp(0.0, 1.0);
    let relief = Relief {
        seed,
        plates,
        mountains: if plates > 0 { (0.12 + 0.12 * rng.unit()) / g.sqrt() as f64 } else { 0.0 } as f32,
        rifts: if plates > 0 { 0.05 + 0.05 * rng.unit() } else { 0.0 } as f32,
        volcanoes,
        volcano_height: volcano_height as f32,
        canyons: if tectonics != Tectonics::Plates && rng.unit() < 0.5 + 0.5 * activity { 0.06 + 0.06 * rng.unit() } else { 0.0 } as f32,
        craters: craters as f32,
        crater_depth: (0.04 + 0.04 * rng.unit()) as f32,
        terraces: if tectonics != Tectonics::Plates && rng.unit() < 0.35 { 0.4 + 0.5 * rng.unit() } else { 0.0 } as f32,
        erosion: erosion as f32,
        tier_step: 0.0,
        cubic_step: 0.0,
    };

    Geology {
        age_gyr: age as f32,
        surface_age_gyr: surface_age as f32,
        activity: activity as f32,
        tectonics,
        volcanism: volcanism as f32,
        quakes: quakes as f32,
        magnetic_field: magnetic as f32,
        rain_erosion: rain as f32,
        wind_erosion: wind as f32,
        ice_erosion: ice as f32,
        relief,
    }
}

/// Relief d'une lune sans air : vieille, criblée de cratères.
pub fn moon_relief(seed: u32) -> Relief {
    Relief { seed, craters: 0.9, crater_depth: 0.07, ..Default::default() }
}

// ─────────────────────────────────────────────────────────────────────────
//  Champ de relief : hauteur ajoutée dans une direction
// ─────────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
enum VolcanoKind {
    Shield,
    Cone,
    Caldera,
}

#[derive(Clone, Copy)]
struct Volcano {
    center: Vec3,
    /// cos du rayon angulaire (test rapide « dans le volcan »).
    cos_radius: f32,
    radius: f32,
    height: f32,
    kind: VolcanoKind,
}

/// Ce que le relief donne dans une direction (0.11, B3) : la hauteur ajoutée et de quoi choisir
/// la matière du sol (éboulis au pied des chaînes, coulées de lave figées, falaises).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ReliefSample {
    pub h: f32,
    /// Dans une chaîne de montagnes (0 à 1).
    pub mountain: f32,
    /// Éboulis au pied d'une chaîne ou d'une falaise.
    pub scree: bool,
    /// Coulée de lave figée (flancs d'un volcan).
    pub lava: bool,
    /// Bord de falaise (mesas, canyons) : 0 à 1.
    pub cliff: f32,
    /// Éjectas et rayons clairs d'un cratère récent (0 à 1).
    pub bright: f32,
    /// Fond de cratère rempli (lave figée, ou glace sur un monde froid).
    pub flooded: bool,
}

/// Bruit en crêtes (0 à 1, pointu en haut) : 1 − |bruit|, au carré, sur trois octaves.
fn ridged(noise: &Perlin, dir: Vec3, freq: f64) -> f32 {
    let mut sum = 0.0;
    let mut norm = 0.0;
    let mut amp = 1.0;
    let mut f = freq;
    for _ in 0..3 {
        let n = noise.get([dir.x as f64 * f, dir.y as f64 * f, dir.z as f64 * f]) as f32;
        let r = (1.0 - n.abs()).powi(2);
        sum += r * amp;
        norm += amp;
        amp *= 0.5;
        f *= 2.3;
    }
    sum / norm
}

fn smooth01(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Formes du relief d'un astre, prêtes à être évaluées colonne par colonne.
pub struct ReliefField {
    relief: Relief,
    /// Centre de chaque plaque et son mouvement (tangent).
    plates: Vec<(Vec3, Vec3)>,
    volcanoes: Vec<Volcano>,
    ridges: Perlin,
    canyons: Perlin,
    /// Niveau de la mer (hauteur relative) : les marches des mesas s'y alignent.
    sea: f32,
    /// Plus petit cratère calculé (rayon angulaire).
    min_crater: f32,
}

fn hash3(x: i32, y: i32, z: i32, k: u32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x8DA6_B343) ^ (y as u32).wrapping_mul(0xD816_3841) ^ (z as u32).wrapping_mul(0xCB1A_B31F) ^ k;
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^ (h >> 15)
}

fn unit(h: u32) -> f32 {
    (h & 0xFF_FFFF) as f32 / 16_777_216.0
}

/// Point uniforme sur la sphère d'après deux tirages.
fn sphere_point(u: f32, v: f32) -> Vec3 {
    let z = 1.0 - 2.0 * u;
    let r = (1.0 - z * z).max(0.0).sqrt();
    let a = v * std::f32::consts::TAU;
    Vec3::new(r * a.cos(), z, r * a.sin())
}

/// Largeur (en produit scalaire) de la bande d'une frontière de plaques.
const BOUNDARY: f32 = 0.06;
/// Cratères (B4) : classes de taille, en cellules par unité de rayon. Chaque classe a ~4 fois
/// plus de cratères que la précédente, deux fois plus grands : N(>D) ∝ D^-2 (loi de puissance).
const CRATER_CLASSES: [f32; 7] = [1.5, 3.0, 6.0, 12.0, 24.0, 48.0, 96.0];
/// Rayon angulaire (radians) au-delà duquel un cratère est complexe (pic central, terrasses),
/// puis un bassin à anneaux (Lune : ~15 km et ~300 km de diamètre).
const COMPLEX_CRATER: f32 = 0.012;
/// Portée d'un cratère (cellules) : rayon maximal 0,45 × 1,6 (rebord, éjectas, rayons).
const CRATER_REACH: f32 = 0.45 * 1.6;
const BASIN_CRATER: f32 = 0.08;

/// Forme d'un cratère.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CraterKind {
    /// Cuvette simple.
    Simple,
    /// Fond plat, parois en terrasses, pic central.
    Complex,
    /// Bassin à anneaux.
    Basin,
}

impl CraterKind {
    pub fn of(angular_radius: f32) -> Self {
        if angular_radius >= BASIN_CRATER {
            CraterKind::Basin
        } else if angular_radius >= COMPLEX_CRATER {
            CraterKind::Complex
        } else {
            CraterKind::Simple
        }
    }

    /// Profil (hauteur relative à la profondeur) à la distance `x` du centre, en rayons du
    /// cratère : creux au fond, rebord au bord, rien au loin. `filled` : fond aplani (lave, glace).
    pub fn profile(self, x: f32, filled: bool) -> f32 {
        // Rebord : bosse étroite autour de x = 1 (polynôme, plus rapide qu'une exponentielle)
        let u = ((x - 1.0) / 0.22).abs();
        let rim = if u < 1.0 { 0.35 * (1.0 - u * u) * (1.0 - u * u) } else { 0.0 };
        let h = match self {
            CraterKind::Simple => {
                if x < 1.0 {
                    -(1.0 - x * x)
                } else {
                    0.0
                }
            }
            CraterKind::Complex => {
                if x < 0.55 {
                    // Fond plat, pic central
                    let peak = if x < 0.18 { 0.55 * (1.0 - x / 0.18).powi(2) } else { 0.0 };
                    -0.7 + if filled { 0.0 } else { peak }
                } else if x < 1.0 {
                    // Parois en trois terrasses
                    let t = (x - 0.55) / 0.45;
                    let steps = 3.0;
                    let stepped = ((t * steps).floor() + smooth01(((t * steps).fract() - 0.7) / 0.3)) / steps;
                    -0.7 * (1.0 - stepped)
                } else {
                    0.0
                }
            }
            CraterKind::Basin => {
                if x < 1.0 {
                    // Fond plat et anneaux concentriques (rides)
                    let ring = |c: f32, w: f32| {
                        let u = ((x - c) / (w * 1.7)).abs();
                        if u < 1.0 { 0.18 * (1.0 - u * u) * (1.0 - u * u) } else { 0.0 }
                    };
                    -0.5 + ring(0.45, 0.05) + ring(0.7, 0.05) + 0.5 * x.powi(6)
                } else {
                    0.0
                }
            }
        };
        if filled && x < 0.85 && self != CraterKind::Simple {
            return -0.45 + rim;
        }
        h + rim
    }
}

impl ReliefField {
    pub fn new(relief: Relief) -> Self {
        let s = relief.seed;
        let r = |k: u32| unit(hash3(k as i32, 7, 11, s));
        let plates = (0..relief.plates as u32)
            .map(|i| {
                let c = sphere_point(r(i * 4), r(i * 4 + 1));
                let raw = sphere_point(r(i * 4 + 2), r(i * 4 + 3));
                (c, (raw - c * raw.dot(c)).normalize_or_zero())
            })
            .collect();
        let volcanoes = (0..relief.volcanoes as u32)
            .map(|i| {
                let k = 1000 + i * 5;
                let radius = 0.03 + 0.09 * r(k + 2);
                let kind = match r(k + 3) {
                    x if x < 0.5 => VolcanoKind::Shield,
                    x if x < 0.8 => VolcanoKind::Cone,
                    _ => VolcanoKind::Caldera,
                };
                Volcano {
                    center: sphere_point(r(k), r(k + 1)),
                    cos_radius: radius.cos(),
                    radius,
                    height: relief.volcano_height * (0.5 + 0.5 * r(k + 4)),
                    kind,
                }
            })
            .collect();
        Self { relief, plates, volcanoes, ridges: Perlin::new(s.wrapping_add(400)), canyons: Perlin::new(s.wrapping_add(500)), sea: 0.5, min_crater: 0.004 }
    }

    /// Le même relief pour une mer au niveau `sea`.
    pub fn with_sea(mut self, sea: f32) -> Self {
        self.sea = sea;
        self
    }

    /// Les cratères plus petits que `rel` (rayon angulaire) ne sont pas calculés : on ne les
    /// verrait pas à cette finesse (deux voxels au moins).
    pub fn with_min_crater(mut self, rel: f32) -> Self {
        self.min_crater = rel;
        self
    }

    /// Cratères dans la direction `dir` : hauteur ajoutée ; éclat des éjectas et des rayons
    /// (cratères récents) et fond rempli (lave, glace) dans `out`.
    fn craters(&self, dir: Vec3, min_crater: f32, out: &mut ReliefSample) -> f32 {
        let r = &self.relief;
        let mut h = 0.0;
        // L'érosion efface d'abord les petits cratères, et use les autres
        let fade = 1.0 - 0.9 * r.erosion;
        for (k, &scale) in CRATER_CLASSES.iter().enumerate() {
            if 0.45 / scale < min_crater.max(0.02 * r.erosion) {
                break;
            }
            // Les très grands bassins sont rares
            let chance = r.craters * if k == 0 { 0.3 } else { 0.6 };
            let p = dir * scale;
            let (cx, cy, cz) = (p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
            let f = p - Vec3::new(cx as f32, cy as f32, cz as f32);
            // Distance (au carré) du point à une cellule voisine, par axe
            let gap = |o: i32, f: f32| match o {
                -1 => f * f,
                1 => (1.0 - f) * (1.0 - f),
                _ => 0.0,
            };
            for dx in -1..=1 {
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        // Un cratère (rayon 0,45 au plus) ne porte pas plus loin que 1,6 rayon
                        if gap(dx, f.x) + gap(dy, f.y) + gap(dz, f.z) > CRATER_REACH * CRATER_REACH {
                            continue;
                        }
                        let (x, y, z) = (cx + dx, cy + dy, cz + dz);
                        let hash = hash3(x, y, z, r.seed ^ (k as u32).wrapping_mul(0x9E37_79B9));
                        if unit(hash) >= chance {
                            continue;
                        }
                        let center = Vec3::new(x as f32 + unit(hash.rotate_left(8)), y as f32 + unit(hash.rotate_left(16)), z as f32 + unit(hash.rotate_left(24)));
                        let radius = 0.15 + 0.3 * unit(hash.wrapping_mul(0x9E37_79B9));
                        let d = p.distance(center);
                        let x = d / radius;
                        if x > 1.6 {
                            continue;
                        }
                        let angular = radius / scale;
                        let kind = CraterKind::of(angular);
                        // Âge : les récents gardent leurs éjectas et leurs rayons clairs, les vieux
                        // sont usés (plus plats, rebord émoussé)
                        let age = unit(hash.rotate_left(5));
                        let fresh = (1.0 - age / 0.2).max(0.0);
                        let worn = 1.0 - 0.6 * age;
                        // Profondeur : croît avec la taille, moins vite que le diamètre
                        let depth = r.crater_depth * (angular / 0.02).sqrt().clamp(0.3, 4.0) * fade * worn;
                        let filled = kind != CraterKind::Simple && unit(hash.rotate_left(13)) < 0.35;
                        if x < 1.4 {
                            h += depth * kind.profile(x, filled);
                            if filled && x < 0.85 {
                                out.flooded = true;
                            }
                        }
                        if fresh > 0.0 && x > 0.9 {
                            // Éjectas : couverture claire et un peu surélevée autour du rebord
                            let blanket = (1.0 - (x - 1.0) / 0.6).clamp(0.0, 1.0);
                            h += depth * 0.08 * blanket;
                            // Rayons : traînées claires qui partent du cratère
                            let c = center.normalize();
                            let (u, w) = c.any_orthonormal_pair();
                            let rel = p - center;
                            let theta = rel.dot(w).atan2(rel.dot(u));
                            let rays = (theta * (5.0 + 6.0 * unit(hash.rotate_left(19)))).sin().abs().powi(6);
                            out.bright = out.bright.max(fresh * (blanket * 0.7).max(rays * (1.0 - (x - 1.0) / 0.6).max(0.0)));
                        }
                    }
                }
            }
        }
        h
    }

    /// Rien à ajouter (planète faite à la main, géante).
    pub fn is_flat(&self) -> bool {
        let r = &self.relief;
        r.plates == 0 && r.volcanoes == 0 && r.canyons == 0.0 && r.craters <= 0.0 && r.terraces <= 0.0
    }

    /// Hauteur relative ajoutée dans la direction `dir` (unitaire), sur un relief de base `base`.
    pub fn offset(&self, dir: Vec3, base: f32) -> f32 {
        self.sample(dir, base).h
    }

    /// Hauteur ajoutée et nature du relief dans la direction `dir`.
    pub fn sample(&self, dir: Vec3, base: f32) -> ReliefSample {
        self.sample_min(dir, base, self.min_crater)
    }

    /// Comme `sample`, sans les cratères plus petits que `min_crater` (rayon angulaire) : une
    /// tuile lointaine ne les verrait pas.
    pub fn sample_min(&self, dir: Vec3, base: f32, min_crater: f32) -> ReliefSample {
        let r = &self.relief;
        let mut out = ReliefSample::default();
        if self.is_flat() {
            return out;
        }
        let soft = 1.0 - 0.5 * r.erosion;
        let mut h = 0.0;

        // Plateaux et mesas : marches plates aux bords abrupts (falaises), d'autant plus nettes
        // que l'érosion est faible
        if r.terraces > 0.0 {
            // Plateau plat au milieu de chaque marche, falaise à mi-chemin entre deux : symétrique,
            // donc le relief ne monte ni ne descend en moyenne (la mer garde sa place)
            // Marches alignées sur le niveau de la mer (plateaux à mi-marche au-dessus et au-dessous) :
            // la côte devient une falaise et la mer garde exactement sa place
            let steps = 8.0;
            let x = (base - self.sea) * steps - 0.5;
            let n = x.round();
            let d = x - n;
            let edge = 0.06 + 0.2 * r.erosion;
            let rise = 0.5 * d.signum() * smooth01((d.abs() - (0.5 - edge)) / edge);
            let stepped = (n + rise + 0.5) / steps + self.sea;
            h += (stepped - base) * r.terraces;
            out.cliff = out.cliff.max(if d.abs() > 0.5 - edge { r.terraces } else { 0.0 });
            // Éboulis au pied des falaises
            out.scree |= r.terraces > 0.3 && (0.5 - 2.0 * edge..0.5 - edge).contains(&d.abs()) && d < 0.0;
        }

        // Plaques : chaînes de montagnes là où deux plaques se rapprochent, rifts où elles s'écartent
        if self.plates.len() >= 2 {
            let (mut i1, mut d1, mut i2, mut d2) = (0, f32::MIN, 0, f32::MIN);
            for (i, (c, _)) in self.plates.iter().enumerate() {
                let d = dir.dot(*c);
                if d > d1 {
                    (i2, d2) = (i1, d1);
                    (i1, d1) = (i, d);
                } else if d > d2 {
                    (i2, d2) = (i, d);
                }
            }
            let gap = d1 - d2;
            if gap < BOUNDARY {
                let w = 1.0 - gap / BOUNDARY;
                let (a, b) = (self.plates[i1], self.plates[i2]);
                let closing = (a.1 - b.1).dot(b.0 - a.0) > 0.0;
                if closing {
                    // Chaîne jeune : crêtes vives et sommets pointus (bruit en crêtes) ; l'érosion
                    // les arrondit. Des cols (creux de la crête) permettent de passer.
                    let ridge = ridged(&self.ridges, dir, 14.0).powf(1.4 - 0.8 * r.erosion);
                    let pass = 0.55 + 0.45 * smooth01(self.canyons.get([dir.x as f64 * 4.0, dir.y as f64 * 4.0, dir.z as f64 * 4.0]) as f32 * 2.0 + 0.5);
                    h += r.mountains * w * w * (0.3 + 1.0 * ridge * pass) * soft;
                    out.mountain = w;
                    // Éboulis au pied des pentes
                    out.scree |= (0.15..0.45).contains(&w);
                } else {
                    h -= r.rifts * w * w * soft;
                }
            }
        }

        // Volcans
        for v in &self.volcanoes {
            let c = dir.dot(v.center);
            if c <= v.cos_radius {
                continue;
            }
            let d = c.min(1.0).acos() / v.radius;
            // Cratère au sommet : un creux, d'autant plus large que le volcan est plat
            let (crater, depth) = match v.kind {
                VolcanoKind::Shield => (0.08, 0.15),
                VolcanoKind::Cone => (0.1, 0.3),
                VolcanoKind::Caldera => (0.25, 0.6),
            };
            let pit = if d < crater { v.height * depth * (1.0 - (d / crater).powi(2)) } else { 0.0 };
            h += soft
                * (match v.kind {
                    VolcanoKind::Shield => v.height * (1.0 - d).powf(1.5),
                    VolcanoKind::Cone => v.height * (1.0 - d).powf(2.6),
                    VolcanoKind::Caldera => v.height * (1.0 - d).powf(1.2),
                } - pit);
            // Coulées figées : des langues qui descendent les flancs (bruit tournant autour du
            // sommet), légèrement en relief
            if (crater..0.95).contains(&d) {
                let side = (dir - v.center * c).normalize_or_zero();
                let n = self.ridges.get([side.x as f64 * 6.0 + 50.0, side.y as f64 * 6.0, side.z as f64 * 6.0 + d as f64 * 1.5]) as f32;
                if n.abs() < 0.12 {
                    out.lava = true;
                    h += v.height * 0.04 * (1.0 - n.abs() / 0.12) * (1.0 - d);
                }
            }
        }

        // Canyons et failles : sillons sinueux, profonds, à fond plat et parois raides
        if r.canyons > 0.0 {
            let n = self.canyons.get([dir.x as f64 * 3.0, dir.y as f64 * 3.0, dir.z as f64 * 3.0]) as f32;
            let w = 0.04;
            if n.abs() < w {
                let t = 1.0 - n.abs() / w;
                let wall = t.powf(0.3 + 0.7 * r.erosion);
                h -= r.canyons * 1.5 * wall * soft;
                out.cliff = out.cliff.max(if t < 0.35 { 1.0 } else { 0.0 });
            }
        }

        // Cratères (B4) : classes de taille en loi de puissance, formes selon la taille
        if r.craters > 0.0 {
            h += self.craters(dir, min_crater, &mut out);
        }
        out.h = h;
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planetgen::seeds::Layer;

    fn input(mass: f64, age: f64, water: bool, pressure: f64) -> GeoInput {
        GeoInput { mass, age_gyr: age, gravity: mass.powf(0.44), liquid_water: water, pressure, ice: false, wind_ms: 10.0, rotation_h: 24.0, locked: false, tidal: 0.0 }
    }

    #[test]
    fn earth_has_plates_mars_and_the_moon_are_dead() {
        assert!((internal_activity(1.0, 4.6) - 0.58).abs() < 0.05);
        assert!(internal_activity(0.107, 4.6) < 0.05);
        assert!(internal_activity(0.0123, 4.5) < 0.01);
        let mut rng = LayerRng::new(1, Layer::Geology);
        let earth = generate(&input(1.0, 4.6, true, 1.0), 1, &mut rng);
        assert_eq!(earth.tectonics, Tectonics::Plates);
        assert!(earth.surface_age_gyr < 0.6 && earth.magnetic_field > 0.5 && earth.quakes > 0.5);
        assert!(earth.relief.plates >= 6 && earth.relief.mountains > 0.0);
        // Vénus : chaude mais sèche, une seule plaque
        let venus = generate(&input(0.815, 4.6, false, 92.0), 2, &mut rng);
        assert_eq!(venus.tectonics, Tectonics::StagnantLid);
        // Mars : éteinte, vieille, criblée de cratères, sans champ magnétique
        let mars = generate(&input(0.107, 4.6, false, 0.006), 3, &mut rng);
        assert_eq!(mars.tectonics, Tectonics::Inactive);
        assert_eq!(mars.magnetic_field, 0.0);
        assert!(mars.surface_age_gyr > 3.5 && mars.relief.craters > 0.5, "{mars:?}");
        // Une atmosphère épaisse et la pluie effacent les cratères
        assert!(earth.relief.craters < 0.2);
    }

    #[test]
    fn tides_keep_a_small_moon_volcanic() {
        let mut rng = LayerRng::new(8, Layer::Geology);
        let io = generate(&GeoInput { tidal: 0.6, ..input(0.015, 4.6, false, 0.0) }, 8, &mut rng);
        assert!(io.activity >= 0.6 && io.volcanism > 0.3 && io.relief.volcanoes > 4, "{io:?}");
        assert!(io.relief.craters < 0.5, "surface renouvelee : peu de crateres");
    }

    #[test]
    fn volcanoes_are_taller_under_low_gravity() {
        let mut low = LayerRng::new(5, Layer::Geology);
        let mut high = LayerRng::new(5, Layer::Geology);
        let a = generate(&GeoInput { gravity: 0.38, ..input(0.5, 1.0, false, 0.01) }, 5, &mut low);
        let b = generate(&GeoInput { gravity: 2.0, ..input(0.5, 1.0, false, 0.01) }, 5, &mut high);
        assert!(a.relief.volcano_height > b.relief.volcano_height * 1.5);
    }

    fn sample(field: &ReliefField, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| {
                let d = sphere_point((i as f32 + 0.5) / n as f32, (i as f32 * 0.618_034).fract());
                field.offset(d, 0.5)
            })
            .collect()
    }

    #[test]
    fn mountains_volcanoes_and_craters_shape_the_ground() {
        let flat = ReliefField::new(Relief::default());
        assert!(sample(&flat, 500).iter().all(|&h| h == 0.0));

        let plates = ReliefField::new(Relief { seed: 9, plates: 10, mountains: 0.2, rifts: 0.08, ..Default::default() });
        let h = sample(&plates, 20_000);
        assert!(h.iter().any(|&x| x > 0.1), "pas de montagnes");
        assert!(h.iter().any(|&x| x < -0.03), "pas de rifts");
        // Les chaînes restent des bandes : la plupart du sol est plat
        let busy = h.iter().filter(|&&x| x.abs() > 0.005).count() as f32 / h.len() as f32;
        assert!(busy > 0.05 && busy < 0.6, "{busy}");

        let volcanic = ReliefField::new(Relief { seed: 3, volcanoes: 20, volcano_height: 0.3, ..Default::default() });
        assert!(sample(&volcanic, 20_000).iter().cloned().fold(0.0, f32::max) > 0.15);

        let cratered = ReliefField::new(Relief { seed: 4, craters: 0.9, crater_depth: 0.07, ..Default::default() });
        let c = sample(&cratered, 20_000);
        assert!(c.iter().any(|&x| x < -0.03) && c.iter().any(|&x| x > 0.005), "bols et rebords");
        // L'érosion efface les cratères
        let eroded = ReliefField::new(Relief { erosion: 0.9, ..cratered.relief });
        let e = sample(&eroded, 20_000);
        let depth = |v: &[f32]| v.iter().cloned().fold(0.0, f32::min);
        assert!(depth(&e) > depth(&c) * 0.3);
    }

    /// B3 : cratère au sommet des volcans, coulées figées sur leurs flancs.
    #[test]
    fn volcanoes_have_a_summit_crater_and_lava_flows() {
        let field = ReliefField::new(Relief { seed: 3, volcanoes: 1, volcano_height: 0.3, ..Default::default() });
        let v = field.volcanoes[0];
        let side = v.center.any_orthonormal_vector();
        let at = |d: f32| field.offset((v.center + side * (d * v.radius).tan()).normalize(), 0.5);
        let crater = match v.kind {
            VolcanoKind::Shield => 0.08,
            VolcanoKind::Cone => 0.1,
            VolcanoKind::Caldera => 0.25,
        };
        assert!(at(0.0) < at(crater * 1.2), "pas de cratere : {} vs {}", at(0.0), at(crater * 1.2));
        assert!(at(crater * 1.2) > at(0.9), "le sommet est plus haut que le pied");
        let lava = (0..2000)
            .filter(|&k| {
                let a = k as f32 * 0.0031;
                let d = 0.2 + 0.7 * (k as f32 * 0.618).fract();
                let around = bevy::math::Quat::from_axis_angle(v.center, a * std::f32::consts::TAU) * side;
                field.sample((v.center + around * (d * v.radius).tan()).normalize(), 0.5).lava
            })
            .count();
        assert!(lava > 20, "{lava} points de coulees");
    }

    /// B3 : mesas aux falaises nettes (plateaux plats, marches abruptes), alignées sur la mer :
    /// la côte ne bouge pas.
    #[test]
    fn mesas_have_flat_tops_sharp_cliffs_and_keep_the_coast() {
        let sea = 0.47;
        let field = ReliefField::new(Relief { seed: 2, terraces: 1.0, ..Default::default() }).with_sea(sea);
        let mut flat = 0;
        let mut cliffs = 0;
        let n = 4000;
        let mut prev: Option<f32> = None;
        for i in 0..n {
            let base = 0.2 + 0.6 * i as f32 / n as f32;
            let h = base + field.offset(Vec3::Y, base);
            // La mer garde sa place
            assert_eq!(base < sea, h < sea, "base {base} -> {h}");
            if let Some(p) = prev {
                let step = h - p;
                if step.abs() < 1e-5 {
                    flat += 1;
                }
                if step > 0.6 / n as f32 * 5.0 {
                    cliffs += 1;
                }
            }
            prev = Some(h);
        }
        assert!(flat > n / 2, "plateaux : {flat}");
        assert!(cliffs > 10, "falaises : {cliffs}");
    }

    /// B4 : formes selon la taille (cuvette, pic central et fond plat, anneaux), fond rempli plat.
    #[test]
    fn crater_shapes_follow_their_size() {
        assert_eq!(CraterKind::of(0.005), CraterKind::Simple);
        assert_eq!(CraterKind::of(0.03), CraterKind::Complex);
        assert_eq!(CraterKind::of(0.15), CraterKind::Basin);
        let simple = |x| CraterKind::Simple.profile(x, false);
        assert!(simple(0.0) < simple(0.5) && simple(0.5) < simple(1.0) && simple(1.0) > simple(1.4), "cuvette et rebord");
        let complex = |x| CraterKind::Complex.profile(x, false);
        assert!(complex(0.0) > complex(0.35) + 0.3, "pic central");
        assert!((complex(0.3) - complex(0.45)).abs() < 1e-3, "fond plat");
        let basin = |x| CraterKind::Basin.profile(x, false);
        assert!(basin(0.7) > basin(0.6) && basin(0.7) > basin(0.8), "anneau");
        // Rempli : fond plat, sans pic
        let filled = |x| CraterKind::Complex.profile(x, true);
        assert!((filled(0.0) - filled(0.5)).abs() < 1e-3);
    }

    /// B4 : loi de puissance (bien plus de petits cratères que de grands), éjectas et rayons
    /// clairs autour des récents, fonds remplis ; rien sur une surface jeune.
    #[test]
    fn craters_follow_a_power_law_with_rays_and_filled_floors() {
        let field = ReliefField::new(Relief { seed: 4, craters: 0.9, crater_depth: 0.07, ..Default::default() }).with_min_crater(0.0);
        let mut per_class = [0usize; 7];
        for (k, &scale) in CRATER_CLASSES.iter().enumerate() {
            // Cratères qui touchent la sphère, comptés par cellule de leur classe
            let n = (scale * 2.0) as i32 + 1;
            for x in -n..n {
                for y in -n..n {
                    for z in -n..n {
                        let hash = hash3(x, y, z, 4u32 ^ (k as u32).wrapping_mul(0x9E37_79B9));
                        let center = Vec3::new(x as f32 + unit(hash.rotate_left(8)), y as f32 + unit(hash.rotate_left(16)), z as f32 + unit(hash.rotate_left(24)));
                        if unit(hash) < 0.54 && (center.length() - scale).abs() < 0.3 {
                            per_class[k] += 1;
                        }
                    }
                }
            }
        }
        for k in 1..4 {
            let ratio = per_class[k] as f32 / per_class[k - 1].max(1) as f32;
            assert!((2.5..6.0).contains(&ratio), "classe {k} : {per_class:?}");
        }
        let (mut bright, mut flooded) = (0, 0);
        for i in 0..20_000 {
            let z = 1.0 - 2.0 * (i as f32 + 0.5) / 20_000.0;
            let a = i as f32 * 2.399_963;
            let rr = (1.0 - z * z).sqrt();
            let s = field.sample(Vec3::new(rr * a.cos(), z, rr * a.sin()), 0.5);
            bright += (s.bright > 0.3) as usize;
            flooded += s.flooded as usize;
        }
        assert!(bright > 100 && flooded > 100, "eclat {bright}, remplis {flooded}");
        let young = ReliefField::new(Relief { seed: 4, craters: 0.0, ..Default::default() });
        assert!(sample(&young, 2000).iter().all(|&h| h == 0.0));
    }

    /// B4 : la Lune est criblée de cratères, Mars en a beaucoup, la Terre presque aucun.
    #[test]
    fn the_moon_mars_and_the_earth_have_their_craters() {
        let cratered = |relief: Relief| {
            let field = ReliefField::new(Relief { plates: 0, volcanoes: 0, canyons: 0.0, terraces: 0.0, ..relief });
            sample(&field, 20_000).iter().filter(|h| h.abs() > 0.004).count() as f32 / 20_000.0
        };
        let moon = cratered(moon_relief(1));
        let mut rng = LayerRng::new(1, Layer::Geology);
        let mars = cratered(generate(&GeoInput { liquid_water: false, ..input(0.107, 4.6, false, 0.006) }, 1, &mut rng).relief);
        let mut rng = LayerRng::new(1, Layer::Geology);
        let earth = cratered(generate(&input(1.0, 4.6, true, 1.0), 1, &mut rng).relief);
        assert!(moon > 0.3, "lune {moon}");
        assert!(mars > 0.1, "mars {mars}");
        assert!(earth < 0.02, "terre {earth}");
    }

    /// B3 : une chaîne jeune a des sommets plus pointus qu'une chaîne érodée.
    #[test]
    fn young_ranges_are_sharper_than_eroded_ones() {
        let young = ReliefField::new(Relief { seed: 9, plates: 10, mountains: 0.2, ..Default::default() });
        let old = ReliefField::new(Relief { erosion: 0.9, ..young.relief });
        let (hy, ho) = (sample(&young, 20_000), sample(&old, 20_000));
        let peak = |v: &[f32]| v.iter().cloned().fold(0.0, f32::max);
        assert!(peak(&hy) > peak(&ho) * 1.3, "{} vs {}", peak(&hy), peak(&ho));
    }
}
