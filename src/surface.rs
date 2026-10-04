//! Atterrissage sur les planètes et les lunes, et exploration à pied.
//!
//! Entrée : atterrit sur l'astre ciblé, à l'endroit pointé par la souris, puis décolle.
//! Pendant le séjour, la caméra est à la première personne dans le repère de l'astre (qui continue
//! de suivre son orbite) et le terrain voxel est affiché par `terrain.rs`, à la place du maillage
//! lointain.

use std::collections::{HashMap, HashSet};

use bevy::ecs::system::SystemParam;
use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;
use bevy::tasks::{block_on, futures_lite::future, AsyncComputeTaskPool, Task};
use bevy::window::{CursorGrabMode, PrimaryWindow};

use crate::net::Net;
use crate::net_ui::NetPanel;
use crate::planet::{MoonId, MoonRoot, PlanetId, PlanetRoot, StarId, StarRoot};
use crate::settings::GameSettings;
use crate::ship::Ship;
use crate::terrain::{BodyParams, Terrain, TileKey, build_tile_mesh_with};
use crate::decor::{tile_decor, DecorAssets, DecorInstance};
use crate::ui::{CameraTarget, MenuState, TargetKind};
use crate::{CameraController, ZoomLevel};

/// Couleur du ciel dans l'espace (celle de `setup_scene`).
pub(crate) const SPACE_SKY: Color = Color::srgb(0.005, 0.005, 0.02);

/// La lumière vient uniquement de l'étoile (lumière ponctuelle réelle : sa position, sa couleur et
/// sa chute en 1/d²), jamais d'un « soleil » ajouté : une planète lointaine reçoit moins de lumière,
/// la nuit tombe d'elle-même. Seule la lumière diffuse du ciel est ajoutée, sous une atmosphère.
const AMBIENT_SPACE: f32 = 300.0;
/// Lumière diffuse du ciel en plein jour sous une atmosphère.
const AMBIENT_DAY: f32 = 1_500.0;
/// Teinte de la lumière diffuse du ciel (plus claire que celle de l'espace).
const AMBIENT_SPACE_TINT: Color = Color::srgb(0.25, 0.25, 0.35);
/// La nuit au sol : presque rien (lueur des étoiles), plus le clair de lune.
const AMBIENT_NIGHT: f32 = 35.0;
/// Clair de lune maximal (pleine lune très proche).
const MOONLIGHT_MAX: f32 = 260.0;
/// Teinte du clair de lune (lumière de l'étoile renvoyée, un peu bleutée à l'œil).
const MOONLIGHT_TINT: Color = Color::srgb(0.55, 0.62, 0.8);
/// Brume la nuit : on voit les lunes, les planètes et les étoiles à travers l'air.
const NIGHT_HAZE: f32 = 0.35;
/// Brume le jour : les lunes restent pâles, les étoiles disparaissent.
const DAY_HAZE: f32 = 0.93;
/// Éclat de la galaxie gardé la nuit au sol (le jour : `DIM_FLOOR`).
const NIGHT_GALAXY: f32 = 0.75;

/// Lampe du marcheur et phares du vaisseau (allumés quand il fait sombre).
pub const LAMP_KEY: KeyCode = KeyCode::KeyN;

/// Tuiles en construction en même temps (hors du fil principal) : deux par cœur (0.13 E3).
fn max_tile_tasks() -> usize {
    std::thread::available_parallelism().map_or(8, |n| n.get()).clamp(4, 32) * 2
}

/// Temps (ms) accordé par image à la pose des tuiles terminées (envoi des maillages, décor) : au
/// delà, la suite attend l'image suivante (pas de pic à l'atterrissage, 0.13 E3).
const TILE_BUDGET_MS: f64 = 3.0;
/// Au-delà, les tuiles inutilisées depuis longtemps sont supprimées.
const MAX_TILES: usize = 520;
const TILE_KEEP_SECS: f64 = 8.0;

/// Touche pour sortir du vaisseau (atterrir) et y rentrer (décoller) : une touche que le reste
/// du jeu n'utilise pas (C, E, F, G, L, M, N, P et T servent déjà).
pub const ENTER_SHIP_KEY: KeyCode = KeyCode::KeyV;

/// Tests (développement) : `SPACESPORE_TEST_LAND=<s>[,<s>...]` appuie sur V à ces instants (atterrir,
/// puis remonter dans le vaisseau...).
fn test_land(now: f64) -> bool {
    static DONE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let Ok(list) = std::env::var("SPACESPORE_TEST_LAND") else { return false };
    let times: Vec<f64> = list.split(',').filter_map(|s| s.trim().parse().ok()).collect();
    let k = DONE.load(std::sync::atomic::Ordering::Relaxed);
    let due = times.get(k).is_some_and(|at| now > *at);
    if due {
        DONE.store(k + 1, std::sync::atomic::Ordering::Relaxed);
    }
    due
}

/// À pied : vue à la troisième personne (on voit son personnage, E7) ou à la première.
pub const VIEW_KEY: KeyCode = KeyCode::F5;

/// Dimensions du vaisseau du joueur (son modèle, E7) : longueur à l'échelle 1 (`models::icon_length`),
/// demi-hauteur à l'échelle 1, plus grande dimension en voxels (vraie taille : 4 voxels = 1 bloc).
#[derive(Clone, Copy, Debug)]
pub struct ShipDims {
    pub icon_len: f32,
    pub half_h: f32,
    pub voxels: f32,
}

impl Default for ShipDims {
    fn default() -> Self {
        Self { icon_len: 5.0, half_h: 0.3, voxels: 64.0 }
    }
}

impl ShipDims {
    /// Longueur réelle du vaisseau au plus (voxels du terrain les plus grands).
    pub fn max_length(&self) -> f32 {
        self.voxels / 4.0 * crate::terrain::MAX_VOXEL
    }

    /// Échelle du vaisseau à sa vraie taille, sur un astre aux voxels de `voxel` unités.
    pub fn real_scale(&self, voxel: f32) -> f32 {
        self.voxels / 4.0 * voxel / self.icon_len
    }
}

pub struct SurfacePlugin;

impl Plugin for SurfacePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Surface>()
            .init_resource::<GalaxyDim>()
            .init_resource::<TileStore>()
            .add_event::<OverhangCommand>()
            .init_resource::<VoxelsChanged>()
            .init_gizmo_group::<IndicatorGizmos>()
            .add_systems(Update, fade_indicators.after(SurfaceControl))
            .add_event::<CaveCommand>()
            .init_resource::<NearestCave>()
            .add_systems(Update, (go_cave.before(SurfaceControl), find_nearest_cave))
            .add_systems(Startup, (setup_hud, spawn_lamps, spawn_suns))
            .add_systems(Update, go_overhang.before(SurfaceControl))
            .add_systems(Update, update_galaxy_dim)
            .add_systems(Update, remember_walker_cam)
            .add_systems(Update, near_plane)
            .init_resource::<TileStats>()
            .add_systems(
                Update,
                (surface_control.in_set(SurfaceControl).run_if(crate::editeur::in_game), update_underground, dim_star_light, update_suns, surface_light, update_lamps, update_season, refresh_voxels, update_tiles, update_hud)
                    .chain()
                    .after(crate::planet::orbit_planets)
                    .after(crate::planet::orbit_moons),
            );
    }
}

/// Pilotage du vaisseau et de la caméra près d'un astre (les géantes gazeuses passent après).
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct SurfaceControl;

/// Maillage lointain d'un astre (remplacé par les tuiles pendant un atterrissage).
#[derive(Component)]
pub struct FarMesh;

#[derive(Component)]
struct SurfaceTile;

#[derive(Component)]
struct SurfaceHud;

// ─────────────────────────────────────────────────────────────────────────
//  Paramètres d'un astre
// ─────────────────────────────────────────────────────────────────────────

/// Paramètres de terrain de l'astre ciblé (planète ou lune), `None` pour tout autre astre.
pub fn body_params(settings: &GameSettings, kind: &TargetKind) -> Option<BodyParams> {
    match *kind {
        TargetKind::Planet(id) => settings.systems.get(id / 1000)?.planets().get(id % 1000).map(BodyParams::planet),
        TargetKind::Moon(planet_id, moon) => {
            let planet = settings.systems.get(planet_id / 1000)?.planets().get(planet_id % 1000)?;
            Some(BodyParams::moon(planet.moons.get(moon)?, planet))
        }
        TargetKind::Asteroid(key) => crate::asteroids::body_params(settings, &key),
        _ => None,
    }
}

/// Rayon (depuis le centre) auquel le vaisseau stationne au-dessus d'un astre.
pub fn hover_radius(p: &BodyParams) -> f32 {
    // Astéroïde : juste au-dessus de sa plus grande bosse
    if let Some(s) = &p.asteroid {
        return s.max_radius() * 1.5 + 150.0;
    }
    p.radius + p.terrain_height * 0.6 + p.radius * 0.15 + 150.0
}

/// Épaisseur d'air visible depuis le sol : le ciel s'efface avec l'altitude (plus vite sous une
/// atmosphère ténue).
fn atmosphere_depth(p: &BodyParams) -> f32 {
    (p.radius * 0.12).max(300.0) * (0.5 + 0.5 * p.pressure.clamp(0.0, 10.0).powf(0.3))
}

/// Couleur du ciel vue d'un astre. `sun_height` : sinus de la hauteur de l'étoile au-dessus de
/// l'horizon ; `air` : part de l'atmosphère au-dessus de la caméra (1 au sol, 0 dans l'espace).
/// Le jour, couleur calculée de l'atmosphère (`planetgen::atmosphere`) ; quand l'étoile est basse,
/// celle du coucher de soleil ; la nuit, le noir de l'espace.
pub fn sky_color(p: &BodyParams, sun_height: f32, air: f32, space: [f32; 3]) -> [f32; 3] {
    if !p.atmosphere || p.pressure < 0.01 {
        return space;
    }
    let day = smoothstep((sun_height + 0.15) / 0.35) * air;
    // Coucher : l'étoile à moins de ~15° de l'horizon
    let low = (1.0 - (sun_height.abs() / 0.28)).clamp(0.0, 1.0) * 0.75;
    let lit = [
        p.sky[0] + (p.sunset[0] - p.sky[0]) * low,
        p.sky[1] + (p.sunset[1] - p.sky[1]) * low,
        p.sky[2] + (p.sunset[2] - p.sky[2]) * low,
    ];
    [
        space[0] + (lit[0] - space[0]) * day,
        space[1] + (lit[1] - space[1]) * day,
        space[2] + (lit[2] - space[2]) * day,
    ]
}

/// Repousse la caméra hors de l'astre pour qu'elle ne passe jamais sous sa surface.
pub fn keep_outside(cam: Vec3, center: Vec3, p: &BodyParams) -> Vec3 {
    let min = p.radius + p.terrain_height * 0.7 + 20.0;
    let rel = cam - center;
    let r = rel.length();
    if r < min && r > 1e-3 {
        center + rel / r * min
    } else {
        cam
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  Le marcheur : physique à la surface (repère de l'astre, centre à l'origine)
// ─────────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug)]
pub struct Walker {
    /// Position des pieds.
    pub pos: Vec3,
    /// Vitesse horizontale (tangente à la sphère).
    pub hvel: Vec3,
    /// Vitesse radiale (vers le haut > 0).
    pub vr: f32,
    /// Direction horizontale du regard (unitaire, tangente).
    pub heading: Vec3,
    pub pitch: f32,
    pub on_ground: bool,
    pub in_water: bool,
    /// Liquide où l'on nage (eau, méthane, ammoniac, lave).
    pub liquid: crate::planet::VoxelType,
    /// Rayon de l'œil, lissé pour adoucir les marches.
    pub eye_r: f32,
}

impl Default for Walker {
    fn default() -> Self {
        Self {
            pos: Vec3::Y,
            hvel: Vec3::ZERO,
            vr: 0.0,
            heading: Vec3::NEG_Z,
            pitch: 0.0,
            on_ground: false,
            in_water: false,
            liquid: crate::planet::VoxelType::Air,
            eye_r: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct WalkInput {
    pub forward: f32,
    pub strafe: f32,
    pub sprint: bool,
    pub jump: bool,
    /// Rotation (radians) : x = vers la droite, y = vers le bas.
    pub look: Vec2,
}

/// Composante de `h` tangente à la sphère en `up`, normalisée.
fn tangent(h: Vec3, up: Vec3) -> Vec3 {
    let t = h - up * h.dot(up);
    if t.length_squared() > 1e-8 {
        t.normalize()
    } else {
        up.any_orthonormal_vector()
    }
}

impl Walker {
    pub fn up(&self) -> Vec3 {
        self.pos.normalize_or(Vec3::Y)
    }

    /// Place le marcheur au sol, dans la direction `dir`, regardant vers `heading`.
    pub fn spawn(t: &Terrain, dir: Vec3, heading: Vec3) -> Self {
        let dir = dir.normalize();
        let ground = t.ground(dir);
        let eye = t.voxel() * EYE_VOXELS;
        Self {
            pos: dir * ground.top,
            heading: tangent(heading, dir),
            on_ground: true,
            eye_r: ground.top + eye,
            ..default()
        }
    }

    /// Un pas de simulation : regard, déplacement horizontal avec marches d'un voxel, gravité radiale.
    pub fn step(&mut self, t: &Terrain, inp: &WalkInput, dt: f32) {
        let v = t.voxel();
        let dt = dt.clamp(0.0, 0.05);
        let up = self.up();
        self.heading = tangent(self.heading, up);

        if inp.look != Vec2::ZERO {
            self.heading = (Quat::from_axis_angle(up, -inp.look.x) * self.heading).normalize();
            self.pitch = (self.pitch - inp.look.y).clamp(-1.5, 1.5);
        }

        let right = self.heading.cross(up).normalize_or_zero();
        let mut wish = self.heading * inp.forward + right * inp.strafe;
        if wish.length_squared() > 1.0 {
            wish = wish.normalize();
        }
        let mut speed = (if inp.sprint { SPRINT_VOXELS } else { WALK_VOXELS }) * v;
        if self.in_water {
            speed *= 0.55;
        }
        let accel = if self.on_ground { 14.0 } else { 2.5 };
        self.hvel = self.hvel.lerp(wish * speed, 1.0 - (-accel * dt).exp());
        self.hvel -= up * self.hvel.dot(up);

        // Déplacement horizontal : on grimpe d'un voxel, pas plus (et rien en l'air)
        let delta = self.hvel * dt;
        if delta.length_squared() > 0.0 {
            let allow = if self.on_ground { STEP_VOXELS * v } else { 0.0 };
            let r = self.pos.length();
            // Voxels 3D : le sol d'arrivée est au plus une marche plus haut, et la place pour le
            // corps est libre (un mur ou un surplomb trop bas arrête)
            // Le corps a une épaisseur : on vérifie aussi un point un tiers de voxel devant lui,
            // sinon il pourrait s'arrêter pile à la limite d'un mur et y basculer à l'arrondi près
            let fits = |dir: Vec3| -> bool {
                let floor = t.floor(dir, r + allow).top;
                floor <= r + allow + 0.05 * v && t.ceiling(dir, floor) >= floor.max(r) + BODY_VOXELS * v
            };
            let try_move = |pos: Vec3, d: Vec3| -> Option<Vec3> {
                let next = (pos + d).normalize() * r;
                let ahead = (next + d.normalize_or_zero() * BODY_RADIUS_VOXELS * v).normalize();
                (fits(next.normalize()) && fits(ahead)).then_some(next)
            };
            if let Some(next) = try_move(self.pos, delta) {
                self.pos = next;
            } else if let Some(next) = try_move(self.pos, right * delta.dot(right)) {
                self.pos = next;
                self.hvel = right * self.hvel.dot(right);
            } else if let Some(next) = try_move(self.pos, self.heading * delta.dot(self.heading)) {
                self.pos = next;
                self.hvel = self.heading * self.hvel.dot(self.heading);
            } else {
                self.hvel = Vec3::ZERO;
            }
        }

        // Vertical
        let up = self.up();
        let mut r = self.pos.length();
        // Le sol sous les pieds (une marche au-dessus au plus) : sous un surplomb, c'est le sol
        // d'en bas ; dessus, c'est le surplomb
        let ground = t.floor(up, r + STEP_VOXELS * v);
        self.in_water = ground.kind.is_liquid();
        self.liquid = ground.kind;
        // Vraie gravité de l'astre : sur une lune à 0,16 g, on saute six fois plus haut
        let min_gravity = if t.params.asteroid.is_some() { MIN_GRAVITY_ASTEROID } else { MIN_GRAVITY };
        let gravity = GRAVITY * t.params.gravity.clamp(min_gravity, 4.0) * v;
        if self.on_ground && inp.jump {
            self.vr = JUMP_VOXELS * v;
            // Microgravité : on saute haut et longtemps, sans quitter le petit astre
            if t.params.asteroid.is_some() {
                self.vr = self.vr.min((2.0 * gravity * t.params.radius * 0.15).sqrt());
            }
            self.on_ground = false;
        } else if self.on_ground {
            // On monte une marche seulement s'il y a la place pour la tête
            if ground.top >= r - STEP_VOXELS * v && (ground.top <= r || t.ceiling(up, ground.top) >= ground.top + BODY_VOXELS * v) {
                r = ground.top;
                self.vr = 0.0;
            } else {
                self.on_ground = false;
            }
        }
        if !self.on_ground {
            self.vr -= gravity * dt;
            r += self.vr * dt;
            if r <= ground.top {
                r = ground.top;
                self.vr = 0.0;
                self.on_ground = true;
            }
            // La tête cogne le plafond (dessous d'un surplomb)
            let roof = t.ceiling(up, ground.top);
            if r + BODY_VOXELS * v > roof {
                r = (roof - BODY_VOXELS * v).max(ground.top);
                self.vr = self.vr.min(0.0);
            }
        }
        self.pos = up * r;
        // Corps dans la roche (coin du cube où trois grilles de colonnes se rencontrent, relief qui
        // vient de changer, ou pile sur la limite de deux colonnes, que l'arrondi de la direction
        // fait basculer) : on remonte jusqu'à 16 voxels, sur le premier sol qui laisse la place du
        // corps ; vérifié avec la direction telle qu'elle sera relue (position normalisée)
        for _ in 0..2 {
            let up = self.up();
            let r = self.pos.length();
            if t.floor(up, r + 2.0 * v).top <= r + 0.05 * v && t.ceiling(up, r) >= r + BODY_VOXELS * v - 0.05 * v {
                break;
            }
            for n in 1..=16 {
                let top = t.floor(up, r + n as f32 * v).top;
                if top > r + 0.05 * v && t.ceiling(up, top) >= top + BODY_VOXELS * v - 0.05 * v && t.floor(up, top + 2.0 * v).top <= top + 0.05 * v {
                    self.pos = up * top;
                    self.vr = 0.0;
                    self.on_ground = true;
                    break;
                }
            }
        }
        let up = self.up();
        let r = self.pos.length();
        self.heading = tangent(self.heading, up);

        let eye_target = r + v * EYE_VOXELS;
        if self.on_ground {
            self.eye_r += (eye_target - self.eye_r) * (1.0 - (-20.0 * dt).exp());
        } else {
            self.eye_r = eye_target;
        }
    }

    /// Direction du regard (avec l'inclinaison).
    pub fn view_dir(&self) -> Vec3 {
        let up = self.up();
        self.heading * self.pitch.cos() + up * self.pitch.sin()
    }
}

// Dimensions en voxels : le joueur fait 2 voxels de haut
const EYE_VOXELS: f32 = 1.8;
const WALK_VOXELS: f32 = 7.0;
const SPRINT_VOXELS: f32 = 21.0;
const STEP_VOXELS: f32 = 1.05;
/// Hauteur du corps : il faut cette place libre pour passer sous un surplomb.
const BODY_VOXELS: f32 = 1.9;
/// Demi-largeur du corps (voxels).
const BODY_RADIUS_VOXELS: f32 = 0.3;
const JUMP_VOXELS: f32 = 7.5;
/// Pesanteur à 1 g, en voxels par seconde².
const GRAVITY: f32 = 22.0;
/// Sous cette gravité (g), on garde un minimum de poids : sinon un saut ne retomberait jamais.
const MIN_GRAVITY: f32 = 0.05;
/// Sur un astéroïde (C1) : microgravité, un saut dure de longues secondes.
const MIN_GRAVITY_ASTEROID: f32 = 0.01;

// ─────────────────────────────────────────────────────────────────────────
//  État de l'atterrissage
// ─────────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    Orbit,
    Descending,
    Walking,
    Ascending,
    /// Navigation à basse altitude autour de l'astre (zoom sous `FLIGHT_ZOOM`).
    Flying,
    /// Le personnage monte dans le vaisseau (cockpit ouvert), puis le vaisseau décolle.
    Boarding,
    /// Après l'atterrissage, le cockpit s'ouvre et le personnage descend du vaisseau.
    Disembarking,
}

/// Monter dans le vaisseau ou en descendre : attente (le cockpit s'ouvre), trajet (s).
const BOARD_WAIT: f32 = 0.5;
const UNBOARD_WAIT: f32 = 1.0;
const BOARD_SECS: f32 = 1.6;

/// Vol bas (0.13, règle 14, en voxels) : hauteur d'arrivée au-dessus du relief, vitesse selon
/// l'altitude (voxels/s, x4 avec Maj), vitesse de montée.
const HOVER_VOXELS: f32 = 22.0;
const FLY_MIN_VOXELS: f32 = 40.0;
const FLY_MAX_VOXELS: f32 = 4_000.0;
const CLIMB_MIN_VOXELS: f32 = 20.0;
const CLIMB_MAX_VOXELS: f32 = 3_000.0;

/// Vol suborbital (Q3) : touche, distance minimale (voxels).
pub const HOP_KEY: KeyCode = KeyCode::KeyJ;
const HOP_MIN_VOXELS: f32 = 5_000.0;

/// Un saut suborbital : d'une direction à l'autre de l'astre par une courbe haute (repère fixe de
/// l'astre).
#[derive(Clone, Copy, Debug)]
struct Hop {
    from: Vec3,
    to: Vec3,
    r0: f32,
    r1: f32,
    peak: f32,
    t: f32,
    dur: f32,
}

impl Hop {
    /// Position (repère de l'astre) et direction du mouvement à l'avancement `u` (0..1).
    fn at(&self, u: f32) -> (Vec3, Vec3) {
        let e = smoothstep(u);
        let rot = Quat::from_rotation_arc(self.from, self.to);
        let dir = (Quat::IDENTITY.slerp(rot, e) * self.from).normalize();
        let r = self.r0 + (self.r1 - self.r0) * e + self.peak * (std::f32::consts::PI * u).sin();
        let ahead = (Quat::IDENTITY.slerp(rot, (e + 0.01).min(1.0)) * self.from).normalize();
        (dir * r, tangent(ahead - dir, dir))
    }
}

/// En zoomant sous cette distance du vaisseau, on passe en navigation autour de l'astre ;
/// en dézoomant au-delà, on revient à la vue orbitale.
pub const FLIGHT_ZOOM: f32 = 1_000.0;

#[derive(Resource)]
pub struct Surface {
    phase: Phase,
    /// Poussée demandée (repère du vaisseau : -z = avant, y = haut ; longueur 0 à 1) et virage :
    /// les propulseurs du modèle suivent les commandes, pas le déplacement (C3).
    pub pilot: Vec3,
    pub pilot_turn: f32,
    /// Vent en vol bas (repère de l'astre), rafale, texte du HUD ; tangage / roulis qu'il donne.
    wind: Vec3,
    gust: f32,
    wind_text: String,
    tilt: Vec2,
    body: Option<TargetKind>,
    terrain: Option<Terrain>,
    walker: Walker,
    /// Animation de descente / de montée.
    t: f32,
    dur: f32,
    dir0: Vec3,
    dir1: Vec3,
    r0: f32,
    r1: f32,
    scale0: f32,
    scale1: f32,
    heading: Vec3,
    /// Pose de la caméra au début de la transition, et avancement du fondu vers la nouvelle pose.
    cam_from: Transform,
    cam_blend: f32,
    /// Vaisseau posé (repère de l'astre).
    ship_local: Vec3,
    ship_rot: Quat,
    ship_scale: f32,
    /// Dernier point d'atterrissage par astre : le vaisseau y stationne ensuite.
    hover: Option<(TargetKind, Vec3)>,
    cursor_locked: bool,
    /// Vol : position du vaisseau (repère de l'astre), vitesse, vitesse verticale, et caméra.
    fpos: Vec3,
    fspeed: f32,
    fvert: f32,
    fdist: f32,
    fyaw: f32,
    fpitch: f32,
    /// Le vaisseau descend à l'altitude de croisière en entrant en navigation.
    fdescend: bool,
    /// Vol suborbital en cours (0.13 E2, Q3).
    hop: Option<Hop>,
    /// Instant jusqu'auquel un zoom avant récent compte comme « je veux approcher ».
    zoom_in_until: f64,
    /// Jour du ciel à la caméra (0 : nuit ou pas d'air, 1 : plein jour), et hauteur de l'étoile
    /// (sinus) au-dessus de l'horizon local.
    daylight: f32,
    sun_height: f32,
    /// Lampe et phares autorisés (touche N) ; ils ne s'allument que dans le noir.
    lamps: bool,
    /// La nuit a déjà été signalée pendant ce séjour.
    night_told: bool,
    /// Sous terre (0 : à l'air libre, 1 : dans une grotte) : la lumière de l'étoile n'arrive pas.
    underground: f32,
    /// Le marcheur a perdu connaissance : retour automatique au vaisseau (A4).
    rescue: bool,
    /// Repère de l'astre à la dernière image (chocs contre les astéroïdes).
    frame: Option<Frame>,
    /// Dimensions du vaisseau (son modèle).
    pub ship_dims: ShipDims,
    /// À pied : vue à la troisième personne ; pose du personnage (monde) et son animation.
    /// Vue à pied (F5) : 0 = 1re personne, 1 = de dos, 2 = de face ; distance de la caméra (voxels).
    view: u8,
    pub cam_dist: f32,
    walker_local: Option<Transform>,
    walker_world: Option<Transform>,
    walker_anim: &'static str,
    /// Montée / descente du vaisseau : instant, d'où et vers où (repère de l'astre), pose du
    /// personnage.
    board_t: f32,
    board_from: Vec3,
    board_to: Vec3,
    board_pose: Option<Transform>,
}

impl Default for Surface {
    fn default() -> Self {
        Self {
            phase: Phase::Orbit,
            body: None,
            terrain: None,
            walker: Walker::default(),
            t: 0.0,
            dur: 1.0,
            dir0: Vec3::Y,
            dir1: Vec3::Y,
            r0: 1.0,
            r1: 1.0,
            scale0: 1.0,
            scale1: 1.0,
            heading: Vec3::NEG_Z,
            cam_from: Transform::IDENTITY,
            cam_blend: 1.0,
            ship_local: Vec3::Y,
            ship_rot: Quat::IDENTITY,
            ship_scale: 1.0,
            hover: None,
            cursor_locked: false,
            fpos: Vec3::Y,
            fspeed: 0.0,
            fvert: 0.0,
            fdist: FLIGHT_ZOOM * 0.5,
            fyaw: 0.0,
            fpitch: 0.35,
            fdescend: false,
            hop: None,
            zoom_in_until: 0.0,
            daylight: 1.0,
            sun_height: 1.0,
            lamps: true,
            night_told: false,
            underground: 0.0,
            rescue: false,
            frame: None,
            ship_dims: ShipDims::default(),
            view: 1,
            cam_dist: 6.0,
            walker_local: None,
            walker_world: None,
            walker_anim: "repos",
            board_t: 0.0,
            board_from: Vec3::ZERO,
            board_to: Vec3::ZERO,
            board_pose: None,
            pilot: Vec3::ZERO,
            pilot_turn: 0.0,
            wind: Vec3::ZERO,
            gust: 0.0,
            wind_text: String::new(),
            tilt: Vec2::ZERO,
        }
    }
}

impl Surface {
    /// Recentrage de l'origine flottante : la pose de caméra mémorisée est en repère monde.
    pub fn shift(&mut self, delta: Vec3) {
        self.cam_from.translation -= delta;
    }

    /// La souris est rendue (éditeur, menus) : elle sera recapturée au retour à pied.
    pub fn release_cursor(&mut self) {
        self.cursor_locked = false;
    }

    /// Atterrissage, séjour ou décollage en cours : la caméra n'est plus pilotée par l'orbite.
    pub fn active(&self) -> bool {
        self.phase != Phase::Orbit
    }

    /// Les soleils directionnels (ombres, C3) remplacent la lumière des étoiles : séjour sur un astre
    /// solide.
    pub fn suns_on(&self) -> bool {
        self.active() && !self.gaseous()
    }

    /// Centre de l'astre où l'on séjourne (monde, dernière image).
    pub fn center(&self) -> Option<Vec3> {
        self.frame.filter(|_| self.active()).map(|f| f.center)
    }

    /// Vent du lieu en vol bas (repère de l'astre, m/s poussés), rafale (0..1), texte du HUD.
    pub fn set_wind(&mut self, v: Vec3, gust: f32, text: String) {
        self.wind = v;
        self.gust = gust;
        self.wind_text = text;
    }

    /// En vol bas : le vent déplace le vaisseau de `local` (repère fixe de l'astre, C5).
    pub fn drift(&mut self, local: Vec3) {
        if self.phase == Phase::Flying {
            let r = self.fpos.length();
            self.fpos = (self.fpos + local).normalize_or(Vec3::Y) * r;
        }
    }

    /// Marée actuelle du terrain (C4).
    pub fn tide(&self) -> Option<crate::terrain::Tide> {
        self.terrain().map(|t| t.params.tide)
    }

    /// Nouvelle marée : le sol (collisions) tout de suite, les tuiles une à une.
    pub fn set_tide(&mut self, tide: crate::terrain::Tide) {
        if let Some(t) = self.terrain.as_mut() {
            t.params.tide = tide;
        }
    }

    /// Hauteur (sinus) du plus haut soleil au-dessus de l'horizon local.
    pub fn sun_height(&self) -> f32 {
        self.sun_height
    }

    /// Le vaisseau stationnera au-dessus de `dir` (repère fixe de l'astre `kind`).
    pub fn set_hover(&mut self, kind: TargetKind, dir: Vec3) {
        self.hover = Some((kind, dir.normalize_or(Vec3::Y)));
    }

    /// Taille d'un voxel du terrain de l'astre où l'on séjourne.
    pub fn voxel(&self) -> Option<f32> {
        self.terrain.as_ref().filter(|_| self.active()).map(|t| t.voxel())
    }

    /// Navigation à basse altitude autour de l'astre.
    pub fn flying(&self) -> bool {
        self.phase == Phase::Flying
    }

    /// Distance de caméra sous laquelle on passe en vol bas (au moins `FLIGHT_ZOOM`, trois longueurs
    /// du vaisseau).
    /// Tests (0.13 E3) : comme un coup de molette vers l'astre (passage en vol bas si on est assez
    /// près).
    pub fn test_zoom_in(&mut self, now: f64) {
        self.zoom_in_until = now + 0.7;
    }

    pub fn flight_zoom(&self) -> f32 {
        FLIGHT_ZOOM.max(self.ship_dims.max_length() * 3.0)
    }

    /// État du vaisseau pour ses animations (E6) : posé, décollage, atterrissage, vol.
    pub fn ship_state(&self) -> &'static str {
        match self.phase {
            Phase::Walking | Phase::Boarding | Phase::Disembarking => "pose",
            Phase::Descending => "atterrissage",
            Phase::Ascending => "decollage",
            Phase::Flying | Phase::Orbit => "vol",
        }
    }

    /// Le personnage du joueur à afficher (à pied, vue à la troisième personne) : pose (monde,
    /// échelle = un voxel du terrain) et animation.
    pub fn walker_view(&self) -> Option<(Transform, &'static str)> {
        match self.phase {
            Phase::Walking if self.view != 0 => self.walker_world.map(|t| (t, self.walker_anim)),
            // On voit toujours le personnage monter dans le vaisseau ou en descendre
            Phase::Boarding | Phase::Disembarking => self.walker_world.map(|t| (t, "marcher")),
            _ => None,
        }
    }

    /// Le marcheur (repère de l'astre, réseau) : astre, pose, animation.
    pub fn walker_state(&self) -> Option<(TargetKind, Transform, &'static str)> {
        match self.phase {
            Phase::Walking => Some((self.body?, self.walker_local?, self.walker_anim)),
            Phase::Boarding | Phase::Disembarking => Some((self.body?, self.board_pose?, "marcher")),
            _ => None,
        }
    }

    /// En vol : le vaisseau est repoussé de `push` (monde) par un choc contre un astéroïde ; il
    /// rebondit s'il fonçait dessus.
    pub fn bump(&mut self, push: Vec3, bounce: bool) {
        let (Phase::Flying, Some(frame)) = (self.phase, self.frame) else { return };
        self.fpos += frame.vector(push);
        if bounce {
            self.fspeed *= -0.35;
            self.fvert *= -0.35;
        }
    }

    /// Le marcheur, s'il est à pied hors du vaisseau.
    pub fn walking(&self) -> Option<&Walker> {
        (self.phase == Phase::Walking).then_some(&self.walker)
    }

    /// Ramène le marcheur au vaisseau et décolle (il a perdu connaissance).
    pub fn request_rescue(&mut self) {
        if self.phase == Phase::Walking {
            self.rescue = true;
        }
    }

    /// Terrain de l'astre où l'on séjourne.
    pub fn terrain(&self) -> Option<&Terrain> {
        self.terrain.as_ref().filter(|_| self.active())
    }

    /// Astre où l'on séjourne (vol bas, atterrissage, marche).
    pub fn body(&self) -> Option<TargetKind> {
        self.body.filter(|_| self.active())
    }

    /// Point survolé ou foulé, dans le repère fixe de l'astre (règle 10).
    pub fn local_point(&self) -> Option<Vec3> {
        match self.phase {
            Phase::Orbit => None,
            Phase::Walking | Phase::Boarding | Phase::Disembarking => Some(self.walker.pos),
            Phase::Flying => Some(self.fpos),
            Phase::Descending | Phase::Ascending => Some(self.dir1),
        }
    }

    /// Altitude relative du sol (0 = niveau de la mer, 1 = sommets) sous le marcheur ou le
    /// vaisseau en vol bas : les sommets sont plus froids.
    /// Taille d'un voxel au sol et hauteur (unités) du joueur ou du vaisseau au-dessus du relief :
    /// la brume de l'horizon se règle en voxels (0.13, règle 14).
    pub fn ground_scale(&self) -> Option<(f32, f32)> {
        let t = self.terrain.as_ref()?;
        let p = self.local_point()?;
        Some((t.voxel(), (p.length() - t.ground(p.normalize_or(Vec3::Y)).top).max(0.0)))
    }

    pub fn ground_altitude(&self) -> Option<f32> {
        let t = self.terrain.as_ref()?;
        let p = &t.params;
        let r = match self.phase {
            Phase::Walking => self.walker.pos.length(),
            Phase::Flying => t.ground(self.fpos.normalize_or(Vec3::Y)).top,
            _ => return None,
        };
        Some(crate::planetgen::climate::relative_altitude((r - p.radius) / p.terrain_height.max(1.0)))
    }

    /// Direction (repère fixe de l'astre, depuis son centre) au-dessus de laquelle le vaisseau
    /// stationne.
    pub fn hover_dir(&self, kind: &TargetKind) -> Option<Vec3> {
        self.hover.filter(|(k, _)| k == kind).map(|(_, d)| d)
    }

    /// Identifiant de la planète dont le maillage lointain est remplacé par les tuiles (jamais une
    /// géante gazeuse : pas de terrain, on vole dans sa sphère).
    pub fn active_planet(&self) -> Option<usize> {
        if self.gaseous() {
            return None;
        }
        match (self.phase, self.body) {
            (Phase::Orbit, _) => None,
            (_, Some(TargetKind::Planet(id))) => Some(id),
            _ => None,
        }
    }

    pub fn params(&self) -> Option<BodyParams> {
        self.terrain.as_ref().map(|t| t.params)
    }

    /// Brume de l'horizon pendant un séjour sous une atmosphère : (couleur, pression en bar).
    pub fn haze(&self) -> Option<([f32; 3], f32)> {
        let p = self.params().filter(|p| self.active() && p.atmosphere && !p.gaseous && p.pressure >= 0.01)?;
        Some((p.haze, p.pressure))
    }

    /// Jour du ciel (0 : nuit, 1 : plein jour) pendant un séjour sous une atmosphère.
    pub fn daylight(&self) -> f32 {
        self.daylight
    }

    /// Opacité maximale de la brume : forte le jour, faible la nuit (lunes et étoiles visibles).
    pub fn haze_opacity(&self) -> f32 {
        NIGHT_HAZE + (DAY_HAZE - NIGHT_HAZE) * self.daylight
    }

    /// Il fait sombre là où l'on est : l'étoile est sous l'horizon (ou à peine levée).
    pub fn dark(&self) -> bool {
        self.active() && (self.sun_height < 0.06 || self.underground > 0.5)
    }

    /// Sous terre (0 à 1).
    pub fn underground(&self) -> f32 {
        if self.active() { self.underground } else { 0.0 }
    }

    /// Séjour dans une géante gazeuse.
    pub fn gaseous(&self) -> bool {
        self.params().is_some_and(|p| p.gaseous)
    }

    /// Vaisseau détruit : retour en orbite, au-dessus de l'endroit survolé (hors de l'astre).
    pub fn eject(&mut self) {
        if let (Some(kind), true) = (self.body, self.active()) {
            self.hover = Some((kind, self.fpos.normalize_or(Vec3::Y)));
        }
        self.abort();
    }

    fn abort(&mut self) {
        self.phase = Phase::Orbit;
        self.body = None;
        self.terrain = None;
        self.daylight = 1.0;
        self.sun_height = 1.0;
        self.night_told = false;
        self.underground = 0.0;
    }
}

/// Le vaisseau posé décolle (le personnage est à bord).
fn start_ascent(surface: &mut Surface, params: &BodyParams, fz: f32, cam_from: Transform) {
    let dir = surface.ship_local.normalize();
    surface.dir0 = dir;
    surface.dir1 = dir;
    surface.r0 = surface.ship_local.length();
    surface.r1 = hover_radius(params);
    surface.scale0 = surface.ship_scale;
    surface.scale1 = fz * 1.2 * 0.008;
    surface.heading = tangent(surface.ship_rot * Vec3::NEG_Z, dir);
    surface.t = 0.0;
    surface.dur = (2.5 + (surface.r1 - surface.r0) / 8000.0).clamp(2.5, 6.0);
    surface.cam_from = cam_from;
    surface.cam_blend = 0.0;
    surface.phase = Phase::Ascending;
}

/// Où l'on monte dans le vaisseau posé (repère de l'astre) : sa verrière, sinon sa rampe ou sa
/// porte (blocs de mouvement de son modèle), sinon le dessus du vaisseau, vers l'avant.
fn cockpit(surface: &Surface, models: &crate::models::GameModels) -> Vec3 {
    let ship = Transform { translation: surface.ship_local, rotation: surface.ship_rot, scale: Vec3::splat(surface.ship_scale) };
    let Some(l) = models.peek(&models.ship) else { return surface.ship_local };
    let m = ship.compute_matrix() * crate::models::fit_transform(l, crate::models::Fit::Ship).compute_matrix();
    let entry = ["verriere", "rampe", "porte_pivotante", "porte_coulissante"]
        .iter()
        .find_map(|b| l.model.zones.iter().find(|z| z.block == *b))
        .map(|z| Vec3::from_array(z.pivot))
        .unwrap_or(Vec3::new(l.center().x, l.hi.y, l.center().z + l.size().z * 0.25));
    m.transform_point3(entry)
}

/// Lance la descente du vaisseau vers `dir1` depuis (`dir0`, `r0`).
#[allow(clippy::too_many_arguments)]
fn begin_descent(
    surface: &mut Surface,
    kind: TargetKind,
    terrain: Terrain,
    dir0: Vec3,
    r0: f32,
    dir1: Vec3,
    heading: Vec3,
    scale0: f32,
    cam_from: Transform,
) {
    let params = terrain.params;
    // Posé : sa vraie taille (4 voxels du modèle = 1 bloc), le dessous sur le sol
    let scale1 = surface.ship_dims.real_scale(terrain.voxel());
    let r1 = terrain.ground(dir1).top + scale1 * surface.ship_dims.half_h;
    let r0 = r0.max(r1 + scale1 * 8.0);
    let angle = dir0.dot(dir1).clamp(-1.0, 1.0).acos();
    surface.dur = (1.8 + (r0 - r1) / 9000.0 + angle * params.radius / 12000.0).clamp(2.5, 8.0);
    surface.heading = tangent(heading, dir0);
    surface.t = 0.0;
    surface.dir0 = dir0;
    surface.dir1 = dir1;
    surface.r0 = r0;
    surface.r1 = r1;
    surface.scale0 = scale0.max(1.0);
    surface.scale1 = scale1;
    surface.cam_from = cam_from;
    surface.cam_blend = 0.0;
    surface.body = Some(kind);
    surface.terrain = Some(terrain);
    surface.phase = Phase::Descending;
}

/// Orientation du vaisseau dans l'espace (une seule fonction, C3) : en croisière, le nez (avant du
/// modèle) vers la destination, sans roulis ; à l'approche d'un astre où l'on se pose, passage en
/// douceur à la pose de stationnement (dessous parallèle à la surface, nez à l'horizontale) ; virage
/// doux, sauf `snap`. `step` = pas fait par les moteurs (zéro en stationnement).
#[allow(clippy::too_many_arguments)]
pub fn orient_ship(before: Quat, pos: Vec3, step: Vec3, hover: Vec3, center: Vec3, levels: bool, snap: bool, dt: f32) -> Quat {
    let fwd = before * Vec3::NEG_Z;
    let travel = (step.length_squared() > 1e-8).then(|| {
        let d = step.normalize();
        let up = if d.dot(Vec3::Y).abs() > 0.98 { before * Vec3::Y } else { Vec3::Y };
        look(Vec3::ZERO, d, up).rotation
    });
    let park = levels.then(|| {
        let up = (pos - center).normalize_or(Vec3::Y);
        let f = tangent(travel.map_or(fwd, |t| t * Vec3::NEG_Z), up);
        look(Vec3::ZERO, f, up).rotation
    });
    let want = match (travel, park) {
        (Some(t), Some(p)) => {
            let hover_r = hover.distance(center).max(1.0);
            let w = 1.0 - smoothstep((pos.distance(hover) - hover_r * 0.5) / (hover_r * 3.0));
            t.slerp(p, w)
        }
        (Some(t), None) => t,
        (None, Some(p)) => p,
        (None, None) => return before,
    };
    if snap { want } else { before.slerp(want, 1.0 - (-4.0 * dt).exp()) }
}

fn smoothstep(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

fn look(from: Vec3, dir: Vec3, up: Vec3) -> Transform {
    Transform::from_translation(from)
        .looking_to(Dir3::new(dir).unwrap_or(Dir3::NEG_Z), Dir3::new(up).unwrap_or(Dir3::Y))
}

/// Caméra qui suit le vaisseau, derrière et au-dessus.
fn chase_pose(ship: Vec3, up: Vec3, heading: Vec3, scale: f32) -> Transform {
    let pos = ship + up * (3.5 * scale) - heading * (11.0 * scale);
    let focus = ship + up * (0.4 * scale);
    look(pos, focus - pos, up)
}

fn blend_pose(from: &Transform, to: Transform, b: f32) -> Transform {
    Transform {
        translation: from.translation.lerp(to.translation, b),
        rotation: from.rotation.slerp(to.rotation, b),
        scale: Vec3::ONE,
    }
}

fn ray_sphere(origin: Vec3, dir: Vec3, radius: f32) -> Option<f32> {
    let b = origin.dot(dir);
    let c = origin.dot(origin) - radius * radius;
    let disc = b * b - c;
    if disc < 0.0 {
        return None;
    }
    let t = -b - disc.sqrt();
    (t > 0.0).then_some(t)
}

#[cfg(target_os = "windows")]
const GRAB: CursorGrabMode = CursorGrabMode::Confined;
#[cfg(not(target_os = "windows"))]
const GRAB: CursorGrabMode = CursorGrabMode::Locked;

fn set_cursor(windows: &mut Query<&mut Window, With<PrimaryWindow>>, locked: bool) {
    let Ok(mut w) = windows.get_single_mut() else { return };
    let (mode, visible) = if locked { (GRAB, false) } else { (CursorGrabMode::None, true) };
    if w.cursor_options.grab_mode != mode {
        w.cursor_options.grab_mode = mode;
    }
    if w.cursor_options.visible != visible {
        w.cursor_options.visible = visible;
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  Système principal
// ─────────────────────────────────────────────────────────────────────────

#[derive(SystemParam)]
struct Ctx<'w, 's> {
    time: Res<'w, Time>,
    keys: Res<'w, ButtonInput<KeyCode>>,
    motion: EventReader<'w, 's, MouseMotion>,
    wheel: EventReader<'w, 's, MouseWheel>,
    buttons: Res<'w, ButtonInput<MouseButton>>,
    settings: Res<'w, GameSettings>,
    menu: Res<'w, MenuState>,
    panel: Res<'w, NetPanel>,
    travel: Res<'w, crate::wormhole::WormholeTravel>,
    viewport: Res<'w, crate::graphics::ViewportScale>,
    target: Res<'w, CameraTarget>,
    planets: Query<'w, 's, (&'static Transform, &'static PlanetId), (With<PlanetRoot>, Without<Ship>, Without<Camera3d>)>,
    moons: Query<'w, 's, (&'static Transform, &'static MoonId), (With<MoonRoot>, Without<Ship>, Without<Camera3d>)>,
    stars: Query<'w, 's, (&'static Transform, &'static StarId), (With<StarRoot>, Without<Ship>, Without<Camera3d>)>,
    windows: Query<'w, 's, &'static mut Window, With<PrimaryWindow>>,
    asteroids: Res<'w, crate::asteroids::AsteroidField>,
    dim: Res<'w, crate::sky::SunDim>,
    weather: Res<'w, crate::weather::WeatherNow>,
    clock: Res<'w, crate::world_clock::WorldClock>,
    models: Res<'w, crate::models::GameModels>,
}

impl Ctx<'_, '_> {
    /// Centre (monde) et orientation d'un astre chargé : son repère fixe tourne avec lui.
    fn pose(&self, kind: &TargetKind) -> Option<Frame> {
        let tf = match *kind {
            TargetKind::Planet(id) => self.planets.iter().find(|(_, p)| p.0 == id).map(|(t, _)| *t),
            TargetKind::Moon(planet_idx, moon_idx) => self
                .moons
                .iter()
                .find(|(_, m)| m.planet_idx == planet_idx && m.moon_idx == moon_idx)
                .map(|(t, _)| *t),
            TargetKind::Asteroid(key) => self.asteroids.pose(&key),
            _ => None,
        }?;
        Some(Frame { center: tf.translation, rot: tf.rotation })
    }

    /// Point de la surface pointé par la souris (sinon, le côté tourné vers la caméra), dans le
    /// repère fixe de l'astre.
    fn aimed_dir(&self, camera: &Camera, cam_gt: &GlobalTransform, frame: &Frame, p: &BodyParams) -> Vec3 {
        let sphere = p.radius + p.terrain_height * 0.2;
        if let Some(cursor) = self.windows.get_single().ok().and_then(|w| w.cursor_position()) {
            if let Ok(ray) = camera.viewport_to_world(cam_gt, self.viewport.to_viewport(cursor)) {
                let origin = frame.point(ray.origin);
                let dir = frame.vector(*ray.direction);
                if let Some(t) = ray_sphere(origin, dir, sphere) {
                    return (origin + dir * t).normalize();
                }
            }
        }
        frame.point(cam_gt.translation()).normalize_or(Vec3::Y)
    }
}

/// Repère fixe d'un astre (règle 10) : centre et orientation dans le monde. Tout ce qui est posé
/// sur l'astre (vaisseau, marcheur, caméra au sol) est calculé dans ce repère, puis placé dans le
/// monde au rendu ; la rotation de l'astre l'emporte sans glissement.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub center: Vec3,
    pub rot: Quat,
}

impl Frame {
    /// Point du monde -> repère de l'astre.
    pub fn point(&self, world: Vec3) -> Vec3 {
        self.rot.inverse() * (world - self.center)
    }

    /// Direction du monde -> repère de l'astre.
    pub fn vector(&self, world: Vec3) -> Vec3 {
        self.rot.inverse() * world
    }

    /// Pose dans le repère de l'astre -> monde.
    pub fn to_world(&self, local: Transform) -> Transform {
        Transform { translation: self.center + self.rot * local.translation, rotation: self.rot * local.rotation, scale: local.scale }
    }

    /// Pose du monde -> repère de l'astre.
    pub fn to_local(&self, world: Transform) -> Transform {
        Transform { translation: self.point(world.translation), rotation: self.rot.inverse() * world.rotation, scale: world.scale }
    }
}

#[allow(clippy::too_many_arguments)]
fn surface_control(
    mut ctx: Ctx,
    mut surface: ResMut<Surface>,
    mut zoom: ResMut<ZoomLevel>,
    mut clear: ResMut<ClearColor>,
    mut net: ResMut<Net>,
    mut ship_q: Query<(&mut Transform, &mut Visibility), (With<Ship>, Without<Camera3d>)>,
    mut cam_q: Query<(&Camera, &GlobalTransform, &mut Transform, &mut CameraController), (With<Camera3d>, Without<Ship>)>,
) {
    let mut look_delta = Vec2::ZERO;
    for ev in ctx.motion.read() {
        look_delta += ev.delta;
    }
    let mut wheel = 0.0;
    for ev in ctx.wheel.read() {
        wheel += crate::ui::wheel_lines(ev);
    }
    let (Ok((camera, cam_gt, mut cam_tf, mut ctrl)), Ok((mut ship_tf, mut ship_vis))) =
        (cam_q.get_single_mut(), ship_q.get_single_mut())
    else {
        return;
    };
    let dt = ctx.time.delta_secs().min(0.1);
    let now = ctx.time.elapsed_secs_f64();
    let ui_open = ctx.menu.open || ctx.panel.open || ctx.panel.guild_open || ctx.panel.focus.is_some();
    let enter = (ctx.keys.just_pressed(ENTER_SHIP_KEY) && !ui_open) || test_land(now);
    // Zoom de passage en vol bas : plus loin pour un grand vaisseau (on doit le voir en entier)
    let fz = surface.flight_zoom();

    // ── En orbite : on attend l'ordre d'atterrir ─────────────────────────
    if surface.phase == Phase::Orbit {
        if surface.cursor_locked {
            surface.cursor_locked = false;
            set_cursor(&mut ctx.windows, false);
        }
        if ctx.travel.active() || ui_open {
            return;
        }
        let kind = ctx.target.0;
        // Zoom sous 1000 sur une planète ou une lune : navigation autour de l'astre (seulement si on
        // vient de zoomer : au lancement ou après un changement de cible, la vue reste orbitale)
        if wheel > 0.0 {
            surface.zoom_in_until = now + 0.7;
        }
        if ctrl.distance < fz && now < surface.zoom_in_until {
            if let (Some(params), Some(frame)) = (body_params(&ctx.settings, &kind), ctx.pose(&kind)) {
                let local = frame.point(ship_tf.translation);
                if (local.length() - hover_radius(&params)).abs() < params.radius * 0.1 + 250.0 {
                    let up = local.normalize_or(Vec3::Y);
                    surface.terrain = Some(Terrain::new(params).with_voxels(crate::voxel::body_voxels(&ctx.settings, &kind)));
                    surface.body = Some(kind);
                    surface.fpos = local;
                    surface.heading = tangent(frame.vector(*ship_tf.forward()), up);
                    surface.fspeed = 0.0;
                    surface.fvert = 0.0;
                    // Caméra derrière le vaisseau à sa vraie taille (quelques longueurs) : il est
                    // petit à l'échelle 0.13 ; la molette recule jusqu'à la vue orbitale
                    let voxel = surface.terrain.as_ref().map_or(1.0, |t| t.voxel());
                    let ship_len = surface.ship_dims.real_scale(voxel) * surface.ship_dims.icon_len;
                    surface.fdist = (ship_len * 4.0).min(ctrl.distance).clamp(ship_len * 1.2, fz * 0.95);
                    surface.fyaw = 0.0;
                    surface.fpitch = 0.35;
                    // Une géante n'a pas de sol : on ne plonge pas d'office vers son cœur
                    surface.fdescend = !params.gaseous;
                    surface.phase = Phase::Flying;
                    *ship_vis = Visibility::Inherited;
                    if params.gaseous {
                        net.notify("Geante gazeuse : Ctrl pour descendre dans l'atmosphere. Attention, la pression y abime la coque !", now);
                    } else if params.asteroid.is_some() {
                        net.notify("Champ d'asteroides : ZQSD/WASD voler, Maj accelerer (attention aux chocs : degats selon la vitesse), V se poser, molette pour revenir.", now);
                    } else {
                        net.notify("Navigation : ZQSD/WASD voler, Maj accelerer, Espace/Ctrl monter/descendre, V atterrir, molette pour revenir.", now);
                    }
                    return;
                }
            }
        }
        if !enter {
            return;
        }
        let Some(params) = body_params(&ctx.settings, &kind) else {
            net.notify("Selectionnez une planete, une lune ou un gros asteroide pour atterrir.", now);
            return;
        };
        let Some(frame) = ctx.pose(&kind) else {
            net.notify("Cet astre est trop loin : approchez-vous de son systeme.", now);
            return;
        };
        if params.gaseous {
            net.notify("Pas de sol : une geante gazeuse n'a pas de surface. Zoomez sous 1000 pour entrer dans son atmosphere (la pression abime la coque).", now);
            return;
        }
        let terrain = Terrain::new(params).with_voxels(crate::voxel::body_voxels(&ctx.settings, &kind));
        let dir1 = ctx.aimed_dir(camera, cam_gt, &frame, &params);
        let local0 = frame.point(ship_tf.translation);
        let dir0 = local0.normalize_or(dir1);
        let heading = dir1 - dir0 * dir0.dot(dir1);
        begin_descent(&mut surface, kind, terrain, dir0, local0.length(), dir1, heading, ship_tf.scale.x, frame.to_local(*cam_tf));
        *ship_vis = Visibility::Inherited;
        net.notify("Atterrissage... (V pour redecoller une fois au sol)", now);
        return;
    }

    // Le corps visité a disparu (changement de système) : retour en orbite
    let Some(kind) = surface.body else {
        surface.abort();
        return;
    };
    let (Some(frame), Some(params)) = (ctx.pose(&kind), surface.params()) else {
        surface.abort();
        clear.0 = SPACE_SKY;
        set_cursor(&mut ctx.windows, false);
        net.notify("Retour en orbite : l'astre n'est plus charge.", now);
        return;
    };
    *zoom = ZoomLevel::Planet;
    *ship_vis = Visibility::Inherited;
    surface.frame = Some(frame);
    let center = frame.center;
    // Tout ce qui suit est calculé dans le repère fixe de l'astre (centre à l'origine), puis placé
    // dans le monde à la fin ; la pose de départ des fondus (`cam_from`) est aussi dans ce repère
    let mut ship_local = frame.to_local(*ship_tf);
    surface.pilot = Vec3::ZERO;
    surface.pilot_turn = 0.0;
    let mut cam_local_tf = frame.to_local(*cam_tf);

    match surface.phase {
        // ── Descente vers le point choisi ────────────────────────────────
        Phase::Descending | Phase::Ascending => {
            let descending = surface.phase == Phase::Descending;
            set_cursor(&mut ctx.windows, false);
            surface.t += dt;
            let u = (surface.t / surface.dur).clamp(0.0, 1.0);
            let e = smoothstep(u);
            let (dir0, dir1) = (surface.dir0, surface.dir1);
            let dir = (Quat::IDENTITY.slerp(Quat::from_rotation_arc(dir0, dir1), e) * dir0).normalize();
            let scale = surface.scale0 + (surface.scale1 - surface.scale0) * e;
            let mut r = surface.r0 + (surface.r1 - surface.r0) * e;
            if descending {
                // Jamais sous le relief avant l'arrivée
                let floor = surface.terrain.as_ref().map_or(0.0, |t| t.ground(dir).top) + scale * 3.0 * (1.0 - e);
                r = r.max(floor);
            }
            surface.heading = tangent(surface.heading, dir);
            let ship_pos = dir * r;
            let ship_rot = look(Vec3::ZERO, surface.heading, dir).rotation;
            // Décollage : pleine poussée vers le haut ; atterrissage : on freine (poussée vers le haut)
            surface.pilot = Vec3::Y * if descending { 0.4 * (1.0 - e) + 0.15 } else { 1.0 };
            ship_local = Transform { translation: ship_pos, rotation: ship_rot, scale: Vec3::splat(scale) };

            surface.cam_blend = (surface.cam_blend + dt / 0.8).min(1.0);
            let chase = chase_pose(ship_pos, dir, surface.heading, scale);
            cam_local_tf = blend_pose(&surface.cam_from, chase, smoothstep(surface.cam_blend));

            if u >= 1.0 {
                if descending {
                    let right = surface.heading.cross(dir).normalize_or_zero();
                    let side = (dir * r + right * (scale * surface.ship_dims.icon_len * 0.6 + 12.0)).normalize();
                    let walker = Walker::spawn(surface.terrain.as_ref().unwrap(), side, -right);
                    surface.ship_local = dir * r;
                    surface.ship_rot = ship_rot;
                    surface.ship_scale = scale;
                    surface.walker = walker;
                    surface.cam_from = cam_local_tf;
                    surface.cam_blend = 0.0;
                    // Le cockpit s'ouvre et le personnage en descend
                    surface.board_from = cockpit(&surface, &ctx.models);
                    surface.board_to = walker.pos;
                    surface.board_t = 0.0;
                    surface.phase = Phase::Disembarking;
                } else {
                    // De retour en orbite : le vaisseau stationne au-dessus du point de décollage
                    surface.hover = Some((kind, dir));
                    ctrl.distance = fz * 1.2;
                    ctrl.zoom_goal = Some((params.radius * 2.5).max(2_500.0));
                    ctrl.last_target_pos = center;
                    surface.abort();
                    clear.0 = SPACE_SKY;
                }
            }
        }

        // ── À pied ───────────────────────────────────────────────────────
        Phase::Walking => {
            let locked = !ui_open;
            if surface.cursor_locked != locked {
                surface.cursor_locked = locked;
                set_cursor(&mut ctx.windows, locked);
            }
            let mut input = WalkInput::default();
            if locked {
                let k = &ctx.keys;
                let axis = |pos: &[KeyCode], neg: &[KeyCode]| {
                    pos.iter().any(|c| k.pressed(*c)) as i32 as f32 - neg.iter().any(|c| k.pressed(*c)) as i32 as f32
                };
                input.forward = axis(&[KeyCode::KeyW, KeyCode::ArrowUp], &[KeyCode::KeyS, KeyCode::ArrowDown]);
                input.strafe = axis(&[KeyCode::KeyD, KeyCode::ArrowRight], &[KeyCode::KeyA, KeyCode::ArrowLeft]);
                input.sprint = k.pressed(KeyCode::ShiftLeft) || k.pressed(KeyCode::ShiftRight);
                input.jump = k.pressed(KeyCode::Space);
                // Mesures (0.13) : `SPACESPORE_TEST_WALK` = marche tout droit (parcours fixe)
                if std::env::var("SPACESPORE_TEST_WALK").is_ok() {
                    input.forward = 1.0;
                    input.jump = true;
                }
                let sens = ctx.settings.mouse_sensitivity * 0.003;
                input.look = look_delta.clamp_length_max(300.0) * sens;
            }
            // F5 : 1re personne -> de dos -> de face ; molette = distance de la caméra
            if ctx.keys.just_pressed(VIEW_KEY) && !ui_open {
                surface.view = (surface.view + 1) % 3;
            }
            if wheel != 0.0 && surface.view != 0 && !ui_open {
                surface.cam_dist = (surface.cam_dist * (-wheel * 0.12).exp()).clamp(3.0, 12.0);
            }
            let terrain = surface.terrain.take().unwrap();
            let mut walker = surface.walker;
            walker.step(&terrain, &input, dt);
            // On ne traverse pas le vaisseau posé (boîtes de collision de son modèle)
            let ship_tf = Transform { translation: surface.ship_local, rotation: surface.ship_rot, scale: Vec3::splat(surface.ship_scale) };
            if let Some(l) = ctx.models.peek(&ctx.models.ship) {
                let to_ship = (ship_tf.compute_matrix() * crate::models::fit_transform(l, crate::models::Fit::Ship).compute_matrix()).inverse();
                let v = terrain.voxel();
                let waist = walker.pos + walker.up() * v;
                if let Some(push) = crate::models::push_out(l, to_ship, waist, v * 0.4) {
                    walker.pos += push;
                }
            }
            let v = terrain.voxel();
            let micro = params.asteroid.is_some();
            surface.walker_anim = crate::models::walker_anim(walker.on_ground, walker.in_water, walker.hvel.length(), input.sprint, micro, walker.vr, v);
            surface.walker = walker;
            surface.terrain = Some(terrain);

            // Le vaisseau reste posé là où il a atterri (et tourne avec l'astre)
            ship_local = Transform {
                translation: surface.ship_local,
                rotation: surface.ship_rot,
                scale: Vec3::splat(surface.ship_scale),
            };

            let up = walker.up();
            let eye = up * walker.eye_r;
            let v = surface.terrain.as_ref().map_or(1.0, |t| t.voxel());
            surface.walker_local = Some(Transform { translation: walker.pos, rotation: look(Vec3::ZERO, walker.heading, up).rotation, scale: Vec3::splat(v) });
            // Troisième personne : derrière (ou devant, tournée vers le visage) et un peu au-dessus,
            // jamais sous le sol
            let fps = if surface.view != 0 {
                let view = walker.view_dir();
                let side = if surface.view == 2 { -1.0 } else { 1.0 };
                let d = surface.cam_dist * v;
                let mut cam = eye - view * (d * side) + up * (v * 0.8 * surface.cam_dist / 4.5);
                if let Some(t) = surface.terrain.as_ref() {
                    let floor = t.floor(cam.normalize(), cam.length()).top + v * 0.4;
                    if cam.length() < floor {
                        cam = cam.normalize() * floor;
                    }
                }
                look(cam, eye + up * (v * 0.2) - cam, up)
            } else {
                look(eye, walker.view_dir(), up)
            };
            surface.cam_blend = (surface.cam_blend + dt / 0.8).min(1.0);
            cam_local_tf = blend_pose(&surface.cam_from, fps, smoothstep(surface.cam_blend));

            if surface.rescue {
                // Secours (A4) : directement au vaisseau
                surface.rescue = false;
                start_ascent(&mut surface, &params, fz, cam_local_tf);
                net.notify("Decollage...", now);
            } else if enter {
                // Le cockpit s'ouvre, le personnage y monte, puis le vaisseau décolle
                surface.board_from = surface.walker.pos;
                surface.board_to = cockpit(&surface, &ctx.models);
                surface.board_t = 0.0;
                surface.cam_from = cam_local_tf;
                surface.cam_blend = 0.0;
                surface.phase = Phase::Boarding;
            }
        }
        // ── Monter dans le vaisseau / en descendre ───────────────────────
        Phase::Boarding | Phase::Disembarking => {
            set_cursor(&mut ctx.windows, false);
            let boarding = surface.phase == Phase::Boarding;
            surface.board_t += dt;
            let wait = if boarding { BOARD_WAIT } else { UNBOARD_WAIT };
            let u = ((surface.board_t - wait) / BOARD_SECS).clamp(0.0, 1.0);
            let e = smoothstep(u);
            let (a, b) = (surface.board_from, surface.board_to);
            let up = ((a + b) * 0.5).normalize_or(Vec3::Y);
            let v = surface.terrain.as_ref().map_or(1.0, |t| t.voxel());
            // Un petit saut entre le sol et le cockpit ; le personnage disparaît dans le vaisseau
            let hop = v * 0.8 + a.distance(b) * 0.12;
            let p = a.lerp(b, e) + up * (std::f32::consts::PI * e).sin() * hop;
            let shown = if boarding { 1.0 - smoothstep((u - 0.8) / 0.2) } else { smoothstep(u / 0.2) };
            let heading = tangent(if boarding { b - a } else { surface.walker.heading }, up);
            surface.board_pose = Some(Transform { translation: p, rotation: look(Vec3::ZERO, heading, up).rotation, scale: Vec3::splat(v * shown.max(0.001)) });
            surface.walker_local = surface.board_pose;
            ship_local = Transform { translation: surface.ship_local, rotation: surface.ship_rot, scale: Vec3::splat(surface.ship_scale) };
            // Caméra de côté, qui voit le personnage et le cockpit
            let mid = (a + b) * 0.5;
            let side = (b - a).cross(up).normalize_or(up.any_orthonormal_vector());
            let dist = (a.distance(b) * 1.4).max(v * 8.0);
            let cam = mid + side * dist + up * (dist * 0.45);
            surface.cam_blend = (surface.cam_blend + dt / 0.8).min(1.0);
            cam_local_tf = blend_pose(&surface.cam_from, look(cam, mid - cam, up), smoothstep(surface.cam_blend));
            if surface.board_t >= wait + BOARD_SECS + 0.25 {
                if boarding {
                    start_ascent(&mut surface, &params, fz, cam_local_tf);
                    net.notify("Decollage...", now);
                } else {
                    surface.cam_from = cam_local_tf;
                    surface.cam_blend = 0.0;
                    surface.phase = Phase::Walking;
                    net.notify("ZQSD/WASD : marcher  Maj : courir  Espace : sauter  V : decoller", now);
                }
            }
        }
        // ── Navigation à basse altitude ──────────────────────────────────
        Phase::Flying => {
            set_cursor(&mut ctx.windows, false);
            let terrain = surface.terrain.take().unwrap();
            let r = surface.fpos.length();
            let up = surface.fpos / r;
            let mut heading = tangent(surface.heading, up);

            let (mut forward, mut turn, mut vertical, mut boost) = (0.0f32, 0.0f32, 0.0f32, false);
            if !ui_open {
                let k = &ctx.keys;
                let axis = |pos: &[KeyCode], neg: &[KeyCode]| {
                    pos.iter().any(|c| k.pressed(*c)) as i32 as f32 - neg.iter().any(|c| k.pressed(*c)) as i32 as f32
                };
                forward = axis(&[KeyCode::KeyW, KeyCode::ArrowUp], &[KeyCode::KeyS, KeyCode::ArrowDown]);
                turn = axis(&[KeyCode::KeyA, KeyCode::ArrowLeft], &[KeyCode::KeyD, KeyCode::ArrowRight]);
                vertical = axis(&[KeyCode::Space], &[KeyCode::ControlLeft]);
                boost = k.pressed(KeyCode::ShiftLeft);
                // Mesures (0.13 E3) : `SPACESPORE_TEST_FLY` = plein gaz tout droit avec Maj
                if let Ok(mode) = std::env::var("SPACESPORE_TEST_FLY") {
                    forward = 1.0;
                    boost = true;
                    // `climb` : en montant (de plus en plus vite, tuiles toujours plus loin)
                    if mode == "climb" {
                        vertical = 1.0;
                    }
                }
                if ctx.buttons.pressed(MouseButton::Right) {
                    let sens = ctx.settings.mouse_sensitivity * 0.01;
                    surface.fyaw -= look_delta.x * sens;
                    surface.fpitch = (surface.fpitch + look_delta.y * sens).clamp(-0.2, 1.4);
                }
                if wheel != 0.0 {
                    let ship_len = surface.ship_dims.real_scale(terrain.voxel()) * surface.ship_dims.icon_len;
                    surface.fdist = (surface.fdist * (-wheel * 0.12).exp()).max(ship_len * 1.2);
                }
                if vertical != 0.0 {
                    surface.fdescend = false;
                }
                // Vol suborbital : vers le point visé au centre de l'écran, s'il est loin
                if k.just_pressed(HOP_KEY) && surface.hop.is_none() && !terrain.params.gaseous && params.asteroid.is_none() {
                    let v = terrain.voxel();
                    let aim = *cam_local_tf.forward();
                    match ray_sphere(cam_local_tf.translation, aim, terrain.params.radius) {
                        None => net.notify("Saut suborbital : visez un point de la planete (centre de l'ecran).", now),
                        Some(d) => {
                            let to = (cam_local_tf.translation + aim * d).normalize();
                            let arc = up.angle_between(to) * terrain.params.radius;
                            if arc < HOP_MIN_VOXELS * v {
                                net.notify(&format!("Saut suborbital : le point vise est a {:.0} voxels, il en faut {HOP_MIN_VOXELS:.0}. Montez pour voir plus loin, ou volez-y.", arc / v), now);
                            } else {
                                let r1 = terrain.ground(to).top + HOVER_VOXELS * v;
                                let dur = (6.0 + arc / (3_000.0 * v)).clamp(6.0, 25.0);
                                surface.hop = Some(Hop { from: up, to, r0: r, r1, peak: (arc * 0.3).max(300.0 * v), t: 0.0, dur });
                                surface.fdescend = false;
                                net.notify(&format!("Saut suborbital : {:.0} voxels, {:.0} s.", arc / v, dur), now);
                            }
                        }
                    }
                }
            }

            // Cap, vitesse et altitude
            heading = (Quat::from_axis_angle(up, turn * 1.3 * dt) * heading).normalize();
            // Vitesses en voxels (règle 14) : plus vite en altitude (rase-mottes lent, haute
            // altitude rapide : la planète est 16 fois plus grande en voxels), Maj x4
            let v = terrain.voxel();
            let alt = ((r - terrain.floor(up, r).top) / v).max(0.0);
            let top_speed = (alt * 1.5).clamp(FLY_MIN_VOXELS, FLY_MAX_VOXELS) * v * if boost { 4.0 } else { 1.0 };
            surface.fspeed += (forward * top_speed - surface.fspeed) * (1.0 - (-2.0 * dt).exp());
            // Dans une géante (des dizaines de milliers d'unités d'atmosphère), on monte et
            // descend plus vite
            let climb = if terrain.params.gaseous { 400.0 * (terrain.params.radius / 8000.0).max(1.0) } else { alt.clamp(CLIMB_MIN_VOXELS, CLIMB_MAX_VOXELS) * v };
            surface.fvert += (vertical * climb - surface.fvert) * (1.0 - (-4.0 * dt).exp());
            let next = (surface.fpos + heading * surface.fspeed * dt).normalize();
            // Voxels 3D : le sol sous le vaisseau (on peut passer sous une arche) et le plafond
            let ground = terrain.floor(next, r).top;
            let roof = terrain.ceiling(next, ground + 1.0);
            // Astéroïde : on peut s'éloigner davantage pour circuler dans le champ
            let ceiling = match &params.asteroid {
                Some(s) => s.max_radius() * 3.0 + 1500.0,
                None => hover_radius(&params),
            };
            // Entrée en navigation : on descend d'abord à `HOVER_VOXELS` au-dessus du relief
            let mut r = r + surface.fvert * dt;
            if surface.fdescend {
                let above = r - ground - HOVER_VOXELS * v;
                if above > 0.1 * v {
                    r -= ((above * 2.0).clamp(4.0 * v, 2000.0) * dt).min(above);
                } else {
                    surface.fdescend = false;
                }
            }
            let real = surface.ship_dims.real_scale(terrain.voxel());
            let clearance = (surface.ship_dims.half_h * real + 0.7 * v).max(3.5 * v);
            let r = r.clamp(ground + clearance, ceiling.max(ground + 15.0 * v)).min((roof - clearance).max(ground + clearance));
            surface.fpos = next * r;
            surface.heading = tangent(heading, next);
            if let Some(mut hop) = surface.hop {
                hop.t += dt;
                let u = (hop.t / hop.dur).clamp(0.0, 1.0);
                let (pos, dir) = hop.at(u);
                surface.fpos = pos;
                surface.heading = dir;
                surface.fspeed = 0.0;
                surface.fvert = 0.0;
                surface.hop = (u < 1.0).then_some(hop);
            }

            // Vol bas : la vraie taille du vaisseau, la caméra assez loin pour le voir en entier
            let scale = real;
            surface.fdist = surface.fdist.max(real * surface.ship_dims.icon_len * 1.6);
            let ship_pos = surface.fpos;
            // Le vent fait rouler (vent de côté) et tanguer (vent de face) ; le pilote corrige quand
            // il manœuvre, et le vaisseau se stabilise quand le vent tombe
            let right = surface.heading.cross(next).normalize_or(Vec3::X);
            let w = surface.wind;
            let k = 0.02 * (1.0 + surface.gust);
            let mut want = Vec2::new((w.dot(surface.heading) * k).clamp(-0.3, 0.3), (-w.dot(right) * k).clamp(-0.4, 0.4));
            if forward != 0.0 || turn != 0.0 || vertical != 0.0 {
                want *= 0.35;
            }
            let tilt = surface.tilt;
            surface.tilt = tilt + (want - tilt) * (1.0 - (-2.5 * dt).exp());
            let ship_rot = look(Vec3::ZERO, surface.heading, next).rotation * Quat::from_rotation_z(surface.tilt.y) * Quat::from_rotation_x(surface.tilt.x);
            // Propulseurs : avancer (plus fort avec Maj), monter / descendre, tourner
            let thrust = (surface.fspeed / top_speed.max(1.0)).clamp(-1.0, 1.0) * if boost { 1.0 } else { 0.6 };
            surface.pilot = Vec3::new(0.0, vertical * 0.6, -thrust).clamp_length_max(1.0);
            surface.pilot_turn = turn;
            if surface.hop.is_some() {
                surface.pilot = Vec3::NEG_Z;
            }
            ship_local = Transform { translation: ship_pos, rotation: ship_rot, scale: Vec3::splat(scale) };

            // Caméra derrière le vaisseau, orientable à la souris, jamais sous le relief
            let back = Quat::from_axis_angle(next, surface.fyaw) * -surface.heading;
            let offset = (back * surface.fpitch.cos() + next * surface.fpitch.sin()) * surface.fdist;
            let mut cam_local = surface.fpos + offset;
            let floor = terrain.floor(cam_local.normalize(), cam_local.length()).top + 1.5 * terrain.voxel();
            if cam_local.length() < floor {
                cam_local = cam_local.normalize() * floor;
            }
            let cam_pos = cam_local;
            cam_local_tf = look(cam_pos, ship_pos + next * (0.4 * scale) - cam_pos, next);
            surface.terrain = Some(terrain);
            ctrl.distance = surface.fdist;

            // Dézoom au-delà de 1000 : retour à la vue orbitale, au-dessus de l'endroit survolé
            if surface.fdist >= fz {
                surface.hover = Some((kind, next));
                ctrl.distance = fz * 1.05;
                ctrl.zoom_goal = None;
                ctrl.last_target_pos = center;
                surface.abort();
                clear.0 = SPACE_SKY;
            } else if enter && params.gaseous {
                net.notify("Pas de sol : une geante gazeuse n'a pas de surface. Zoomez sous 1000 pour entrer dans son atmosphere (la pression abime la coque).", now);
            } else if enter {
                // Atterrir juste devant le vaisseau
                let terrain = surface.terrain.take().unwrap();
                let dir1 = (surface.fpos + surface.heading * scale * 6.0).normalize();
                let heading = surface.heading;
                let cam_from = cam_local_tf;
                begin_descent(&mut surface, kind, terrain, next, r, dir1, heading, scale, cam_from);
                net.notify("Atterrissage...", now);
            }
        }
        Phase::Orbit => {}
    }
    // Retour au monde (sauf si l'on vient de quitter l'astre : la vue orbitale reprend la main)
    if surface.active() {
        *ship_tf = frame.to_world(ship_local);
        *cam_tf = frame.to_world(cam_local_tf);
    }
    surface.walker_world = match (surface.phase, surface.walker_local) {
        (Phase::Walking | Phase::Boarding | Phase::Disembarking, Some(w)) => Some(frame.to_world(w)),
        _ => None,
    };

    // Ciel : couleur de l'atmosphère le jour (coucher de soleil près de l'horizon), noir dans l'espace
    if surface.active() {
        let cam_local = cam_tf.translation - center;
        let up = cam_local.normalize_or(Vec3::Y);
        let altitude = cam_local.length() - params.radius;
        // Tous les soleils (étoiles doubles, C3) : chacun teinte le ciel selon sa hauteur et son
        // éclat ; deux couchers de soleil se suivent
        let suns = sun_list(&ctx.settings, ctx.stars.iter().map(|(t, id)| (t.translation, id.0)), center, &ctx.dim);
        let air = (1.0 - altitude / atmosphere_depth(&params)).clamp(0.0, 1.0);
        let space = SPACE_SKY.to_srgba();
        let space = [space.red, space.green, space.blue];
        let (c, day, height) = combined_sky(&params, &suns, up, air, space);
        // Météo (C5) : ciel gris sous les nuages, brun dans la poussière, blanc dans l'éclair
        let w = &ctx.weather.sample;
        let grey = (c[0] + c[1] + c[2]) / 3.0 * 0.85;
        let mut c = c.map(|x| x + (grey - x) * (0.7 * w.cloud + 0.3 * w.fog).min(0.85));
        c = [0, 1, 2].map(|i| c[i] + ([0.55, 0.4, 0.25][i] * day - c[i]) * 0.8 * w.dust);
        let flash = ctx.weather.flash_at(ctx.clock.secs) * 0.6;
        let c = c.map(|x| (x + flash).min(1.0));
        // Sous terre : noir (on ne voit plus le ciel)
        let dark = 1.0 - surface.underground;
        clear.0 = Color::srgb(c[0] * dark, c[1] * dark, c[2] * dark);
        surface.daylight = day;
        surface.sun_height = height;
        if surface.dark() && !surface.night_told && !params.gaseous {
            surface.night_told = true;
            let what = if surface.phase == Phase::Walking { "lampe" } else { "phares" };
            net.notify(&format!("Il fait nuit ici. N : allumer / eteindre la {what}."), now);
        }
        if ctx.keys.just_pressed(LAMP_KEY) && !ui_open {
            surface.lamps = !surface.lamps;
            net.notify(if surface.lamps { "Lampe et phares : automatiques (allumes dans le noir)." } else { "Lampe et phares eteints." }, now);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  Plusieurs soleils (C3)
// ─────────────────────────────────────────────────────────────────────────

/// Éclairement (lux) d'une étoile à la distance `d` : celui de sa lumière ponctuelle (même portée).
pub fn star_lux(cfg: &crate::settings::StarConfig, d: f32) -> f32 {
    let x = (d / cfg.light_range.max(1.0)).min(1.0);
    let window = (1.0 - x.powi(4)).max(0.0).powi(2);
    cfg.lumens() / (4.0 * std::f32::consts::PI * d.max(1.0).powi(2)) * window
}

/// Soleils vus depuis `center` : (direction vers l'étoile, éclat relatif au plus brillant, lux),
/// du plus brillant au plus faible.
pub fn sun_list(settings: &GameSettings, stars: impl Iterator<Item = (Vec3, usize)>, center: Vec3, dim: &crate::sky::SunDim) -> Vec<(Vec3, f32, f32)> {
    let mut out: Vec<(Vec3, f32, f32)> = stars
        .filter_map(|(pos, id)| {
            let cfg = settings.systems.get(id / 1000)?.stars.get(id % 1000)?;
            let d = pos - center;
            // Éclipse (C4) : une lune devant le soleil
            Some((d.normalize_or(Vec3::Y), 0.0, star_lux(cfg, d.length()) * dim.factor(id)))
        })
        .collect();
    out.sort_by(|a, b| b.2.total_cmp(&a.2));
    let max = out.first().map_or(1.0, |s| s.2.max(1e-6));
    for s in &mut out {
        s.1 = s.2 / max;
    }
    out
}

/// Ciel de tous les soleils : couleur, jour (0..1) et hauteur du plus haut soleil qui compte.
pub fn combined_sky(p: &BodyParams, suns: &[(Vec3, f32, f32)], up: Vec3, air: f32, space: [f32; 3]) -> ([f32; 3], f32, f32) {
    let (mut c, mut day, mut height) = (space, 0.0f32, -1.0f32);
    for &(dir, w, _) in suns {
        let h = up.dot(dir);
        let ci = sky_color(p, h, air, space);
        for k in 0..3 {
            c[k] += (ci[k] - space[k]) * w;
        }
        day += daylight(p, h, air) * w;
        if w > 0.15 {
            height = height.max(h);
        }
    }
    (c.map(|x| x.clamp(0.0, 1.0)), day.min(1.0), height)
}

/// Lumières directionnelles des soleils près d'un astre : une par étoile, avec ombres.
#[derive(Component)]
struct SurfaceSun(usize);

const MAX_SUNS: usize = 3;

fn spawn_suns(mut commands: Commands) {
    for i in 0..MAX_SUNS {
        commands.spawn((DirectionalLight { illuminance: 0.0, shadows_enabled: false, ..default() }, Transform::default(), SurfaceSun(i)));
    }
}

#[allow(clippy::type_complexity)]
fn update_suns(
    mut commands: Commands,
    surface: Res<Surface>,
    settings: Res<GameSettings>,
    stars: Query<(&Transform, &StarId), (With<StarRoot>, Without<SurfaceSun>)>,
    dim: Res<crate::sky::SunDim>,
    mut suns: Query<(Entity, &SurfaceSun, &mut DirectionalLight, &mut Transform)>,
    mut cascades: Local<f32>,
) {
    let list = match (surface.suns_on(), surface.center()) {
        (true, Some(center)) => sun_list(&settings, stars.iter().map(|(t, id)| (t.translation, id.0)), center, &dim),
        _ => Vec::new(),
    };
    // Ombres jusqu'à ~200 voxels autour de la caméra (le marcheur, le vaisseau posé, le décor)
    let voxel = surface.terrain.as_ref().map_or(10.0, |t| t.voxel());
    let reshape = (*cascades - voxel).abs() > 1e-3 || settings.is_changed();
    let k = 1.0 - 0.985 * surface.underground();
    for (e, sun, mut light, mut tf) in &mut suns {
        if reshape {
            // Ultra : une 4e cascade jusqu'à 4 000 voxels pour les ombres des montagnes (T4)
            let (cascades, far) = if crate::graphics::terrain_detail(&settings).3 { (4, 4000.0) } else { (3, 200.0) };
            commands.entity(e).try_insert(
                bevy::pbr::CascadeShadowConfigBuilder { num_cascades: cascades, minimum_distance: voxel * 0.5, first_cascade_far_bound: voxel * 25.0, maximum_distance: voxel * far, overlap_proportion: 0.2 }.build(),
            );
        }
        let (illuminance, shadows) = match list.get(sun.0) {
            Some(&(dir, w, lux)) if w > 0.02 => {
                // La lumière va de l'étoile vers l'astre
                *tf = Transform::default().looking_to(-dir, if dir.y.abs() > 0.99 { Vec3::X } else { Vec3::Y });
                (lux * k, settings.shadows)
            }
            _ => (0.0, false),
        };
        if (light.illuminance - illuminance).abs() > light.illuminance.max(illuminance) * 1e-3 {
            light.illuminance = illuminance;
        }
        if light.shadows_enabled != shadows {
            light.shadows_enabled = shadows;
        }
    }
    if reshape {
        *cascades = voxel;
    }
}

/// Jour du ciel (0 : nuit, 1 : plein jour) : l'étoile au-dessus de l'horizon, sous une
/// atmosphère (sans air, le ciel reste noir même en plein jour).
pub fn daylight(p: &BodyParams, sun_height: f32, air: f32) -> f32 {
    if !p.atmosphere || p.pressure < 0.01 {
        return 0.0;
    }
    smoothstep((sun_height + 0.15) / 0.35) * air
}

/// Clair de lune (0 à 1) vu d'un point du sol : chaque astre au-dessus de l'horizon renvoie la
/// lumière de l'étoile selon sa phase (part éclairée vue d'ici) et sa taille apparente. Une pleine
/// lune comme la nôtre donne ~0,15 ; une grosse lune proche, davantage.
pub fn moonlight(eye: Vec3, up: Vec3, star: Vec3, bodies: &[(Vec3, f32)]) -> f32 {
    let mut total = 0.0;
    for &(center, radius) in bodies {
        let to_body = center - eye;
        let d = to_body.length();
        if d <= radius * 1.01 {
            continue;
        }
        let dir = to_body / d;
        // Sous l'horizon (on garde le bord du disque)
        if up.dot(dir) < -radius / d {
            continue;
        }
        // Phase : angle, vu de l'astre, entre l'étoile et nous ; part éclairée (1 + cos) / 2
        let to_star = (star - center).normalize_or(Vec3::X);
        let to_eye = -dir;
        let lit = (1.0 + to_star.dot(to_eye)) * 0.5;
        // Taille apparente par rapport à notre Lune (rayon angulaire ~ 1/220)
        let size = (radius / d * 220.0).powi(2);
        total += 0.15 * lit * size;
    }
    total.min(1.0)
}

// ─────────────────────────────────────────────────────────────────────────
//  Galaxie estompée près d'un astre
// ─────────────────────────────────────────────────────────────────────────

/// Part de l'éclat de la galaxie principale (étoiles, bras, nuages) conservée : 1 loin de tout
/// astre, de plus en plus faible à mesure que la caméra s'approche d'une planète ou d'une lune
/// (le ciel d'un astre n'est plus noyé sous les couleurs de la galaxie).
#[derive(Resource)]
pub struct GalaxyDim(pub f32);

impl Default for GalaxyDim {
    fn default() -> Self {
        Self(1.0)
    }
}

/// À cette distance de l'astre (en rayons, depuis sa surface), l'estompage commence.
const DIM_RADII: f32 = 12.0;
/// Part de l'éclat conservée au ras de la surface.
const DIM_FLOOR: f32 = 0.08;

fn update_galaxy_dim(
    surface: Res<Surface>,
    target: Res<CameraTarget>,
    settings: Res<GameSettings>,
    planets: Query<(&Transform, &PlanetId), With<PlanetRoot>>,
    moons: Query<(&Transform, &MoonId), With<MoonRoot>>,
    asteroids: Res<crate::asteroids::AsteroidField>,
    cam_q: Query<&Transform, (With<Camera3d>, Without<PlanetRoot>, Without<MoonRoot>)>,
    mut dim: ResMut<GalaxyDim>,
) {
    let Ok(cam) = cam_q.get_single() else { return };
    let body = match target.0 {
        TargetKind::Planet(id) => planets.iter().find(|(_, p)| p.0 == id).map(|(t, _)| t.translation),
        TargetKind::Moon(pid, mi) => moons.iter().find(|(_, m)| m.planet_idx == pid && m.moon_idx == mi).map(|(t, _)| t.translation),
        TargetKind::Asteroid(key) => asteroids.pose(&key).map(|t| t.translation),
        _ => None,
    };
    let radius = body_params(&settings, &target.0).map(|p| p.radius);
    let keep = match (body, radius) {
        (Some(center), Some(r)) => {
            let altitude = (cam.translation.distance(center) - r).max(0.0);
            let closeness = (1.0 - altitude / (DIM_RADII * r)).clamp(0.0, 1.0);
            // Au sol, la nuit (ou sans air), les étoiles et la galaxie reviennent
            let floor = if surface.active() { DIM_FLOOR + (NIGHT_GALAXY - DIM_FLOOR) * (1.0 - surface.daylight()) } else { DIM_FLOOR };
            1.0 - (1.0 - floor) * smoothstep(closeness)
        }
        _ => 1.0,
    };
    if (dim.0 - keep).abs() > 0.002 {
        dim.0 = keep;
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  Lumière
// ─────────────────────────────────────────────────────────────────────────

/// Lumière diffuse du ciel pendant un séjour : de jour sous une atmosphère seulement (la nuit, et
/// sans air, il n'y a que la lumière directe de l'étoile).
#[allow(clippy::too_many_arguments)]
fn surface_light(
    surface: Res<Surface>,
    settings: Res<GameSettings>,
    mut ambient: ResMut<AmbientLight>,
    planets: Query<(&Transform, &PlanetId), With<PlanetRoot>>,
    moons: Query<(&Transform, &MoonId), With<MoonRoot>>,
    stars: Query<(&Transform, &StarId), With<StarRoot>>,
    asteroids: Res<crate::asteroids::AsteroidField>,
    dim: Res<crate::sky::SunDim>,
    (weather, clock): (Res<crate::weather::WeatherNow>, Res<crate::world_clock::WorldClock>),
    cam_q: Query<&Transform, (With<Camera3d>, Without<PlanetRoot>, Without<MoonRoot>, Without<StarRoot>)>,
    mut was_active: Local<bool>,
) {
    if !surface.active() {
        if *was_active {
            *was_active = false;
            ambient.brightness = AMBIENT_SPACE;
            ambient.color = AMBIENT_SPACE_TINT;
        }
        return;
    }
    *was_active = true;
    let (Some(kind), Some(params), Ok(cam)) = (surface.body, surface.params(), cam_q.get_single()) else { return };
    let center = match kind {
        TargetKind::Planet(id) => planets.iter().find(|(_, p)| p.0 == id).map(|(t, _)| t.translation),
        TargetKind::Moon(planet_idx, moon_idx) => moons
            .iter()
            .find(|(_, m)| m.planet_idx == planet_idx && m.moon_idx == moon_idx)
            .map(|(t, _)| t.translation),
        TargetKind::Asteroid(key) => asteroids.pose(&key).map(|t| t.translation),
        _ => None,
    };
    let Some(center) = center else { return };
    let suns = sun_list(&settings, stars.iter().map(|(t, id)| (t.translation, id.0)), center, &dim);
    // Le plus brillant des soleils (clair de lune : la lumière vient surtout de lui)
    let Some(&(to_star, _, _)) = suns.first() else { return };
    let cam_local = cam.translation - center;
    let up = cam_local.normalize_or(Vec3::Y);
    let altitude = cam_local.length() - params.radius;
    let air = (1.0 - altitude / atmosphere_depth(&params)).clamp(0.0, 1.0);
    let (_, day, _) = combined_sky(&params, &suns, up, air, [0.0; 3]);
    // Au sol, la nuit est noire (lueur des étoiles) ; en s'élevant, on retrouve l'éclairage de
    // l'espace. Le clair de lune s'y ajoute.
    let ground = (1.0 - altitude / (params.radius * 0.5).max(1.0)).clamp(0.0, 1.0);
    let night = AMBIENT_SPACE + (AMBIENT_NIGHT - AMBIENT_SPACE) * ground;
    let star = center + to_star * 1.0e7;
    let mut bodies: Vec<(Vec3, f32)> = Vec::new();
    for (t, pid) in &planets {
        if Some(TargetKind::Planet(pid.0)) == surface.body {
            continue;
        }
        if let Some(p) = settings.systems.get(pid.0 / 1000).and_then(|s| s.planets().get(pid.0 % 1000)) {
            bodies.push((t.translation, p.radius));
        }
    }
    for (t, mid) in &moons {
        if Some(TargetKind::Moon(mid.planet_idx, mid.moon_idx)) == surface.body {
            continue;
        }
        let r = settings
            .systems
            .get(mid.planet_idx / 1000)
            .and_then(|s| s.planets().get(mid.planet_idx % 1000))
            .and_then(|p| p.moons.get(mid.moon_idx))
            .map_or(0.0, |m| m.radius);
        bodies.push((t.translation, r));
    }
    let moon = moonlight(cam.translation, up, star, &bodies) * (1.0 - day);
    // Une atmosphère épaisse diffuse plus de lumière ; la teinte est celle du ciel
    let sky_light = (AMBIENT_DAY - night) * day * params.pressure.clamp(0.05, 4.0).powf(0.25);
    // Éclair (C5) : un flash bref
    let flash = weather.flash_at(clock.secs) * 4_000.0;
    ambient.brightness = (night + sky_light + MOONLIGHT_MAX * moon + flash) * (1.0 - 0.95 * surface.underground());
    ambient.color = if day > 0.05 {
        let s = params.sky;
        Color::srgb(0.4 + 0.3 * s[0], 0.4 + 0.3 * s[1], 0.4 + 0.3 * s[2])
    } else if moon > 0.02 {
        MOONLIGHT_TINT
    } else {
        AMBIENT_SPACE_TINT
    };
}

// ─────────────────────────────────────────────────────────────────────────
//  Lampe du marcheur et phares du vaisseau
// ─────────────────────────────────────────────────────────────────────────

#[derive(Component)]
struct Lamp {
    /// Phares du vaisseau (sinon, lampe du marcheur).
    headlight: bool,
}

fn spawn_lamps(mut commands: Commands) {
    for headlight in [false, true] {
        commands.spawn((
            SpotLight {
                intensity: 0.0,
                range: 1_000.0,
                outer_angle: if headlight { 0.55 } else { 0.5 },
                inner_angle: if headlight { 0.3 } else { 0.25 },
                shadows_enabled: false,
                color: Color::srgb(1.0, 0.95, 0.85),
                ..default()
            },
            Transform::IDENTITY,
            Visibility::Hidden,
            Lamp { headlight },
        ));
    }
}

/// Plan proche de la caméra (0.13 E2) : 0,1 voxel au sol (le marcheur fait 2 voxels), 0,1 unité
/// ailleurs. La projection de Bevy est en profondeur inversée : rien à perdre au loin.
fn near_plane(surface: Res<Surface>, mut cams: Query<&mut Projection, With<Camera3d>>) {
    let near = surface.terrain.as_ref().filter(|_| surface.active()).map_or(0.1, |t| 0.1 * t.voxel());
    for mut p in &mut cams {
        if let Projection::Perspective(pp) = &mut *p {
            if (pp.near - near).abs() > 1e-4 {
                pp.near = near;
            }
        }
    }
}

/// La distance de la caméra à pied (molette) est reprise au lancement et enregistrée (C4).
fn remember_walker_cam(mut surface: ResMut<Surface>, mut settings: ResMut<GameSettings>, mut started: Local<bool>, mut since: Local<f32>, time: Res<Time>) {
    if !*started {
        *started = true;
        surface.cam_dist = settings.walker_cam.clamp(3.0, 12.0);
        return;
    }
    if (settings.walker_cam - surface.cam_dist).abs() < 0.01 {
        *since = 0.0;
        return;
    }
    // Enregistré une seconde après le dernier coup de molette
    *since += time.delta_secs();
    if *since > 1.0 {
        settings.walker_cam = surface.cam_dist;
        settings.save();
        *since = 0.0;
    }
}

/// Dans le noir, la lampe suit le regard du marcheur ; en vol, les phares éclairent devant et
/// sous le vaisseau. Leur puissance est réglée sur la lumière de l'étoile à midi (moitié de
/// celle-ci à quelques mètres), quelle que soit la distance de la planète à son étoile.
#[allow(clippy::type_complexity)]
fn update_lamps(
    surface: Res<Surface>,
    stars: Query<(&GlobalTransform, &PointLight, Option<&StarLightBase>), Without<Lamp>>,
    ship_q: Query<&Transform, (With<Ship>, Without<Lamp>, Without<Camera3d>)>,
    cam_q: Query<&Transform, (With<Camera3d>, Without<Lamp>, Without<Ship>)>,
    mut lamps: Query<(&Lamp, &mut SpotLight, &mut Transform, &mut Visibility), (Without<Ship>, Without<Camera3d>)>,
    hand_q: Query<(&GlobalTransform, &crate::models::Rig), With<crate::models::WalkerRig>>,
) {
    let walking = surface.phase == Phase::Walking;
    let on = surface.lamps && surface.dark() && !surface.gaseous();
    let (Ok(ship), Ok(cam)) = (ship_q.get_single(), cam_q.get_single()) else { return };
    let voxel = surface.terrain.as_ref().map_or(10.0, |t| t.voxel());
    // Éclairement de l'étoile au niveau de la planète (lux), d'après la plus brillante
    let sun = stars
        .iter()
        .map(|(gt, l, base)| base.map_or(l.intensity, |b| b.0) / (4.0 * std::f32::consts::PI * gt.translation().distance_squared(cam.translation).max(1.0)))
        .fold(0.0f32, f32::max)
        .max(20.0);
    for (lamp, mut light, mut tf, mut vis) in &mut lamps {
        let wanted = on && (lamp.headlight != walking);
        let v = if wanted { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != v {
            *vis = v;
        }
        if !wanted {
            continue;
        }
        let (reach, place) = if lamp.headlight {
            // Devant le vaisseau, inclinés de 25° vers le sol
            let scale = ship.scale.x;
            let up = *ship.up();
            let dir = (*ship.forward() * 0.9 - up * 0.42).normalize();
            (scale * 25.0 + 9.0 * voxel, Transform::from_translation(ship.translation + *ship.forward() * scale * 1.2).looking_to(dir, up))
        } else {
            // La lampe est tenue dans la main droite : elle éclaire là où l'on regarde (de face :
            // là où regarde le personnage)
            let looking = match (surface.view, surface.walker_world) {
                (2, Some(w)) => *w.forward(),
                _ => *cam.forward(),
            };
            let hand = hand_q.get_single().ok().filter(|_| surface.view != 0).and_then(|(gt, rig)| rig.hand.map(|h| gt.transform_point(h)));
            let at = hand.unwrap_or(cam.translation + *cam.right() * voxel * 0.35 + *cam.down() * voxel * 0.3);
            (voxel * 6.0, Transform::from_translation(at + looking * voxel * 0.2).looking_to(looking, *cam.up()))
        };
        *tf = place;
        light.intensity = 0.5 * sun * 4.0 * std::f32::consts::PI * reach * reach;
        light.range = reach * 12.0;
    }
}

/// Sous terre : de la roche au-dessus de la tête et sous la surface du relief. La lumière de
/// l'étoile (sans ombres) traverserait la roche : on l'éteint sous terre.
fn update_underground(time: Res<Time>, mut surface: ResMut<Surface>) {
    let dt = time.delta_secs().min(0.1);
    let target = match (&surface.terrain, surface.phase) {
        (Some(t), Phase::Walking | Phase::Flying) => {
            let p = if surface.phase == Phase::Walking { surface.walker.pos } else { surface.fpos };
            let up = p.normalize_or(Vec3::Y);
            let r = p.length();
            let v = t.voxel();
            let below = t.base_column(up, v).top - r;
            if below > 2.0 * v && t.ceiling(up, r).is_finite() { 1.0 } else { 0.0 }
        }
        _ => 0.0,
    };
    let u = surface.underground + (target - surface.underground) * (1.0 - (-3.0 * dt).exp());
    if (u - surface.underground).abs() > 1e-4 {
        surface.underground = u;
    }
}

/// Intensité de départ d'une lumière d'étoile (pour l'éteindre sous terre et la rallumer).
#[derive(Component)]
struct StarLightBase(f32);

fn dim_star_light(
    mut commands: Commands,
    surface: Res<Surface>,
    roots: Query<(), With<StarRoot>>,
    mut lights: Query<(Entity, &mut PointLight, Option<&StarLightBase>, Option<&Parent>)>,
) {
    let k = 1.0 - 0.985 * surface.underground();
    for (e, mut light, base, parent) in &mut lights {
        let Some(base) = base else {
            commands.entity(e).try_insert(StarLightBase(light.intensity));
            continue;
        };
        // Près d'un astre solide, les soleils directionnels (avec ombres) éclairent à la place
        let star = parent.is_some_and(|p| roots.contains(p.get()));
        let wanted = if star && surface.suns_on() { 0.0 } else { base.0 * k };
        if (light.intensity - wanted).abs() > base.0 * 1e-4 {
            light.intensity = wanted;
        }
    }
}

/// Grotte la plus proche (scanner) : entrée, sorte, distance et direction.
#[derive(Resource, Default)]
pub struct NearestCave {
    pub text: String,
    entrance: Option<Vec3>,
    last: f64,
}

fn find_nearest_cave(time: Res<Time>, surface: Res<Surface>, mut nearest: ResMut<NearestCave>) {
    let now = time.elapsed_secs_f64();
    if now - nearest.last < 2.0 {
        return;
    }
    nearest.last = now;
    let (Some(t), Some(p)) = (surface.terrain.as_ref(), surface.local_point()) else {
        if !nearest.text.is_empty() {
            *nearest = NearestCave { last: now, ..default() };
        }
        return;
    };
    let Some(caves) = t.caves.as_ref() else { return };
    let p = p.normalize_or(Vec3::Y) * t.surface_r(p.normalize_or(Vec3::Y));
    let found = caves.nearest_entrance(p, 6, &|d| t.surface_r(d));
    nearest.entrance = found.map(|(e, _)| e);
    nearest.text = match found {
        Some((e, kind)) => {
            let up = p.normalize();
            let north = (Vec3::Y - up * up.y).normalize_or(Vec3::Z);
            let east = north.cross(up);
            let d = e - p;
            let angle = d.dot(east).atan2(d.dot(north)).to_degrees().rem_euclid(360.0);
            let names = ["nord", "nord-est", "est", "sud-est", "sud", "sud-ouest", "ouest", "nord-ouest"];
            let side = names[((angle / 45.0).round() as usize) % 8];
            format!("Grotte la plus proche : {} a {:.0} ({side})  /grotte : y aller", kind.name(), d.length())
        }
        None => "Aucune grotte connue a proximite.".into(),
    };
}

/// `/grotte` : aller au bord de l'entrée de grotte la plus proche.
#[derive(Event)]
pub struct CaveCommand;

fn go_cave(time: Res<Time>, mut events: EventReader<CaveCommand>, mut surface: ResMut<Surface>, mut nearest: ResMut<NearestCave>, mut net: ResMut<Net>) {
    let now = time.elapsed_secs_f64();
    for _ in events.read() {
        let found = match (surface.terrain.as_ref(), surface.local_point()) {
            (Some(t), Some(p)) => t.caves.as_ref().and_then(|c| {
                let p = p.normalize_or(Vec3::Y) * t.surface_r(p.normalize_or(Vec3::Y));
                c.nearest_entrance(p, 10, &|d| t.surface_r(d))
            }),
            _ => None,
        };
        let (Some((e, kind)), Some(t)) = (found, surface.terrain.as_ref()) else {
            net.notify("Pas de grotte trouvee : posez-vous ou volez bas sur un astre solide (zoom sous 1000).", now);
            continue;
        };
        let v = t.voxel();
        let up = e.normalize();
        let side = Vec3::Y.cross(up).normalize_or(Vec3::X);
        // Au bord du puits, en le regardant
        let rim = (e + side * 7.0 * v).normalize();
        match surface.phase {
            Phase::Walking => {
                let w = Walker::spawn(t, rim, -side);
                surface.walker = w;
                net.notify(&format!("Au bord d'une entree ({}). Lampe : N.", kind.name()), now);
            }
            Phase::Flying => {
                surface.fpos = up * (t.ground(up).top.max(e.length()) + 60.0 * v);
                surface.fdescend = false;
                net.notify(&format!("Une entree ({}) est sous le vaisseau (V pour se poser).", kind.name()), now);
            }
            _ => net.notify("Attendez la fin de l'atterrissage.", now),
        }
        nearest.last = 0.0;
    }
}

/// Indicateurs visuels de l'espace (orbites, traînées des planètes, cercles autour des astres,
/// portée du vaisseau, trous de ver, zones) : un groupe de traits à part, qui s'efface en douceur
/// quand on s'approche d'une planète ou d'une lune, et revient quand on remonte.
#[derive(Default, Reflect, GizmoConfigGroup)]
pub struct IndicatorGizmos;

/// Épaisseur des traits des indicateurs au loin (pixels).
const INDICATOR_WIDTH: f32 = 2.0;

fn fade_indicators(
    time: Res<Time>,
    target: Res<CameraTarget>,
    settings: Res<GameSettings>,
    surface: Res<Surface>,
    planets: Query<(&Transform, &PlanetId), With<PlanetRoot>>,
    moons: Query<(&Transform, &MoonId), With<MoonRoot>>,
    asteroids: Res<crate::asteroids::AsteroidField>,
    cam_q: Query<&Transform, (With<Camera3d>, Without<PlanetRoot>, Without<MoonRoot>)>,
    mut store: ResMut<GizmoConfigStore>,
    mut fade: Local<Option<f32>>,
) {
    let Ok(cam) = cam_q.get_single() else { return };
    let kind = surface.body().unwrap_or(target.0);
    let body = match kind {
        TargetKind::Planet(id) => planets.iter().find(|(_, p)| p.0 == id).map(|(t, _)| t.translation),
        TargetKind::Moon(pid, mi) => moons.iter().find(|(_, m)| m.planet_idx == pid && m.moon_idx == mi).map(|(t, _)| t.translation),
        TargetKind::Asteroid(key) => asteroids.pose(&key).map(|t| t.translation),
        _ => None,
    };
    // Altitude en rayons de l'astre : tout disparaît sous 0,3 rayon, tout revient au-dessus de 2
    let wanted = match (body, body_params(&settings, &kind)) {
        (Some(center), Some(p)) if !p.gaseous || surface.active() => {
            let altitude = (cam.translation.distance(center) - p.radius).max(0.0) / p.radius.max(1.0);
            smoothstep((altitude - 0.3) / 1.7)
        }
        _ => 1.0,
    };
    // En douceur (environ une seconde)
    let current = fade.unwrap_or(wanted);
    let next = current + (wanted - current) * (1.0 - (-3.0 * time.delta_secs()).exp());
    *fade = Some(next);
    let (config, _) = store.config_mut::<IndicatorGizmos>();
    config.enabled = next > 0.02;
    config.line_width = INDICATOR_WIDTH * next;
}

/// Les cellules modifiées ont changé (impact de météorite, delta reçu d'un autre joueur).
#[derive(Resource, Default)]
pub struct VoxelsChanged(pub bool);

/// Prend en compte des cellules modifiées : le sol (collisions) et les tuiles, reconstruites une
/// à une (les anciennes restent affichées en attendant).
fn refresh_voxels(settings: Res<GameSettings>, mut changed: ResMut<VoxelsChanged>, mut surface: ResMut<Surface>, mut store: ResMut<TileStore>) {
    if !changed.0 {
        return;
    }
    changed.0 = false;
    let Some(kind) = surface.body else { return };
    let voxels = crate::voxel::body_voxels(&settings, &kind);
    if let Some(t) = surface.terrain.as_mut() {
        t.set_voxels(voxels.clone());
    }
    if store.body == Some(kind) {
        store.voxels = voxels;
        store.generation = store.generation.wrapping_add(1);
    }
}

/// `/surplomb` (None) : aller à l'arche de test (voxels 3D) de l'astre où l'on se trouve ;
/// `/relief [forme]` (Some) : à la forme du relief la plus proche (`rocks::Feature`, T2).
#[derive(Event)]
pub struct OverhangCommand(pub Option<String>);

/// `/relief [forme]` : aller voir la forme du relief la plus proche (corniche, piton, gorge...).
fn go_relief(t: &Terrain, surface: &mut Surface, arg: &str) -> Result<String, String> {
    let rocks = t.rocks.clone().ok_or("Pas de formes du relief sur cet astre.")?;
    let want = if arg.trim().is_empty() {
        None
    } else {
        Some(crate::rocks::Feature::parse(arg).ok_or_else(|| {
            format!("Formes : {}", crate::rocks::Feature::ALL.map(|f| f.name()).join(", "))
        })?)
    };
    let here = surface.local_point().ok_or("Posez-vous ou volez bas d'abord.")?.normalize_or(Vec3::Y);
    let (kind, at, look) = rocks.nearest(here, want, 40, &|d| t.rock_ground(d)).ok_or("Aucune forme de ce genre pres d'ici.")?;
    let v = t.voxel();
    let dir = at.normalize();
    match surface.phase {
        Phase::Walking => {
            let floor = t.floor(dir, f32::INFINITY).top;
            let heading = (look - at).normalize_or(Vec3::X);
            let mut w = Walker::spawn(t, dir, heading);
            w.pos = dir * floor;
            w.eye_r = floor + v * EYE_VOXELS;
            surface.walker = w;
            Ok(format!("Devant : {}.", kind.name()))
        }
        Phase::Flying => {
            surface.fpos = dir * (t.ground(dir).top + 40.0 * v);
            surface.fdescend = false;
            Ok(format!("{} sous le vaisseau (V pour se poser).", kind.name()))
        }
        _ => Err("Attendez la fin de l'atterrissage.".into()),
    }
}

fn go_overhang(time: Res<Time>, mut events: EventReader<OverhangCommand>, mut surface: ResMut<Surface>, mut net: ResMut<Net>) {
    let now = time.elapsed_secs_f64();
    for ev in events.read() {
        if let Some(arg) = &ev.0 {
            let Some(t) = surface.terrain.take() else {
                net.notify("Posez-vous ou volez bas sur une planete ou une lune solide d'abord (zoom sous 1000).", now);
                continue;
            };
            let msg = go_relief(&t, &mut surface, arg);
            surface.terrain = Some(t);
            match msg {
                Ok(m) | Err(m) => net.notify(&m, now),
            }
            continue;
        }
        let Some(o) = surface.terrain.as_ref().and_then(|t| t.overhang) else {
            net.notify("Posez-vous ou volez bas sur une planete ou une lune solide d'abord (zoom sous 1000).", now);
            continue;
        };
        let Some(t) = surface.terrain.as_ref() else { continue };
        let v = t.voxel();
        match surface.phase {
            Phase::Walking => {
                // Sous l'auvent, regardant vers l'arche
                let dir = o.visit_dir();
                let floor = t.floor(dir, o.base + 3.0 * v).top;
                let heading = (o.dir - dir).normalize_or(Vec3::X);
                let mut w = Walker::spawn(t, dir, heading);
                w.pos = dir * floor;
                w.eye_r = floor + v * EYE_VOXELS;
                surface.walker = w;
                net.notify("Vous etes sous l'auvent de l'arche de test (voxels 3D).", now);
            }
            Phase::Flying => {
                surface.fpos = o.dir * (o.base + 30.0 * v);
                surface.fdescend = false;
                net.notify("L'arche de test est juste sous le vaisseau (V pour s'y poser).", now);
            }
            _ => net.notify("Attendez la fin de l'atterrissage.", now),
        }
    }
}

/// Saison (et heure, pour le givre du matin) du terrain où l'on séjourne : neige et calottes
/// avancent et reculent ; les tuiles sont reconstruites quand elle change vraiment.
fn update_season(
    clock: Res<crate::world_clock::WorldClock>,
    settings: Res<GameSettings>,
    mut surface: ResMut<Surface>,
    planets: Query<(&Transform, &PlanetId), With<PlanetRoot>>,
    moons: Query<(&Transform, &MoonId), With<MoonRoot>>,
    stars: Query<&Transform, With<StarRoot>>,
) {
    let Some(kind) = surface.body() else { return };
    let Some((spin, _, _)) = crate::world_clock::body_spin(&settings, &kind) else { return };
    let tf = match kind {
        TargetKind::Planet(id) => planets.iter().find(|(_, p)| p.0 == id).map(|(t, _)| *t),
        TargetKind::Moon(pid, mi) => moons.iter().find(|(_, m)| m.planet_idx == pid && m.moon_idx == mi).map(|(t, _)| *t),
        _ => None,
    };
    let Some(tf) = tf else { return };
    let Some(star) = stars.iter().map(|s| s.translation).min_by(|a, b| a.distance_squared(tf.translation).total_cmp(&b.distance_squared(tf.translation))) else { return };
    let Some(terrain) = surface.terrain.as_mut() else { return };
    let p = &mut terrain.params;
    let mut season = spin.season(clock.secs, p.climate.mean_c);
    season.sun_lon = Some(crate::world_clock::sun_longitude(tf.rotation, star - tf.translation));
    // L'heure ne compte que pour le givre (de l'eau, de l'air)
    let frost = p.atmosphere && !p.airless && p.hydro.snow;
    let season = season.quantized(frost);
    if p.climate.season != season {
        p.climate = p.climate.at(season);
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  Tuiles de terrain
// ─────────────────────────────────────────────────────────────────────────

/// Tuiles voulues autour de la caméra et celles déjà prêtes à leur finesse (mesures, 0.13 E3).
#[derive(Resource, Default)]
pub struct TileStats {
    pub leaves: usize,
    pub ready: usize,
}

struct TileEntry {
    entity: Entity,
    last_needed: f64,
    /// Climat (saison, heure) avec lequel la tuile a été construite : `TileStore::generation`.
    generation: u32,
}

#[derive(Resource, Default)]
struct TileStore {
    body: Option<TargetKind>,
    built: HashMap<TileKey, TileEntry>,
    tasks: HashMap<TileKey, (Task<(Mesh, Vec<DecorInstance>)>, u32)>,
    material: Option<Handle<StandardMaterial>>,
    far_hidden: bool,
    /// Climat des tuiles : quand la saison (ou l'heure, pour le givre) change, on les reconstruit
    /// une à une, en gardant les anciennes affichées en attendant.
    climate: Option<crate::planetgen::climate::Climate>,
    /// Marée avec laquelle les tuiles sont construites (C4).
    tide: Option<crate::terrain::Tide>,
    generation: u32,
    /// Cellules modifiées de l'astre (minage, 0.14).
    voxels: Option<std::sync::Arc<crate::voxel::BodyVoxels>>,
    /// Tranche de grottes maillée autour du joueur sous terre (0.13 E2).
    cave_window: Option<(f32, f32)>,
    /// Tuiles remplacées qui s'effacent en fondu (0.13 T4) : début et matériau transparent.
    fading: HashMap<Entity, (f64, Handle<StandardMaterial>)>,
    /// Niveaux de tuiles avec décor des tuiles construites (réglage du détail du sol).
    decor_levels: u32,
    /// Les tuiles projettent des ombres (Ultra).
    shadows: bool,
}

/// Durée du fondu entre deux niveaux de détail du sol (s).
const TILE_FADE_SECS: f64 = 0.35;

fn set_far_visibility(
    root: Entity,
    visible: bool,
    children: &Query<&Children>,
    far: &Query<(), With<FarMesh>>,
    vis: &mut Query<&mut Visibility>,
) {
    let Ok(kids) = children.get(root) else { return };
    let wanted = if visible { Visibility::Inherited } else { Visibility::Hidden };
    for &child in kids.iter() {
        if far.contains(child) {
            if let Ok(mut v) = vis.get_mut(child) {
                if *v != wanted {
                    *v = wanted;
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn update_tiles(
    mut commands: Commands,
    time: Res<Time>,
    settings: Res<GameSettings>,
    surface: Res<Surface>,
    mut store: ResMut<TileStore>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    decor: Res<DecorAssets>,
    cam_q: Query<&Transform, With<Camera3d>>,
    planets: Query<(Entity, &PlanetId, &Transform), (With<PlanetRoot>, Without<Camera3d>)>,
    moons: Query<(Entity, &MoonId, &Transform), (With<MoonRoot>, Without<Camera3d>)>,
    asteroids: Res<crate::asteroids::AsteroidField>,
    children: Query<&Children>,
    far: Query<(), With<FarMesh>>,
    mut vis: Query<&mut Visibility>,
    mut stats: ResMut<TileStats>,
) {
    // Pas de tuiles dans une géante gazeuse : sa sphère reste affichée
    let wanted = if surface.active() && !surface.gaseous() { surface.body } else { None };

    // Changement (ou fin) de séjour : on jette les tuiles et on rend le maillage lointain
    if store.body != wanted {
        if let Some(old) = store.body {
            let root = find_root(&old, &planets, &moons, &asteroids).map(|(e, _)| e);
            for (_, entry) in store.built.drain() {
                commands.entity(entry.entity).despawn_recursive();
            }
            store.tasks.clear();
            store.fading.clear();
            if let Some(root) = root {
                set_far_visibility(root, true, &children, &far, &mut vis);
            }
        }
        store.far_hidden = false;
        store.body = wanted;
        store.climate = None;
        store.tide = None;
        store.voxels = wanted.and_then(|k| crate::voxel::body_voxels(&settings, &k));
    }
    let Some(kind) = store.body else { return };
    let (Some(terrain), Ok(cam)) = (surface.terrain.as_ref(), cam_q.get_single()) else { return };
    let Some((root, root_tf)) = find_root(&kind, &planets, &moons, &asteroids) else { return };
    let params = terrain.params;
    let layout = terrain.layout;
    let now = time.elapsed_secs_f64();
    if store.climate != Some(params.climate) || store.tide != Some(params.tide) {
        // Pas de nouvelle saison tant que la précédente n'est pas finie (temps très accéléré)
        let rebuilding = store.built.values().any(|e| e.generation != store.generation);
        if store.climate.is_none() || !rebuilding {
            store.climate = Some(params.climate);
            store.tide = Some(params.tide);
            store.generation = store.generation.wrapping_add(1);
        }
    }
    // Sous terre, plus bas que les grottes toujours maillées : une tranche autour de la caméra (par
    // pas d'une demi-tranche, pour ne pas tout reconstruire à chaque pas)
    let cam_local0 = root_tf.rotation.inverse() * (cam.translation - root_tf.translation);
    let v = layout.voxel;
    let depth = terrain.surface_r(cam_local0.normalize_or(Vec3::Y)) - cam_local0.length();
    let window = (depth > (crate::terrain::NEAR_CAVE_VOXELS - crate::terrain::CAVE_WINDOW_VOXELS) * v).then(|| {
        let step = crate::terrain::CAVE_WINDOW_VOXELS * 0.5 * v;
        let mid = (depth / step).round() * step;
        (mid - crate::terrain::CAVE_WINDOW_VOXELS * v, mid + crate::terrain::CAVE_WINDOW_VOXELS * v)
    });
    if window != store.cave_window {
        store.cave_window = window;
        store.generation = store.generation.wrapping_add(1);
    }
    let cave_window = store.cave_window;
    let params = BodyParams { climate: store.climate.unwrap_or(params.climate), tide: store.tide.unwrap_or(params.tide), ..params };
    let voxels = store.voxels.clone();
    // Les grottes de l'astre (et leur cache) sont partagées par toutes les tuiles
    let caves = terrain.caves.clone();
    let rocks = terrain.rocks.clone();

    let material = store
        .material
        .get_or_insert_with(|| {
            materials.add(StandardMaterial {
                base_color: Color::WHITE,
                perceptual_roughness: 0.95,
                reflectance: 0.15,
                ..default()
            })
        })
        .clone();

    // Tuiles voulues autour de la caméra, avec leurs ancêtres (repli le temps de la construction).
    // Caméra dans le repère fixe de l'astre : les tuiles tournent avec lui.
    let cam_local = root_tf.rotation.inverse() * (cam.translation - root_tf.translation);
    let mut leaves = Vec::new();
    let ground_r = terrain.ground(cam_local.normalize_or(Vec3::Y)).top;
    // Distance de détail réglable (0.13 T4) : rien de plus fin derrière l'horizon
    let (_, split, decor_levels, tile_shadows) = crate::graphics::terrain_detail(&settings);
    let tile_shadows = tile_shadows && settings.shadows;
    crate::terrain::select_tiles_with(layout, ground_r, cam_local, split, terrain.relief_span(), &mut leaves);
    // Autre réglage : le décor change (tuiles reconstruites), les ombres aussi
    if store.decor_levels != decor_levels {
        if store.decor_levels != 0 {
            store.generation = store.generation.wrapping_add(1);
        }
        store.decor_levels = decor_levels;
    }
    if store.shadows != tile_shadows {
        store.shadows = tile_shadows;
        for entry in store.built.values() {
            if tile_shadows {
                commands.entity(entry.entity).remove::<NotShadowCaster>();
            } else {
                commands.entity(entry.entity).try_insert(NotShadowCaster);
            }
        }
    }
    let generation = store.generation;
    let mut needed: HashSet<TileKey> = HashSet::with_capacity(leaves.len() * 2);
    for &leaf in &leaves {
        let mut k = Some(leaf);
        while let Some(key) = k {
            if !needed.insert(key) {
                break;
            }
            k = key.parent();
        }
    }

    // Les premières tuiles (les 6 racines) sont construites tout de suite : jamais de planète vide
    if store.built.is_empty() && store.tasks.is_empty() {
        for face in 0..6 {
            let key = TileKey::root(face);
            let first = Terrain::new(params).with_voxels(voxels.clone()).with_caves(caves.clone()).with_rocks(rocks.clone()).with_cave_window(cave_window);
            let entity = spawn_tile(&mut commands, &mut meshes, &material, root, build_tile_mesh_with(&first, key), tile_shadows);
            store.built.insert(key, TileEntry { entity, last_needed: now, generation });
        }
    }

    // Récupère les tuiles terminées, dans le budget de l'image (les plus grosses d'abord : jamais de trou)
    let started = std::time::Instant::now();
    let mut finished: Vec<TileKey> = store.tasks.keys().copied().collect();
    finished.sort_by_key(|k| k.depth);
    for key in finished {
        if started.elapsed().as_secs_f64() * 1000.0 > TILE_BUDGET_MS {
            break;
        }
        let Some((task, built_gen)) = store.tasks.get_mut(&key) else { continue };
        let built_gen = *built_gen;
        if let Some((mesh, objects)) = block_on(future::poll_once(task)) {
            store.tasks.remove(&key);
            let entity = spawn_tile(&mut commands, &mut meshes, &material, root, mesh, tile_shadows);
            // Décor de la tuile (tuiles proches seulement) : il disparaît avec elle
            decor.spawn(&mut commands, entity, &objects);
            // Nouvelle saison : la tuile remplace l'ancienne (visible jusque-là)
            let shown = store.built.get(&key).and_then(|old| vis.get(old.entity).ok().map(|v| *v)).unwrap_or(Visibility::Hidden);
            if let Ok(mut v) = vis.get_mut(entity) {
                *v = shown;
            }
            if let Some(old) = store.built.insert(key, TileEntry { entity, last_needed: now, generation: built_gen }) {
                commands.entity(old.entity).despawn_recursive();
            }
        }
    }

    // Lance les constructions manquantes : d'abord les grosses tuiles, puis les plus proches et celles
    // devant la caméra (au centre de l'écran) ; les tuiles d'une ancienne saison passent après les
    // manquantes
    let cam_fwd = root_tf.rotation.inverse() * *cam.forward();
    let mut missing: Vec<(bool, u8, f32, TileKey)> = needed
        .iter()
        .filter(|k| !store.tasks.contains_key(k) && store.built.get(k).is_none_or(|e| e.generation != generation))
        .map(|k| {
            let to = k.center_dir() * params.radius - cam_local;
            let ahead = to.normalize_or(Vec3::Y).dot(cam_fwd);
            (store.built.contains_key(k), k.depth, to.length_squared() * (1.6 - 0.6 * ahead), *k)
        })
        .collect();
    missing.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.total_cmp(&b.2)));
    let pool = AsyncComputeTaskPool::get();
    for (_, _, _, key) in missing {
        if store.tasks.len() >= max_tile_tasks() {
            break;
        }
        let p = params;
        let vx = voxels.clone();
        let cv = caves.clone();
        let rk = rocks.clone();
        store.tasks.insert(
            key,
            (
                pool.spawn(async move {
                    let terrain = Terrain::new(p).with_voxels(vx).with_caves(cv).with_rocks(rk).with_cave_window(cave_window).with_decor_levels(decor_levels);
                    (build_tile_mesh_with(&terrain, key), tile_decor(&terrain, key))
                }),
                generation,
            ),
        );
    }

    stats.leaves = leaves.len();
    stats.ready = leaves.iter().filter(|k| store.built.contains_key(k)).count();
    // Tuiles à afficher : la feuille si elle est prête, sinon son plus proche ancêtre prêt
    let mut shown: HashSet<TileKey> = HashSet::new();
    for &leaf in &leaves {
        let mut k = Some(leaf);
        while let Some(key) = k {
            if store.built.contains_key(&key) {
                shown.insert(key);
                break;
            }
            k = key.parent();
        }
    }
    let covered: Vec<TileKey> = shown
        .iter()
        .copied()
        .filter(|key| {
            let mut p = key.parent();
            while let Some(a) = p {
                if shown.contains(&a) {
                    return true;
                }
                p = a.parent();
            }
            false
        })
        .collect();
    for key in covered {
        shown.remove(&key);
    }

    // Une tuile remplacée par ses filles (ou par son parent) reste affichée en s'effaçant par-dessus
    // (0.13 T4) : pas de saut visible d'un niveau de détail à l'autre
    let mut start_fade = Vec::new();
    let mut stop_fade = Vec::new();
    let far_hidden = store.far_hidden;
    let fading_now: HashSet<Entity> = store.fading.keys().copied().collect();
    for (key, entry) in store.built.iter_mut() {
        if needed.contains(key) {
            entry.last_needed = now;
        }
        if let Ok(mut v) = vis.get_mut(entry.entity) {
            let wanted = if shown.contains(key) { Visibility::Inherited } else { Visibility::Hidden };
            let fading = fading_now.contains(&entry.entity);
            if wanted == Visibility::Hidden && *v != Visibility::Hidden && far_hidden {
                if !fading {
                    start_fade.push(entry.entity);
                }
            } else {
                if fading && wanted != Visibility::Hidden {
                    stop_fade.push(entry.entity);
                }
                if *v != wanted {
                    *v = wanted;
                }
            }
        }
    }
    for e in stop_fade {
        store.fading.remove(&e);
        commands.entity(e).try_insert(MeshMaterial3d(material.clone()));
    }
    for e in start_fade {
        let fade = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.95,
            reflectance: 0.15,
            alpha_mode: AlphaMode::Blend,
            ..default()
        });
        commands.entity(e).try_insert(MeshMaterial3d(fade.clone()));
        store.fading.insert(e, (now, fade));
    }
    let alive: HashSet<Entity> = store.built.values().map(|e| e.entity).collect();
    let mut done = Vec::new();
    for (&e, (start, handle)) in store.fading.iter() {
        let f = (now - start) / TILE_FADE_SECS;
        if f >= 1.0 || !alive.contains(&e) {
            done.push(e);
        } else if let Some(m) = materials.get_mut(handle) {
            m.base_color = Color::WHITE.with_alpha((1.0 - f) as f32);
        }
    }
    for e in done {
        store.fading.remove(&e);
        if alive.contains(&e) {
            commands.entity(e).try_insert(MeshMaterial3d(material.clone()));
            if let Ok(mut v) = vis.get_mut(e) {
                *v = Visibility::Hidden;
            }
        }
    }

    // Le maillage lointain disparaît une fois les tuiles affichées
    if !store.far_hidden && !shown.is_empty() && store.built.values().all(|e| vis.get(e.entity).is_ok()) {
        set_far_visibility(root, false, &children, &far, &mut vis);
        store.far_hidden = true;
    }

    // Ménage : tuiles inutilisées depuis longtemps
    if store.built.len() > MAX_TILES {
        let stale: Vec<TileKey> = store
            .built
            .iter()
            .filter(|(k, e)| !needed.contains(k) && now - e.last_needed > TILE_KEEP_SECS)
            .map(|(k, _)| *k)
            .collect();
        for key in stale {
            if let Some(entry) = store.built.remove(&key) {
                commands.entity(entry.entity).despawn_recursive();
            }
        }
    }
}

fn find_root(
    kind: &TargetKind,
    planets: &Query<(Entity, &PlanetId, &Transform), (With<PlanetRoot>, Without<Camera3d>)>,
    moons: &Query<(Entity, &MoonId, &Transform), (With<MoonRoot>, Without<Camera3d>)>,
    asteroids: &crate::asteroids::AsteroidField,
) -> Option<(Entity, Transform)> {
    match *kind {
        TargetKind::Planet(id) => planets.iter().find(|(_, p, _)| p.0 == id).map(|(e, _, t)| (e, *t)),
        TargetKind::Moon(planet_idx, moon_idx) => moons
            .iter()
            .find(|(_, m, _)| m.planet_idx == planet_idx && m.moon_idx == moon_idx)
            .map(|(e, _, t)| (e, *t)),
        TargetKind::Asteroid(key) => asteroids.root(&key),
        _ => None,
    }
}

fn spawn_tile(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    material: &Handle<StandardMaterial>,
    root: Entity,
    mesh: Mesh,
    shadows: bool,
) -> Entity {
    let entity = commands.spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(material.clone()), Transform::IDENTITY, Visibility::Hidden, SurfaceTile)).id();
    // Ombres du relief (Ultra) : montagnes et falaises ombrent le sol
    if !shadows {
        commands.entity(entity).insert(NotShadowCaster);
    }
    commands.entity(root).add_child(entity);
    entity
}

// ─────────────────────────────────────────────────────────────────────────
//  Interface
// ─────────────────────────────────────────────────────────────────────────

fn setup_hud(mut commands: Commands) {
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(64.0),
            left: Val::Px(0.0),
            right: Val::Px(0.0),
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_children(|p| {
            p.spawn((
                Text::new(""),
                TextFont { font_size: 16.0, ..default() },
                TextColor(Color::srgba(0.85, 0.95, 1.0, 0.92)),
                TextLayout::new_with_justify(JustifyText::Center),
                SurfaceHud,
            ));
        });
}

fn update_hud(
    surface: Res<Surface>,
    target: Res<CameraTarget>,
    settings: Res<GameSettings>,
    weather: Res<crate::world_clock::LocalWeather>,
    suit: Res<crate::suit::Suit>,
    mut hud: Query<&mut Text, With<SurfaceHud>>,
) {
    let label = match surface.phase {
        Phase::Orbit => match body_params(&settings, &target.0) {
            Some(p) if p.gaseous => "Geante gazeuse (pas de sol)   Zoomez sous 1000 pour entrer dans son atmosphere   P : planete suivante   M : lune".to_string(),
            Some(_) => "Zoomez sous 1000 pour naviguer autour de l'astre   V : atterrir   P : planete suivante   M : lune".to_string(),
            None if matches!(target.0, TargetKind::Star(_)) => "P : aller a la planete suivante du systeme".to_string(),
            None => String::new(),
        },
        Phase::Flying if surface.gaseous() => "ZQSD/WASD : voler   A/D : tourner   Maj : accelerer   Espace/Ctrl : monter/descendre\nClic droit : orbiter   Molette : zoom (>1000 : orbite)   Pas de sol : la pression abime la coque".to_string(),
        Phase::Flying => format!(
            "ZQSD/WASD : voler   A/D : tourner   Maj : accelerer   Espace/Ctrl : monter/descendre\nClic droit : orbiter   Molette : zoom (>1000 : orbite)   V : atterrir   N : phares   J : saut suborbital (point vise, > 5 000 voxels)\n{}   {}",
            weather.short(),
            surface.wind_text
        ),
        Phase::Descending => "Atterrissage en cours...".to_string(),
        Phase::Ascending => "Decollage en cours...".to_string(),
        Phase::Boarding => "Embarquement...".to_string(),
        Phase::Disembarking => "Le cockpit s'ouvre...".to_string(),
        Phase::Walking => {
            let w = &surface.walker;
            let up = w.up();
            let lat = up.y.clamp(-1.0, 1.0).asin().to_degrees();
            let lon = up.z.atan2(up.x).to_degrees();
            let params = surface.params();
            let radius = params.map_or(0.0, |p| p.radius);
            let alt = w.pos.length() - radius;
            // Heure, saison et température locales (latitude, altitude, heure, saison)
            let now = if weather.body.is_some() { weather.short() } else { String::new() };
            // Combinaison : oxygène, vie, alertes (A4)
            let now = if suit.hud.is_empty() { now } else { format!("{now}
{}", suit.hud) };
            format!(
                "ZQSD/WASD : marcher   Maj : courir   Espace : sauter   V : decoller   N : lampe   F5 : vue (1re / dos / face)   Molette : distance\nLat {lat:.1}  Lon {lon:.1}  Alt {alt:.0}   {now}{}",
                match (w.in_water, w.liquid) {
                    (false, _) => "",
                    (_, crate::planet::VoxelType::Methane) => "  (dans le methane)",
                    (_, crate::planet::VoxelType::Ammonia) => "  (dans l'ammoniac)",
                    (_, crate::planet::VoxelType::Lava) => "  (dans la lave !)",
                    _ => "  (a l'eau)",
                }
            )
        }
    };
    for mut text in &mut hud {
        if **text != label {
            **text = label.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    /// E2 (Q3) : le saut suborbital part du vaisseau, monte haut au milieu et arrive au-dessus du
    /// point visé ; il avance toujours vers la cible.
    #[test]
    fn the_suborbital_hop_arcs_to_its_target() {
        let from = Vec3::Y;
        let to = Vec3::new(1.0, 1.0, 0.0).normalize();
        let hop = super::Hop { from, to, r0: 9_000.0, r1: 9_010.0, peak: 500.0, t: 0.0, dur: 10.0 };
        let (p0, _) = hop.at(0.0);
        let (pm, dm) = hop.at(0.5);
        let (p1, _) = hop.at(1.0);
        assert!(p0.distance(from * 9_000.0) < 1e-2 && p1.distance(to * 9_010.0) < 1e-2, "{p0} {p1}");
        assert!(pm.length() > 9_400.0, "sommet {}", pm.length());
        assert!(dm.dot(to - from) > 0.0);
        let mut last = 0.0;
        for i in 0..=20 {
            let a = hop.at(i as f32 / 20.0).0.normalize().angle_between(from);
            assert!(a + 1e-4 >= last, "recule a {i}");
            last = a;
        }
    }

    /// C3 : en croisière, le nez du vaisseau (-Z, avant du modèle) regarde la destination ; près du
    /// point de stationnement, le dessous est parallèle à la surface.
    #[test]
    fn the_ship_points_at_its_destination_then_levels() {
        let center = Vec3::new(0.0, 0.0, -100_000.0);
        let hover = center + Vec3::Y * 1_000.0;
        // Loin : on fonce vers l'astre, le nez suit la route
        let pos = Vec3::ZERO;
        let step = (hover - pos).normalize() * 500.0;
        let mut rot = Quat::IDENTITY;
        for _ in 0..200 {
            rot = super::orient_ship(rot, pos, step, hover, center, true, false, 1.0 / 60.0);
        }
        assert!((rot * Vec3::NEG_Z).dot(step.normalize()) > 0.99, "nez {:?}", rot * Vec3::NEG_Z);
        // Sans roulis : l'aile droite reste à l'horizontale
        assert!((rot * Vec3::X).y.abs() < 0.05);
        // Arrivé : la pose de stationnement (haut = verticale de l'astre)
        let pos = hover + Vec3::X * 10.0;
        for _ in 0..200 {
            rot = super::orient_ship(rot, pos, Vec3::ZERO, hover, center, true, false, 1.0 / 60.0);
        }
        let up = (pos - center).normalize();
        assert!((rot * Vec3::Y).dot(up) > 0.99, "haut {:?}", rot * Vec3::Y);
    }

    use super::*;
    use crate::planetgen::climate::Climate;
    use crate::terrain::{EARTH_SKY, EARTH_SUNSET};

    fn world() -> Terrain {
        Terrain::new(BodyParams {
            airless: false,
            atmosphere: true,
            radius: 6000.0,
            sea_level: 0.35,
            terrain_height: 240.0,
            seed: 77,
            noise_scale: 2.0,
            detail_scale: 4.0,
            temperature: 15.0,
            gravity: 1.0,
            gaseous: false,
            climate: Climate::default(),
            sky: EARTH_SKY,
            sunset: EARTH_SUNSET,
            haze: EARTH_SKY,
            pressure: 1.0,
            hydro: crate::planetgen::hydrology::Hydro::default(),
            relief: crate::planetgen::geology::Relief::default(),
            biomes: crate::planetgen::biome::BiomeParams::default(),
            asteroid: None,
            tide: Default::default(),
        })
    }

    #[test]
    fn two_suns_light_the_sky_and_set_one_after_the_other() {
        let p = world().params;
        let up = Vec3::Y;
        let space = [0.0, 0.0, 0.02];
        let high = Vec3::new(0.0, 1.0, 0.2).normalize();
        let low = Vec3::new(1.0, 0.05, 0.0).normalize();
        let below = Vec3::new(1.0, -0.6, 0.0).normalize();
        // Un seul soleil couché : nuit ; le second encore haut : jour
        let (_, night, _) = combined_sky(&p, &[(below, 1.0, 1.0)], up, 1.0, space);
        let (_, day, h) = combined_sky(&p, &[(below, 1.0, 1.0), (high, 0.8, 0.8)], up, 1.0, space);
        assert!(night < 0.05 && day > 0.5 && h > 0.9, "{night} {day} {h}");
        // Deux soleils bas : ciel de coucher (plus rouge que bleu)
        let (c, _, _) = combined_sky(&p, &[(low, 1.0, 1.0), (Vec3::new(0.9, 0.12, 0.3).normalize(), 0.7, 0.7)], up, 1.0, space);
        assert!(c[0] > c[2], "{c:?}");
    }

    fn settle(w: &mut Walker, t: &Terrain, secs: f32) {
        for _ in 0..(secs * 60.0) as usize {
            w.step(t, &WalkInput::default(), 1.0 / 60.0);
        }
    }

    #[test]
    fn standing_player_stays_on_the_ground() {
        let t = world();
        let dir = Vec3::new(0.2, 0.7, 0.4).normalize();
        let mut w = Walker::spawn(&t, dir, Vec3::X);
        settle(&mut w, &t, 3.0);
        assert!(w.on_ground);
        let ground = t.ground(w.up()).top;
        assert!((w.pos.length() - ground).abs() < 1e-2, "{} vs {}", w.pos.length(), ground);
        assert!(w.hvel.length() < 1e-2);
    }

    /// Règle 10 : 10 min debout sur une planète qui tourne (jour de 24 min, orbite autour de
    /// l'étoile) sans glisser d'un voxel : le marcheur vit dans le repère fixe de l'astre, et sa
    /// position dans le monde suit exactement le sol sous ses pieds.
    #[test]
    fn standing_ten_minutes_on_a_spinning_planet() {
        use crate::world_clock::{day_secs, season_secs, Spin};
        let t = world();
        let dir = Vec3::new(0.3, 0.5, -0.6).normalize();
        let mut w = Walker::spawn(&t, dir, Vec3::X);
        settle(&mut w, &t, 3.0);
        let start = w.pos;
        let spin = Spin { tilt: 0.4, day_s: day_secs(24.0), year_s: 4.0 * season_secs(365.0), day_phase: 0.3, year_phase: 0.1, locked: false, ecc: 0.0, peri: 0.0 };
        let dt = 1.0 / 60.0;
        let mut clock = 5_000.0f64;
        for k in 0..(600 * 60) {
            clock += dt as f64;
            w.step(&t, &WalkInput::default(), dt);
            if k % 600 == 0 {
                // Le pied, posé dans le monde, est au-dessus du même point du sol
                let center = Vec3::new(1.0e5, 0.0, 0.0).lerp(Vec3::new(0.0, 0.0, 1.0e5), k as f32 / 36_000.0);
                let frame = Frame { center, rot: spin.rotation(clock, -center) };
                let foot = frame.to_world(Transform::from_translation(w.pos)).translation;
                let ground = frame.to_world(Transform::from_translation(start)).translation;
                assert!(foot.distance(ground) < t.voxel() * 0.05, "{}", foot.distance(ground));
            }
        }
        assert!(w.pos.distance(start) < t.voxel() * 0.05, "a glisse de {}", w.pos.distance(start));
    }

    #[test]
    fn moonlight_follows_the_phase_and_the_horizon() {
        let eye = Vec3::new(0.0, 1_000.0, 0.0);
        let up = Vec3::Y;
        // Une lune comme la nôtre (rayon angulaire 1/220) au zénith
        let moon = (Vec3::new(0.0, 1_000.0 + 220_000.0, 0.0), 1_000.0);
        let star_behind_us = Vec3::new(0.0, -1.0e9, 0.0);
        let star_behind_moon = Vec3::new(0.0, 1.0e9, 0.0);
        let full = moonlight(eye, up, star_behind_us, &[moon]);
        let new = moonlight(eye, up, star_behind_moon, &[moon]);
        assert!((full - 0.15).abs() < 0.01, "{full}");
        assert!(new < 0.001, "{new}");
        let quarter = moonlight(eye, up, Vec3::new(1.0e9, 0.0, 0.0), &[moon]);
        assert!((quarter - 0.075).abs() < 0.01, "{quarter}");
        // Sous l'horizon : rien
        let below = (Vec3::new(0.0, -220_000.0, 0.0), 1_000.0);
        assert_eq!(moonlight(eye, up, star_behind_us, &[below]), 0.0);
        // Une grosse lune proche éclaire bien plus (plafonnée)
        let big = (Vec3::new(0.0, 30_000.0, 0.0), 3_000.0);
        assert!(moonlight(eye, up, star_behind_us, &[big]) > 0.9);
    }

    #[test]
    fn the_sky_is_dark_at_night_and_without_air() {
        let mut p = world().params;
        p.atmosphere = true;
        p.pressure = 1.0;
        assert!(daylight(&p, 0.8, 1.0) > 0.99);
        assert!(daylight(&p, -0.3, 1.0) < 0.01);
        assert!((0.1..0.9).contains(&daylight(&p, 0.0, 1.0)), "crepuscule");
        p.atmosphere = false;
        assert_eq!(daylight(&p, 0.8, 1.0), 0.0);
    }

    /// Voxels 3D : on passe à pied sous l'arche de test (sans traverser la roche), et on tient
    /// debout sur son tablier.
    #[test]
    fn walking_under_and_on_the_test_arch() {
        let t = world();
        let o = t.overhang.expect("arche");
        let v = t.voxel();
        // Sous l'auvent, on marche vers l'est puis on revient vers l'arche
        let under = o.visit_dir();
        let mut w = Walker::spawn(&t, under, o.dir - under);
        w.pos = under * t.floor(under, o.base + 3.0 * v).top;
        settle(&mut w, &t, 1.0);
        let start = w.pos.length();
        let roof = t.ceiling(w.up(), start + 0.01);
        assert!(roof.is_finite(), "pas de plafond sous l'auvent");
        for _ in 0..240 {
            w.step(&t, &WalkInput { forward: 1.0, ..default() }, 1.0 / 60.0);
            let r = w.pos.length();
            let up = w.up();
            // Jamais dans la roche : la tête reste sous le plafond, les pieds sur un sol
            assert!(r + BODY_VOXELS * v <= t.ceiling(up, r + 0.01) + 1e-2, "tete dans la roche");
            assert!(r >= t.floor(up, r + 0.01).top - 1e-2, "pieds dans la roche");
        }
        // Lâché au-dessus du tablier : on s'y pose (pas au sol, dessous)
        let top = t.ground(under);
        let mut w = Walker::spawn(&t, under, Vec3::X);
        w.pos = under * (top.top + 30.0 * v);
        w.on_ground = false;
        settle(&mut w, &t, 4.0);
        assert!(w.on_ground && (w.pos.length() - top.top).abs() < 1e-2, "{} vs {}", w.pos.length(), top.top);
    }

    #[test]
    fn a_player_dropped_from_the_sky_lands() {
        let t = world();
        let dir = Vec3::new(-0.5, 0.2, 0.8).normalize();
        let mut w = Walker::spawn(&t, dir, Vec3::X);
        w.pos = dir * (t.ground(dir).top + 400.0);
        w.on_ground = false;
        settle(&mut w, &t, 8.0);
        assert!(w.on_ground);
        assert!((w.pos.length() - t.ground(w.up()).top).abs() < 1e-2);
    }

    #[test]
    fn walking_covers_distance_and_stays_above_ground() {
        let t = world();
        let dir = Vec3::new(0.6, 0.3, -0.7).normalize();
        let mut w = Walker::spawn(&t, dir, Vec3::X);
        let start = w.pos;
        let input = WalkInput { forward: 1.0, ..default() };
        let mut stuck_frames = 0;
        for _ in 0..600 {
            let before = w.pos;
            w.step(&t, &input, 1.0 / 60.0);
            if w.pos.distance(before) < 1e-4 {
                stuck_frames += 1;
            }
            assert!(w.pos.length() >= t.ground(w.up()).top - 1e-2, "sous le sol");
        }
        // 10 s à 7 voxels/s : on a parcouru une bonne part de la distance (les obstacles ralentissent)
        let walked = w.pos.distance(start);
        assert!(walked > t.voxel() * 7.0 * 10.0 * 0.4, "parcouru {walked}");
        assert!(stuck_frames < 450, "bloqué {stuck_frames} images");
    }

    #[test]
    fn sprinting_is_faster_than_walking() {
        // Terrain plat de grottes : seule la vitesse compte ici
        let mut t = world();
        t.caves = None;
        t.overhang = None;
        let dir = Vec3::new(0.0, 1.0, 0.1).normalize();
        let run = |sprint: bool| {
            let mut w = Walker::spawn(&t, dir, Vec3::X);
            let start = w.pos;
            for _ in 0..180 {
                w.step(&t, &WalkInput { forward: 1.0, sprint, ..default() }, 1.0 / 60.0);
            }
            w.pos.distance(start)
        };
        assert!(run(true) > run(false) * 1.5);
    }

    #[test]
    fn jumping_leaves_then_returns_to_the_ground() {
        let t = world();
        let dir = Vec3::new(0.1, 0.9, 0.3).normalize();
        let mut w = Walker::spawn(&t, dir, Vec3::X);
        settle(&mut w, &t, 1.0);
        let ground = w.pos.length();
        w.step(&t, &WalkInput { jump: true, ..default() }, 1.0 / 60.0);
        assert!(!w.on_ground);
        let mut apex = ground;
        for _ in 0..120 {
            w.step(&t, &WalkInput::default(), 1.0 / 60.0);
            apex = apex.max(w.pos.length());
        }
        assert!(apex - ground > t.voxel() * 0.8, "saut trop bas : {}", apex - ground);
        assert!(w.on_ground);
    }

    #[test]
    fn looking_turns_around_the_local_vertical() {
        let t = world();
        let mut w = Walker::spawn(&t, Vec3::Y, Vec3::X);
        let up = w.up();
        let before = w.heading;
        w.step(&t, &WalkInput { look: Vec2::new(std::f32::consts::FRAC_PI_2, 0.0), ..default() }, 0.0);
        // Un quart de tour à droite : le cap est perpendiculaire à l'ancien et reste horizontal
        assert!(w.heading.dot(before).abs() < 1e-3);
        assert!(w.heading.dot(up).abs() < 1e-3);
        assert!(w.heading.dot(before.cross(up)) > 0.99, "doit tourner vers la droite");
    }

    #[test]
    fn moons_have_floaty_gravity() {
        let mut p = world().params;
        p.airless = true;
        p.atmosphere = false;
        p.radius = 3000.0;
        p.gravity = 0.16;
        let t = Terrain::new(p);
        let dir = Vec3::new(0.3, 0.8, 0.2).normalize();
        let mut w = Walker::spawn(&t, dir, Vec3::X);
        settle(&mut w, &t, 1.0);
        let ground = w.pos.length();
        w.step(&t, &WalkInput { jump: true, ..default() }, 1.0 / 60.0);
        let mut apex = ground;
        for _ in 0..240 {
            w.step(&t, &WalkInput::default(), 1.0 / 60.0);
            apex = apex.max(w.pos.length());
        }
        assert!(apex - ground > t.voxel() * 2.0, "saut lunaire trop bas : {}", apex - ground);
    }

    #[test]
    fn camera_never_goes_under_the_surface() {
        let p = world().params;
        let c = Vec3::new(1.0e6, 2.0e5, -3.0e5);
        let inside = c + Vec3::new(0.0, p.radius * 0.5, 0.0);
        let out = keep_outside(inside, c, &p);
        assert!((out - c).length() > p.radius);
        let far = c + Vec3::new(p.radius * 5.0, 0.0, 0.0);
        assert_eq!(keep_outside(far, c, &p), far);
    }

    /// Audit : tous les corps générés (planètes et lunes de nombreux systèmes) sont sûrs à explorer.
    #[test]
    fn every_generated_body_is_safe_to_walk_on() {
        use crate::terrain::{build_tile_mesh, select_tiles, TileKey};
        let settings = GameSettings::default();
        let mut bodies = Vec::new();
        for sys in settings.systems.dense().iter().take(150) {
            for p in sys.planets() {
                // Pas de sol sur une géante gazeuse : on n'y marche pas
                if !p.gaseous() {
                    bodies.push(BodyParams::planet(p));
                }
                for m in &p.moons {
                    bodies.push(BodyParams::moon(m, p));
                }
            }
        }
        assert!(bodies.len() > 200);
        let spots = [
            Vec3::Y,
            Vec3::NEG_Y,
            Vec3::new(1.0, 1.0, 1.0),
            Vec3::new(-1.0, 0.2, -1.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, -1.0),
        ];
        for params in bodies {
            let t = Terrain::new(params);
            let l = t.layout;
            let max = crate::terrain::MAX_VOXEL / crate::terrain::GROUND_SCALE as f32;
            assert!(l.voxel > max * 0.25 && l.voxel <= max, "voxel {} pour un rayon de {}", l.voxel, params.radius);
            for dir in spots {
                let dir = dir.normalize();
                // Le sol existe, est fini et reste près de la surface
                let g = t.ground(dir);
                assert!(g.top.is_finite() && (g.top - params.radius).abs() < params.radius * 0.5, "sol {} (rayon {})", g.top, params.radius);
                // Marche 5 s dans chaque sens sans jamais passer sous le sol ni produire de NaN
                for heading in [Vec3::X, Vec3::Z, Vec3::NEG_X] {
                    let mut w = Walker::spawn(&t, dir, heading);
                    for _ in 0..300 {
                        w.step(&t, &WalkInput { forward: 1.0, sprint: true, ..default() }, 1.0 / 60.0);
                        assert!(w.pos.is_finite() && w.heading.is_finite());
                        // À la couture entre deux faces du cube, les deux grilles de colonnes se recouvrent : la
                        // hauteur peut différer d'un voxel pour un même point (corrigé à l'image suivante)
                        // Jamais dans la roche (sous une arche ou dans une grotte, le dessus de la
                        // colonne n'est pas le sol) : dans sa colonne, aucun sol plein entre les pieds
                        // et un voxel au-dessus
                        let r = w.pos.length();
                        // (à la couture de deux faces du cube, les grilles diffèrent d'un voxel :
                        // corrigé à l'image suivante, d'où 1,5 voxel de tolérance)
                        let above = t.floor(w.up(), r + t.voxel() * 2.0);
                        assert!(above.top <= r + t.voxel() * 1.5 || above.kind.is_liquid(), "dans la roche (rayon {}) : sol {} au-dessus des pieds {r}", params.radius, above.top);
                    }
                }
                // Le quadtree reste borné et ses tuiles sont valides
                let cam = dir * (g.top + t.voxel() * 2.0);
                let mut tiles = Vec::new();
                select_tiles(l, t.ground(cam.normalize()).top, cam, &mut tiles);
                assert!(tiles.len() < 700, "{} tuiles pour un rayon de {}", tiles.len(), params.radius);
                let key = *tiles.iter().max_by_key(|k| k.depth).unwrap();
                let mesh = build_tile_mesh(&params, key);
                assert!(mesh.count_vertices() > 1000);
                let _ = TileKey::root(0);
            }
        }
    }
}
