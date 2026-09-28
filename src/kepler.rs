use bevy::prelude::*;

/// Éléments orbitaux képlériens
#[derive(Clone, Copy, Debug)]
pub struct OrbitalElements {
    /// Demi-grand axe (taille de l'orbite)
    pub a: f32,
    /// Excentricité (0 = cercle, 0..1 = ellipse)
    pub e: f32,
    /// Inclinaison du plan orbital (radians)
    pub i: f32,
    /// Longitude du nœud ascendant (radians)
    pub omega_big: f32,
    /// Argument du périastre (radians)
    pub omega: f32,
    /// Anomalie moyenne initiale (radians)
    pub m0: f32,
}

impl Default for OrbitalElements {
    fn default() -> Self {
        Self { a: 1000.0, e: 0.0, i: 0.0, omega_big: 0.0, omega: 0.0, m0: 0.0 }
    }
}

impl OrbitalElements {
    pub fn circular(orbit_distance: f32, initial_phase: f32) -> Self {
        Self { a: orbit_distance, e: 0.0, i: 0.0, omega_big: 0.0, omega: 0.0, m0: initial_phase }
    }

    /// Mouvement moyen n = sqrt(mu / a^3)
    pub fn mean_motion(&self, mu: f32) -> f32 {
        if self.a <= 0.0 { return 0.0; }
        (mu / (self.a * self.a * self.a)).sqrt()
    }

    /// Position 3D au temps t (secondes écoulées)
    pub fn position(&self, t: f32, mu: f32) -> Vec3 {
        let n = self.mean_motion(mu);
        let mean_anomaly = (self.m0 + n * t) % std::f32::consts::TAU;

        // Newton-Raphson pour l'anomalie excentrique
        let ecc_anomaly = solve_kepler(mean_anomaly, self.e);

        // Position dans le plan orbital
        let x_orb = self.a * (ecc_anomaly.cos() - self.e);
        let y_orb = self.a * (1.0 - self.e * self.e).sqrt() * ecc_anomaly.sin();

        // Rotation 3D : Rz(Ω) × Rx(i) × Rz(ω)
        rotate_to_3d(x_orb, y_orb, self.omega, self.i, self.omega_big)
    }
}

/// Newton-Raphson : M = E - e*sin(E)  →  E
fn solve_kepler(m: f32, e: f32) -> f32 {
    let mut ecc = m;
    for _ in 0..8 {
        let f = ecc - e * ecc.sin() - m;
        let fp = 1.0 - e * ecc.cos();
        ecc -= f / fp;
    }
    ecc
}

/// Plan orbital → coordonnées monde (Y-up Bevy)
fn rotate_to_3d(x_orb: f32, y_orb: f32, omega: f32, inc: f32, omega_big: f32) -> Vec3 {
    let (cw, sw) = (omega.cos(), omega.sin());
    let (co, so) = (omega_big.cos(), omega_big.sin());
    let (ci, si) = (inc.cos(), inc.sin());

    let x = (co * cw - so * sw * ci) * x_orb + (-co * sw - so * cw * ci) * y_orb;
    let y = (so * cw + co * sw * ci) * x_orb + (-so * sw + co * cw * ci) * y_orb;
    let z = (sw * si) * x_orb + (cw * si) * y_orb;

    Vec3::new(x, z, y) // Y-up Bevy: swap y↔z
}

/// Paramètre gravitationnel approximatif mu pour une étoile centrale.
/// Calibré pour que orbit_distance=2000 donne une période de ~200s de jeu.
pub const DEFAULT_MU: f32 = 2_500_000_000.0;
