// ─────────────────────────────────────────────────────────────────────────
//  Économie du joueur : crédits, soute, réputation auprès des factions PNJ
//  et missions.
//
//  - Crédits et soute sont enregistrés dans `saves/economy.json`.
//  - Chaque faction a ses prix (±25 % autour du prix de base) : acheter chez l'une
//    et revendre chez l'autre rapporte. Acheter coûte toujours plus cher que
//    revendre au même endroit.
//  - La réputation (Hostile → Allié) donne des remises et débloque les missions.
//  - Missions suivies automatiquement : Exploration, Reconnaissance, Livraison.
//  Les articles d'équipement (armes, boucliers, modules) s'achètent et se
//  rangent en soute ; ils n'ont pas encore d'effet en jeu.
// ─────────────────────────────────────────────────────────────────────────

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::galaxy_fx::NpcTerritories;
use crate::net::Net;
use crate::settings::GameSettings;
use crate::ui::CameraTarget;
use crate::{target_system, StarId, StarRoot};

pub const START_CREDITS: i64 = 2_500;
pub const MAX_ACTIVE_MISSIONS: usize = 3;
/// Prix d'une étoile : très élevé. Fixe + supplément par planète du système.
const STAR_BASE_PRICE: i64 = 150_000;
const STAR_PRICE_PER_PLANET: i64 = 15_000;
/// Une faction garde au moins ce nombre d'étoiles : elle refuse de vendre la dernière.
pub const MIN_FACTION_STARS: usize = 4;

/// Prix d'une étoile et de ses planètes chez `faction`.
pub fn star_price(eco: &Economy, faction: usize, planets: usize) -> i64 {
    let base = (STAR_BASE_PRICE + STAR_PRICE_PER_PLANET * planets as i64) as f64;
    (base * (1.0 - eco.tier(faction).discount())).round() as i64
}

// ── Marchandises ────────────────────────────────────────────────────────

pub type GoodId = usize;

/// Groupes du shop dans l'ordre d'affichage.
pub const GROUPS: [&str; 7] = ["Armes", "Boucliers", "Modules de vaisseau", "Carburant", "Ressources", "Cartes stellaires", "Donnees"];

/// (nom, groupe, prix de base en crédits). L'index est l'identifiant stable de la marchandise.
/// Biens : (nom, rayon, prix de base). On n'ajoute qu'à la fin (les sauvegardes gardent les numéros).
pub const GOODS: [(&str, usize, i64); 34] = [
    ("Laser leger", 0, 1200),
    ("Canon a plasma", 0, 4800),
    ("Lance-missiles", 0, 7500),
    ("Bouclier basique", 1, 900),
    ("Bouclier renforce", 1, 3200),
    ("Bouclier quantique", 1, 9800),
    ("Moteur ionique", 2, 2500),
    ("Soute etendue", 2, 1800),
    ("Scanner longue portee", 2, 3900),
    ("Cellule de carburant", 3, 15),
    ("Reservoir complet", 3, 1200),
    ("Fer raffine", 4, 25),
    ("Cristaux", 4, 60),
    ("Deuterium", 4, 40),
    ("Carte du secteur", 5, 600),
    ("Carte de la galaxie", 5, 2400),
    ("Route de trou de ver", 5, 5200),
    ("Position de pirates", 6, 800),
    ("Archives anciennes", 6, 3000),
    // Minerais des astres (phase 8, `planetgen/resources.rs`)
    ("Nickel", 4, 45),
    ("Cuivre", 4, 70),
    ("Aluminium", 4, 40),
    ("Titane", 4, 120),
    ("Or", 4, 900),
    ("Platine", 4, 1100),
    ("Uranium", 4, 600),
    ("Terres rares", 4, 350),
    ("Silicium", 4, 30),
    ("Glace d'eau", 4, 12),
    ("Helium-3", 4, 1500),
    ("Hydrocarbures", 4, 35),
    ("Xenium (fictif)", 4, 2500),
    ("Aetherite (fictif)", 4, 4000),
    ("Chronite (fictif)", 4, 9000),
];

/// Premier bien de minerai ajouté en phase 8.
pub const FIRST_ORE_GOOD: GoodId = 19;

pub const FUEL: GoodId = 9;
pub const IRON: GoodId = 11;

pub fn goods_of_group(group: usize) -> impl Iterator<Item = GoodId> {
    (0..GOODS.len()).filter(move |&g| GOODS[g].1 == group)
}

/// Une faction ne vend pas tous les minerais : les biens d'origine partout, ~1 minerai sur 3,
/// et les minerais fictifs (très chers) rarement. Elle les rachète tous.
pub fn sold_by(faction: usize, good: GoodId) -> bool {
    if good < FIRST_ORE_GOOD {
        return true;
    }
    let roll = hash(faction, good, 11) % 100;
    if GOODS[good].2 >= 2500 {
        roll < 10
    } else {
        roll < 35
    }
}

fn hash(a: usize, b: usize, c: usize) -> u32 {
    let mut x = (a as u32).wrapping_mul(0x9E37_79B1) ^ (b as u32).wrapping_mul(0x85EB_CA6B) ^ (c as u32).wrapping_mul(0xC2B2_AE35);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    x = x.wrapping_mul(0x297A_2D39);
    x ^ (x >> 15)
}

/// Nombre pseudo-aléatoire stable dans `0..len`.
pub fn pick(a: usize, b: usize, c: usize, len: usize) -> usize {
    hash(a, b, c) as usize % len.max(1)
}

/// Coefficient de prix propre à une faction pour une marchandise (0,80 à 1,30).
fn local_factor(faction: usize, good: GoodId) -> f64 {
    0.8 + (hash(faction, good, 7) % 51) as f64 / 100.0
}

// ── Réputation ──────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tier {
    Enemy,
    Wary,
    Unknown,
    Neutral,
    Trusted,
    Friend,
    Ally,
}

impl Tier {
    pub fn of(rep: i32) -> Tier {
        match rep {
            i32::MIN..=-41 => Tier::Enemy,
            -40..=-1 => Tier::Wary,
            0..=9 => Tier::Unknown,
            10..=29 => Tier::Neutral,
            30..=59 => Tier::Trusted,
            60..=99 => Tier::Friend,
            _ => Tier::Ally,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Tier::Enemy => "Ennemi",
            Tier::Wary => "Mefiant",
            Tier::Unknown => "Inconnu",
            Tier::Neutral => "Neutre",
            Tier::Trusted => "Confiance",
            Tier::Friend => "Ami",
            Tier::Ally => "Allie",
        }
    }

    /// Remise sur les achats (0 à 15 %).
    pub fn discount(self) -> f64 {
        match self {
            Tier::Neutral => 0.03,
            Tier::Trusted => 0.07,
            Tier::Friend => 0.11,
            Tier::Ally => 0.15,
            _ => 0.0,
        }
    }

    /// Un ennemi ne commerce pas.
    pub fn trades(self) -> bool {
        self != Tier::Enemy
    }
}

// ── Missions ────────────────────────────────────────────────────────────

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub enum MissionKind {
    /// Visiter `need` systèmes différents.
    Explore { need: u32 },
    /// Se rendre dans un système précis.
    Scout { sys: usize },
    /// Apporter un colis à la faction `to`.
    Deliver { to: usize },
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct Mission {
    pub kind: MissionKind,
    pub issuer: usize,
    pub reward: i64,
    pub rep: i32,
    /// Systèmes déjà visités (exploration).
    #[serde(default)]
    pub seen: Vec<usize>,
}

impl Mission {
    pub fn describe(&self, npcs: &NpcTerritories, settings: &GameSettings) -> String {
        let fname = |i: usize| npcs.factions.get(&i).map_or("?".to_string(), |f| f.name.clone());
        match &self.kind {
            MissionKind::Explore { need } => format!("Exploration : visiter {need} systemes differents ({}/{need})", self.seen.len()),
            MissionKind::Scout { sys } => {
                format!("Reconnaissance : atteindre le systeme {}", settings.systems.get(*sys).map_or("?", |s| s.name.as_str()))
            }
            MissionKind::Deliver { to } => format!("Livraison : apporter un colis a {}", fname(*to)),
        }
    }
}

/// Les trois missions proposées par une faction : fixes pour une `edition` donnée.
pub fn offered_missions(faction: usize, edition: usize, npcs: &NpcTerritories, settings: &GameSettings) -> Vec<Mission> {
    let Some(me) = npcs.factions.get(&faction) else { return Vec::new() };
    let home = me.stars.first().and_then(|&s| settings.systems.get(s)).map(|s| s.center()).unwrap_or_default();
    (0..3)
        .map(|slot| {
            let roll = pick(faction, edition, 100 + slot, 3);
            let kind = match roll {
                0 => MissionKind::Explore { need: 3 + pick(faction, edition, slot, 4) as u32 },
                1 if me.stars.len() > 1 => {
                    let i = 1 + pick(faction, edition, 200 + slot, me.stars.len() - 1);
                    MissionKind::Scout { sys: me.stars[i] }
                }
                _ => {
                    // Faction la plus proche, de la même galaxie
                    let to = npcs
                        .factions
                        .iter()
                        .map(|(&i, f)| (i, f))
                        .filter(|(i, f)| *i != faction && f.galaxy == me.galaxy)
                        .min_by(|(_, a), (_, b)| {
                            let d = |f: &crate::galaxy_fx::NpcFaction| {
                                f.stars.first().and_then(|&s| settings.systems.get(s)).map_or(f32::MAX, |s| s.center().distance(home))
                            };
                            d(a).total_cmp(&d(b))
                        })
                        .map(|(i, _)| i);
                    match to {
                        Some(to) => MissionKind::Deliver { to },
                        None => MissionKind::Explore { need: 3 },
                    }
                }
            };
            let (reward, rep) = match &kind {
                MissionKind::Explore { need } => (300 * *need as i64, 3),
                MissionKind::Scout { .. } => (700 + 50 * pick(faction, edition, slot, 10) as i64, 4),
                MissionKind::Deliver { .. } => (1_200 + 100 * pick(faction, edition, slot, 10) as i64, 6),
            };
            Mission { kind, issuer: faction, reward, rep, seen: Vec::new() }
        })
        .collect()
}

// ── L'économie elle-même ────────────────────────────────────────────────

#[derive(Resource, Serialize, Deserialize, Clone, Debug)]
pub struct Economy {
    pub credits: i64,
    pub inventory: BTreeMap<GoodId, u32>,
    /// Réputation par faction (index dans `NpcTerritories`).
    pub reputation: BTreeMap<usize, i32>,
    pub missions: Vec<Mission>,
    /// Édition des missions proposées, par faction : change quand on en accepte une.
    #[serde(default)]
    pub editions: BTreeMap<usize, usize>,
    /// Étoiles achetées à des factions PNJ (retirées de leur territoire à chaque lancement).
    #[serde(default)]
    pub sold_stars: Vec<usize>,
    #[serde(skip)]
    pub dirty: bool,
}

impl Default for Economy {
    fn default() -> Self {
        Self {
            credits: START_CREDITS,
            inventory: BTreeMap::from([(IRON, 40), (FUEL, 20)]),
            reputation: BTreeMap::new(),
            missions: Vec::new(),
            editions: BTreeMap::new(),
            sold_stars: Vec::new(),
            dirty: false,
        }
    }
}

impl Economy {
    pub fn rep(&self, faction: usize) -> i32 {
        self.reputation.get(&faction).copied().unwrap_or(0)
    }

    pub fn tier(&self, faction: usize) -> Tier {
        Tier::of(self.rep(faction))
    }

    pub fn add_rep(&mut self, faction: usize, delta: i32) {
        let r = self.reputation.entry(faction).or_insert(0);
        *r = (*r + delta).clamp(-100, 150);
        self.dirty = true;
    }

    pub fn count(&self, good: GoodId) -> u32 {
        self.inventory.get(&good).copied().unwrap_or(0)
    }

    /// Prix d'achat à l'unité chez `faction` (`haggle` : remise négociée, 0 à 0,1).
    pub fn buy_price(&self, faction: usize, good: GoodId, haggle: f64) -> i64 {
        let base = GOODS[good].2 as f64 * local_factor(faction, good);
        ((base * (1.0 - self.tier(faction).discount() - haggle)).round() as i64).max(1)
    }

    /// Prix de rachat à l'unité chez `faction`.
    pub fn sell_price(&self, faction: usize, good: GoodId) -> i64 {
        let base = GOODS[good].2 as f64 * local_factor(faction, good);
        ((base * 0.6 * (1.0 + self.tier(faction).discount() / 2.0)).round() as i64).max(1)
    }

    pub fn buy(&mut self, faction: usize, good: GoodId, qty: u32, haggle: f64) -> Result<i64, &'static str> {
        let cost = self.buy_price(faction, good, haggle) * qty as i64;
        if qty == 0 {
            return Err("Quantite nulle.");
        }
        if cost > self.credits {
            return Err("Credits insuffisants.");
        }
        self.credits -= cost;
        *self.inventory.entry(good).or_insert(0) += qty;
        self.dirty = true;
        Ok(cost)
    }

    pub fn sell(&mut self, faction: usize, good: GoodId, qty: u32) -> Result<i64, &'static str> {
        if qty == 0 || self.count(good) < qty {
            return Err("Pas assez en soute.");
        }
        let gain = self.sell_price(faction, good) * qty as i64;
        self.credits += gain;
        self.take(good, qty);
        Ok(gain)
    }

    /// Retire `qty` unités de la soute (le test de quantité est à la charge de l'appelant).
    pub fn take(&mut self, good: GoodId, qty: u32) {
        if let Some(n) = self.inventory.get_mut(&good) {
            *n = n.saturating_sub(qty);
            if *n == 0 {
                self.inventory.remove(&good);
            }
        }
        self.dirty = true;
    }

    pub fn give(&mut self, good: GoodId, qty: u32) {
        *self.inventory.entry(good).or_insert(0) += qty;
        self.dirty = true;
    }

    /// Valeur de référence d'un lot (prix de base, sans coefficient local).
    pub fn base_value(good: GoodId, qty: u32) -> i64 {
        GOODS[good].2 * qty as i64
    }

    pub fn accept_mission(&mut self, m: Mission) -> Result<(), &'static str> {
        if self.missions.len() >= MAX_ACTIVE_MISSIONS {
            return Err("Trop de missions en cours.");
        }
        if self.missions.iter().any(|x| x.kind == m.kind && x.issuer == m.issuer) {
            return Err("Mission deja acceptee.");
        }
        *self.editions.entry(m.issuer).or_insert(0) += 1;
        self.missions.push(m);
        self.dirty = true;
        Ok(())
    }

    /// Paie et retire la mission `i`.
    pub fn complete(&mut self, i: usize) -> Mission {
        let m = self.missions.remove(i);
        self.credits += m.reward;
        self.add_rep(m.issuer, m.rep);
        m
    }

    fn path() -> std::path::PathBuf {
        crate::settings::data_dir().join("economy.json")
    }

    pub fn load() -> Self {
        if cfg!(test) {
            return Self::default();
        }
        std::fs::read_to_string(Self::path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }

    fn save(&self) {
        if cfg!(test) {
            return;
        }
        if let Ok(json) = serde_json::to_string(self) {
            std::fs::write(Self::path(), json).ok();
        }
    }
}

// ── Plugin : enregistrement et suivi des missions ───────────────────────

pub struct EconomyPlugin;

impl Plugin for EconomyPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Economy::load()).add_systems(Update, (track_missions, save_economy).chain());
    }
}

fn save_economy(mut eco: ResMut<Economy>) {
    if eco.dirty {
        eco.dirty = false;
        eco.save();
    }
}

/// Exploration et reconnaissance : suivies d'après le système ciblé par la caméra.
fn track_missions(
    target: Res<CameraTarget>,
    star_q: Query<&StarId, With<StarRoot>>,
    time: Res<Time>,
    npcs: Res<NpcTerritories>,
    settings: Res<GameSettings>,
    mut eco: ResMut<Economy>,
    mut net: ResMut<Net>,
    mut last: Local<Option<usize>>,
) {
    let Some(Some(sys)) = target_system(&target.0, &star_q) else { return };
    if *last == Some(sys) {
        return;
    }
    *last = Some(sys);
    let mut done = Vec::new();
    for (i, m) in eco.missions.iter_mut().enumerate() {
        match m.kind {
            MissionKind::Explore { need } => {
                if !m.seen.contains(&sys) {
                    m.seen.push(sys);
                }
                if m.seen.len() as u32 >= need {
                    done.push(i);
                }
            }
            MissionKind::Scout { sys: goal } if goal == sys => done.push(i),
            _ => {}
        }
    }
    eco.dirty = true;
    for i in done.into_iter().rev() {
        let m = eco.complete(i);
        let text = format!("Mission accomplie : {} (+{} cr)", m.describe(&npcs, &settings), m.reward);
        net.notify(&text, time.elapsed_secs_f64());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buying_costs_more_than_selling_back() {
        for tier_rep in [0, 20, 40, 80, 120] {
            let mut e = Economy::default();
            e.reputation.insert(3, tier_rep);
            for g in 0..GOODS.len() {
                assert!(e.buy_price(3, g, 0.0) > e.sell_price(3, g), "good {g} rep {tier_rep}");
            }
        }
    }

    #[test]
    fn buy_and_sell_move_credits_and_stock() {
        let mut e = Economy::default();
        let cost = e.buy(0, IRON, 10, 0.0).unwrap();
        assert_eq!(e.credits, START_CREDITS - cost);
        assert_eq!(e.count(IRON), 50);
        let gain = e.sell(0, IRON, 50).unwrap();
        assert_eq!(e.count(IRON), 0);
        assert!(gain > 0);
        assert!(e.sell(0, IRON, 1).is_err());
        assert!(e.buy(0, 2, 1000, 0.0).is_err());
    }

    #[test]
    fn mission_limit_and_payout() {
        let mut e = Economy::default();
        let m = Mission { kind: MissionKind::Explore { need: 3 }, issuer: 1, reward: 900, rep: 3, seen: vec![] };
        assert!(e.accept_mission(m.clone()).is_ok());
        assert!(e.accept_mission(m.clone()).is_err());
        let done = e.complete(0);
        assert_eq!(e.credits, START_CREDITS + done.reward);
        assert_eq!(e.rep(1), 3);
    }

    #[test]
    fn tiers_and_persistence() {
        assert_eq!(Tier::of(-50), Tier::Enemy);
        assert_eq!(Tier::of(100), Tier::Ally);
        let e = Economy::default();
        let back: Economy = serde_json::from_str(&serde_json::to_string(&e).unwrap()).unwrap();
        assert_eq!(back.credits, e.credits);
        assert_eq!(back.inventory, e.inventory);
    }

    #[test]
    fn ore_goods_match_the_ores() {
        use crate::planetgen::resources::Ore;
        for ore in Ore::ALL {
            let g = ore.good();
            assert_eq!(GOODS[g].1, 4, "{ore:?} dans le rayon Ressources");
            assert!(GOODS[g].0.to_lowercase().starts_with(&ore.name()[..ore.name().len().min(3)]) || g < FIRST_ORE_GOOD, "{ore:?} -> {}", GOODS[g].0);
        }
        // Chaque faction vend au moins quelques minerais, jamais tous
        for f in 0..20 {
            let n = (FIRST_ORE_GOOD..GOODS.len()).filter(|&g| sold_by(f, g)).count();
            assert!(n < GOODS.len() - FIRST_ORE_GOOD);
        }
        assert!((0..20).map(|f| (FIRST_ORE_GOOD..GOODS.len()).filter(|&g| sold_by(f, g)).count()).sum::<usize>() > 40);
    }
}
