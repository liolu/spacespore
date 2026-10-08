//! Séquences plein écran (0.13 V2 et V4) : une interface qui recouvre le jeu et que `cinematic.wgsl`
//! dessine pixel par pixel (aucun maillage, aucune lumière du jeu).
//!
//! - `Dig` : la foreuse creuse un tunnel du sub-espace (prototype `foreuse.rs`) : très grand dézoom,
//!   grille 2D sur toute la zone, grille 3D, la zone proche tourne vers la droite en tordant l'espace
//!   comme un trou noir, zoom sur la foreuse (vise à l'avant), elle avance et le tunnel se forme derrière.
//! - `Ride` : l'intérieur d'un tunnel (cylindre, voies, traits de vitesse), piloté par `tunnel.rs`.
//! - `Galaxy` : le saut entre deux galaxies (~6 s) : la caméra du jeu traverse vraiment l'espace de la
//!   galaxie de départ à celle d'arrivée (`steer_camera`), sans rien dessiné par-dessus. Se déclenche
//!   toute seule quand le vaisseau change de galaxie (`watch_jumps`).
//!
//! Foreuse et saut : la séquence est transparente là où il n'y a que le fond, et la caméra du jeu regarde
//! dans le même sens : le fond est le vrai rendu du jeu à cet endroit (étoiles, galaxies, astres proches).
//! Échap passe la séquence (ou sort du tunnel) ; le jeu continue de tourner dessous.

use bevy::asset::load_internal_asset;
use bevy::math::DVec3;
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderRef, ShaderType};
use bevy::window::PrimaryWindow;

use crate::settings::{origin, GALAXY_SCALE};
use bevy::gizmos::config::GizmoConfigStore;
use crate::ship::Ship;

pub const CINE_SHADER: Handle<Shader> = Handle::weak_from_u128(0x0c1e_a71c_d00d_f00d_1357_9bdf_2468_ace0);

/// Durée de la séquence de creusement (s).
pub const DIG_SECS: f32 = 34.0;
/// Durée du saut entre galaxies (s) : la caméra va de la galaxie de départ à celle d'arrivée.
pub const GALAXY_SECS: f32 = 6.0;
/// Un saut du vaisseau plus long que ça (en un instant) est un changement de galaxie : 5 fois la portée d'un déplacement.
const GALAXY_JUMP_MIN: f64 = 5.0 * 750_000.0 * GALAXY_SCALE as f64;

pub struct CinematicPlugin;

impl Plugin for CinematicPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, CINE_SHADER, "cinematic.wgsl", Shader::from_wgsl);
        app.add_plugins(UiMaterialPlugin::<CineMaterial>::default())
            .init_resource::<Cinematic>()
            .add_systems(PostStartup, spawn_overlay)
            .add_systems(Update, (watch_jumps, update_overlay).chain())
            .add_systems(Update, (test_start, quit_after))
            .add_systems(PostUpdate, hide_during.before(bevy::transform::TransformSystem::TransformPropagate))
            .add_systems(Last, skip_with_escape);
    }
}

#[derive(ShaderType, Clone, Copy, Debug, Default)]
pub struct CineParams {
    pub res: Vec4,
    pub cam: Vec4,
    pub look: Vec4,
    pub drill: Vec4,
    pub fx: Vec4,
    pub fx2: Vec4,
    pub extra: Vec4,
    /// Repère du fond : directions locales de la séquence -> directions du monde (pour lire le vrai ciel).
    pub bx: Vec4,
    pub by: Vec4,
    pub bz: Vec4,
    /// x : 1 si le vrai ciel (skybox) est disponible ; y : tangente du demi-champ vertical de la caméra du jeu.
    pub flags: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct CineMaterial {
    #[uniform(0)]
    pub p: CineParams,
    /// Le vrai ciel calculé (`skybox.rs`) : fond du creusement, étoiles étirées du saut entre galaxies.
    #[texture(1, dimension = "cube")]
    #[sampler(2)]
    pub sky: Handle<Image>,
}

impl UiMaterial for CineMaterial {
    fn fragment_shader() -> ShaderRef {
        CINE_SHADER.into()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CineKind {
    Dig,
    Ride,
    Galaxy,
}

/// Ce que montre l'intérieur d'un tunnel (rempli par `tunnel.rs` à chaque image).
#[derive(Clone, Copy, Debug, Default)]
pub struct RideView {
    /// Position de la voie dans la section, de -1 (gauche) à 1 (droite).
    pub lane: f32,
    /// Vitesse, de 0 à 1 (1 = vitesse de la voie rapide).
    pub speed: f32,
    /// Distance parcourue (pour faire défiler les anneaux).
    pub s: f32,
    /// +1 on avance, -1 on recule.
    pub dir: f32,
    /// Éclair doré (péage payé).
    pub flash: f32,
}

#[derive(Resource, Default)]
pub struct Cinematic {
    pub kind: Option<CineKind>,
    pub t: f32,
    pub dur: f32,
    /// Longueur du tunnel creusé (u), pour la séquence de creusement.
    pub dig_len: f32,
    pub ride: RideView,
    /// Teintes de départ et d'arrivée et graine (saut entre galaxies).
    pub hues: (f32, f32),
    pub seed: f32,
    /// Texte du bas (vitesse, voie, reste à parcourir...).
    pub caption: String,
    /// Échap pressé pendant un `Ride` : `tunnel.rs` en sort.
    pub leave: bool,
    /// Direction du monde du tunnel creusé (`Dig`) ou du saut (`Galaxy`) : le fond est le vrai ciel vu dans ce sens.
    pub axis: Vec3,
    /// Ciel utilisé (celui du lieu de départ, gardé pendant toute la séquence).
    pub sky: Option<Handle<Image>>,
    /// Saut entre galaxies : position absolue du vaisseau au départ (la caméra fait le trajet jusqu'à
    /// sa place d'arrivée) et l'écart caméra - vaisseau gardé au départ.
    pub from: DVec3,
    cam_offset: Option<Vec3>,
}

impl Cinematic {
    pub fn active(&self) -> bool {
        self.kind.is_some()
    }

    pub fn start(&mut self, kind: CineKind, dur: f32) {
        self.kind = Some(kind);
        self.t = 0.0;
        self.dur = dur;
        self.leave = false;
        self.cam_offset = None;
        self.caption.clear();
    }

    /// Le fond de la séquence = le vrai ciel d'ici (s'il est calculé et que l'option est active), vu dans le sens `axis`.
    pub fn use_sky(&mut self, state: &crate::skybox::SkyState, settings: &crate::settings::GameSettings, axis: Vec3) {
        self.axis = axis.normalize_or(Vec3::Z);
        self.sky = (state.ready && settings.show_skybox).then(|| state.image.clone());
    }

    pub fn stop(&mut self) {
        self.kind = None;
        self.t = 0.0;
        self.leave = false;
        self.sky = None;
    }
}

#[derive(Component)]
struct CineNode;

#[derive(Component)]
struct CineText;

fn spawn_overlay(mut commands: Commands, mut mats: ResMut<Assets<CineMaterial>>, sky: Res<crate::skybox::SkyState>) {
    commands.spawn((
        Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
        MaterialNode(mats.add(CineMaterial { p: CineParams::default(), sky: sky.image.clone() })),
        GlobalZIndex(900),
        Visibility::Hidden,
        CineNode,
    ));
    commands.spawn((
        Text::new(""),
        TextFont { font_size: 18.0, ..default() },
        TextColor(Color::srgba(0.8, 0.9, 1.0, 0.9)),
        Node { position_type: PositionType::Absolute, bottom: Val::Px(26.0), left: Val::Percent(0.0), width: Val::Percent(100.0), justify_content: JustifyContent::Center, ..default() },
        GlobalZIndex(901),
        Visibility::Hidden,
        CineText,
    ));
}

fn ss(a: f32, b: f32, t: f32) -> f32 {
    let x = ((t - a) / (b - a)).clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

fn mix(a: f32, b: f32, x: f32) -> f32 {
    a + (b - a) * x
}

/// Paramètres du creusement à l'instant `t`, dans l'ordre : la foreuse seule, de près (0-4 s) ; on recule
/// en plongeant et le quadrillage s'étend depuis elle jusqu'à couvrir tout l'écran (4-11 s) ; grille 3D
/// (9-13 s) ; l'espace tourne comme un trou noir (15-21 s) ; zoom sur la foreuse (22-27 s) ; elle avance
/// et le tunnel se forme (27-34 s).
pub fn dig_params(t: f32, res: Vec2) -> CineParams {
    // Foreuse : immobile jusqu'à 27 s, puis elle accélère vers +z (le nez, avec la vise)
    let s = (t - 27.0).max(0.0);
    let z = 1.2 * s * s;
    let drill = Vec3::new(0.0, 0.0, z);
    // Distance de la caméra : de près, puis on recule pour voir le quadrillage, puis on revient
    let mut d = mix(26.0, 150.0, ss(4.0, 11.0, t));
    d = mix(d, 120.0, ss(15.0, 22.0, t));
    d = mix(d, 16.0, ss(22.0, 27.0, t));
    d = mix(d, 24.0, ss(27.0, 34.0, t));
    // Hauteur : de côté (la foreuse), vue plongeante (le plan quadrillé remplit l'écran), puis presque de côté
    let mut el = mix(0.3, 1.2, ss(4.0, 9.0, t));
    el = mix(el, 0.55, ss(13.0, 18.0, t));
    el = mix(el, 0.22, ss(22.0, 27.0, t));
    el = mix(el, 0.14, ss(27.0, 34.0, t));
    let az = 0.5 + 0.02 * t + 0.6 * ss(15.0, 22.0, t) + 0.5 * ss(22.0, 27.0, t);
    let cam = drill + d * Vec3::new(el.cos() * az.sin(), el.sin(), el.cos() * az.cos());
    let look = drill + Vec3::new(0.0, 0.0, 2.0 * ss(22.0, 27.0, t));
    let spin = mix(0.5, 3.0, ss(10.0, 22.0, t)) + 4.0 * ss(25.0, 30.0, t);
    let glow = 0.3 + 0.7 * ss(12.0, 22.0, t) + 0.6 * ss(25.0, 30.0, t);
    let grid2 = ss(4.0, 5.0, t) * (1.0 - 0.65 * ss(15.0, 22.0, t));
    // Rayon du quadrillage autour de la foreuse : il grandit jusqu'à dépasser l'écran
    let reveal = 2.0 + 3000.0 * ss(4.0, 11.0, t).powi(2);
    let grid3 = ss(9.5, 13.0, t) * (1.0 - 0.4 * ss(24.0, 28.0, t));
    let swirl = ss(15.0, 21.0, t) * (1.0 - 0.4 * ss(28.0, 34.0, t));
    let lens = 0.9 * ss(15.0, 21.0, t) * (1.0 - ss(24.0, 28.0, t));
    let fade = (1.0 - ss(0.0, 1.5, t)) + ss(32.5, 34.0, t);
    let flash = (-((t - 27.0) * 2.0).powi(2)).exp();
    CineParams {
        res: Vec4::new(res.x, res.y, t, 0.0),
        cam: cam.extend(0.0),
        look: look.extend(0.0),
        drill: drill.extend(spin),
        fx: Vec4::new(glow, grid2, grid3, swirl),
        fx2: Vec4::new(lens, fade.clamp(0.0, 1.0), z, flash),
        extra: Vec4::new(reveal, 0.0, 0.0, 0.0),
        ..default()
    }
}

pub fn ride_params(r: &RideView, t: f32, res: Vec2) -> CineParams {
    CineParams {
        res: Vec4::new(res.x, res.y, t, 1.0),
        cam: Vec4::ZERO,
        look: Vec4::new(0.0, 0.0, 1.0, 0.0),
        drill: Vec4::ZERO,
        fx: Vec4::ZERO,
        fx2: Vec4::new(0.0, 0.0, 0.0, r.flash),
        extra: Vec4::new(r.lane, r.speed, r.s, r.dir),
        ..default()
    }
}

pub fn galaxy_params(c: &Cinematic, res: Vec2) -> CineParams {
    CineParams {
        res: Vec4::new(res.x, res.y, c.t, 2.0),
        extra: Vec4::new(c.hues.0, c.hues.1, c.seed, c.dur),
        ..default()
    }
}

#[allow(clippy::too_many_arguments)]
fn update_overlay(
    time: Res<Time>,
    mut cine: ResMut<Cinematic>,
    mut mats: ResMut<Assets<CineMaterial>>,
    mut node_q: Query<(&MaterialNode<CineMaterial>, &mut Visibility), With<CineNode>>,
    mut text_q: Query<(&mut Text, &mut Visibility), (With<CineText>, Without<CineNode>)>,
    window: Query<&Window, With<PrimaryWindow>>,
    cam_q: Query<&Projection, With<Camera3d>>,
) {
    let Ok((node, mut vis)) = node_q.get_single_mut() else { return };
    let Ok((mut text, mut tvis)) = text_q.get_single_mut() else { return };
    let Some(kind) = cine.kind else {
        if *vis != Visibility::Hidden {
            *vis = Visibility::Hidden;
            *tvis = Visibility::Hidden;
        }
        return;
    };
    cine.t += time.delta_secs().min(0.1);
    if kind != CineKind::Ride && cine.t >= cine.dur {
        cine.stop();
        return;
    }
    let res = window.get_single().map_or(Vec2::new(1280.0, 720.0), |w| Vec2::new(w.physical_width() as f32, w.physical_height() as f32));
    let fov = match cam_q.get_single() {
        Ok(Projection::Perspective(p)) => p.fov,
        _ => std::f32::consts::FRAC_PI_4,
    };
    let Some(mat) = mats.get_mut(&node.0) else { return };
    let mut p = match kind {
        CineKind::Dig => dig_params(cine.t, res),
        CineKind::Ride => ride_params(&cine.ride, cine.t, res),
        CineKind::Galaxy => galaxy_params(&cine, res),
    };
    let (x, y, z) = basis(cine.axis);
    p.bx = x.extend(0.0);
    p.by = y.extend(0.0);
    p.bz = z.extend(0.0);
    p.flags = Vec4::new(if cine.sky.is_some() { 1.0 } else { 0.0 }, (fov * 0.5).tan(), 0.0, 0.0);
    mat.p = p;
    if let Some(h) = &cine.sky {
        if mat.sky != *h {
            mat.sky = h.clone();
        }
    }
    // Saut entre galaxies : aucune image par-dessus, seulement le vrai trajet de la caméra (`steer_camera`)
    *vis = if kind == CineKind::Galaxy { Visibility::Hidden } else { Visibility::Visible };
    *tvis = Visibility::Visible;
    let hint = match kind {
        CineKind::Dig => "Creusement du tunnel   -   Echap : passer".to_string(),
        CineKind::Galaxy => "Saut entre galaxies   -   Echap : passer".to_string(),
        CineKind::Ride => format!("{}   -   Echap : sortir", cine.caption),
    };
    if text.0 != hint {
        text.0 = hint;
    }
}

/// Repère de la séquence : x, y, z locaux -> directions du monde (z = l'axe du tunnel ou du saut).
fn basis(axis: Vec3) -> (Vec3, Vec3, Vec3) {
    let z = axis.normalize_or(Vec3::Z);
    let x = Vec3::Y.cross(z).normalize_or(Vec3::X);
    (x, z.cross(x), z)
}

/// Rotation de la caméra du jeu qui regarde de `cam` vers `look` (repère local de la séquence), comme le shader.
fn local_look(cam: Vec3, look: Vec3, b: (Vec3, Vec3, Vec3)) -> Quat {
    let f = (look - cam).normalize_or(Vec3::Z);
    let world_up = if f.y.abs() > 0.95 { Vec3::Z } else { Vec3::Y };
    let right = f.cross(world_up).normalize_or(Vec3::X);
    let up = right.cross(f);
    let w = |v: Vec3| b.0 * v.x + b.1 * v.y + b.2 * v.z;
    Transform::IDENTITY.looking_to(w(f), w(up)).rotation
}

/// Progression du trajet de la caméra entre les deux galaxies (0 au départ, 1 à l'arrivée) : elle part
/// doucement, file au milieu, freine à l'arrivée.
pub fn galaxy_path(u: f32) -> f32 {
    let u = u.clamp(0.0, 1.0);
    u * u * u * (u * (u * 6.0 - 15.0) + 10.0)
}

/// Pendant la foreuse et le saut, la caméra du jeu suit la séquence : le fond transparent de la séquence
/// montre le vrai rendu du jeu dans la bonne direction. Appelé à la fin de `camera_controller` (qui vient
/// de placer la caméra d'arrivée, celle qu'on rejoint en fin de saut) : tout le jeu (étoiles chargées,
/// éclaircissement, ciel) voit la caméra là où elle est vraiment.
pub fn steer_camera(cine: &mut Cinematic, cam: &mut Transform, ship: Vec3) {
    match cine.kind {
        Some(CineKind::Dig) => {
            // Seule la direction compte : le fond est à l'infini
            let p = dig_params(cine.t, Vec2::ONE);
            cam.rotation = local_look(p.cam.truncate(), p.look.truncate(), basis(cine.axis));
        }
        Some(CineKind::Galaxy) => {
            // Caméra d'arrivée et écart caméra - vaisseau gardé au départ
            let arrive = cam.translation.as_dvec3() + origin();
            let offset = *cine.cam_offset.get_or_insert(cam.translation - ship);
            let start = cine.from + offset.as_dvec3();
            let u = (cine.t / cine.dur.max(0.01)).clamp(0.0, 1.0);
            let pos = start + (arrive - start) * galaxy_path(u) as f64;
            // Le nez vers la destination pendant le trajet, la vue normale au départ et à l'arrivée
            let travel = Transform::IDENTITY.looking_to((arrive - start).as_vec3().normalize_or(Vec3::NEG_Z), Vec3::Y).rotation;
            let turn = ss(0.0, 0.2, u) * (1.0 - ss(0.8, 1.0, u));
            cam.rotation = cam.rotation.slerp(travel, turn);
            cam.translation = (pos - origin()).as_vec3();
            cine.axis = (arrive - start).as_vec3();
        }
        _ => {}
    }
}

/// Pendant la foreuse et le saut : ni vaisseau (la foreuse dessinée le remplace), ni orbites, cercles ou
/// portées, ni interface du jeu (HUD, scanner) par-dessus le vrai fond.
#[allow(clippy::type_complexity)]
fn hide_during(
    cine: Res<Cinematic>,
    mut ship_q: Query<&mut Visibility, With<Ship>>,
    mut ui_q: Query<(Entity, &mut Visibility), (With<Node>, Without<Parent>, Without<CineNode>, Without<CineText>, Without<Ship>)>,
    mut body_q: Query<
        (Entity, &mut Visibility),
        (
            Or<(With<crate::planet::PlanetRoot>, With<crate::planet::MoonRoot>, With<crate::planet::StarRoot>, With<crate::asteroids::AsteroidBody>, With<crate::asteroids::CometPart>, With<crate::galaxy_fx::GalaxyCloud>)>,
            Without<Node>,
            Without<Ship>,
        ),
    >,
    mut gizmos: ResMut<GizmoConfigStore>,
    mut hidden: Local<Vec<std::any::TypeId>>,
    mut hidden_ui: Local<Vec<(Entity, Visibility)>>,
    mut hidden_bodies: Local<bevy::utils::HashMap<Entity, Visibility>>,
) {
    let driving = matches!(cine.kind, Some(CineKind::Dig | CineKind::Galaxy));
    // Foreuse : seul le ciel d'ici (étoiles, galaxies) ; la planète, l'étoile ou la lune toutes proches, et
    // les nuages de la galaxie vus de tout près (une grande nappe crème), couvraient la moitié de l'écran
    if cine.kind == Some(CineKind::Dig) {
        for (e, mut vis) in &mut body_q {
            if *vis != Visibility::Hidden {
                hidden_bodies.entry(e).or_insert(*vis);
                *vis = Visibility::Hidden;
            }
        }
    } else {
        for (e, before) in hidden_bodies.drain() {
            if let Ok((_, mut vis)) = body_q.get_mut(e) {
                if *vis == Visibility::Hidden {
                    *vis = before;
                }
            }
        }
    }
    if driving {
        for (e, mut vis) in &mut ui_q {
            if *vis != Visibility::Hidden {
                if !hidden_ui.iter().any(|(h, _)| *h == e) {
                    hidden_ui.push((e, *vis));
                }
                *vis = Visibility::Hidden;
            }
        }
    } else {
        for (e, before) in hidden_ui.drain(..) {
            if let Ok((_, mut vis)) = ui_q.get_mut(e) {
                if *vis == Visibility::Hidden {
                    *vis = before;
                }
            }
        }
    }
    if driving && hidden.is_empty() {
        for (id, cfg, _) in gizmos.iter_mut() {
            if cfg.enabled {
                cfg.enabled = false;
                hidden.push(*id);
            }
        }
    } else if !driving && !hidden.is_empty() {
        for (id, cfg, _) in gizmos.iter_mut() {
            if hidden.contains(id) {
                cfg.enabled = true;
            }
        }
        hidden.clear();
    }
    if cine.kind == Some(CineKind::Dig) {
        for mut vis in &mut ship_q {
            *vis = Visibility::Hidden;
        }
    }
}

/// Échap passe la séquence. Il tourne après le reste (`Last`) : le menu que la même touche vient d'ouvrir est refermé.
fn skip_with_escape(keys: Res<ButtonInput<KeyCode>>, mut cine: ResMut<Cinematic>, mut menu: ResMut<crate::ui::MenuState>) {
    if !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    match cine.kind {
        Some(CineKind::Ride) => {
            cine.leave = true;
            menu.open = false;
        }
        Some(_) => {
            cine.t = cine.dur;
            menu.open = false;
        }
        None => {}
    }
}

/// Test : `SPACESPORE_TEST_CINE=dig|ride|galaxie[:t]` lance la séquence après 3 s, à l'instant `t` (captures).
fn test_start(time: Res<Time>, mut cine: ResMut<Cinematic>, mut done: Local<bool>, sky: Res<crate::skybox::SkyState>, settings: Res<crate::settings::GameSettings>) {
    if *done || time.elapsed_secs() < 3.0 {
        return;
    }
    *done = true;
    let Ok(v) = std::env::var("SPACESPORE_TEST_CINE") else { return };
    let (name, t0) = v.split_once(':').map_or((v.as_str(), 0.0), |(n, t)| (n, t.parse::<f32>().unwrap_or(0.0)));
    match name {
        "dig" => cine.start(CineKind::Dig, DIG_SECS),
        "ride" => {
            cine.start(CineKind::Ride, f32::MAX);
            cine.ride = RideView { lane: 0.0, speed: 0.6, s: 10.0, dir: 1.0, flash: 0.0 };
            cine.caption = "Voie 2/3   9 u/s   reste 40 u".into();
        }
        _ => {
            cine.start(CineKind::Galaxy, GALAXY_SECS);
            cine.hues = (0.6, 0.05);
            cine.seed = 0.3;
            // Test : un saut fictif depuis quatre portées de saut en arrière
            cine.from = origin() + DVec3::new(0.0, 0.0, GALAXY_JUMP_MIN * 4.0);
        }
    }
    cine.use_sky(&sky, &settings, Vec3::new(0.3, 0.1, -1.0));
    cine.t = t0;
}

/// Test sans capture : `SPACESPORE_QUIT_SECS=<s>` ferme le jeu à `<s>` s (on ne lit que le journal).
fn quit_after(time: Res<Time>, mut exit: EventWriter<AppExit>) {
    if let Some(s) = std::env::var("SPACESPORE_QUIT_SECS").ok().and_then(|v| v.parse::<f32>().ok()) {
        if time.elapsed_secs() > s {
            exit.send(AppExit::Success);
        }
    }
}

/// Un saut du vaisseau d'une galaxie à l'autre lance l'animation (aussi pour `/tp` et `/galaxie`).
fn watch_jumps(
    mut cine: ResMut<Cinematic>,
    sky: Res<crate::skybox::SkyState>,
    settings: Res<crate::settings::GameSettings>,
    ship_q: Query<&Transform, With<Ship>>,
    mut last: Local<Option<DVec3>>,
    mut frames: Local<u32>,
) {
    let Ok(ship) = ship_q.get_single() else { return };
    let abs = ship.translation.as_dvec3() + origin();
    *frames += 1;
    let prev = last.replace(abs);
    // Les premieres secondes : chargement de la partie, pas un voyage
    if *frames < 180 || cine.active() {
        return;
    }
    if let Some(prev) = prev {
        let jump = (abs - prev).length();
        if jump > GALAXY_JUMP_MIN {
            let h = |v: DVec3| ((v.x * 12.9898 + v.y * 78.233 + v.z * 37.719).sin() * 43758.5453).rem_euclid(1.0) as f32;
            cine.start(CineKind::Galaxy, GALAXY_SECS);
            cine.hues = (h(prev), h(abs));
            cine.seed = h(prev + abs);
            cine.from = prev;
            // Le ciel encore affiché est celui du départ : ses vraies étoiles s'étirent vers la destination
            cine.use_sky(&sky, &settings, (abs - prev).as_vec3());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dig_storyboard_follows_the_order() {
        let res = Vec2::new(1280.0, 720.0);
        let at = |t: f32| dig_params(t, res);
        // Très grand dézoom d'abord
        let d = |p: &CineParams| (p.cam - p.look).truncate().length();
        // La foreuse d'abord, seule et de près : pas encore de quadrillage
        assert!(at(2.0).fx.y < 0.01 && d(&at(2.0)) < 30.0);
        // Puis on recule et le quadrillage s'étend jusqu'à couvrir tout l'écran
        assert!(d(&at(11.0)) > 4.0 * d(&at(2.0)));
        assert!(at(7.0).fx.y > 0.5 && at(7.0).extra.x < at(11.0).extra.x && at(11.0).extra.x > 2000.0);
        // 2D avant 3D, puis le tourbillon, puis le zoom, puis l'avance
        assert!(at(7.0).fx.z < 0.01);
        assert!(at(12.0).fx.z > 0.5 && at(12.0).fx.w < 0.01);
        assert!(at(21.0).fx.w > 0.99);
        assert!(d(&at(27.0)) < 20.0);
        assert!(at(26.0).drill.z == 0.0 && at(33.0).drill.z > 20.0);
        // Écran noir au début et à la fin
        assert!(at(0.0).fx2.y > 0.99 && at(34.0).fx2.y > 0.99);
    }
}

