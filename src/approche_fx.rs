//! Effets de l'approche planétaire (0.13 bloc P) : plasma de rentrée, bang supersonique, cône et
//! traînées de condensation, ombre du vaisseau, poussière à l'atterrissage, nuages qui s'écartent,
//! brouillard dans les nuages, secousses, liseré bleu de l'atmosphère, feux de position.
//!
//! Tout dépend de la VITESSE et de la densité de l'air (`approche::FlightInfo`), jamais de
//! l'altitude seule (Q11) : lent = rien, rapide = flammes. Les effets posés dans le monde (ombre,
//! poussière, traînées, onde de choc) sont des enfants de la racine de l'astre : ils tournent avec
//! lui (règle 10) et suivent le recentrage de l'origine flottante.

#![allow(dead_code)]

use bevy::asset::load_internal_asset;
use bevy::pbr::{DistanceFog, ExtendedMaterial, FogFalloff, MaterialExtension, NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderRef, ShaderType};

use crate::planet::{CloudVoxel, MoonId, MoonRoot, PlanetId, PlanetRoot, StarId, StarRoot};
use crate::settings::GameSettings;
use crate::ship::Ship;
use crate::surface::{Surface, SurfaceControl};
use crate::ui::TargetKind;

pub const CLOUD_SHADER: Handle<Shader> = Handle::weak_from_u128(0x5ea_f00d_0a7e_c0de_1234_5678_9ac0);
pub const RIM_SHADER: Handle<Shader> = Handle::weak_from_u128(0x5ea_f00d_0a7e_c0de_1234_5678_9ac1);

// ─────────────────────────────────────────────────────────────────────────
//  Matériaux
// ─────────────────────────────────────────────────────────────────────────

/// Nuages : le matériau standard, effacé en tramage autour du vaisseau (`cloud_clear.wgsl`).
pub type CloudMaterial = ExtendedMaterial<StandardMaterial, CloudClear>;

#[derive(ShaderType, Reflect, Clone, Copy, Debug, Default)]
pub struct ClearParams {
    pub slots: [Vec4; 8],
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct CloudClear {
    #[uniform(100)]
    pub params: ClearParams,
}

impl MaterialExtension for CloudClear {
    fn fragment_shader() -> ShaderRef {
        CLOUD_SHADER.into()
    }
}

#[derive(ShaderType, Clone, Copy, Debug, Default)]
pub struct RimParams {
    pub color: Vec4,
    pub sun: Vec4,
    pub center: Vec4,
}

/// Liseré de l'atmosphère vu de l'espace.
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug, Default)]
pub struct RimMaterial {
    #[uniform(0)]
    pub params: RimParams,
}

impl Material for RimMaterial {
    fn fragment_shader() -> ShaderRef {
        RIM_SHADER.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  Plugin
// ─────────────────────────────────────────────────────────────────────────

/// Bang supersonique (passage du mur du son) : force 0..1, pour le son et les secousses.
#[derive(Event, Clone, Copy, Debug)]
pub struct Boom {
    pub strength: f32,
}

pub struct ApprocheFxPlugin;

impl Plugin for ApprocheFxPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, CLOUD_SHADER, "cloud_clear.wgsl", Shader::from_wgsl);
        load_internal_asset!(app, RIM_SHADER, "rim.wgsl", Shader::from_wgsl);
        app.add_plugins(MaterialPlugin::<CloudMaterial>::default())
            .add_plugins(MaterialPlugin::<RimMaterial>::default())
            .add_event::<Boom>()
            .add_event::<CloudCommand>()
            .add_systems(Update, go_cloud.before(SurfaceControl))
            .init_resource::<FxState>()
            .add_systems(
                Update,
                (spawn_ship_fx, ship_fx, remote_fx, body_fx, convert_clouds, cloud_clear, cloud_state, rim_fx, shake_camera)
                    .chain()
                    .after(SurfaceControl)
                    .after(crate::water::update_water),
            )
            .add_systems(Update, cloud_fog.after(crate::gas::gas_atmosphere).after(cloud_state));
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  État
// ─────────────────────────────────────────────────────────────────────────

/// `/nuage [dedans | dessus | dessous]` (tests, P4) : va dans la couche de nuages au-dessus (ou autour)
/// de la position, en vol bas.
#[derive(Event)]
pub struct CloudCommand(pub String);

fn go_cloud(
    time: Res<Time>,
    clock: Res<crate::world_clock::WorldClock>,
    settings: Res<GameSettings>,
    weather: Res<crate::weather::WeatherNow>,
    mut events: EventReader<CloudCommand>,
    mut surface: ResMut<Surface>,
    mut net: ResMut<crate::net::Net>,
) {
    let now = time.elapsed_secs_f64();
    for CloudCommand(arg) in events.read() {
        let (Some(TargetKind::Planet(id)), Some(w), Some(p0)) = (surface.body(), weather.params, surface.local_point()) else {
            net.notify("/nuage : volez bas sur une planete a nuages (zoom sous 1000).", now);
            continue;
        };
        let Some(p) = settings.systems.get(id / 1000).and_then(|s| s.planets().get(id % 1000)) else { continue };
        let cr = crate::weather::cloud_radius(p);
        let cell = std::f32::consts::FRAC_PI_2 * cr / 48.0;
        // Un endroit bien nuageux, le plus près possible
        let up0 = p0.normalize_or(Vec3::Y);
        let east = Vec3::Y.cross(up0).normalize_or(Vec3::X);
        let north = up0.cross(east);
        let cloudy = |d: Vec3| crate::weather::cloud_field(&w, d, clock.secs);
        let mut found = (cloudy(up0).0 > 0.6).then_some((up0, cloudy(up0).1, cloudy(up0).0));
        let mut ring = 0;
        while found.is_none() && ring < 40 {
            let ang = 0.6 * cell * 1.3f32.powi(ring) / cr;
            found = (0..16).find_map(|k| {
                let az = k as f32 * std::f32::consts::TAU / 16.0;
                let d = (up0 * ang.cos() + (east * az.cos() + north * az.sin()) * ang.sin()).normalize();
                let (c, t) = cloudy(d);
                (c > 0.6).then_some((d, t, c))
            });
            ring += 1;
        }
        let Some((dir, thick, c)) = found else {
            net.notify("Pas de nuage trouve (ciel degage partout).", now);
            continue;
        };
        let top = cr + cell * (0.25 + 0.75 * thick) * (1.0 + (c > 0.8) as u8 as f32);
        let r = match arg.trim() {
            "dessus" | "above" => top + 3.0 * cell,
            "dessous" | "below" => cr - 0.6 * cell,
            _ => 0.5 * (cr + top),
        };
        surface.teleport_flight(dir * r);
        net.notify(&format!("Dans les nuages : couche a {:.0} unites, epaisseur {:.0}.", cr - p.radius, top - cr), now);
    }
}

#[derive(Clone, Copy)]
struct Particle {
    entity: Entity,
    pos: Vec3,
    vel: Vec3,
    age: f32,
    life: f32,
    size0: f32,
    size1: f32,
    color: [f32; 3],
    alpha: f32,
    /// Ajoutée en ce moment (sinon libre).
    live: bool,
}

#[derive(Clone, Copy)]
struct Wake {
    pos: Vec3,
    age: f32,
    radius: f32,
}

#[derive(Clone, Copy)]
struct Ring {
    pos: Vec3,
    axis: Vec3,
    age: f32,
    len: f32,
    strength: f32,
    entity: Entity,
}

#[derive(Resource, Default)]
pub struct FxState {
    prev_mach: f32,
    prev_speed: f32,
    shake: f32,
    boom: f32,
    rng: u32,
    dust: Vec<Particle>,
    trail: Vec<Particle>,
    wake: Vec<Wake>,
    wake_acc: f32,
    ring: Option<Ring>,
    /// Anneaux de choc remplacés avant la fin : à retirer.
    dead: Vec<Entity>,
    /// Bang : côté du mur du son (-1 dessous, +1 dessus) et instant du dernier (anti-rebond).
    boom_side: i8,
    boom_at: f32,
    sparks: Vec<Particle>,
    spark_acc: f32,
    contrail_acc: f32,
    dust_acc: f32,
    /// Corps dont on a posé les effets (racine) ; ombre de l'ombre au sol.
    root: Option<Entity>,
    shadow: Option<Entity>,
    meshes: Option<FxMeshes>,
    /// Dans un nuage (0..1, lissé), givre sur la vitre (0..1), éclair récent.
    pub inside: f32,
    pub frost: f32,
    pub above_clouds: Option<f32>,
    rim: Option<(Entity, TargetKind)>,
    last_flash: f64,
}

#[derive(Clone)]
struct FxMeshes {
    sphere: Handle<Mesh>,
    cone: Handle<Mesh>,
    low: Handle<Mesh>,
    ring: Handle<Mesh>,
    disc: Handle<Mesh>,
}

impl FxState {
    fn rand(&mut self) -> f32 {
        self.rng = self.rng.wrapping_mul(1664525).wrapping_add(1013904223);
        ((self.rng >> 8) & 0xFFFF) as f32 / 65535.0
    }

    fn signed(&mut self) -> f32 {
        self.rand() * 2.0 - 1.0
    }
}

fn fx_meshes(state: &mut FxState, meshes: &mut Assets<Mesh>) -> FxMeshes {
    state
        .meshes
        .get_or_insert_with(|| FxMeshes {
            sphere: meshes.add(Sphere::new(1.0).mesh().ico(3).unwrap()),
            // Cône de rayon 1 et de hauteur 1 : base en bas (y = -0,5), pointe en haut
            cone: meshes.add(Cone::new(1.0, 1.0).mesh().resolution(24).build()),
            low: meshes.add(Sphere::new(1.0).mesh().ico(1).unwrap()),
            ring: meshes.add(Torus::new(0.93, 1.0).mesh().build()),
            disc: meshes.add(Circle::new(1.0).mesh().build()),
        })
        .clone()
}

fn glow(color: [f32; 3], alpha: f32) -> StandardMaterial {
    StandardMaterial { base_color: Color::srgba(color[0], color[1], color[2], alpha), unlit: true, alpha_mode: AlphaMode::Add, cull_mode: None, ..default() }
}

/// Un cône dont la base (rayon `r`) est au plan z = `z0` du vaisseau et dont la pointe va vers +Z sur
/// `len` : (centre, échelle) pour le maillage de cône unitaire, tourné de +Y vers +Z.
fn cone(r: f32, z0: f32, len: f32) -> (Vec3, Vec3) {
    (Vec3::new(0.0, 0.0, z0 + len * 0.5), Vec3::new(r, len, r))
}

fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Un astre humide : de la vapeur d'eau se condense derrière le vaisseau.
fn humid(p: &crate::terrain::BodyParams) -> bool {
    p.atmosphere && p.hydro.liquid == crate::planetgen::hydrology::Liquid::Water && p.pressure > 0.05
}

// ─────────────────────────────────────────────────────────────────────────
//  Effets portés par le vaisseau
// ─────────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum FxKind {
    Sheath,
    Core,
    Trail,
    Cone,
    RetroL,
    RetroR,
    NavRed,
    NavGreen,
    Strobe,
}

#[derive(Component)]
struct ShipFx(FxKind);

#[derive(Component)]
struct ShipFxLight;

fn spawn_ship_fx(
    mut commands: Commands,
    mut state: ResMut<FxState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    ship_q: Query<Entity, With<Ship>>,
    existing: Query<(), With<ShipFx>>,
) {
    if !existing.is_empty() {
        return;
    }
    let Ok(ship) = ship_q.get_single() else { return };
    let m = fx_meshes(&mut state, &mut meshes);
    let mut add = |kind: FxKind, color: [f32; 3], commands: &mut Commands| {
        let mesh = if matches!(kind, FxKind::Sheath | FxKind::Core | FxKind::Trail | FxKind::Cone) { m.cone.clone() } else { m.sphere.clone() };
        let e = commands
            .spawn((Mesh3d(mesh), MeshMaterial3d(materials.add(glow(color, 0.0))), Transform::default(), Visibility::Hidden, NotShadowCaster, NotShadowReceiver, ShipFx(kind)))
            .id();
        commands.entity(ship).add_child(e);
    };
    for (kind, color) in [
        (FxKind::Sheath, [1.0, 0.55, 0.2]),
        (FxKind::Core, [1.0, 0.9, 0.7]),
        (FxKind::Trail, [1.0, 0.5, 0.2]),
        (FxKind::Cone, [1.0, 1.0, 1.0]),
        (FxKind::RetroL, [0.6, 0.8, 1.0]),
        (FxKind::RetroR, [0.6, 0.8, 1.0]),
        (FxKind::NavRed, [1.0, 0.1, 0.1]),
        (FxKind::NavGreen, [0.1, 1.0, 0.2]),
        (FxKind::Strobe, [1.0, 1.0, 1.0]),
    ] {
        add(kind, color, &mut commands);
    }
    let light = commands
        .spawn((PointLight { intensity: 0.0, range: 10.0, shadows_enabled: false, color: Color::srgb(1.0, 0.6, 0.3), ..default() }, Transform::from_xyz(0.0, 0.0, -0.8), ShipFxLight))
        .id();
    commands.entity(ship).add_child(light);
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn ship_fx(
    time: Res<Time>,
    surface: Res<Surface>,
    mut state: ResMut<FxState>,
    mut booms: EventWriter<Boom>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    ship_q: Query<(&Transform, &Visibility), (With<Ship>, Without<ShipFx>, Without<ShipFxLight>)>,
    cam_q: Query<&Transform, (With<Camera3d>, Without<Ship>, Without<ShipFx>, Without<ShipFxLight>)>,
    mut fx: Query<(&ShipFx, &MeshMaterial3d<StandardMaterial>, &mut Transform, &mut Visibility), (Without<Ship>, Without<ShipFxLight>, Without<Camera3d>)>,
    mut lights: Query<(&mut PointLight, &mut Transform), (With<ShipFxLight>, Without<Ship>, Without<ShipFx>, Without<Camera3d>)>,
) {
    let dt = time.delta_secs().min(0.1);
    let t = time.elapsed_secs();
    let f = *surface.flight();
    let Ok((ship_tf, _)) = ship_q.get_single() else { return };
    let params = surface.params();
    let l = surface.ship_dims.icon_len;
    let s = ship_tf.scale.x.max(1e-6);
    let len_world = l * s;
    let cam_dist = cam_q.get_single().map_or(len_world * 4.0, |c| c.translation.distance(ship_tf.translation));

    // Chaleur de rentrée : géantes gazeuses, plus violent (P3)
    let mut heat = if f.active { f.heat } else { 0.0 };
    if params.is_some_and(|p| p.gaseous) {
        heat = (heat * 1.6).min(1.0);
    }
    let plasma = params.map_or(crate::approche::PLASMA_DEFAULT, |p| p.plasma);

    // Cône de condensation près de Mach 1 dans un air humide (P6)
    let cond = if f.active && params.is_some_and(|p| humid(&p)) { smooth(0.12, 0.0, (f.mach - 1.0).abs()) * smooth(0.05, 0.3, f.density) } else { 0.0 };
    // Rétrofusées : le vaisseau freine dans le vide (P3, « sans air : pas de flammes, rétrofusées »)
    let decel = if dt > 0.0 { (state.prev_speed - f.speed_vox) / dt } else { 0.0 };
    let retro = if f.active && f.density < 0.02 && f.speed_vox > 30.0 { (decel / f.speed_vox.max(1.0) * 1.2).clamp(0.0, 1.0) } else { 0.0 };

    // Bang supersonique : franchissement du mur du son, avec de l'air
    // (anti-rebond : le mur du son se franchit en accélérant, de franchement dessous à franchement
    // dessus, et pas plus d'un bang toutes les 4 s ; ralentir ou reculer doucement n'en fait pas)
    if f.active {
        if f.mach < 0.9 {
            state.boom_side = -1;
        }
        if f.mach > 1.12 && state.boom_side < 0 && f.density > 0.02 && f.speed_vox > 120.0 && t - state.boom_at > 4.0 && f.vert_vox.abs() <= f.speed_vox {
            state.boom_side = 1;
            state.boom_at = t;
            let strength = (f.density.sqrt() * 0.8 + 0.2).clamp(0.2, 1.0);
            state.boom = state.boom.max(strength);
            booms.send(Boom { strength });
            if let Some(old) = state.ring.take() {
                if old.entity != Entity::PLACEHOLDER {
                    state.dead.push(old.entity);
                }
            }
            state.ring = Some(Ring { pos: f.pos, axis: f.heading, age: 0.0, len: f.length, strength, entity: Entity::PLACEHOLDER });
        }
    } else {
        state.boom_side = 0;
    }
    state.prev_mach = if f.active { f.mach } else { 0.0 };
    state.prev_speed = if f.active { f.speed_vox } else { 0.0 };

    let flicker = |i: f32| 1.0 + 0.08 * (t * 41.0 + i * 1.7).sin() + 0.05 * (t * 67.0 + i).sin();
    let strobe = (t % 1.3) < 0.09;
    let blink = (t % 1.0) < 0.55;
    let nav_size = (0.035 * l).max(cam_dist * 0.0035 / s);
    for (kind, mat, mut tf, mut vis) in &mut fx {
        let (alpha, color, center, scale): (f32, [f32; 3], Vec3, Vec3) = match kind.0 {
            // Le plasma est un cône qui part du nez et s'effile vers l'arrière (pointe vers +Z)
            FxKind::Sheath => {
                let (c, s) = cone(0.5 * l, -0.55 * l, (1.4 + 1.8 * heat) * l * flicker(0.0));
                (0.3 * heat, plasma, c, s)
            }
            FxKind::Core => {
                let white = [plasma[0] + (1.0 - plasma[0]) * 0.75, plasma[1] + (1.0 - plasma[1]) * 0.75, plasma[2] + (1.0 - plasma[2]) * 0.75];
                let (c, s) = cone(0.26 * l, -0.5 * l, (0.9 + 1.2 * heat) * l * flicker(2.0));
                (0.55 * heat, white, c, s)
            }
            FxKind::Trail => {
                let (c, s) = cone(0.34 * l, 0.1 * l, (2.5 + 7.0 * heat) * l * flicker(4.0));
                (0.22 * heat, plasma, c, s)
            }
            FxKind::Cone => {
                let (c, s) = cone(0.5 * l, 0.25 * l, 1.6 * l);
                (0.4 * cond, [1.0, 1.0, 1.0], c, s)
            }
            FxKind::RetroL => (0.8 * retro, [0.6, 0.8, 1.0], Vec3::new(-0.2 * l, 0.0, -0.75 * l), Vec3::new(0.1 * l, 0.1 * l, 0.5 * l) * flicker(6.0)),
            FxKind::RetroR => (0.8 * retro, [0.6, 0.8, 1.0], Vec3::new(0.2 * l, 0.0, -0.75 * l), Vec3::new(0.1 * l, 0.1 * l, 0.5 * l) * flicker(7.0)),
            FxKind::NavRed => (if blink && f.active { 1.0 } else { 0.0 }, [1.0, 0.1, 0.1], Vec3::new(-0.5 * l, 0.0, 0.1 * l), Vec3::splat(nav_size)),
            FxKind::NavGreen => (if blink && f.active { 1.0 } else { 0.0 }, [0.1, 1.0, 0.25], Vec3::new(0.5 * l, 0.0, 0.1 * l), Vec3::splat(nav_size)),
            FxKind::Strobe => (if strobe && f.active { 1.0 } else { 0.0 }, [1.0, 1.0, 1.0], Vec3::new(0.0, 0.18 * l, 0.45 * l), Vec3::splat(nav_size * 1.6)),
        };
        let on = alpha > 0.01;
        let v = if on { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != v {
            *vis = v;
        }
        if !on {
            continue;
        }
        tf.translation = center;
        tf.scale = scale;
        tf.rotation = if matches!(kind.0, FxKind::Sheath | FxKind::Core | FxKind::Trail | FxKind::Cone) { Quat::from_rotation_arc(Vec3::Y, Vec3::Z) } else { Quat::IDENTITY };
        if let Some(m) = materials.get_mut(&mat.0) {
            m.base_color = Color::srgba(color[0], color[1], color[2], alpha.clamp(0.0, 1.0));
        }
    }
    // Lueur orange sur la coque et autour (l'éclairement est celui d'un point à une longueur)
    for (mut light, mut tf) in &mut lights {
        light.color = Color::srgb(plasma[0], plasma[1], plasma[2]);
        light.intensity = heat * 1.5e5 * len_world * len_world;
        light.range = (len_world * 40.0).max(1.0);
        tf.translation = Vec3::new(0.0, 0.0, -0.8 * l);
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  Effets posés sur l'astre : ombre, poussière, traînées, onde de choc, sillage dans les nuages
// ─────────────────────────────────────────────────────────────────────────

#[derive(Component)]
struct BodyFx;

/// Racine de l'astre où l'on vole (planète ou lune).
fn root_of(kind: TargetKind, planets: &Query<(Entity, &PlanetId), With<PlanetRoot>>, moons: &Query<(Entity, &MoonId), With<MoonRoot>>) -> Option<Entity> {
    match kind {
        TargetKind::Planet(id) => planets.iter().find(|(_, p)| p.0 == id).map(|(e, _)| e),
        TargetKind::Moon(pi, mi) => moons.iter().find(|(_, m)| m.planet_idx == pi && m.moon_idx == mi).map(|(e, _)| e),
        _ => None,
    }
}

fn dust_color(kind: crate::planet::VoxelType) -> [f32; 3] {
    use crate::planet::VoxelType as V;
    match kind {
        V::Sand | V::Salt | V::Savanna | V::Steppe | V::Sulfur => [0.78, 0.68, 0.48],
        V::Snow | V::Ice => [0.93, 0.95, 1.0],
        V::Water | V::Methane | V::Ammonia => [0.85, 0.92, 0.98],
        V::Basalt | V::Glass | V::Lava => [0.32, 0.3, 0.3],
        V::Grass | V::Forest | V::Jungle | V::Taiga | V::Swamp | V::Tundra => [0.45, 0.42, 0.3],
        V::Rust => [0.62, 0.34, 0.2],
        _ => [0.55, 0.52, 0.48],
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn body_fx(
    mut commands: Commands,
    time: Res<Time>,
    clock: Res<crate::world_clock::WorldClock>,
    surface: Res<Surface>,
    settings: Res<GameSettings>,
    weather: Res<crate::weather::WeatherNow>,
    mut state: ResMut<FxState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    planets: Query<(Entity, &PlanetId), With<PlanetRoot>>,
    moons: Query<(Entity, &MoonId), With<MoonRoot>>,
    mut tfs: Query<&mut Transform, With<BodyFx>>,
    fx_ents: Query<Entity, With<BodyFx>>,
    suns: Query<(&crate::surface::SurfaceSun, &DirectionalLight, &Transform), Without<BodyFx>>,
    mut vis: Query<&mut Visibility, With<BodyFx>>,
    mut mats: Query<&MeshMaterial3d<StandardMaterial>, With<BodyFx>>,
) {
    let dt = time.delta_secs().min(0.1);
    let f = *surface.flight();
    let body = surface.body();
    let root = body.and_then(|k| root_of(k, &planets, &moons));
    // Autre astre (ou plus de vol) : on retire tout
    if root != state.root || !f.active {
        if state.root.is_some() {
            for e in &fx_ents {
                commands.entity(e).despawn_recursive();
            }
            state.dust.clear();
            state.trail.clear();
            state.sparks.clear();
            state.ring = None;
            state.shadow = None;
            state.wake.clear();
        }
        state.root = if f.active { root } else { None };
        if !f.active {
            state.above_clouds = None;
            return;
        }
    }
    for e in std::mem::take(&mut state.dead) {
        if let Some(mut c) = commands.get_entity(e) {
            c.despawn_recursive();
        }
    }
    let (Some(root), Some(terrain)) = (root, surface.terrain()) else { return };
    let params = terrain.params;
    let m = fx_meshes(&mut state, &mut meshes);
    let len = f.length.max(1e-3);
    let real_len = surface.ship_dims.real_scale(f.voxel) * surface.ship_dims.icon_len;
    let up = f.up;

    // Lumière du soleil le plus brillant, dans le repère de l'astre
    let to_local = surface.frame().map_or(Quat::IDENTITY, |fr| fr.rot.inverse());
    let mut sun = Vec3::Y;
    let mut best = 0.0;
    for (_, l, tf) in &suns {
        if l.illuminance > best {
            best = l.illuminance;
            sun = to_local * (-*tf.forward());
        }
    }
    let day = (surface.daylight() * weather.light()).clamp(0.0, 1.0);

    // ── Surface sous le vaisseau : le sol, ou le dessus des nuages (mer de nuages, P4) ──
    let mut surf_r = f.ground_r;
    state.above_clouds = None;
    if let (Some(w), TargetKind::Planet(id)) = (&weather.params, body.unwrap_or(TargetKind::GalacticCore)) {
        if let Some(p) = settings.systems.get(id / 1000).and_then(|s| s.planets().get(id % 1000)) {
            let cr = crate::weather::cloud_radius(p);
            let (c, thick) = crate::weather::cloud_field(w, up, clock.secs);
            if c > 0.35 {
                let cell = std::f32::consts::FRAC_PI_2 * cr / 48.0;
                let top = cr + cell * (0.25 + 0.75 * thick) * (1.0 + (c > 0.8) as u8 as f32);
                if f.pos.length() > top {
                    surf_r = top;
                    state.above_clouds = Some(top);
                }
            }
        }
    }
    let alt_s = (f.pos.length() - surf_r).max(0.0);

    // ── Ombre du vaisseau (P5) : projetée selon le soleil, plus nette et sombre près de la surface ──
    let near = (1.0 - alt_s / (60.0 * len)).clamp(0.0, 1.0);
    let sun_up = sun.dot(up).max(0.12);
    if near > 0.0 && day > 0.05 && best > 0.0 {
        let shadow = *state.shadow.get_or_insert_with(|| {
            let e = commands
                .spawn((
                    Mesh3d(m.disc.clone()),
                    MeshMaterial3d(materials.add(StandardMaterial { base_color: Color::srgba(0.0, 0.0, 0.0, 0.0), unlit: true, alpha_mode: AlphaMode::Blend, cull_mode: None, depth_bias: 3.0, ..default() })),
                    Transform::default(),
                    Visibility::Inherited,
                    NotShadowCaster,
                    NotShadowReceiver,
                    BodyFx,
                ))
                .id();
            commands.entity(root).add_child(e);
            e
        });
        // Point où le rayon du soleil passant par le vaisseau touche la surface
        let along = (alt_s / sun_up).min(alt_s * 6.0);
        let hit = f.pos - sun * along;
        let dir = hit.normalize_or(up);
        let g = terrain.ground(dir).top.max(surf_r.min(hit.length() + 1.0));
        let r = if state.above_clouds.is_some() { surf_r } else { g };
        if let Ok(mut tf) = tfs.get_mut(shadow) {
            tf.translation = dir * (r + 0.12 * f.voxel);
            tf.rotation = Quat::from_rotation_arc(Vec3::Z, dir);
            let spread = 1.0 + alt_s / len * 0.25;
            tf.scale = Vec3::new(len * 0.55 * spread, len * 0.8 * spread, 1.0);
        }
        if let Ok(mat) = mats.get(shadow) {
            if let Some(mm) = materials.get_mut(&mat.0) {
                mm.base_color = Color::srgba(0.0, 0.0, 0.0, (0.55 * near * day / (1.0 + alt_s / (8.0 * len))).clamp(0.0, 0.6));
            }
        }
        if let Ok(mut v) = vis.get_mut(shadow) {
            *v = Visibility::Inherited;
        }
    } else if let Some(shadow) = state.shadow {
        if let Ok(mut v) = vis.get_mut(shadow) {
            *v = Visibility::Hidden;
        }
    }

    // ── Poussière, neige, sable, eau soulevés par la poussée près du sol (P5) ──
    let ground_alt = (f.pos.length() - f.ground_r).max(0.0);
    let reach = 6.0 * real_len.max(len.min(real_len * 3.0));
    let thrust = surface.pilot.length().clamp(0.0, 1.0).max((f.vert_vox.abs() / (f.alt / f.voxel).max(20.0)).min(1.0) * 0.5);
    let lift = (1.0 - ground_alt / reach).clamp(0.0, 1.0) * thrust;
    let ground_kind = terrain.ground(up).kind;
    let col = dust_color(ground_kind);
    state.dust_acc += lift * dt * 55.0;
    while state.dust_acc >= 1.0 {
        state.dust_acc -= 1.0;
        let a = state.rand() * std::f32::consts::TAU;
        let east = Vec3::Y.cross(up).normalize_or(Vec3::X);
        let north = up.cross(east);
        let out = east * a.cos() + north * a.sin();
        let dir = (up + out * (real_len * (0.2 + 0.5 * state.rand()) / terrain.params.radius)).normalize();
        let g = terrain.ground(dir).top;
        let speed = real_len * (0.8 + 1.6 * state.rand());
        let p = Particle {
            entity: Entity::PLACEHOLDER,
            pos: dir * (g + 0.2 * f.voxel),
            vel: out * speed + up * speed * 0.35,
            age: 0.0,
            life: 1.2 + 1.2 * state.rand(),
            size0: real_len * 0.12,
            size1: real_len * (0.5 + 0.5 * state.rand()),
            color: col,
            alpha: 0.5 * lift.sqrt(),
            live: true,
        };
        if state.dust.len() < 90 {
            let e = commands
                .spawn((Mesh3d(m.low.clone()), MeshMaterial3d(materials.add(StandardMaterial { base_color: Color::srgba(col[0], col[1], col[2], p.alpha), unlit: false, alpha_mode: AlphaMode::Blend, perceptual_roughness: 1.0, ..default() })), Transform::from_translation(p.pos).with_scale(Vec3::splat(p.size0)), Visibility::Inherited, NotShadowCaster, NotShadowReceiver, BodyFx))
                .id();
            commands.entity(root).add_child(e);
            state.dust.push(Particle { entity: e, ..p });
        } else if let Some(slot) = state.dust.iter_mut().find(|q| !q.live) {
            *slot = Particle { entity: slot.entity, ..p };
            slot_visible(&mut vis, slot.entity, true);
        }
    }
    update_pool(&mut state.dust, dt, &mut tfs, &mut vis, &mats, &mut materials, false);

    // ── Étincelles de la rentrée : des points brillants arrachés au nez, qui restent sur place ──
    let spark_rate = f.heat * 150.0;
    state.spark_acc += dt * spark_rate;
    let up_s = f.up;
    let east_s = Vec3::Y.cross(up_s).normalize_or(Vec3::X);
    let north_s = up_s.cross(east_s);
    let white = [params.plasma[0] * 0.4 + 0.6, params.plasma[1] * 0.4 + 0.6, params.plasma[2] * 0.4 + 0.6];
    let speed_h = (f.speed_vox * f.speed_vox - f.vert_vox * f.vert_vox).max(0.0).sqrt() * f.voxel;
    while state.spark_acc >= 1.0 {
        state.spark_acc -= 1.0;
        let (a, b, c) = (state.signed(), state.signed(), state.signed());
        let side = (east_s * a + north_s * b) * len * 0.3;
        let p = Particle {
            entity: Entity::PLACEHOLDER,
            pos: f.pos + f.heading * (0.4 * len) + up_s * (c * len * 0.12) + side,
            vel: (side + up_s * c * len * 0.4) * 3.0 - f.heading * speed_h * 0.1,
            age: 0.0,
            life: 0.45 + 0.6 * state.rand(),
            size0: len * 0.035,
            size1: len * 0.012,
            color: if state.rand() < 0.5 { params.plasma } else { white },
            alpha: 0.95,
            live: true,
        };
        if state.sparks.len() < 130 {
            let e = commands
                .spawn((Mesh3d(m.low.clone()), MeshMaterial3d(materials.add(glow(p.color, p.alpha))), Transform::from_translation(p.pos).with_scale(Vec3::splat(p.size0)), Visibility::Inherited, NotShadowCaster, NotShadowReceiver, BodyFx))
                .id();
            commands.entity(root).add_child(e);
            state.sparks.push(Particle { entity: e, ..p });
        } else if let Some(slot) = state.sparks.iter_mut().find(|q| !q.live) {
            *slot = Particle { entity: slot.entity, ..p };
            slot_visible(&mut vis, slot.entity, true);
        } else {
            state.spark_acc = 0.0;
            break;
        }
    }
    update_pool(&mut state.sparks, dt, &mut tfs, &mut vis, &mats, &mut materials, true);

    // ── Traînées de condensation en haute altitude (P6) ──
    let high = f.density > 0.004 && f.density < 0.22 && humid(&params);
    state.contrail_acc += dt * f.speed_vox.min(2000.0) * if high { 1.0 } else { 0.0 };
    let step = (f.sound * 0.35).max(60.0);
    while state.contrail_acc >= step && f.mach > 0.35 {
        state.contrail_acc -= step;
        let p = Particle { entity: Entity::PLACEHOLDER, pos: f.pos, vel: Vec3::ZERO, age: 0.0, life: 24.0, size0: len * 0.3, size1: len * 1.8, color: [1.0, 1.0, 1.0], alpha: 0.5, live: true };
        if state.trail.len() < 140 {
            let e = commands
                .spawn((Mesh3d(m.low.clone()), MeshMaterial3d(materials.add(StandardMaterial { base_color: Color::srgba(1.0, 1.0, 1.0, 0.5), unlit: true, alpha_mode: AlphaMode::Blend, ..default() })), Transform::from_translation(p.pos).with_scale(Vec3::splat(p.size0)), Visibility::Inherited, NotShadowCaster, NotShadowReceiver, BodyFx))
                .id();
            commands.entity(root).add_child(e);
            state.trail.push(Particle { entity: e, ..p });
        } else if let Some(slot) = state.trail.iter_mut().filter(|q| !q.live).next().or(None) {
            *slot = Particle { entity: slot.entity, ..p };
            slot_visible(&mut vis, slot.entity, true);
        } else {
            state.contrail_acc = 0.0;
            break;
        }
    }
    if !high {
        state.contrail_acc = 0.0;
    }
    update_pool(&mut state.trail, dt, &mut tfs, &mut vis, &mats, &mut materials, true);

    // ── Onde de choc du bang : un anneau qui grandit et s'efface ──
    if let Some(mut ring) = state.ring {
        if ring.entity == Entity::PLACEHOLDER {
            ring.entity = commands
                .spawn((Mesh3d(m.ring.clone()), MeshMaterial3d(materials.add(glow([1.0, 1.0, 1.0], 0.0))), Transform::default(), Visibility::Inherited, NotShadowCaster, NotShadowReceiver, BodyFx))
                .id();
            commands.entity(root).add_child(ring.entity);
        }
        ring.age += dt;
        let u = ring.age / 1.6;
        if u >= 1.0 {
            commands.entity(ring.entity).despawn_recursive();
            state.ring = None;
        } else {
            if let Ok(mut tf) = tfs.get_mut(ring.entity) {
                tf.translation = ring.pos;
                tf.rotation = Quat::from_rotation_arc(Vec3::Y, ring.axis);
                tf.scale = Vec3::splat(ring.len * (1.0 + 70.0 * u.sqrt()) * (0.5 + 0.5 * ring.strength));
            }
            if let Ok(mat) = mats.get(ring.entity) {
                if let Some(mm) = materials.get_mut(&mat.0) {
                    mm.base_color = Color::srgba(1.0, 1.0, 1.0, (1.0 - u) * 0.7 * ring.strength);
                }
            }
            state.ring = Some(ring);
        }
    }

    // ── Sillage dans les nuages : des trous qui se referment (P4) ──
    state.wake_acc += dt;
    let moved = state.wake.last().map_or(f32::INFINITY, |w| w.pos.distance(f.pos));
    if moved > len * 2.5 || state.wake_acc > 0.5 {
        state.wake_acc = 0.0;
        state.wake.push(Wake { pos: f.pos, age: 0.0, radius: len.max(60.0 * f.voxel) * 3.0 });
        if state.wake.len() > 7 {
            state.wake.remove(0);
        }
    }
    for w in &mut state.wake {
        w.age += dt;
    }
    state.wake.retain(|w| w.age < 6.0);
}

fn slot_visible(vis: &mut Query<&mut Visibility, With<BodyFx>>, e: Entity, on: bool) {
    if let Ok(mut v) = vis.get_mut(e) {
        *v = if on { Visibility::Inherited } else { Visibility::Hidden };
    }
}

fn update_pool(
    pool: &mut [Particle],
    dt: f32,
    tfs: &mut Query<&mut Transform, With<BodyFx>>,
    vis: &mut Query<&mut Visibility, With<BodyFx>>,
    mats: &Query<&MeshMaterial3d<StandardMaterial>, With<BodyFx>>,
    materials: &mut Assets<StandardMaterial>,
    unlit: bool,
) {
    for p in pool.iter_mut().filter(|p| p.live) {
        p.age += dt;
        let u = p.age / p.life;
        if u >= 1.0 {
            p.live = false;
            slot_visible(vis, p.entity, false);
            continue;
        }
        p.pos += p.vel * dt;
        p.vel *= (-1.2 * dt).exp();
        if let Ok(mut tf) = tfs.get_mut(p.entity) {
            tf.translation = p.pos;
            tf.scale = Vec3::splat(p.size0 + (p.size1 - p.size0) * u.sqrt());
        }
        if let Ok(m) = mats.get(p.entity) {
            if let Some(mm) = materials.get_mut(&m.0) {
                let a = p.alpha * (1.0 - u) * (if u < 0.1 { u / 0.1 } else { 1.0 });
                mm.base_color = Color::srgba(p.color[0], p.color[1], p.color[2], a);
                mm.unlit = unlit;
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  Nuages
// ─────────────────────────────────────────────────────────────────────────

/// Les dalles de nuages deviennent des `CloudMaterial` (elles sont créées en standard par `planet.rs`).
fn convert_clouds(
    mut commands: Commands,
    mut standard: ResMut<Assets<StandardMaterial>>,
    mut clouds: ResMut<Assets<CloudMaterial>>,
    q: Query<(Entity, &MeshMaterial3d<StandardMaterial>), With<CloudVoxel>>,
) {
    for (e, m) in &q {
        let Some(base) = standard.get(&m.0).cloned() else { continue };
        let handle = clouds.add(CloudMaterial { base, extension: CloudClear::default() });
        commands.entity(e).remove::<MeshMaterial3d<StandardMaterial>>().insert(MeshMaterial3d(handle));
    }
}

/// Trous dans les nuages : la bulle autour du vaisseau et son sillage (positions du monde).
fn cloud_clear(
    time: Res<Time>,
    clock: Res<crate::world_clock::WorldClock>,
    surface: Res<Surface>,
    state: Res<FxState>,
    mut clouds: ResMut<Assets<CloudMaterial>>,
) {
    let _ = &clock;
    let f = surface.flight();
    let mut params = ClearParams::default();
    if f.active {
        if let Some(frame) = surface.frame() {
            let to_world = |p: Vec3| frame.center + frame.rot * p;
            let len = f.length.max(60.0 * f.voxel);
            params.slots[0] = to_world(f.pos).extend(len * 4.0);
            for (i, w) in state.wake.iter().rev().take(7).enumerate() {
                let k = 1.0 - w.age / 6.0;
                params.slots[i + 1] = to_world(w.pos).extend(w.radius * k.max(0.0));
            }
        }
    }
    let _ = time;
    for (_, m) in clouds.iter_mut() {
        if m.extension.params.slots != params.slots {
            m.extension.params = params;
        }
    }
}

/// Dans un nuage ? (0..1) : épaisseur des dalles de `weather::cloud_mesh` ; givre, turbulences,
/// éclairs dans un orage (P4, P8).
#[allow(clippy::too_many_arguments)]
fn cloud_state(
    time: Res<Time>,
    clock: Res<crate::world_clock::WorldClock>,
    settings: Res<GameSettings>,
    weather: Res<crate::weather::WeatherNow>,
    local: Res<crate::world_clock::LocalWeather>,
    mut surface: ResMut<Surface>,
    mut state: ResMut<FxState>,
    mut net: ResMut<crate::net::Net>,
) {
    let dt = time.delta_secs().min(0.1);
    let now = time.elapsed_secs_f64();
    let f = *surface.flight();
    let mut inside = 0.0;
    if f.active {
        if let (Some(w), Some(TargetKind::Planet(id))) = (&weather.params, surface.body()) {
            if let Some(p) = settings.systems.get(id / 1000).and_then(|s| s.planets().get(id % 1000)) {
                let cr = crate::weather::cloud_radius(p);
                let (c, thick) = crate::weather::cloud_field(w, f.up, clock.secs);
                if std::env::var_os("SPACESPORE_TEST_FLY").is_some() && (now * 2.0).fract() < dt as f64 * 2.0 {
                    info!("CLOUD c={c:.2} thick={thick:.2} r-cr={:.0} cell={:.0} voxel={:.2}", f.pos.length() - cr, std::f32::consts::FRAC_PI_2 * cr / 48.0, f.voxel);
                }
                if c > 0.35 {
                    let cell = std::f32::consts::FRAC_PI_2 * cr / 48.0;
                    let top = cr + cell * (0.25 + 0.75 * thick) * (1.0 + (c > 0.8) as u8 as f32);
                    let r = f.pos.length();
                    if r > cr - cell * 0.1 && r < top {
                        inside = 1.0;
                    }
                }
            }
        }
    }
    state.inside += (inside - state.inside) * (1.0 - (-3.0 * dt).exp());
    surface.set_cloud_dark(state.inside);
    // Turbulences dans un nuage (plus fortes dans un orage)
    if state.inside > 0.3 {
        let storm = weather.sample.storm;
        let k = (0.4 + 1.6 * storm) * state.inside * dt;
        let (a, b) = (state.signed(), state.signed());
        surface.jolt(Vec2::new(a, b) * 0.35 * k * 4.0);
        state.shake = state.shake.max(0.05 + 0.2 * storm);
        // Foudre sur le vaisseau, dans un orage (un éclair et une secousse ; pas de dégâts, Q8)
        if storm > 0.5 && now - state.last_flash > 8.0 && state.rand() < dt * 0.15 * storm {
            state.last_flash = now;
            state.boom = state.boom.max(0.7);
            net.notify("Foudre ! L'eclair frappe le vaisseau.", now);
        }
    }
    // Givre : se forme dans un nuage ou sous la pluie par grand froid, fond au chaud
    let cold = local.temp < -2.0;
    let wet = state.inside > 0.4 || weather.sample.precip > 0.2;
    let rate = if f.active && cold && wet && f.speed_vox > 20.0 { 0.12 * (1.0 + (-local.temp / 30.0).clamp(0.0, 1.0)) } else { -0.07 };
    state.frost = (state.frost + rate * dt).clamp(0.0, 1.0);
}

/// Brouillard blanc ou gris dans un nuage.
fn cloud_fog(mut commands: Commands, state: Res<FxState>, surface: Res<Surface>, weather: Res<crate::weather::WeatherNow>, under: Res<crate::water::Underwater>, cam_q: Query<Entity, With<Camera3d>>, mut clear: ResMut<ClearColor>) {
    if state.inside < 0.05 || under.active() {
        return;
    }
    let Ok(cam) = cam_q.get_single() else { return };
    let day = (surface.daylight() * weather.light()).clamp(0.05, 1.0);
    let grey = (0.55 + 0.4 * (1.0 - weather.sample.storm)) * (0.25 + 0.75 * day);
    let color = Color::srgb(grey, grey, grey * 1.03);
    let f = surface.flight();
    let vis = (f.length * 9.0).max(60.0 * f.voxel) / state.inside.max(0.05);
    clear.0 = color;
    commands.entity(cam).insert(DistanceFog { color, falloff: FogFalloff::from_visibility(vis), ..default() });
}

// ─────────────────────────────────────────────────────────────────────────
//  Secousses
// ─────────────────────────────────────────────────────────────────────────

fn shake_camera(time: Res<Time>, surface: Res<Surface>, mut state: ResMut<FxState>, ship_q: Query<&Transform, (With<Ship>, Without<Camera3d>)>, mut cam_q: Query<&mut Transform, With<Camera3d>>) {
    let dt = time.delta_secs().min(0.1);
    let f = *surface.flight();
    state.boom *= (-3.5 * dt).exp();
    state.shake *= (-4.0 * dt).exp();
    if !f.active {
        return;
    }
    // La rentrée secoue ; le mur du son aussi (buffet transsonique), le bang d'un coup
    let heat = f.heat;
    let buffet = smooth(0.35, 0.0, (f.mach - 1.0).abs()) * smooth(0.02, 0.2, f.density) * 0.35;
    let amp = (heat * 0.8 + buffet + state.boom * 1.2 + state.shake).clamp(0.0, 1.5);
    if amp < 0.01 {
        return;
    }
    let (Ok(mut cam), Ok(ship)) = (cam_q.get_single_mut(), ship_q.get_single()) else { return };
    let d = cam.translation.distance(ship.translation).max(1e-3);
    let k = amp * 0.005 * d * if surface.cockpit() { 2.5 } else { 1.0 };
    let (a, b, c) = (state.signed(), state.signed(), state.signed());
    let off = cam.rotation * Vec3::new(a, b, c * 0.3) * k;
    cam.translation += off;
}

// ─────────────────────────────────────────────────────────────────────────
//  Liseré de l'atmosphère (P8)
// ─────────────────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn rim_fx(
    mut commands: Commands,
    surface: Res<Surface>,
    target: Res<crate::ui::CameraTarget>,
    settings: Res<GameSettings>,
    dim: Res<crate::sky::SunDim>,
    mut state: ResMut<FxState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut rims: ResMut<Assets<RimMaterial>>,
    planets: Query<(Entity, &PlanetId, &Transform), With<PlanetRoot>>,
    stars: Query<(&Transform, &StarId), (With<StarRoot>, Without<PlanetRoot>)>,
    cam_q: Query<&Transform, (With<Camera3d>, Without<PlanetRoot>, Without<StarRoot>)>,
    mut ents: Query<(&MeshMaterial3d<RimMaterial>, &mut Transform, &mut Visibility), (Without<PlanetRoot>, Without<StarRoot>, Without<Camera3d>)>,
) {
    // L'astre visité, sinon l'astre ciblé
    let kind = surface.body().unwrap_or(target.0);
    let wanted = match kind {
        TargetKind::Planet(id) => settings.systems.get(id / 1000).and_then(|s| s.planets().get(id % 1000)).filter(|p| p.atmosphere && p.air.present() && !p.gaseous()).map(|_| kind),
        _ => None,
    };
    if state.rim.map(|(_, k)| k) != wanted {
        if let Some((e, _)) = state.rim.take() {
            commands.entity(e).despawn_recursive();
        }
        if let (Some(k @ TargetKind::Planet(id)), Some(p)) = (wanted, wanted.and_then(|k| if let TargetKind::Planet(id) = k { settings.systems.get(id / 1000).and_then(|s| s.planets().get(id % 1000)) } else { None })) {
            if let Some((root, _, _)) = planets.iter().find(|(_, pid, _)| pid.0 == id) {
                let bp = crate::terrain::BodyParams::planet(p);
                let shell = p.radius + p.terrain_height + 2.0 * crate::terrain::landforms_of(&bp).max_height() + 0.17 * crate::surface::atmosphere_depth(&bp);
                let mesh = meshes.add(Sphere::new(1.0).mesh().ico(5).unwrap());
                let col = [p.air.sky[0], p.air.sky[1], p.air.sky[2]];
                let mat = rims.add(RimMaterial { params: RimParams { color: Vec4::new(col[0], col[1], col[2], 0.0), sun: Vec4::new(0.0, 1.0, 0.0, 0.0), center: Vec4::new(0.0, 0.0, 0.0, shell) } });
                let e = commands
                    .spawn((Mesh3d(mesh), MeshMaterial3d(mat), Transform::from_scale(Vec3::splat(shell)), Visibility::Inherited, NotShadowCaster, NotShadowReceiver))
                    .id();
                commands.entity(root).add_child(e);
                state.rim = Some((e, k));
            }
        }
    }
    let Some((e, TargetKind::Planet(id))) = state.rim else { return };
    let Ok((mat, mut tf, mut vis)) = ents.get_mut(e) else { return };
    let (Some((_, _, root_tf)), Ok(cam)) = (planets.iter().find(|(_, p, _)| p.0 == id), cam_q.get_single()) else { return };
    let shell = tf.scale.x;
    let dist = cam.translation.distance(root_tf.translation);
    // À l'intérieur de la coque (dans l'air) : plus de liseré
    let strength = smooth(shell * 0.995, shell * 1.15, dist);
    if std::env::var_os("SPACESPORE_TEST_RIM").is_some() {
        info!("RIM shell={shell:.0} dist={dist:.0} strength={strength:.2}");
    }
    let v = if strength > 0.01 { Visibility::Inherited } else { Visibility::Hidden };
    if *vis != v {
        *vis = v;
    }
    tf.scale = Vec3::splat(shell);
    let suns = crate::surface::sun_list(&settings, stars.iter().map(|(t, id)| (t.translation, id.0)), root_tf.translation, &dim);
    let to_star = suns.first().map_or(Vec3::Y, |s| s.0);
    if let Some(m) = rims.get_mut(&mat.0) {
        m.params.color.w = strength * 0.9;
        m.params.sun = to_star.extend(0.0);
        m.params.center = root_tf.translation.extend(shell);
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  Autres joueurs (P9) : leur rentrée vue de loin comme une étoile filante, leur bang entendu
//  avec le retard de la distance
// ─────────────────────────────────────────────────────────────────────────

#[derive(Component)]
struct RemoteFx(FxKind);

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn remote_fx(
    mut commands: Commands,
    time: Res<Time>,
    net: Res<crate::net::Net>,
    surface: Res<Surface>,
    mut state: ResMut<FxState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut booms: EventWriter<crate::sound::RemoteBoom>,
    ships: Query<(Entity, &crate::net::RemoteShip, &Transform), Without<RemoteFx>>,
    mut fx: Query<(&Parent, &RemoteFx, &MeshMaterial3d<StandardMaterial>, &mut Transform, &mut Visibility), Without<crate::net::RemoteShip>>,
    cam_q: Query<&Transform, (With<Camera3d>, Without<crate::net::RemoteShip>, Without<RemoteFx>)>,
    mut spawned: Local<std::collections::HashSet<Entity>>,
    mut prev_mach: Local<std::collections::HashMap<u32, f32>>,
) {
    let t = time.elapsed_secs();
    let cam = cam_q.get_single().map_or(Vec3::ZERO, |c| c.translation);
    let m = fx_meshes(&mut state, &mut meshes);
    let live: std::collections::HashSet<Entity> = ships.iter().map(|(e, _, _)| e).collect();
    spawned.retain(|e| live.contains(e));
    for (e, rs, tf) in &ships {
        let Some(peer) = net.peers.get(&rs.id) else { continue };
        let heat = peer.look.heat as f32 / 255.0;
        let mach = peer.look.mach as f32 / 20.0;
        // Son bang : il franchit le mur du son (avec de l'air chez nous), entendu après le retard
        let before = prev_mach.insert(rs.id, mach).unwrap_or(0.0);
        if before > 0.0 && ((before < 1.0) != (mach < 1.0)) && mach > 0.0 {
            let dist = cam.distance(tf.translation) / surface.voxel().unwrap_or(1.0).max(1e-3);
            booms.send(crate::sound::RemoteBoom { distance: dist, strength: 0.7 });
        }
        if !spawned.contains(&e) {
            spawned.insert(e);
            for (kind, color) in [(FxKind::Sheath, [1.0, 0.55, 0.2]), (FxKind::Core, [1.0, 0.9, 0.7]), (FxKind::Trail, [1.0, 0.5, 0.2])] {
                let c = commands
                    .spawn((Mesh3d(m.cone.clone()), MeshMaterial3d(materials.add(glow(color, 0.0))), Transform::default(), Visibility::Hidden, NotShadowCaster, NotShadowReceiver, RemoteFx(kind)))
                    .id();
                commands.entity(e).add_child(c);
            }
        }
    }
    for (parent, kind, mat, mut tf, mut vis) in &mut fx {
        let Ok((_, rs, ship_tf)) = ships.get(parent.get()) else { continue };
        let Some(peer) = net.peers.get(&rs.id) else { continue };
        let heat = peer.look.heat as f32 / 255.0;
        let l = 5.0;
        // Vu de loin, la traînée garde la taille apparente de l'icône : un trait lumineux dans le ciel
        let far = (cam.distance(ship_tf.translation) / ship_tf.scale.x.max(1e-6) / 125.0).clamp(1.0, 3.0);
        let flick = 1.0 + 0.1 * (t * 37.0).sin();
        let (alpha, color, center, scale) = match kind.0 {
            FxKind::Sheath => {
                let (c, s) = cone(0.5 * l, -0.55 * l, (1.4 + 1.8 * heat) * l * flick);
                (0.35 * heat, [1.0, 0.55, 0.2], c, s)
            }
            FxKind::Core => {
                let (c, s) = cone(0.26 * l, -0.5 * l, (0.9 + 1.2 * heat) * l * flick);
                (0.6 * heat, [1.0, 0.92, 0.75], c, s)
            }
            _ => {
                let (c, s) = cone(0.34 * l, 0.1 * l, (2.5 + 7.0 * heat) * l * far);
                (0.4 * heat, [1.0, 0.55, 0.25], c, s)
            }
        };
        let on = alpha > 0.01;
        let v = if on { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != v {
            *vis = v;
        }
        if on {
            tf.translation = center;
            tf.scale = scale;
            tf.rotation = Quat::from_rotation_arc(Vec3::Y, Vec3::Z);
            if let Some(mm) = materials.get_mut(&mat.0) {
                mm.base_color = Color::srgba(color[0], color[1], color[2], alpha);
            }
        }
    }
}
