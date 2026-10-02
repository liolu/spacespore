//! Traits et anomalies (phase 9).
//!
//! Chaque astre tire une rareté : 98 % ordinaire, 1,5 % peu commun, 0,4 % rare, 0,1 % légendaire
//! (anomalies, presque toutes fictives). Le trait est choisi parmi ceux que l'astre permet (pas de
//! lacs de lave sans volcans). S'y ajoutent les traits qui découlent de ses données (anneaux,
//! aurores, rotation synchrone ou rétrograde, monde-océan...), qui ne sont pas des tirages.

use super::profile::{Realism, Trait};
use super::seeds::LayerRng;

/// Raretés des tirages (somme = 1).
pub const TIERS: [(&str, f64); 4] = [("ordinaire", 0.98), ("peu commun", 0.015), ("rare", 0.004), ("legendaire", 0.001)];

/// Ce que l'astre permet.
pub struct TraitContext {
    pub gaseous: bool,
    pub atmosphere: bool,
    pub volcanism: f32,
    pub liquid_water: bool,
    pub icy: bool,
    pub magnetic: f32,
    pub old_surface: bool,
    pub tilt: f32,
    pub locked: bool,
    pub ocean_fraction: f32,
    pub lava: bool,
    pub ring: bool,
    pub aurora: bool,
}

struct Candidate {
    name: &'static str,
    realism: Realism,
    allowed: fn(&TraitContext) -> bool,
}

const fn c(name: &'static str, realism: Realism, allowed: fn(&TraitContext) -> bool) -> Candidate {
    Candidate { name, realism, allowed }
}

const R: Realism = Realism::Realistic;
const S: Realism = Realism::Speculative;
const F: Realism = Realism::Fictional;

const UNCOMMON: [Candidate; 6] = [
    c("geysers geants", R, |x| x.icy || x.volcanism > 0.2),
    c("super-tempete permanente", R, |x| x.atmosphere || x.gaseous),
    c("lacs de lave actifs", R, |x| !x.gaseous && x.volcanism > 0.3),
    c("cratere d'impact geant", R, |x| !x.gaseous && x.old_surface),
    c("arches et cheminees de fees", R, |x| !x.gaseous && x.atmosphere),
    c("champ magnetique inverse", R, |x| x.magnetic > 0.2),
];

const RARE: [Candidate; 5] = [
    c("pluie de diamants", S, |x| x.gaseous),
    c("ocean bioluminescent", S, |x| x.liquid_water && x.ocean_fraction > 0.3),
    c("foret petrifiee", S, |x| !x.gaseous && x.atmosphere),
    c("glace superionique", S, |x| x.icy || x.gaseous),
    c("noyau de fer expose", S, |x| !x.gaseous && !x.atmosphere),
];

const LEGENDARY: [Candidate; 5] = [
    c("ruines d'une civilisation disparue", F, |x| !x.gaseous),
    c("monolithe parfait", F, |x| !x.gaseous),
    c("cristaux chantants", F, |x| !x.gaseous),
    c("anomalie gravitationnelle", F, |_| true),
    c("echo temporel", F, |_| true),
];

/// Rareté tirée (indice dans `TIERS`) et le trait correspondant, s'il y en a un.
pub fn roll(ctx: &TraitContext, rng: &mut LayerRng) -> (usize, Option<Trait>) {
    let tier = rng.weighted(&TIERS.map(|(_, w)| w));
    let list: &[Candidate] = match tier {
        1 => &UNCOMMON,
        2 => &RARE,
        3 => &LEGENDARY,
        _ => return (0, None),
    };
    let allowed: Vec<&Candidate> = list.iter().filter(|c| (c.allowed)(ctx)).collect();
    let pick = if allowed.is_empty() { None } else { Some(allowed[(rng.unit() * allowed.len() as f64) as usize % allowed.len()]) };
    (tier, pick.map(|c| Trait { name: c.name.to_string(), realism: c.realism, rarity: TIERS[tier].1 }))
}

/// Traits de l'astre : ceux qui découlent de ses données, puis le tirage.
pub fn traits_of(ctx: &TraitContext, rng: &mut LayerRng) -> Vec<Trait> {
    let mut out = Vec::new();
    let mut natural = |name: &str| out.push(Trait { name: name.to_string(), realism: R, rarity: 0.98 });
    if ctx.ring {
        natural("anneaux");
    }
    if ctx.aurora {
        natural("aurores polaires");
    }
    if ctx.locked {
        natural("rotation synchrone : une face toujours au jour");
    }
    if ctx.tilt > 90.0 {
        natural("rotation retrograde");
    }
    if ctx.ocean_fraction > 0.9 {
        natural("monde-ocean");
    }
    if ctx.lava {
        natural("ocean de magma");
    }
    let (_, rolled) = roll(ctx, rng);
    out.extend(rolled);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planetgen::seeds::Layer;

    fn ctx() -> TraitContext {
        TraitContext {
            gaseous: false,
            atmosphere: true,
            volcanism: 0.5,
            liquid_water: true,
            icy: false,
            magnetic: 1.0,
            old_surface: true,
            tilt: 23.0,
            locked: false,
            ocean_fraction: 0.6,
            lava: false,
            ring: false,
            aurora: true,
        }
    }

    #[test]
    fn rarities_follow_98_1_5_0_4_0_1() {
        let n = 400_000;
        let mut counts = [0usize; 4];
        for i in 0..n {
            let mut rng = LayerRng::new(i, Layer::Traits);
            counts[roll(&ctx(), &mut rng).0] += 1;
        }
        let share = |k: usize| counts[k] as f64 / n as f64;
        assert!((share(0) - 0.98).abs() < 0.002, "{counts:?}");
        assert!((share(1) - 0.015).abs() < 0.001, "{counts:?}");
        assert!((share(2) - 0.004).abs() < 0.0005, "{counts:?}");
        assert!((share(3) - 0.001).abs() < 0.0003, "{counts:?}");
    }

    #[test]
    fn traits_respect_the_world() {
        let gas = TraitContext { gaseous: true, atmosphere: false, volcanism: 0.0, liquid_water: false, ocean_fraction: 0.0, ..ctx() };
        for i in 0..200_000u64 {
            let mut rng = LayerRng::new(i, Layer::Traits);
            if let (_, Some(t)) = roll(&gas, &mut rng) {
                assert!(!matches!(t.name.as_str(), "lacs de lave actifs" | "ruines d'une civilisation disparue" | "foret petrifiee"), "{}", t.name);
            }
        }
        let mut rng = LayerRng::new(1, Layer::Traits);
        let list = traits_of(&TraitContext { ring: true, locked: true, ..ctx() }, &mut rng);
        assert!(list.iter().any(|t| t.name == "anneaux") && list.iter().any(|t| t.name.starts_with("rotation synchrone")));
    }
}
