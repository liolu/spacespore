// ─────────────────────────────────────────────────────────────────────────
//  Relief à l'échelle du marcheur (0.13 T1, `roadmaps/a-faire/ROADMAP-0.13.md`)
//
//  La forme générale (continents, plaques, mers) reste en fraction du rayon (`geology.rs`) ; ces
//  couches-ci ont une hauteur et une largeur en **voxels** (règle 14) :
//    - déformation du domaine (crêtes et vallées tordues) ;
//    - collines (8 à 25 voxels de haut, ~100 voxels de large) ;
//    - massifs : crêtes serrées et hautes le long des plaques (et quelques massifs régionaux) ;
//    - bosses (2 à 6 voxels, ~12 voxels de large) ;
//    - vallées creusées par l'érosion (mondes à air et à eau).
//  Une seule fonction (`Landforms::offset`), partagée par le terrain proche (`terrain.rs`) et le
//  maillage vu de l'espace (`mesher.rs`) : un sommet vu de l'espace est le même à pied (règle 16).
// ─────────────────────────────────────────────────────────────────────────

use bevy::math::Vec3;
use noise::{NoiseFn, Perlin};

use super::geology::Relief;

/// Largeurs (voxels) des couches.
const WARP_WAVE: f32 = 450.0;
const WARP_VOXELS: f32 = 70.0;
const HILL_WAVE: f32 = 110.0;
const MASSIF_WAVE: f32 = 340.0;
const REGION_WAVE: f32 = 4_000.0;
const BUMP_WAVE: f32 = 12.0;
const VALLEY_WAVE: f32 = 700.0;

/// Le relief en voxels d'un astre.
#[derive(Clone)]
pub struct Landforms {
    noise: [Perlin; 8],
    radius: f32,
    voxel: f32,
    /// Hauteurs (voxels) : collines, massifs, bosses, profondeur des vallées.
    hill: f32,
    massif: f32,
    bump: f32,
    valley: f32,
    /// Part des massifs hors des chaînes de plaques (massifs régionaux).
    regional: f32,
}

/// Fraction 0..1 d'une graine (tirage fixe par couche).
fn unit(seed: u32, k: u32) -> f32 {
    let mut x = (seed as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (k as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    x ^= x >> 31;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 29;
    (x >> 40) as f32 / (1u64 << 24) as f32
}

fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

impl Landforms {
    /// `wet` : air et eau liquide (vallées d'érosion) ; `airless` : relief plus vif, pas d'érosion.
    pub fn new(seed: u32, radius: f32, voxel: f32, relief: &Relief, airless: bool, wet: bool) -> Self {
        let noise = std::array::from_fn(|i| Perlin::new(seed.wrapping_add(4_001 + i as u32 * 97)));
        let erosion = if airless { 0.0 } else { relief.erosion.clamp(0.0, 1.0) };
        // Collines : 8 à 25 voxels selon l'astre, adoucies par l'érosion
        let hill = (8.0 + 17.0 * unit(seed, 1)) * (1.0 - 0.45 * erosion) * if airless { 0.7 } else { 1.0 };
        // Massifs : 120 à 400 voxels avec des plaques actives, plus bas sans tectonique
        let tect = if relief.plates > 0 { ((relief.mountains - 0.08) / 0.2).clamp(0.0, 1.0) } else { 0.0 };
        let massif = if relief.plates > 0 { 120.0 + 280.0 * tect } else { 60.0 + 60.0 * unit(seed, 2) } * (1.0 - 0.35 * erosion);
        let bump = (2.0 + 4.0 * unit(seed, 3)) * if airless { 1.2 } else { 1.0 };
        let valley = if wet { (20.0 + 40.0 * unit(seed, 4)) * (0.4 + erosion) } else { 0.0 };
        let regional = 0.25 + 0.35 * unit(seed, 5);
        Self { noise, radius: radius.max(1.0), voxel: voxel.max(1e-4), hill, massif, bump, valley, regional }
    }

    /// Fréquence (dans l'espace des directions unitaires) d'une largeur de `wave` voxels.
    fn freq(&self, wave: f32) -> f64 {
        (self.radius / (wave * self.voxel)) as f64
    }

    fn at(&self, i: usize, p: Vec3, f: f64) -> f32 {
        self.noise[i].get([p.x as f64 * f, p.y as f64 * f, p.z as f64 * f]) as f32
    }

    /// Bruit fractal (somme d'octaves) normalisé à -1..1.
    fn fbm(&self, i: usize, p: Vec3, f: f64, octaves: u32) -> f32 {
        let (mut sum, mut amp, mut norm, mut f) = (0.0, 1.0, 0.0, f);
        for o in 0..octaves {
            sum += self.at(i, p + Vec3::splat(o as f32 * 17.3), f) * amp;
            norm += amp;
            amp *= 0.5;
            f *= 2.03;
        }
        sum / norm
    }

    /// Crêtes (0..1, pointues en haut).
    fn ridged(&self, i: usize, p: Vec3, f: f64, octaves: u32) -> f32 {
        let (mut sum, mut amp, mut norm, mut f, mut weight) = (0.0, 1.0, 0.0, f, 1.0f32);
        for o in 0..octaves {
            let r = (1.0 - self.at(i, p + Vec3::splat(o as f32 * 29.1), f).abs()).powi(2) * weight;
            weight = (r * 1.6).clamp(0.0, 1.0);
            sum += r * amp;
            norm += amp;
            amp *= 0.5;
            f *= 2.07;
        }
        sum / norm
    }

    /// Hauteur (unités) à ajouter au sol dans la direction `dir` : `land` = 0 sous la mer, 1 sur les
    /// terres ; `mountain` = poids des chaînes de montagnes de `geology.rs` (0..1) ; `fine` = avec les
    /// bosses (le maillage vu de l'espace s'en passe : plus petites que ses cellules).
    pub fn offset(&self, dir: Vec3, land: f32, mountain: f32, fine: bool) -> f32 {
        if land <= 0.0 && self.valley <= 0.0 {
            return 0.0;
        }
        // Déformation du domaine : les formes se tordent au lieu d'être régulières
        let wf = self.freq(WARP_WAVE);
        let wa = WARP_VOXELS * self.voxel / self.radius;
        let warp = Vec3::new(self.at(0, dir, wf), self.at(1, dir + Vec3::splat(5.2), wf), self.at(2, dir - Vec3::splat(3.7), wf));
        let q = dir + warp * wa;

        // Collines
        let hills = (self.fbm(3, q, self.freq(HILL_WAVE), 3) * 0.5 + 0.5) * self.hill;

        // Massifs : dans les chaînes des plaques, et des massifs régionaux ailleurs
        let region = smooth((self.at(4, dir, self.freq(REGION_WAVE)) - 0.15) / 0.35) * self.regional;
        let mask = mountain.max(region);
        let massif = if mask > 0.01 { self.ridged(5, q, self.freq(MASSIF_WAVE), 4).powf(1.6) * self.massif * mask } else { 0.0 };

        // Vallées d'érosion : là où un bruit tordu passe par zéro, sur les terres
        let valley = if self.valley > 0.0 {
            let n = self.at(6, q, self.freq(VALLEY_WAVE)).abs();
            let v = (1.0 - n / 0.09).clamp(0.0, 1.0);
            v * v * self.valley * (1.0 - 0.6 * mask)
        } else {
            0.0
        };

        let bumps = if fine { self.fbm(7, dir, self.freq(BUMP_WAVE), 2) * self.bump } else { 0.0 };
        let land = land.clamp(0.0, 1.0);
        // Sous la mer : un fond moins plat (un cinquième du relief)
        let k = 0.2 + 0.8 * land;
        (k * (hills + massif + bumps) - land * valley) * self.voxel
    }

    /// Plus grande hauteur ajoutée (unités) : marge des maillages.
    pub fn max_height(&self) -> f32 {
        (self.hill + self.massif + self.bump) * self.voxel
    }
}

/// Masque des terres : 0 sous la mer, 1 un peu au-dessus (hauteur relative `hv`).
pub fn land_mask(hv: f32, sea: f32) -> f32 {
    smooth((hv - sea) / 0.025)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn forms(voxel: f32, airless: bool, wet: bool) -> Landforms {
        Landforms::new(1234, 9_000.0, voxel, &Relief { seed: 9, plates: 8, mountains: 0.2, erosion: 0.3, ..Default::default() }, airless, wet)
    }

    /// Les pentes vont de 0 à ~60° (une part réelle au-dessus de 15° et 30°), sans pic absurde.
    #[test]
    fn slopes_are_walkable_and_varied() {
        let v = 0.43;
        let f = forms(v, false, true);
        let mut slopes = Vec::new();
        let mut spikes = 0;
        for i in 0..4_000 {
            let a = i as f32 * 2.399;
            let z = 1.0 - 2.0 * (i as f32 + 0.5) / 4_000.0;
            let dir = Vec3::new(a.cos() * (1.0 - z * z).sqrt(), z, a.sin() * (1.0 - z * z).sqrt());
            let east = Vec3::Y.cross(dir).normalize_or(Vec3::X);
            let mountain = if i % 3 == 0 { 1.0 } else { 0.0 };
            let h = |d: Vec3| f.offset(d.normalize(), 1.0, mountain, true);
            let step = v / 9_000.0;
            let h0 = h(dir);
            let h1 = h(dir + east * step);
            slopes.push(((h1 - h0) / v).atan().to_degrees().abs());
            // Pic isolé : beaucoup plus haut que tous ses voisins à 4 voxels
            let around = [east, dir.cross(east)].iter().flat_map(|e| [h(dir + *e * step * 4.0), h(dir - *e * step * 4.0)]).fold(f32::MIN, f32::max);
            if h0 - around > 40.0 * v {
                spikes += 1;
            }
        }
        let share = |deg: f32| slopes.iter().filter(|s| **s > deg).count() as f32 / slopes.len() as f32;
        let (s15, s30, s45, s70) = (share(15.0), share(30.0), share(45.0), share(70.0));
        println!("pentes > 15 : {:.0} %, > 30 : {:.0} %, > 45 : {:.0} %, > 70 : {:.1} %", s15 * 100.0, s30 * 100.0, s45 * 100.0, s70 * 100.0);
        assert!(s15 > 0.10 && s30 > 0.02 && s45 < 0.25 && s70 < 0.03, "{s15} {s30} {s45} {s70}");
        assert_eq!(spikes, 0);
    }

    /// Les hauteurs sont en voxels : même relief en voxels quelle que soit la taille du voxel ;
    /// les massifs sont bien plus hauts que les collines ; rien sous la mer en dehors d'un peu de fond.
    #[test]
    fn heights_are_in_voxels() {
        let dir = Vec3::new(0.3, 0.8, -0.5).normalize();
        let a = forms(0.43, false, false);
        let b = forms(0.86, false, false);
        let ha = a.offset(dir, 1.0, 1.0, false) / 0.43;
        let hb = b.offset(dir * 1.0, 1.0, 1.0, false) / 0.86;
        assert!(ha.is_finite() && hb.is_finite());
        assert!(a.massif > 3.0 * a.hill && a.massif <= 400.0 && (8.0..=25.0).contains(&(a.hill / (1.0 - 0.45 * 0.3))));
        let sea = a.offset(dir, 0.0, 0.0, true).abs() / 0.43;
        assert!(sea <= 0.2 * (a.hill + a.massif + a.bump) + 1e-3);
        assert_eq!(land_mask(0.3, 0.4), 0.0);
        assert_eq!(land_mask(0.5, 0.4), 1.0);
    }
}
