//! Faune visible (0.14 D4) : les créatures des mondes à vie complexe (`planetgen::life::Fauna`).
//!
//! Tout est f(graine, cellule, horloge) (règle 9) : les **espèces** d'un astre sortent de sa graine (famille de
//! l'éditeur, couleurs, taille, façon de se déplacer, jour ou nuit) ; les **troupeaux** sont hachés par cellule
//! d'environ `CELL_VOXELS` sur la sphère-cube (un troupeau au plus par cellule, là où le sol s'y prête : terre
//! ferme pour les marcheurs, mer pour les nageurs, n'importe où pour les volants) ; la **position** de chaque
//! bête à un instant ne dépend que de la graine du troupeau et de `WorldClock::secs` (le troupeau dérive
//! lentement, chaque bête tourne autour de lui) : deux joueurs voient le même troupeau au même endroit. Seule la
//! **fuite** devant le joueur est locale (un écart qui s'efface quand il s'éloigne).
//!
//! Les créatures sont des modèles de l'éditeur (`ModelKey::Fauna`, `build_model` : rig d'une famille de
//! `assets/editeur/races/`, peint aux couleurs de l'espèce) animés par les rigs du jeu (`models::Rig` : trot,
//! vol, ondulation...). Taille réelle (`Fauna::max_size_m`) à l'échelle k = 16 (le marcheur fait 2 voxels pour
//! 1,8 m). Les marcheurs sont posés sur le sol, les nageurs juste sous la surface de la mer (jamais ailleurs),
//! les volants planent entre 25 et 60 voxels. Diurnes ou nocturnes (`Fauna::nocturnal` : tout le monde la
//! nuit ; sinon une espèce sur trois). Trait « océan bioluminescent » réel : des méduses lumineuses la nuit.

use bevy::prelude::*;
use std::collections::HashMap;

use crate::editeur::format::{Material, Model, PaletteEntry};
use crate::editeur::motion::Library;
use crate::models::{Fit, ModelKey, Rig};
use crate::planet::{MoonId, MoonRoot, PlanetId, PlanetRoot};
use crate::planetgen::seeds::splitmix64;
use crate::surface::{Surface, SurfaceControl};
use crate::terrain::{dir_to_face, face_dir, BodyParams, Terrain};
use crate::ui::TargetKind;

/// Côté d'une cellule de troupeaux, en voxels.
pub const CELL_VOXELS: f32 = 600.0;
/// Distance (voxels) jusqu'où les troupeaux sont affichés, et bêtes affichées au plus.
const SHOW_VOXELS: f32 = 1400.0;
const MAX_CREATURES: usize = 64;
/// Le marcheur du joueur : 2 voxels pour 1,8 m.
const VOXELS_PER_M: f32 = 2.0 / 1.8;
/// Distance (voxels) sous laquelle une bête fuit le joueur.
const FLEE_VOXELS: f32 = 40.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Move {
    Walk,
    Fly,
    Swim,
}

/// Une espèce : famille de l'éditeur, couleurs, taille, déplacement.
#[derive(Clone, Debug, PartialEq)]
pub struct Species {
    pub name: String,
    pub family: &'static str,
    pub moving: Move,
    /// Animation en mouvement et au repos.
    pub anim: &'static str,
    pub idle: &'static str,
    pub size_m: f32,
    pub color_a: [u8; 3],
    pub color_b: [u8; 3],
    pub nocturnal: bool,
    /// Méduse lumineuse (trait « océan bioluminescent »).
    pub glowing: bool,
    /// Bêtes par troupeau (min, max).
    pub herd: (u32, u32),
    /// Vitesse de déplacement (voxels / s).
    pub speed: f32,
}

/// Familles de l'éditeur par façon de se déplacer : (race, animation de marche, animation de repos, nom).
const WALKERS: [(&str, &str, &str, &str); 4] = [("quadrupede", "trot", "repos", "quadrupede"), ("insectoide", "marche_araignee", "repos", "insecte"), ("reptilien", "marcher", "repos", "reptile"), ("slime", "bond", "slime_tremble", "slime")];
const FLYERS: [(&str, &str, &str, &str); 2] = [("harpie", "voler", "planer", "oiseau"), ("aile_celeste", "voler", "planer", "volant")];
const SWIMMERS: [(&str, &str, &str, &str); 2] = [("sirene", "ondulation", "ondulation", "poisson"), ("cephalopode", "nager", "flotter", "cephalopode")];

/// Toutes les familles possibles, dans l'ordre du code des modèles (`ModelKey::Fauna`).
pub const FAMILIES: [&str; 8] = ["quadrupede", "insectoide", "reptilien", "slime", "harpie", "aile_celeste", "sirene", "cephalopode"];

fn unit(h: u64) -> f32 {
    (h >> 40) as f32 / (1u64 << 24) as f32
}

/// Ce que la vie de l'astre permet (copié de `planetgen::life::Fauna`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FaunaParams {
    pub max_size_m: f32,
    pub fly: bool,
    pub swim: bool,
    pub nocturnal: bool,
    pub glow_ocean: bool,
}

impl FaunaParams {
    pub fn of(p: &crate::settings::PlanetConfig) -> Option<Self> {
        let f = p.life.fauna.as_ref()?;
        Some(Self {
            max_size_m: f.max_size_m,
            fly: f.locomotion.iter().any(|l| l == "vol"),
            swim: f.locomotion.iter().any(|l| l == "nage"),
            nocturnal: f.nocturnal,
            glow_ocean: p.traits.iter().any(|t| t.name == "ocean bioluminescent"),
        })
    }
}

/// Les espèces d'un astre (3 à 7, plus les méduses) : f(graine).
pub fn species(seed: u32, f: &FaunaParams) -> Vec<Species> {
    let mut out = Vec::new();
    let mut x = (seed as u64 ^ 0xFA_0A_0001).wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    let mut rnd = || {
        x = splitmix64(x);
        unit(x)
    };
    let n = 3 + (rnd() * 5.0) as usize;
    for k in 0..n {
        let roll = rnd();
        let moving = if f.fly && roll < 0.3 {
            Move::Fly
        } else if f.swim && roll < 0.55 {
            Move::Swim
        } else {
            Move::Walk
        };
        let table: &[(&str, &str, &str, &str)] = match moving {
            Move::Walk => &WALKERS,
            Move::Fly => &FLYERS,
            Move::Swim => &SWIMMERS,
        };
        let (family, anim, idle, base) = table[(rnd() * table.len() as f32) as usize % table.len()];
        // Tailles : la plus grande espèce approche `max_size_m`, les autres sont bien plus petites
        let size_m = (f.max_size_m * (0.08 + 0.92 * rnd().powi(2))).clamp(0.4, 40.0);
        let hue = |r: f32, s: f32, v: f32| -> [u8; 3] {
            let i = (r * 6.0).floor();
            let fr = r * 6.0 - i;
            let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * fr), v * (1.0 - s * (1.0 - fr)));
            let (r, g, b) = match i as i32 % 6 {
                0 => (v, t, p),
                1 => (q, v, p),
                2 => (p, v, t),
                3 => (p, q, v),
                4 => (t, p, v),
                _ => (v, p, q),
            };
            [(r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8]
        };
        let h = rnd();
        let color_a = hue(h, 0.25 + 0.45 * rnd(), 0.35 + 0.45 * rnd());
        let color_b = hue((h + 0.08 + 0.2 * rnd()).fract(), 0.2 + 0.3 * rnd(), 0.6 + 0.35 * rnd());
        let nocturnal = f.nocturnal || rnd() < 0.33;
        let herd = match moving {
            Move::Walk => (2, 8),
            Move::Fly => (1, 4),
            Move::Swim => (2, 6),
        };
        let speed = match moving {
            Move::Walk => 1.5 + 2.5 * rnd(),
            Move::Fly => 4.0 + 4.0 * rnd(),
            Move::Swim => 2.0 + 2.0 * rnd(),
        } * (1.0 + size_m * 0.05).min(2.0);
        out.push(Species { name: format!("{base} {}", k + 1), family, moving, anim, idle, size_m, color_a, color_b, nocturnal, glowing: false, herd, speed });
    }
    if f.glow_ocean && f.swim {
        out.push(Species { name: "meduse lumineuse".into(), family: "cephalopode", moving: Move::Swim, anim: "nager", idle: "flotter", size_m: 1.2, color_a: [120, 230, 255], color_b: [60, 200, 220], nocturnal: true, glowing: true, herd: (5, 10), speed: 0.8 });
    }
    out
}

/// Code du modèle d'une espèce (`ModelKey::Fauna`) : famille, deux couleurs, lumineux.
pub fn model_code(s: &Species) -> u64 {
    let fam = FAMILIES.iter().position(|f| *f == s.family).unwrap_or(0) as u64;
    let pack = |c: [u8; 3]| (c[0] as u64) << 16 | (c[1] as u64) << 8 | c[2] as u64;
    fam | pack(s.color_a) << 4 | pack(s.color_b) << 28 | (s.glowing as u64) << 52
}

/// Le modèle d'une créature d'après son code : le rig de la famille, peint.
pub fn build_model(code: u64, lib: &Library) -> Option<Model> {
    let family = FAMILIES[(code & 15) as usize % FAMILIES.len()];
    let unpack = |v: u64| [((v >> 16) & 255) as u8, ((v >> 8) & 255) as u8, (v & 255) as u8];
    let a = unpack(code >> 4);
    let b = unpack(code >> 28);
    let glowing = (code >> 52) & 1 == 1;
    let race = lib.races.iter().find(|r| r.id == family)?;
    let mut options = race.default_options();
    // Un insecte à ailes, un reptile digitigrade : d'après les couleurs (donc la graine)
    for (i, o) in race.options.iter().enumerate() {
        if matches!(o.id.as_str(), "ailes" | "digitigrade" | "cornes" | "criniere" | "huit") {
            options[i] = (a[i % 3] as u32 + b[(i + 1) % 3] as u32) % 3 == 0;
        }
    }
    let mut m = race.build("creature", &options);
    let mat = if glowing { Material::Lumineuse } else { Material::Mate };
    let dark = [28, 26, 30];
    let darker = |c: [u8; 3]| c.map(|x| (x as f32 * 0.7) as u8);
    let color_of = |bone: &str| -> PaletteEntry {
        let bn = bone.trim_end_matches("_g").trim_end_matches("_d");
        let starts = |p: &[&str]| p.iter().any(|x| bn.starts_with(x));
        if starts(&["pied", "orteils", "main", "mandibule", "bec", "machoire"]) {
            PaletteEntry { rgb: dark, material: Material::Mate }
        } else if starts(&["patte", "cuisse", "tibia", "bras", "avant_bras", "tav", "tar", "queue", "antenne", "cou"]) {
            PaletteEntry { rgb: darker(a), material: mat }
        } else if starts(&["aile", "plumes", "pointe", "nageoire", "dorsale", "crete", "criniere", "oreille", "museau"]) {
            PaletteEntry { rgb: b, material: mat }
        } else {
            PaletteEntry { rgb: a, material: mat }
        }
    };
    let bones: Vec<String> = m.zones.iter().map(crate::editeur::motion::bone).collect();
    let mut index = [0u8; 256];
    for (z, bn) in bones.iter().enumerate() {
        index[z + 1] = m.color_index(color_of(bn)).unwrap_or(1);
    }
    index[0] = m.color_index(PaletteEntry { rgb: a, material: mat }).unwrap_or(1);
    let cells: Vec<(IVec3, u8)> = m.voxels.iter().collect();
    for (p, _) in cells {
        let z = m.zone_map.get(p);
        m.voxels.set(p, index[z as usize]);
    }
    // Les yeux
    if let Some(t) = bones.iter().position(|bn| bn == "tete") {
        let (mut lo, mut hi) = (IVec3::MAX, IVec3::MIN);
        for (p, z) in m.zone_map.iter() {
            if z as usize == t + 1 {
                lo = lo.min(p);
                hi = hi.max(p);
            }
        }
        if lo.x <= hi.x {
            let eye = m.color_index(PaletteEntry { rgb: if glowing { [255, 255, 200] } else { dark }, material: if glowing { Material::Lumineuse } else { Material::Mate } }).unwrap_or(1);
            let y = lo.y + (hi.y - lo.y) * 3 / 5;
            let cx = (lo.x + hi.x) / 2;
            for x in [cx - 1, hi.x + lo.x - (cx - 1)] {
                let p = IVec3::new(x, y, hi.z);
                if m.voxels.get(p) != 0 {
                    m.voxels.set(p, eye);
                }
            }
        }
    }
    m.compact_palette();
    Some(m)
}

// ─────────────────────────────────────────────────────────────────────────
//  Troupeaux
// ─────────────────────────────────────────────────────────────────────────

/// Un troupeau : espèce, cellule, point d'attache, graine, nombre de bêtes.
#[derive(Clone, Debug, PartialEq)]
pub struct Herd {
    pub species: usize,
    pub cell: (u8, i64, i64),
    pub home: Vec3,
    pub seed: u64,
    pub count: u32,
    /// Rayon de la dérive (angle) et de la ronde de chaque bête (angle).
    drift: f32,
    ring: f32,
}

/// Cellules par côté de face.
pub fn cells(p: &BodyParams) -> i64 {
    ((std::f32::consts::FRAC_PI_2 * p.radius) / (CELL_VOXELS * p.layout().voxel)).round().max(1.0) as i64
}

pub fn cell_of(p: &BodyParams, dir: Vec3) -> (u8, i64, i64) {
    let n = cells(p);
    let (face, s, t) = dir_to_face(dir);
    let x = (((s + 1.0) * 0.5 * n as f32).floor() as i64).clamp(0, n - 1);
    let y = (((t + 1.0) * 0.5 * n as f32).floor() as i64).clamp(0, n - 1);
    (face, x, y)
}

/// Le troupeau de la cellule, s'il y en a un. `sea` : la direction est en mer (nageurs) ou non (marcheurs).
pub fn cell_herd(p: &BodyParams, all: &[Species], (face, x, y): (u8, i64, i64), sea: &dyn Fn(Vec3) -> bool) -> Option<Herd> {
    if all.is_empty() {
        return None;
    }
    let n = cells(p) as f32;
    let mut h = splitmix64(splitmix64((p.seed as u64) ^ ((face as u64) << 56) ^ 0xFA_0E) ^ (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F));
    let mut next = || {
        h = splitmix64(h);
        unit(h)
    };
    // Un troupeau dans une cellule sur deux environ
    if next() > 0.55 {
        return None;
    }
    let s = -1.0 + 2.0 * (x as f32 + 0.25 + 0.5 * next()) / n;
    let t = -1.0 + 2.0 * (y as f32 + 0.25 + 0.5 * next()) / n;
    let home = face_dir(face, s, t);
    let in_sea = sea(home);
    // L'espèce : une qui peut vivre là
    let candidates: Vec<usize> = all.iter().enumerate().filter(|(_, sp)| match sp.moving {
        Move::Walk => !in_sea,
        Move::Swim => in_sea,
        Move::Fly => true,
    }).map(|(i, _)| i).collect();
    if candidates.is_empty() {
        return None;
    }
    let species = candidates[(next() * candidates.len() as f32) as usize % candidates.len()];
    let sp = &all[species];
    let count = sp.herd.0 + (next() * (sp.herd.1 - sp.herd.0 + 1) as f32) as u32;
    let voxel = p.layout().voxel;
    let drift = (CELL_VOXELS * 0.2) * voxel / p.radius.max(1.0);
    let ring = (6.0 + 10.0 * next() + sp.size_m * VOXELS_PER_M) * voxel / p.radius.max(1.0);
    Some(Herd { species, cell: (face, x, y), home, seed: splitmix64(h), count: count.min(12), drift, ring })
}

impl Herd {
    /// Direction (repère de l'astre) et cap (tangent, unitaire) de la bête `i` à l'instant `t` (secondes de jeu).
    pub fn member(&self, i: u32, t: f64, speed_voxels: f32, radius: f32, voxel: f32) -> (Vec3, Vec3) {
        let at = |t: f64| {
            let r = |k: u64| unit(splitmix64(self.seed ^ k)) as f64;
            let up = self.home;
            let e1 = up.any_orthonormal_vector();
            let e2 = up.cross(e1);
            // Dérive lente du troupeau (Lissajous), ronde de chaque bête, à la vitesse de l'espèce
            let w = (speed_voxels * voxel / radius.max(1.0)) as f64;
            let c = (t * w * 0.12 / self.drift.max(1e-6) as f64 + r(1) * 6.28).sin() as f32 * self.drift;
            let d = (t * w * 0.09 / self.drift.max(1e-6) as f64 + r(2) * 6.28).sin() as f32 * self.drift;
            let ri = |k: u64| unit(splitmix64(self.seed ^ (i as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ k)) as f64;
            let rr = self.ring * (0.3 + 0.7 * ri(1) as f32);
            let ang = t * w * 0.5 / rr.max(1e-6) as f64 + ri(2) * 6.28;
            let (ox, oy) = (c + rr * ang.cos() as f32, d + rr * ang.sin() as f32);
            (up + e1 * ox + e2 * oy).normalize()
        };
        let now = at(t);
        let next = at(t + 0.25);
        let heading = (next - now - now * (next - now).dot(now)).normalize_or(now.any_orthonormal_vector());
        (now, heading)
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  Affichage
// ─────────────────────────────────────────────────────────────────────────

pub struct FaunaPlugin;

impl Plugin for FaunaPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FaunaState>().add_event::<FaunaCommand>().add_systems(Update, (drive_fauna, go_fauna).after(SurfaceControl));
    }
}

/// Une bête affichée : son troupeau et son rang.
#[derive(Component)]
pub struct Creature {
    pub cell: (u8, i64, i64),
    pub index: u32,
    /// Écart de fuite (repère de l'astre, unités), local.
    flee: Vec3,
}

/// Les troupeaux de l'astre où l'on est (cache par cellule) et les bêtes à l'écran.
#[derive(Resource, Default)]
pub struct FaunaState {
    body: Option<TargetKind>,
    pub species: Vec<Species>,
    params: Option<FaunaParams>,
    cells: HashMap<(u8, i64, i64), Option<Herd>>,
    shown: HashMap<((u8, i64, i64), u32), Entity>,
}

impl FaunaState {
    /// Troupeau de la cellule (calculé une fois).
    pub fn herd(&mut self, p: &BodyParams, t: &Terrain, key: (u8, i64, i64)) -> Option<Herd> {
        if let Some(h) = self.cells.get(&key) {
            return h.clone();
        }
        let sea = |d: Vec3| t.vent_ground(d).is_liquid();
        let h = cell_herd(p, &self.species, key, &sea);
        if self.cells.len() > 4096 {
            self.cells.clear();
        }
        self.cells.insert(key, h.clone());
        h
    }
}

/// Position d'une bête : au sol, sous la surface de la mer ou en l'air selon l'espèce ; `None` si elle n'a pas
/// sa place ici (un nageur sur la terre ferme).
fn place(t: &Terrain, sp: &Species, dir: Vec3, index: u32, time: f64) -> Option<f32> {
    let v = t.voxel();
    match sp.moving {
        Move::Walk => {
            let g = t.ground(dir);
            if g.is_sea() {
                return None;
            }
            Some(g.top)
        }
        Move::Swim => {
            let g = t.ground(dir);
            // Jamais dans une flaque : au moins un voxel d'eau
            if !g.is_sea() || g.top - g.bed < v {
                return None;
            }
            let depth = (1.5 + 1.5 * ((time * 0.3 + index as f64).sin() as f32 + 1.0)) * v * (0.5 + sp.size_m * VOXELS_PER_M * 0.5).max(1.0);
            Some((g.top - depth).max(g.bed).min(g.top - 0.5 * v))
        }
        Move::Fly => {
            let g = t.ground(dir).top.max(t.sea_surface(dir).map_or(0.0, |(r, _)| r));
            Some(g + (25.0 + 35.0 * (0.5 + 0.5 * ((time * 0.17 + index as f64 * 1.3).sin() as f32))) * v)
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn drive_fauna(
    mut commands: Commands,
    clock: Res<crate::world_clock::WorldClock>,
    time: Res<Time>,
    settings: Res<crate::settings::GameSettings>,
    surface: Res<Surface>,
    mut state: ResMut<FaunaState>,
    planets: Query<(Entity, &Transform, &PlanetId), With<PlanetRoot>>,
    moons: Query<(Entity, &Transform, &MoonId), With<MoonRoot>>,
    mut creatures: Query<(Entity, &mut Creature, &mut Transform, &mut Visibility, &Children), (Without<PlanetRoot>, Without<MoonRoot>)>,
    mut rigs: Query<&mut Rig>,
) {
    let body = surface.body().filter(|_| surface.active());
    if state.body != body {
        state.body = body;
        state.cells.clear();
        for (_, e) in state.shown.drain() {
            if let Some(ec) = commands.get_entity(e) {
                ec.despawn_recursive();
            }
        }
        state.species.clear();
        state.params = None;
        if let Some(TargetKind::Planet(id)) = body {
            if let Some(pc) = settings.systems.get(id / 1000).and_then(|s| s.planets().get(id % 1000).cloned()) {
                state.params = FaunaParams::of(&pc);
                if let Some(f) = state.params {
                    state.species = species(pc.seed, &f);
                }
            }
        }
    }
    let root = match body {
        Some(TargetKind::Planet(id)) => planets.iter().find(|(_, _, p)| p.0 == id).map(|(e, t, _)| (e, *t)),
        Some(TargetKind::Moon(pid, mi)) => moons.iter().find(|(_, _, m)| m.planet_idx == pid && m.moon_idx == mi).map(|(e, t, _)| (e, *t)),
        _ => None,
    };
    let (Some((root_e, _)), Some(t), Some(here)) = (root, surface.terrain(), surface.local_point()) else {
        for (_, e) in state.shown.drain() {
            if let Some(ec) = commands.get_entity(e) {
                ec.despawn_recursive();
            }
        }
        return;
    };
    if state.species.is_empty() {
        return;
    }
    let p = &t.params;
    let voxel = t.voxel();
    let now = clock.secs;
    let up = here.normalize_or(Vec3::Y);
    let day = surface.daylight();

    // Troupeaux à portée
    let reach = (SHOW_VOXELS / CELL_VOXELS).ceil() as i64;
    let (face, x, y) = cell_of(p, up);
    let n = cells(p);
    let mut wanted: Vec<((u8, i64, i64), u32, Herd)> = Vec::new();
    for cy in (y - reach).max(0)..=(y + reach).min(n - 1) {
        for cx in (x - reach).max(0)..=(x + reach).min(n - 1) {
            let key = (face, cx, cy);
            let Some(h) = state.herd(p, t, key) else { continue };
            if (h.home - up).length() * p.radius > SHOW_VOXELS * voxel {
                continue;
            }
            let sp = &state.species[h.species];
            // Diurnes le jour, nocturnes la nuit
            let awake = if sp.nocturnal { day < 0.35 } else { day > 0.2 };
            if !awake {
                continue;
            }
            for i in 0..h.count {
                wanted.push((key, i, h.clone()));
            }
        }
    }
    wanted.sort_by(|a, b| (a.2.home - up).length_squared().total_cmp(&(b.2.home - up).length_squared()));
    wanted.truncate(MAX_CREATURES);
    let keys: std::collections::HashSet<((u8, i64, i64), u32)> = wanted.iter().map(|(k, i, _)| (*k, *i)).collect();
    // Celles qui ne sont plus voulues disparaissent
    let gone: Vec<_> = state.shown.keys().filter(|k| !keys.contains(k)).copied().collect();
    for k in gone {
        if let Some(e) = state.shown.remove(&k) {
            if let Some(ec) = commands.get_entity(e) {
                ec.despawn_recursive();
            }
        }
    }
    // Les nouvelles apparaissent, les autres bougent
    let dt = time.delta_secs();
    for (key, i, h) in wanted {
        let sp = state.species[h.species].clone();
        let sp = &sp;
        let (dir, heading) = h.member(i, now, sp.speed, p.radius, voxel);
        let entity = match state.shown.get(&(key, i)) {
            Some(e) => *e,
            None => {
                let size_m = sp.size_m * (0.7 + 0.3 * unit(splitmix64(h.seed ^ (i as u64 + 7))));
                let height = size_m * VOXELS_PER_M * voxel;
                let e = commands
                    .spawn((Transform::IDENTITY, Visibility::Hidden, Creature { cell: key, index: i, flee: Vec3::ZERO }))
                    .with_child((Rig::new(ModelKey::Fauna(model_code(sp)), Fit::Character { height }, sp.anim), Transform::IDENTITY, Visibility::default()))
                    .id();
                commands.entity(root_e).add_child(e);
                state.shown.insert((key, i), e);
                e
            }
        };
        let Ok((_, mut c, mut tf, mut vis, children)) = creatures.get_mut(entity) else { continue };
        // Fuite : l'écart grandit tant que le joueur est près, s'efface ensuite (local)
        let pos0 = dir * p.radius;
        let away = (pos0 + c.flee - here).normalize_or_zero();
        let near = (pos0 + c.flee - here).length() < FLEE_VOXELS * voxel;
        if near && sp.moving != Move::Swim {
            c.flee += (away - dir * away.dot(dir)) * sp.speed * 2.0 * voxel * dt;
        } else {
            c.flee *= (1.0 - 0.3 * dt).max(0.0);
        }
        let dir = (dir * p.radius + c.flee).normalize();
        let Some(r) = place(t, sp, dir, i, now) else {
            *vis = Visibility::Hidden;
            continue;
        };
        let heading = if near { (away - dir * away.dot(dir)).normalize_or(heading) } else { heading };
        let right = dir.cross(heading).normalize_or(dir.any_orthonormal_vector());
        let rotation = Quat::from_mat3(&Mat3::from_cols(right, dir, -heading));
        *tf = Transform { translation: dir * r, rotation, scale: Vec3::ONE };
        if *vis != Visibility::Inherited {
            *vis = Visibility::Inherited;
        }
        // Au repos une partie du temps (f(horloge))
        let resting = sp.moving == Move::Walk && ((now * 0.05 + (h.seed % 97) as f64 + i as f64 * 0.7).sin() > 0.7);
        for child in children {
            if let Ok(mut rig) = rigs.get_mut(*child) {
                rig.play(if resting || (near && sp.moving == Move::Swim) { sp.idle } else { sp.anim });
            }
        }
    }
}

/// `/faune` : aller au troupeau le plus proche (en vol bas ou à pied).
#[derive(Event)]
pub struct FaunaCommand;

fn go_fauna(time: Res<Time>, clock: Res<crate::world_clock::WorldClock>, mut events: EventReader<FaunaCommand>, mut state: ResMut<FaunaState>, mut surface: ResMut<Surface>, mut net: ResMut<crate::net::Net>) {
    let now = time.elapsed_secs_f64();
    for _ in events.read() {
        let (Some(terrain), Some(here)) = (surface.terrain(), surface.local_point()) else {
            net.notify("Pas de faune : posez-vous ou volez bas sur une planete a vie complexe.", now);
            continue;
        };
        if state.species.is_empty() {
            net.notify("Pas de faune sur cet astre (il faut une vie complexe).", now);
            continue;
        }
        let p = terrain.params;
        let up = here.normalize_or(Vec3::Y);
        let (face, x, y) = cell_of(&p, up);
        let n = cells(&p);
        let mut best: Option<(f32, Herd)> = None;
        for cy in (y - 6).max(0)..=(y + 6).min(n - 1) {
            for cx in (x - 6).max(0)..=(x + 6).min(n - 1) {
                let Some(h) = state.herd(&p, terrain, (face, cx, cy)) else { continue };
                // Le plus proche, avec une préférence pour les grandes bêtes (plus faciles à voir)
                let d = (h.home - up).length() / (1.0 + state.species[h.species].size_m).sqrt();
                if best.as_ref().is_none_or(|(bd, _)| d < *bd) {
                    best = Some((d, h));
                }
            }
        }
        let Some((d, h)) = best else {
            net.notify("Aucun troupeau par ici.", now);
            continue;
        };
        let sp = state.species[h.species].clone();
        let v = terrain.voxel();
        let (dir, _) = h.member(0, clock.secs, sp.speed, p.radius, v);
        let east = Vec3::Y.cross(dir).normalize_or(Vec3::X);
        let _ = d;
        let msg = format!("Troupeau : {} ({} betes, {:.1} m, {}).", sp.name, h.count, sp.size_m, match sp.moving { Move::Walk => "marche", Move::Fly => "vol", Move::Swim => "nage" });
        let top = terrain.sea_surface(dir).map_or(terrain.ground(dir).top, |(r, _)| r);
        let side = (dir + east * (12.0 * v / p.radius)).normalize();
        if surface.teleport(side, -east, dir * (top + 16.0 * v)) {
            net.notify(&msg, now);
        } else {
            net.notify("Attendez la fin de l'atterrissage.", now);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> FaunaParams {
        FaunaParams { max_size_m: 12.0, fly: true, swim: true, nocturnal: false, glow_ocean: true }
    }

    /// Les espèces et les modèles : f(graine), toutes les familles se construisent et se peignent.
    #[test]
    fn species_and_models_follow_the_seed() {
        let a = species(42, &params());
        let b = species(42, &params());
        assert_eq!(a, b);
        assert!(a.len() >= 4 && a.len() <= 8, "{}", a.len());
        assert!(a.iter().any(|s| s.glowing), "méduses avec le trait");
        assert_ne!(species(43, &params()), a);
        let (lib, _) = Library::load(None);
        for family in FAMILIES {
            let sp = Species { name: family.into(), family, moving: Move::Walk, anim: "repos", idle: "repos", size_m: 2.0, color_a: [200, 90, 40], color_b: [240, 220, 120], nocturnal: false, glowing: false, herd: (1, 1), speed: 1.0 };
            let m = build_model(model_code(&sp), &lib).expect(family);
            assert!(m.voxels.iter().count() > 50, "{family}");
            assert!(!m.zones.is_empty(), "{family}");
        }
    }

    /// Les troupeaux : hachés (mêmes pour la même graine), nageurs en mer, marcheurs à terre ; une bête ne quitte
    /// jamais son troupeau de plus que sa ronde, et deux instants proches donnent des positions proches.
    #[test]
    fn herds_are_deterministic_and_stay_in_their_element() {
        let p = crate::terrain::tests::earth_like();
        let all = species(p.seed, &params());
        let sea = |d: Vec3| d.y > 0.0;
        let mut herds = Vec::new();
        for x in 0..40 {
            for y in 0..40 {
                if let Some(h) = cell_herd(&p, &all, (1, x, y), &sea) {
                    assert_eq!(Some(h.clone()), cell_herd(&p, &all, (1, x, y), &sea));
                    let sp = &all[h.species];
                    match sp.moving {
                        Move::Walk => assert!(!sea(h.home)),
                        Move::Swim => assert!(sea(h.home)),
                        Move::Fly => {}
                    }
                    herds.push(h);
                }
            }
        }
        assert!(herds.len() > 100, "{}", herds.len());
        let h = &herds[0];
        let (d0, head) = h.member(2, 1000.0, 3.0, p.radius, 0.5);
        let (d1, _) = h.member(2, 1000.1, 3.0, p.radius, 0.5);
        assert!((d0 - h.home).length() <= h.drift * 2.0 + h.ring + 1e-4);
        assert!((d1 - d0).length() * p.radius < 1.0);
        assert!(head.dot(d0).abs() < 1e-3, "le cap est tangent");
    }

    /// Un marcheur est posé sur le sol, un nageur sous la surface de la mer et jamais sur la terre.
    #[test]
    fn creatures_stand_on_the_ground_or_swim_in_the_sea() {
        let p = crate::terrain::tests::earth_like();
        let t = Terrain::new(p);
        let walker = Species { name: "w".into(), family: "quadrupede", moving: Move::Walk, anim: "trot", idle: "repos", size_m: 2.0, color_a: [1; 3], color_b: [2; 3], nocturnal: false, glowing: false, herd: (1, 1), speed: 1.0 };
        let fish = Species { moving: Move::Swim, ..walker.clone() };
        let (mut land, mut water) = (0, 0);
        for k in 0..200 {
            let a = k as f32 * 0.37;
            let dir = Vec3::new(a.cos() * (0.3 + 0.1 * (k as f32 * 0.11).sin()), (k as f32 * 0.05).sin(), a.sin()).normalize();
            let g = t.ground(dir);
            if g.is_sea() {
                assert!(place(&t, &walker, dir, 0, 5.0).is_none());
                if g.top - g.bed < t.voxel() {
                    assert!(place(&t, &fish, dir, 0, 5.0).is_none(), "pas de nageur dans une flaque");
                    continue;
                }
                let r = place(&t, &fish, dir, 0, 5.0).unwrap();
                assert!(r < g.top && r >= g.bed, "sous la surface, au-dessus du fond");
                water += 1;
            } else {
                assert!(place(&t, &fish, dir, 0, 5.0).is_none());
                assert!((place(&t, &walker, dir, 0, 5.0).unwrap() - g.top).abs() < 1e-3);
                land += 1;
            }
        }
        assert!(land > 10 && water > 10, "{land} {water}");
    }
}
