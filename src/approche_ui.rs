//! Aide à l'approche et cockpit (0.13 P7, P8) : trajectoire et point d'impact, couloir de descente,
//! alertes, points d'atterrissage proposés (scanner, touche L), et vitre du cockpit (F5) qui rougeoit
//! à la rentrée, givre, gouttes de pluie et de nuage.

#![allow(dead_code)]

use bevy::prelude::*;

use crate::surface::{Surface, LAND_ALT_VOXELS, LAND_MAX_SLOPE};

pub struct ApprocheUiPlugin;

impl Plugin for ApprocheUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Approach>()
            .init_resource::<ShieldHeat>()
            .add_event::<HeatDamage>()
            .add_systems(Startup, (spawn_alert, spawn_glass))
            .add_systems(Update, (scan_flats, approach_keys, shield, alerts, draw_approach, update_glass).chain().after(crate::surface::SurfaceControl));
    }
}

/// Un point d'atterrissage plat proposé.
#[derive(Clone, Debug)]
pub struct Flat {
    pub dir: Vec3,
    /// Distance (voxels), pente (degrés), nature du lieu, cap relatif au nez (degrés, + = à droite).
    pub dist: f32,
    pub slope: f32,
    pub kind: &'static str,
    pub bearing: f32,
}

#[derive(Resource, Default)]
pub struct Approach {
    pub flats: Vec<Flat>,
    pub selected: Option<usize>,
    /// Lignes du scanner (« Points d'atterrissage »).
    pub landing_text: String,
    pub alert: String,
    scan_at: f64,
    alert_at: f64,
    told_atmo: bool,
}

// ─────────────────────────────────────────────────────────────────────────
//  Points d'atterrissage (P7, Q10)
// ─────────────────────────────────────────────────────────────────────────

pub(crate) fn bearing_text(b: f32) -> &'static str {
    match b.abs() {
        a if a < 25.0 => "devant",
        a if a > 155.0 => "derriere",
        _ if b > 0.0 => if b.abs() < 70.0 { "devant a droite" } else if b.abs() < 110.0 { "a droite" } else { "derriere a droite" },
        _ => if b.abs() < 70.0 { "devant a gauche" } else if b.abs() < 110.0 { "a gauche" } else { "derriere a gauche" },
    }
}

/// Cherche les points plats autour du vaisseau (sous 25 deg, hors de l'eau), classés par distance.
fn scan_flats(time: Res<Time>, surface: Res<Surface>, mut ap: ResMut<Approach>) {
    let now = time.elapsed_secs_f64();
    let f = *surface.flight();
    let Some(t) = surface.terrain().filter(|_| f.active && surface.phase_is_flying()) else {
        if !ap.flats.is_empty() {
            ap.flats.clear();
            ap.selected = None;
            ap.landing_text.clear();
        }
        return;
    };
    if t.params.gaseous || f.alt / f.voxel.max(1e-3) > 2500.0 || now - ap.scan_at < 2.5 {
        return;
    }
    ap.scan_at = now;
    let v = t.voxel();
    let radius = t.params.radius;
    let up = f.pos.normalize_or(Vec3::Y);
    let east = crate::surface::tangent_of(f.heading, up);
    let north = up.cross(east).normalize_or(Vec3::X);
    let span = (surface.ship_dims.real_scale(v) * surface.ship_dims.icon_len * 0.6).max(3.0 * v);
    let mut flats: Vec<Flat> = Vec::new();
    for ring in 0..6 {
        let dist = 150.0 * v * 2.0f32.powi(ring);
        let ang = dist / radius;
        let mut found: Vec<Flat> = Vec::new();
        for k in 0..14 {
            let az = k as f32 * std::f32::consts::TAU / 14.0;
            let out = east * az.cos() + north * az.sin();
            let d = (up * ang.cos() + out * ang.sin()).normalize();
            let g = t.ground(d);
            if g.kind.is_liquid() {
                continue;
            }
            let slope = crate::surface::slope_deg(t, d, east, span);
            if slope >= LAND_MAX_SLOPE {
                continue;
            }
            // Nature du lieu : plage, neige, clairière, plateau ou plaine
            let elev = (g.top - radius) / t.params.terrain_height.max(1.0);
            let kind = match g.kind {
                crate::planet::VoxelType::Sand => "plage",
                crate::planet::VoxelType::Snow | crate::planet::VoxelType::Ice => "neige",
                crate::planet::VoxelType::Grass | crate::planet::VoxelType::Forest | crate::planet::VoxelType::Jungle | crate::planet::VoxelType::Taiga | crate::planet::VoxelType::Steppe | crate::planet::VoxelType::Savanna => "clairiere",
                _ if elev > 0.35 => "plateau",
                _ => "plaine",
            };
            let bearing = out.dot(f.heading.cross(up).normalize_or(Vec3::X)).atan2(out.dot(f.heading)).to_degrees();
            found.push(Flat { dir: d, dist: dist / v, slope, kind, bearing });
        }
        // Les deux plus plats de chaque anneau
        found.sort_by(|a, b| a.slope.total_cmp(&b.slope));
        flats.extend(found.into_iter().take(2));
    }
    flats.sort_by(|a, b| a.dist.total_cmp(&b.dist));
    flats.truncate(6);
    let keep = ap.selected.and_then(|i| ap.flats.get(i)).map(|s| s.dir);
    ap.selected = keep.and_then(|d| flats.iter().position(|f| f.dir.angle_between(d) < 1e-4));
    ap.flats = flats;
    ap.landing_text = if ap.flats.is_empty() {
        "Points d'atterrissage : aucun point plat a proximite".to_string()
    } else {
        let mut s = String::from("Points d'atterrissage (touche L)");
        for (i, f) in ap.flats.iter().enumerate() {
            let mark = if ap.selected == Some(i) { ">" } else { " " };
            s.push_str(&format!("\n{mark}{} {:.0} vox, {}, pente {:.0} deg", f.kind, f.dist, bearing_text(f.bearing), f.slope));
        }
        s
    };
}

/// L : désigne le point d'atterrissage suivant (le cercle vert) ; V s'y pose.
fn approach_keys(keys: Res<ButtonInput<KeyCode>>, panel: Res<crate::net_ui::NetPanel>, menu: Res<crate::ui::MenuState>, mut surface: ResMut<Surface>, mut ap: ResMut<Approach>, mut net: ResMut<crate::net::Net>, time: Res<Time>) {
    if !keys.just_pressed(KeyCode::KeyL) || panel.focus.is_some() || menu.open || !surface.phase_is_flying() {
        return;
    }
    let now = time.elapsed_secs_f64();
    if ap.flats.is_empty() {
        net.notify("Aucun point d'atterrissage plat a proximite (survolez le sol, sous 2 500 voxels).", now);
        return;
    }
    let next = ap.selected.map_or(0, |i| (i + 1) % ap.flats.len());
    ap.selected = Some(next);
    let f = ap.flats[next].clone();
    surface.set_land_hint(Some(f.dir));
    net.notify(&format!("Point d'atterrissage : {} a {:.0} voxels ({}), pente {:.0} deg. V pour s'y poser (sous {LAND_ALT_VOXELS:.0} voxels).", f.kind, f.dist, bearing_text(f.bearing), f.slope), now);
    // Le texte du scanner suit la sélection
    ap.scan_at = 0.0;
}

// ─────────────────────────────────────────────────────────────────────────
//  Bouclier thermique (option, P6)
// ─────────────────────────────────────────────────────────────────────────

/// Dégâts de coque dus à la surchauffe (PV).
#[derive(Event)]
pub struct HeatDamage(pub u8);

/// Jauge de surchauffe (0..1) : avec l'option `heat_shield`, la rentrée trop rapide la remplit ; pleine,
/// elle abîme la coque. Sans l'option : jamais de dégâts (Q8).
#[derive(Resource, Default)]
pub struct ShieldHeat {
    pub gauge: f32,
    acc: f32,
}

fn shield(time: Res<Time>, settings: Res<crate::settings::GameSettings>, surface: Res<Surface>, mut gauge: ResMut<ShieldHeat>, mut damage: EventWriter<HeatDamage>) {
    let dt = time.delta_secs().min(0.1);
    let f = *surface.flight();
    if !settings.heat_shield || !f.active {
        gauge.gauge = (gauge.gauge - 0.2 * dt).max(0.0);
        return;
    }
    let rise = if f.heat > 0.35 { f.heat * 0.28 } else { -0.12 };
    gauge.gauge = (gauge.gauge + rise * dt).clamp(0.0, 1.0);
    if gauge.gauge >= 1.0 {
        gauge.acc += 6.0 * dt;
        if gauge.acc >= 1.0 {
            let d = gauge.acc.floor();
            gauge.acc -= d;
            damage.send(HeatDamage(d as u8));
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  Alertes
// ─────────────────────────────────────────────────────────────────────────

#[derive(Component)]
struct AlertText;

fn spawn_alert(mut commands: Commands) {
    commands
        .spawn(Node { position_type: PositionType::Absolute, top: Val::Px(120.0), left: Val::Px(0.0), right: Val::Px(0.0), justify_content: JustifyContent::Center, ..default() })
        .with_children(|p| {
            p.spawn((Text::new(""), TextFont { font_size: 22.0, ..default() }, TextColor(Color::srgb(1.0, 0.35, 0.25)), TextLayout::new_with_justify(JustifyText::Center), AlertText));
        });
}

/// Alertes d'approche (P7) : trop vite avant la rentrée et avant le sol, entrée dans l'atmosphère.
fn alerts(time: Res<Time>, surface: Res<Surface>, gauge: Res<ShieldHeat>, settings: Res<crate::settings::GameSettings>, mut ap: ResMut<Approach>, mut net: ResMut<crate::net::Net>, mut text: Query<&mut Text, With<AlertText>>) {
    let now = time.elapsed_secs_f64();
    let f = *surface.flight();
    let mut alert = String::new();
    if f.active && surface.phase_is_flying() {
        let alt_v = f.alt / f.voxel.max(1e-3);
        if settings.heat_shield && gauge.gauge > 0.05 {
            alert = format!("SURCHAUFFE {:.0} % - ralentissez !", gauge.gauge * 100.0);
        } else if f.heat > 0.45 {
            alert = "TROP VITE : rentree violente - ralentissez (relachez Maj)".to_string();
        } else if alt_v < 160.0 && f.speed_vox > 420.0 && f.vert_vox < -80.0 {
            alert = "TROP VITE POUR LE SOL - ralentissez".to_string();
        } else if f.density > 0.02 && f.density < 0.2 && f.mach > 4.0 && f.vert_vox < -200.0 {
            alert = "Rentree dans l'atmosphere : vitesse elevee".to_string();
        }
        // Premier passage dans l'air à grande vitesse : message unique
        if f.density > 0.02 && f.mach > 3.0 && !ap.told_atmo {
            ap.told_atmo = true;
            net.notify("Entree dans l'atmosphere : plus on va vite, plus l'air chauffe. Ralentir (relacher Maj) pour eviter les flammes.", now);
        }
        if f.density < 0.005 {
            ap.told_atmo = false;
        }
    }
    if alert != ap.alert {
        ap.alert = alert.clone();
    }
    for mut t in &mut text {
        // Clignote
        let on = !alert.is_empty() && (now * 3.0).fract() < 0.75;
        let wanted = if on { alert.clone() } else { String::new() };
        if t.0 != wanted {
            t.0 = wanted;
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  Indicateurs dessinés : trajectoire, point d'impact, couloir, points d'atterrissage
// ─────────────────────────────────────────────────────────────────────────

fn draw_approach(mut gizmos: Gizmos, surface: Res<Surface>, ap: Res<Approach>) {
    let f = *surface.flight();
    let (Some(frame), Some(t)) = (surface.frame(), surface.terrain()) else { return };
    if !f.active || !surface.phase_is_flying() || t.params.asteroid.is_some() {
        return;
    }
    let world = |p: Vec3| frame.center + frame.rot * p;
    let up = f.pos.normalize_or(Vec3::Y);
    let len = f.length.max(1e-3);
    let real_len = surface.ship_dims.real_scale(f.voxel) * surface.ship_dims.icon_len;
    let ring = |gizmos: &mut Gizmos, at: Vec3, normal: Vec3, r: f32, color: Color| {
        gizmos.circle(Isometry3d::new(world(at), frame.rot * Quat::from_rotation_arc(Vec3::Z, normal)), r, color);
    };

    // Point au sol sous le vaisseau : ligne et cercle
    let g = f.ground_r;
    let below = up * g;
    let ground_ring = real_len.max(len * 0.35);
    gizmos.line(world(f.pos), world(below), Color::srgba(1.0, 1.0, 1.0, 0.35));
    ring(&mut gizmos, below + up * 0.1 * f.voxel, up, ground_ring, Color::srgba(1.0, 1.0, 1.0, 0.8));

    // Trajectoire prévue : le cap à la vitesse actuelle, 10 s
    let speed_h = (f.speed_vox * f.speed_vox - f.vert_vox * f.vert_vox).max(0.0).sqrt() * f.voxel;
    let vert = f.vert_vox * f.voxel;
    let heading = crate::surface::tangent_of(f.heading, up);
    let r0 = f.pos.length();
    let mut pts = Vec::with_capacity(21);
    let mut impact: Option<(f32, Vec3)> = None;
    let horizon = if vert < -1.0 { (f.alt / -vert * 1.2).clamp(1.0, 12.0) } else { 8.0 };
    for i in 0..=20 {
        let tt = horizon * i as f32 / 20.0;
        let flat = f.pos + heading * speed_h * tt;
        let dir = flat.normalize_or(up);
        let r = r0 + vert * tt;
        let ground = t.ground(dir).top;
        if r <= ground + 0.5 * f.voxel && impact.is_none() {
            impact = Some((tt, dir * ground));
            pts.push(world(dir * ground));
            break;
        }
        pts.push(world(dir * r));
    }
    let alert_color = if f.heat > 0.45 { Color::srgb(1.0, 0.3, 0.2) } else { Color::srgb(0.3, 0.9, 1.0) };
    if pts.len() > 1 {
        gizmos.linestrip(pts.clone(), alert_color);
    }
    // Couloir de descente : des portes (cercles) le long de la trajectoire, verte si la descente est douce
    if let Some((tt, hit)) = impact {
        let alt_v = f.alt / f.voxel.max(1e-3);
        let soft = f.vert_vox > -(alt_v * 1.2 + 25.0) && f.speed_vox < 450.0;
        let col = if soft { Color::srgba(0.4, 1.0, 0.5, 0.8) } else { Color::srgba(1.0, 0.4, 0.25, 0.9) };
        // Point d'impact prévu
        ring(&mut gizmos, hit.normalize_or(up) * (hit.length() + 0.1 * f.voxel), hit.normalize_or(up), real_len * 1.4, Color::srgba(1.0, 0.25, 0.2, 0.9));
        if tt < 40.0 && alt_v < 800.0 {
            for k in 1..6 {
                let u = k as f32 / 6.0;
                let tg = tt * u;
                let flat = f.pos + heading * speed_h * tg;
                let dir = flat.normalize_or(up);
                let p = dir * (r0 + vert * tg);
                let tangent = (heading * speed_h + up * vert).normalize_or(heading);
                ring(&mut gizmos, p, tangent, len * (1.2 + 1.2 * (1.0 - u)), col);
            }
        }
    }

    // Point d'atterrissage choisi : cercle vert et rayon
    if let Some(h) = surface.land_hint() {
        let at = h * (t.ground(h).top + 0.1 * f.voxel);
        ring(&mut gizmos, at, h, real_len * 2.0, Color::srgb(0.3, 1.0, 0.4));
        ring(&mut gizmos, at, h, real_len * 3.2, Color::srgba(0.3, 1.0, 0.4, 0.5));
        gizmos.line(world(at), world(at + h * (30.0 * real_len)), Color::srgba(0.3, 1.0, 0.4, 0.6));
    } else {
        // Les points proposés, en petit
        for (i, fl) in ap.flats.iter().enumerate() {
            let at = fl.dir * (t.ground(fl.dir).top + 0.1 * f.voxel);
            let c = if ap.selected == Some(i) { Color::srgb(0.3, 1.0, 0.4) } else { Color::srgba(0.7, 1.0, 0.7, 0.45) };
            ring(&mut gizmos, at, fl.dir, real_len * 1.2, c);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  Vitre du cockpit (P8)
// ─────────────────────────────────────────────────────────────────────────

#[derive(Component)]
pub(crate) struct Glass;

#[derive(Component)]
struct GlassTint;

#[derive(Component)]
struct GlassFrost;

#[derive(Component)]
struct GlassDrop {
    age: f32,
    life: f32,
}

fn spawn_glass(mut commands: Commands) {
    commands
        .spawn((Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, Visibility::Hidden, Glass, GlobalZIndex(-1)))
        .with_children(|p| {
            p.spawn((Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(Color::srgba(1.0, 0.3, 0.1, 0.0)), GlassTint));
            p.spawn((Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), border: UiRect::all(Val::Px(90.0)), ..default() }, BorderColor(Color::srgba(0.9, 0.95, 1.0, 0.0)), BackgroundColor(Color::NONE), GlassFrost));
            for i in 0..36 {
                let x = (i * 37 % 97) as f32;
                p.spawn((
                    Node { position_type: PositionType::Absolute, left: Val::Percent(x), top: Val::Percent((i * 53 % 91) as f32), width: Val::Px(8.0), height: Val::Px(11.0), ..default() },
                    BorderRadius::MAX,
                    BackgroundColor(Color::srgba(0.85, 0.92, 1.0, 0.0)),
                    GlassDrop { age: 99.0, life: 1.0 },
                ));
            }
        });
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn update_glass(
    time: Res<Time>,
    surface: Res<Surface>,
    state: Res<crate::approche_fx::FxState>,
    weather: Res<crate::weather::WeatherNow>,
    mut glass: Query<&mut Visibility, With<Glass>>,
    mut tint: Query<&mut BackgroundColor, (With<GlassTint>, Without<GlassDrop>)>,
    mut frost: Query<&mut BorderColor, With<GlassFrost>>,
    mut drops: Query<(&mut GlassDrop, &mut Node, &mut BackgroundColor), Without<GlassTint>>,
    mut seed: Local<u32>,
) {
    let dt = time.delta_secs().min(0.1);
    let show = surface.cockpit();
    for mut v in &mut glass {
        let wanted = if show { Visibility::Inherited } else { Visibility::Hidden };
        if *v != wanted {
            *v = wanted;
        }
    }
    if !show {
        return;
    }
    let f = *surface.flight();
    let plasma = surface.params().map_or(crate::approche::PLASMA_DEFAULT, |p| p.plasma);
    // Vitre qui rougeoit : la couleur du plasma, de plus en plus forte avec la chaleur
    for mut c in &mut tint {
        c.0 = Color::srgba(plasma[0], plasma[1] * 0.7, plasma[2] * 0.5, (f.heat * 0.5).min(0.5));
    }
    for mut c in &mut frost {
        c.0 = Color::srgba(0.9, 0.95, 1.0, state.frost * 0.75);
    }
    // Gouttes : pluie, grêle, nuage ; elles glissent et sèchent
    let wet = (weather.sample.precip * 1.2 + state.inside * 0.6).clamp(0.0, 1.0) * (f.speed_vox / 120.0).clamp(0.2, 1.0);
    let mut rng = |s: &mut u32| -> f32 {
        *s = s.wrapping_mul(1664525).wrapping_add(1013904223 + (time.elapsed_secs() * 1000.0) as u32);
        ((*s >> 8) & 0xFFFF) as f32 / 65535.0
    };
    for (mut d, mut node, mut bg) in &mut drops {
        d.age += dt;
        if d.age >= d.life {
            if rng(&mut seed) < wet * dt * 12.0 {
                d.age = 0.0;
                d.life = 0.8 + 1.4 * rng(&mut seed);
                node.left = Val::Percent(rng(&mut seed) * 96.0);
                node.top = Val::Percent(rng(&mut seed) * 85.0);
                let w = 5.0 + 9.0 * rng(&mut seed);
                node.width = Val::Px(w);
                node.height = Val::Px(w * 1.3);
            } else {
                bg.0 = Color::srgba(0.85, 0.92, 1.0, 0.0);
                continue;
            }
        }
        let u = d.age / d.life;
        if let Val::Percent(t) = node.top {
            node.top = Val::Percent(t + dt * 6.0 * u);
        }
        bg.0 = Color::srgba(0.85, 0.92, 1.0, 0.35 * (1.0 - u));
    }
}
