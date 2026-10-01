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
use crate::terrain::{build_tile_mesh, select_tiles, BodyParams, Terrain, TileKey};
use crate::ui::{CameraTarget, MenuState, TargetKind};
use crate::{CameraController, ZoomLevel};

/// Couleur du ciel dans l'espace (celle de `setup_scene`).
const SPACE_SKY: Color = Color::srgb(0.005, 0.005, 0.02);
const DAY_SKY: [f32; 3] = [0.36, 0.58, 0.92];

/// Éclairement du soleil pendant un séjour (lux) : la lumière ponctuelle de l'étoile est bien trop
/// faible, à cette distance, pour lire le relief à hauteur d'homme.
const SUN_LUX: f32 = 4_500.0;
/// Lumière ambiante : celle de l'espace, et celle de jour sous une atmosphère.
const AMBIENT_SPACE: f32 = 300.0;
const AMBIENT_NIGHT: f32 = 600.0;
const AMBIENT_DAY: f32 = 2_500.0;
const AMBIENT_AIRLESS: f32 = 1_300.0;
/// Teinte de la lumière diffuse pendant un séjour (plus claire que celle de l'espace).
const AMBIENT_TINT: Color = Color::srgb(0.55, 0.6, 0.75);
const AMBIENT_SPACE_TINT: Color = Color::srgb(0.25, 0.25, 0.35);

/// Nombre maximum de tuiles construites en même temps en arrière-plan.
const MAX_TILE_TASKS: usize = 10;
/// Au-delà, les tuiles inutilisées depuis longtemps sont supprimées.
const MAX_TILES: usize = 520;
const TILE_KEEP_SECS: f64 = 8.0;

pub struct SurfacePlugin;

impl Plugin for SurfacePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Surface>()
            .init_resource::<TileStore>()
            .add_systems(Startup, setup_hud)
            .add_systems(
                Update,
                (surface_control, surface_light, update_tiles, update_hud)
                    .chain()
                    .after(crate::planet::orbit_planets)
                    .after(crate::planet::orbit_moons),
            );
    }
}

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
        TargetKind::Planet(id) => settings.systems.get(id / 1000)?.planets.get(id % 1000).map(BodyParams::planet),
        TargetKind::Moon(planet_id, moon) => {
            let planet = settings.systems.get(planet_id / 1000)?.planets.get(planet_id % 1000)?;
            Some(BodyParams::moon(planet.moons.get(moon)?, planet))
        }
        _ => None,
    }
}

/// Rayon (depuis le centre) auquel le vaisseau stationne au-dessus d'un astre.
pub fn hover_radius(p: &BodyParams) -> f32 {
    p.radius + p.terrain_height * 0.6 + p.radius * 0.12
}

/// Distance minimale de la caméra au vaisseau autour d'un astre.
pub fn min_camera_distance(p: &BodyParams) -> f32 {
    p.radius * 0.3
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
        self.in_water = ground.kind == crate::planet::VoxelType::Water;
        let gravity = (if t.params.airless { MOON_GRAVITY } else { GRAVITY }) * v;
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
const GRAVITY: f32 = 22.0;
const MOON_GRAVITY: f32 = 7.0;

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
        }
    }
}

impl Surface {
    /// Atterrissage, séjour ou décollage en cours : la caméra n'est plus pilotée par l'orbite.
    pub fn active(&self) -> bool {
        self.phase != Phase::Orbit
    }

    /// Direction (depuis le centre de l'astre) au-dessus de laquelle le vaisseau stationne.
    pub fn hover_dir(&self, kind: &TargetKind) -> Option<Vec3> {
        self.hover.filter(|(k, _)| k == kind).map(|(_, d)| d)
    }

    /// Identifiant de la planète dont le maillage lointain est remplacé par les tuiles.
    pub fn active_planet(&self) -> Option<usize> {
        match (self.phase, self.body) {
            (Phase::Orbit, _) => None,
            (_, Some(TargetKind::Planet(id))) => Some(id),
            _ => None,
        }
    }

    fn params(&self) -> Option<BodyParams> {
        self.terrain.as_ref().map(|t| t.params)
    }

    fn abort(&mut self) {
        self.phase = Phase::Orbit;
        self.body = None;
        self.terrain = None;
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
pub fn level_ship(ship: &mut Transform, center: Vec3) {
    let up = (ship.translation - center).normalize_or(Vec3::Y);
    let forward = tangent(*ship.forward(), up);
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
    /// Centre (monde) d'un astre chargé.
    fn center(&self, kind: &TargetKind) -> Option<Vec3> {
        match *kind {
            TargetKind::Planet(id) => self.planets.iter().find(|(_, p)| p.0 == id).map(|(t, _)| t.translation),
            TargetKind::Moon(planet_idx, moon_idx) => self
                .moons
                .iter()
                .find(|(_, m)| m.planet_idx == planet_idx && m.moon_idx == moon_idx)
                .map(|(t, _)| t.translation),
            _ => None,
        }
    }

    /// Point de la surface pointé par la souris (sinon, le côté tourné vers la caméra).
    fn aimed_dir(&self, camera: &Camera, cam_gt: &GlobalTransform, center: Vec3, p: &BodyParams) -> Vec3 {
        let sphere = p.radius + p.terrain_height * 0.2;
        if let Some(cursor) = self.windows.get_single().ok().and_then(|w| w.cursor_position()) {
            if let Ok(ray) = camera.viewport_to_world(cam_gt, self.viewport.to_viewport(cursor)) {
                let origin = ray.origin - center;
                if let Some(t) = ray_sphere(origin, *ray.direction, sphere) {
                    return (origin + *ray.direction * t).normalize();
                }
            }
        }
        (cam_gt.translation() - center).normalize_or(Vec3::Y)
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
    let enter = (ctx.keys.just_pressed(KeyCode::Enter) || ctx.keys.just_pressed(KeyCode::NumpadEnter)) && !ui_open;

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
        // Zoom sous 1000 sur une planète ou une lune : navigation autour de l'astre
        if ctrl.distance < FLIGHT_ZOOM {
            if let (Some(params), Some(center)) = (body_params(&ctx.settings, &kind), ctx.center(&kind)) {
                let local = ship_tf.translation - center;
                if (local.length() - hover_radius(&params)).abs() < 400.0 {
                    let up = local.normalize_or(Vec3::Y);
                    surface.terrain = Some(Terrain::new(params));
                    surface.body = Some(kind);
                    surface.fpos = local;
                    surface.heading = tangent(*ship_tf.forward(), up);
                    surface.fspeed = 0.0;
                    surface.fvert = 0.0;
                    surface.fdist = ctrl.distance.clamp(60.0, FLIGHT_ZOOM * 0.95);
                    surface.fyaw = 0.0;
                    surface.fpitch = 0.35;
                    surface.phase = Phase::Flying;
                    *ship_vis = Visibility::Inherited;
                    net.notify("Navigation : ZQSD/WASD voler, Maj accelerer, Espace/Ctrl monter/descendre, Entree atterrir, molette pour revenir.", now);
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
        let Some(center) = ctx.center(&kind) else {
            net.notify("Cet astre est trop loin : approchez-vous de son systeme.", now);
            return;
        };
        let terrain = Terrain::new(params);
        let dir1 = ctx.aimed_dir(camera, cam_gt, center, &params);
        let local0 = ship_tf.translation - center;
        let dir0 = local0.normalize_or(dir1);
        let heading = dir1 - dir0 * dir0.dot(dir1);
        begin_descent(&mut surface, kind, terrain, dir0, local0.length(), dir1, heading, ship_tf.scale.x, *cam_tf);
        *ship_vis = Visibility::Inherited;
        net.notify("Atterrissage... (Entree pour redecoller une fois au sol)", now);
        return;
    }

    // Le corps visité a disparu (changement de système) : retour en orbite
    let Some(kind) = surface.body else {
        surface.abort();
        return;
    };
    let (Some(center), Some(params)) = (ctx.center(&kind), surface.params()) else {
        surface.abort();
        clear.0 = SPACE_SKY;
        set_cursor(&mut ctx.windows, false);
        net.notify("Retour en orbite : l'astre n'est plus charge.", now);
        return;
    };
    *zoom = ZoomLevel::Planet;
    *ship_vis = Visibility::Inherited;

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
            let ship_pos = center + dir * r;
            let ship_rot = look(Vec3::ZERO, surface.heading, dir).rotation;
            *ship_tf = Transform { translation: ship_pos, rotation: ship_rot, scale: Vec3::splat(scale) };

            surface.cam_blend = (surface.cam_blend + dt / 0.8).min(1.0);
            let chase = chase_pose(ship_pos, dir, surface.heading, scale);
            *cam_tf = blend_pose(&surface.cam_from, chase, smoothstep(surface.cam_blend));

            if u >= 1.0 {
                if descending {
                    let right = surface.heading.cross(dir).normalize_or_zero();
                    let side = (dir * r + right * (scale * 5.0 + 12.0)).normalize();
                    let walker = Walker::spawn(surface.terrain.as_ref().unwrap(), side, -right);
                    surface.ship_local = dir * r;
                    surface.ship_rot = ship_rot;
                    surface.ship_scale = scale;
                    surface.walker = walker;
                    surface.cam_from = *cam_tf;
                    surface.cam_blend = 0.0;
                    surface.phase = Phase::Walking;
                    net.notify("ZQSD/WASD : marcher  Maj : courir  Espace : sauter  Entree : decoller", now);
                } else {
                    // De retour en orbite : le vaisseau stationne au-dessus du point de décollage
                    surface.hover = Some((kind, dir));
                    ctrl.distance = min_camera_distance(&params);
                    ctrl.zoom_goal = Some(params.radius * 0.9);
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

            // Le vaisseau reste posé là où il a atterri
            *ship_tf = Transform {
                translation: center + surface.ship_local,
                rotation: surface.ship_rot,
                scale: Vec3::splat(surface.ship_scale),
            };

            let up = walker.up();
            let eye = center + up * walker.eye_r;
            let fps = look(eye, walker.view_dir(), up);
            surface.cam_blend = (surface.cam_blend + dt / 0.8).min(1.0);
            *cam_tf = blend_pose(&surface.cam_from, fps, smoothstep(surface.cam_blend));

            if enter {
                let dir = surface.ship_local.normalize();
                surface.dir0 = dir;
                surface.dir1 = dir;
                surface.r0 = surface.ship_local.length();
                surface.r1 = hover_radius(&params);
                surface.scale0 = surface.ship_scale;
                surface.scale1 = min_camera_distance(&params) * 0.008;
                surface.heading = tangent(surface.ship_rot * Vec3::NEG_Z, dir);
                surface.t = 0.0;
                surface.dur = (2.5 + (surface.r1 - surface.r0) / 8000.0).clamp(2.5, 6.0);
                surface.cam_from = *cam_tf;
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
                    surface.fdist *= (-wheel * 0.12).exp();
                }
            }

            // Cap, vitesse et altitude
            heading = (Quat::from_axis_angle(up, turn * 1.3 * dt) * heading).normalize();
            let top_speed = (terrain.params.radius * 0.1).clamp(300.0, 1500.0) * if boost { 4.0 } else { 1.0 };
            surface.fspeed += (forward * top_speed - surface.fspeed) * (1.0 - (-2.0 * dt).exp());
            surface.fvert += (vertical * 400.0 - surface.fvert) * (1.0 - (-4.0 * dt).exp());
            let next = (surface.fpos + heading * surface.fspeed * dt).normalize();
            let ground = terrain.ground(next).top;
            let ceiling = hover_radius(&params);
            let r = (r + surface.fvert * dt).clamp(ground + 25.0, ceiling.max(ground + 100.0));
            surface.fpos = next * r;
            surface.heading = tangent(heading, next);

            let scale = (surface.fdist * 0.025).clamp(2.0, 40.0);
            let ship_pos = center + surface.fpos;
            let ship_rot = look(Vec3::ZERO, surface.heading, next).rotation;
            *ship_tf = Transform { translation: ship_pos, rotation: ship_rot, scale: Vec3::splat(scale) };

            // Caméra derrière le vaisseau, orientable à la souris, jamais sous le relief
            let back = Quat::from_axis_angle(next, surface.fyaw) * -surface.heading;
            let offset = (back * surface.fpitch.cos() + next * surface.fpitch.sin()) * surface.fdist;
            let mut cam_local = surface.fpos + offset;
            let floor = terrain.ground(cam_local.normalize()).top + 10.0;
            if cam_local.length() < floor {
                cam_local = cam_local.normalize() * floor;
            }
            let cam_pos = center + cam_local;
            *cam_tf = look(cam_pos, ship_pos + next * (0.4 * scale) - cam_pos, next);
            surface.terrain = Some(terrain);

            // Dézoom au-delà de 1000 : retour à la vue orbitale, au-dessus de l'endroit survolé
            if surface.fdist >= FLIGHT_ZOOM {
                surface.hover = Some((kind, next));
                ctrl.distance = FLIGHT_ZOOM * 1.05;
                ctrl.zoom_goal = None;
                ctrl.last_target_pos = center;
                surface.abort();
                clear.0 = SPACE_SKY;
            } else if enter {
                // Atterrir juste devant le vaisseau
                let terrain = surface.terrain.take().unwrap();
                let dir1 = (surface.fpos + surface.heading * scale * 6.0).normalize();
                let heading = surface.heading;
                let cam_from = *cam_tf;
                begin_descent(&mut surface, kind, terrain, next, r, dir1, heading, scale, cam_from);
                net.notify("Atterrissage...", now);
            }
        }
        Phase::Orbit => {}
    }

    // Ciel : bleu le jour près d'une atmosphère, noir dans l'espace
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
        let blue = if params.atmosphere {
            let day = sun.map_or(0.0, |s| smoothstep((up.dot(s) + 0.15) / 0.35));
            let air = (1.0 - altitude / (params.radius * 0.12)).clamp(0.0, 1.0);
            day * air
        } else {
            0.0
        };
        let space = SPACE_SKY.to_srgba();
        clear.0 = Color::srgb(
            space.red + (DAY_SKY[0] - space.red) * blue,
            space.green + (DAY_SKY[1] - space.green) * blue,
            space.blue + (DAY_SKY[2] - space.blue) * blue,
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  Lumière
// ─────────────────────────────────────────────────────────────────────────

/// Pendant un séjour, le soleil (lumière directionnelle) éclaire l'astre depuis son étoile : la
/// face éclairée, la nuit et le crépuscule viennent de l'ombrage, comme dans la réalité.
#[allow(clippy::too_many_arguments)]
fn surface_light(
    surface: Res<Surface>,
    mut ambient: ResMut<AmbientLight>,
    mut sun: Query<(&mut DirectionalLight, &mut Transform), (With<crate::SunLight>, Without<PlanetRoot>, Without<MoonRoot>, Without<StarRoot>)>,
    planets: Query<(&Transform, &PlanetId), With<PlanetRoot>>,
    moons: Query<(&Transform, &MoonId), With<MoonRoot>>,
    stars: Query<&Transform, With<StarRoot>>,
    cam_q: Query<&Transform, (With<Camera3d>, Without<PlanetRoot>, Without<MoonRoot>, Without<StarRoot>, Without<crate::SunLight>)>,
    mut was_active: Local<bool>,
) {
    let active = surface.active();
    if !active {
        if *was_active {
            *was_active = false;
            ambient.brightness = AMBIENT_SPACE;
            ambient.color = AMBIENT_SPACE_TINT;
            for (mut light, _) in &mut sun {
                light.illuminance = 0.0;
            }
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
    for (mut light, mut tf) in &mut sun {
        light.illuminance = SUN_LUX;
        light.shadows_enabled = false;
        *tf = Transform::IDENTITY.looking_to(Dir3::new(-to_star).unwrap_or(Dir3::NEG_Y), Dir3::Y);
    }

    // Lumière diffuse : l'atmosphère la renforce de jour (et la nuit tombe), sans air elle reste faible
    let cam_local = cam.translation - center;
    let up = cam_local.normalize_or(Vec3::Y);
    let altitude = cam_local.length() - params.radius;
    ambient.brightness = if params.atmosphere {
        let day = smoothstep((up.dot(to_star) + 0.15) / 0.35) * (1.0 - altitude / (params.radius * 0.12)).clamp(0.0, 1.0);
        AMBIENT_NIGHT + (AMBIENT_DAY - AMBIENT_NIGHT) * day
    } else {
        AMBIENT_AIRLESS
    };
    ambient.color = AMBIENT_TINT;
}

// ─────────────────────────────────────────────────────────────────────────
//  Tuiles de terrain
// ─────────────────────────────────────────────────────────────────────────

struct TileEntry {
    entity: Entity,
    last_needed: f64,
}

#[derive(Resource, Default)]
struct TileStore {
    body: Option<TargetKind>,
    built: HashMap<TileKey, TileEntry>,
    tasks: HashMap<TileKey, Task<Mesh>>,
    material: Option<Handle<StandardMaterial>>,
    far_hidden: bool,
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
    cam_q: Query<&Transform, With<Camera3d>>,
    planets: Query<(Entity, &PlanetId, &Transform), (With<PlanetRoot>, Without<Camera3d>)>,
    moons: Query<(Entity, &MoonId, &Transform), (With<MoonRoot>, Without<Camera3d>)>,
    children: Query<&Children>,
    far: Query<(), With<FarMesh>>,
    mut vis: Query<&mut Visibility>,
) {
    let wanted = if surface.active() { surface.body } else { None };

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
    }
    let Some(kind) = store.body else { return };
    let (Some(terrain), Ok(cam)) = (surface.terrain.as_ref(), cam_q.get_single()) else { return };
    let Some((root, center)) = find_root(&kind, &planets, &moons) else { return };
    let params = terrain.params;
    let layout = terrain.layout;
    let now = time.elapsed_secs_f64();

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

    // Tuiles voulues autour de la caméra, avec leurs ancêtres (repli le temps de la construction)
    let cam_local = cam.translation - center;
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
            store.built.insert(key, TileEntry { entity, last_needed: now });
        }
    }

    // Récupère les tuiles terminées
    let finished: Vec<TileKey> = store.tasks.keys().copied().collect();
    for key in finished {
        let Some(task) = store.tasks.get_mut(&key) else { continue };
        if let Some(mesh) = block_on(future::poll_once(task)) {
            store.tasks.remove(&key);
            let entity = spawn_tile(&mut commands, &mut meshes, &material, root, mesh);
            store.built.insert(key, TileEntry { entity, last_needed: now });
        }
    }

    // Lance les constructions manquantes : d'abord les grosses tuiles, puis les plus proches
    let mut missing: Vec<(u8, f32, TileKey)> = needed
        .iter()
        .filter(|k| !store.built.contains_key(k) && !store.tasks.contains_key(k))
        .map(|k| (k.depth, (cam_local - k.center_dir() * params.radius).length_squared(), *k))
        .collect();
    missing.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
    let pool = AsyncComputeTaskPool::get();
    for (_, _, key) in missing {
        if store.tasks.len() >= MAX_TILE_TASKS {
            break;
        }
        let p = params;
        store.tasks.insert(key, pool.spawn(async move { build_tile_mesh(&p, key) }));
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
) -> Option<(Entity, Vec3)> {
    match *kind {
        TargetKind::Planet(id) => planets.iter().find(|(_, p, _)| p.0 == id).map(|(e, _, t)| (e, t.translation)),
        TargetKind::Moon(planet_idx, moon_idx) => moons
            .iter()
            .find(|(_, m, _)| m.planet_idx == planet_idx && m.moon_idx == moon_idx)
            .map(|(e, _, t)| (e, t.translation)),
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
    mut hud: Query<&mut Text, With<SurfaceHud>>,
) {
    let label = match surface.phase {
        Phase::Orbit => match body_params(&settings, &target.0) {
            Some(_) => "Zoomez sous 1000 pour naviguer autour de l'astre   Entree : atterrir (visez un point de sa surface)".to_string(),
            None => String::new(),
        },
        Phase::Flying => "ZQSD/WASD : voler   A/D : tourner   Maj : accelerer   Espace/Ctrl : monter/descendre\nClic droit : orbiter   Molette : zoom (>1000 : orbite)   Entree : atterrir".to_string(),
        Phase::Descending => "Atterrissage en cours...".to_string(),
        Phase::Ascending => "Decollage en cours...".to_string(),
        Phase::Walking => {
            let w = &surface.walker;
            let up = w.up();
            let lat = up.y.clamp(-1.0, 1.0).asin().to_degrees();
            let lon = up.z.atan2(up.x).to_degrees();
            let params = surface.params();
            let radius = params.map_or(0.0, |p| p.radius);
            let temp = params.map_or(0.0, |p| p.temperature);
            let alt = w.pos.length() - radius;
            format!(
                "ZQSD/WASD : marcher   Maj : courir   Espace : sauter   Entree : decoller\nLat {lat:.1}  Lon {lon:.1}  Alt {alt:.0}  Temp. {temp:.0} C{}",
                if w.in_water { "  (a l'eau)" } else { "" }
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
}
