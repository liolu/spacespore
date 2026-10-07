//! Approche planétaire (0.13 bloc P) : de l'orbite au sol en un seul vol continu (`surface.rs`), et
//! tout ce qui se voit et s'entend sur le chemin : rentrée atmosphérique, nuages qui s'écartent,
//! poussière à l'atterrissage, bang supersonique, traînées, indicateurs d'approche, météo à
//! bord, son. Les effets dépendent de la VITESSE et de la densité de l'air (Q11), jamais de
//! l'altitude seule.
//!
//! Ce fichier : les grandeurs physiques partagées (densité de l'air, vitesse du son, chaleur de
//! rentrée) et l'état de vol `FlightInfo` rempli par `surface.rs` à chaque image.

use bevy::prelude::*;

use crate::terrain::BodyParams;

/// Un voxel vaut environ un mètre (marche à ~5 voxels/s) : la vitesse du son au sol de la Terre
/// est donc d'environ 340 voxels/s.
pub const SOUND_EARTH: f32 = 340.0;

/// Pression dynamique (densité x vitesse², voxels) qui divise par deux la vitesse maximale :
/// l'air freine (P3, « la vitesse maxi baisse avec la densité »).
pub const DRAG_Q0: f32 = 2.0e6;

/// État du vol, rempli à chaque image par `surface.rs` (zéro hors vol).
#[derive(Clone, Copy, Debug, Default)]
pub struct FlightInfo {
    pub active: bool,
    /// Altitude au-dessus du relief (unités) et taille d'un voxel (unités).
    pub alt: f32,
    pub voxel: f32,
    /// Vitesse (voxels/s) : totale, et verticale (+ = monte).
    pub speed_vox: f32,
    pub vert_vox: f32,
    /// Densité de l'air relative à celle du sol de la Terre (1 bar, 15 C = 1).
    pub density: f32,
    /// Vitesse du son (voxels/s) et nombre de Mach.
    pub sound: f32,
    pub mach: f32,
    /// Chaleur de rentrée (0..1) : densité x vitesse au cube, d'après `heat`.
    pub heat: f32,
    pub boost: bool,
    /// Verticale du lieu (repère fixe de l'astre) et cap.
    pub up: Vec3,
    pub heading: Vec3,
    /// Position (repère fixe de l'astre), rayon du sol dessous, rayon de l'astre.
    pub pos: Vec3,
    pub ground_r: f32,
    pub radius: f32,
    /// Taille affichée du vaisseau (échelle) et sa longueur (unités).
    pub scale: f32,
    pub length: f32,
    /// Secondes avant de toucher le sol à la vitesse verticale actuelle (infini si on monte).
    pub time_to_ground: f32,
}

/// Hauteur d'échelle de l'atmosphère (unités) : la densité baisse d'un facteur e tous les `h`.
pub fn scale_height(p: &BodyParams) -> f32 {
    crate::surface::atmosphere_depth(p) / 5.0
}

/// Densité de l'air à `alt` unités au-dessus du sol, relative à la Terre au niveau de la mer.
pub fn air_density(p: &BodyParams, alt: f32) -> f32 {
    if !p.atmosphere || p.pressure < 0.005 {
        return 0.0;
    }
    // Plus froid = plus dense à pression égale
    let t = (p.climate.mean_c + 273.15).clamp(40.0, 1500.0);
    p.pressure * (288.0 / t) * (-alt.max(0.0) / scale_height(p).max(1.0)).exp()
}

/// Vitesse du son (voxels/s) : proportionnelle à la racine de la température.
pub fn sound_speed(p: &BodyParams) -> f32 {
    let t = (p.climate.mean_c + 273.15).clamp(40.0, 1500.0);
    SOUND_EARTH * (t / 288.0).sqrt()
}

/// Chaleur de rentrée (0..1) : densité x vitesse^3 (flux de chaleur), 0 en dessous de Mach 2 et
/// de la moitié de la densité de la haute atmosphère. Lent = rien, rapide = flammes (Q11).
pub fn heat(density: f32, speed_vox: f32, sound: f32) -> f32 {
    let mach = speed_vox / sound.max(1.0);
    let flux = density * mach.powi(3);
    ((flux - 1.5) / 14.0).clamp(0.0, 1.0)
}

/// Couleur des flammes de plasma par défaut (azote : orange).
pub const PLASMA_DEFAULT: [f32; 3] = [1.0, 0.55, 0.2];

/// Couleur du plasma de rentrée d'après le gaz dominant de l'air (0.13 P3) : azote orange, CO2 rose,
/// méthane vert-bleu, hydrogène / hélium violet, oxygène jaune-orangé, soufre jaune-vert.
pub fn plasma_color(gases: &[(String, f32)]) -> [f32; 3] {
    let share = |f: &str| gases.iter().filter(|(g, _)| g == f).map(|(_, x)| *x).sum::<f32>();
    let table: [(&str, [f32; 3]); 6] = [
        ("CH4", [0.25, 0.9, 0.8]),
        ("CO2", [1.0, 0.45, 0.7]),
        ("H2", [0.75, 0.6, 1.0]),
        ("He", [0.75, 0.6, 1.0]),
        ("SO2", [0.85, 0.9, 0.25]),
        ("O2", [1.0, 0.7, 0.35]),
    ];
    // Un gaz rare mais coloré compte plus que l'azote : on pondère les fractions
    let mut best = (share("N2") * 0.35, PLASMA_DEFAULT);
    for (g, c) in table {
        let w = share(g) * if g == "CH4" || g == "SO2" { 8.0 } else if g == "O2" { 1.5 } else { 1.0 };
        if w > best.0 {
            best = (w, c);
        }
    }
    best.1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::PlanetConfig;

    fn earth() -> BodyParams {
        let mut p = BodyParams::planet(&PlanetConfig::default());
        p.atmosphere = true;
        p.pressure = 1.0;
        p.radius = 10_000.0;
        p.climate = crate::planetgen::climate::Climate::from_mean(15.0, false);
        p
    }

    #[test]
    fn the_air_thins_with_altitude_and_vanishes_without_an_atmosphere() {
        let p = earth();
        let (a, b, c) = (air_density(&p, 0.0), air_density(&p, 1000.0), air_density(&p, 5000.0));
        assert!(a > b && b > c && c > 0.0, "{a} {b} {c}");
        assert!((a - 1.0).abs() < 0.05, "la Terre : {a}");
        assert_eq!(air_density(&BodyParams { atmosphere: false, ..p }, 0.0), 0.0);
        assert_eq!(air_density(&BodyParams { pressure: 0.001, ..p }, 0.0), 0.0);
    }

    #[test]
    fn the_speed_of_sound_follows_the_temperature() {
        let cold = BodyParams { climate: crate::planetgen::climate::Climate::from_mean(-80.0, false), ..earth() };
        let hot = BodyParams { climate: crate::planetgen::climate::Climate::from_mean(300.0, false), ..earth() };
        assert!((sound_speed(&earth()) - SOUND_EARTH).abs() < 6.0);
        assert!(sound_speed(&cold) < sound_speed(&earth()) && sound_speed(&earth()) < sound_speed(&hot));
    }

    /// Q11 : lent = rien, rapide dans l'air dense = flammes, rapide dans le vide = rien.
    #[test]
    fn heat_depends_on_speed_and_air() {
        let c = SOUND_EARTH;
        assert_eq!(heat(1.0, 0.5 * c, c), 0.0, "lent");
        assert_eq!(heat(1.0, 1.0 * c, c), 0.0, "Mach 1 au sol");
        assert!(heat(0.5, 6.0 * c, c) > 0.9, "Mach 6 dans un air dense");
        assert_eq!(heat(0.0, 20.0 * c, c), 0.0, "vide");
        // Plus vite = plus chaud
        assert!(heat(0.2, 5.0 * c, c) > heat(0.2, 3.0 * c, c));
    }

    #[test]
    fn plasma_follows_the_air() {
        let g = |v: &[(&str, f32)]| v.iter().map(|(a, b)| (a.to_string(), *b)).collect::<Vec<_>>();
        assert_eq!(plasma_color(&g(&[("N2", 0.78), ("O2", 0.01)])), PLASMA_DEFAULT);
        let co2 = plasma_color(&g(&[("CO2", 0.96), ("N2", 0.03)]));
        assert!(co2[0] > 0.9 && co2[2] > 0.6 && co2[1] < 0.5, "rose : {co2:?}");
        let ch4 = plasma_color(&g(&[("N2", 0.9), ("CH4", 0.06)]));
        assert!(ch4[1] > 0.8 && ch4[0] < 0.4, "vert-bleu : {ch4:?}");
    }
}
