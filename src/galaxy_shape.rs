// ─────────────────────────────────────────────────────────────────────────
//  Formes de galaxies
//
//  Une galaxie n'est plus « un centre + quelques bras » : chaque `GalaxyKind`
//  construit une structure globale (`Shape`) faite de courbes (bras, anneaux,
//  filaments, ponts, queues…) et d'un fond diffus (disque, ellipsoïde, amas).
//  Les étoiles, les capsules lumineuses et les nuages de gaz échantillonnent
//  tous la même `Shape` : la silhouette est donc identique partout.
//
//  Tout est déterministe : la forme ne dépend que de `GalaxyConfig`.
// ─────────────────────────────────────────────────────────────────────────

use bevy::math::{Quat, Vec3};

use crate::settings::{pseudo_rand, GalaxyConfig};

const TAU: f32 = std::f32::consts::TAU;

/// Les types de galaxies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GalaxyKind {
    Spiral,
    Barred,
    Lenticular,
    Ring,
    MultiArm,
    Flocculent,
    Asymmetric,
    Elliptical,
    Cigar,
    Irregular,
    TightSpiral,
    OpenSpiral,
    SShape,
    Bipolar,
    Warped,
    Interacting,
    MultiRing,
    Fractal,
    Wavy,
    Filamentary,
}

impl GalaxyKind {
    pub const ALL: [GalaxyKind; 20] = [
        Self::Spiral, Self::Barred, Self::Lenticular, Self::Ring, Self::MultiArm,
        Self::Flocculent, Self::Asymmetric, Self::Elliptical, Self::Cigar, Self::Irregular,
        Self::TightSpiral, Self::OpenSpiral, Self::SShape, Self::Bipolar, Self::Warped,
        Self::Interacting, Self::MultiRing, Self::Fractal, Self::Wavy, Self::Filamentary,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Spiral => "Spirale classique",
            Self::Barred => "Spirale barrée",
            Self::Lenticular => "Lenticulaire",
            Self::Ring => "Annulaire",
            Self::MultiArm => "Spirale multi-bras",
            Self::Flocculent => "Floculente",
            Self::Asymmetric => "Spirale asymétrique",
            Self::Elliptical => "Elliptique",
            Self::Cigar => "Elliptique en cigare",
            Self::Irregular => "Irrégulière",
            Self::TightSpiral => "Spirale logarithmique serrée",
            Self::OpenSpiral => "Spirale ouverte",
            Self::SShape => "Bras en S",
            Self::Bipolar => "Bipolaire",
            Self::Warped => "Spirale tordue",
            Self::Interacting => "En interaction",
            Self::MultiRing => "Anneaux multiples",
            Self::Fractal => "Fractale",
            Self::Wavy => "Spirale ondulante",
            Self::Filamentary => "Filamentaire",
        }
    }

    /// Type de la galaxie extérieure n° `gi` : les 20 types reviennent à parts égales, mélangés
    /// différemment selon le monde (7 est premier avec 20 : deux voisines diffèrent toujours).
    pub fn for_index(gi: usize, world_hash: u32) -> Self {
        let offset = (world_hash % 20) as usize;
        Self::ALL[(gi * 7 + offset) % Self::ALL.len()]
    }
}

/// Petit générateur pseudo-aléatoire déterministe.
pub struct Rng(u32);

impl Rng {
    pub fn new(seed: u32) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B1) ^ 0x85EB_CA6B)
    }
    pub fn f(&mut self) -> f32 {
        self.0 = self.0.wrapping_add(0x9E37_79B9);
        pseudo_rand(self.0)
    }
    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f()
    }
    fn sign(&mut self) -> f32 {
        if self.f() < 0.5 { -1.0 } else { 1.0 }
    }
}

/// Capsules lumineuses posées le long d'une courbe (visibles de loin).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapMode {
    /// Série dense, comme les bras d'une spirale.
    Arm,
    /// Répartie tout le long d'une boucle.
    Loop,
    /// Une seule capsule au milieu.
    Few,
    None,
}

#[derive(Clone, Debug)]
pub struct Curve {
    pub pts: Vec<Vec3>,
    /// Dispersion latérale des étoiles (fraction du rayon, à l'extrémité).
    pub width: f32,
    /// Part des étoiles de structure portées par cette courbe.
    pub weight: f32,
    pub caps: CapMode,
}

impl Curve {
    fn new(pts: Vec<Vec3>, width: f32, weight: f32, caps: CapMode) -> Self {
        debug_assert!(pts.len() >= 2);
        Self { pts, width, weight, caps }
    }

    /// Point et tangente au paramètre `p` (0..1).
    pub fn at(&self, p: f32) -> (Vec3, Vec3) {
        let n = self.pts.len();
        let f = p.clamp(0.0, 1.0) * (n - 1) as f32;
        let i = (f as usize).min(n - 2);
        let t = f - i as f32;
        let (a, b) = (self.pts[i], self.pts[i + 1]);
        (a.lerp(b, t), (b - a).normalize_or_zero())
    }

    #[cfg(test)]
    fn length(&self) -> f32 {
        self.pts.windows(2).map(|w| w[0].distance(w[1])).sum()
    }
}

/// Fond diffus de la galaxie.
#[derive(Clone, Debug)]
pub enum Background {
    /// Disque plat, dense au centre.
    Disk { reach: f32, thick: f32 },
    /// Ellipsoïde (demi-axes), dense au centre.
    Ellipsoid(Vec3),
    /// Amas : (centre, rayon, poids).
    Blobs(Vec<(Vec3, f32, f32)>),
}

#[derive(Clone, Debug)]
pub struct Shape {
    pub curves: Vec<Curve>,
    pub background: Background,
    /// Part des étoiles placées sur les courbes (le reste va dans le fond).
    pub structure_share: f32,
    thickness: f32,
    radius: f32,
    total_weight: f32,
    /// Étirement du disque (1 = rond) et rotation, appliqués au fond diffus.
    bg_stretch: f32,
    bg_yaw: Quat,
}

// ── Aides de construction ───────────────────────────────────────────────

fn pts_from(n: usize, f: impl Fn(f32) -> Vec3) -> Vec<Vec3> {
    (0..n).map(|i| f(i as f32 / (n - 1) as f32)).collect()
}

fn polar(r: f32, theta: f32, y: f32) -> Vec3 {
    Vec3::new(r * theta.cos(), y, r * theta.sin())
}

/// Bras spiral de la forme historique : r = p²·R, θ = base + p²·twist.
fn spiral_arm(radius: f32, base: f32, twist: f32, reach: f32) -> Vec<Vec3> {
    pts_from(48, |p| polar(p * p * radius * reach, base + p * p * twist * reach, 0.0))
}

/// Boucle irrégulière de rayon moyen `r`, parcourant `arc` radians.
fn ring(rng: &mut Rng, r: f32, arc: f32, start: f32, wobble: f32) -> Vec<Vec3> {
    let (a2, a3) = (rng.range(0.0, wobble), rng.range(0.0, wobble * 0.7));
    let (f2, f3) = (rng.f() * TAU, rng.f() * TAU);
    pts_from(56, |p| {
        let th = start + p * arc;
        let rr = r * (1.0 + a2 * (2.0 * th + f2).sin() + a3 * (3.0 * th + f3).sin());
        polar(rr, th, 0.0)
    })
}

fn translate(pts: &mut [Vec3], off: Vec3) {
    for p in pts {
        *p += off;
    }
}

/// Arc de Bézier quadratique.
fn quad(a: Vec3, c: Vec3, b: Vec3, n: usize) -> Vec<Vec3> {
    pts_from(n, |t| a * (1.0 - t) * (1.0 - t) + c * 2.0 * t * (1.0 - t) + b * t * t)
}

/// Marche aléatoire à direction persistante (filaments, tentacules).
fn wander(rng: &mut Rng, start: Vec3, heading: f32, len: f32, steps: usize, turn: f32) -> Vec<Vec3> {
    let mut p = start;
    let mut h = heading;
    let mut out = vec![p];
    for _ in 0..steps {
        h += (rng.f() - 0.5) * turn;
        p += Vec3::new(h.cos(), 0.0, h.sin()) * len / steps as f32;
        out.push(p);
    }
    out
}

fn fractal_branches(rng: &mut Rng, out: &mut Vec<Curve>, pts: &[Vec3], depth: u32, weight: f32) {
    if depth == 0 {
        return;
    }
    let parent_len: f32 = pts.windows(2).map(|w| w[0].distance(w[1])).sum();
    for (at, side) in [(0.45_f32, 1.0_f32), (0.75, -1.0)] {
        let idx = (((pts.len() - 1) as f32 * at) as usize).min(pts.len() - 2);
        let start = pts[idx];
        let mut dir = Quat::from_rotation_y(side * rng.range(0.5, 1.0))
            * (pts[idx + 1] - pts[idx]).normalize_or_zero();
        let step = parent_len * 0.4 * rng.range(0.8, 1.1) / 9.0;
        let mut p = start;
        let mut child = vec![p];
        for _ in 0..9 {
            dir = Quat::from_rotation_y(side * 0.12) * dir;
            p += dir * step;
            child.push(p);
        }
        out.push(Curve::new(child.clone(), 0.03, weight, CapMode::None));
        fractal_branches(rng, out, &child, depth - 1, weight * 0.6);
    }
}

impl Shape {
    pub fn build(gal: &GalaxyConfig) -> Self {
        let r = gal.radius;
        let mut rng = Rng::new(gal.seed ^ 0x5EED_1234);
        let arms = gal.num_arms.clamp(2, 6);
        let tw = gal.twist;
        let mut curves: Vec<Curve> = Vec::new();
        let mut background = Background::Disk { reach: 0.85, thick: 16_000.0 };
        let mut share = 0.8;
        let mut thickness = 24_000.0;
        let phase = rng.f() * TAU;

        match gal.kind {
            GalaxyKind::Spiral => {
                let n = gal.num_arms.max(1);
                for a in 0..n {
                    let base = a as f32 * TAU / n as f32;
                    curves.push(Curve::new(spiral_arm(r, base, tw, 1.0), 0.45, 1.0, CapMode::Arm));
                }
            }

            GalaxyKind::Barred => {
                let bar = 0.33 * r;
                for s in [1.0_f32, -1.0] {
                    let (bx, bz) = (phase.cos() * s, phase.sin() * s);
                    let dir = Vec3::new(bx, 0.0, bz);
                    // Barre lumineuse
                    curves.push(Curve::new(
                        pts_from(10, |p| dir * bar * p), 0.05, 1.3, CapMode::Few));
                    // Bras qui partent du bout de la barre
                    let th0 = bz.atan2(bx);
                    let sweep = rng.range(2.2, 3.4);
                    curves.push(Curve::new(
                        pts_from(48, |p| polar(bar + (r - bar) * p.powf(1.15), th0 + sweep * p, 0.0)),
                        0.22, 1.6, CapMode::Arm));
                }
                // Bras secondaires discrets
                for s in [1.0_f32, -1.0] {
                    let th0 = phase + s * 1.45;
                    curves.push(Curve::new(
                        pts_from(32, |p| polar(bar * 1.3 + r * 0.55 * p, th0 + 2.0 * p * s, 0.0)),
                        0.14, 0.4, CapMode::None));
                }
            }

            GalaxyKind::Lenticular => {
                thickness = 70_000.0;
                share = 0.35;
                background = Background::Disk { reach: 0.95, thick: 50_000.0 };
                for (i, rr) in [0.3_f32, 0.55, 0.8].iter().enumerate() {
                    let c = ring(&mut rng, r * rr, TAU, 0.0, 0.03);
                    curves.push(Curve::new(c, 0.04, 0.5 - 0.1 * i as f32, CapMode::Loop));
                }
            }

            GalaxyKind::Ring => {
                share = 0.88;
                background = Background::Blobs(vec![(Vec3::ZERO, 0.09 * r, 1.0)]);
                curves.push(Curve::new(ring(&mut rng, r * 0.62, TAU, 0.0, 0.14), 0.09, 1.0, CapMode::Loop));
                curves.push(Curve::new(ring(&mut rng, r * 0.92, TAU, 1.0, 0.1), 0.05, 0.18, CapMode::Loop));
            }

            GalaxyKind::MultiArm => {
                let n = rng.range(7.0, 11.0) as usize;
                for a in 0..n {
                    let base = a as f32 * TAU / n as f32 + rng.range(-0.15, 0.15);
                    let reach = rng.range(0.4, 1.0);
                    let arm = spiral_arm(r, base, tw * rng.range(0.8, 1.2), reach);
                    // Une ramification qui se sépare du bras
                    let mut branch_pts = Vec::new();
                    let at = rng.range(0.45, 0.7);
                    let idx = (((arm.len() - 1) as f32) * at) as usize;
                    let (start, dir) = (arm[idx], (arm[idx + 1] - arm[idx]).normalize_or_zero());
                    let rot = Quat::from_rotation_y(rng.sign() * rng.range(0.4, 0.8));
                    let mut d = rot * dir;
                    let mut p = start;
                    for _ in 0..12 {
                        branch_pts.push(p);
                        d = Quat::from_rotation_y(0.07) * d;
                        p += d * r * 0.02 * reach * 1.6;
                    }
                    let caps = if a < 6 { CapMode::Arm } else { CapMode::Few };
                    curves.push(Curve::new(arm, 0.3, rng.range(0.2, 1.0), caps));
                    if branch_pts.len() >= 2 {
                        curves.push(Curve::new(branch_pts, 0.08, 0.3, CapMode::None));
                    }
                }
            }

            GalaxyKind::Flocculent => {
                share = 0.6;
                let n = 16;
                for i in 0..n {
                    let base = i as f32 * TAU / 5.0 + rng.f() * 1.5;
                    let full = spiral_arm(r, base, tw * 0.8, 1.0);
                    let s = rng.range(0.15, 0.7);
                    let e = (s + rng.range(0.12, 0.25)).min(1.0);
                    let (i0, i1) = ((s * 47.0) as usize, ((e * 47.0) as usize).max((s * 47.0) as usize + 2));
                    let frag = full[i0..=i1.min(47)].to_vec();
                    curves.push(Curve::new(frag, 0.16, rng.range(0.3, 1.0), CapMode::Few));
                }
            }

            GalaxyKind::Asymmetric => {
                let n = arms.min(3);
                for a in 0..n {
                    let base = a as f32 * TAU / n as f32;
                    let (reach, w) = if a == 0 { (1.0, 1.4) } else { (rng.range(0.4, 0.6), 0.4) };
                    let mut c = spiral_arm(r, base, tw, reach);
                    translate(&mut c, Vec3::new(0.09 * r, 0.0, -0.06 * r));
                    curves.push(Curve::new(c, 0.4, w, CapMode::Arm));
                }
                background = Background::Blobs(vec![
                    (Vec3::new(0.09 * r, 0.0, -0.06 * r), 0.25 * r, 1.0),
                    (Vec3::new(-0.3 * r, 0.0, 0.2 * r), 0.12 * r, 0.25),
                ]);
                share = 0.7;
            }

            GalaxyKind::Elliptical => {
                share = 0.0;
                background = Background::Ellipsoid(Vec3::new(0.7 * r, 0.5 * r, 0.6 * r));
                for (i, rr) in [0.25_f32, 0.5].iter().enumerate() {
                    let mut c = ring(&mut rng, r * rr, TAU, 0.0, 0.05);
                    let q = Quat::from_rotation_x(i as f32 * 1.1 + 0.5);
                    for p in &mut c {
                        *p = q * *p;
                    }
                    curves.push(Curve::new(c, 0.05, 0.0, CapMode::Loop));
                }
            }

            GalaxyKind::Cigar => {
                share = 0.25;
                let len = 0.95 * r;
                // Axe long = X local (le tilt de la galaxie l'oriente ensuite)
                let dir = Vec3::X;
                background = Background::Ellipsoid(Vec3::new(len, 0.13 * r, 0.16 * r));
                curves.push(Curve::new(pts_from(12, |p| dir * len * (2.0 * p - 1.0)), 0.05, 1.0, CapMode::Arm));
            }

            GalaxyKind::Irregular => {
                share = 0.4;
                let n = rng.range(5.0, 8.0) as usize;
                let mut blobs = Vec::new();
                for _ in 0..n {
                    let th = rng.f() * TAU;
                    let d = r * rng.range(0.05, 0.7);
                    let c = polar(d, th, 0.0);
                    blobs.push((c, r * rng.range(0.08, 0.22), rng.range(0.3, 1.0)));
                    curves.push(Curve::new(
                        pts_from(4, |p| c + Vec3::new(th.cos(), 0.0, th.sin()) * r * 0.05 * p),
                        0.05, 0.0, CapMode::Few));
                }
                thickness = 60_000.0;
                background = Background::Blobs(blobs);
                for _ in 0..3 {
                    let start = polar(r * rng.range(0.05, 0.25), rng.f() * TAU, 0.0);
                    let heading = rng.f() * TAU;
                    let len = r * rng.range(0.5, 0.9);
                    curves.push(Curve::new(
                        wander(&mut rng, start, heading, len, 14, 1.2),
                        0.07, 0.5, CapMode::Few));
                }
            }

            GalaxyKind::TightSpiral => {
                let n = arms.min(4);
                let tight = rng.range(9.0, 13.0);
                for a in 0..n {
                    let base = a as f32 * TAU / n as f32;
                    // Spirale logarithmique : r = r0·e^(k·θ)
                    curves.push(Curve::new(
                        pts_from(72, |p| {
                            let rr = r * 0.03 * (1.0_f32 / 0.03).powf(p);
                            polar(rr, base + tight * p, 0.0)
                        }),
                        0.18, 1.0, CapMode::Arm));
                }
            }

            GalaxyKind::OpenSpiral => {
                let n = arms.min(3);
                let open = rng.range(1.6, 2.6);
                for a in 0..n {
                    let base = a as f32 * TAU / n as f32;
                    curves.push(Curve::new(
                        pts_from(48, |p| polar(r * p.powf(1.1), base + open * p, 0.0)),
                        0.22, 1.0, CapMode::Arm));
                }
            }

            GalaxyKind::SShape => {
                for s in [1.0_f32, -1.0] {
                    let arm = pts_from(48, |p| {
                        polar(r * p, phase + if s > 0.0 { 0.0 } else { std::f32::consts::PI }
                            + 1.3 * (p * std::f32::consts::PI * 1.5).sin(), 0.0)
                    });
                    // Filaments secondaires au bout de chaque bras
                    for k in 0..2 {
                        let start = arm[38 + k * 4];
                        let d = (arm[39 + k * 4] - arm[38 + k * 4]).normalize_or_zero();
                        let head = d.z.atan2(d.x) + if k == 0 { 0.7 } else { -0.7 };
                        curves.push(Curve::new(
                            wander(&mut rng, start, head, r * 0.3, 10, 0.5), 0.05, 0.3, CapMode::None));
                    }
                    curves.push(Curve::new(arm, 0.2, 1.3, CapMode::Arm));
                }
            }

            GalaxyKind::Bipolar => {
                share = 0.45;
                let dir = Vec3::new(phase.cos(), 0.0, phase.sin());
                background = Background::Blobs(vec![
                    (dir * 0.45 * r, 0.26 * r, 1.0),
                    (-dir * 0.55 * r, 0.17 * r, 0.6),
                ]);
                for s in [1.0_f32, -1.0] {
                    let len = if s > 0.0 { 0.95 * r } else { 0.6 * r };
                    for ang in [-0.6_f32, 0.0, 0.6] {
                        let q = Quat::from_rotation_y(ang);
                        let d = q * dir * s;
                        let c = pts_from(24, |p| {
                            let side = Vec3::new(-d.z, 0.0, d.x);
                            d * len * p + side * (p * 6.0 + ang * 3.0).sin() * 0.02 * r
                        });
                        let caps = if ang == 0.0 { CapMode::Arm } else { CapMode::Few };
                        curves.push(Curve::new(c, 0.12, if ang == 0.0 { 1.2 } else { 0.7 }, caps));
                    }
                }
            }

            GalaxyKind::Warped => {
                let n = arms.min(4);
                let warp = 0.14 * r;
                for a in 0..n {
                    let base = a as f32 * TAU / n as f32;
                    curves.push(Curve::new(
                        pts_from(56, |p| {
                            let th = base + p * p * tw;
                            polar(p * p * r, th, warp * p * p * (th - phase).cos())
                        }),
                        0.4, 1.0, CapMode::Arm));
                }
                thickness = 12_000.0;
                background = Background::Disk { reach: 0.8, thick: 8_000.0 };
            }

            GalaxyKind::Interacting => {
                share = 0.75;
                let p_nuc = Vec3::new(-0.12 * r, 0.0, 0.0);
                let s_nuc = Vec3::new(0.42 * r, 0.03 * r, 0.1 * r);
                for k in 0..2 {
                    let mut c = spiral_arm(r, k as f32 * std::f32::consts::PI, 3.0, 0.5);
                    translate(&mut c, p_nuc);
                    curves.push(Curve::new(c, 0.4, 1.0, CapMode::Arm));
                }
                for k in 0..2 {
                    let mut c = spiral_arm(r, phase + k as f32 * std::f32::consts::PI, -3.0, 0.3);
                    translate(&mut c, s_nuc);
                    curves.push(Curve::new(c, 0.45, 0.6, CapMode::Arm));
                }
                // Pont de matière et queues de marée
                curves.push(Curve::new(
                    quad(p_nuc + Vec3::new(0.2 * r, 0.0, 0.0), Vec3::new(0.3 * r, 0.05 * r, 0.25 * r), s_nuc, 24),
                    0.06, 0.6, CapMode::Few));
                curves.push(Curve::new(
                    quad(p_nuc, Vec3::new(-0.8 * r, 0.0, -0.1 * r), Vec3::new(-0.95 * r, 0.0, 0.6 * r), 32),
                    0.08, 0.7, CapMode::Arm));
                curves.push(Curve::new(
                    quad(s_nuc, Vec3::new(0.9 * r, 0.0, 0.5 * r), Vec3::new(0.95 * r, 0.0, -0.6 * r), 32),
                    0.08, 0.5, CapMode::Arm));
                background = Background::Blobs(vec![
                    (p_nuc, 0.2 * r, 1.0),
                    (s_nuc, 0.12 * r, 0.6),
                ]);
            }

            GalaxyKind::MultiRing => {
                share = 0.85;
                let radii = [0.2_f32, 0.38, 0.58, 0.8];
                for (i, rr) in radii.iter().enumerate() {
                    let full = i % 2 == 0;
                    let arc = if full { TAU } else { rng.range(3.4, 5.4) };
                    let start = rng.f() * TAU;
                    let c = ring(&mut rng, r * rr, arc, start, 0.06);
                    curves.push(Curve::new(c, 0.04 + 0.01 * i as f32, rng.range(0.4, 1.0), CapMode::Loop));
                }
            }

            GalaxyKind::Fractal => {
                share = 0.9;
                let n = rng.range(4.0, 6.0) as usize;
                for a in 0..n {
                    let base = a as f32 * TAU / n as f32;
                    let trunk = spiral_arm(r, base, tw * 0.6, 0.8);
                    curves.push(Curve::new(trunk.clone(), 0.04, 1.0, CapMode::Arm));
                    fractal_branches(&mut rng, &mut curves, &trunk, 3, 0.5);
                }
            }

            GalaxyKind::Wavy => {
                let n = arms.min(4);
                for a in 0..n {
                    let base = a as f32 * TAU / n as f32;
                    let ph = rng.f() * TAU;
                    let freq = rng.range(10.0, 16.0);
                    curves.push(Curve::new(
                        pts_from(80, |p| {
                            let th = base + p * p * tw + 0.3 * (p * freq + ph).sin() * p;
                            polar(p * p * r * (1.0 + 0.08 * (p * freq * 0.7 + ph).cos()), th, 0.0)
                        }),
                        0.22, 1.0, CapMode::Arm));
                }
            }

            GalaxyKind::Filamentary => {
                share = 0.85;
                background = Background::Disk { reach: 0.3, thick: 12_000.0 };
                let n = 18;
                let mut ends = Vec::new();
                for i in 0..n {
                    let head = i as f32 * TAU / n as f32 + rng.range(-0.15, 0.15);
                    let len = r * rng.range(0.4, 1.0);
                    let start = polar(r * 0.03, head, 0.0);
                    let c = wander(&mut rng, start, head, len, 16, 0.9);
                    ends.push(c[10]);
                    let caps = if i % 4 == 0 { CapMode::Arm } else { CapMode::Few };
                    curves.push(Curve::new(c, 0.02, rng.range(0.5, 1.0), caps));
                }
                // Filaments de liaison entre filaments voisins
                for i in 0..n {
                    let (a, b) = (ends[i], ends[(i + 1) % n]);
                    if a.distance(b) < r * 0.6 {
                        curves.push(Curve::new(
                            quad(a, (a + b) * 0.5 + Vec3::new(0.0, 0.0, 0.03 * r), b, 8),
                            0.015, 0.25, CapMode::None));
                    }
                }
            }
        }

        // Variations propres à chaque galaxie extérieure (la principale reste telle quelle) :
        // largeur des bras, épaisseur, part d'étoiles sur la structure, disque plus ou moins ovale
        let (mut bg_stretch, mut bg_yaw) = (1.0, Quat::IDENTITY);
        if gal.seed != 0 {
            let width_mul = rng.range(0.6, 1.5);
            for c in &mut curves {
                c.width *= width_mul;
            }
            thickness *= rng.range(0.6, 2.0);
            if share > 0.0 {
                share = (share + rng.range(-0.15, 0.1)).clamp(0.1, 0.95);
            }
            bg_stretch = rng.range(0.6, 1.0);
            bg_yaw = Quat::from_rotation_y(rng.f() * TAU);
            for c in &mut curves {
                for p in &mut c.pts {
                    *p = bg_yaw * Vec3::new(p.x, p.y, p.z * bg_stretch);
                }
            }
        }

        let total_weight = curves.iter().map(|c| c.weight).sum();
        Self { curves, background, structure_share: share, thickness, radius: r, total_weight, bg_stretch, bg_yaw }
    }

    /// Étoile (ou nuage) posée sur la structure. `p_min` écarte le noyau.
    pub fn sample_structure(&self, rng: &mut Rng, p_min: f32) -> Vec3 {
        if self.total_weight <= 0.0 {
            return self.sample_background(rng);
        }
        let mut x = rng.f() * self.total_weight;
        let mut curve = &self.curves[0];
        for c in &self.curves {
            curve = c;
            if x < c.weight {
                break;
            }
            x -= c.weight;
        }
        let p = p_min + (1.0 - p_min) * rng.f();
        let (pos, tan) = curve.at(p);
        let side = Vec3::new(-tan.z, 0.0, tan.x).normalize_or_zero();
        let w = curve.width * self.radius * (0.2 + 0.8 * p);
        let off = side * (rng.f() - 0.5) * w + tan * (rng.f() - 0.5) * w * 0.4;
        let y = (rng.f() - 0.5) * self.thickness * (1.0 - 0.8 * p);
        pos + off + Vec3::Y * y
    }

    /// Étoile du fond diffus.
    pub fn sample_background(&self, rng: &mut Rng) -> Vec3 {
        let p = self.sample_background_raw(rng);
        self.bg_yaw * Vec3::new(p.x, p.y, p.z * self.bg_stretch)
    }

    fn sample_background_raw(&self, rng: &mut Rng) -> Vec3 {
        match &self.background {
            Background::Disk { reach, thick } => {
                let u = rng.f();
                let th = rng.f() * TAU;
                polar(u * u * self.radius * reach, th, (rng.f() - 0.5) * thick)
            }
            Background::Ellipsoid(ax) => {
                let z = rng.range(-1.0, 1.0);
                let th = rng.f() * TAU;
                let s = (1.0 - z * z).sqrt();
                let dir = Vec3::new(s * th.cos(), z, s * th.sin());
                dir * *ax * rng.f().powf(1.5)
            }
            Background::Blobs(blobs) => {
                let total: f32 = blobs.iter().map(|b| b.2).sum();
                let mut x = rng.f() * total;
                let mut pick = &blobs[0];
                for b in blobs {
                    pick = b;
                    if x < b.2 {
                        break;
                    }
                    x -= b.2;
                }
                let z = rng.range(-1.0, 1.0);
                let th = rng.f() * TAU;
                let s = (1.0 - z * z).sqrt();
                let dir = Vec3::new(s * th.cos(), z * 0.35, s * th.sin());
                pick.0 + dir * pick.1 * rng.f().powf(1.4)
            }
        }
    }
}

impl GalaxyConfig {
    pub fn shape(&self) -> Shape {
        Shape::build(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::default_galaxies;

    fn config(kind: GalaxyKind) -> GalaxyConfig {
        let mut g = default_galaxies(crate::settings::DEFAULT_WORLD_SEED).remove(1);
        g.kind = kind;
        g
    }

    #[test]
    fn every_kind_builds_finite_bounded_shapes() {
        for kind in GalaxyKind::ALL {
            for seed in 0..6u32 {
                let mut g = config(kind);
                g.seed = 1000 + seed * 77;
                let shape = g.shape();
                for c in &shape.curves {
                    assert!(c.pts.len() >= 2, "{kind:?}: courbe trop courte");
                    assert!(c.pts.iter().all(|p| p.is_finite()), "{kind:?}: point non fini");
                    assert!(c.length() > 0.0);
                }
                let mut rng = Rng::new(seed);
                for i in 0..400 {
                    let p = if (i as f32) < 400.0 * shape.structure_share {
                        shape.sample_structure(&mut rng, 0.0)
                    } else {
                        shape.sample_background(&mut rng)
                    };
                    assert!(p.is_finite(), "{kind:?}: étoile non finie");
                    assert!(p.length() < g.radius * 2.0, "{kind:?}: étoile hors galaxie ({})", p.length() / g.radius);
                }
            }
        }
    }

    #[test]
    fn capsule_budget_stays_reasonable() {
        for kind in GalaxyKind::ALL {
            let shape = config(kind).shape();
            let n: usize = shape.curves.iter().map(|c| match c.caps {
                CapMode::Arm => 17,
                CapMode::Loop => 12,
                CapMode::Few => 1,
                CapMode::None => 0,
            }).sum();
            assert!(n <= 160, "{kind:?}: {n} capsules");
            assert!(n >= 1, "{kind:?}: aucune capsule visible de loin");
        }
    }

    #[test]
    fn kinds_have_distinct_names_and_every_kind_is_used_equally() {
        let mut names: Vec<_> = GalaxyKind::ALL.iter().map(|k| k.name()).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), GalaxyKind::ALL.len());
        for world in [0u32, 5, 12345] {
            let mut count = [0usize; 20];
            for gi in 0..100 {
                count[GalaxyKind::for_index(gi, world) as usize] += 1;
            }
            assert!(count.iter().all(|&n| n == 5), "{count:?}");
            // Deux galaxies voisines ne sont jamais du même type
            assert!((0..99).all(|gi| GalaxyKind::for_index(gi, world) != GalaxyKind::for_index(gi + 1, world)));
        }
    }

    #[test]
    fn another_world_seed_gives_other_galaxies() {
        let a = default_galaxies(42);
        let b = default_galaxies(43);
        let differing = a.iter().zip(&b).skip(1).filter(|(x, y)| x.center.distance(y.center) > 1.0).count();
        assert!(differing > 90, "{differing}");
        assert!(a.iter().zip(&b).skip(1).any(|(x, y)| x.kind != y.kind));
    }

    #[test]
    fn galaxies_of_the_same_kind_differ_from_each_other() {
        let g = default_galaxies(crate::settings::DEFAULT_WORLD_SEED);
        let same: Vec<&GalaxyConfig> = g.iter().skip(1).filter(|x| x.kind == g[1].kind).collect();
        assert!(same.len() >= 2);
        let s0 = same[0].shape();
        let s1 = same[1].shape();
        let (mut r0, mut r1) = (Rng::new(1), Rng::new(1));
        let p0: Vec<Vec3> = (0..20).map(|_| s0.sample_structure(&mut r0, 0.0) / same[0].radius).collect();
        let p1: Vec<Vec3> = (0..20).map(|_| s1.sample_structure(&mut r1, 0.0) / same[1].radius).collect();
        assert!(p0.iter().zip(&p1).any(|(a, b)| a.distance(*b) > 0.05));
    }

    #[test]
    fn classic_spiral_keeps_the_historic_arm_formula() {
        let mut g = config(GalaxyKind::Spiral);
        g.num_arms = 3;
        // Graine 0 = galaxie principale : aucune variation aléatoire
        g.seed = 0;
        let shape = g.shape();
        assert_eq!(shape.curves.len(), 3);
        let (pos, _) = shape.curves[1].at(0.5);
        let expect = polar(0.25 * g.radius, TAU / 3.0 + 0.25 * g.twist, 0.0);
        assert!(pos.distance(expect) < g.radius * 0.01);
    }
}
