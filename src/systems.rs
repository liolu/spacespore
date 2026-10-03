//! Les systèmes stellaires de l'univers (0.11) : 10 000 galaxies, des dizaines de millions de
//! systèmes, impossibles à garder tous en mémoire.
//!
//! - Notre galaxie et les galaxies extérieures (les 21 premières) sont générées au départ, comme
//!   avant (mêmes systèmes, mêmes numéros).
//! - Chaque galaxie lointaine a d'avance une plage de numéros fixe (son budget d'étoiles) : ses
//!   systèmes ne sont générés qu'au premier accès (`get`), puis gardés. Les numéros ne dépendent
//!   donc pas de l'ordre des visites : revendications, trous de ver et réseau restent cohérents.
//! - `iter` ne parcourt que les systèmes déjà générés, et donne leur numéro avec : on ne peut pas
//!   confondre un rang avec un numéro.

use std::ops::{Index, IndexMut, Range};
use std::sync::{Arc, OnceLock};

use crate::settings::{GalaxyConfig, StarSystemConfig, SystemMaker};

/// Une galaxie lointaine : sa plage de numéros et ses systèmes (une fois générés). Un numéro de la
/// plage peut rester vide (étoile tirée dans le trou noir central).
#[derive(Clone, Debug)]
struct LazyGalaxy {
    gid: u32,
    base: usize,
    len: usize,
    data: OnceLock<Vec<Option<StarSystemConfig>>>,
}

#[derive(Clone, Debug, Default)]
pub struct Systems {
    dense: Vec<StarSystemConfig>,
    /// Plage de numéros de chaque galaxie du bloc généré au départ.
    dense_ranges: Vec<Range<usize>>,
    lazy: Vec<LazyGalaxy>,
    total: usize,
    maker: Option<SystemMaker>,
    galaxies: Arc<Vec<GalaxyConfig>>,
}

impl Systems {
    /// `dense_galaxies` premières galaxies générées tout de suite, les autres à la demande.
    pub fn new(galaxies: &[GalaxyConfig], world_seed: u64, dense_galaxies: usize) -> Self {
        let maker = SystemMaker::new(world_seed);
        let (dense, dense_ranges) = maker.dense(&galaxies[..dense_galaxies.min(galaxies.len())]);
        let mut base = dense.len();
        let mut lazy = Vec::new();
        for (gid, g) in galaxies.iter().enumerate().skip(dense_galaxies) {
            let len = g.arm_stars + g.scatter_stars;
            lazy.push(LazyGalaxy { gid: gid as u32, base, len, data: OnceLock::new() });
            base += len;
        }
        Self { dense, dense_ranges, lazy, total: base, maker: Some(maker), galaxies: Arc::new(galaxies.to_vec()) }
    }

    /// Une liste donnée telle quelle (tests, éditeur).
    pub fn from_vec(dense: Vec<StarSystemConfig>) -> Self {
        let total = dense.len();
        Self { dense, total, ..Default::default() }
    }

    /// Nombre de numéros de systèmes (y compris ceux des galaxies pas encore générées).
    pub fn len(&self) -> usize {
        self.total
    }

    pub fn is_empty(&self) -> bool {
        self.total == 0
    }

    /// Systèmes générés au départ (notre galaxie et les extérieures) : les mêmes sur toutes les
    /// machines.
    pub fn dense(&self) -> &[StarSystemConfig] {
        &self.dense
    }

    #[cfg(test)]
    pub fn dense_mut(&mut self) -> &mut [StarSystemConfig] {
        &mut self.dense
    }

    pub fn first(&self) -> Option<&StarSystemConfig> {
        self.dense.first()
    }

    pub fn first_mut(&mut self) -> Option<&mut StarSystemConfig> {
        self.dense.first_mut()
    }

    fn block(&self, i: usize) -> Option<&LazyGalaxy> {
        if i < self.dense.len() || i >= self.total {
            return None;
        }
        let k = self.lazy.partition_point(|b| b.base + b.len <= i);
        self.lazy.get(k)
    }

    fn data<'a>(&'a self, b: &'a LazyGalaxy) -> &'a Vec<Option<StarSystemConfig>> {
        b.data.get_or_init(|| match (&self.maker, self.galaxies.get(b.gid as usize)) {
            (Some(maker), Some(g)) => maker.lazy(b.gid, g, b.base),
            _ => vec![None; b.len],
        })
    }

    /// Le système `i` (sa galaxie est générée si besoin).
    pub fn get(&self, i: usize) -> Option<&StarSystemConfig> {
        if i < self.dense.len() {
            return self.dense.get(i);
        }
        let b = self.block(i)?;
        self.data(b).get(i - b.base)?.as_ref()
    }

    /// Le système `i` s'il est déjà généré (sans rien générer).
    pub fn peek(&self, i: usize) -> Option<&StarSystemConfig> {
        if i < self.dense.len() {
            return self.dense.get(i);
        }
        let b = self.block(i)?;
        b.data.get()?.get(i - b.base)?.as_ref()
    }

    pub fn get_mut(&mut self, i: usize) -> Option<&mut StarSystemConfig> {
        if i < self.dense.len() {
            return self.dense.get_mut(i);
        }
        let k = self.lazy.partition_point(|b| b.base + b.len <= i);
        let b = self.lazy.get_mut(k)?;
        let base = b.base;
        b.data.get_mut()?.get_mut(i - base)?.as_mut()
    }

    /// Systèmes déjà générés, avec leur numéro.
    pub fn iter(&self) -> impl Iterator<Item = (usize, &StarSystemConfig)> + '_ {
        let dense = self.dense.iter().enumerate();
        let lazy = self
            .lazy
            .iter()
            .filter_map(|b| b.data.get().map(|d| (b.base, d)))
            .flat_map(|(base, d)| d.iter().enumerate().filter_map(move |(k, s)| s.as_ref().map(|s| (base + k, s))));
        dense.chain(lazy)
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (usize, &mut StarSystemConfig)> + '_ {
        let dense = self.dense.iter_mut().enumerate();
        let lazy = self
            .lazy
            .iter_mut()
            .filter_map(|b| {
                let base = b.base;
                b.data.get_mut().map(move |d| (base, d))
            })
            .flat_map(|(base, d)| d.iter_mut().enumerate().filter_map(move |(k, s)| s.as_mut().map(|s| (base + k, s))));
        dense.chain(lazy)
    }

    /// Plage de numéros de la galaxie `gid`.
    pub fn galaxy_range(&self, gid: u32) -> Range<usize> {
        if let Some(r) = self.dense_ranges.get(gid as usize) {
            return r.clone();
        }
        self.lazy.iter().find(|b| b.gid == gid).map_or(0..0, |b| b.base..b.base + b.len)
    }

    /// Systèmes de la galaxie `gid` (générés si besoin), avec leur numéro.
    pub fn in_galaxy(&self, gid: u32) -> Vec<(usize, &StarSystemConfig)> {
        if let Some(r) = self.dense_ranges.get(gid as usize) {
            return r.clone().filter_map(|i| self.dense.get(i).map(|s| (i, s))).collect();
        }
        match self.lazy.iter().find(|b| b.gid == gid) {
            Some(b) => self.data(b).iter().enumerate().filter_map(|(k, s)| s.as_ref().map(|s| (b.base + k, s))).collect(),
            None => Vec::new(),
        }
    }

    /// La galaxie `gid` est générée (toujours vrai pour les galaxies du départ).
    pub fn is_loaded(&self, gid: u32) -> bool {
        (gid as usize) < self.dense_ranges.len() || self.lazy.iter().any(|b| b.gid == gid && b.data.get().is_some())
    }

    /// Génère la galaxie `gid` maintenant (approche du joueur).
    pub fn load(&self, gid: u32) {
        if let Some(b) = self.lazy.iter().find(|b| b.gid == gid) {
            self.data(b);
        }
    }

    /// Galaxies lointaines déjà générées.
    pub fn loaded_far_galaxies(&self) -> Vec<u32> {
        self.lazy.iter().filter(|b| b.data.get().is_some()).map(|b| b.gid).collect()
    }

    /// Ajoute un système (éditeur) : seulement dans une liste donnée telle quelle.
    pub fn push(&mut self, sys: StarSystemConfig) {
        if self.lazy.is_empty() {
            self.dense.push(sys);
            self.total = self.dense.len();
        }
    }
}

impl Index<usize> for Systems {
    type Output = StarSystemConfig;
    fn index(&self, i: usize) -> &StarSystemConfig {
        self.get(i).unwrap_or_else(|| panic!("systeme {i} inexistant"))
    }
}

impl IndexMut<usize> for Systems {
    fn index_mut(&mut self, i: usize) -> &mut StarSystemConfig {
        self.get(i);
        self.get_mut(i).unwrap_or_else(|| panic!("systeme {i} inexistant"))
    }
}
