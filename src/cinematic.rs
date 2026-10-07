//! Séquences plein écran (0.13 V2 et V4) : une interface qui recouvre le jeu et que `cinematic.wgsl`
//! dessine pixel par pixel (aucun maillage, aucune lumière du jeu).
//!
//! - `Dig` : la foreuse creuse un tunnel du sub-espace (prototype `foreuse.rs`) : très grand dézoom,
//!   grille 2D sur toute la zone, grille 3D, la zone proche tourne vers la droite en tordant l'espace
//!   comme un trou noir, zoom sur la foreuse (vise à l'avant), elle avance et le tunnel se forme derrière.
//! - `Ride` : l'intérieur d'un tunnel (cylindre, voies, traits de vitesse), piloté par `tunnel.rs`.
//! - `Galaxy` : le saut entre deux galaxies (~10 s) : chute dans la galaxie de départ, tunnel d'étoiles
//!   étirées qui chauffe du bleu au rouge, iris et flash, la galaxie d'arrivée éclot. Se déclenche toute
//!   seule quand le vaisseau change de galaxie (`watch_jumps`).
//!
//! Échap passe la séquence (ou sort du tunnel) ; le jeu continue de tourner dessous.

use bevy::asset::load_internal_asset;
use bevy::math::DVec3;
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderRef, ShaderType};
use bevy::window::PrimaryWindow;

use crate::settings::{origin, GALAXY_SCALE};
use crate::ship::Ship;

pub const CINE_SHADER: Handle<Shader> = Handle::weak_from_u128(0x0c1e_a71c_d00d_f00d_1357_9bdf_2468_ace0);

/// Durée de la séquence de creusement (s).
pub const DIG_SECS: f32 = 34.0;
/// Durée du saut entre galaxies (s).
pub const GALAXY_SECS: f32 = 10.0;
/// Un saut du vaisseau plus long que ça (en un instant) est un changement de galaxie : 5 fois la portée d'un déplacement.
const GALAXY_JUMP_MIN: f64 = 5.0 * 750_000.0 * GALAXY_SCALE as f64;

pub struct CinematicPlugin;

impl Plugin for CinematicPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, CINE_SHADER, "cinematic.wgsl", Shader::from_wgsl);
        app.add_plugins(UiMaterialPlugin::<CineMaterial>::default())
            .init_resource::<Cinematic>()
            .add_systems(Startup, spawn_overlay)
            .add_systems(Update, (watch_jumps, update_overlay).chain())
            .add_systems(Update, test_start)
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
}

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug, Default)]
pub struct CineMaterial {
    #[uniform(0)]
    pub p: CineParams,
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
        self.caption.clear();
    }

    pub fn stop(&mut self) {
        self.kind = None;
        self.t = 0.0;
        self.leave = false;
    }
}

#[derive(Component)]
struct CineNode;

#[derive(Component)]
struct CineText;

fn spawn_overlay(mut commands: Commands, mut mats: ResMut<Assets<CineMaterial>>) {
    commands.spawn((
        Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
        MaterialNode(mats.add(CineMaterial::default())),
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

/// Paramètres du creusement à l'instant `t` : le scénario de l'utilisateur, étape par étape.
pub fn dig_params(t: f32, res: Vec2) -> CineParams {
    // Foreuse : immobile jusqu'à 27 s, puis elle accélère vers +z (le nez, avec la vise)
    let s = (t - 27.0).max(0.0);
    let z = 1.2 * s * s;
    let drill = Vec3::new(0.0, 0.0, z);
    // Distance de la caméra : très grand dézoom, puis on se rapproche étape par étape
    let mut d = mix(40.0, 700.0, ss(0.0, 6.0, t));
    d = mix(d, 420.0, ss(6.0, 15.0, t));
    d = mix(d, 160.0, ss(15.0, 22.0, t));
    d = mix(d, 16.0, ss(22.0, 27.0, t));
    d = mix(d, 24.0, ss(27.0, 34.0, t));
    // Hauteur et azimut : vue plongeante pour les grilles, puis presque de côté
    let mut el = mix(1.2, 1.0, ss(0.0, 6.0, t));
    el = mix(el, 0.55, ss(10.0, 18.0, t));
    el = mix(el, 0.22, ss(22.0, 27.0, t));
    el = mix(el, 0.14, ss(27.0, 34.0, t));
    let az = 0.5 + 0.02 * t + 0.6 * ss(15.0, 22.0, t) + 0.5 * ss(22.0, 27.0, t);
    let cam = drill + d * Vec3::new(el.cos() * az.sin(), el.sin(), el.cos() * az.cos());
    let look = drill + Vec3::new(0.0, 0.0, 2.0 * ss(22.0, 27.0, t));
    let spin = mix(0.5, 3.0, ss(10.0, 22.0, t)) + 4.0 * ss(25.0, 30.0, t);
    let glow = 0.3 + 0.7 * ss(12.0, 22.0, t) + 0.6 * ss(25.0, 30.0, t);
    let grid2 = ss(4.5, 8.0, t) * (1.0 - 0.65 * ss(15.0, 22.0, t));
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
        extra: Vec4::ZERO,
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
    let Some(mat) = mats.get_mut(&node.0) else { return };
    mat.p = match kind {
        CineKind::Dig => dig_params(cine.t, res),
        CineKind::Ride => ride_params(&cine.ride, cine.t, res),
        CineKind::Galaxy => galaxy_params(&cine, res),
    };
    *vis = Visibility::Visible;
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
fn test_start(time: Res<Time>, mut cine: ResMut<Cinematic>, mut done: Local<bool>) {
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
        }
    }
    cine.t = t0;
}

/// Un saut du vaisseau d'une galaxie à l'autre lance l'animation (aussi pour `/tp` et `/galaxie`).
fn watch_jumps(
    mut cine: ResMut<Cinematic>,
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
        assert!(d(&at(6.0)) > 10.0 * d(&at(0.0)));
        // 2D avant 3D, puis le tourbillon, puis le zoom, puis l'avance
        assert!(at(7.0).fx.y > 0.5 && at(7.0).fx.z < 0.01);
        assert!(at(12.0).fx.z > 0.5 && at(12.0).fx.w < 0.01);
        assert!(at(21.0).fx.w > 0.99);
        assert!(d(&at(27.0)) < 20.0);
        assert!(at(26.0).drill.z == 0.0 && at(33.0).drill.z > 20.0);
        // Écran noir au début et à la fin
        assert!(at(0.0).fx2.y > 0.99 && at(34.0).fx2.y > 0.99);
    }
}
