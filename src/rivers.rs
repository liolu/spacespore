//! Rivières et lacs (0.14 D1, « eau vivante »).
//!
//! Le réseau d'un astre est tracé une fois (`RiverNet::build`) depuis le relief seul, avec la même fonction
//! de hauteur que les tuiles du sol et le maillage vu de l'espace (règle 16) : des sources tirées de la
//! graine sur les terres humides descendent la pente la plus forte, pas après pas (`STEP_VOXELS`), jusqu'à
//! la mer (embouchure élargie en estuaire), jusqu'à une rivière déjà tracée (confluent) ou jusqu'à une
//! cuvette : un lac se forme, rempli jusqu'au col le plus bas, et la rivière repart du col. Le lit ne remonte
//! jamais (test `rivers_only_go_down`) : une falaise sur le chemin fait une cascade. La largeur suit le
//! débit (pas en amont, affluents compris).
//!
//! Ensuite chaque colonne du terrain interroge le réseau (`carve`) : le sol est creusé en auge sous le lit,
//! les berges descendent en pente douce vers lui (`REACH_VOXELS` au plus), et la surface de l'eau est celle
//! du lit à cet endroit ; un lac remplit sa cuvette jusqu'à son niveau. Tout est f(graine) : identique pour
//! tous les joueurs (règle 9). Le réseau est gardé en cache par astre (`of`) : les tâches de tuiles et le
//! maillage lointain partagent le même.

use bevy::math::Vec3;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use crate::planetgen::hydrology::Liquid;
use crate::terrain::{BodyParams, Terrain};

/// Pas du tracé (voxels).
pub const STEP_VOXELS: f32 = 12.0;
/// Portée (voxels) d'une rivière sur le sol autour de son lit (berges) : au-delà, le relief est intact.
pub const REACH_VOXELS: f32 = 90.0;
/// Largeur (voxels) de la plus petite et de la plus grande rivière (hors estuaire).
pub const MIN_WIDTH_VOXELS: f32 = 3.0;
pub const MAX_WIDTH_VOXELS: f32 = 60.0;
/// Largeur (voxels) à partir de laquelle une rivière se voit de l'espace.
pub const SPACE_WIDTH_VOXELS: f32 = 20.0;
/// Pente des berges (montée par unité de distance au-delà du lit).
const BANK_SLOPE: f32 = 0.6;
/// Longueur maximale d'une rivière (pas) et nombre de lacs qu'elle peut traverser.
const MAX_STEPS: usize = 1500;
const MAX_LAKES: usize = 20;
/// Recherche du col d'un lac : directions essayées et pas parcourus dans chacune.
const LAKE_RAYS: usize = 16;
const LAKE_RANGE: usize = 40;
/// Sources tentées sur une planète de 9 000 unités de rayon (proportionnel à la surface).
const SOURCES_AT_9000: f32 = 700.0;
/// Nœuds au plus par astre (mémoire).
const MAX_NODES: usize = 250_000;
/// Réseaux gardés en mémoire.
const CACHE_NETS: usize = 8;
/// Bit des lacs dans la grille.
const LAKE_BIT: u32 = 1 << 31;

#[derive(Clone, Copy, Debug)]
struct Node {
    dir: Vec3,
    /// Surface de l'eau (rayon).
    bed: f32,
    /// Largeur du lit (unités), connue après le calcul des débits.
    width: f32,
}

#[derive(Clone, Copy, Debug)]
struct River {
    start: u32,
    end: u32,
    /// Où continue le débit : (rivière, nœud) : le tronçon après un lac, ou la rivière rejointe.
    next: Option<(u32, u32)>,
    /// Finit dans la mer (estuaire).
    mouth: bool,
}

/// Un lac : la cuvette autour de `center` sous `level`, dans un rayon angulaire `reach`.
#[derive(Clone, Copy, Debug)]
pub struct Lake {
    pub center: Vec3,
    pub level: f32,
    pub reach: f32,
    /// Largeur (unités) de la rivière qui s'y jette (visibilité de l'espace).
    pub width: f32,
}

/// Ce que le réseau dit d'un point : le sol creusé, et la surface de l'eau s'il est dans un lit ou un lac.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Carve {
    pub ground: f32,
    pub water: Option<f32>,
    /// Largeur (unités) de la plus grande rivière ou du lac à cet endroit (0 : aucun).
    pub width: f32,
}

pub struct RiverNet {
    radius: f32,
    voxel: f32,
    nodes: Vec<Node>,
    rivers: Vec<River>,
    lakes: Vec<Lake>,
    /// Grille 3D de la sphère unité -> segments (indice du nœud amont) et lacs (`LAKE_BIT`).
    grid: HashMap<(i32, i32, i32), Vec<u32>>,
    inv_cell: f32,
    /// Longueur totale (unités).
    length: f32,
}

type Key = (u32, u32, u32, u32, u32, u32);

fn key_of(p: &BodyParams, voxel: f32) -> Key {
    (p.seed, p.radius.to_bits(), voxel.to_bits(), p.terrain_height.to_bits(), p.sea_level.to_bits(), p.noise_scale.to_bits())
}

static NETS: Mutex<Vec<(Key, Arc<OnceLock<Arc<RiverNet>>>)>> = Mutex::new(Vec::new());

impl RiverNet {
    /// Un astre a des rivières s'il a de l'air et un liquide (eau, méthane...).
    pub fn wanted(p: &BodyParams) -> bool {
        !p.gaseous && p.asteroid.is_none() && p.atmosphere && !p.airless && p.pressure >= 0.05 && p.hydro.liquid != Liquid::None
    }

    /// Le réseau de cet astre (calculé une fois, puis partagé). `t` ne doit pas encore avoir de rivières.
    pub fn of(t: &Terrain) -> Option<Arc<RiverNet>> {
        // (`SPACESPORE_NO_RIVERS=1` : mesures avant / après, `bench_tiles`)
        if !Self::wanted(&t.params) || std::env::var_os("SPACESPORE_NO_RIVERS").is_some() {
            return None;
        }
        let key = key_of(&t.params, t.voxel());
        let slot = {
            let mut nets = NETS.lock().unwrap_or_else(|e| e.into_inner());
            match nets.iter().find(|(k, _)| *k == key) {
                Some((_, s)) => s.clone(),
                None => {
                    let s = Arc::new(OnceLock::new());
                    nets.push((key, s.clone()));
                    if nets.len() > CACHE_NETS {
                        nets.remove(0);
                    }
                    s
                }
            }
        };
        Some(slot.get_or_init(|| Arc::new(Self::build(t))).clone())
    }

    /// Le réseau déjà calculé pour ces paramètres, s'il est en cache (maillage lointain : sans bloquer).
    pub fn cached(p: &BodyParams, voxel: f32) -> Option<Arc<RiverNet>> {
        let key = key_of(p, voxel);
        let nets = NETS.lock().unwrap_or_else(|e| e.into_inner());
        nets.iter().find(|(k, _)| *k == key).and_then(|(_, s)| s.get().cloned())
    }

    /// Trace toutes les rivières de l'astre (quelques dixièmes de seconde par planète).
    fn build(t: &Terrain) -> Self {
        let p = &t.params;
        let radius = p.radius;
        let voxel = t.voxel();
        let step = STEP_VOXELS * voxel / radius;
        let sea = radius - 0.5 * voxel;
        let dig = 1.0 * voxel;
        let min_drop = 0.002 * voxel;
        let influence = (REACH_VOXELS + MAX_WIDTH_VOXELS * 0.5) * voxel / radius;
        let cell = 2.5 * influence;
        let mut net = Self { radius, voxel, nodes: Vec::new(), rivers: Vec::new(), lakes: Vec::new(), grid: HashMap::new(), inv_cell: 1.0 / cell, length: 0.0 };

        let mut x = (p.seed as u64 ^ 0x51BE_D0C5_A11E_0000).wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
        let mut rnd = || {
            x ^= x >> 33;
            x = x.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
            x ^= x >> 29;
            (x >> 11) as f32 / (1u64 << 53) as f32
        };
        let wetness = if p.biomes.defined { p.biomes.wetness } else { 0.5 };
        let scale = (radius / 9000.0).powi(2);
        let sources = (SOURCES_AT_9000 * scale * (0.3 + 0.7 * wetness)).clamp(20.0, 1500.0) as usize;
        let golden = std::f32::consts::PI * (3.0 - 5f32.sqrt());
        let phase = rnd() * std::f32::consts::TAU;

        let frame = |d: Vec3| {
            let east = Vec3::Y.cross(d).normalize_or(Vec3::X);
            (east, d.cross(east).normalize())
        };
        let around = |d: Vec3, east: Vec3, north: Vec3, az: f32, ang: f32| (d * ang.cos() + (east * az.cos() + north * az.sin()) * ang.sin()).normalize();

        for k in 0..sources {
            if net.nodes.len() >= MAX_NODES {
                break;
            }
            // Sources : spirale de Fibonacci décalée par la graine, puis un peu d'aléa ; sur une terre
            // au-dessus de la mer, d'autant plus souvent que le lieu est humide
            let z = (1.0 - 2.0 * (k as f32 + 0.5) / sources as f32).clamp(-1.0, 1.0);
            let r = (1.0 - z * z).sqrt();
            let a = k as f32 * golden + phase;
            let mut dir = Vec3::new(r * a.cos(), z, r * a.sin());
            let (e, n) = frame(dir);
            dir = around(dir, e, n, rnd() * std::f32::consts::TAU, rnd() * 3.0 * step);
            let (u1, u2) = (rnd(), rnd());
            let h = t.ground_radius(dir);
            if h < radius + 3.0 * voxel {
                continue;
            }
            let hum = t.humidity(dir);
            if hum < 0.25 || u1 > hum {
                continue;
            }
            // Plus souvent en altitude (les sources sont dans les hauteurs)
            let alt = ((h - radius) / p.terrain_height.max(1.0)).clamp(0.0, 1.0);
            if u2 > 0.35 + alt {
                continue;
            }
            net.trace(t, dir, h, step, sea, dig, min_drop, influence);
        }
        net.finish(voxel, influence);
        net
    }

    /// Une rivière depuis sa source : descente de la pente la plus forte.
    #[allow(clippy::too_many_arguments)]
    fn trace(&mut self, t: &Terrain, source: Vec3, h0: f32, step: f32, sea: f32, dig: f32, min_drop: f32, influence: f32) {
        let frame = |d: Vec3| {
            let east = Vec3::Y.cross(d).normalize_or(Vec3::X);
            (east, d.cross(east).normalize())
        };
        let around = |d: Vec3, east: Vec3, north: Vec3, az: f32, ang: f32| (d * ang.cos() + (east * az.cos() + north * az.sin()) * ang.sin()).normalize();
        let bias = 0.3 * self.voxel;

        let mut pos = source;
        let mut h = h0;
        let mut bed = h0 - dig;
        let mut prev = Vec3::ZERO;
        let mut lakes = 0;
        let mut river = River { start: self.nodes.len() as u32, end: 0, next: None, mouth: false };
        self.nodes.push(Node { dir: pos, bed, width: 0.0 });
        let first_river = self.rivers.len();

        for _ in 0..MAX_STEPS {
            if self.nodes.len() >= MAX_NODES {
                break;
            }
            // Confluent : dans le lit d'une rivière déjà tracée
            if let Some((idx, point, main_bed)) = self.join_point(pos, influence) {
                bed = (bed - min_drop).min(main_bed);
                self.nodes.push(Node { dir: point, bed, width: 0.0 });
                let r = self.river_of(idx);
                river.next = Some((r, idx));
                break;
            }
            let (e, n) = frame(pos);
            let mut best: Option<(f32, Vec3, f32)> = None;
            for k in 0..8 {
                let az = k as f32 * std::f32::consts::TAU / 8.0;
                let d = around(pos, e, n, az, step);
                let hd = t.ground_radius(d);
                // Un peu d'élan : la rivière préfère continuer tout droit sur un sol presque plat
                let score = hd - bias * (d - pos).normalize_or_zero().dot(prev).max(0.0);
                if best.is_none_or(|(s, _, _)| score < s) {
                    best = Some((score, d, hd));
                }
            }
            let Some((_, mut d, mut hd)) = best else { break };
            // Petite bosse devant (relief à l'échelle du voxel) : la rivière la traverse si le sol redescend
            // à quelques pas de là, et ne forme un lac qu'au fond d'une vraie cuvette
            if hd >= h - 1e-4 {
                'look: for k in 2..=16 {
                    let mut low: Option<(f32, Vec3)> = None;
                    for a in 0..12 {
                        let az = (a as f32 + 0.5 * (k % 2) as f32) * std::f32::consts::TAU / 12.0;
                        let dd = around(pos, e, n, az, step * k as f32);
                        let hh = t.ground_radius(dd);
                        if hh < h - 1e-4 && low.is_none_or(|(l, _)| hh < l) {
                            low = Some((hh, dd));
                        }
                    }
                    if let Some((hh, dd)) = low {
                        d = dd;
                        hd = hh;
                        break 'look;
                    }
                }
            }
            if hd < h - 1e-4 {
                prev = (d - pos).normalize_or_zero();
                pos = d;
                h = hd;
                bed = (bed - min_drop).min(h - dig);
                self.nodes.push(Node { dir: pos, bed, width: 0.0 });
                if h < sea {
                    river.mouth = true;
                    break;
                }
                continue;
            }
            // Cuvette : un lac jusqu'au col le plus bas, et la rivière repart du col
            if lakes >= MAX_LAKES {
                break;
            }
            let mut best_saddle: Option<(f32, Vec3, Vec3)> = None;
            let mut lowest_wall = f32::INFINITY;
            for k in 0..LAKE_RAYS {
                let az = (k as f32 + 0.5) * std::f32::consts::TAU / LAKE_RAYS as f32;
                let mut wall = h;
                let mut wall_pos = pos;
                let mut escaped = None;
                for s in 1..=LAKE_RANGE {
                    let d = around(pos, e, n, az, step * s as f32);
                    let hd = t.ground_radius(d);
                    if hd > wall {
                        wall = hd;
                        wall_pos = d;
                    } else if hd < h - 1e-4 {
                        escaped = Some(d);
                        break;
                    }
                }
                lowest_wall = lowest_wall.min(wall);
                if escaped.is_some() && best_saddle.is_none_or(|(w, _, _)| wall < w) {
                    best_saddle = Some((wall, wall_pos, escaped.unwrap()));
                }
            }
            let Some((saddle, saddle_pos, exit)) = best_saddle else {
                // Bassin fermé : un lac sans issue, la rivière s'y arrête
                let level = (lowest_wall - 0.5 * self.voxel).max(h + 2.0 * self.voxel);
                let reach = step * LAKE_RANGE as f32;
                self.lakes.push(Lake { center: pos, level, reach, width: 0.0 });
                break;
            };
            let level = (saddle - 0.3 * self.voxel).max(h + 1.5 * self.voxel);
            let reach = (saddle_pos - pos).length().max(step);
            self.lakes.push(Lake { center: pos, level, reach, width: 0.0 });
            lakes += 1;
            // Le tronçon jusqu'au lac est fini ; le suivant part du col (pas de segment à travers le lac)
            river.end = self.nodes.len() as u32;
            if river.end - river.start >= 2 {
                let continuation = self.rivers.len() as u32 + 1;
                river.next = Some((continuation, self.nodes.len() as u32));
                self.rivers.push(river);
            } else {
                // (source tombée tout de suite dans la cuvette : le lac suffit)
                self.nodes.truncate(river.start as usize);
            }
            river = River { start: self.nodes.len() as u32, end: 0, next: None, mouth: false };
            bed = (bed - min_drop).min(level - dig);
            self.nodes.push(Node { dir: saddle_pos, bed, width: 0.0 });
            // Premier pas hors du lac : le point où la pente redescend
            pos = exit;
            h = t.ground_radius(exit);
            bed = (bed - min_drop).min(h - dig);
            prev = (exit - saddle_pos).normalize_or_zero();
            self.nodes.push(Node { dir: pos, bed, width: 0.0 });
            if h < sea {
                river.mouth = true;
                break;
            }
        }
        river.end = self.nodes.len() as u32;
        if river.end - river.start < 2 {
            // Une source sans chemin : rien
            self.nodes.truncate(river.start as usize);
            if self.rivers.len() > first_river {
                // (le tronçon d'avant le lac reste : il finit dans le lac)
                if let Some(last) = self.rivers.last_mut() {
                    last.next = None;
                }
            }
            return;
        }
        self.rivers.push(river);
        // Largeur provisoire (débit propre) pour les confluents, puis les nouveaux segments entrent dans la
        // grille : les rivières suivantes pourront les rejoindre
        for r in first_river..self.rivers.len() {
            let (s, e) = (self.rivers[r].start, self.rivers[r].end);
            for i in s..e {
                self.nodes[i as usize].width = (MIN_WIDTH_VOXELS + ((i - s + 1) as f32).sqrt()).min(MAX_WIDTH_VOXELS) * self.voxel;
            }
            for i in s..e.saturating_sub(1) {
                self.insert_segment(i, influence);
            }
        }
    }

    /// Rivière qui contient le nœud `idx`.
    fn river_of(&self, idx: u32) -> u32 {
        self.rivers.iter().position(|r| r.start <= idx && idx < r.end).map_or(0, |r| r as u32)
    }

    /// Le point du lit d'une rivière déjà tracée où `pos` tombe (segment, point, surface de l'eau).
    fn join_point(&self, pos: Vec3, _influence: f32) -> Option<(u32, Vec3, f32)> {
        let list = self.grid.get(&self.key(pos))?;
        let mut best: Option<(f32, u32, Vec3, f32)> = None;
        for &id in list {
            if id & LAKE_BIT != 0 {
                continue;
            }
            let (a, b) = (self.nodes[id as usize], self.nodes[id as usize + 1]);
            let ab = b.dir - a.dir;
            let len2 = ab.length_squared().max(1e-12);
            let t = ((pos - a.dir).dot(ab) / len2).clamp(0.0, 1.0);
            let p = a.dir + ab * t;
            let d = (pos - p).length() * self.radius;
            // (largeur provisoire : le débit propre de la rivière rejointe)
            let hw = (a.width + (b.width - a.width) * t) * 0.5;
            if d < hw.max(1.5 * self.voxel) + 1.0 * self.voxel && best.is_none_or(|(bd, ..)| d < bd) {
                best = Some((d, id, p.normalize(), a.bed + (b.bed - a.bed) * t));
            }
        }
        best.map(|(_, id, p, bed)| (id, p, bed))
    }

    fn key(&self, dir: Vec3) -> (i32, i32, i32) {
        ((dir.x * self.inv_cell).floor() as i32, (dir.y * self.inv_cell).floor() as i32, (dir.z * self.inv_cell).floor() as i32)
    }

    /// Toutes les cellules de la grille qu'une boîte (étendue de `pad`) touche.
    fn cells(&self, lo: Vec3, hi: Vec3, pad: f32) -> impl Iterator<Item = (i32, i32, i32)> {
        let (lo, hi) = (lo - Vec3::splat(pad), hi + Vec3::splat(pad));
        let a = self.key(lo);
        let b = self.key(hi);
        (a.0..=b.0).flat_map(move |x| (a.1..=b.1).flat_map(move |y| (a.2..=b.2).map(move |z| (x, y, z))))
    }

    fn insert_segment(&mut self, i: u32, influence: f32) {
        let (a, b) = (self.nodes[i as usize].dir, self.nodes[i as usize + 1].dir);
        let cells: Vec<_> = self.cells(a.min(b), a.max(b), influence).collect();
        for c in cells {
            self.grid.entry(c).or_default().push(i);
        }
    }

    /// Débits, largeurs, estuaires, lacs dans la grille.
    fn finish(&mut self, voxel: f32, influence: f32) {
        // Largeurs provisoires pendant le tracé : le débit propre. Ici le vrai débit : chaque rivière
        // ajoute son nombre de pas à tout ce qui est en aval d'elle (ordre indifférent).
        let mut flow = vec![0.0f32; self.nodes.len()];
        for r in 0..self.rivers.len() {
            let River { start, end, .. } = self.rivers[r];
            for i in start..end {
                flow[i as usize] += (i - start + 1) as f32;
            }
            let amount = (end - start) as f32;
            let mut next = self.rivers[r].next;
            let mut guard = 0;
            while let Some((r2, j)) = next {
                let River { end: e2, next: n2, .. } = self.rivers[r2 as usize];
                for i in j..e2 {
                    flow[i as usize] += amount;
                }
                next = n2;
                guard += 1;
                if guard > self.rivers.len() {
                    break;
                }
            }
        }
        for (i, n) in self.nodes.iter_mut().enumerate() {
            n.width = (MIN_WIDTH_VOXELS + flow[i].sqrt()).clamp(MIN_WIDTH_VOXELS, MAX_WIDTH_VOXELS) * voxel;
        }
        // Estuaires : les derniers pas s'élargissent vers la mer
        for r in &self.rivers {
            if r.mouth {
                let n = (r.end - r.start).min(8);
                for k in 0..n {
                    let i = (r.end - 1 - k) as usize;
                    self.nodes[i].width *= 1.0 + 0.15 * (n - k) as f32;
                }
            }
        }
        // Lacs : la largeur de la rivière qui s'y jette, et leur place dans la grille
        let mut lake_width = Vec::with_capacity(self.lakes.len());
        for l in &self.lakes {
            let w = self.rivers.iter().filter(|r| r.next.is_some_and(|(r2, _)| self.nodes.get(self.rivers[r2 as usize].start as usize).is_some_and(|n| (n.dir - l.center).length() < l.reach * 1.01))).map(|r| self.nodes[(r.end - 1) as usize].width).fold(0.0f32, f32::max);
            lake_width.push(w);
        }
        for (k, l) in self.lakes.iter_mut().enumerate() {
            l.width = lake_width[k].max(MIN_WIDTH_VOXELS * voxel);
        }
        let lakes: Vec<(usize, Vec3, f32)> = self.lakes.iter().enumerate().map(|(k, l)| (k, l.center, l.reach)).collect();
        for (k, center, reach) in lakes {
            let cells: Vec<_> = self.cells(center, center, reach + influence * 0.1).collect();
            for c in cells {
                self.grid.entry(c).or_default().push(k as u32 | LAKE_BIT);
            }
        }
        let mut length = 0.0;
        for r in &self.rivers {
            for i in r.start..r.end.saturating_sub(1) {
                length += (self.nodes[i as usize + 1].dir - self.nodes[i as usize].dir).length() * self.radius;
            }
        }
        self.length = length;
        for v in self.grid.values_mut() {
            v.shrink_to_fit();
        }
    }

    /// Le sol et l'eau au point `dir` dont le sol naturel est au rayon `h`.
    pub fn carve(&self, dir: Vec3, h: f32) -> Carve {
        let mut out = Carve { ground: h, water: None, width: 0.0 };
        let Some(list) = self.grid.get(&self.key(dir)) else { return out };
        let reach = REACH_VOXELS * self.voxel;
        for &id in list {
            if id & LAKE_BIT != 0 {
                let l = &self.lakes[(id & !LAKE_BIT) as usize];
                if (dir - l.center).length() < l.reach && h < l.level - 0.1 * self.voxel {
                    out.water = Some(out.water.map_or(l.level, |w| w.max(l.level)));
                    out.width = out.width.max(l.width);
                }
                continue;
            }
            let (a, b) = (self.nodes[id as usize], self.nodes[id as usize + 1]);
            let ab = b.dir - a.dir;
            let len2 = ab.length_squared().max(1e-12);
            let t = ((dir - a.dir).dot(ab) / len2).clamp(0.0, 1.0);
            let p = a.dir + ab * t;
            let d = (dir - p).length() * self.radius;
            let width = a.width + (b.width - a.width) * t;
            let hw = width * 0.5;
            if d >= hw + reach {
                continue;
            }
            let bed = a.bed + (b.bed - a.bed) * t;
            if d < hw {
                let f = d / hw.max(1e-3);
                let depth = (width * 0.08).clamp(1.0 * self.voxel, 5.0 * self.voxel);
                out.ground = out.ground.min(bed - depth * (1.0 - f * f));
                out.water = Some(out.water.map_or(bed, |w| w.max(bed)));
                out.width = out.width.max(width);
            } else {
                out.ground = out.ground.min(bed + (d - hw) * BANK_SLOPE);
            }
        }
        out
    }

    /// Vu de l'espace (maillage lointain, cases de `cell` unités) : la surface de l'eau si une grande rivière
    /// ou un lac passe assez près du centre de la case pour la colorer.
    pub fn from_space(&self, dir: Vec3, h: f32, cell: f32) -> Option<f32> {
        let list = self.grid.get(&self.key(dir))?;
        let min_width = (SPACE_WIDTH_VOXELS * self.voxel).max(0.15 * cell);
        let detect = (0.5 * cell).min(REACH_VOXELS * self.voxel);
        let mut water: Option<f32> = None;
        for &id in list {
            if id & LAKE_BIT != 0 {
                let l = &self.lakes[(id & !LAKE_BIT) as usize];
                if l.reach * self.radius * 2.0 >= 0.5 * cell && (dir - l.center).length() < l.reach && h < l.level {
                    water = Some(water.map_or(l.level, |w| w.max(l.level)));
                }
                continue;
            }
            let (a, b) = (self.nodes[id as usize], self.nodes[id as usize + 1]);
            let width = a.width.max(b.width);
            if width < min_width {
                continue;
            }
            let ab = b.dir - a.dir;
            let len2 = ab.length_squared().max(1e-12);
            let t = ((dir - a.dir).dot(ab) / len2).clamp(0.0, 1.0);
            let p = a.dir + ab * t;
            let d = (dir - p).length() * self.radius;
            if d < (width * 0.5).max(detect) {
                let bed = a.bed + (b.bed - a.bed) * t;
                water = Some(water.map_or(bed, |w| w.max(bed)));
            }
        }
        water
    }

    /// Le point de rivière le plus proche de `dir` parmi celles d'au moins `min_width` unités de large (direction,
    /// largeur en unités), s'il y en a.
    pub fn nearest(&self, dir: Vec3, min_width: f32) -> Option<(Vec3, f32)> {
        self.nodes.iter().filter(|n| n.width >= min_width).min_by(|a, b| (a.dir - dir).length_squared().total_cmp(&(b.dir - dir).length_squared())).map(|n| (n.dir, n.width))
    }

    /// Les cascades (un pas du lit qui tombe d'au moins 3 voxels) à moins de `reach` (angle) de `dir` : direction et
    /// hauteur de la chute (unités).
    pub fn waterfalls(&self, dir: Vec3, reach: f32) -> Vec<(Vec3, f32)> {
        let mut out = Vec::new();
        for r in &self.rivers {
            for i in r.start..r.end.saturating_sub(1) {
                let (a, b) = (self.nodes[i as usize], self.nodes[i as usize + 1]);
                let drop = a.bed - b.bed;
                if drop >= 3.0 * self.voxel && (a.dir - dir).length() < reach {
                    out.push((a.dir, drop));
                }
            }
        }
        out
    }

    /// Le lac le plus proche de `dir`.
    pub fn nearest_lake(&self, dir: Vec3) -> Option<&Lake> {
        self.lakes.iter().min_by(|a, b| (a.center - dir).length_squared().total_cmp(&(b.center - dir).length_squared()))
    }

    /// Nombre de rivières (sources), de lacs, longueur totale (unités).
    pub fn stats(&self) -> (usize, usize, f32) {
        // Une source : un tronçon dans lequel aucun autre ne se continue par son premier nœud
        let mut continued = vec![false; self.rivers.len()];
        for r in &self.rivers {
            if let Some((r2, j)) = r.next {
                if self.rivers.get(r2 as usize).is_some_and(|x| x.start == j) {
                    continued[r2 as usize] = true;
                }
            }
        }
        (continued.iter().filter(|c| !**c).count(), self.lakes.len(), self.length)
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    #[cfg(test)]
    fn rivers_beds(&self) -> impl Iterator<Item = (Vec<f32>, bool, bool)> + '_ {
        self.rivers.iter().map(|r| ((r.start..r.end).map(|i| self.nodes[i as usize].bed).collect(), r.mouth, r.next.is_some()))
    }

    #[cfg(test)]
    fn node_dirs(&self) -> impl Iterator<Item = (Vec3, f32)> + '_ {
        self.nodes.iter().map(|n| (n.dir, n.width))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::tests::earth_like;

    fn net() -> Arc<RiverNet> {
        let t = Terrain::new(earth_like());
        t.rivers.clone().expect("une planète tempérée avec de l'eau a des rivières")
    }

    /// D1 : le lit d'une rivière ne remonte jamais, et chaque rivière finit quelque part (mer, confluent, lac).
    #[test]
    fn rivers_only_go_down() {
        let n = net();
        let mut count = 0;
        let (mut mouths, mut joins) = (0, 0);
        for (beds, mouth, next) in n.rivers_beds() {
            count += 1;
            assert!(beds.len() >= 2, "une rivière a au moins deux nœuds");
            for w in beds.windows(2) {
                assert!(w[1] <= w[0] + 1e-4, "le lit remonte : {} -> {}", w[0], w[1]);
            }
            mouths += mouth as usize;
            joins += next as usize;
        }
        let (sources, lakes, length) = n.stats();
        println!("{count} tronçons, {sources} sources, {mouths} embouchures, {joins} suites, {lakes} lacs, {length:.0} unités");
        assert!(count >= 20, "trop peu de rivières : {count}");
        assert!(mouths + joins + lakes >= count / 2, "trop de rivières qui finissent nulle part");
        assert!(mouths > 0, "aucune rivière n'atteint la mer");
    }

    /// Une colonne du terrain dans un lit est de l'eau au-dessus d'un sol creusé.
    #[test]
    fn a_river_bed_holds_water_in_a_channel() {
        let t = Terrain::new(earth_like());
        let n = t.rivers.clone().unwrap();
        let v = t.voxel();
        let mut checked = 0;
        for (dir, width) in n.node_dirs().step_by(37).take(200) {
            let raw = t.ground_radius(dir);
            let c = t.base_column(dir, v);
            if c.kind == crate::planet::VoxelType::Water {
                assert!(c.bed <= raw + 0.5 * v, "le lit n'est pas creusé : fond {} sol {}", c.bed, raw);
                assert!(c.top > c.bed, "pas d'eau au-dessus du lit");
                checked += 1;
            }
            let _ = width;
        }
        assert!(checked > 50, "trop peu de colonnes d'eau sur les lits : {checked}");
    }

    /// Sans air ni liquide : aucune rivière ; même graine : même réseau.
    #[test]
    fn dry_worlds_have_no_rivers_and_the_network_is_deterministic() {
        let dry = Terrain::new(BodyParams { airless: true, atmosphere: false, ..earth_like() });
        assert!(dry.rivers.is_none());
        let a = net();
        let b = net();
        assert!(Arc::ptr_eq(&a, &b), "le réseau est partagé");
        assert_eq!(a.nodes.len(), RiverNet::build(&Terrain::new(earth_like())).nodes.len());
    }

    /// Temps de tracé (`cargo test --release bench_rivers -- --ignored --nocapture`).
    #[test]
    #[ignore]
    fn bench_rivers() {
        let t = Terrain::new(earth_like());
        let start = std::time::Instant::now();
        let n = RiverNet::build(&t);
        let ms = start.elapsed().as_secs_f64() * 1000.0;
        let (sources, lakes, length) = n.stats();
        let entries: usize = n.grid.values().map(Vec::len).sum();
        println!("BENCH rivières : {ms:.0} ms, {} nœuds, {sources} sources, {lakes} lacs, {length:.0} unités, {} cellules, {entries} entrées", n.nodes.len(), n.grid.len());
    }
}
