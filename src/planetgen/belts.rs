//! Ceintures d'astéroïdes d'un système (C1 de `ROADMAP-0.11.md`).
//!
//! Comme le Soleil : une ceinture rocheuse là où une géante a empêché une planète de se former
//! (juste avant la première géante, au-delà de la ligne des glaces), et une ceinture glacée externe
//! (type Kuiper) après la dernière planète. Chaque ceinture a une masse (M⊕), une largeur, une
//! épaisseur et une richesse (densité des champs) ; son mélange de types C / S / M (et de glaces)
//! change avec la distance à l'étoile, comme dans la ceinture principale du Soleil (S à
//! l'intérieur, C à l'extérieur).
//!
//! Les astéroïdes eux-mêmes ne sont pas stockés : `crate::asteroids` les tire par cellule
//! (règle 12), d'après la graine de la ceinture.

use serde::{Deserialize, Serialize};

use super::genome::SystemGenome;
use super::resources::Ore;
use super::seeds::{Layer, LayerRng};
use super::star::{StarClass, StarPhysics};
use super::system::display_distance;
use crate::settings::{AsteroidBeltConfig, PlanetConfig, SPACE_STRETCH};

/// Nature d'une ceinture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BeltKind {
    /// Rocheuse, entre les rocheuses et les géantes (ceinture principale du Soleil).
    Main,
    /// Glacée, après la dernière planète (ceinture de Kuiper).
    Kuiper,
}

impl BeltKind {
    pub fn name(self) -> &'static str {
        match self {
            BeltKind::Main => "ceinture d'asteroides",
            BeltKind::Kuiper => "ceinture glacee (type Kuiper)",
        }
    }
}

/// Type spectral d'un astéroïde : il dit sa composition (et ses minerais, phase 8).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AsteroidClass {
    /// Carboné : sombre, argiles hydratées, matière organique (75 % de la ceinture du Soleil).
    C,
    /// Silicaté : roche claire, un peu de fer-nickel.
    S,
    /// Métallique : fer-nickel (noyau d'un ancien petit monde), métaux précieux.
    M,
    /// Glacé : glace d'eau et de méthane, poussière sombre (ceinture externe).
    Ice,
}

impl AsteroidClass {
    pub const ALL: [AsteroidClass; 4] = [AsteroidClass::C, AsteroidClass::S, AsteroidClass::M, AsteroidClass::Ice];

    pub fn name(self) -> &'static str {
        match self {
            AsteroidClass::C => "type C (carbone)",
            AsteroidClass::S => "type S (silicates)",
            AsteroidClass::M => "type M (metal)",
            AsteroidClass::Ice => "glace (type Kuiper)",
        }
    }

    /// Masse volumique moyenne (g/cm³), porosité comprise (tas de gravats).
    pub fn density(self) -> f64 {
        match self {
            AsteroidClass::C => 1.4,
            AsteroidClass::S => 2.7,
            AsteroidClass::M => 5.0,
            AsteroidClass::Ice => 0.9,
        }
    }

    /// Couleur de la surface (sRGB).
    pub fn color(self) -> [f32; 3] {
        match self {
            AsteroidClass::C => [0.2, 0.19, 0.18],
            AsteroidClass::S => [0.56, 0.49, 0.41],
            AsteroidClass::M => [0.6, 0.59, 0.56],
            AsteroidClass::Ice => [0.74, 0.8, 0.86],
        }
    }

    /// Minerais (phase 8) : fraction de la masse de l'astéroïde. Le reste est de la roche (ou de
    /// la poussière) sans valeur.
    pub fn ores(self) -> &'static [(Ore, f64)] {
        match self {
            AsteroidClass::C => &[
                (Ore::WaterIce, 0.1),
                (Ore::Hydrocarbons, 0.03),
                (Ore::Iron, 0.05),
                (Ore::Nickel, 0.01),
                (Ore::Platinum, 1e-6),
            ],
            AsteroidClass::S => &[
                (Ore::Silicon, 0.2),
                (Ore::Iron, 0.1),
                (Ore::Nickel, 0.015),
                (Ore::Aluminium, 0.01),
                (Ore::Titanium, 0.001),
                (Ore::Platinum, 2e-6),
                (Ore::Gold, 1e-7),
            ],
            AsteroidClass::M => &[
                (Ore::Iron, 0.85),
                (Ore::Nickel, 0.1),
                (Ore::Copper, 2e-4),
                (Ore::Platinum, 2e-5),
                (Ore::Gold, 1e-6),
                (Ore::Xenium, 1e-6),
            ],
            AsteroidClass::Ice => &[
                (Ore::WaterIce, 0.5),
                (Ore::Hydrocarbons, 0.05),
                (Ore::Deuterium, 3e-4),
            ],
        }
    }
}

/// Une ceinture d'astéroïdes : un anneau dans le plan du système (y = 0), centré sur l'étoile.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Belt {
    pub kind: BeltKind,
    /// Graine (astéroïdes, champs denses).
    pub seed: u32,
    /// Bords en UA (physique réelle : températures, composition).
    pub au_inner: f32,
    pub au_outer: f32,
    /// Bords affichés (unités du jeu, depuis l'étoile, étirement compris), demi-épaisseur.
    pub inner: f32,
    pub outer: f32,
    pub half_thickness: f32,
    /// Masse totale (M⊕) : ceinture principale du Soleil 0,000 45, Kuiper ~0,02 à 0,1.
    pub mass_earth: f32,
    /// Richesse des champs (0,3 à 1,5) : nombre d'astéroïdes par cellule.
    pub density: f32,
    /// Ligne des glaces (UA) : au-delà, les astéroïdes gardent leur glace.
    pub snow_au: f32,
}

impl Belt {
    /// Rayon moyen affiché.
    pub fn mid(&self) -> f32 {
        0.5 * (self.inner + self.outer)
    }

    pub fn width(&self) -> f32 {
        self.outer - self.inner
    }

    /// Distance en UA à la fraction `f` (0 : bord intérieur, 1 : extérieur) de la largeur.
    pub fn au_at(&self, f: f32) -> f32 {
        self.au_inner * (self.au_outer / self.au_inner.max(1e-6)).powf(f.clamp(0.0, 1.0))
    }

    /// Mélange des types (C, S, M, glace) à la fraction `f` de la largeur : S surtout au bord
    /// intérieur, C au bord extérieur ; glaces au-delà de la ligne des glaces.
    pub fn class_mix(&self, f: f32) -> [f32; 4] {
        let f = f.clamp(0.0, 1.0);
        match self.kind {
            BeltKind::Kuiper => [0.17, 0.0, 0.03, 0.8],
            BeltKind::Main => {
                let au = self.au_at(f);
                // Glace : nulle avant la ligne des glaces, jusqu'à 30 % au-delà
                let ice = ((au / self.snow_au.max(1e-6) - 1.0) * 0.6).clamp(0.0, 0.3);
                let s = 0.55 - 0.4 * f;
                let m = 0.1 - 0.03 * f;
                let c = 1.0 - s - m;
                [c * (1.0 - ice), s * (1.0 - ice), m * (1.0 - ice), ice]
            }
        }
    }

    /// Type d'un astéroïde à la fraction `f` de la largeur, pour un tirage `u` dans [0, 1[.
    pub fn class_at(&self, f: f32, u: f32) -> AsteroidClass {
        let mix = self.class_mix(f);
        let mut roll = u * mix.iter().sum::<f32>();
        for (c, w) in AsteroidClass::ALL.iter().zip(mix) {
            if roll < w {
                return *c;
            }
            roll -= w;
        }
        AsteroidClass::C
    }

    /// Nombre d'astéroïdes de plus d'un kilomètre (ceinture du Soleil : ~1,5 million pour
    /// 0,000 45 M⊕).
    pub fn count_over_km(&self) -> f64 {
        3.3e9 * self.mass_earth as f64
    }

    /// Température d'équilibre (°C) d'un caillou sombre à `au` UA d'une étoile de luminosité `lum`.
    pub fn temperature_c(au: f64, lum: f64) -> f64 {
        278.0 * lum.max(1e-7).powf(0.25) / au.max(1e-4).sqrt() - 273.15
    }

    /// Ceinture faite à la main (éditeur) : distance et largeur données, le reste par défaut.
    pub fn from_config(cfg: &AsteroidBeltConfig, index: usize) -> Self {
        let inner = (cfg.distance - cfg.width * 0.5).max(1.0);
        let outer = (cfg.distance + cfg.width * 0.5).max(inner + 1.0);
        Self {
            kind: BeltKind::Main,
            seed: 0xA57E_0000 ^ index as u32,
            au_inner: 2.1,
            au_outer: 3.3,
            inner,
            outer,
            half_thickness: cfg.width * 0.15,
            mass_earth: 4.5e-4,
            density: (cfg.count as f32 / 300.0).clamp(0.3, 1.5),
            snow_au: 2.7,
        }
    }
}

/// Arrondi au multiple de `step` (valeurs affichées, identiques sur toutes les machines).
fn q(x: f64, step: f64) -> f32 {
    ((x / step).round() * step) as f32
}

/// Arrondi à 4 chiffres significatifs (distances en UA autour des naines brunes : 0,000 1 UA).
fn qs(x: f64) -> f32 {
    if x <= 0.0 {
        return 0.0;
    }
    q(x, 10f64.powi(x.log10().floor() as i32 - 3))
}

/// Ce qu'occupe une planète autour de son orbite : rayon, anneaux, lunes.
fn reach(p: &PlanetConfig) -> f64 {
    let ring = p.ring.map_or(0.0, |r| r.outer as f64);
    p.moons.iter().map(|m| (m.orbit_distance + m.radius) as f64).fold((p.radius as f64).max(ring), f64::max)
}

/// Ceintures du système d'étoile `star` (échelle G `scale`) dont les planètes sont `planets`.
pub fn generate(genome: SystemGenome, star: &StarPhysics, scale: f32, star_radius: f32, planets: &[PlanetConfig]) -> Vec<Belt> {
    // La planète errante (C2) n'est pas sur une orbite : elle ne compte pas
    let planets = &planets[..planets.iter().position(|p| p.rogue).unwrap_or(planets.len())];
    let mut rng = LayerRng::new(genome.seed as u64, Layer::Belts);
    let scale = scale as f64;
    let stretch = SPACE_STRETCH as f64;
    let lum = star.luminosity_sun.max(1e-7);
    let hz = lum.sqrt();
    let snow = 2.7 * hz;
    let remnant = matches!(star.class, StarClass::WhiteDwarf | StarClass::BrownDwarf);
    let margin = 0.08 * scale * stretch;
    // Bords affichés de chaque planète (orbite excentrique, lunes et anneaux compris)
    let extent = |p: &PlanetConfig| -> (f64, f64) {
        let a = p.orbit_distance as f64;
        let e = p.eccentricity as f64;
        let r = reach(p);
        (a * (1.0 - e) - r, a * (1.0 + e) + r)
    };
    let to_display = |au: f64| display_distance(au, hz) * scale * stretch;
    let mut belts = Vec::new();

    // ── Ceinture principale : avant la première géante au-delà de la ligne des glaces ──
    let roll = rng.unit();
    let thick = rng.range(0.05, 0.1);
    let mass = 10f64.powf(rng.range(-4.5, -2.5));
    let seed = rng.next_u64() as u32;
    let giant = planets.iter().position(|p| p.kind.gaseous() && !p.hot && p.semi_major_au as f64 > snow * 0.8);
    let chance = if remnant { 0.3 } else if giant.is_some() { 0.75 } else { 0.25 };
    if roll < chance {
        // Bords réels (UA) : entre la planète d'avant et la zone que la géante vide, ou autour de
        // la ligne des glaces sans géante
        let (au_lo, au_hi, prev, next) = match giant {
            Some(g) => {
                let ga = planets[g].semi_major_au as f64;
                let prev_au = if g > 0 { planets[g - 1].semi_major_au as f64 * 1.12 } else { 0.0 };
                (prev_au.max(ga * 0.3), ga * 0.78, g.checked_sub(1), Some(g))
            }
            None => {
                let (lo, hi) = (snow * 0.7, snow * 1.2);
                let next = planets.iter().position(|p| p.semi_major_au as f64 > lo);
                (lo, hi, next.map_or(planets.len().checked_sub(1), |n| n.checked_sub(1)), next)
            }
        };
        // Bords affichés : la même échelle que les planètes, sans jamais toucher leurs orbites
        let lo_limit = prev.map_or(star_radius as f64 * 2.0, |i| extent(&planets[i]).1) + margin;
        let hi_limit = next.map_or(f64::INFINITY, |i| extent(&planets[i]).0) - margin;
        let inner = to_display(au_lo).max(lo_limit);
        let outer = to_display(au_hi).min(hi_limit);
        if au_hi > au_lo * 1.12 && outer - inner > 0.03 * scale * stretch {
            let mid = 0.5 * (inner + outer);
            belts.push(Belt {
                kind: BeltKind::Main,
                seed,
                au_inner: qs(au_lo),
                au_outer: qs(au_hi),
                inner: q(inner, 10.0),
                outer: q(outer, 10.0),
                half_thickness: q(mid * thick, 10.0),
                mass_earth: mass as f32,
                density: q(((mass.log10() + 5.0) / 2.0).clamp(0.3, 1.5), 1e-3),
                snow_au: qs(snow),
            });
        }
    }

    // ── Ceinture glacée externe (type Kuiper) : après la dernière planète ──
    let roll = rng.unit();
    let thick = rng.range(0.1, 0.2);
    let mass = 10f64.powf(rng.range(-2.0, -0.7));
    let seed = rng.next_u64() as u32;
    let start = rng.range(1.25, 1.6);
    let span = rng.range(1.4, 1.8);
    let chance = if remnant { 0.3 } else { 0.65 };
    if let (true, Some(last)) = (roll < chance, planets.last()) {
        let au_lo = (last.semi_major_au as f64).max(snow) * start;
        let au_hi = au_lo * span;
        let inner = to_display(au_lo).max(extent(last).1 + margin);
        // Même largeur affichée qu'en échelle logarithmique, partant du bord réel
        let outer = inner + (to_display(au_hi) - to_display(au_lo)).max(0.3 * scale * stretch);
        let mid = 0.5 * (inner + outer);
        belts.push(Belt {
            kind: BeltKind::Kuiper,
            seed,
            au_inner: qs(au_lo),
            au_outer: qs(au_hi),
            inner: q(inner, 10.0),
            outer: q(outer, 10.0),
            half_thickness: q(mid * thick, 10.0),
            mass_earth: mass as f32,
            // Énorme volume : moins d'astéroïdes par cellule à masse égale
            density: q(((mass.log10() + 3.0) / 2.5).clamp(0.3, 1.2), 1e-3),
            snow_au: qs(snow),
        });
    }
    belts
}

/// Troyens (C2) : essaim d'astéroïdes au point de Lagrange L4 (60° devant) ou L5 (60° derrière)
/// d'une géante, sur son orbite.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Swarm {
    /// Indice de la géante dans `planets()`.
    pub planet: u8,
    /// L4 (devant la planète) ou L5 (derrière).
    pub leading: bool,
    pub seed: u32,
    pub mass_earth: f32,
    /// Richesse (comme une ceinture).
    pub density: f32,
    /// Étendue le long de l'orbite (fraction du rayon de l'orbite) ; en largeur et en hauteur, un
    /// cinquième.
    pub spread: f32,
}

impl Swarm {
    pub fn name(&self) -> &'static str {
        if self.leading { "Troyens (L4)" } else { "Troyens (L5)" }
    }

    /// Les Troyens sont des C et des D sombres (comme ceux de Jupiter), un peu de S.
    pub fn class_at(&self, u: f32) -> AsteroidClass {
        if u < 0.7 { AsteroidClass::C } else if u < 0.92 { AsteroidClass::S } else { AsteroidClass::M }
    }
}

/// Troyens des géantes (gazeuses surtout, de glace parfois), jamais d'une planète errante.
pub fn trojans(genome: SystemGenome, planets: &[PlanetConfig]) -> Vec<Swarm> {
    use super::system::PlanetKind;
    let mut rng = LayerRng::new(genome.seed as u64 ^ 0x5452_4F4A, Layer::Belts);
    let mut out = Vec::new();
    for (i, p) in planets.iter().enumerate() {
        let chance = match p.kind {
            PlanetKind::GasGiant => 0.85,
            PlanetKind::IceGiant => 0.5,
            _ => 0.0,
        };
        // Tirages toujours faits : les essaims ne dépendent pas des planètes d'avant
        let (roll, mass, spread, seed) = (rng.unit(), 10f64.powf(rng.range(-5.5, -4.0)), rng.range(0.08, 0.14), rng.next_u64());
        if p.rogue || p.hot || roll >= chance {
            continue;
        }
        for leading in [true, false] {
            // L4 un peu plus riche que L5 (comme chez Jupiter)
            let share = if leading { 0.6 } else { 0.4 };
            out.push(Swarm {
                planet: i as u8,
                leading,
                seed: (seed >> if leading { 0 } else { 32 }) as u32,
                mass_earth: (mass * share) as f32,
                density: q(((mass.log10() + 6.0) / 1.5).clamp(0.4, 1.3), 1e-3),
                spread: q(spread, 1e-4),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{default_galaxies, default_systems};

    #[test]
    fn belts_sit_between_planets_and_never_touch_them() {
        let systems = default_systems(&default_galaxies(42), 42);
        let (mut main, mut kuiper, mut n) = (0, 0, 0);
        for sys in systems.dense().iter().take(2000) {
            let all = sys.planets_uncached();
            let planets: Vec<&PlanetConfig> = all.iter().filter(|p| !p.rogue).collect();
            let belts = sys.belts();
            n += 1;
            for b in &belts {
                assert!(b.inner < b.outer && b.half_thickness > 0.0 && b.mass_earth > 0.0, "{b:?}");
                assert!(b.au_inner < b.au_outer);
                for p in &planets {
                    let (a, e, r) = (p.orbit_distance, p.eccentricity, reach(p) as f32);
                    let (lo, hi) = (a * (1.0 - e) - r, a * (1.0 + e) + r);
                    assert!(hi < b.inner || lo > b.outer, "{} : planete {lo}..{hi} dans la ceinture {}..{}", sys.name, b.inner, b.outer);
                }
                match b.kind {
                    BeltKind::Main => {
                        main += 1;
                        // La ceinture rocheuse est avant la première géante froide
                        if let Some(g) = planets.iter().find(|p| p.kind.gaseous() && !p.hot && p.semi_major_au > b.snow_au * 0.8) {
                            assert!(b.outer < g.orbit_distance, "{}", sys.name);
                        }
                    }
                    BeltKind::Kuiper => {
                        kuiper += 1;
                        assert!(planets.iter().all(|p| p.orbit_distance < b.inner), "{}", sys.name);
                    }
                }
            }
            // Au plus une de chaque
            assert!(belts.iter().filter(|b| b.kind == BeltKind::Main).count() <= 1);
            assert!(belts.iter().filter(|b| b.kind == BeltKind::Kuiper).count() <= 1);
        }
        let (main, kuiper) = (main as f64 / n as f64, kuiper as f64 / n as f64);
        assert!((0.2..0.8).contains(&main), "ceintures principales {main}");
        assert!((0.4..0.75).contains(&kuiper), "ceintures glacees {kuiper}");
    }

    #[test]
    fn belts_are_reproducible() {
        let a = default_systems(&default_galaxies(7), 7);
        let b = default_systems(&default_galaxies(7), 7);
        for (x, y) in a.dense().iter().zip(b.dense()).take(300) {
            assert_eq!(x.belts(), y.belts());
        }
    }

    #[test]
    fn main_belt_mix_goes_from_stony_to_carbon() {
        let b = Belt {
            kind: BeltKind::Main,
            seed: 1,
            au_inner: 2.1,
            au_outer: 3.3,
            inner: 1.0,
            outer: 2.0,
            half_thickness: 0.1,
            mass_earth: 4.5e-4,
            density: 1.0,
            snow_au: 2.7,
        };
        let (inner, outer) = (b.class_mix(0.0), b.class_mix(1.0));
        assert!(inner[1] > outer[1] && inner[0] < outer[0], "{inner:?} {outer:?}");
        assert_eq!(inner[3], 0.0, "pas de glace avant la ligne des glaces");
        assert!(outer[3] > 0.0);
        // Soleil : ~75 % de C au total, ~17 % de S, ~8 % de M
        let mut count = [0usize; 4];
        for i in 0..10_000 {
            let c = b.class_at((i % 100) as f32 / 100.0, ((i * 7919) % 10_000) as f32 / 10_000.0);
            count[c as usize] += 1;
        }
        assert!(count[0] > count[1] && count[1] > count[2] && count[2] > 300, "{count:?}");
        // ~1,5 million d'astéroïdes de plus d'un km
        assert!((1.0e6..2.0e6).contains(&b.count_over_km()));
        // Ceinture du Soleil : environ -100 °C
        assert!((-130.0..-70.0).contains(&Belt::temperature_c(2.7, 1.0)));
    }
}
