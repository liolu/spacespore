//! Étoiles (phase 1) : type spectral, masse → luminosité → température → couleur → rayon, âge,
//! durée de vie, activité magnétique, UV/X et vent stellaire.
//!
//! Le type est tiré avec des fréquences réalistes (beaucoup de naines rouges, presque pas d'O),
//! plus des étoiles rares : naines blanches, naines brunes, sous-géantes, géantes rouges. La
//! physique est en unités réelles (M☉, R☉, L☉, K) ; `render_radius` la convertit pour l'affichage.
//!
//! Taille affichée : une étoile G garde l'échelle d'avant (600 000 à 1 500 000) ; les autres types
//! ont les proportions réelles par rapport à elle (1 R☉ ≈ 1 050 000 à l'écran). Au-delà de la
//! plus grosse G, le rayon est compressé (une géante de 100 R☉ s'affiche vers 6 M) : les étoiles
//! voisines sont à ~5 M dans la galaxie principale, ~1,5 M dans les autres.
//!
//! L'ancien code (`astre/etoile/*`) n'apporte que les noms et l'ordre des classes : ses couleurs
//! et rayons étaient fixés à la main ; ici tout vient de la masse.

use serde::{Deserialize, Serialize};

use super::seeds::{Layer, LayerRng};
use super::units;

/// Type d'une étoile.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StarClass {
    O,
    B,
    A,
    F,
    #[default]
    G,
    K,
    M,
    WhiteDwarf,
    BrownDwarf,
    Subgiant,
    RedGiant,
}

impl StarClass {
    pub const ALL: [StarClass; 11] = [
        StarClass::O,
        StarClass::B,
        StarClass::A,
        StarClass::F,
        StarClass::G,
        StarClass::K,
        StarClass::M,
        StarClass::WhiteDwarf,
        StarClass::BrownDwarf,
        StarClass::Subgiant,
        StarClass::RedGiant,
    ];

    /// Part des systèmes de chaque type (somme = 1). Séquence principale d'après le voisinage
    /// solaire (~75 % de M, une O pour ~3 millions), rares un peu plus fréquentes que dans la
    /// réalité pour qu'on en croise.
    pub const WEIGHTS: [f64; 11] = [
        0.000_03, // O
        0.001_3,  // B
        0.006,    // A
        0.03,     // F
        0.075,    // G
        0.12,     // K
        0.66,     // M
        0.05,     // naine blanche
        0.04,     // naine brune
        0.012,    // sous-géante
        0.005_67, // géante rouge
    ];

    pub fn name(self) -> &'static str {
        match self {
            StarClass::O => "etoile bleue (O)",
            StarClass::B => "etoile bleu-blanc (B)",
            StarClass::A => "etoile blanche (A)",
            StarClass::F => "etoile jaune-blanc (F)",
            StarClass::G => "naine jaune (G)",
            StarClass::K => "naine orange (K)",
            StarClass::M => "naine rouge (M)",
            StarClass::WhiteDwarf => "naine blanche",
            StarClass::BrownDwarf => "naine brune",
            StarClass::Subgiant => "sous-geante",
            StarClass::RedGiant => "geante rouge",
        }
    }

    pub fn index(self) -> u8 {
        StarClass::ALL.iter().position(|&c| c == self).unwrap_or(4) as u8
    }

    /// Lettre spectrale d'un type de la séquence principale.
    pub fn letter(self) -> Option<char> {
        Some(match self {
            StarClass::O => 'O',
            StarClass::B => 'B',
            StarClass::A => 'A',
            StarClass::F => 'F',
            StarClass::G => 'G',
            StarClass::K => 'K',
            StarClass::M => 'M',
            _ => return None,
        })
    }

    pub fn main_sequence(self) -> Option<(f64, f64, f64)> {
        // (masse min, masse max, facteur d'activité magnétique)
        Some(match self {
            StarClass::O => (16.0, 60.0, 0.02),
            StarClass::B => (2.1, 16.0, 0.02),
            StarClass::A => (1.4, 2.1, 0.05),
            StarClass::F => (1.04, 1.4, 0.25),
            StarClass::G => (0.8, 1.04, 0.4),
            StarClass::K => (0.45, 0.8, 0.6),
            StarClass::M => (0.08, 0.45, 1.0),
            _ => return None,
        })
    }
}

/// Âge de l'Univers (Gyr) : aucune étoile n'est plus vieille.
const MAX_AGE: f64 = 13.0;

/// Physique complète d'une étoile (unités réelles).
#[derive(Clone, Debug, Serialize)]
pub struct StarPhysics {
    pub class: StarClass,
    /// Type spectral complet, ex. « G2 V », « M4 V », « K1 III », « DA3.1 », « T6 ».
    pub spectral_type: String,
    pub mass_sun: f64,
    pub radius_sun: f64,
    pub luminosity_sun: f64,
    pub temperature_k: f64,
    /// Couleur du corps noir (sRGB, la composante la plus forte vaut 1).
    pub color: [f32; 3],
    pub age_gyr: f64,
    /// Durée de vie totale dans cet état (Gyr).
    pub lifetime_gyr: f64,
    /// Activité magnétique, 0 (calme) à 1 (étoile à éruptions).
    pub activity: f64,
    /// UV reçus dans la zone habitable, Soleil/Terre = 1.
    pub uv_flux: f64,
    /// Rayons X (couronne, éruptions), Soleil = 1.
    pub xray_flux: f64,
    /// Perte de masse par le vent stellaire, Soleil = 1.
    pub stellar_wind: f64,
}

/// Luminosité d'une étoile de la séquence principale (relation masse-luminosité par morceaux).
fn ms_luminosity(m: f64) -> f64 {
    if m < 0.43 {
        0.23 * m.powf(2.3)
    } else if m < 2.0 {
        m.powi(4)
    } else if m < 55.0 {
        1.4 * m.powf(3.5)
    } else {
        32_000.0 * m
    }
}

/// Rayon d'une étoile de la séquence principale.
fn ms_radius(m: f64) -> f64 {
    if m < 1.0 { m.powf(0.8) } else { m.powf(0.57) }
}

/// Température effective (K) d'après la luminosité et le rayon (loi de Stefan-Boltzmann).
fn temperature_of(luminosity: f64, radius: f64) -> f64 {
    units::SUN_TEMPERATURE * (luminosity / (radius * radius)).powf(0.25)
}

/// Luminosité d'après le rayon et la température.
fn luminosity_of(radius: f64, temperature: f64) -> f64 {
    radius * radius * (temperature / units::SUN_TEMPERATURE).powi(4)
}

/// Rang d'une lettre spectrale, de la plus chaude (O = 0) à la plus froide.
fn spectral_rank(letter: char) -> usize {
    "OBAFGKMLTY".find(letter).unwrap_or(9)
}

/// Lettre spectrale et sous-classe (0 = plus chaude) d'après la température.
pub fn spectral_letter(t: f64) -> (char, u8) {
    const BANDS: [(char, f64, f64); 10] = [
        ('O', 30_000.0, 52_000.0),
        ('B', 10_000.0, 30_000.0),
        ('A', 7_500.0, 10_000.0),
        ('F', 6_000.0, 7_500.0),
        ('G', 5_200.0, 6_000.0),
        ('K', 3_700.0, 5_200.0),
        ('M', 2_400.0, 3_700.0),
        ('L', 1_300.0, 2_400.0),
        ('T', 500.0, 1_300.0),
        ('Y', 0.0, 500.0),
    ];
    for (letter, lo, hi) in BANDS {
        if t >= lo {
            let sub = (10.0 * (hi - t) / (hi - lo)).floor().clamp(0.0, 9.0);
            return (letter, sub as u8);
        }
    }
    ('Y', 9)
}

/// Couleur d'un corps noir à la température `t` (K) : spectre de Planck intégré avec les
/// fonctions colorimétriques CIE 1931 (approximation de Wyman et al. 2013), puis sRGB.
pub fn blackbody_color(t: f64) -> [f32; 3] {
    fn lobe(x: f64, mu: f64, s1: f64, s2: f64) -> f64 {
        let s = if x < mu { s1 } else { s2 };
        let d = (x - mu) / s;
        (-0.5 * d * d).exp()
    }
    let t = t.clamp(500.0, 60_000.0);
    let (mut x, mut y, mut z) = (0.0, 0.0, 0.0);
    let mut lambda = 380.0_f64;
    while lambda <= 780.0 {
        // Planck (constantes omises : on normalise ensuite) ; c2 = hc/k en nm·K
        let b = lambda.powi(-5) / ((1.438_8e7 / (lambda * t)).exp() - 1.0);
        x += b * (1.056 * lobe(lambda, 599.8, 37.9, 31.0) + 0.362 * lobe(lambda, 442.0, 16.0, 26.7)
            - 0.065 * lobe(lambda, 501.1, 20.4, 26.2));
        y += b * (0.821 * lobe(lambda, 568.8, 46.9, 40.5) + 0.286 * lobe(lambda, 530.9, 16.3, 31.1));
        z += b * (1.217 * lobe(lambda, 437.0, 11.8, 36.0) + 0.681 * lobe(lambda, 459.0, 26.0, 13.8));
        lambda += 5.0;
    }
    let lin = [
        (3.2406 * x - 1.5372 * y - 0.4986 * z).max(0.0),
        (-0.9689 * x + 1.8758 * y + 0.0415 * z).max(0.0),
        (0.0557 * x - 0.2040 * y + 1.0570 * z).max(0.0),
    ];
    let top = lin.iter().cloned().fold(f64::MIN_POSITIVE, f64::max);
    lin.map(|c| {
        let c = c / top;
        let s = if c <= 0.003_130_8 { 12.92 * c } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
        s.clamp(0.0, 1.0) as f32
    })
}

impl StarPhysics {
    /// Étoile d'un système. `seed` : graine du système ; `percentile` (0..1) : rang de l'étoile
    /// dans son type (masse), le même tirage qui fixait la taille des étoiles avant la phase 1 ;
    /// `forced` : impose le type (système de départ), sans changer les autres tirages.
    pub fn generate(seed: u64, percentile: f64, forced: Option<StarClass>) -> Self {
        let mut rng = LayerRng::new(seed, Layer::Star);
        let drawn = StarClass::ALL[rng.weighted(&StarClass::WEIGHTS)];
        let class = forced.unwrap_or(drawn);
        let u = percentile.clamp(0.0, 1.0);
        let age_draw = rng.unit();
        let extra = rng.unit();

        let (mass, radius, luminosity, temperature, age, lifetime, activity_factor, lum_class) = match class {
            StarClass::WhiteDwarf => {
                let mass = 0.5 + 0.6 * u * u;
                let radius = 0.012_7 * mass.powf(-1.0 / 3.0);
                // Refroidissement : 40 000 K à la naissance, ~5 000 K après 10 Gyr
                let cooling = 0.05 + age_draw * 9.95;
                let temperature = (40_000.0 * (cooling / 0.05).powf(-0.4)).max(4_000.0);
                let age = (cooling + 0.3 + extra * 2.0).min(MAX_AGE);
                (mass, radius, luminosity_of(radius, temperature), temperature, age, 1.0e4, 0.0, "")
            }
            StarClass::BrownDwarf => {
                let mass = 0.013 + 0.062 * u;
                let age = 0.1 + age_draw * 9.9;
                let radius = 0.08 + 0.04 * (1.0 / (1.0 + age)).sqrt() + 0.01 * extra;
                // Se refroidit toute sa vie : plus lourde et plus jeune = plus chaude
                let temperature = (2_600.0 * (mass / 0.075).powf(0.8) * (age / 0.1).powf(-0.3)).clamp(300.0, 2_800.0);
                (mass, radius, luminosity_of(radius, temperature), temperature, age, 1.0e4, 0.3, "")
            }
            StarClass::Subgiant => {
                let mass = 1.0 + 1.5 * u;
                let radius = 1.8 + 2.7 * extra;
                let temperature = 5_000.0 + 1_500.0 * (1.0 - extra) * (0.6 + 0.4 * u);
                let ms_life = 10.0 * mass.powf(-2.5);
                let age = (ms_life * (1.0 + 0.1 * age_draw)).min(MAX_AGE);
                (mass, radius, luminosity_of(radius, temperature), temperature, age, ms_life * 0.15, 0.15, " IV")
            }
            StarClass::RedGiant => {
                let mass = 0.8 + 2.2 * u * u;
                // Surtout 10 à 30 R☉, quelques géantes de la branche asymptotique jusqu'à 100 R☉
                let radius = 10.0 * 10.0_f64.powf(extra * extra);
                let temperature = 4_900.0 - 1_400.0 * extra;
                let ms_life = 10.0 * mass.powf(-2.5);
                let age = (ms_life * (1.1 + 0.1 * age_draw)).min(MAX_AGE);
                (mass, radius, luminosity_of(radius, temperature), temperature, age, ms_life * 0.1, 0.1, " III")
            }
            main => {
                let (lo, hi, factor) = main.main_sequence().expect("sequence principale");
                let mass = lo * (hi / lo).powf(u);
                let lifetime = 10.0 * mass.powf(-2.5);
                let age = lifetime.min(MAX_AGE) * (0.02 + 0.93 * age_draw);
                // Une étoile grossit et brille un peu plus en vieillissant (Soleil : +30 % en 4,6 Gyr)
                let f = (age / lifetime).min(1.0);
                let luminosity = ms_luminosity(mass) * (0.75 + 0.55 * f);
                let radius = ms_radius(mass) * (0.9 + 0.22 * f);
                (mass, radius, luminosity, temperature_of(luminosity, radius), age, lifetime, factor, " V")
            }
        };

        // Activité : forte chez les étoiles jeunes et froides (convection), quasi nulle sinon
        let activity = (activity_factor * (age + 0.1).powf(-0.5) * 0.83).clamp(0.0, 1.0);
        let sun_activity = 0.4 * (4.6_f64 + 0.1).powf(-0.5) * 0.83;
        let xray_flux = (activity / sun_activity).powi(2);
        // UV : rayonnement de la photosphère (étoiles chaudes) + couronne (étoiles actives)
        let uv_flux = (temperature / units::SUN_TEMPERATURE).powi(3) + 0.1 * (activity / sun_activity);
        let stellar_wind = match class {
            StarClass::O | StarClass::B => 1.0e6 * (luminosity / 1.0e4),
            StarClass::RedGiant => 1.0e3 * radius,
            StarClass::Subgiant => 10.0,
            StarClass::WhiteDwarf | StarClass::BrownDwarf => 0.0,
            _ => (activity / sun_activity) * radius * radius,
        };

        let spectral_type = match class {
            StarClass::WhiteDwarf => format!("DA{:.1}", 50_400.0 / temperature),
            _ => {
                let (mut letter, mut sub) = spectral_letter(temperature);
                // Séquence principale : la lettre est celle du type tiré (une G un peu froide reste G9)
                if let Some(own) = class.letter() {
                    if own != letter {
                        sub = if spectral_rank(own) < spectral_rank(letter) { 9 } else { 0 };
                        letter = own;
                    }
                }
                format!("{letter}{sub}{lum_class}")
            }
        };

        Self {
            class,
            spectral_type,
            mass_sun: mass,
            radius_sun: radius,
            luminosity_sun: luminosity,
            temperature_k: temperature,
            color: blackbody_color(temperature),
            age_gyr: age,
            lifetime_gyr: lifetime,
            activity,
            uv_flux,
            xray_flux,
            stellar_wind,
        }
    }

    /// Rayon affiché (unités du jeu). `g_radius` : rayon qu'aurait une G dans ce système (l'échelle
    /// d'avant la phase 1, 600 000 à 1 500 000) ; une G garde exactement ce rayon.
    pub fn render_radius(&self, g_radius: f64) -> f32 {
        let real = if self.class == StarClass::G { g_radius } else { self.radius_sun * RENDER_PER_SUN_RADIUS };
        // Compression au-delà de la plus grosse G, puis plafond : pas de débordement sur les voisins
        let shown = if real <= MAX_G_RADIUS { real } else { MAX_G_RADIUS * (real / MAX_G_RADIUS).cbrt() };
        // Arrondi à 10 unités : le résultat est identique sur toutes les machines (empreinte réseau)
        ((shown.min(MAX_RENDER_RADIUS) / 10.0).round() * 10.0).max(10.0) as f32
    }

    /// Intensité de la lumière dans le jeu (8 à 35 avant la phase 1 ; ~20 pour le Soleil). Les
    /// orbites ne suivent pas encore la luminosité (phase 2) : l'écart est donc compressé.
    pub fn light_intensity(&self) -> f32 {
        (20.0 * self.luminosity_sun.powf(0.25)).clamp(3.0, 60.0) as f32
    }

    /// Éruptions affichées : (nombre, hauteur, vitesse) d'après l'activité magnétique.
    pub fn flares(&self) -> (u32, f32, f32) {
        if self.class == StarClass::WhiteDwarf {
            return (0, 0.0, 1.0);
        }
        let a = self.activity as f32;
        ((1.0 + a * 11.0).round() as u32, 40.0 + 80.0 * a, 0.6 + 1.4 * a)
    }
}

/// Une étoile G moyenne (≈ 1 R☉) s'affiche au milieu de l'échelle d'avant (600 000 à 1 500 000).
pub const RENDER_PER_SUN_RADIUS: f64 = 1_050_000.0;
/// Plus grosse étoile G : au-delà, le rayon affiché est compressé.
pub const MAX_G_RADIUS: f64 = 1_500_000.0;
/// Aucun rayon affiché ne dépasse 6,5 M (une géante de 100 R☉ ≈ 6,2 M).
pub const MAX_RENDER_RADIUS: f64 = 6_500_000.0;

#[cfg(test)]
mod tests {
    use super::*;

    fn many() -> Vec<StarPhysics> {
        (0..40_000u64).map(|i| StarPhysics::generate(crate::planetgen::seeds::splitmix64(i), (i % 997) as f64 / 996.0, None)).collect()
    }

    #[test]
    fn frequencies_are_realistic() {
        assert!((StarClass::WEIGHTS.iter().sum::<f64>() - 1.0).abs() < 1e-9);
        let stars = many();
        let share = |c: StarClass| stars.iter().filter(|s| s.class == c).count() as f64 / stars.len() as f64;
        assert!(share(StarClass::M) > 0.6, "M {}", share(StarClass::M));
        assert!(share(StarClass::K) > share(StarClass::G) && share(StarClass::G) > share(StarClass::F));
        assert!(share(StarClass::F) > share(StarClass::A) && share(StarClass::A) > share(StarClass::B));
        assert!(share(StarClass::O) < 0.000_5);
        for rare in [StarClass::WhiteDwarf, StarClass::BrownDwarf, StarClass::Subgiant, StarClass::RedGiant] {
            assert!(share(rare) > 0.002 && share(rare) < 0.07, "{rare:?} {}", share(rare));
        }
    }

    #[test]
    fn physics_is_coherent() {
        for s in many() {
            assert!(s.mass_sun > 0.0 && s.radius_sun > 0.0 && s.luminosity_sun > 0.0, "{s:?}");
            assert!(s.age_gyr > 0.0 && s.age_gyr <= MAX_AGE + 1e-9, "{s:?}");
            assert!((0.0..=1.0).contains(&s.activity));
            // Stefan-Boltzmann tenu : L = R² (T/T☉)⁴
            let l = luminosity_of(s.radius_sun, s.temperature_k);
            assert!((l / s.luminosity_sun - 1.0).abs() < 1e-6, "{s:?}");
            let letter = s.spectral_type.chars().next().unwrap();
            match s.class {
                StarClass::M => assert!((0.08..=0.45).contains(&s.mass_sun) && s.temperature_k < 4_200.0, "{s:?}"),
                StarClass::G => assert!((4_800.0..6_600.0).contains(&s.temperature_k) && letter == 'G', "{s:?}"),
                StarClass::O => assert!(s.temperature_k > 25_000.0 && s.luminosity_sun > 1.0e4, "{s:?}"),
                StarClass::WhiteDwarf => {
                    assert!(s.radius_sun < 0.02 && s.mass_sun > 0.4 && s.spectral_type.starts_with("DA"), "{s:?}");
                    assert_eq!(s.flares().0, 0);
                }
                StarClass::BrownDwarf => {
                    assert!(s.mass_sun < 0.08 && s.temperature_k < 2_900.0 && s.luminosity_sun < 1e-3, "{s:?}");
                    assert!(matches!(letter, 'M' | 'L' | 'T' | 'Y'), "{s:?}");
                }
                StarClass::RedGiant => assert!(s.radius_sun >= 10.0 && s.temperature_k < 5_000.0 && s.spectral_type.ends_with("III"), "{s:?}"),
                StarClass::Subgiant => assert!(s.spectral_type.ends_with("IV"), "{s:?}"),
                _ => {}
            }
            if let Some(own) = s.class.letter() {
                assert!(s.age_gyr < s.lifetime_gyr, "{s:?}");
                assert_eq!(letter, own, "{s:?}");
            }
        }
    }

    #[test]
    fn a_forced_type_keeps_the_other_draws() {
        let free = StarPhysics::generate(1234, 0.4, None);
        let same = StarPhysics::generate(1234, 0.4, Some(free.class));
        assert_eq!(free.age_gyr.to_bits(), same.age_gyr.to_bits());
        let g = StarPhysics::generate(1234, 0.4, Some(StarClass::G));
        assert_eq!(g.class, StarClass::G);
        assert!(g.spectral_type.starts_with('G') && g.spectral_type.ends_with(" V"));
    }

    #[test]
    fn the_sun_is_the_sun() {
        // Masse 1, mi-vie : ~1 L☉, ~1 R☉, ~5 800 K, G2
        let l = ms_luminosity(1.0) * (0.75 + 0.55 * 0.46);
        let r = ms_radius(1.0) * (0.9 + 0.22 * 0.46);
        let t = temperature_of(l, r);
        assert!((l - 1.0).abs() < 0.02 && (r - 1.0).abs() < 0.02, "{l} {r}");
        assert!((t - 5_772.0).abs() < 60.0, "{t}");
        assert_eq!(spectral_letter(5_772.0), ('G', 2));
        assert_eq!(spectral_letter(3_100.0).0, 'M');
        assert_eq!(spectral_letter(40_000.0).0, 'O');
    }

    #[test]
    fn blackbody_colors_go_from_red_to_blue() {
        let red = blackbody_color(2_500.0);
        let sun = blackbody_color(5_772.0);
        let blue = blackbody_color(30_000.0);
        assert!(red[0] == 1.0 && red[2] < 0.3, "{red:?}");
        assert!(sun[0] >= sun[1] && sun[1] >= sun[2] && sun[2] > 0.75, "soleil presque blanc : {sun:?}");
        assert!(blue[2] == 1.0 && blue[0] < 0.8, "{blue:?}");
    }

    #[test]
    fn displayed_sizes_keep_g_and_cap_giants() {
        let mut biggest = 0.0_f32;
        for s in many() {
            let g = 600_000.0 + 900_000.0 * 0.5;
            let r = s.render_radius(g);
            biggest = biggest.max(r);
            assert!(r as f64 <= MAX_RENDER_RADIUS && r >= 10.0);
            match s.class {
                StarClass::G => assert_eq!(r as f64, g),
                StarClass::M => assert!(r < 600_000.0, "{s:?} {r}"),
                StarClass::WhiteDwarf => assert!(r < 25_000.0, "naine blanche de {r}"),
                StarClass::RedGiant => assert!(r > 2_500_000.0, "{r}"),
                _ => {}
            }
            assert_eq!(r % 10.0, 0.0);
        }
        assert!(biggest > 5_000_000.0, "{biggest}");
    }
}
