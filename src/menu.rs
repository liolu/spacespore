//! Menu principal : au lancement du jeu, trois boutons (Jouer, Editeur, Benchmark) devant la vraie
//! galaxie du jeu (pas une image : la galaxie principale de la graine), que la caméra contourne
//! lentement.
//!
//! - Le menu est l'état `AppState::Menu` (`editeur/mod.rs`), celui du lancement. Le jeu n'y lit ni
//!   le clavier ni la souris ; la caméra du jeu est pilotée ici (`orbit_galaxy`) et rendue telle
//!   quelle en sortant : « Jouer » redescend de la galaxie jusqu'au vaisseau (zoom automatique).
//! - « Editeur » ouvre l'éditeur de modèles, qui revient au menu en le quittant.
//! - « Benchmark » (`AppState::Benchmark`) : la caméra du jeu suit son vaisseau sans clavier ni
//!   souris, à trois distances (galaxie, système, planète) ; images par seconde moyennes, 1 % les
//!   plus lentes et pire image de chaque étape, affichées au menu et ajoutées à `benchmark.txt`.
//! - Les tests (`SPACESPORE_TEST_*`, `SPACESPORE_CAPTURE`, `SPACESPORE_EDITOR_DEMO`,
//!   `SPACESPORE_PERF`, `SPACESPORE_QUIT_SECS`) sautent le menu ; `SPACESPORE_MENU=1` le garde.

use bevy::prelude::*;

use crate::editeur::format::ModelKind;
use crate::editeur::{AppState, Editor, OpenEditor};
use crate::settings::GameSettings;
use crate::ship::Ship;
use crate::ui::TargetKind;
use crate::{CameraController, ZoomLevel};

/// Vitesse de rotation de la caméra autour de la galaxie (radians par seconde : un tour en ~5 min).
const ORBIT_SPEED: f32 = 0.02;
/// Inclinaison de la caméra au-dessus du plan de la galaxie (radians).
const ORBIT_PITCH: f32 = -0.55;

const BG_BUTTON: Color = Color::srgba(0.06, 0.06, 0.10, 0.75);
const BG_BUTTON_HOVER: Color = Color::srgba(0.3, 0.6, 1.0, 0.35);
const ACCENT: Color = Color::srgb(0.3, 0.6, 1.0);
const TEXT_COLOR: Color = Color::srgb(0.9, 0.9, 0.95);
const TEXT_DIM: Color = Color::srgb(0.7, 0.7, 0.75);

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MainMenu>()
            .init_resource::<Bench>()
            .init_resource::<HiddenUi>()
            .add_systems(Startup, skip_for_tests)
            .add_systems(Update, (orbit_galaxy, spawn_menu, buttons).chain().run_if(in_state(AppState::Menu)))
            .add_systems(OnExit(AppState::Menu), leave_menu)
            .add_systems(OnEnter(AppState::Benchmark), start_bench)
            .add_systems(Update, run_bench.run_if(in_state(AppState::Benchmark)))
            .add_systems(OnExit(AppState::Benchmark), leave_bench)
            .add_systems(PostUpdate, hide_game_ui.run_if(menu_or_bench));
    }
}

fn menu_or_bench(state: Res<State<AppState>>) -> bool {
    matches!(state.get(), AppState::Menu | AppState::Benchmark)
}

/// Les variables de test lancent directement le jeu (captures, vols automatiques…).
fn menu_skipped() -> bool {
    if std::env::var("SPACESPORE_MENU").is_ok() {
        return false;
    }
    const TESTS: [&str; 5] = ["SPACESPORE_TEST_", "SPACESPORE_CAPTURE", "SPACESPORE_EDITOR_DEMO", "SPACESPORE_PERF", "SPACESPORE_QUIT_SECS"];
    std::env::vars().any(|(k, _)| TESTS.iter().any(|t| k.starts_with(t)))
}

fn skip_for_tests(settings: Res<GameSettings>, mut next: ResMut<NextState<AppState>>, mut open: EventWriter<OpenEditor>, mut editor: ResMut<Editor>) {
    if menu_skipped() {
        next.set(AppState::Jeu);
        crate::editeur::open_on_first_launch(&settings, &mut open, &mut editor);
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  Caméra
// ─────────────────────────────────────────────────────────────────────────

/// La caméra du jeu telle qu'avant le menu ou le benchmark, rendue en sortant.
#[derive(Clone, Copy)]
struct SavedCam {
    tf: Transform,
    yaw: f32,
    pitch: f32,
    distance: f32,
    frame: Quat,
    zoom: ZoomLevel,
}

impl SavedCam {
    fn of(tf: &Transform, ctrl: &CameraController, zoom: ZoomLevel) -> Self {
        Self { tf: *tf, yaw: ctrl.yaw, pitch: ctrl.pitch, distance: ctrl.distance, frame: ctrl.frame, zoom }
    }

    fn restore(&self, tf: &mut Transform, ctrl: &mut CameraController, zoom: &mut ZoomLevel) {
        *tf = self.tf;
        ctrl.yaw = self.yaw;
        ctrl.pitch = self.pitch;
        ctrl.distance = self.distance;
        ctrl.frame = self.frame;
        ctrl.zoom_goal = None;
        *zoom = self.zoom;
    }
}

#[derive(Resource, Default)]
struct MainMenu {
    saved: Option<SavedCam>,
    angle: f32,
    /// « Jouer » : en sortant, la caméra redescend de la galaxie jusqu'au vaisseau.
    play: bool,
    /// Résultats du dernier benchmark, affichés sous les boutons.
    results: Vec<String>,
}

/// La caméra contourne lentement la galaxie principale, vue de haut, dans son plan.
fn orbit_galaxy(
    time: Res<Time>,
    settings: Res<GameSettings>,
    mut menu: ResMut<MainMenu>,
    mut zoom: ResMut<ZoomLevel>,
    mut cam_q: Query<(&mut Transform, &mut CameraController)>,
    mut ship_q: Query<&mut Visibility, With<Ship>>,
) {
    let Ok((mut tf, mut ctrl)) = cam_q.get_single_mut() else { return };
    if menu.saved.is_none() {
        menu.saved = Some(SavedCam::of(&tf, &ctrl, *zoom));
    }
    let Some(galaxy) = settings.galaxies.first() else { return };
    menu.angle += ORBIT_SPEED * time.delta_secs();
    let distance = crate::galaxy_view_distance(&TargetKind::GalacticCore, &settings);
    let center = galaxy.center();
    let rotation = galaxy.tilt * Quat::from_euler(EulerRot::YXZ, menu.angle, ORBIT_PITCH, 0.0);
    tf.translation = center + rotation * Vec3::new(0.0, 0.0, distance);
    tf.look_at(center, galaxy.tilt * Vec3::Y);
    // Le reste du jeu (étoiles, fond du ciel) se règle comme en vue galaxie
    ctrl.yaw = menu.angle;
    ctrl.pitch = ORBIT_PITCH;
    ctrl.distance = distance;
    ctrl.frame = galaxy.tilt;
    ctrl.zoom_goal = None;
    if *zoom != ZoomLevel::Galaxy {
        *zoom = ZoomLevel::Galaxy;
    }
    for mut v in &mut ship_q {
        *v = Visibility::Hidden;
    }
}

fn leave_menu(
    mut commands: Commands,
    mut menu: ResMut<MainMenu>,
    mut zoom: ResMut<ZoomLevel>,
    mut cam_q: Query<(&mut Transform, &mut CameraController)>,
    mut ship_q: Query<&mut Visibility, With<Ship>>,
    mut hidden: ResMut<HiddenUi>,
    mut nodes: Query<&mut Visibility, (With<Node>, Without<Ship>)>,
    ui: Query<Entity, With<MenuUi>>,
) {
    let play = std::mem::take(&mut menu.play);
    if let (Some(saved), Ok((mut tf, mut ctrl))) = (menu.saved.take(), cam_q.get_single_mut()) {
        if play {
            // On garde l'angle et la distance du menu : la caméra redescend jusqu'au vaisseau
            tf.translation = saved.tf.translation;
            ctrl.zoom_goal = Some(saved.distance);
        } else {
            saved.restore(&mut tf, &mut ctrl, &mut zoom);
        }
    }
    for mut v in &mut ship_q {
        *v = Visibility::Inherited;
    }
    show_game_ui(&mut commands, &mut hidden, &mut nodes, &ui);
}

// ─────────────────────────────────────────────────────────────────────────
//  Interface
// ─────────────────────────────────────────────────────────────────────────

/// Interface du menu et du benchmark (pas cachée avec celle du jeu).
#[derive(Component)]
struct MenuUi;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum MenuButton {
    Jouer,
    Editeur,
    Benchmark,
}

impl MenuButton {
    fn label(self) -> &'static str {
        match self {
            MenuButton::Jouer => "Jouer",
            MenuButton::Editeur => "Editeur",
            MenuButton::Benchmark => "Benchmark",
        }
    }
}

/// L'interface du jeu cachée pendant le menu et le benchmark, rendue en sortant.
#[derive(Resource, Default)]
struct HiddenUi(Vec<Entity>);

fn hide_game_ui(
    mut hidden: ResMut<HiddenUi>,
    mut roots: Query<(Entity, &mut Visibility), (With<Node>, Without<Parent>, Without<MenuUi>, Without<TargetCamera>)>,
) {
    for (e, mut v) in &mut roots {
        if *v != Visibility::Hidden {
            if !hidden.0.contains(&e) {
                hidden.0.push(e);
            }
            *v = Visibility::Hidden;
        }
    }
}

fn show_game_ui(
    commands: &mut Commands,
    hidden: &mut HiddenUi,
    nodes: &mut Query<&mut Visibility, (With<Node>, Without<Ship>)>,
    ui: &Query<Entity, With<MenuUi>>,
) {
    for e in std::mem::take(&mut hidden.0) {
        if let Ok(mut v) = nodes.get_mut(e) {
            *v = Visibility::Inherited;
        }
    }
    for e in ui {
        commands.entity(e).try_despawn_recursive();
    }
}

fn spawn_menu(mut commands: Commands, menu: Res<MainMenu>, existing: Query<(), With<MenuUi>>) {
    if !existing.is_empty() {
        return;
    }
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(16.0),
                ..default()
            },
            MenuUi,
        ))
        .with_children(|root| {
            root.spawn((Text::new("SPACESPORE"), TextFont { font_size: 72.0, ..default() }, TextColor(TEXT_COLOR)));
            root.spawn((
                Text::new("Voxel Universe"),
                TextFont { font_size: 20.0, ..default() },
                TextColor(TEXT_DIM),
                Node { margin: UiRect::bottom(Val::Px(28.0)), ..default() },
            ));
            for b in [MenuButton::Jouer, MenuButton::Editeur, MenuButton::Benchmark] {
                root.spawn((
                    Button,
                    Node {
                        width: Val::Px(300.0),
                        height: Val::Px(56.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(BG_BUTTON),
                    BorderColor(ACCENT),
                    BorderRadius::all(Val::Px(8.0)),
                    b,
                ))
                .with_child((Text::new(b.label()), TextFont { font_size: 26.0, ..default() }, TextColor(TEXT_COLOR)));
            }
            if !menu.results.is_empty() {
                root.spawn((
                    Text::new(menu.results.join("\n")),
                    TextFont { font_size: 15.0, ..default() },
                    TextColor(TEXT_COLOR),
                    TextLayout::new_with_justify(JustifyText::Center),
                    Node { margin: UiRect::top(Val::Px(20.0)), padding: UiRect::all(Val::Px(12.0)), ..default() },
                    BackgroundColor(BG_BUTTON),
                    BorderRadius::all(Val::Px(8.0)),
                ));
            }
        });
    // Version, en bas à gauche
    commands.spawn((
        Text::new(format!("SpaceSpore {}", spacespore_common::installed_label())),
        TextFont { font_size: 13.0, ..default() },
        TextColor(TEXT_DIM),
        Node { position_type: PositionType::Absolute, left: Val::Px(12.0), bottom: Val::Px(10.0), ..default() },
        MenuUi,
    ));
}

#[allow(clippy::too_many_arguments)]
fn buttons(
    mut q: Query<(&Interaction, &MenuButton, &mut BackgroundColor), Changed<Interaction>>,
    settings: Res<GameSettings>,
    mut menu: ResMut<MainMenu>,
    mut editor: ResMut<Editor>,
    mut open: EventWriter<OpenEditor>,
    mut next: ResMut<NextState<AppState>>,
) {
    for (interaction, button, mut bg) in &mut q {
        match interaction {
            Interaction::Hovered => bg.0 = BG_BUTTON_HOVER,
            Interaction::None => bg.0 = BG_BUTTON,
            Interaction::Pressed => match button {
                MenuButton::Jouer => {
                    menu.play = true;
                    // Premier lancement : création du personnage d'abord, puis le jeu
                    if settings.first_launch {
                        crate::editeur::open_on_first_launch(&settings, &mut open, &mut editor);
                    } else {
                        next.set(AppState::Jeu);
                    }
                }
                MenuButton::Editeur => {
                    open.send(OpenEditor(ModelKind::Personnage));
                }
                MenuButton::Benchmark => next.set(AppState::Benchmark),
            },
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  Benchmark
// ─────────────────────────────────────────────────────────────────────────

/// Étapes : nom et distance de la caméra au vaisseau.
fn stages(settings: &GameSettings) -> [(&'static str, f32); 3] {
    [
        ("Galaxie", crate::galaxy_view_distance(&TargetKind::GalacticCore, settings)),
        ("Systeme", 9_000_000.0 * crate::settings::SPACE_STRETCH),
        ("Planete", 300.0),
    ]
}
/// Secondes de mise en route d'une étape (chargements), non mesurées.
const WARMUP_SECS: f32 = 3.0;
/// Secondes mesurées par étape.
const MEASURE_SECS: f32 = 10.0;

#[derive(Resource, Default)]
struct Bench {
    saved: Option<SavedCam>,
    stage: usize,
    t: f32,
    /// Durées des images de l'étape en cours (secondes).
    frames: Vec<f32>,
    lines: Vec<String>,
}

#[derive(Component)]
struct BenchText;

fn start_bench(mut commands: Commands, mut bench: ResMut<Bench>) {
    *bench = Bench::default();
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(16.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            MenuUi,
        ))
        .with_children(|p| {
            p.spawn((
                Text::new("Benchmark"),
                TextFont { font_size: 20.0, ..default() },
                TextColor(TEXT_COLOR),
                Node { padding: UiRect::axes(Val::Px(14.0), Val::Px(6.0)), ..default() },
                BackgroundColor(BG_BUTTON),
                BorderRadius::all(Val::Px(6.0)),
                BenchText,
            ));
        });
}

/// Moyenne, 1 % les plus lentes (en images par seconde) et pire image (ms), arrondies.
fn summarize(frames: &mut [f32]) -> (u32, u32, u32) {
    if frames.is_empty() {
        return (0, 0, 0);
    }
    let total: f32 = frames.iter().sum();
    frames.sort_by(|a, b| b.total_cmp(a));
    let slow = &frames[..(frames.len() / 100).max(1)];
    let slow_avg = slow.iter().sum::<f32>() / slow.len() as f32;
    let fps = |dt: f32| if dt > 0.0 { (1.0 / dt).round() as u32 } else { 0 };
    (fps(total / frames.len() as f32), fps(slow_avg), (frames[0] * 1000.0).round() as u32)
}

#[allow(clippy::too_many_arguments)]
fn run_bench(
    real: Res<Time<Real>>,
    keys: Res<ButtonInput<KeyCode>>,
    settings: Res<GameSettings>,
    zoom: Res<ZoomLevel>,
    mut bench: ResMut<Bench>,
    mut menu: ResMut<MainMenu>,
    mut next: ResMut<NextState<AppState>>,
    mut cam_q: Query<(&Transform, &mut CameraController)>,
    mut text_q: Query<&mut Text, With<BenchText>>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        menu.results = vec!["Benchmark annule".into()];
        next.set(AppState::Menu);
        return;
    }
    let Ok((tf, mut ctrl)) = cam_q.get_single_mut() else { return };
    if bench.saved.is_none() {
        bench.saved = Some(SavedCam::of(tf, &ctrl, *zoom));
    }
    let stages = stages(&settings);
    let dt = real.delta_secs();
    let (name, distance) = stages[bench.stage];
    // La caméra tourne doucement autour du vaisseau, à la distance de l'étape
    ctrl.distance = distance;
    ctrl.zoom_goal = None;
    ctrl.pitch = -0.35;
    ctrl.yaw += 0.15 * dt;
    bench.t += dt;
    if bench.t > WARMUP_SECS {
        bench.frames.push(dt);
    }
    let left = (WARMUP_SECS + MEASURE_SECS - bench.t).max(0.0).ceil() as u32;
    for mut text in &mut text_q {
        text.0 = format!("Benchmark : {name} ({}/{}) - {left} s   |   Echap : annuler", bench.stage + 1, stages.len());
    }
    if bench.t < WARMUP_SECS + MEASURE_SECS {
        return;
    }

    let mut frames = std::mem::take(&mut bench.frames);
    let (avg, low, worst) = summarize(&mut frames);
    bench.lines.push(format!("{name} : {avg} FPS en moyenne, {low} FPS (1 % les plus lentes), pire image {worst} ms"));
    bench.stage += 1;
    bench.t = 0.0;
    if bench.stage < stages.len() {
        return;
    }

    let mut lines = vec![format!("Benchmark - SpaceSpore {}{}", spacespore_common::installed_label(), if settings.vsync { " (VSync actif : FPS limites a l'ecran)" } else { "" })];
    lines.append(&mut bench.lines);
    let path = crate::settings::data_dir().join("benchmark.txt");
    let entry = format!("{}\n{}\n\n", chrono::Local::now().format("%Y-%m-%d %H:%M"), lines.join("\n"));
    let previous = std::fs::read_to_string(&path).unwrap_or_default();
    match std::fs::write(&path, format!("{previous}{entry}")) {
        Ok(()) => lines.push(format!("Resultats ajoutes a {}", path.display())),
        Err(e) => warn!("benchmark.txt : {e}"),
    }
    info!("{}", lines.join(" | "));
    menu.results = lines;
    next.set(AppState::Menu);
}

#[allow(clippy::too_many_arguments)]
fn leave_bench(
    mut commands: Commands,
    mut bench: ResMut<Bench>,
    mut zoom: ResMut<ZoomLevel>,
    mut cam_q: Query<(&mut Transform, &mut CameraController)>,
    mut hidden: ResMut<HiddenUi>,
    mut nodes: Query<&mut Visibility, (With<Node>, Without<Ship>)>,
    ui: Query<Entity, With<MenuUi>>,
) {
    if let (Some(saved), Ok((mut tf, mut ctrl))) = (bench.saved.take(), cam_q.get_single_mut()) {
        saved.restore(&mut tf, &mut ctrl, &mut zoom);
    }
    show_game_ui(&mut commands, &mut hidden, &mut nodes, &ui);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_of_frame_times() {
        // 99 images à 10 ms et une à 50 ms : 100 images en 1,04 s
        let mut frames = vec![0.01; 99];
        frames.push(0.05);
        assert_eq!(summarize(&mut frames), (96, 20, 50));
        assert_eq!(summarize(&mut []), (0, 0, 0));
    }
}
