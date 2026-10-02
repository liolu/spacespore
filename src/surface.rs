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
use crate::planet::{MoonId, MoonRoot, PlanetId, PlanetRoot, StarRoot};
use crate::settings::GameSettings;
use crate::ship::Ship;
use crate::terrain::{build_tile_mesh, select_tiles, BodyParams, Terrain, TileKey, build_tile_mesh_with};
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

/// Nombre maximum de tuiles construites en même temps en arrière-plan.
const MAX_TILE_TASKS: usize = 10;
/// Au-delà, les tuiles inutilisées depuis longtemps sont supprimées.
const MAX_TILES: usize = 520;
const TILE_KEEP_SECS: f64 = 8.0;

/// Touche pour sortir du vaisseau (atterrir) et y rentrer (décoller) : une touche que le reste
/// du jeu n'utilise pas (C, E, F, G, L, M, N, P et T servent déjà).
pub const ENTER_SHIP_KEY: KeyCode = KeyCode::KeyV;

pub struct SurfacePlugin;

impl Plugin for SurfacePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Surface>()
            .init_resource::<GalaxyDim>()
            .init_resource::<TileStore>()
            .add_systems(Startup, (setup_hud, spawn_lamps))
            .add_systems(Update, update_galaxy_dim)
            .add_systems(
                Update,
                (surface_control.in_set(SurfaceControl), surface_light, update_lamps, update_season, update_tiles, update_hud)
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
        _ => None,
    }
}

/// Rayon (depuis le centre) auquel le vaisseau stationne au-dessus d'un astre.
pub fn hover_radius(p: &BodyParams) -> f32 {
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
            let try_move = |pos: Vec3, d: Vec3| -> Option<Vec3> {
                let next = (pos + d).normalize() * r;
                (t.ground(next.normalize()).top <= r + allow + 1e-3).then_some(next)
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
        let ground = t.ground(up);
        self.in_water = ground.kind.is_liquid();
        self.liquid = ground.kind;
        // Vraie gravité de l'astre : sur une lune à 0,16 g, on saute six fois plus haut
        let gravity = GRAVITY * t.params.gravity.clamp(MIN_GRAVITY, 4.0) * v;
        if self.on_ground && inp.jump {
            self.vr = JUMP_VOXELS * v;
            self.on_ground = false;
        } else if self.on_ground {
            if ground.top >= r - STEP_VOXELS * v {
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
        }
        self.pos = up * r;
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
const JUMP_VOXELS: f32 = 7.5;
/// Pesanteur à 1 g, en voxels par seconde².
const GRAVITY: f32 = 22.0;
/// Sous cette gravité (g), on garde un minimum de poids : sinon un saut ne retomberait jamais.
const MIN_GRAVITY: f32 = 0.05;

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
}

/// En zoomant sous cette distance du vaisseau, on passe en navigation autour de l'astre ;
/// en dézoomant au-delà, on revient à la vue orbitale.
pub const FLIGHT_ZOOM: f32 = 1_000.0;

#[derive(Resource)]
pub struct Surface {
    phase: Phase,
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
            zoom_in_until: 0.0,
            daylight: 1.0,
            sun_height: 1.0,
            lamps: true,
            night_told: false,
        }
    }
}

impl Surface {
    /// Recentrage de l'origine flottante : la pose de caméra mémorisée est en repère monde.
    pub fn shift(&mut self, delta: Vec3) {
        self.cam_from.translation -= delta;
    }

    /// Atterrissage, séjour ou décollage en cours : la caméra n'est plus pilotée par l'orbite.
    pub fn active(&self) -> bool {
        self.phase != Phase::Orbit
    }

    /// Astre où l'on séjourne (vol bas, atterrissage, marche).
    pub fn body(&self) -> Option<TargetKind> {
        self.body.filter(|_| self.active())
    }

    /// Point survolé ou foulé, dans le repère fixe de l'astre (règle 10).
    pub fn local_point(&self) -> Option<Vec3> {
        match self.phase {
            Phase::Orbit => None,
            Phase::Walking => Some(self.walker.pos),
            Phase::Flying => Some(self.fpos),
            Phase::Descending | Phase::Ascending => Some(self.dir1),
        }
    }

    /// Altitude relative du sol (0 = niveau de la mer, 1 = sommets) sous le marcheur ou le
    /// vaisseau en vol bas : les sommets sont plus froids.
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

    fn params(&self) -> Option<BodyParams> {
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
        self.active() && self.sun_height < 0.06
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
    }
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
    let scale1 = terrain.voxel() * 0.9;
    let r1 = terrain.ground(dir1).top + scale1 * 0.45;
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

/// Le dessous du vaisseau reste parallèle à la surface de l'astre : son « haut » est la verticale
/// locale, et son nez suit l'horizontale.
///
/// `prev_forward` est le nez au tour précédent : il n'est abandonné que si le vaisseau fait un vrai
/// trajet (`cruising`), sinon le petit déplacement qui suit l'astre en orbite ferait trembler le nez.
pub fn level_ship(ship: &mut Transform, center: Vec3, prev_forward: Vec3, cruising: bool) {
    let up = (ship.translation - center).normalize_or(Vec3::Y);
    let forward = tangent(if cruising { *ship.forward() } else { prev_forward }, up);
    ship.rotation = look(Vec3::ZERO, forward, up).rotation;
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
    stars: Query<'w, 's, &'static Transform, (With<StarRoot>, Without<Ship>, Without<Camera3d>)>,
    windows: Query<'w, 's, &'static mut Window, With<PrimaryWindow>>,
}

impl Ctx<'_, '_> {
    /// Centre (monde) et orientation d'un astre chargé : son repère fixe tourne avec lui.
    fn pose(&self, kind: &TargetKind) -> Option<Frame> {
        let tf = match *kind {
            TargetKind::Planet(id) => self.planets.iter().find(|(_, p)| p.0 == id).map(|(t, _)| t),
            TargetKind::Moon(planet_idx, moon_idx) => self
                .moons
                .iter()
                .find(|(_, m)| m.planet_idx == planet_idx && m.moon_idx == moon_idx)
                .map(|(t, _)| t),
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
    let enter = ctx.keys.just_pressed(ENTER_SHIP_KEY) && !ui_open;

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
        if ctrl.distance < FLIGHT_ZOOM && now < surface.zoom_in_until {
            if let (Some(params), Some(frame)) = (body_params(&ctx.settings, &kind), ctx.pose(&kind)) {
                let local = frame.point(ship_tf.translation);
                if (local.length() - hover_radius(&params)).abs() < params.radius * 0.1 + 250.0 {
                    let up = local.normalize_or(Vec3::Y);
                    surface.terrain = Some(Terrain::new(params));
                    surface.body = Some(kind);
                    surface.fpos = local;
                    surface.heading = tangent(frame.vector(*ship_tf.forward()), up);
                    surface.fspeed = 0.0;
                    surface.fvert = 0.0;
                    surface.fdist = ctrl.distance.clamp(60.0, FLIGHT_ZOOM * 0.95);
                    surface.fyaw = 0.0;
                    surface.fpitch = 0.35;
                    // Une géante n'a pas de sol : on ne plonge pas d'office vers son cœur
                    surface.fdescend = !params.gaseous;
                    surface.phase = Phase::Flying;
                    *ship_vis = Visibility::Inherited;
                    if params.gaseous {
                        net.notify("Geante gazeuse : Ctrl pour descendre dans l'atmosphere. Attention, la pression y abime la coque !", now);
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
            net.notify("Selectionnez une planete ou une lune pour atterrir.", now);
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
        let terrain = Terrain::new(params);
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
    let center = frame.center;
    // Tout ce qui suit est calculé dans le repère fixe de l'astre (centre à l'origine), puis placé
    // dans le monde à la fin ; la pose de départ des fondus (`cam_from`) est aussi dans ce repère
    let mut ship_local = frame.to_local(*ship_tf);
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
            ship_local = Transform { translation: ship_pos, rotation: ship_rot, scale: Vec3::splat(scale) };

            surface.cam_blend = (surface.cam_blend + dt / 0.8).min(1.0);
            let chase = chase_pose(ship_pos, dir, surface.heading, scale);
            cam_local_tf = blend_pose(&surface.cam_from, chase, smoothstep(surface.cam_blend));

            if u >= 1.0 {
                if descending {
                    let right = surface.heading.cross(dir).normalize_or_zero();
                    let side = (dir * r + right * (scale * 5.0 + 12.0)).normalize();
                    let walker = Walker::spawn(surface.terrain.as_ref().unwrap(), side, -right);
                    surface.ship_local = dir * r;
                    surface.ship_rot = ship_rot;
                    surface.ship_scale = scale;
                    surface.walker = walker;
                    surface.cam_from = cam_local_tf;
                    surface.cam_blend = 0.0;
                    surface.phase = Phase::Walking;
                    net.notify("ZQSD/WASD : marcher  Maj : courir  Espace : sauter  V : decoller", now);
                } else {
                    // De retour en orbite : le vaisseau stationne au-dessus du point de décollage
                    surface.hover = Some((kind, dir));
                    ctrl.distance = FLIGHT_ZOOM * 1.2;
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
                let sens = ctx.settings.mouse_sensitivity * 0.003;
                input.look = look_delta.clamp_length_max(300.0) * sens;
            }
            let terrain = surface.terrain.take().unwrap();
            let mut walker = surface.walker;
            walker.step(&terrain, &input, dt);
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
            let fps = look(eye, walker.view_dir(), up);
            surface.cam_blend = (surface.cam_blend + dt / 0.8).min(1.0);
            cam_local_tf = blend_pose(&surface.cam_from, fps, smoothstep(surface.cam_blend));

            if enter {
                let dir = surface.ship_local.normalize();
                surface.dir0 = dir;
                surface.dir1 = dir;
                surface.r0 = surface.ship_local.length();
                surface.r1 = hover_radius(&params);
                surface.scale0 = surface.ship_scale;
                surface.scale1 = FLIGHT_ZOOM * 1.2 * 0.008;
                surface.heading = tangent(surface.ship_rot * Vec3::NEG_Z, dir);
                surface.t = 0.0;
                surface.dur = (2.5 + (surface.r1 - surface.r0) / 8000.0).clamp(2.5, 6.0);
                surface.cam_from = cam_local_tf;
                surface.cam_blend = 0.0;
                surface.phase = Phase::Ascending;
                net.notify("Decollage...", now);
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
                if ctx.buttons.pressed(MouseButton::Right) {
                    let sens = ctx.settings.mouse_sensitivity * 0.01;
                    surface.fyaw -= look_delta.x * sens;
                    surface.fpitch = (surface.fpitch + look_delta.y * sens).clamp(-0.2, 1.4);
                }
                if wheel != 0.0 {
                    surface.fdist = (surface.fdist * (-wheel * 0.12).exp()).max(20.0);
                }
                if vertical != 0.0 {
                    surface.fdescend = false;
                }
            }

            // Cap, vitesse et altitude
            heading = (Quat::from_axis_angle(up, turn * 1.3 * dt) * heading).normalize();
            let top_speed = (terrain.params.radius * 0.15).clamp(120.0, 4000.0) * if boost { 4.0 } else { 1.0 };
            surface.fspeed += (forward * top_speed - surface.fspeed) * (1.0 - (-2.0 * dt).exp());
            // Dans une géante (des dizaines de milliers d'unités d'atmosphère), on monte et
            // descend plus vite
            let climb = if terrain.params.gaseous { 400.0 * (terrain.params.radius / 8000.0).max(1.0) } else { 400.0 };
            surface.fvert += (vertical * climb - surface.fvert) * (1.0 - (-4.0 * dt).exp());
            let next = (surface.fpos + heading * surface.fspeed * dt).normalize();
            let ground = terrain.ground(next).top;
            let ceiling = hover_radius(&params);
            // Entrée en navigation : on descend d'abord à 150 au-dessus du relief
            let mut r = r + surface.fvert * dt;
            if surface.fdescend {
                let above = r - ground - 150.0;
                if above > 1.0 {
                    r -= ((above * 2.0).clamp(30.0, 2000.0) * dt).min(above);
                } else {
                    surface.fdescend = false;
                }
            }
            let r = r.clamp(ground + 25.0, ceiling.max(ground + 100.0));
            surface.fpos = next * r;
            surface.heading = tangent(heading, next);

            let scale = (surface.fdist * 0.025).clamp(2.0, 40.0);
            let ship_pos = surface.fpos;
            let ship_rot = look(Vec3::ZERO, surface.heading, next).rotation;
            ship_local = Transform { translation: ship_pos, rotation: ship_rot, scale: Vec3::splat(scale) };

            // Caméra derrière le vaisseau, orientable à la souris, jamais sous le relief
            let back = Quat::from_axis_angle(next, surface.fyaw) * -surface.heading;
            let offset = (back * surface.fpitch.cos() + next * surface.fpitch.sin()) * surface.fdist;
            let mut cam_local = surface.fpos + offset;
            let floor = terrain.ground(cam_local.normalize()).top + 10.0;
            if cam_local.length() < floor {
                cam_local = cam_local.normalize() * floor;
            }
            let cam_pos = cam_local;
            cam_local_tf = look(cam_pos, ship_pos + next * (0.4 * scale) - cam_pos, next);
            surface.terrain = Some(terrain);
            ctrl.distance = surface.fdist;

            // Dézoom au-delà de 1000 : retour à la vue orbitale, au-dessus de l'endroit survolé
            if surface.fdist >= FLIGHT_ZOOM {
                surface.hover = Some((kind, next));
                ctrl.distance = FLIGHT_ZOOM * 1.05;
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

    // Ciel : couleur de l'atmosphère le jour (coucher de soleil près de l'horizon), noir dans l'espace
    if surface.active() {
        let cam_local = cam_tf.translation - center;
        let up = cam_local.normalize_or(Vec3::Y);
        let altitude = cam_local.length() - params.radius;
        let sun = ctx
            .stars
            .iter()
            .map(|t| t.translation - center)
            .min_by(|a, b| a.length_squared().total_cmp(&b.length_squared()))
            .map(|d| d.normalize_or(Vec3::Y));
        let height = sun.map_or(-1.0, |s| up.dot(s));
        let air = (1.0 - altitude / atmosphere_depth(&params)).clamp(0.0, 1.0);
        let space = SPACE_SKY.to_srgba();
        let c = sky_color(&params, height, air, [space.red, space.green, space.blue]);
        clear.0 = Color::srgb(c[0], c[1], c[2]);
        surface.daylight = daylight(&params, height, air);
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
    cam_q: Query<&Transform, (With<Camera3d>, Without<PlanetRoot>, Without<MoonRoot>)>,
    mut dim: ResMut<GalaxyDim>,
) {
    let Ok(cam) = cam_q.get_single() else { return };
    let body = match target.0 {
        TargetKind::Planet(id) => planets.iter().find(|(_, p)| p.0 == id).map(|(t, _)| t.translation),
        TargetKind::Moon(pid, mi) => moons.iter().find(|(_, m)| m.planet_idx == pid && m.moon_idx == mi).map(|(t, _)| t.translation),
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
    stars: Query<&Transform, With<StarRoot>>,
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
        _ => None,
    };
    let Some(center) = center else { return };
    let Some(to_star) = stars
        .iter()
        .map(|t| t.translation - center)
        .min_by(|a, b| a.length_squared().total_cmp(&b.length_squared()))
        .and_then(|d| d.try_normalize())
    else {
        return;
    };
    let cam_local = cam.translation - center;
    let up = cam_local.normalize_or(Vec3::Y);
    let altitude = cam_local.length() - params.radius;
    let air = (1.0 - altitude / atmosphere_depth(&params)).clamp(0.0, 1.0);
    let day = daylight(&params, up.dot(to_star), air);
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
    ambient.brightness = night + sky_light + MOONLIGHT_MAX * moon;
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

/// Dans le noir, la lampe suit le regard du marcheur ; en vol, les phares éclairent devant et
/// sous le vaisseau. Leur puissance est réglée sur la lumière de l'étoile à midi (moitié de
/// celle-ci à quelques mètres), quelle que soit la distance de la planète à son étoile.
#[allow(clippy::type_complexity)]
fn update_lamps(
    surface: Res<Surface>,
    stars: Query<(&GlobalTransform, &PointLight), Without<Lamp>>,
    ship_q: Query<&Transform, (With<Ship>, Without<Lamp>, Without<Camera3d>)>,
    cam_q: Query<&Transform, (With<Camera3d>, Without<Lamp>, Without<Ship>)>,
    mut lamps: Query<(&Lamp, &mut SpotLight, &mut Transform, &mut Visibility), (Without<Ship>, Without<Camera3d>)>,
) {
    let walking = surface.phase == Phase::Walking;
    let on = surface.lamps && surface.dark() && !surface.gaseous();
    let (Ok(ship), Ok(cam)) = (ship_q.get_single(), cam_q.get_single()) else { return };
    let voxel = surface.terrain.as_ref().map_or(10.0, |t| t.voxel());
    // Éclairement de l'étoile au niveau de la planète (lux), d'après la plus brillante
    let sun = stars
        .iter()
        .map(|(gt, l)| l.intensity / (4.0 * std::f32::consts::PI * gt.translation().distance_squared(cam.translation).max(1.0)))
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
            (scale * 25.0 + 60.0, Transform::from_translation(ship.translation + *ship.forward() * scale * 1.2).looking_to(dir, up))
        } else {
            (voxel * 6.0, Transform::from_translation(cam.translation + *cam.down() * voxel * 0.3).looking_to(*cam.forward(), *cam.up()))
        };
        *tf = place;
        light.intensity = 0.5 * sun * 4.0 * std::f32::consts::PI * reach * reach;
        light.range = reach * 12.0;
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
    generation: u32,
}

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
    surface: Res<Surface>,
    mut store: ResMut<TileStore>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    decor: Res<DecorAssets>,
    cam_q: Query<&Transform, With<Camera3d>>,
    planets: Query<(Entity, &PlanetId, &Transform), (With<PlanetRoot>, Without<Camera3d>)>,
    moons: Query<(Entity, &MoonId, &Transform), (With<MoonRoot>, Without<Camera3d>)>,
    children: Query<&Children>,
    far: Query<(), With<FarMesh>>,
    mut vis: Query<&mut Visibility>,
) {
    // Pas de tuiles dans une géante gazeuse : sa sphère reste affichée
    let wanted = if surface.active() && !surface.gaseous() { surface.body } else { None };

    // Changement (ou fin) de séjour : on jette les tuiles et on rend le maillage lointain
    if store.body != wanted {
        if let Some(old) = store.body {
            let root = find_root(&old, &planets, &moons).map(|(e, _)| e);
            for (_, entry) in store.built.drain() {
                commands.entity(entry.entity).despawn_recursive();
            }
            store.tasks.clear();
            if let Some(root) = root {
                set_far_visibility(root, true, &children, &far, &mut vis);
            }
        }
        store.far_hidden = false;
        store.body = wanted;
        store.climate = None;
    }
    let Some(kind) = store.body else { return };
    let (Some(terrain), Ok(cam)) = (surface.terrain.as_ref(), cam_q.get_single()) else { return };
    let Some((root, root_tf)) = find_root(&kind, &planets, &moons) else { return };
    let params = terrain.params;
    let layout = terrain.layout;
    let now = time.elapsed_secs_f64();
    if store.climate != Some(params.climate) {
        // Pas de nouvelle saison tant que la précédente n'est pas finie (temps très accéléré)
        let rebuilding = store.built.values().any(|e| e.generation != store.generation);
        if store.climate.is_none() || !rebuilding {
            store.climate = Some(params.climate);
            store.generation = store.generation.wrapping_add(1);
        }
    }
    let generation = store.generation;
    let params = BodyParams { climate: store.climate.unwrap_or(params.climate), ..params };

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
    select_tiles(layout, params.radius, cam_local, &mut leaves);
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
            let entity = spawn_tile(&mut commands, &mut meshes, &material, root, build_tile_mesh(&params, key));
            store.built.insert(key, TileEntry { entity, last_needed: now, generation });
        }
    }

    // Récupère les tuiles terminées
    let finished: Vec<TileKey> = store.tasks.keys().copied().collect();
    for key in finished {
        let Some((task, built_gen)) = store.tasks.get_mut(&key) else { continue };
        let built_gen = *built_gen;
        if let Some((mesh, objects)) = block_on(future::poll_once(task)) {
            store.tasks.remove(&key);
            let entity = spawn_tile(&mut commands, &mut meshes, &material, root, mesh);
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

    // Lance les constructions manquantes : d'abord les grosses tuiles, puis les plus proches
    // (les tuiles d'une ancienne saison passent après les manquantes)
    let mut missing: Vec<(bool, u8, f32, TileKey)> = needed
        .iter()
        .filter(|k| !store.tasks.contains_key(k) && store.built.get(k).is_none_or(|e| e.generation != generation))
        .map(|k| (store.built.contains_key(k), k.depth, (cam_local - k.center_dir() * params.radius).length_squared(), *k))
        .collect();
    missing.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.total_cmp(&b.2)));
    let pool = AsyncComputeTaskPool::get();
    for (_, _, _, key) in missing {
        if store.tasks.len() >= MAX_TILE_TASKS {
            break;
        }
        let p = params;
        store.tasks.insert(
            key,
            (
                pool.spawn(async move {
                    let terrain = Terrain::new(p);
                    (build_tile_mesh_with(&terrain, key), tile_decor(&terrain, key))
                }),
                generation,
            ),
        );
    }

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

    for (key, entry) in store.built.iter_mut() {
        if needed.contains(key) {
            entry.last_needed = now;
        }
        if let Ok(mut v) = vis.get_mut(entry.entity) {
            let wanted = if shown.contains(key) { Visibility::Inherited } else { Visibility::Hidden };
            if *v != wanted {
                *v = wanted;
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
) -> Option<(Entity, Transform)> {
    match *kind {
        TargetKind::Planet(id) => planets.iter().find(|(_, p, _)| p.0 == id).map(|(e, _, t)| (e, *t)),
        TargetKind::Moon(planet_idx, moon_idx) => moons
            .iter()
            .find(|(_, m, _)| m.planet_idx == planet_idx && m.moon_idx == moon_idx)
            .map(|(e, _, t)| (e, *t)),
        _ => None,
    }
}

fn spawn_tile(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    material: &Handle<StandardMaterial>,
    root: Entity,
    mesh: Mesh,
) -> Entity {
    let entity = commands
        .spawn((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(material.clone()),
            Transform::IDENTITY,
            Visibility::Hidden,
            NotShadowCaster,
            SurfaceTile,
        ))
        .id();
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
            "ZQSD/WASD : voler   A/D : tourner   Maj : accelerer   Espace/Ctrl : monter/descendre\nClic droit : orbiter   Molette : zoom (>1000 : orbite)   V : atterrir   N : phares\n{}",
            weather.short()
        ),
        Phase::Descending => "Atterrissage en cours...".to_string(),
        Phase::Ascending => "Decollage en cours...".to_string(),
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
            format!(
                "ZQSD/WASD : marcher   Maj : courir   Espace : sauter   V : decoller   N : lampe\nLat {lat:.1}  Lon {lon:.1}  Alt {alt:.0}   {now}{}",
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
        })
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
        let t = world();
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
        for sys in settings.systems.iter().take(150) {
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
            assert!(l.voxel > 3.0 && l.voxel <= 11.0, "voxel {} pour un rayon de {}", l.voxel, params.radius);
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
                        assert!(w.pos.length() >= t.ground(w.up()).top - t.voxel() * 1.5, "sous le sol (rayon {})", params.radius);
                    }
                }
                // Le quadtree reste borné et ses tuiles sont valides
                let cam = dir * (g.top + t.voxel() * 2.0);
                let mut tiles = Vec::new();
                select_tiles(l, params.radius, cam, &mut tiles);
                assert!(tiles.len() < 700, "{} tuiles pour un rayon de {}", tiles.len(), params.radius);
                let key = *tiles.iter().max_by_key(|k| k.depth).unwrap();
                let mesh = build_tile_mesh(&params, key);
                assert!(mesh.count_vertices() > 1000);
                let _ = TileKey::root(0);
            }
        }
    }
}
