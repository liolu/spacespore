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

/// Formes du relief d'un astre, prêtes à être évaluées colonne par colonne.
pub struct ReliefField {
    relief: Relief,
    /// Centre de chaque plaque et son mouvement (tangent).
    plates: Vec<(Vec3, Vec3)>,
    volcanoes: Vec<Volcano>,
    ridges: Perlin,
    canyons: Perlin,
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
/// Cratères : deux tailles, cellules par unité de rayon.
const CRATER_SCALES: [(f32, f32); 2] = [(5.0, 1.0), (16.0, 0.45)];

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
        Self { relief, plates, volcanoes, ridges: Perlin::new(s.wrapping_add(400)), canyons: Perlin::new(s.wrapping_add(500)) }
    }

    /// Rien à ajouter (planète faite à la main, géante).
    pub fn is_flat(&self) -> bool {
        let r = &self.relief;
        r.plates == 0 && r.volcanoes == 0 && r.canyons == 0.0 && r.craters <= 0.0 && r.terraces <= 0.0
    }

    /// Hauteur relative ajoutée dans la direction `dir` (unitaire), sur un relief de base `base`.
    pub fn offset(&self, dir: Vec3, base: f32) -> f32 {
        let r = &self.relief;
        if self.is_flat() {
            return 0.0;
        }
        let soft = 1.0 - 0.5 * r.erosion;
        let mut h = 0.0;

        // Plateaux : marches douces sur le relief de base
        if r.terraces > 0.0 {
            let steps = 14.0;
            let x = base * steps;
            let f = x.fract();
            let stepped = (x.floor() + f * f * f * (f * (f * 6.0 - 15.0) + 10.0)) / steps;
            h += (stepped - base) * r.terraces;
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
                    // Crêtes irrégulières le long de la chaîne
                    let n = self.ridges.get([dir.x as f64 * 14.0, dir.y as f64 * 14.0, dir.z as f64 * 14.0]) as f32;
                    let ridge = 1.0 - n.abs();
                    h += r.mountains * w * w * (0.45 + 0.55 * ridge) * soft;
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
            h += soft
                * match v.kind {
                    VolcanoKind::Shield => v.height * (1.0 - d).powf(1.5),
                    VolcanoKind::Cone => v.height * (1.0 - d).powf(2.6),
                    VolcanoKind::Caldera => {
                        let pit = if d < 0.25 { v.height * 0.6 * (1.0 - d / 0.25) } else { 0.0 };
                        v.height * (1.0 - d).powf(1.2) - pit
                    }
                };
        }

        // Canyons et failles : sillons étroits et sinueux
        if r.canyons > 0.0 {
            let n = self.canyons.get([dir.x as f64 * 3.0, dir.y as f64 * 3.0, dir.z as f64 * 3.0]) as f32;
            let w = 0.035;
            if n.abs() < w {
                let t = 1.0 - n.abs() / w;
                h -= r.canyons * t * t * soft;
            }
        }

        // Cratères : bol creusé et rebord surélevé
        if r.craters > 0.0 {
            let fade = 1.0 - 0.9 * r.erosion;
            for (k, &(scale, depth_scale)) in CRATER_SCALES.iter().enumerate() {
                let p = dir * scale;
                let (cx, cy, cz) = (p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
                for dx in -1..=1 {
                    for dy in -1..=1 {
                        for dz in -1..=1 {
                            let (x, y, z) = (cx + dx, cy + dy, cz + dz);
                            let hash = hash3(x, y, z, r.seed ^ (k as u32 * 0x9E37_79B9));
                            if unit(hash) >= r.craters * 0.6 {
                                continue;
                            }
                            let center = Vec3::new(
                                x as f32 + unit(hash.rotate_left(8)),
                                y as f32 + unit(hash.rotate_left(16)),
                                z as f32 + unit(hash.rotate_left(24)),
                            );
                            let radius = 0.15 + 0.3 * unit(hash.wrapping_mul(0x9E37_79B9));
                            let x = p.distance(center) / radius;
                            if x < 1.4 {
                                let depth = r.crater_depth * depth_scale * fade;
                                let bowl = if x < 1.0 { -(1.0 - x * x) } else { 0.0 };
                                let rim = 0.35 * (-((x - 1.0) / 0.2).powi(2)).exp();
                                h += depth * (bowl + rim);
                            }
                        }
                    }
                }
            }
        }
        h
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
}
