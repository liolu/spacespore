mod astre;
mod lod;
mod mesher;
mod planet;
mod settings;
mod ship;
mod ui;

use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowCloseRequested};

use planet::{MoonId, MoonRoot, PlanetId, PlanetPlugin, PlanetRoot, StarId, StarRoot};
use settings::GameSettings;
use ship::{Ship, ShipMode, ShipPlugin};
use ui::{CameraTarget, MenuState, TargetKind, UiPlugin};

// ── Planètes ──────────────────────────────────────────────────────────────
use astre::planete::gas_planet::{GasPlanetPlugin, GasPlanetRoot};
use astre::planete::comet::{CometPlugin, CometRoot};
use astre::planete::meteoroid::{MeteoroidPlugin, MeteoroidRoot};

// ── Étoiles ───────────────────────────────────────────────────────────────
use astre::etoile::star::{
    StarPlugin as VoxelStarPlugin,
    StarRoot as VoxelStarRoot,
};
use astre::etoile::protostar::{ProtostarPlugin, ProtostarRoot};
use astre::etoile::dwarf_star::{DwarfStarPlugin, DwarfRoot};
use astre::etoile::main_sequence_star::{MainSequencePlugin, MsRoot};
use astre::etoile::giant_star::{GiantStarPlugin, GiantRoot};
use astre::etoile::supergiant_star::{SupergiantPlugin, SgRoot};
use astre::etoile::hypergiant_star::{HypergiantPlugin, HgRoot};

// ── Rémanents stellaires ──────────────────────────────────────────────────
#[allow(non_snake_case)]
use astre::Remnant_stellaire::black_hole::{
    BlackHolePlugin,
    BlackHoleRoot,
};

#[allow(non_snake_case)]
use astre::Remnant_stellaire::nebula::{
    NebulaPlugin,
    NebulaRoot,
};

#[allow(non_snake_case)]
use astre::Remnant_stellaire::pulsar::{
    PulsarPlugin,
    PulsarRoot,
};

#[allow(non_snake_case)]
use astre::Remnant_stellaire::magnetar::{
    MagnetarPlugin,
    MagnetarRoot,
};

#[allow(non_snake_case)]
use astre::Remnant_stellaire::neutron_star::{
    NeutronStarPlugin,
    NeutronStarRoot,
};

#[allow(non_snake_case)]
use astre::Remnant_stellaire::supernova::{
    SupernovaPlugin,
    SupernovaRoot,
};


// ─────────────────────────────────────────────────────────────────────────
//  TargetQueries
//
//  Regroupe toutes les queries nécessaires au contrôleur de caméra.
//  IMPORTANT : ne pas mettre &'w devant les composants dans Query.
// ─────────────────────────────────────────────────────────────────────────

#[derive(SystemParam)]
pub struct TargetQueries<'w, 's> {
    pub planet_q:
        Query<'w, 's, (&'static GlobalTransform, &'static PlanetId), With<PlanetRoot>>,

    pub moon_q:
        Query<'w, 's, (&'static GlobalTransform, &'static MoonId), With<MoonRoot>>,

    pub star_q:
        Query<'w, 's, (&'static GlobalTransform, &'static StarId), With<StarRoot>>,

    pub gas_q:
        Query<'w, 's, (&'static GlobalTransform, &'static GasPlanetRoot)>,

    pub comet_q:
        Query<'w, 's, (&'static GlobalTransform, &'static CometRoot)>,

    pub meteoroid_q:
        Query<'w, 's, (&'static GlobalTransform, &'static MeteoroidRoot)>,

    pub vstar_q:
        Query<'w, 's, (&'static GlobalTransform, &'static VoxelStarRoot)>,

    pub proto_q:
        Query<'w, 's, (&'static GlobalTransform, &'static ProtostarRoot)>,

    pub dwarf_q:
        Query<'w, 's, (&'static GlobalTransform, &'static DwarfRoot)>,

    pub ms_q:
        Query<'w, 's, (&'static GlobalTransform, &'static MsRoot)>,

    pub giant_q:
        Query<'w, 's, (&'static GlobalTransform, &'static GiantRoot)>,

    pub sg_q:
        Query<'w, 's, (&'static GlobalTransform, &'static SgRoot)>,

    pub hg_q:
        Query<'w, 's, (&'static GlobalTransform, &'static HgRoot)>,

    pub nebula_q:
        Query<'w, 's, (&'static GlobalTransform, &'static NebulaRoot)>,

    pub black_q:
        Query<'w, 's, (&'static GlobalTransform, &'static BlackHoleRoot)>,

    pub pulsar_q:
        Query<'w, 's, (&'static GlobalTransform, &'static PulsarRoot)>,

    pub magnetar_q:
        Query<'w, 's, (&'static GlobalTransform, &'static MagnetarRoot)>,

    pub neutron_q:
        Query<'w, 's, (&'static GlobalTransform, &'static NeutronStarRoot)>,

    pub supernova_q:
        Query<'w, 's, (&'static GlobalTransform, &'static SupernovaRoot)>,
}


// ─────────────────────────────────────────────────────────────────────────
//  Main
// ─────────────────────────────────────────────────────────────────────────

fn main() {
    let settings = GameSettings::load();

    App::new()
        .add_plugins(
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "SpaceSpore - Voxel Universe".into(),
                    resolution: (1280.0_f32, 720.0_f32).into(),
                    ..default()
                }),
                ..default()
            }),
        )

        .add_plugins(FrameTimeDiagnosticsPlugin)

        .insert_resource(settings)

        // ── Planètes & corps ────────────────────────────────────────────
        .add_plugins(PlanetPlugin)

        .add_plugins((
            GasPlanetPlugin,
            CometPlugin,
            MeteoroidPlugin,
        ))

        // ── Étoiles ─────────────────────────────────────────────────────
        .add_plugins((
            VoxelStarPlugin,
            ProtostarPlugin,
            DwarfStarPlugin,
            MainSequencePlugin,
            GiantStarPlugin,
            SupergiantPlugin,
            HypergiantPlugin,
        ))

        // ── Rémanents stellaires ────────────────────────────────────────
        .add_plugins((
            NebulaPlugin,
            BlackHolePlugin,
            PulsarPlugin,
            MagnetarPlugin,
            NeutronStarPlugin,
            SupernovaPlugin,
        ))

        // ── Vaisseau ────────────────────────────────────────────────────
        .add_plugins(ShipPlugin)

        // ── UI ──────────────────────────────────────────────────────────
        .add_plugins(UiPlugin)

        // ── Startup ─────────────────────────────────────────────────────
        .add_systems(
            Startup,
            (
                setup_scene,
                setup_fps_display,
            ),
        )

        // ── Update ──────────────────────────────────────────────────────
        .add_systems(
            Update,
            (
                select_world_target,
                select_next_moon,
                camera_controller,
                update_sun_direction,
                update_fps_display,
                draw_light_indicator,
                draw_orbits,
                close_game_when_primary_window_closes,
            ),
        )

        .run();
}

fn close_game_when_primary_window_closes(
    mut close_events: EventReader<WindowCloseRequested>,
    primary_window: Query<Entity, With<PrimaryWindow>>,
    mut exit: EventWriter<AppExit>,
) {
    let Ok(primary_entity) = primary_window.get_single() else {
        return;
    };

    if close_events.read().any(|event| event.window == primary_entity) {
        exit.send(AppExit::Success);
    }
}


// ─────────────────────────────────────────────────────────────────────────
//  Scène initiale
// ─────────────────────────────────────────────────────────────────────────

fn setup_scene(
    mut commands: Commands,
    settings: Res<GameSettings>,
) {
    commands.insert_resource(
        ClearColor(Color::srgb(
            0.005,
            0.005,
            0.02,
        )),
    );

    // Lumière directionnelle représentant le soleil
    commands.spawn((
        DirectionalLight {
            illuminance: 0.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::default(),
        SunLight,
    ));

    // Lumière ambiante
    commands.insert_resource(
        AmbientLight {
            color: Color::srgb(
                0.25,
                0.25,
                0.35,
            ),
            brightness: 60.0,
        },
    );

    // Distance orbitale initiale
    let orbit_dist = settings
        .planets
        .first()
        .map(|p| p.orbit_distance)
        .unwrap_or(450.0);

    let planet_start = Vec3::new(
        orbit_dist,
        0.0,
        0.0,
    );

    // Caméra
    commands.spawn((
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            far: 100_000.0,
            ..default()
        }),

        Transform::from_translation(
            planet_start + Vec3::new(
                0.0,
                80.0,
                200.0,
            ),
        )
        .looking_at(
            planet_start,
            Vec3::Y,
        ),

        CameraController {
            yaw: 0.0,
            pitch: -0.3,
            distance: 200.0,
        },
    ));
}

fn select_world_target(
    buttons: Res<ButtonInput<MouseButton>>,
    primary_window: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<CameraController>>,
    queries: TargetQueries,
    mut target: ResMut<CameraTarget>,
) {
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    let Ok(window) = primary_window.get_single() else { return; };
    let Some(cursor) = window.cursor_position() else { return; };
    let Ok((camera, camera_transform)) = camera_q.get_single() else { return; };
    let Ok(ray) = camera.viewport_to_world(camera_transform, cursor) else { return; };
    let mut best: Option<(f32, TargetKind)> = None;

    for (transform, id) in &queries.moon_q {
        let position = transform.translation();
        let along = (position - ray.origin).dot(*ray.direction);
        if along > 0.0 {
            let closest = ray.origin + *ray.direction * along;
            let ray_distance = closest.distance(position);
            if ray_distance <= 300.0 && best.map_or(true, |(distance, _)| along < distance) {
                best = Some((along, TargetKind::Moon(id.planet_idx, id.moon_idx)));
            }
        }
    }

    let mut consider = |position: Vec3, tolerance: f32, candidate: TargetKind| {
        let Ok(screen_position) = camera.world_to_viewport(camera_transform, position) else {
            return;
        };
        let screen_distance = screen_position.distance(cursor);
        if screen_distance <= tolerance && best.map_or(true, |(distance, _)| screen_distance < distance) {
            best = Some((screen_distance, candidate));
        }
    };

    for (transform, id) in &queries.planet_q {
        consider(transform.translation(), 80.0, TargetKind::Planet(id.0));
    }
    for (transform, id) in &queries.star_q {
        consider(transform.translation(), 80.0, TargetKind::Star(id.0));
    }
    for (transform, root) in &queries.gas_q {
        consider(transform.translation(), 80.0, TargetKind::GasPlanet(root.idx));
    }
    for (transform, root) in &queries.comet_q {
        consider(transform.translation(), 80.0, TargetKind::Comet(root.idx));
    }
    for (transform, root) in &queries.meteoroid_q {
        consider(transform.translation(), 80.0, TargetKind::Meteoroid(root.idx));
    }
    for (transform, root) in &queries.vstar_q {
        consider(transform.translation(), 80.0, TargetKind::VoxelStar(root.idx));
    }
    for (transform, root) in &queries.proto_q {
        consider(transform.translation(), 80.0, TargetKind::Protostar(root.idx));
    }
    for (transform, root) in &queries.dwarf_q {
        consider(transform.translation(), 80.0, TargetKind::DwarfStar(root.idx));
    }
    for (transform, root) in &queries.ms_q {
        consider(transform.translation(), 80.0, TargetKind::MainSequence(root.idx));
    }
    for (transform, root) in &queries.giant_q {
        consider(transform.translation(), 80.0, TargetKind::GiantStar(root.idx));
    }
    for (transform, root) in &queries.sg_q {
        consider(transform.translation(), 80.0, TargetKind::Supergiant(root.idx));
    }
    for (transform, root) in &queries.hg_q {
        consider(transform.translation(), 80.0, TargetKind::Hypergiant(root.idx));
    }
    for (transform, _) in &queries.nebula_q {
        consider(transform.translation(), 80.0, TargetKind::Nebula);
    }
    for (transform, root) in &queries.black_q {
        consider(transform.translation(), 80.0, TargetKind::BlackHole(root.idx));
    }
    for (transform, root) in &queries.pulsar_q {
        consider(transform.translation(), 80.0, TargetKind::Pulsar(root.idx));
    }
    for (transform, root) in &queries.magnetar_q {
        consider(transform.translation(), 80.0, TargetKind::Magnetar(root.idx));
    }
    for (transform, root) in &queries.neutron_q {
        consider(transform.translation(), 80.0, TargetKind::NeutronStar(root.idx));
    }
    for (transform, root) in &queries.supernova_q {
        consider(transform.translation(), 80.0, TargetKind::Supernova(root.idx));
    }

    if let Some((_, selected)) = best {
        target.0 = selected;
    }
}

fn select_next_moon(
    keys: Res<ButtonInput<KeyCode>>,
    settings: Res<GameSettings>,
    mut target: ResMut<CameraTarget>,
) {
    if !keys.just_pressed(KeyCode::KeyM) {
        return;
    }

    let (planet_idx, current_moon) = match target.0 {
        TargetKind::Planet(index) => (index, None),
        TargetKind::Moon(planet_idx, moon_idx) => (planet_idx, Some(moon_idx)),
        _ => return,
    };
    let Some(moons) = settings.planets.get(planet_idx).map(|planet| &planet.moons) else {
        return;
    };
    if moons.is_empty() {
        return;
    }
    let next = current_moon.map_or(0, |index| (index + 1) % moons.len());
    target.0 = TargetKind::Moon(planet_idx, next);
}


// ─────────────────────────────────────────────────────────────────────────
//  Contrôleur de caméra
// ─────────────────────────────────────────────────────────────────────────

#[derive(Component)]
struct CameraController {
    yaw: f32,
    pitch: f32,
    distance: f32,
}

fn camera_controller(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    settings: Res<GameSettings>,
    menu_state: Res<MenuState>,
    camera_target: Res<CameraTarget>,
    ship_mode: Res<ShipMode>,

    mut mouse_wheel:
        EventReader<bevy::input::mouse::MouseWheel>,

    mut mouse_motion:
        EventReader<bevy::input::mouse::MouseMotion>,

    mouse_buttons:
        Res<ButtonInput<MouseButton>>,

    queries: TargetQueries,

    ship_q: Query<&Transform, With<Ship>>,

    mut cam_q:
        Query<(&mut Transform, &mut CameraController), Without<Ship>>,
) {
    let Ok((mut transform, mut ctrl)) =
        cam_q.get_single_mut()
    else {
        return;
    };

    // ── Mode libre : caméra derrière le vaisseau ──────────────────────
    if *ship_mode == ShipMode::Free {
        mouse_motion.clear();

        if let Ok(ship_tf) = ship_q.get_single() {
            for ev in mouse_wheel.read() {
                ctrl.distance -= ev.y * settings.scroll_speed * 0.5;
            }
            ctrl.distance = ctrl.distance.clamp(5.0, 200.0);

            let behind = ship_tf.rotation * Vec3::new(0.0, 2.5, ctrl.distance);
            transform.translation = ship_tf.translation + behind;
            transform.look_at(ship_tf.translation + ship_tf.rotation * Vec3::NEG_Z * 10.0, Vec3::Y);
        }
        return;
    }

    // ── Mode orbite : comportement normal ─────────────────────────────
    let target_pos =
        resolve_target(
            &camera_target,
            &queries,
        );

    // ── Menu ouvert ────────────────────────────────────────────────────
    if menu_state.open {
        mouse_motion.clear();
        mouse_wheel.clear();

        let rotation =
            Quat::from_euler(
                EulerRot::YXZ,
                ctrl.yaw,
                ctrl.pitch,
                0.0,
            );

        transform.translation =
            target_pos
            + rotation
                * Vec3::new(
                    0.0,
                    0.0,
                    ctrl.distance,
                );

        transform.look_at(
            target_pos,
            Vec3::Y,
        );

        return;
    }

    // ── Vitesse clavier ────────────────────────────────────────────────
    let rotate_speed =
        settings.keyboard_speed
        * time.delta_secs();

    let mouse_sens =
        settings.mouse_sensitivity
        * 0.01;

    let y_mult =
        if settings.invert_y {
            -1.0
        } else {
            1.0
        };

    // ── Rotation clavier ──────────────────────────────────────────────

    if keys.pressed(KeyCode::ArrowLeft)
        || keys.pressed(KeyCode::KeyA)
    {
        ctrl.yaw += rotate_speed;
    }

    if keys.pressed(KeyCode::ArrowRight)
        || keys.pressed(KeyCode::KeyD)
    {
        ctrl.yaw -= rotate_speed;
    }

    if keys.pressed(KeyCode::ArrowUp)
        || keys.pressed(KeyCode::KeyW)
    {
        ctrl.pitch +=
            rotate_speed * y_mult;
    }

    if keys.pressed(KeyCode::ArrowDown)
        || keys.pressed(KeyCode::KeyS)
    {
        ctrl.pitch -=
            rotate_speed * y_mult;
    }

    // ── Rotation souris ───────────────────────────────────────────────

    if mouse_buttons.pressed(MouseButton::Right) {
        for ev in mouse_motion.read() {
            ctrl.yaw -=
                ev.delta.x * mouse_sens;

            ctrl.pitch +=
                ev.delta.y
                * mouse_sens
                * y_mult;
        }
    } else {
        mouse_motion.clear();
    }

    // ── Zoom ──────────────────────────────────────────────────────────

    for ev in mouse_wheel.read() {
        let direction_factor = if ev.y < 0.0 { 2.5 } else { 1.0 };
        let zoom_factor = 1.0 + ctrl.distance.abs() * 0.004 * direction_factor;
        ctrl.distance -=
            ev.y * settings.scroll_speed * zoom_factor;
    }

    // ── Limites rotation ──────────────────────────────────────────────

    ctrl.pitch =
        ctrl.pitch.clamp(
            -1.5,
            1.5,
        );

    // ── Limites distance ──────────────────────────────────────────────

    let (min_dist, max_dist) =
        camera_distance_range(
            &camera_target,
            &settings,
        );

    ctrl.distance =
        ctrl.distance.clamp(
            min_dist,
            max_dist,
        );

    // ── Application caméra ────────────────────────────────────────────

    let rotation =
        Quat::from_euler(
            EulerRot::YXZ,
            ctrl.yaw,
            ctrl.pitch,
            0.0,
        );

    transform.translation =
        target_pos
        + rotation
            * Vec3::new(
                0.0,
                0.0,
                ctrl.distance,
            );

    transform.look_at(
        target_pos,
        Vec3::Y,
    );
}


// ─────────────────────────────────────────────────────────────────────────
//  Résolution de la cible caméra
// ─────────────────────────────────────────────────────────────────────────

fn resolve_target(
    target: &CameraTarget,
    q: &TargetQueries,
) -> Vec3 {
    match target.0 {
        TargetKind::Planet(i) =>
            q.planet_q
                .iter()
                .find(|(_, pid)| pid.0 == i)
                .map(|(gt, _)| gt.translation())
                .unwrap_or_default(),

        TargetKind::Moon(planet_idx, moon_idx) =>
            q.moon_q
                .iter()
                .find(|(_, mid)| mid.planet_idx == planet_idx && mid.moon_idx == moon_idx)
                .map(|(gt, _)| gt.translation())
                .unwrap_or_default(),

        TargetKind::Star(i) =>
            q.star_q
                .iter()
                .find(|(_, sid)| sid.0 == i)
                .map(|(gt, _)| gt.translation())
                .unwrap_or_default(),

        TargetKind::GasPlanet(i) =>
            q.gas_q
                .iter()
                .find(|(_, r)| r.idx == i)
                .map(|(gt, _)| gt.translation())
                .unwrap_or_default(),

        TargetKind::Comet(i) =>
            q.comet_q
                .iter()
                .find(|(_, r)| r.idx == i)
                .map(|(gt, _)| gt.translation())
                .unwrap_or_default(),

        TargetKind::Meteoroid(i) =>
            q.meteoroid_q
                .iter()
                .find(|(_, r)| r.idx == i)
                .map(|(gt, _)| gt.translation())
                .unwrap_or_default(),

        TargetKind::VoxelStar(i) =>
            q.vstar_q
                .iter()
                .find(|(_, r)| r.idx == i)
                .map(|(gt, _)| gt.translation())
                .unwrap_or_default(),

        TargetKind::Protostar(i) =>
            q.proto_q
                .iter()
                .find(|(_, r)| r.idx == i)
                .map(|(gt, _)| gt.translation())
                .unwrap_or_default(),

        TargetKind::DwarfStar(i) =>
            q.dwarf_q
                .iter()
                .find(|(_, r)| r.idx == i)
                .map(|(gt, _)| gt.translation())
                .unwrap_or_default(),

        TargetKind::MainSequence(i) =>
            q.ms_q
                .iter()
                .find(|(_, r)| r.idx == i)
                .map(|(gt, _)| gt.translation())
                .unwrap_or_default(),

        TargetKind::GiantStar(i) =>
            q.giant_q
                .iter()
                .find(|(_, r)| r.idx == i)
                .map(|(gt, _)| gt.translation())
                .unwrap_or_default(),

        TargetKind::Supergiant(i) =>
            q.sg_q
                .iter()
                .find(|(_, r)| r.idx == i)
                .map(|(gt, _)| gt.translation())
                .unwrap_or_default(),

        TargetKind::Hypergiant(i) =>
            q.hg_q
                .iter()
                .find(|(_, r)| r.idx == i)
                .map(|(gt, _)| gt.translation())
                .unwrap_or_default(),

        TargetKind::Nebula =>
            q.nebula_q
                .iter()
                .next()
                .map(|(gt, _)| gt.translation())
                .unwrap_or_default(),

        TargetKind::BlackHole(i) =>
            q.black_q
                .iter()
                .find(|(_, r)| r.idx == i)
                .map(|(gt, _)| gt.translation())
                .unwrap_or_default(),

        TargetKind::Pulsar(i) =>
            q.pulsar_q
                .iter()
                .find(|(_, r)| r.idx == i)
                .map(|(gt, _)| gt.translation())
                .unwrap_or_default(),

        TargetKind::Magnetar(i) =>
            q.magnetar_q
                .iter()
                .find(|(_, r)| r.idx == i)
                .map(|(gt, _)| gt.translation())
                .unwrap_or_default(),

        TargetKind::NeutronStar(i) =>
            q.neutron_q
                .iter()
                .find(|(_, r)| r.idx == i)
                .map(|(gt, _)| gt.translation())
                .unwrap_or_default(),

        TargetKind::Supernova(i) =>
            q.supernova_q
                .iter()
                .find(|(_, r)| r.idx == i)
                .map(|(gt, _)| gt.translation())
                .unwrap_or_default(),
    }
}


// ─────────────────────────────────────────────────────────────────────────
//  Distance caméra selon la cible
// ─────────────────────────────────────────────────────────────────────────

fn camera_distance_range(
    target: &CameraTarget,
    settings: &GameSettings,
) -> (f32, f32) {
    match target.0 {
        TargetKind::Planet(i) => {
            let r = settings
                .planets
                .get(i)
                .map(|p| p.radius)
                .unwrap_or(50.0);

            (
                r * 1.4,
                r * 200.0,
            )
        }

        TargetKind::Moon(_, _) => (20.0, 2000.0),

        TargetKind::Star(i) => {
            let r = settings
                .stars
                .get(i)
                .map(|s| s.radius)
                .unwrap_or(200.0);

            (
                r * 0.5,
                r * 150.0,
            )
        }

        // ── Planètes / corps ──────────────────────────────────────────

        TargetKind::GasPlanet(_) =>
            (
                220.0 * 1.3,
                220.0 * 200.0,
            ),

        TargetKind::Comet(_) =>
            (
                80.0,
                20000.0,
            ),

        TargetKind::Meteoroid(_) =>
            (
                30.0,
                4000.0,
            ),

        // ── Étoiles ───────────────────────────────────────────────────

        TargetKind::VoxelStar(_) =>
            (
                120.0 * 0.5,
                120.0 * 200.0,
            ),

        TargetKind::Protostar(_) =>
            (
                60.0 * 1.2,
                12000.0,
            ),

        TargetKind::DwarfStar(_) =>
            (
                45.0 * 1.5,
                8000.0,
            ),

        TargetKind::MainSequence(_) =>
            (
                100.0 * 1.2,
                20000.0,
            ),

        TargetKind::GiantStar(_) =>
            (
                350.0 * 0.6,
                350.0 * 150.0,
            ),

        TargetKind::Supergiant(_) =>
            (
                700.0 * 0.4,
                700.0 * 120.0,
            ),

        TargetKind::Hypergiant(_) =>
            (
                1400.0 * 0.3,
                1400.0 * 100.0,
            ),

        // ── Rémanents ─────────────────────────────────────────────────

        TargetKind::Nebula =>
            (
                600.0 * 0.5,
                600.0 * 100.0,
            ),

        TargetKind::BlackHole(_) =>
            (
                40.0 * 3.0,
                40.0 * 800.0,
            ),

        TargetKind::Pulsar(_) =>
            (
                28.0 * 4.0,
                20000.0,
            ),

        TargetKind::Magnetar(_) =>
            (
                35.0 * 3.0,
                15000.0,
            ),

        TargetKind::NeutronStar(_) =>
            (
                22.0 * 4.0,
                12000.0,
            ),

        TargetKind::Supernova(_) =>
            (
                500.0,
                50000.0,
            ),
    }
}


// ─────────────────────────────────────────────────────────────────────────
//  Direction du soleil
// ─────────────────────────────────────────────────────────────────────────

#[derive(Component)]
struct SunLight;

fn update_sun_direction(
    camera_target: Res<CameraTarget>,

    planet_q:
        Query<
            (&GlobalTransform, &PlanetId),
            (
                With<PlanetRoot>,
                Without<StarRoot>,
            ),
        >,

    star_q:
        Query<
            (&GlobalTransform, &StarId),
            (
                With<StarRoot>,
                Without<PlanetRoot>,
            ),
        >,

    mut sun_q:
        Query<
            &mut Transform,
            (
                With<SunLight>,
                Without<PlanetRoot>,
                Without<StarRoot>,
            ),
        >,
) {
    let target_pos = match camera_target.0 {
        TargetKind::Planet(i) =>
            planet_q
                .iter()
                .find(|(_, pid)| pid.0 == i)
                .map(|(gt, _)| gt.translation()),

        TargetKind::Star(i) =>
            star_q
                .iter()
                .find(|(_, sid)| sid.0 == i)
                .map(|(gt, _)| gt.translation()),

        _ => return,
    };

    let Some(target) = target_pos else {
        return;
    };

    let nearest_star = star_q
        .iter()
        .map(|(gt, _)| gt.translation())
        .min_by(|a, b| {
            a.distance_squared(*b)
                .partial_cmp(
                    &b.distance_squared(*b),
                )
                .unwrap_or(
                    std::cmp::Ordering::Equal,
                )
        });

    let Some(star_pos) = nearest_star else {
        return;
    };

    let dir =
        (target - star_pos)
            .normalize_or_zero();

    if dir.length_squared() < 0.5 {
        return;
    }

    for mut sun_tf in &mut sun_q {
        *sun_tf =
            Transform::default()
                .looking_to(
                    dir,
                    Vec3::Y,
                );
    }
}


// ─────────────────────────────────────────────────────────────────────────
//  HUD — FPS + température
// ─────────────────────────────────────────────────────────────────────────

#[derive(Component)]
struct FpsText;

#[derive(Component)]
struct TempText;


fn setup_fps_display(
    mut commands: Commands,
) {
    commands
        .spawn((
            Node {
                position_type:
                    PositionType::Absolute,

                left:
                    Val::Px(12.0),

                top:
                    Val::Px(12.0),

                flex_direction:
                    FlexDirection::Column,

                row_gap:
                    Val::Px(2.0),

                padding:
                    UiRect::axes(
                        Val::Px(10.0),
                        Val::Px(4.0),
                    ),

                ..default()
            },
        ))
        .with_children(|p| {
            p.spawn((
                Text::new("FPS: --"),

                TextFont {
                    font_size: 16.0,
                    ..default()
                },

                TextColor(
                    Color::srgb(
                        0.0,
                        1.0,
                        0.3,
                    ),
                ),

                FpsText,
            ));

            p.spawn((
                Text::new("Temp: --"),

                TextFont {
                    font_size: 14.0,
                    ..default()
                },

                TextColor(
                    Color::srgb(
                        1.0,
                        0.8,
                        0.2,
                    ),
                ),

                TempText,
            ));
        });
}


fn update_fps_display(
    diagnostics: Res<DiagnosticsStore>,
    settings: Res<GameSettings>,
    camera_target: Res<CameraTarget>,

    mut fps_q:
        Query<
            &mut Text,
            (
                With<FpsText>,
                Without<TempText>,
            ),
        >,

    mut temp_q:
        Query<
            &mut Text,
            (
                With<TempText>,
                Without<FpsText>,
            ),
        >,
) {
    // ── FPS ────────────────────────────────────────────────────────────

    for mut text in &mut fps_q {
        if let Some(fps) =
            diagnostics.get(
                &FrameTimeDiagnosticsPlugin::FPS,
            )
        {
            if let Some(v) =
                fps.smoothed()
            {
                **text =
                    format!(
                        "FPS: {:.0}",
                        v,
                    );
            }
        }
    }

    // ── Température ───────────────────────────────────────────────────

    let temp: f32 =
        match camera_target.0 {
            TargetKind::Planet(i) =>
                settings
                    .planets
                    .get(i)
                    .map(|p| p.temperature())
                    .unwrap_or(15.0),

            TargetKind::Star(_) =>
                5_000.0,

            TargetKind::VoxelStar(_) =>
                5_800.0,

            TargetKind::Protostar(_) =>
                3_200.0,

            TargetKind::DwarfStar(_) =>
                3_500.0,

            TargetKind::MainSequence(_) =>
                5_500.0,

            TargetKind::GiantStar(_) =>
                4_000.0,

            TargetKind::Supergiant(_) =>
                3_500.0,

            TargetKind::Hypergiant(_) =>
                3_000.0,

            TargetKind::BlackHole(_) =>
                1_000_000_000.0,

            TargetKind::Pulsar(_) =>
                1_000_000.0,

            TargetKind::Magnetar(_) =>
                10_000_000.0,

            TargetKind::NeutronStar(_) =>
                600_000.0,

            TargetKind::Supernova(_) =>
                500_000.0,

            _ =>
                -270.0,
        };

    let label =
        temp_label(temp);

    for mut text in &mut temp_q {
        if temp >= 1_000_000.0 {
            **text =
                format!(
                    "{:.2e} C  {}",
                    temp,
                    label,
                );
        } else {
            **text =
                format!(
                    "{:.0} C  {}",
                    temp,
                    label,
                );
        }
    }
}


fn temp_label(
    temp: f32,
) -> &'static str {
    if temp >= 1_000_000.0 {
        "Singularite"
    } else if temp >= 100_000.0 {
        "Plasma"
    } else if temp >= 10_000.0 {
        "Stellaire"
    } else if temp >= 500.0 {
        "Infernal"
    } else if temp >= 200.0 {
        "Brulant"
    } else if temp >= 60.0 {
        "Torride"
    } else if temp >= 30.0 {
        "Chaud"
    } else if temp >= 10.0 {
        "Tempere"
    } else if temp >= -20.0 {
        "Froid"
    } else if temp >= -100.0 {
        "Glace"
    } else if temp >= -200.0 {
        "Gele"
    } else {
        "Zero absolu"
    }
}


// ─────────────────────────────────────────────────────────────────────────
//  Gizmos — indicateur de lumière
// ─────────────────────────────────────────────────────────────────────────

fn draw_light_indicator(
    mut gizmos: Gizmos,
    settings: Res<GameSettings>,

    planet_q:
        Query<
            &GlobalTransform,
            (
                With<PlanetRoot>,
                Without<StarRoot>,
            ),
        >,

    star_q:
        Query<
            &GlobalTransform,
            (
                With<StarRoot>,
                Without<PlanetRoot>,
            ),
        >,
) {
    if !settings.show_light_indicator {
        return;
    }

    let color =
        Color::srgb(
            1.0,
            0.9,
            0.2,
        );

    for star_gt in &star_q {
        let sp =
            star_gt.translation();

        let s = 15.0;

        // Croix X
        gizmos.line(
            sp + Vec3::new(
                -s,
                -s,
                0.0,
            ),
            sp + Vec3::new(
                s,
                s,
                0.0,
            ),
            color,
        );

        gizmos.line(
            sp + Vec3::new(
                -s,
                s,
                0.0,
            ),
            sp + Vec3::new(
                s,
                -s,
                0.0,
            ),
            color,
        );

        // Croix profondeur
        gizmos.line(
            sp + Vec3::new(
                0.0,
                -s,
                -s,
            ),
            sp + Vec3::new(
                0.0,
                s,
                s,
            ),
            color,
        );

        // Lignes vers les planètes
        for planet_gt in &planet_q {
            let pp =
                planet_gt.translation();

            gizmos.line(
                sp,
                pp,
                color,
            );

            let dir =
                (pp - sp)
                    .normalize_or_zero();

            let tip =
                pp - dir * 20.0;

            let up =
                if dir.y.abs() > 0.9 {
                    Vec3::X
                } else {
                    Vec3::Y
                };

            let r =
                dir
                    .cross(up)
                    .normalize_or_zero()
                    * 8.0;

            let u =
                dir
                    .cross(r)
                    .normalize_or_zero()
                    * 8.0;

            gizmos.line(
                pp,
                tip + r,
                color,
            );

            gizmos.line(
                pp,
                tip - r,
                color,
            );

            gizmos.line(
                pp,
                tip + u,
                color,
            );

            gizmos.line(
                pp,
                tip - u,
                color,
            );
        }
    }
}


// ─────────────────────────────────────────────────────────────────────────
//  Gizmos — orbites
// ─────────────────────────────────────────────────────────────────────────

fn draw_orbits(
    mut gizmos: Gizmos,
    settings: Res<GameSettings>,

    planet_q:
        Query<
            (&GlobalTransform, &PlanetId),
            With<PlanetRoot>,
        >,
) {
    if !settings.show_orbits {
        return;
    }

    let seg = 128;

    let draw_ring =
        |gizmos: &mut Gizmos,
         r: f32,
         color: Color,
         center: Vec3| {
            for i in 0..seg {
                let a0 =
                    i as f32
                    / seg as f32
                    * std::f32::consts::TAU;

                let a1 =
                    (i + 1) as f32
                    / seg as f32
                    * std::f32::consts::TAU;

                gizmos.line(
                    center
                        + Vec3::new(
                            a0.cos() * r,
                            0.0,
                            a0.sin() * r,
                        ),

                    center
                        + Vec3::new(
                            a1.cos() * r,
                            0.0,
                            a1.sin() * r,
                        ),

                    color,
                );
            }
        };

    // ── Couleurs ──────────────────────────────────────────────────────

    let planet_color =
        Color::srgba(
            0.30,
            0.60,
            1.00,
            0.40,
        );

    let star_color =
        Color::srgba(
            1.00,
            0.75,
            0.25,
            0.40,
        );

    let belt_color =
        Color::srgba(
            0.70,
            0.55,
            0.95,
            0.30,
        );

    let moon_color =
        Color::srgba(
            0.60,
            0.60,
            0.70,
            0.50,
        );

    let comet_color =
        Color::srgba(
            0.50,
            0.85,
            1.00,
            0.30,
        );

    let remnant_color =
        Color::srgba(
            0.80,
            0.30,
            0.80,
            0.35,
        );

    let astro_color =
        Color::srgba(
            0.90,
            0.60,
            0.20,
            0.35,
        );


    // ── Planètes & lunes ──────────────────────────────────────────────

    for (pi, pcfg) in
        settings.planets.iter().enumerate()
    {
        let r =
            pcfg.orbit_distance;

        if r >= 1.0 {
            draw_ring(
                &mut gizmos,
                r,
                planet_color,
                Vec3::ZERO,
            );
        }

        let planet_pos =
            planet_q
                .iter()
                .find(|(_, pid)| pid.0 == pi)
                .map(|(gt, _)| gt.translation())
                .unwrap_or_default();

        for mcfg in &pcfg.moons {
            draw_ring(
                &mut gizmos,
                mcfg.orbit_distance,
                moon_color,
                planet_pos,
            );
        }
    }


    // ── Étoiles ───────────────────────────────────────────────────────

    for scfg in &settings.stars {
        let r =
            scfg.orbit_distance;

        if r >= 1.0 {
            draw_ring(
                &mut gizmos,
                r,
                star_color,
                Vec3::ZERO,
            );
        }
    }


    // ── Ceintures d'astéroïdes ────────────────────────────────────────

    for belt in &settings.asteroid_belts {
        let r_inner =
            belt.distance
            - belt.width * 1.5;

        let r_outer =
            belt.distance
            + belt.width * 1.5;

        draw_ring(
            &mut gizmos,
            r_inner,
            belt_color,
            Vec3::ZERO,
        );

        draw_ring(
            &mut gizmos,
            r_outer,
            belt_color,
            Vec3::ZERO,
        );
    }


    // ── Comètes ────────────────────────────────────────────────────────

    for comcfg in &settings.comets {
        let r =
            comcfg.orbit_distance;

        if r >= 1.0 {
            draw_ring(
                &mut gizmos,
                r,
                comet_color,
                Vec3::ZERO,
            );
        }
    }


    // ── Rémanents stellaires ──────────────────────────────────────────

    for cfg in &settings.black_holes {
        if cfg.orbit_distance >= 1.0 {
            draw_ring(
                &mut gizmos,
                cfg.orbit_distance,
                remnant_color,
                Vec3::ZERO,
            );
        }
    }

    for cfg in &settings.pulsars {
        if cfg.orbit_distance >= 1.0 {
            draw_ring(
                &mut gizmos,
                cfg.orbit_distance,
                remnant_color,
                Vec3::ZERO,
            );
        }
    }

    for cfg in &settings.magnetars {
        if cfg.orbit_distance >= 1.0 {
            draw_ring(
                &mut gizmos,
                cfg.orbit_distance,
                remnant_color,
                Vec3::ZERO,
            );
        }
    }

    for cfg in &settings.neutron_stars {
        if cfg.orbit_distance >= 1.0 {
            draw_ring(
                &mut gizmos,
                cfg.orbit_distance,
                remnant_color,
                Vec3::ZERO,
            );
        }
    }


    // ── Astres stellaires ─────────────────────────────────────────────

    for cfg in &settings.voxel_stars {
        if cfg.orbit_distance >= 1.0 {
            draw_ring(
                &mut gizmos,
                cfg.orbit_distance,
                astro_color,
                Vec3::ZERO,
            );
        }
    }

    for cfg in &settings.protostars {
        if cfg.orbit_distance >= 1.0 {
            draw_ring(
                &mut gizmos,
                cfg.orbit_distance,
                astro_color,
                Vec3::ZERO,
            );
        }
    }

    for cfg in &settings.dwarf_stars {
        if cfg.orbit_distance >= 1.0 {
            draw_ring(
                &mut gizmos,
                cfg.orbit_distance,
                astro_color,
                Vec3::ZERO,
            );
        }
    }

    for cfg in &settings.main_sequence {
        if cfg.orbit_distance >= 1.0 {
            draw_ring(
                &mut gizmos,
                cfg.orbit_distance,
                astro_color,
                Vec3::ZERO,
            );
        }
    }

    for cfg in &settings.giants {
        if cfg.orbit_distance >= 1.0 {
            draw_ring(
                &mut gizmos,
                cfg.orbit_distance,
                astro_color,
                Vec3::ZERO,
            );
        }
    }

    for cfg in &settings.supergiants {
        if cfg.orbit_distance >= 1.0 {
            draw_ring(
                &mut gizmos,
                cfg.orbit_distance,
                astro_color,
                Vec3::ZERO,
            );
        }
    }

    for cfg in &settings.hypergiants {
        if cfg.orbit_distance >= 1.0 {
            draw_ring(
                &mut gizmos,
                cfg.orbit_distance,
                astro_color,
                Vec3::ZERO,
            );
        }
    }


    // ── Supernovae ────────────────────────────────────────────────────

    for cfg in &settings.supernovae {
        if cfg.orbit_distance >= 1.0 {
            draw_ring(
                &mut gizmos,
                cfg.orbit_distance,
                remnant_color,
                Vec3::ZERO,
            );
        }
    }


    // ── Météoroïdes ───────────────────────────────────────────────────

    for cfg in &settings.meteoroids {
        if cfg.orbit_distance >= 1.0 {
            draw_ring(
                &mut gizmos,
                cfg.orbit_distance,
                comet_color,
                Vec3::ZERO,
            );
        }
    }
}