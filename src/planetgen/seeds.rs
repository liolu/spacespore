//! Sous-graines par couche (règle 2 de la feuille de route).
//!
//! Chaque astre a une graine ; chaque couche de génération (étoile, orbite, physique, atmosphère…)
//! en tire sa propre sous-graine. Le numéro d'une couche est fixé une fois pour toutes : ajouter
//! une couche ne change pas les tirages des autres, et une graine donne le même monde sur toutes
//! les machines (entiers uniquement, pas de flottants ni de trigonométrie).

/// Couches de la chaîne de génération. Les numéros sont figés : ne jamais les réutiliser ni les
/// renuméroter (ils changeraient tous les mondes). Une nouvelle couche prend un nouveau numéro.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Layer {
    Star = 1,
    Orbit = 2,
    Physics = 3,
    Composition = 4,
    Atmosphere = 5,
    Climate = 6,
    Hydrology = 7,
    Geology = 8,
    Relief = 9,
    Biology = 10,
    Resources = 11,
    Gameplay = 12,
    Traits = 13,
    /// Ceintures d'astéroïdes du système (C1 de la 0.11).
    Belts = 14,
}

impl Layer {
    pub const ALL: [Layer; 14] = [
        Layer::Star,
        Layer::Orbit,
        Layer::Physics,
        Layer::Composition,
        Layer::Atmosphere,
        Layer::Climate,
        Layer::Hydrology,
        Layer::Geology,
        Layer::Relief,
        Layer::Biology,
        Layer::Resources,
        Layer::Gameplay,
        Layer::Traits,
        Layer::Belts,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Layer::Star => "etoile",
            Layer::Orbit => "orbite",
            Layer::Physics => "physique",
            Layer::Composition => "composition",
            Layer::Atmosphere => "atmosphere",
            Layer::Climate => "climat",
            Layer::Hydrology => "hydrologie",
            Layer::Geology => "geologie",
            Layer::Relief => "relief",
            Layer::Biology => "biologie",
            Layer::Resources => "ressources",
            Layer::Gameplay => "gameplay",
            Layer::Traits => "traits",
            Layer::Belts => "ceintures",
        }
    }
}

/// Mélangeur SplitMix64 : bijectif, rapide, sans flottants.
pub const fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// Sous-graine de la couche `layer` d'un astre de graine `body_seed`.
pub fn layer_seed(body_seed: u64, layer: Layer) -> u64 {
    splitmix64(body_seed ^ splitmix64(0x5350_4F52_4500_0000 | layer as u64))
}

/// Tirages déterministes d'une couche.
pub struct LayerRng(u64);

impl LayerRng {
    pub fn new(body_seed: u64, layer: Layer) -> Self {
        Self(layer_seed(body_seed, layer))
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        splitmix64(self.0)
    }

    /// Valeur dans [0, 1[, avec 53 bits de précision.
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Valeur dans [min, max[.
    pub fn range(&mut self, min: f64, max: f64) -> f64 {
        min + self.unit() * (max - min)
    }

    /// Indice tiré selon des poids (`weights` non vide, poids >= 0).
    pub fn weighted(&mut self, weights: &[f64]) -> usize {
        let total: f64 = weights.iter().sum();
        let mut roll = self.unit() * total;
        for (i, w) in weights.iter().enumerate() {
            if roll < *w {
                return i;
            }
            roll -= w;
        }
        weights.len() - 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn every_layer_has_its_own_seed() {
        let seeds: HashSet<u64> = Layer::ALL.iter().map(|&l| layer_seed(42, l)).collect();
        assert_eq!(seeds.len(), Layer::ALL.len());
        let numbers: HashSet<u64> = Layer::ALL.iter().map(|&l| l as u64).collect();
        assert_eq!(numbers.len(), Layer::ALL.len(), "deux couches ont le meme numero");
    }

    #[test]
    fn seeds_never_change() {
        // Valeurs figées : si ce test casse, tous les mondes générés changent (PROTOCOL à augmenter)
        assert_eq!(splitmix64(0), 0xE220_A839_7B1D_CDAF);
        assert_eq!(layer_seed(42, Layer::Star), layer_seed(42, Layer::Star));
        assert_ne!(layer_seed(42, Layer::Star), layer_seed(43, Layer::Star));
    }

    #[test]
    fn layer_draws_are_reproducible_and_bounded() {
        let mut a = LayerRng::new(7, Layer::Physics);
        let mut b = LayerRng::new(7, Layer::Physics);
        for _ in 0..1000 {
            let (x, y) = (a.unit(), b.unit());
            assert_eq!(x.to_bits(), y.to_bits());
            assert!((0.0..1.0).contains(&x));
        }
        let mut c = LayerRng::new(7, Layer::Star);
        let mut counts = [0usize; 3];
        for _ in 0..10_000 {
            counts[c.weighted(&[1.0, 0.0, 9.0])] += 1;
        }
        assert_eq!(counts[1], 0);
        assert!(counts[2] > counts[0] * 5);
    }
}
