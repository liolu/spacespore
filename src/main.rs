mod astre;
mod claims;
mod combat;
mod decor;
mod diplomacy;
mod dock;
mod caves;
mod chat_cmd;
mod galaxy_fx;
mod gas;
mod npc_ui;
mod economy;
mod galaxy_shape;
mod graphics;
mod guild;
mod guild_ui;
mod kepler;
mod lod;
mod meteors;
mod mesher;
mod models;
mod net;
mod net_models;
mod net_ui;
mod origin;
mod planet;
mod planetgen;
mod rocks;
mod scanner;
mod settings;
mod systems;
mod asteroids;
mod rings;
mod sky;
mod weather;
mod editeur;
mod ship;
mod stats;
mod suit;
mod surface;
mod test_cmd;
mod system_gen;
mod terrain;
mod ui;
mod voxel;
mod wormhole;
mod world_clock;
mod zones;
mod update_checker;

use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowCloseRequested};

use kepler::OrbitalElements;
use planet::{DistantGalaxyCore, FarStar, GalacticCore, MoonId, MoonRoot, PlanetId, PlanetPlugin, PlanetRoot, StarId, StarRoot};
use settings::{GameSettings, SYSTEM_CELL_SIZE, SYSTEM_GRID_SIZE};
use ship::{Ship, ShipMode, ShipPlugin};
use ui::{CameraTarget, MenuState, TargetKind, UiPlugin};
use net::{Net, NetPlugin};
use net_ui::{NetPanel, NetUiPlugin};

// ── Planètes ──────────────────────────────────────────────────────────────
use astre::{astre_lod_cull, process_pending_reloads, profiling_snapshot, toggle_profiling, ProfilingLog, ReloadAstre};
use astre::planete::gas_planet::GasPlanetRoot;
use astre::planete::comet::CometRoot;
use astre::planete::meteoroid::MeteoroidRoot;

// ── Étoiles ───────────────────────────────────────────────────────────────
use astre::etoile::star::StarRoot as VoxelStarRoot;
use astre::etoile::protostar::ProtostarRoot;
use astre::etoile::dwarf_star::DwarfRoot;
use astre::etoile::main_sequence_star::MsRoot;
use astre::etoile::giant_star::GiantRoot;
use astre::etoile::supergiant_star::SgRoot;
use astre::etoile::hypergiant_star::HgRoot;

// ── Rémanents stellaires ──────────────────────────────────────────────────
#[allow(non_snake_case)]
use astre::Remnant_stellaire::black_hole::BlackHoleRoot;

#[allow(non_snake_case)]
use astre::Remnant_stellaire::nebula::NebulaRoot;

#[allow(non_snake_case)]
use astre::Remnant_stellaire::pulsar::PulsarRoot;

#[allow(non_snake_case)]
use astre::Remnant_stellaire::magnetar::MagnetarRoot;

#[allow(non_snake_case)]
use astre::Remnant_stellaire::neutron_star::NeutronStarRoot;

#[allow(non_snake_case)]
use astre::Remnant_stellaire::supernova::SupernovaRoot;


// ─────────────────────────────────────────────────────────────────────────
//  TargetQueries
//
//  Regroupe toutes les queries nécessaires au contrôleur de caméra.
//  IMPORTANT : ne pas mettre &'w devant les composants dans Query.
// ─────────────────────────────────────────────────────────────────────────

#[derive(SystemParam)]
pub struct TargetQueries<'w, 's> {
    pub wormholes: Res<'w, wormhole::Wormholes>,
    pub surface: Res<'w, surface::Surface>,
    pub asteroids: Res<'w, asteroids::AsteroidField>,

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

    pub far_star_q:
        Query<'w, 's, (&'static GlobalTransform, &'static FarStar)>,

    pub core_q:
        Query<'w, 's, &'static GlobalTransform, With<GalacticCore>>,

    pub dist_core_q:
        Query<'w, 's, (&'static GlobalTransform, &'static DistantGalaxyCore)>,
}


// ─────────────────────────────────────────────────────────────────────────
//  Main
// ─────────────────────────────────────────────────────────────────────────

fn main() {
    // Un raccourci qui ouvre le jeu directement contourne les mises à jour : on le redirige vers le launcher
    std::thread::spawn(spacespore_common::repair_shortcuts);
    let log_path = settings::data_dir().join("crash.log");
    std::panic::set_hook({
        let log_path = log_path.clone();
        Box::new(move |info| {
            let msg = format!("{}\n{:?}\n", info, std::backtrace::Backtrace::capture());
            let _ = std::fs::write(&log_path, &msg);
            eprintln!("{msg}");
        })
    });

    let settings = GameSettings::load();
    lod::set_lod_quality(settings.lod_quality);
    let present_mode = if settings.vsync {
        bevy::window::PresentMode::AutoVsync
    } else {
        bevy::window::PresentMode::AutoNoVsync
    };
    let update_state = update_checker::spawn_update_check();

    App::new()
        .add_plugins(
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "SpaceSpore - Voxel Universe".into(),
                    resolution: (1280.0_f32, 720.0_f32).into(),
                    present_mode,
                    ..default()
                }),
                ..default()
            }),
        )

        .add_plugins(FrameTimeDiagnosticsPlugin)
        .add_plugins(graphics::GraphicsPlugin)

        .insert_resource(settings)
        .insert_resource(update_state)
        .add_plugins(update_checker::UpdateCheckerPlugin)

        // ── Planètes & corps ────────────────────────────────────────────
        .add_plugins(PlanetPlugin)
        .add_plugins(planetgen::PlanetGenPlugin)
        .add_plugins(gas::GasPlugin)
        .add_plugins(scanner::ScannerPlugin)
        .add_plugins(decor::DecorPlugin)
        .add_plugins(test_cmd::TestCmdPlugin)
        .add_plugins(stats::StatsPlugin)
        .add_plugins(zones::ZonesPlugin)
        .add_plugins(models::ModelsPlugin)
        .add_plugins(dock::DockPlugin)

        // ── Legacy astre plugins désactivés — la galaxie gère tout ──
        // Ressources + events vides pour l'UI (pas de Startup spawn)
        .init_resource::<astre::planete::gas_planet::GasPlanetRes>()
        .init_resource::<astre::planete::comet::CometRes>()
        .init_resource::<astre::planete::meteoroid::MeteoroidRes>()
        .init_resource::<astre::etoile::star::StarRes>()
        .init_resource::<astre::etoile::protostar::ProtostarRes>()
        .init_resource::<astre::etoile::dwarf_star::DwarfStarRes>()
        .init_resource::<astre::etoile::main_sequence_star::MainSequenceRes>()
        .init_resource::<astre::etoile::giant_star::GiantStarRes>()
        .init_resource::<astre::etoile::supergiant_star::SupergiantRes>()
        .init_resource::<astre::etoile::hypergiant_star::HypergiantRes>()
        .init_resource::<astre::Remnant_stellaire::nebula::NebulaRes>()
        .init_resource::<astre::Remnant_stellaire::black_hole::BlackHoleRes>()
        .init_resource::<astre::Remnant_stellaire::pulsar::PulsarRes>()
        .init_resource::<astre::Remnant_stellaire::magnetar::MagnetarRes>()
        .init_resource::<astre::Remnant_stellaire::neutron_star::NeutronStarRes>()
        .init_resource::<astre::Remnant_stellaire::supernova::SupernovaRes>()
        .add_event::<astre::planete::gas_planet::RegenerateGasPlanet>()
        .add_event::<astre::planete::comet::RegenerateComet>()
        .add_event::<astre::planete::meteoroid::RegenerateMeteoroid>()
        .add_event::<astre::etoile::star::RegenerateStar>()
        .add_event::<astre::etoile::protostar::RegenerateProtostar>()
        .add_event::<astre::etoile::dwarf_star::RegenerateDwarfStar>()
        .add_event::<astre::etoile::main_sequence_star::RegenerateMainSequence>()
        .add_event::<astre::etoile::giant_star::RegenerateGiantStar>()
        .add_event::<astre::etoile::supergiant_star::RegenerateSupergiant>()
        .add_event::<astre::etoile::hypergiant_star::RegenerateHypergiant>()
        .add_event::<astre::Remnant_stellaire::nebula::RegenerateNebula>()
        .add_event::<astre::Remnant_stellaire::black_hole::RegenerateBlackHole>()
        .add_event::<astre::Remnant_stellaire::pulsar::RegeneratePulsar>()
        .add_event::<astre::Remnant_stellaire::magnetar::RegenerateMagnetar>()
        .add_event::<astre::Remnant_stellaire::neutron_star::RegenerateNeutronStar>()
        .add_event::<astre::Remnant_stellaire::supernova::RegenerateSupernova>()

        // ── Vaisseau ────────────────────────────────────────────────────
        .add_plugins(ShipPlugin)
        .add_plugins(surface::SurfacePlugin)
        .add_plugins(origin::OriginPlugin)
        .add_plugins(world_clock::WorldClockPlugin)
        .add_plugins(meteors::MeteorsPlugin)
        .add_plugins(suit::SuitPlugin)
        .add_plugins(asteroids::AsteroidsPlugin)
        .add_plugins(rings::RingsPlugin)
        .add_plugins(sky::SkyPlugin)
        .add_plugins(weather::WeatherPlugin)
        .add_plugins(editeur::EditeurPlugin)

        // ── UI ──────────────────────────────────────────────────────────
        .add_plugins(UiPlugin)

        // ── Multijoueur ─────────────────────────────────────────────────
        .add_plugins((NetPlugin, NetUiPlugin, claims::ClaimsPlugin, combat::CombatPlugin, guild::GuildPlugin, guild_ui::GuildUiPlugin, wormhole::WormholePlugin, galaxy_fx::GalaxyFxPlugin, npc_ui::NpcUiPlugin, economy::EconomyPlugin, chat_cmd::ChatCmdPlugin))

        .add_event::<ReloadAstre>()
        .init_resource::<ProfilingLog>()
        .insert_resource(ZoomLevel::Planet)

        // ── PreStartup (legacy seed/astres désactivé — galaxie gère tout) ─

        // ── Startup ─────────────────────────────────────────────────────
        .add_systems(
            Startup,
            (
                setup_scene,
                setup_fps_display,
            ),
        )
        .add_systems(PostUpdate, lock_system_at_planet_zoom)

        // ── Update ──────────────────────────────────────────────────────
        .add_systems(
            Update,
            (
                select_world_target,
                highlight_hovered_galaxy,
                select_next_moon,
                select_next_planet,
                draw_body_markers,
                camera_controller,
                update_sun_direction,
                update_fps_display,
                update_system_hud,
                update_zoom_hud,
                draw_light_indicator,
                draw_orbits,
                draw_planet_trails,
                draw_travel_range,
                close_game_when_primary_window_closes,
                toggle_profiling,
                profiling_snapshot,
                astre_lod_cull,
                process_pending_reloads,
                promote_star_target,
            )
                // Pendant l'éditeur (0.12), le jeu ne lit plus le clavier ni la souris
                .run_if(editeur::in_game),
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
            brightness: 300.0,
        },
    );

    // Distance orbitale initiale (système 0)
    let sys0 = settings.systems.first();
    let sys0_center = sys0.map(|s| s.center()).unwrap_or(Vec3::ZERO);
    let orbit_dist = sys0
        .and_then(|s| s.planets().first())
        .map(|p| p.orbit_distance)
        .unwrap_or(450.0);

    let planet_start = sys0_center + Vec3::new(
        orbit_dist,
        0.0,
        0.0,
    );

    // Caméra
    commands.spawn((
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            // Les galaxies lointaines vont jusqu'à ~2 G x GALAXY_SCALE du centre : de quoi les voir
            // de l'autre bout de l'univers
            far: 12_000_000_000.0 * settings::GALAXY_SCALE,
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
            last_target_pos: Vec3::ZERO,
            zoom_goal: None,
        },
    ));
}

/// Rayon de clic (en pixels) autour du centre d'une planète : au moins 80, sinon son disque à l'écran.
fn body_click_tolerance(
    camera: &Camera,
    camera_transform: &GlobalTransform,
    viewport: &graphics::ViewportScale,
    center: Vec3,
    radius: f32,
) -> f32 {
    const MIN: f32 = 80.0;
    let edge = center + camera_transform.right() * radius;
    match (camera.world_to_viewport(camera_transform, center), camera.world_to_viewport(camera_transform, edge)) {
        (Ok(a), Ok(b)) => viewport.to_window(a).distance(viewport.to_window(b)).max(MIN),
        _ => MIN,
    }
}

/// Rayon de clic (en pixels) autour du centre d'une galaxie. À la vue d'ensemble, il suit la
/// taille de la galaxie à l'écran : on la sélectionne en cliquant n'importe où sur son disque.
fn galaxy_click_tolerance(
    camera: &Camera,
    camera_transform: &GlobalTransform,
    viewport: &graphics::ViewportScale,
    settings: &GameSettings,
    overview: bool,
    center: Vec3,
    galaxy_id: usize,
) -> f32 {
    const MIN: f32 = 150.0;
    if !overview {
        return MIN;
    }
    let radius = settings.galaxies.get(galaxy_id).map_or(0.0, |g| g.radius);
    let edge = center + camera_transform.right() * radius;
    match (camera.world_to_viewport(camera_transform, center), camera.world_to_viewport(camera_transform, edge)) {
        (Ok(a), Ok(b)) => viewport.to_window(a).distance(viewport.to_window(b)).clamp(MIN, 600.0),
        _ => MIN,
    }
}

/// À la vue d'ensemble, entoure d'un anneau la galaxie qu'un clic sélectionnerait.
fn highlight_hovered_galaxy(
    primary_window: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform)>,
    queries: TargetQueries,
    settings: Res<GameSettings>,
    zoom: Res<ZoomLevel>,
    viewport: Res<graphics::ViewportScale>,
    ui_interactions: Query<&Interaction>,
    mut gizmos: Gizmos,
) {
    if !matches!(*zoom, ZoomLevel::Cosmos | ZoomLevel::DeepSpace) || ui_interactions.iter().any(|i| *i != Interaction::None) {
        return;
    }
    let Ok(window) = primary_window.get_single() else { return };
    let Some(cursor) = window.cursor_position() else { return };
    let cursor = viewport.to_viewport(cursor);
    let Ok((camera, cam_tf)) = camera_q.get_single() else { return };

    let cores = queries
        .core_q
        .iter()
        .map(|gt| (gt.translation(), 0usize))
        .chain(queries.dist_core_q.iter().map(|(gt, dc)| (gt.translation(), dc.galaxy_id as usize)));
    let mut best: Option<(f32, Vec3, usize)> = None;
    for (center, gid) in cores {
        let Ok(screen) = camera.world_to_viewport(cam_tf, center) else { continue };
        let d = viewport.to_window(screen).distance(viewport.to_window(cursor));
        if d <= galaxy_click_tolerance(camera, cam_tf, &viewport, &settings, true, center, gid) && best.map_or(true, |b| d < b.0) {
            best = Some((d, center, gid));
        }
    }
    let Some((_, center, gid)) = best else { return };
    let Some(gal) = settings.galaxies.get(gid) else { return };
    // Anneau dans le plan de la galaxie
    const SEGMENTS: usize = 64;
    let point = |a: f32| center + gal.tilt * (Vec3::new(a.cos(), 0.0, a.sin()) * gal.radius);
    for s in 0..SEGMENTS {
        let a0 = s as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
        let a1 = (s + 1) as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
        gizmos.line(point(a0), point(a1), Color::srgba(1.0, 1.0, 1.0, 0.7));
    }
}

/// Un astre qu'un clic peut choisir maintenant : où il est à l'écran (pixels de la fenêtre), la
/// distance de clic autour de lui, sa taille à l'écran (rayon en pixels, 0 pour un point).
pub struct Clickable {
    pub kind: TargetKind,
    pub pos: Vec3,
    pub screen: Vec2,
    pub tolerance: f32,
    pub size: f32,
}

/// Rayon d'un astre à l'écran (pixels).
fn screen_radius(camera: &Camera, camera_transform: &GlobalTransform, viewport: &graphics::ViewportScale, center: Vec3, radius: f32) -> f32 {
    let edge = center + camera_transform.right() * radius;
    match (camera.world_to_viewport(camera_transform, center), camera.world_to_viewport(camera_transform, edge)) {
        (Ok(a), Ok(b)) => viewport.to_window(a).distance(viewport.to_window(b)),
        _ => 0.0,
    }
}

/// Tous les astres cliquables maintenant (C6) : une seule règle pour le clic et pour les cercles.
/// Même zoom permis (`can_navigate_to`), mêmes distances de clic ; rien pendant un séjour sur un
/// astre ou un voyage en trou de ver (le clic y est ignoré, l'appelant le vérifie).
#[allow(clippy::too_many_arguments)]
fn clickables(
    camera: &Camera,
    camera_transform: &GlobalTransform,
    viewport: &graphics::ViewportScale,
    ctrl_dist: f32,
    queries: &TargetQueries,
    settings: &GameSettings,
    zoom: &ZoomLevel,
    current_gal: u32,
) -> Vec<Clickable> {
    let mut out = Vec::new();
    let mut add = |position: Vec3, tolerance: f32, size: f32, kind: TargetKind| {
        // Un objet que ce zoom ne permet pas de cibler ne doit pas voler le clic à une galaxie
        if !zoom.can_navigate_to(&kind) {
            return;
        }
        // Derrière la caméra : pas de clic
        if (position - camera_transform.translation()).dot(*camera_transform.forward()) <= 0.0 {
            return;
        }
        let Ok(screen) = camera.world_to_viewport(camera_transform, position) else { return };
        out.push(Clickable { kind, pos: position, screen: viewport.to_window(screen), tolerance, size });
    };
    let body = |center: Vec3, radius: f32| (body_click_tolerance(camera, camera_transform, viewport, center, radius), screen_radius(camera, camera_transform, viewport, center, radius));

    for (transform, id) in &queries.moon_q {
        let kind = TargetKind::Moon(id.planet_idx, id.moon_idx);
        let radius = surface::body_params(settings, &kind).map_or(0.0, |p| p.radius);
        let (tol, size) = body(transform.translation(), radius.max(300.0));
        add(transform.translation(), tol, size, kind);
    }
    for (transform, id) in &queries.planet_q {
        // Les planètes sont immenses : on les sélectionne en cliquant n'importe où sur leur disque
        let radius = surface::body_params(settings, &TargetKind::Planet(id.0)).map_or(0.0, |p| p.radius);
        let (tol, size) = body(transform.translation(), radius);
        add(transform.translation(), tol, size, TargetKind::Planet(id.0));
    }
    for live in queries.asteroids.iter().filter(|l| l.ast.landable()) {
        let center = live.pose.translation;
        let (tol, size) = body(center, live.ast.shape.radius);
        add(center, tol.min(200.0), size, TargetKind::Asteroid(live.ast.key));
    }
    for (transform, id) in &queries.star_q {
        // Les étoiles sont immenses : on les sélectionne en cliquant n'importe où sur leur disque
        let radius = settings.systems.get(id.0 / 1000).and_then(|s| s.stars.get(id.0 % 1000)).map_or(0.0, |s| s.radius);
        let (tol, size) = body(transform.translation(), radius);
        add(transform.translation(), tol, size, TargetKind::Star(id.0));
    }
    for (transform, root) in &queries.gas_q {
        add(transform.translation(), 80.0, 0.0, TargetKind::GasPlanet(root.idx));
    }
    for (transform, root) in &queries.comet_q {
        add(transform.translation(), 80.0, 0.0, TargetKind::Comet(root.idx));
    }
    for (transform, root) in &queries.meteoroid_q {
        add(transform.translation(), 80.0, 0.0, TargetKind::Meteoroid(root.idx));
    }
    for (transform, root) in &queries.vstar_q {
        add(transform.translation(), 80.0, 0.0, TargetKind::VoxelStar(root.idx));
    }
    for (transform, root) in &queries.proto_q {
        add(transform.translation(), 80.0, 0.0, TargetKind::Protostar(root.idx));
    }
    for (transform, root) in &queries.dwarf_q {
        add(transform.translation(), 80.0, 0.0, TargetKind::DwarfStar(root.idx));
    }
    for (transform, root) in &queries.ms_q {
        add(transform.translation(), 80.0, 0.0, TargetKind::MainSequence(root.idx));
    }
    for (transform, root) in &queries.giant_q {
        add(transform.translation(), 80.0, 0.0, TargetKind::GiantStar(root.idx));
    }
    for (transform, root) in &queries.sg_q {
        add(transform.translation(), 80.0, 0.0, TargetKind::Supergiant(root.idx));
    }
    for (transform, root) in &queries.hg_q {
        add(transform.translation(), 80.0, 0.0, TargetKind::Hypergiant(root.idx));
    }
    for (transform, _) in &queries.nebula_q {
        add(transform.translation(), 80.0, 0.0, TargetKind::Nebula);
    }
    for (transform, root) in &queries.black_q {
        add(transform.translation(), 80.0, 0.0, TargetKind::BlackHole(root.idx));
    }
    for (transform, root) in &queries.pulsar_q {
        add(transform.translation(), 80.0, 0.0, TargetKind::Pulsar(root.idx));
    }
    for (transform, root) in &queries.magnetar_q {
        add(transform.translation(), 80.0, 0.0, TargetKind::Magnetar(root.idx));
    }
    for (transform, root) in &queries.neutron_q {
        add(transform.translation(), 80.0, 0.0, TargetKind::NeutronStar(root.idx));
    }
    for (transform, root) in &queries.supernova_q {
        add(transform.translation(), 80.0, 0.0, TargetKind::Supernova(root.idx));
    }

    // Vue d'ensemble (zoom 5-6) : une galaxie se sélectionne en cliquant n'importe où
    // sur son disque, pas seulement sur son trou noir (minuscule à cette distance)
    let overview = matches!(*zoom, ZoomLevel::Cosmos | ZoomLevel::DeepSpace);
    let galaxy = |center: Vec3, galaxy_id: usize| -> (f32, f32) {
        let r = settings.galaxies.get(galaxy_id).map_or(0.0, |g| g.radius);
        (galaxy_click_tolerance(camera, camera_transform, viewport, settings, overview, center, galaxy_id), screen_radius(camera, camera_transform, viewport, center, r))
    };
    for gt in &queries.core_q {
        let center = gt.translation();
        let (tol, size) = galaxy(center, 0);
        add(center, tol, size, TargetKind::GalacticCore);
    }
    // DistantGalaxyCore : depuis la vue d'ensemble (saut entre galaxies), ou le trou noir de la
    // galaxie où l'on se trouve (toutes les galaxies, même celles qui ne sont qu'un point)
    for (gid, g) in settings.galaxies.iter().enumerate().skip(1) {
        let gid = gid as u32;
        if overview || gid == current_gal {
            let center = g.center();
            let (tol, size) = galaxy(center, gid as usize);
            add(center, tol, size, TargetKind::DistantGalaxyCore(gid));
        }
    }

    // Étoiles lointaines (spatial hash autour de la caméra)
    let cam_pos = camera_transform.translation();
    let cell = SYSTEM_CELL_SIZE;
    let half = SYSTEM_GRID_SIZE as f32 * cell / 2.0;
    let cam_col = ((cam_pos.x + half) / cell) as i32;
    let cam_row = ((cam_pos.z + half) / cell) as i32;
    let grid_radius = (ctrl_dist / cell).ceil() as i32 + 2;
    for (gt, fs) in &queries.far_star_q {
        let pos = gt.translation();
        let col = ((pos.x + half) / cell) as i32;
        let row = ((pos.z + half) / cell) as i32;
        if (col - cam_col).abs() > grid_radius || (row - cam_row).abs() > grid_radius || fs.sys_idx >= settings.systems.len() {
            continue;
        }
        add(pos, 60.0, 0.0, TargetKind::Star(fs.sys_idx));
    }

    // Ouvertures de trous de ver (visibles de près), même taille que leur dessin
    for w in &queries.wormholes.list {
        for (sys, mouth) in [(w.a, w.mouth_a()), (w.b, w.mouth_b())] {
            let dist = cam_pos.distance(mouth);
            if dist <= wormhole::DRAW_RANGE {
                let r = (dist * 0.012).max(wormhole::MIN_DRAW_RADIUS);
                add(mouth, r * 1.5, screen_radius(camera, camera_transform, viewport, mouth, r), TargetKind::WormholeMouth(sys));
            }
        }
    }
    out
}

fn select_world_target(
    buttons: Res<ButtonInput<MouseButton>>,
    primary_window: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform, &CameraController)>,
    queries: TargetQueries,
    settings: Res<GameSettings>,
    mut target: ResMut<CameraTarget>,
    zoom: Res<ZoomLevel>,
    ui_interactions: Query<&Interaction>,
    viewport: Res<graphics::ViewportScale>,
    ship_q: Query<&GlobalTransform, With<Ship>>,
    time: Res<Time>,
    mut net: ResMut<Net>,
    travel: Res<wormhole::WormholeTravel>,
) {
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    // Pendant un voyage en trou de ver, on ne change pas de cible
    if travel.active() {
        return;
    }
    // Atterri ou en vol d'atterrissage : la souris sert à regarder autour
    if queries.surface.active() {
        return;
    }
    // Clic sur un élément d'interface : ne pas sélectionner d'astre derrière
    if ui_interactions.iter().any(|i| *i != Interaction::None) {
        return;
    }
    let Ok(window) = primary_window.get_single() else { return; };
    let Some(cursor) = window.cursor_position() else { return; };
    // Échelle de rendu < 100 % : la caméra 3D a son propre repère écran
    let cursor = viewport.to_viewport(cursor);
    let Ok((camera, camera_transform, ctrl)) = camera_q.get_single() else { return; };
    let ctrl_dist = ctrl.distance;
    let current_gal = current_galaxy(&target.0, &queries, &settings);
    // La règle « cliquable » est la même que celle des cercles (`clickables`)
    let mut best: Option<(f32, TargetKind)> = None;
    for c in clickables(camera, camera_transform, &viewport, ctrl_dist, &queries, &settings, &zoom, current_gal) {
        let d = c.screen.distance(viewport.to_window(cursor));
        if d <= c.tolerance && best.map_or(true, |(b, _)| d < b) {
            best = Some((d, c.kind));
        }
    }

    if let Some((_, selected)) = best {
        // Changer de galaxie (trou noir d'une autre galaxie, mais aussi n'importe laquelle de ses
        // étoiles) demande deux choses : être dézoomé à plus de 10 000 000, et que le vaisseau
        // soit sur le trou noir de la galaxie où l'on est
        if current_galaxy(&selected, &queries, &settings) != current_gal {
            let now = time.elapsed_secs_f64();
            if !ZoomLevel::is_core(&selected) || ctrl_dist < GALAXY_JUMP_MIN_ZOOM {
                net.notify(&format!("Pour changer de galaxie : dezoomez a plus de {:.0} et choisissez son trou noir.", GALAXY_JUMP_MIN_ZOOM), now);
                return;
            }
            let at_core = settings.galaxies.get(current_gal as usize).zip(ship_q.get_single().ok()).is_some_and(|(g, ship)| {
                ship.translation().distance(g.center()) <= g.core_radius * 4.0
            });
            if !at_core {
                net.notify("Rejoignez le trou noir de votre galaxie pour sauter vers une autre galaxie.", now);
                return;
            }
        }
        if zoom.can_navigate_to(&selected) {
            // Portée de déplacement fixe : seuls les trous noirs de galaxie (sauts entre galaxies)
            // et les cibles du système où l'on est peuvent être hors de portée
            let too_far = !ZoomLevel::is_core(&selected)
                && ship_q.get_single().is_ok_and(|ship| {
                    let pos = resolve_target(&CameraTarget(selected), &queries, &settings);
                    pos != Vec3::ZERO && pos.distance(ship.translation()) > MAX_TRAVEL_RANGE
                });
            if too_far {
                net.notify(&format!("Trop loin : votre vaisseau ne peut pas se deplacer a plus de {:.0} d'un coup (cercle blanc). Passez par un trou de ver ou avancez etape par etape.", MAX_TRAVEL_RANGE), time.elapsed_secs_f64());
            } else {
                target.0 = selected;
            }
        }
    }
}

/// Galaxie dans laquelle se trouve la cible (0 = galaxie principale).
fn current_galaxy(kind: &TargetKind, queries: &TargetQueries, settings: &GameSettings) -> u32 {
    let sys_idx = match *kind {
        TargetKind::DistantGalaxyCore(id) => return id,
        TargetKind::Planet(id) => id / 1000,
        TargetKind::Moon(planet_idx, _) => planet_idx / 1000,
        TargetKind::Asteroid(key) => key.sys as usize,
        // Étoile chargée : id = sys * 1000 + i ; étoile lointaine : id = index du système
        TargetKind::Star(id) => {
            if queries.star_q.iter().any(|(_, sid)| sid.0 == id) { id / 1000 } else { id }
        }
        TargetKind::WormholeMouth(sys) => sys,
        _ => return 0,
    };
    settings.systems.get(sys_idx).map_or(0, |s| s.galaxy_id)
}

/// Système auquel appartient une cible (`None` = hors de tout système, ex. noyau).
pub(crate) fn target_system(
    kind: &TargetKind,
    star_q: &Query<&StarId, With<StarRoot>>,
) -> Option<Option<usize>> {
    match *kind {
        TargetKind::Planet(id) => Some(Some(id / 1000)),
        TargetKind::Moon(planet_idx, _) => Some(Some(planet_idx / 1000)),
        TargetKind::Asteroid(key) => Some(Some(key.sys as usize)),
        // Étoile chargée : id = sys * 1000 + i ; étoile lointaine : id = index du système
        TargetKind::Star(id) => {
            if star_q.iter().any(|sid| sid.0 == id) {
                Some(Some(id / 1000))
            } else {
                Some(Some(id))
            }
        }
        TargetKind::GalacticCore | TargetKind::DistantGalaxyCore(_) => Some(None),
        // Astres historiques (désactivés) : pas de contrainte
        _ => None,
    }
}

/// Au zoom 1 (< 10 000), on ne peut pas sortir du système courant :
/// toute nouvelle cible appartenant à un autre système est annulée.
fn lock_system_at_planet_zoom(
    mut target: ResMut<CameraTarget>,
    zoom: Res<ZoomLevel>,
    spawned: Res<planet::SpawnedSystems>,
    star_q: Query<&StarId, With<StarRoot>>,
    mut previous: Local<Option<TargetKind>>,
) {
    if target.is_changed() && *zoom == ZoomLevel::Planet {
        if let Some(&current_sys) = spawned.0.iter().next() {
            let leaves_system = match target_system(&target.0, &star_q) {
                Some(Some(si)) => si != current_sys,
                Some(None) => true,
                None => false,
            };
            if leaves_system {
                if let Some(prev) = *previous {
                    target.0 = prev;
                }
                return;
            }
        }
    }
    *previous = Some(target.0);
}

/// `P` : passe à la planète suivante du système chargé (à ces échelles, les planètes sont des
/// points minuscules : impossible de les viser à la souris depuis l'étoile).
fn select_next_planet(
    keys: Res<ButtonInput<KeyCode>>,
    settings: Res<GameSettings>,
    mut target: ResMut<CameraTarget>,
    net_panel: Res<NetPanel>,
    menu: Res<MenuState>,
    travel: Res<wormhole::WormholeTravel>,
    surface: Res<surface::Surface>,
    spawned: Res<planet::SpawnedSystems>,
) {
    if !keys.just_pressed(KeyCode::KeyP) || net_panel.focus.is_some() || menu.open || travel.active() || surface.active() {
        return;
    }
    let Some(&sys_i) = spawned.0.iter().next() else { return };
    let Some(count) = settings.systems.get(sys_i).map(|s| s.planets().len()).filter(|&n| n > 0) else { return };
    let next = match target.0 {
        TargetKind::Planet(id) | TargetKind::Moon(id, _) if id / 1000 == sys_i => (id % 1000 + 1) % count,
        _ => 0,
    };
    target.0 = TargetKind::Planet(sys_i * 1000 + next);
}

/// Cercles autour des planètes et des lunes vues de loin : sans eux, à l'échelle d'un système
/// (étoile de 100 000 de rayon), elles sont invisibles.
/// Un cercle autour de chaque astre cliquable (C6), avec la même règle que le clic
/// (`clickables`) : il apparaît dès que le clic est possible et s'efface dès qu'il ne l'est plus
/// (fondu court). Couleur par type, la cible en jaune. Rien autour d'un astre déjà grand à l'écran.
/// Groupe de gizmos par défaut : les indicateurs (orbites...) s'effacent près d'un astre, pas eux.
#[allow(clippy::too_many_arguments)]
fn draw_body_markers(
    time: Res<Time>,
    zoom: Res<ZoomLevel>,
    settings: Res<GameSettings>,
    target: Res<CameraTarget>,
    camera_q: Query<(&Camera, &GlobalTransform, &CameraController, &Projection)>,
    viewport: Res<graphics::ViewportScale>,
    queries: TargetQueries,
    travel: Res<wormhole::WormholeTravel>,
    mut fades: Local<Vec<(TargetKind, f32, Vec3, f32)>>,
    mut gizmos: Gizmos,
) {
    let dt = time.delta_secs().min(0.1);
    let Ok((camera, cam_gt, ctrl, projection)) = camera_q.get_single() else { return };
    let now: Vec<Clickable> = if queries.surface.active() || travel.active() {
        Vec::new()
    } else {
        let current_gal = current_galaxy(&target.0, &queries, &settings);
        let mut list = clickables(camera, cam_gt, &viewport, ctrl.distance, &queries, &settings, &zoom, current_gal);
        // Les étoiles lointaines sont des centaines : les plus proches du centre de l'écran
        let center = viewport.to_window(camera.logical_viewport_size().unwrap_or(Vec2::splat(800.0)) * 0.5);
        list.sort_by(|a, b| a.screen.distance(center).total_cmp(&b.screen.distance(center)));
        let mut stars = 0;
        list.retain(|c| {
            let far = c.size == 0.0 && matches!(c.kind, TargetKind::Star(_));
            stars += far as usize;
            !far || stars <= 60
        });
        list
    };
    // Fondu : 0,2 s pour apparaître ou disparaître
    let rate = dt / 0.2;
    for f in fades.iter_mut() {
        f.1 = (f.1 - rate).max(0.0);
    }
    for c in &now {
        let r = (c.size * 1.25 + 6.0).clamp(10.0, c.tolerance.max(10.0));
        match fades.iter_mut().find(|f| f.0 == c.kind) {
            Some(f) => {
                f.1 = (f.1 + rate * 2.0).min(1.0);
                f.2 = c.pos;
                f.3 = r;
            }
            None => fades.push((c.kind, rate, c.pos, r)),
        }
    }
    fades.retain(|f| f.1 > 0.0);
    let height = camera.logical_viewport_size().map_or(800.0, |s| viewport.to_window(s).y);
    // Le cercle est posé à la profondeur de l'astre : taille d'un pixel à cette profondeur
    let (eye, fwd) = (cam_gt.translation(), *cam_gt.forward());
    let fov = match projection {
        Projection::Perspective(p) => p.fov,
        _ => std::f32::consts::FRAC_PI_4,
    };
    let px_at = |_pos: Vec3, depth: f32| -> f32 { 2.0 * depth * (fov * 0.5).tan() / height.max(1.0) };
    for (kind, a, pos, r) in fades.iter() {
        // Un astre déjà grand à l'écran se voit sans cercle
        if *r > height * 0.25 {
            continue;
        }
        let selected = *kind == target.0;
        let color = if selected { Color::srgba(1.0, 0.9, 0.3, *a) } else { marker_color(kind).with_alpha(0.75 * a) };
        let depth = (*pos - eye).dot(fwd);
        if depth > 0.0 {
            gizmos.circle(Isometry3d::new(*pos, cam_gt.rotation()), r * px_at(*pos, depth) * if selected { 1.3 } else { 1.0 }, color);
        }
    }
}

/// Couleur du cercle d'un astre cliquable, par type.
fn marker_color(kind: &TargetKind) -> Color {
    match kind {
        TargetKind::Planet(_) => Color::srgb(0.4, 0.8, 1.0),
        TargetKind::Moon(..) => Color::srgb(0.75, 0.75, 0.8),
        TargetKind::Star(_) => Color::srgb(1.0, 0.75, 0.45),
        TargetKind::Asteroid(_) => Color::srgb(0.75, 0.6, 0.45),
        TargetKind::Comet(_) => Color::srgb(0.5, 0.9, 1.0),
        TargetKind::WormholeMouth(_) => Color::srgb(0.75, 0.45, 1.0),
        TargetKind::GalacticCore | TargetKind::DistantGalaxyCore(_) => Color::srgb(1.0, 0.95, 0.85),
        TargetKind::BlackHole(_) | TargetKind::Pulsar(_) | TargetKind::Magnetar(_) | TargetKind::NeutronStar(_) => Color::srgb(0.9, 0.5, 1.0),
        _ => Color::srgb(0.85, 0.85, 0.85),
    }
}

fn select_next_moon(
    keys: Res<ButtonInput<KeyCode>>,
    settings: Res<GameSettings>,
    mut target: ResMut<CameraTarget>,
    net_panel: Res<NetPanel>,
    travel: Res<wormhole::WormholeTravel>,
    surface: Res<surface::Surface>,
) {
    if !keys.just_pressed(KeyCode::KeyM) || net_panel.focus.is_some() || travel.active() || surface.active() {
        return;
    }

    let (planet_idx, current_moon) = match target.0 {
        TargetKind::Planet(index) => (index, None),
        TargetKind::Moon(planet_idx, moon_idx) => (planet_idx, Some(moon_idx)),
        _ => return,
    };
    let sys_i = planet_idx / 1000;
    let local_i = planet_idx % 1000;
    let Some(moons) = settings.systems.get(sys_i).and_then(|s| s.planets().get(local_i)).map(|p| &p.moons) else {
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
pub struct CameraController {
    yaw: f32,
    pitch: f32,
    distance: f32,
    last_target_pos: Vec3,
    /// Zoom automatique en cours (ex. clic sur une galaxie depuis l'espace profond).
    zoom_goal: Option<f32>,
}

/// Limite du zoom 1 : en dessous, on reste verrouillé dans le système courant.
/// Un système s'étend sur ~4 millions d'unités en médiane, 11 millions pour 99 % d'entre eux (1 à 8
/// planètes, voir `planetgen::system`) : le niveau « Planète » les contient en entier.
pub const ZOOM_PLANET_MAX: f32 = 18_000_000.0 * settings::SPACE_STRETCH;

/// Distance de caméra pour voir une galaxie entière (reste au zoom 4 pour
/// pouvoir cliquer ses étoiles).
fn galaxy_view_distance(kind: &TargetKind, settings: &GameSettings) -> f32 {
    let gid = match *kind {
        TargetKind::DistantGalaxyCore(id) => id as usize,
        _ => 0,
    };
    let radius = settings.galaxies.get(gid).map_or(settings::GALAXY_RADIUS, |g| g.radius);
    (radius * 1.5).clamp(2_000_000.0 * settings::GALAXY_SCALE, 5_900_000.0 * settings::GALAXY_SCALE)
}

/// Au-delà de cette distance (changement de galaxie), le vaisseau saute
/// directement à destination au lieu de voyager en croisière.
const HYPERJUMP_DIST: f32 = 20_000_000.0 * settings::GALAXY_SCALE;

/// Distance de caméra minimale pour pouvoir sélectionner une autre galaxie (saut entre galaxies).
const GALAXY_JUMP_MIN_ZOOM: f32 = 10_000_000.0 * settings::GALAXY_SCALE;

/// Portée fixe d'un déplacement du vaisseau (le cercle blanc). Au-delà, il faut avancer par
/// étapes, passer par un trou de ver, ou sauter entre galaxies via leur trou noir.
/// Un déplacement va d'une étoile à sa voisine (≈ 15 millions) : quelques fois l'écart entre étoiles.
const MAX_TRAVEL_RANGE: f32 = 750_000.0 * settings::GALAXY_SCALE;
/// Zoom maximal de la caméra.
const MAX_ZOOM: f32 = 400_000_000.0 * settings::GALAXY_SCALE;

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoomLevel {
    Planet,
    System,
    Sector,
    Galaxy,
    Cosmos,
    DeepSpace,
}

impl ZoomLevel {
    fn from_distance(d: f32) -> Self {
        if d < ZOOM_PLANET_MAX {
            ZoomLevel::Planet
        } else if d < 30_000_000.0 * settings::SPACE_STRETCH {
            ZoomLevel::System
        } else if d < 2_000_000.0 * settings::GALAXY_SCALE {
            ZoomLevel::Sector
        } else if d < 6_000_000.0 * settings::GALAXY_SCALE {
            ZoomLevel::Galaxy
        } else if d < 20_000_000.0 * settings::GALAXY_SCALE {
            ZoomLevel::Cosmos
        } else {
            ZoomLevel::DeepSpace
        }
    }

    pub fn level_number(&self) -> u32 {
        match self {
            ZoomLevel::Planet => 1,
            ZoomLevel::System => 2,
            ZoomLevel::Sector => 3,
            ZoomLevel::Galaxy => 4,
            ZoomLevel::Cosmos => 5,
            ZoomLevel::DeepSpace => 6,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            ZoomLevel::Planet => "Planete",
            ZoomLevel::System => "Systeme",
            ZoomLevel::Sector => "Secteur",
            ZoomLevel::Galaxy => "Galaxie",
            ZoomLevel::Cosmos => "Cosmos",
            ZoomLevel::DeepSpace => "Espace Profond",
        }
    }

    fn is_star(kind: &TargetKind) -> bool {
        matches!(kind,
            TargetKind::Star(_) | TargetKind::VoxelStar(_) | TargetKind::Protostar(_)
            | TargetKind::DwarfStar(_) | TargetKind::MainSequence(_) | TargetKind::GiantStar(_)
            | TargetKind::Supergiant(_) | TargetKind::Hypergiant(_)
        )
    }

    fn is_core(kind: &TargetKind) -> bool {
        matches!(kind, TargetKind::GalacticCore | TargetKind::DistantGalaxyCore(_))
    }

    fn can_navigate_to(&self, kind: &TargetKind) -> bool {
        match self {
            ZoomLevel::Planet | ZoomLevel::System => true,
            ZoomLevel::Sector | ZoomLevel::Galaxy => {
                Self::is_star(kind) || Self::is_core(kind) || matches!(kind, TargetKind::WormholeMouth(_))
            }
            ZoomLevel::Cosmos => Self::is_core(kind),
            ZoomLevel::DeepSpace => matches!(kind, TargetKind::GalacticCore | TargetKind::DistantGalaxyCore(_)),
        }
    }
}

/// Hauteur fixe du vaisseau au-dessus de l'astre ciblé : toujours au-dessus, quelle que soit sa
/// taille (une géante ne l'engloutit pas), et indépendante du zoom.
fn hover_height(target: &CameraTarget, settings: &GameSettings) -> f32 {
    hover_height_for(target, settings, None)
}

/// Système et rang d'une étoile ciblée : chargée = `système * 1000 + n`, lointaine = indice du
/// système (son étoile principale).
pub(crate) fn star_parts(id: usize, loaded: bool) -> (usize, usize) {
    if loaded { (id / 1000, id % 1000) } else { (id, 0) }
}

/// Étoile ciblée de loin (indice du système) : une fois son système chargé, la cible devient sa
/// vraie étoile principale. Sans ça, le vaisseau reste au centre du système, qui est vide pour une
/// étoile double ou triple (centre de masse entre les étoiles).
fn promote_star_target(spawned: Res<planet::SpawnedSystems>, star_q: Query<&StarId, With<StarRoot>>, mut target: ResMut<CameraTarget>) {
    let TargetKind::Star(i) = target.0 else { return };
    if !spawned.0.contains(&i) || star_q.iter().any(|s| s.0 == i) {
        return;
    }
    if star_q.iter().any(|s| s.0 == i * 1000) {
        target.0 = TargetKind::Star(i * 1000);
    }
}

/// Rayon de l'étoile ciblée (chargée : `système * 1000 + n` ; lointaine : indice du système).
fn star_radius(kind: &TargetKind, queries: &TargetQueries, settings: &GameSettings) -> Option<f32> {
    let TargetKind::Star(id) = *kind else { return None };
    let loaded = queries.star_q.iter().any(|(_, s)| s.0 == id);
    let (system, index) = if loaded { (id / 1000, id % 1000) } else { (id, 0) };
    settings.systems.get(system)?.stars.get(index).map(|s| s.radius)
}

fn hover_height_for(target: &CameraTarget, settings: &GameSettings, star_r: Option<f32>) -> f32 {
    // Étoile : au-dessus de sa surface (elles font 100 000 de rayon et plus)
    if let Some(r) = star_r {
        return r * 1.3;
    }
    // Trou noir : le vaisseau se place au-dessus de la sphère, pas à l'intérieur
    let core_radius = match target.0 {
        TargetKind::GalacticCore => settings.galaxies.first().map(|g| g.core_radius),
        TargetKind::DistantGalaxyCore(id) => settings.galaxies.get(id as usize).map(|g| g.core_radius),
        _ => None,
    };
    if let Some(r) = core_radius {
        return r * 1.8;
    }
    // Planète ou lune : au-dessus de sa surface (elles sont immenses)
    if let Some(params) = surface::body_params(settings, &target.0) {
        return surface::hover_radius(&params);
    }
    let (min_distance, _) = camera_distance_range(target, settings, None);
    (min_distance * 0.5).max(80.0)
}

/// Orientation (rotation propre) d'une planète ou d'une lune chargée ; identité sinon.
fn body_spin(q: &TargetQueries, kind: &TargetKind) -> Quat {
    let gt = match *kind {
        TargetKind::Planet(id) => q.planet_q.iter().find(|(_, p)| p.0 == id).map(|(gt, _)| gt),
        TargetKind::Moon(planet_idx, moon_idx) => q.moon_q.iter().find(|(_, m)| m.planet_idx == planet_idx && m.moon_idx == moon_idx).map(|(gt, _)| gt),
        TargetKind::Asteroid(key) => return q.asteroids.pose(&key).map_or(Quat::IDENTITY, |t| t.rotation),
        _ => None,
    };
    gt.map_or(Quat::IDENTITY, |gt| gt.to_scale_rotation_translation().1)
}

/// Point de stationnement du vaisseau : au-dessus de l'astre ciblé (de son dernier point
/// d'atterrissage s'il y en a un, sinon au pôle nord).
#[allow(clippy::too_many_arguments)]
fn hover_position(
    target_pos: Vec3,
    target: &CameraTarget,
    settings: &GameSettings,
    surface: &surface::Surface,
    net: &Net,
    zoom_distance: f32,
    star_r: Option<f32>,
    spin: Quat,
) -> Vec3 {
    // Le point de stationnement est fixe dans le repère de l'astre : il tourne avec lui (règle 10)
    let up = spin * surface.hover_dir(&target.0).unwrap_or(Vec3::Y);
    target_pos + up * hover_height_for(target, settings, star_r) + hover_offset(&target.0, net, zoom_distance)
}

/// Décalage du point de stationnement : aucun sur un trou noir ou un trou de ver (le vaisseau s'y
/// centre), un léger écart autour des autres astres pour les joueurs qui les partagent.
fn hover_offset(kind: &TargetKind, net: &Net, zoom_distance: f32) -> Vec3 {
    match kind {
        TargetKind::GalacticCore | TargetKind::DistantGalaxyCore(_) | TargetKind::WormholeMouth(_) => Vec3::ZERO,
        _ => net.hover_offset(zoom_distance),
    }
}

/// Amène le vaisseau à son point de stationnement : croisière, ou saut direct si `snap`
/// (vaisseau caché, autre galaxie, ou déjà arrivé). Renvoie le pas fait par ses moteurs (zéro
/// quand il suit simplement l'astre).
fn steer_ship(ship_tf: &mut Transform, hover_pos: Vec3, snap: bool, dt: f32) -> Vec3 {
    let to_hover = hover_pos - ship_tf.translation;
    let dist = to_hover.length();
    // Un astre en orbite se déplace de plusieurs unités par image : on le suit sans « croisière »
    if snap || dist > HYPERJUMP_DIST || dist <= (1500.0 * dt).max(30.0) {
        ship_tf.translation = hover_pos;
        Vec3::ZERO
    } else {
        let cruise = (dist * 0.8).max(3000.0).min(50_000_000.0 * settings::SPACE_STRETCH);
        let step = to_hover.normalize() * (cruise * dt).min(dist);
        ship_tf.translation += step;
        step
    }
}

/// Croisière puis stationnement : le vaisseau va vers son point, le nez vers la destination
/// (virage doux, sans roulis), puis prend la pose de stationnement à l'approche ; la poussée suit.
#[allow(clippy::too_many_arguments)]
fn fly_ship(ship_tf: &mut Transform, thrust: &mut ship::ShipThrust, hover_pos: Vec3, target_pos: Vec3, levels: bool, snap: bool, wormhole: bool, dt: f32) {
    if wormhole {
        thrust.push = Vec3::NEG_Z;
        return;
    }
    let before = ship_tf.rotation;
    let step = steer_ship(ship_tf, hover_pos, snap, dt);
    ship_tf.rotation = surface::orient_ship(before, ship_tf.translation, step, hover_pos, target_pos, levels, snap, dt);
    thrust.push = if step == Vec3::ZERO { Vec3::ZERO } else { ship_tf.rotation.inverse() * step.normalize() };
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

    mut ship_q: Query<(&mut Transform, &mut Visibility, &mut ship::ShipThrust), With<Ship>>,
    mut zoom_level: ResMut<ZoomLevel>,

    mut cam_q:
        Query<(&mut Transform, &mut CameraController), Without<Ship>>,

    net_panel: Res<NetPanel>,
    net: Res<Net>,
    travel: Res<wormhole::WormholeTravel>,
) {
    // Saisie de texte en cours (panneau multijoueur) : clavier réservé au champ
    let menu_open = menu_state.open || net_panel.focus.is_some();

    let Ok((mut cam_tf, mut ctrl)) =
        cam_q.get_single_mut()
    else {
        return;
    };

    // ── Atterrissage / marche : `surface.rs` pilote la caméra et le vaisseau ──
    if queries.surface.active() {
        mouse_motion.clear();
        mouse_wheel.clear();
        return;
    }

    // ── Mode vue libre (F1) : caméra libre sans vaisseau ─────────────
    if *ship_mode == ShipMode::Free {
        if menu_open {
            mouse_motion.clear();
            mouse_wheel.clear();
            return;
        }

        let dt = time.delta_secs();
        let move_speed = 300.0 * dt;
        let mouse_sens = settings.mouse_sensitivity * 0.003;

        if mouse_buttons.pressed(MouseButton::Right) {
            for ev in mouse_motion.read() {
                ctrl.yaw -= ev.delta.x * mouse_sens;
                ctrl.pitch -= ev.delta.y * mouse_sens;
            }
        } else {
            mouse_motion.clear();
        }
        mouse_wheel.clear();

        ctrl.pitch = ctrl.pitch.clamp(-1.5, 1.5);

        let rotation = Quat::from_euler(EulerRot::YXZ, ctrl.yaw, ctrl.pitch, 0.0);
        let forward = rotation * Vec3::NEG_Z;
        let right = rotation * Vec3::X;

        let mut dir = Vec3::ZERO;
        if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) { dir += forward; }
        if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) { dir -= forward; }
        if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) { dir -= right; }
        if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) { dir += right; }
        if keys.pressed(KeyCode::Space) { dir += Vec3::Y; }
        if keys.pressed(KeyCode::ShiftLeft) { dir -= Vec3::Y; }

        if dir.length_squared() > 0.0 {
            cam_tf.translation += dir.normalize() * move_speed;
        }
        cam_tf.rotation = rotation;
        return;
    }

    // ── Mode vaisseau (defaut) : vaisseau orbite l'astre ─────────────
    let star_r = star_radius(&camera_target.0, &queries, &settings);
    let raw_target = resolve_target(&camera_target, &queries, &settings);
    // Un astre introuvable (pas encore chargé) résout à l'origine : on garde alors la dernière position.
    // Mais le trou noir de la galaxie principale peut être lui-même à l'origine : c'est une vraie position.
    let is_origin_core = matches!(camera_target.0, TargetKind::GalacticCore);
    let target_pos = if raw_target == Vec3::ZERO && !is_origin_core && ctrl.last_target_pos.length_squared() > 100.0 {
        ctrl.last_target_pos
    } else {
        ctrl.last_target_pos = raw_target;
        raw_target
    };

    if menu_open {
        mouse_motion.clear();
        mouse_wheel.clear();

        let cam_rotation = Quat::from_euler(EulerRot::YXZ, ctrl.yaw, ctrl.pitch, 0.0);

        let sp = if let Ok((mut ship_tf, mut ship_vis, mut thrust)) = ship_q.get_single_mut() {
            let hide_ship = matches!(*zoom_level, ZoomLevel::Galaxy | ZoomLevel::Cosmos | ZoomLevel::DeepSpace);
            let hover_pos = hover_position(target_pos, &camera_target, &settings, &queries.surface, &net, ctrl.distance, star_r, body_spin(&queries, &camera_target.0));
            // Caché (vue galaxie) : le vaisseau se place quand même sur son astre, sans croisière
            let levels = surface::body_params(&settings, &camera_target.0).is_some();
            fly_ship(&mut ship_tf, &mut thrust, hover_pos, target_pos, levels, hide_ship, travel.active(), time.delta_secs());
            if hide_ship {
                *ship_vis = Visibility::Hidden;
                cam_tf.translation = target_pos + cam_rotation * Vec3::new(0.0, 0.0, ctrl.distance);
                cam_tf.look_at(target_pos, Vec3::Y);
                return;
            }
            *ship_vis = Visibility::Inherited;
            let pos = ship_tf.translation;
            ship_tf.scale = Vec3::splat(ctrl.distance.max(1.0) * 0.008);
            pos
        } else {
            target_pos
        };

        cam_tf.translation = sp + cam_rotation * Vec3::new(0.0, 0.0, ctrl.distance);
        cam_tf.look_at(sp, Vec3::Y);
        return;
    }

    // ── Controles caméra orbitale ──────────────────────────────────
    let rotate_speed = settings.keyboard_speed * time.delta_secs();
    let mouse_sens = settings.mouse_sensitivity * 0.01;
    let y_mult = if settings.invert_y { -1.0 } else { 1.0 };

    if keys.pressed(KeyCode::ArrowLeft) || keys.pressed(KeyCode::KeyA) {
        ctrl.yaw += rotate_speed;
    }
    if keys.pressed(KeyCode::ArrowRight) || keys.pressed(KeyCode::KeyD) {
        ctrl.yaw -= rotate_speed;
    }
    if keys.pressed(KeyCode::ArrowUp) || keys.pressed(KeyCode::KeyW) {
        ctrl.pitch += rotate_speed * y_mult;
    }
    if keys.pressed(KeyCode::ArrowDown) || keys.pressed(KeyCode::KeyS) {
        ctrl.pitch -= rotate_speed * y_mult;
    }

    if mouse_buttons.pressed(MouseButton::Right) {
        for ev in mouse_motion.read() {
            ctrl.yaw -= ev.delta.x * mouse_sens;
            ctrl.pitch += ev.delta.y * mouse_sens * y_mult;
        }
    } else {
        mouse_motion.clear();
    }

    // Galaxie sélectionnée depuis l'espace profond : on plonge dedans
    if camera_target.is_changed()
        && ZoomLevel::is_core(&camera_target.0)
        && ctrl.distance >= 20_000_000.0 * settings::GALAXY_SCALE
    {
        ctrl.zoom_goal = Some(galaxy_view_distance(&camera_target.0, &settings));
    }

    for ev in mouse_wheel.read() {
        // La molette reprend la main sur le zoom automatique
        ctrl.zoom_goal = None;
        let y = ui::wheel_lines(ev);
        let direction_factor = if y < 0.0 { 2.5 } else { 1.0 };
        let zoom_factor = 1.0 + ctrl.distance.abs() * 0.004 * direction_factor;
        ctrl.distance -= y * settings.scroll_speed * zoom_factor;
    }

    if let Some(goal) = ctrl.zoom_goal {
        // Interpolation logarithmique : descente fluide sur plusieurs ordres de grandeur
        let t = 1.0 - (-3.0 * time.delta_secs()).exp();
        let cur = ctrl.distance.max(1.0).ln();
        ctrl.distance = (cur + (goal.ln() - cur) * t).exp();
        if (ctrl.distance / goal - 1.0).abs() < 0.01 {
            ctrl.distance = goal;
            ctrl.zoom_goal = None;
        }
    }

    ctrl.pitch = ctrl.pitch.clamp(-1.5, 1.5);

    let (min_dist, max_dist) = camera_distance_range(&camera_target, &settings, star_r);
    ctrl.distance = ctrl.distance.clamp(min_dist, max_dist);

    *zoom_level = ZoomLevel::from_distance(ctrl.distance);

    let cam_rotation = Quat::from_euler(EulerRot::YXZ, ctrl.yaw, ctrl.pitch, 0.0);

    // ── Vaisseau : croisière puis posé au-dessus de l'astre ────────
    let ship_pos = if let Ok((mut ship_tf, mut ship_vis, mut thrust)) = ship_q.get_single_mut() {
        let hide_ship = matches!(*zoom_level, ZoomLevel::Galaxy | ZoomLevel::Cosmos | ZoomLevel::DeepSpace);
        let hover_pos = hover_position(target_pos, &camera_target, &settings, &queries.surface, &net, ctrl.distance, star_r, body_spin(&queries, &camera_target.0));
        // Caché (vue galaxie) : le vaisseau se place quand même sur son astre, sans croisière
        let levels = surface::body_params(&settings, &camera_target.0).is_some();
        fly_ship(&mut ship_tf, &mut thrust, hover_pos, target_pos, levels, hide_ship, travel.active(), time.delta_secs());
        if hide_ship {
            *ship_vis = Visibility::Hidden;
            cam_tf.translation = target_pos + cam_rotation * Vec3::new(0.0, 0.0, ctrl.distance);
            cam_tf.look_at(target_pos, Vec3::Y);
            return;
        }
        *ship_vis = Visibility::Inherited;
        let pos = ship_tf.translation;
        let d = ctrl.distance.max(1.0);
        ship_tf.scale = Vec3::splat(d * 0.008);
        pos
    } else {
        target_pos
    };

    // ── Caméra centrée sur le vaisseau ────────────────────────────
    cam_tf.translation = ship_pos + cam_rotation * Vec3::new(0.0, 0.0, ctrl.distance);
    // Autour d'une planète ou d'une lune, la caméra ne passe pas sous la surface
    if let Some(params) = surface::body_params(&settings, &camera_target.0) {
        cam_tf.translation = surface::keep_outside(cam_tf.translation, target_pos, &params);
    }
    cam_tf.look_at(ship_pos, Vec3::Y);
}


// ─────────────────────────────────────────────────────────────────────────
//  Résolution de la cible caméra
// ─────────────────────────────────────────────────────────────────────────

fn resolve_target(
    target: &CameraTarget,
    q: &TargetQueries,
    settings: &GameSettings,
) -> Vec3 {
    match target.0 {
        TargetKind::Asteroid(key) => q.asteroids.pose(&key).map(|t| t.translation).unwrap_or_default(),

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

        TargetKind::WormholeMouth(sys) =>
            q.wormholes.mouth_at(sys).unwrap_or_default(),

        TargetKind::Star(i) =>
            q.star_q
                .iter()
                .find(|(_, sid)| sid.0 == i)
                .map(|(gt, _)| gt.translation())
                // Étoile lointaine : le billboard est posé au centre du système
                .unwrap_or_else(|| settings.systems.get(i)
                    .map(|s| s.center())
                    .unwrap_or_default()),

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

        TargetKind::GalacticCore =>
            q.core_q
                .iter()
                .next()
                .map(|gt| gt.translation())
                .unwrap_or(Vec3::ZERO),

        // (le trou noir d'une galaxie lointaine n'existe qu'à l'approche : sinon, son centre)
        TargetKind::DistantGalaxyCore(id) =>
            q.dist_core_q
                .iter()
                .find(|(_, dc)| dc.galaxy_id == id)
                .map(|(gt, _)| gt.translation())
                .or_else(|| settings.galaxies.get(id as usize).map(|g| g.center()))
                .unwrap_or_default(),
    }
}


// ─────────────────────────────────────────────────────────────────────────
//  Distance caméra selon la cible
// ─────────────────────────────────────────────────────────────────────────

fn camera_distance_range(
    target: &CameraTarget,
    settings: &GameSettings,
    star_r: Option<f32>,
) -> (f32, f32) {
    match target.0 {
        TargetKind::Planet(_) | TargetKind::Moon(_, _) | TargetKind::Asteroid(_) => {
            // On peut s'approcher jusqu'à 40 du vaisseau : sous 1000, navigation autour de l'astre
            (40.0, MAX_ZOOM)
        }

        TargetKind::Star(i) => {
            let r = star_r.or_else(|| settings.stars.get(i).map(|s| s.radius)).unwrap_or(200.0);

            (r * 0.6, MAX_ZOOM)
        }

        // ── Planètes / corps ──────────────────────────────────────────

        TargetKind::GasPlanet(_) => (220.0 * 1.3, MAX_ZOOM),
        TargetKind::Comet(_) => (80.0, MAX_ZOOM),
        TargetKind::Meteoroid(_) => (30.0, MAX_ZOOM),

        // ── Étoiles ───────────────────────────────────────────────────

        TargetKind::VoxelStar(_) => (120.0 * 0.5, MAX_ZOOM),
        TargetKind::Protostar(_) => (60.0 * 1.2, MAX_ZOOM),
        TargetKind::DwarfStar(_) => (45.0 * 1.5, MAX_ZOOM),
        TargetKind::MainSequence(_) => (100.0 * 1.2, MAX_ZOOM),
        TargetKind::GiantStar(_) => (350.0 * 0.6, MAX_ZOOM),
        TargetKind::Supergiant(_) => (700.0 * 0.4, MAX_ZOOM),
        TargetKind::Hypergiant(_) => (1400.0 * 0.3, MAX_ZOOM),

        // ── Rémanents ─────────────────────────────────────────────────

        TargetKind::Nebula => (600.0 * 0.5, MAX_ZOOM),
        TargetKind::BlackHole(_) => (40.0 * 3.0, MAX_ZOOM),
        TargetKind::Pulsar(_) => (28.0 * 4.0, MAX_ZOOM),
        TargetKind::Magnetar(_) => (35.0 * 3.0, MAX_ZOOM),
        TargetKind::NeutronStar(_) => (22.0 * 4.0, MAX_ZOOM),
        TargetKind::Supernova(_) => (500.0, MAX_ZOOM),

        // Ouverture de trou de ver : le vaisseau se pose juste au-dessus (hover = moitié du minimum)
        TargetKind::WormholeMouth(_) => (5_000.0, MAX_ZOOM),
        TargetKind::GalacticCore => (50_000.0 * settings::GALAXY_SIZE_SCALE, MAX_ZOOM * 10.0),
        // Comme le trou noir principal : on peut zoomer dans la galaxie extérieure
        TargetKind::DistantGalaxyCore(_) => (50_000.0 * settings::GALAXY_SIZE_SCALE, MAX_ZOOM * 10.0),
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

#[derive(Component)]
struct SystemHudText;

#[derive(Component)]
struct ZoomHudText;


fn setup_fps_display(
    mut commands: Commands,
    settings: Res<GameSettings>,
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
            // Version et build du jeu en cours
            p.spawn((
                Text::new(format!("SpaceSpore {}", spacespore_common::installed_label())),
                TextFont { font_size: 13.0, ..default() },
                TextColor(Color::srgb(0.7, 0.7, 0.75)),
            ));
            // Graine du monde en code court, à partager (/graine la copie)
            p.spawn((
                Text::new(format!("Graine du monde : {}", planetgen::seed_code::encode(settings.world_seed))),
                TextFont { font_size: 13.0, ..default() },
                TextColor(Color::srgb(0.7, 0.7, 0.75)),
            ));

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

            p.spawn((
                Text::new("Niv. 1 Planete"),
                TextFont { font_size: 14.0, ..default() },
                TextColor(Color::srgb(0.5, 0.7, 1.0)),
                ZoomHudText,
            ));
        });

    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            left: Val::Px(0.0),
            right: Val::Px(0.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(2.0),
            ..default()
        },
    )).with_children(|p| {
        p.spawn((
            Text::new(""),
            TextFont { font_size: 18.0, ..default() },
            TextColor(Color::srgba(0.9, 0.85, 0.6, 0.9)),
            SystemHudText,
        ));
    });
}


fn update_fps_display(
    diagnostics: Res<DiagnosticsStore>,
    settings: Res<GameSettings>,
    camera_target: Res<CameraTarget>,
    queries: TargetQueries,
    cam_q: Query<&GlobalTransform, With<CameraController>>,
    star_ids: Query<&StarId, With<StarRoot>>,

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
    // ── FPS + distance ────────────────────────────────────────────────

    let dist_str = if let Ok(cam_gt) = cam_q.get_single() {
        let tp = resolve_target(&camera_target, &queries, &settings);
        let d = cam_gt.translation().distance(tp);
        format!("  Dist: {:.0}", d)
    } else {
        String::new()
    };

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
                        "FPS: {:.0}{}",
                        v, dist_str,
                    );
            }
        }
    }

    // ── Température ───────────────────────────────────────────────────

    let temp: f32 =
        match camera_target.0 {
            TargetKind::Planet(id) => {
                let si = id / 1000;
                let li = id % 1000;
                settings.systems.get(si)
                    .and_then(|s| s.planets().get(li))
                    .map(|p| p.temperature())
                    .unwrap_or(15.0)
            }

            // Une lune reçoit autant de chaleur que sa planète
            TargetKind::Moon(planet_id, _) => {
                settings.systems.get(planet_id / 1000)
                    .and_then(|s| s.planets().get(planet_id % 1000))
                    .map(|p| p.temperature())
                    .unwrap_or(-270.0)
            }

            TargetKind::Star(id) => {
                let (si, li) = star_parts(id, star_ids.iter().any(|s| s.0 == id));
                settings.systems.get(si)
                    .and_then(|s| s.stars.get(li))
                    .map(|s| s.temperature())
                    .unwrap_or(5_000.0)
            }

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

            TargetKind::GalacticCore | TargetKind::DistantGalaxyCore(_) =>
                1_000_000_000_000.0,

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


/// Les textes de l'interface n'ont pas de lettres accentuées (la police les affiche en carrés).
fn ascii(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'à' | 'â' | 'ä' => 'a',
            'î' | 'ï' => 'i',
            'ô' | 'ö' => 'o',
            'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            other => other,
        })
        .collect()
}

fn update_system_hud(
    settings: Res<GameSettings>,
    camera_target: Res<CameraTarget>,
    mut hud_q: Query<&mut Text, With<SystemHudText>>,
    net: Res<Net>,
    guilds: Res<guild::Guilds>,
    star_q: Query<&StarId, With<StarRoot>>,
    wormholes: Res<wormhole::Wormholes>,
    npcs: Res<galaxy_fx::NpcTerritories>,
    sectors: Res<planet::StarSectors>,
    asteroids: Res<asteroids::AsteroidField>,
) {
    // Étoile revendiquée : on affiche son propriétaire
    let target_sys = match target_system(&camera_target.0, &star_q) {
        Some(Some(si)) => Some(si),
        _ => None,
    };
    let mut extra = String::new();
    if let Some((sector, stars)) = target_sys.and_then(|si| sectors.sector_of(si)) {
        extra.push_str(&format!("\nSecteur {} ({} etoiles)", sector + 1, stars));
    }
    if let Some(who) = target_sys.and_then(|si| claims::claim_owner_label(si, &net, &settings, &guilds)) {
        extra.push_str(&format!("\nRevendiquee par {who}"));
    }
    if let Some(faction) = target_sys.and_then(|si| npcs.faction_of(si)) {
        extra.push_str(&format!("\nTerritoire de {} (PNJ)", faction.name));
    }
    let wormhole_line = match camera_target.0 {
        TargetKind::WormholeMouth(si) => wormholes.hud_line(si, &settings),
        TargetKind::Star(_) => target_sys.and_then(|si| wormholes.star_hint(si)),
        _ => None,
    };
    if let Some(line) = wormhole_line {
        extra.push_str(&format!("\n{line}"));
    }
    let claim_line = (!extra.is_empty()).then_some(extra);
    let (sys_idx, body_label) = match camera_target.0 {
        TargetKind::Star(id) => {
            let (si, li) = star_parts(id, star_q.iter().any(|s| s.0 == id));
            let label = settings.systems.get(si).map(|s| match s.stars.get(li) {
                Some(star) => format!("{} - {}", s.name, star.class.name()),
                None => s.name.clone(),
            });
            (Some(si), label)
        }
        TargetKind::Planet(id) => {
            let si = id / 1000;
            let li = id % 1000;
            // « Sol 3 - geante gazeuse, 0,9 g »
            let label = settings.systems.get(si).map(|s| match s.planets().get(li) {
                Some(p) if p.radius_earth > 0.0 => format!(
                    "{} {} - {}, {:.2} g",
                    s.name,
                    li + 1,
                    planetgen::system::size_class(p.kind, p.radius_earth as f64, p.hot),
                    p.gravity_g
                ),
                _ => format!("{} {}", s.name, li + 1),
            });
            (Some(si), label)
        }
        TargetKind::Moon(planet_id, moon_idx) => {
            let si = planet_id / 1000;
            let li = planet_id % 1000;
            let label = settings.systems.get(si).map(|s| {
                format!("{} {} lune {}", s.name, li + 1, moon_idx + 1)
            });
            (Some(si), label)
        }
        TargetKind::WormholeMouth(si) => (None, settings.systems.get(si).map(|s| format!("Trou de ver de {}", s.name))),
        TargetKind::Asteroid(key) => (Some(key.sys as usize), asteroids.get(&key).map(|a| a.title())),
        TargetKind::GalacticCore => (None, Some("Trou Noir Galactique".to_string())),
        TargetKind::DistantGalaxyCore(id) => (None, Some(match settings.galaxies.get(id as usize) {
            Some(g) => format!("Galaxie {} - {}", id, ascii(g.kind.name())),
            None => format!("Galaxie {}", id),
        })),
        _ => (None, None),
    };

    let label = if let (Some(si), Some(body)) = (sys_idx, &body_label) {
        if let Some(sys) = settings.systems.get(si) {
            let mut full = body.clone();
            full.push('\n');
            full.push_str(&sys.name);
            let mut planets_line = String::new();
            for (i, _) in sys.planets().iter().enumerate() {
                if !planets_line.is_empty() { planets_line.push_str("   "); }
                planets_line.push_str(&format!("{} {}", sys.name, i + 1));
            }
            if !planets_line.is_empty() {
                full.push_str(" | ");
                full.push_str(&planets_line);
            }
            full
        } else {
            String::new()
        }
    } else if let Some(body) = &body_label {
        body.clone()
    } else {
        String::new()
    };

    let label = match claim_line {
        Some(line) if !label.is_empty() => label + &line,
        Some(line) => line.trim_start().to_string(),
        None => label,
    };

    for mut text in &mut hud_q {
        **text = label.clone();
    }
}

/// Cercle blanc autour du vaisseau : la portée maximale d'un déplacement.
fn draw_travel_range(
    zoom: Res<ZoomLevel>,
    ship_q: Query<&GlobalTransform, With<Ship>>,
    mut gizmos: Gizmos<surface::IndicatorGizmos>,
) {
    if !matches!(*zoom, ZoomLevel::System | ZoomLevel::Sector | ZoomLevel::Galaxy | ZoomLevel::Cosmos) {
        return;
    }
    let Ok(ship) = ship_q.get_single() else { return };
    const SEGMENTS: usize = 128;
    let center = ship.translation();
    let point = |a: f32| center + Vec3::new(a.cos(), 0.0, a.sin()) * MAX_TRAVEL_RANGE;
    for s in 0..SEGMENTS {
        let a0 = s as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
        let a1 = (s + 1) as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
        gizmos.line(point(a0), point(a1), Color::srgba(1.0, 1.0, 1.0, 0.75));
    }
}

/// Trace derrière chaque planète : les positions récentes forment une queue qui
/// s'estompe, et montre d'où elle vient et son orbite.
fn draw_planet_trails(
    zoom: Res<ZoomLevel>,
    time: Res<Time>,
    planets: Query<(&GlobalTransform, &PlanetId)>,
    mut trails: Local<std::collections::HashMap<usize, std::collections::VecDeque<Vec3>>>,
    epoch: Res<origin::OriginEpoch>,
    mut seen_epoch: Local<u32>,
    mut gizmos: Gizmos<surface::IndicatorGizmos>,
) {
    // L'origine flottante a bougé : les anciennes positions ne sont plus dans le même repère
    if *seen_epoch != epoch.0 {
        *seen_epoch = epoch.0;
        trails.clear();
    }
    const MAX_POINTS: usize = 120;
    const STEP: f32 = 3_000.0;
    // Seulement aux zooms où l'on voit les planètes
    if !matches!(*zoom, ZoomLevel::Planet | ZoomLevel::System) || time.delta_secs() == 0.0 {
        trails.clear();
        return;
    }
    let present: Vec<usize> = planets.iter().map(|(_, id)| id.0).collect();
    trails.retain(|id, _| present.contains(id));
    for (gt, id) in &planets {
        let pos = gt.translation();
        let trail = trails.entry(id.0).or_default();
        match trail.back() {
            // Saut (système rechargé) : on repart de zéro
            Some(last) if last.distance(pos) > 400_000.0 => trail.clear(),
            Some(last) if last.distance(pos) < STEP => {}
            _ => {
                trail.push_back(pos);
                if trail.len() > MAX_POINTS {
                    trail.pop_front();
                }
            }
        }
        let n = trail.len();
        for (i, pair) in trail.iter().zip(trail.iter().skip(1)).enumerate() {
            let alpha = 0.7 * (i + 1) as f32 / n as f32;
            gizmos.line(*pair.0, *pair.1, Color::srgba(0.7, 0.85, 1.0, alpha));
        }
        // Dernier point de la queue jusqu'à la planète
        if let Some(last) = trail.back() {
            gizmos.line(*last, pos, Color::srgba(0.7, 0.85, 1.0, 0.7));
        }
    }
}

fn update_zoom_hud(
    zoom: Res<ZoomLevel>,
    cam_q: Query<&CameraController>,
    ship_q: Query<&GlobalTransform, With<Ship>>,
    mut hud_q: Query<&mut Text, With<ZoomHudText>>,
) {
    // Position du vaisseau dans l'univers
    let xyz = ship_q
        .get_single()
        .map(|gt| {
            // Position absolue dans l'univers (l'origine flottante rend la position monde sans intérêt)
            let p = settings::to_abs(gt.translation());
            format!("\nX {:.0}  Y {:.0}  Z {:.0}", p.x, p.y, p.z)
        })
        .unwrap_or_default();
    let dist = cam_q.iter().next().map(|c| c.distance).unwrap_or(0.0);
    let dist_str = if dist >= 1_000_000.0 {
        format!("{:.1}M", dist / 1_000_000.0)
    } else if dist >= 1_000.0 {
        format!("{:.0}K", dist / 1_000.0)
    } else {
        format!("{:.0}", dist)
    };
    let label = format!("Niv. {} {}  [{}]{}", zoom.level_number(), zoom.label(), dist_str, xyz);
    for mut text in &mut hud_q {
        **text = label.clone();
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  Gizmos — indicateur de lumière
// ─────────────────────────────────────────────────────────────────────────

fn draw_light_indicator(
    mut gizmos: Gizmos<surface::IndicatorGizmos>,
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
    mut gizmos: Gizmos<surface::IndicatorGizmos>,
    settings: Res<GameSettings>,
    cam_q: Query<&GlobalTransform, With<Camera3d>>,
    spatial: Res<settings::SystemSpatialIndex>,
    spawned: Res<planet::SpawnedSystems>,
    asteroids: Res<asteroids::AsteroidField>,

    planet_q:
        Query<
            (&GlobalTransform, &PlanetId),
            With<PlanetRoot>,
        >,
) {
    // ── Chunks (grille locale autour de la caméra) ────────────────────
    if settings.show_systems {
        use crate::settings::{SYSTEM_GRID_SIZE, SYSTEM_CELL_SIZE};
        let chunk_color = Color::srgba(0.2, 1.0, 0.4, 0.25);
        let star_dot = Color::srgba(1.0, 0.3, 0.2, 0.6);
        let cell = SYSTEM_CELL_SIZE;
        let half = SYSTEM_GRID_SIZE as f32 * cell / 2.0;

        let cam_pos = cam_q.single().translation();
        let view_radius = 10;
        let cx = ((cam_pos.x + half) / cell) as i32;
        let cz = ((cam_pos.z + half) / cell) as i32;
        let col_min = (cx - view_radius).max(0) as usize;
        let col_max = ((cx + view_radius) as usize).min(SYSTEM_GRID_SIZE);
        let row_min = (cz - view_radius).max(0) as usize;
        let row_max = ((cz + view_radius) as usize).min(SYSTEM_GRID_SIZE);

        let z_lo = row_min as f32 * cell - half;
        let z_hi = row_max as f32 * cell - half;
        let x_lo = col_min as f32 * cell - half;
        let x_hi = col_max as f32 * cell - half;
        for col in col_min..=col_max {
            let x = col as f32 * cell - half;
            gizmos.line(Vec3::new(x, 0.0, z_lo), Vec3::new(x, 0.0, z_hi), chunk_color);
        }
        for row in row_min..=row_max {
            let z = row as f32 * cell - half;
            gizmos.line(Vec3::new(x_lo, 0.0, z), Vec3::new(x_hi, 0.0, z), chunk_color);
        }

        let nearby = spatial.systems_in_radius(cam_pos, view_radius as f32 * cell);
        for si in nearby {
            if let Some(sys) = settings.systems.get(si) {
                let c = sys.center();
                gizmos.line(c - Vec3::Y * 500.0, c + Vec3::Y * 500.0, star_dot);
                gizmos.line(c - Vec3::X * 500.0, c + Vec3::X * 500.0, star_dot);
                gizmos.line(c - Vec3::Z * 500.0, c + Vec3::Z * 500.0, star_dot);
            }
        }
    }

    if !settings.show_orbits {
        return;
    }

    let seg = 128;

    let draw_ring =
        |gizmos: &mut Gizmos<surface::IndicatorGizmos>,
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


    // ── Planètes, étoiles, lunes, ceintures (système courant uniquement) ─
    // Les autres systèmes ne sont pas chargés : tracer leurs ~12 500 orbites
    // coûtait des millions de segments par image pour rien.

    let draw_ellipse = |gizmos: &mut Gizmos<surface::IndicatorGizmos>, elems: &OrbitalElements, color: Color, center: Vec3| {
        let mut prev = center + elems.point_at(0.0);
        for i in 1..=seg {
            let e_anom = i as f32 / seg as f32 * std::f32::consts::TAU;
            let p = center + elems.point_at(e_anom);
            gizmos.line(prev, p, color);
            prev = p;
        }
    };

    let current_sys = spawned.0.iter().next().copied();
    if let Some((si, sys)) = current_sys.and_then(|si| settings.systems.get(si).map(|s| (si, s))) {
        let sc = sys.center();

        for (pi, pcfg) in sys.planets().iter().enumerate() {
            // Une planète errante n'a pas d'orbite
            if pcfg.orbit_distance >= 1.0 && !pcfg.rogue {
                let elems = OrbitalElements {
                    a: pcfg.orbit_distance,
                    e: pcfg.eccentricity,
                    i: pcfg.inclination,
                    omega_big: pcfg.ascending_node,
                    omega: pcfg.arg_periapsis,
                    m0: 0.0,
                };
                draw_ellipse(&mut gizmos, &elems, planet_color, sc);
            }

            let id = si * 1000 + pi;
            let Some(planet_pos) = planet_q
                .iter()
                .find(|(_, pid)| pid.0 == id)
                .map(|(gt, _)| gt.translation())
            else {
                continue;
            };

            for mcfg in &pcfg.moons {
                let elems = OrbitalElements {
                    a: mcfg.orbit_distance,
                    e: mcfg.eccentricity,
                    i: mcfg.inclination,
                    omega_big: mcfg.ascending_node,
                    omega: mcfg.arg_periapsis,
                    m0: 0.0,
                };
                draw_ellipse(&mut gizmos, &elems, moon_color, planet_pos);
            }
        }

        for scfg in &sys.stars {
            if scfg.orbit_distance >= 1.0 {
                draw_ring(&mut gizmos, scfg.orbit_distance, star_color, sc);
            }
        }

        // Ceintures (C1) et orbites des comètes (C2)
        if let Some(src) = asteroids.sources(si) {
            for belt in &src.belts {
                draw_ring(&mut gizmos, belt.inner, belt_color, sc);
                draw_ring(&mut gizmos, belt.outer, belt_color, sc);
            }
            for c in &src.comets {
                let elems = OrbitalElements { a: c.a as f32, e: c.e as f32, i: c.inc as f32, omega_big: c.node as f32, omega: c.peri as f32, m0: 0.0 };
                draw_ellipse(&mut gizmos, &elems, comet_color, sc);
            }
        }
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