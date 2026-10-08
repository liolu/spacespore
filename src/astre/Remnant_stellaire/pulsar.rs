use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;
use crate::astre::{AstreLodRoot, ReloadAstre};

// ─────────────────────────────────────────────
//  Config
// ─────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct PulsarConfig {
    // --- Position & orbite ---
    pub position:               Vec3,
    pub orbit_distance:         f32,
    pub orbit_speed:            f32,

    // --- Rotation (spin-down de Larmor) ---
    /// Période initiale (secondes par tour)
    pub period_initial:         f32,
    /// Taux de freinage (s/s²) — augmente la période dans le temps
    /// Valeur réaliste : ~1e-15 à ~1e-11, pour le jeu : 0.00001 à 0.001
    pub spin_down_rate:         f32,
    /// Période maximale avant extinction du faisceau
    pub period_max:             f32,
    /// Inclinaison de l'axe magnétique / axe de rotation (rad)
    pub magnetic_tilt:          f32,

    // --- Corps stellaire ---
    pub radius:                 f32,
    pub surface_voxel_size:     f32,
    pub surface_resolution:     u32,
    pub color_equator:          [f32; 3],
    pub color_pole:             [f32; 3],
    pub surface_emissive:       f32,

    // --- Faisceaux rotatifs (lighthouse) ---
    pub beam_voxel_size:        f32,
    pub beam_samples:           u32,    // voxels par faisceau
    pub beam_length:            f32,
    pub beam_width:             f32,    // largeur angulaire (rad)
    pub beam_color_core:        [f32; 3],
    pub beam_color_edge:        [f32; 3],
    pub beam_emissive:          f32,
    /// Nombre de cônes de balayage par faisceau (épaisseur angulaire)
    pub beam_cone_rings:        u32,

    // --- Émission sphérique périodique ---
    pub pulse_enabled:          bool,
    pub pulse_voxel_size:       f32,
    pub pulse_voxel_count:      u32,
    pub pulse_color:            [f32; 3],
    pub pulse_emissive:         f32,
    pub pulse_max_radius:       f32,
    pub pulse_speed:            f32,
    /// Fraction de la période pendant laquelle le pulse est visible
    pub pulse_duty_cycle:       f32,

    // --- Toroïde magnétosphérique ---
    pub torus_enabled:          bool,
    pub torus_radius:           f32,    // grand rayon du tore
    pub torus_tube_radius:      f32,    // petit rayon du tube
    pub torus_voxel_size:       f32,
    pub torus_count:            u32,
    pub torus_color:            [f32; 3],
    pub torus_emissive:         f32,
    pub torus_rotation_speed:   f32,

    // --- Nébuleuse de vent (PWN) ---
    pub pwn_enabled:            bool,
    pub pwn_inner_radius:       f32,
    pub pwn_outer_radius:       f32,
    pub pwn_voxel_size:         f32,
    pub pwn_voxel_count:        u32,
    pub pwn_color_inner:        [f32; 3],
    pub pwn_color_outer:        [f32; 3],
    pub pwn_emissive:           f32,
    pub pwn_drift_speed:        f32,
    pub pwn_drift_amplitude:    f32,
    /// Filaments de vent (structures en Sigma dans la PWN)
    pub pwn_filament_count:     u32,
    pub pwn_filament_samples:   u32,
    pub pwn_filament_color:     [f32; 3],
    pub pwn_filament_emissive:  f32,

    // --- Anneau équatorial de choc (termination shock) ---
    pub shock_ring_enabled:     bool,
    pub shock_ring_radius:      f32,
    pub shock_ring_width:       f32,
    pub shock_ring_thickness:   f32,
    pub shock_ring_count:       u32,
    pub shock_ring_voxel_size:  f32,
    pub shock_ring_color:       [f32; 3],
    pub shock_ring_emissive:    f32,

    // --- Seed ---
    pub seed:                   u32,
}

impl Default for PulsarConfig {
    fn default() -> Self {
        Self {
            position:               Vec3::ZERO,
            orbit_distance:         0.0,
            orbit_speed:            0.0,

            period_initial:         0.033,   // 33ms — type Crab pulsar
            spin_down_rate:         0.00008,
            period_max:             8.0,
            magnetic_tilt:          0.52,    // ~30°

            radius:                 28.0,
            surface_voxel_size:     4.0,
            surface_resolution:     18,
            color_equator:          [0.2, 0.5, 0.9],
            color_pole:             [1.0, 0.9, 1.0],
            surface_emissive:       6.0,

            beam_voxel_size:        8.0,
            beam_samples:           60,
            beam_length:            1200.0,
            beam_width:             0.12,
            beam_color_core:        [1.0, 0.98, 0.9],
            beam_color_edge:        [0.3, 0.6, 1.0],
            beam_emissive:          20.0,
            beam_cone_rings:        4,

            pulse_enabled:          true,
            pulse_voxel_size:       10.0,
            pulse_voxel_count:      280,
            pulse_color:            [0.8, 0.9, 1.0],
            pulse_emissive:         8.0,
            pulse_max_radius:       700.0,
            pulse_speed:            320.0,
            pulse_duty_cycle:       0.12,

            torus_enabled:          true,
            torus_radius:           75.0,
            torus_tube_radius:      18.0,
            torus_voxel_size:       7.0,
            torus_count:            320,
            torus_color:            [0.4, 0.7, 1.0],
            torus_emissive:         4.5,
            torus_rotation_speed:   0.45,

            pwn_enabled:            true,
            pwn_inner_radius:       120.0,
            pwn_outer_radius:       550.0,
            pwn_voxel_size:         16.0,
            pwn_voxel_count:        600,
            pwn_color_inner:        [0.9, 0.7, 1.0],
            pwn_color_outer:        [0.15, 0.3, 0.7],
            pwn_emissive:           2.5,
            pwn_drift_speed:        0.18,
            pwn_drift_amplitude:    12.0,
            pwn_filament_count:     12,
            pwn_filament_samples:   30,
            pwn_filament_color:     [0.6, 0.85, 1.0],
            pwn_filament_emissive:  5.0,

            shock_ring_enabled:     true,
            shock_ring_radius:      110.0,
            shock_ring_width:       35.0,
            shock_ring_thickness:   12.0,
            shock_ring_count:       280,
            shock_ring_voxel_size:  8.0,
            shock_ring_color:       [0.7, 0.9, 1.0],
            shock_ring_emissive:    7.0,

            seed:                   31,
        }
    }
}

// ─────────────────────────────────────────────
//  Plugin
// ─────────────────────────────────────────────

pub struct PulsarPlugin;

impl Plugin for PulsarPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<PulsarRes>()
            .init_resource::<PulsarSpinState>()
            .add_event::<RegeneratePulsar>()
            .add_event::<PulseFireEvent>()
            .add_systems(Startup, spawn_pulsars)
            .add_systems(Update, (
                orbit_pulsars,
                tick_pulsar_spin,
                animate_beams,
                tick_pulse_emission,
                animate_pulse_wave,
                animate_torus,
                animate_pwn_voxels,
                animate_pwn_filaments,
                animate_shock_ring,
                regenerate_pulsars,
                reload_pulsars,
            ).chain());
    }
}

// ─────────────────────────────────────────────
//  Resource & Events
// ─────────────────────────────────────────────

#[derive(Resource)]
pub struct PulsarRes {
    pub pulsars: Vec<PulsarConfig>,
}

impl Default for PulsarRes {
    fn default() -> Self {
        Self { pulsars: vec![PulsarConfig { position: Vec3::new(2000.0, 0.0, -5000.0), ..Default::default() }] }
    }
}

#[derive(Event)]
pub struct RegeneratePulsar;

/// Émis à chaque impulsion — utile pour effets sonores, caméra, dégâts
#[derive(Event)]
pub struct PulseFireEvent {
    pub pulsar_idx: usize,
    pub origin:     Vec3,
    pub period:     f32,
}

// ─────────────────────────────────────────────
//  État dynamique du spin (Resource séparée pour partage entre systèmes)
// ─────────────────────────────────────────────

/// État de spin pour chaque pulsar — mis à jour par tick_pulsar_spin
#[derive(Resource, Default)]
pub struct PulsarSpinState {
    /// period actuelle (s) par pulsar_idx
    pub periods:      Vec<f32>,
    /// angle de rotation courant (rad) par pulsar_idx
    pub angles:       Vec<f32>,
    /// temps accumulé depuis le dernier pulse
    pub pulse_timers: Vec<f32>,
    /// true si le pulse est en cours d'expansion
    pub pulse_active: Vec<bool>,
    /// rayon courant de l'onde sphérique
    pub pulse_radii:  Vec<f32>,
}

// ─────────────────────────────────────────────
//  Composants ECS
// ─────────────────────────────────────────────

#[derive(Component)]
pub struct PulsarRoot {
    pub idx: usize,
}

/// Corps stellaire (tourne sur lui-même)
#[derive(Component)]
pub struct PulsarBody {
    pub idx: usize,
}

/// Voxel de surface
#[derive(Component)]
pub struct PulsarSurface {
    pub idx: usize,
    pub lat: f32,
    pub lon: f32,
}

/// Voxel d'un faisceau (beam) rotatif
/// +1 = faisceau nord, -1 = faisceau sud (opposé)
#[derive(Component)]
pub struct BeamVoxel {
    pub idx:        usize,
    pub pole:       f32,
    pub sample_idx: u32,
    pub ring_idx:   u32,    // anneau conique
    pub seed:       f32,
}

/// Voxel de l'onde sphérique périodique
#[derive(Component)]
pub struct PulseWaveVoxel {
    pub idx:  usize,
    pub dir:  Vec3,
    pub seed: f32,
}

/// Voxel du toroïde magnétosphérique
#[derive(Component)]
pub struct TorusVoxel {
    pub idx:         usize,
    pub big_angle:   f32,   // angle autour de l'axe Y
    pub small_angle: f32,   // angle dans le tube
    pub seed:        f32,
}

/// Voxel de la nébuleuse de vent (PWN)
#[derive(Component)]
pub struct PwnVoxel {
    pub idx:    usize,
    pub origin: Vec3,
    pub seed:   f32,
    pub color_t: f32,
}

/// Voxel de filament de vent (structures Sigma)
#[derive(Component)]
pub struct PwnFilament {
    pub idx:        usize,
    pub filament_idx: u32,
    pub sample_idx: u32,
    pub seed:       f32,
    pub lon:        f32,
    pub tilt:       f32,
}

/// Voxel de l'anneau de choc (termination shock)
#[derive(Component)]
pub struct ShockRingVoxel {
    pub idx:    usize,
    pub angle:  f32,
    pub radius: f32,
    pub height: f32,
    pub seed:   f32,
}

// ─────────────────────────────────────────────
//  Helpers
// ─────────────────────────────────────────────

fn pseudo_hash(a: f32, b: f32) -> f32 {
    ((a * 12.9898 + b * 78.233).sin() * 43758.5453).fract()
}

fn snap_grid(v: Vec3, grid: f32) -> Vec3 {
    Vec3::new(
        (v.x / grid).round() * grid,
        (v.y / grid).round() * grid,
        (v.z / grid).round() * grid,
    )
}

fn lerp_color(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

fn sphere_pos(lat: f32, lon: f32, r: f32) -> Vec3 {
    Vec3::new(
        r * lat.cos() * lon.cos(),
        r * lat.sin(),
        r * lat.cos() * lon.sin(),
    )
}

/// Direction du pôle magnétique nord en espace local,
/// compte tenu du tilt et de l'angle de spin courant.
fn magnetic_axis(cfg: &PulsarConfig, spin_angle: f32) -> Vec3 {
    let tilt = cfg.magnetic_tilt;
    Vec3::new(
        tilt.sin() * spin_angle.cos(),
        tilt.cos(),
        tilt.sin() * spin_angle.sin(),
    ).normalize()
}

/// Période courante après spin-down de Larmor :
/// P(t) = sqrt(P0² + 2 * P_dot * t)
fn current_period(cfg: &PulsarConfig, elapsed: f32) -> f32 {
    let p0  = cfg.period_initial;
    let val = p0 * p0 + 2.0 * cfg.spin_down_rate * elapsed;
    val.sqrt().min(cfg.period_max)
}

// ─────────────────────────────────────────────
//  Spawn
// ─────────────────────────────────────────────

fn spawn_pulsars(
    mut commands:  Commands,
    res:           Res<PulsarRes>,
    mut spin:      ResMut<PulsarSpinState>,
    mut meshes:    ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    spin.periods.clear();
    spin.angles.clear();
    spin.pulse_timers.clear();
    spin.pulse_active.clear();
    spin.pulse_radii.clear();

    for (idx, cfg) in res.pulsars.iter().enumerate() {
        spin.periods.push(cfg.period_initial);
        spin.angles.push(0.0);
        spin.pulse_timers.push(0.0);
        spin.pulse_active.push(false);
        spin.pulse_radii.push(0.0);

        build_pulsar(&mut commands, cfg, idx, &mut meshes, &mut materials);
    }
}

fn build_pulsar(
    commands:  &mut Commands,
    cfg:       &PulsarConfig,
    idx:       usize,
    meshes:    &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) {
    let init_pos = if cfg.orbit_distance > 1.0 {
        Vec3::new(cfg.orbit_distance, 0.0, 0.0)
    } else {
        cfg.position
    };

    let root = commands.spawn((
        Transform::from_translation(init_pos),
        Visibility::default(),
        PulsarRoot { idx },
        AstreLodRoot { cull_dist: 5000.0, radius: cfg.radius, streamable: true, label: "Pulsar" },
    )).id();

    // ── Corps stellaire ────────────────────────────────────────────────────
    let body = commands.spawn((
        Transform::IDENTITY,
        Visibility::default(),
        PulsarBody { idx },
    )).id();
    commands.entity(root).add_child(body);

    let res_s    = cfg.surface_resolution;
    let vstep    = std::f32::consts::PI / res_s as f32;
    let hstep    = std::f32::consts::TAU / (res_s * 2) as f32;
    let surf_mesh = meshes.add(Mesh::from(Cuboid::new(
        cfg.surface_voxel_size, cfg.surface_voxel_size, cfg.surface_voxel_size,
    )));

    let mut lat = -std::f32::consts::FRAC_PI_2;
    while lat <= std::f32::consts::FRAC_PI_2 {
        let mut lon = 0.0_f32;
        while lon < std::f32::consts::TAU {
            let lat_norm = (lat + std::f32::consts::FRAC_PI_2) / std::f32::consts::PI;
            let pole_t   = (lat_norm * 2.0 - 1.0).abs().powf(1.5);
            let [sr, sg, sb] = lerp_color(cfg.color_equator, cfg.color_pole, pole_t);
            let se = cfg.surface_emissive;

            let surf_mat = materials.add(StandardMaterial {
                base_color: Color::srgb(sr, sg, sb),
                emissive:   LinearRgba::new(sr * se, sg * se, sb * se, 1.0),
                unlit:      true,
                ..default()
            });
            let pos = sphere_pos(lat, lon, cfg.radius);

            let v = commands.spawn((
                Mesh3d(surf_mesh.clone()),
                MeshMaterial3d(surf_mat),
                Transform::from_translation(snap_grid(pos, cfg.surface_voxel_size)),
                NotShadowCaster,
                PulsarSurface { idx, lat, lon },
            )).id();
            commands.entity(body).add_child(v);

            lon += hstep;
        }
        lat += vstep;
    }

    // ── Faisceaux rotatifs (2 pôles × cone_rings × samples) ───────────────
    let beam_mesh = meshes.add(Mesh::from(Cuboid::new(
        cfg.beam_voxel_size, cfg.beam_voxel_size, cfg.beam_voxel_size,
    )));

    for pole in [1.0_f32, -1.0_f32] {
        for ring in 0..cfg.beam_cone_rings {
            // ring_t : 0 = axe central, 1 = bord du cône
            let ring_t = ring as f32 / cfg.beam_cone_rings.max(1) as f32;
            let [br, bg, bb] = lerp_color(cfg.beam_color_core, cfg.beam_color_edge, ring_t);
            let be = cfg.beam_emissive * (1.0 - ring_t * 0.75);

            let beam_mat = materials.add(StandardMaterial {
                base_color: Color::srgb(br, bg, bb),
                emissive:   LinearRgba::new(br * be, bg * be, bb * be, 1.0),
                alpha_mode: AlphaMode::Add,
                unlit:      true,
                ..default()
            });

            for si in 0..cfg.beam_samples {
                let seed = pseudo_hash(
                    cfg.seed as f32 + pole * 50.0 + ring as f32 * 7.3,
                    si as f32,
                );
                let bv = commands.spawn((
                    Mesh3d(beam_mesh.clone()),
                    MeshMaterial3d(beam_mat.clone()),
                    Transform::from_translation(Vec3::ZERO),
                    Visibility::default(),
                    NotShadowCaster,
                    BeamVoxel { idx, pole, sample_idx: si, ring_idx: ring, seed },
                )).id();
                commands.entity(root).add_child(bv);
            }
        }
    }

    // ── Onde sphérique périodique ──────────────────────────────────────────
    if cfg.pulse_enabled {
        let [pr, pg, pb] = cfg.pulse_color;
        let pe = cfg.pulse_emissive;
        let pulse_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(pr, pg, pb),
            emissive:   LinearRgba::new(pr * pe, pg * pe, pb * pe, 1.0),
            alpha_mode: AlphaMode::Add,
            unlit:      true,
            ..default()
        });
        let pulse_mesh = meshes.add(Mesh::from(Cuboid::new(
            cfg.pulse_voxel_size, cfg.pulse_voxel_size, cfg.pulse_voxel_size,
        )));

        for pi in 0..cfg.pulse_voxel_count {
            let h1 = pseudo_hash(cfg.seed as f32 + 100.0, pi as f32);
            let h2 = pseudo_hash(cfg.seed as f32 + 101.0, pi as f32);
            let theta = h1 * std::f32::consts::TAU;
            let phi   = (h2 * 2.0 - 1.0).clamp(-1.0, 1.0).acos();
            let dir   = Vec3::new(phi.sin() * theta.cos(), phi.cos(), phi.sin() * theta.sin());

            let pv = commands.spawn((
                Mesh3d(pulse_mesh.clone()),
                MeshMaterial3d(pulse_mat.clone()),
                Transform::from_translation(Vec3::ZERO).with_scale(Vec3::ZERO),
                Visibility::Hidden,
                NotShadowCaster,
                PulseWaveVoxel { idx, dir: dir.normalize(), seed: pseudo_hash(h1, h2) },
            )).id();
            commands.entity(root).add_child(pv);
        }
    }

    // ── Toroïde magnétosphérique ───────────────────────────────────────────
    if cfg.torus_enabled {
        let [tr, tg, tb] = cfg.torus_color;
        let te = cfg.torus_emissive;
        let torus_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(tr, tg, tb),
            emissive:   LinearRgba::new(tr * te, tg * te, tb * te, 1.0),
            alpha_mode: AlphaMode::Add,
            unlit:      true,
            ..default()
        });
        let torus_mesh = meshes.add(Mesh::from(Cuboid::new(
            cfg.torus_voxel_size, cfg.torus_voxel_size, cfg.torus_voxel_size,
        )));

        for ti in 0..cfg.torus_count {
            let h1 = pseudo_hash(cfg.seed as f32 + 200.0, ti as f32);
            let h2 = pseudo_hash(cfg.seed as f32 + 201.0, ti as f32);
            let big   = h1 * std::f32::consts::TAU;
            let small = h2 * std::f32::consts::TAU;
            let seed  = pseudo_hash(h1, h2);

            // Position sur le tore : point de départ — sera recalculée
            let x = (cfg.torus_radius + cfg.torus_tube_radius * small.cos()) * big.cos();
            let y = cfg.torus_tube_radius * small.sin();
            let z = (cfg.torus_radius + cfg.torus_tube_radius * small.cos()) * big.sin();

            let tv = commands.spawn((
                Mesh3d(torus_mesh.clone()),
                MeshMaterial3d(torus_mat.clone()),
                Transform::from_translation(snap_grid(Vec3::new(x, y, z), cfg.torus_voxel_size)),
                Visibility::default(),
                NotShadowCaster,
                TorusVoxel { idx, big_angle: big, small_angle: small, seed },
            )).id();
            commands.entity(root).add_child(tv);
        }
    }

    // ── PWN — nuage de vent ────────────────────────────────────────────────
    if cfg.pwn_enabled {
        let pwn_mesh = meshes.add(Mesh::from(Cuboid::new(
            cfg.pwn_voxel_size, cfg.pwn_voxel_size, cfg.pwn_voxel_size,
        )));

        for pi in 0..cfg.pwn_voxel_count {
            let h1 = pseudo_hash(cfg.seed as f32 + 300.0, pi as f32);
            let h2 = pseudo_hash(cfg.seed as f32 + 301.0, pi as f32);
            let h3 = pseudo_hash(cfg.seed as f32 + 302.0, pi as f32);
            let h4 = pseudo_hash(cfg.seed as f32 + 303.0, pi as f32);

            let theta = h1 * std::f32::consts::TAU;
            let phi   = (h2 * 2.0 - 1.0).clamp(-1.0, 1.0).acos();
            let r     = cfg.pwn_inner_radius
                + h3.powf(0.55) * (cfg.pwn_outer_radius - cfg.pwn_inner_radius);

            let color_t = ((r - cfg.pwn_inner_radius)
                / (cfg.pwn_outer_radius - cfg.pwn_inner_radius)).clamp(0.0, 1.0);
            let [wr, wg, wb] = lerp_color(cfg.pwn_color_inner, cfg.pwn_color_outer, color_t);
            let we = cfg.pwn_emissive * (1.0 - color_t * 0.6);

            let pwn_mat = materials.add(StandardMaterial {
                base_color: Color::srgb(wr, wg, wb),
                emissive:   LinearRgba::new(wr * we, wg * we, wb * we, 1.0),
                alpha_mode: AlphaMode::Add,
                unlit:      true,
                ..default()
            });

            let origin = Vec3::new(
                r * phi.sin() * theta.cos(),
                r * phi.cos(),
                r * phi.sin() * theta.sin(),
            );
            let snapped = snap_grid(origin, cfg.pwn_voxel_size);

            let pv = commands.spawn((
                Mesh3d(pwn_mesh.clone()),
                MeshMaterial3d(pwn_mat),
                Transform::from_translation(snapped),
                Visibility::default(),
                NotShadowCaster,
                PwnVoxel { idx, origin: snapped, seed: h4, color_t },
            )).id();
            commands.entity(root).add_child(pv);
        }

        // Filaments de vent (Sigma structures — arcs magnétiques dans la PWN)
        let fil_mesh = meshes.add(Mesh::from(Cuboid::new(
            cfg.pwn_voxel_size * 0.5,
            cfg.pwn_voxel_size * 0.5,
            cfg.pwn_voxel_size * 0.5,
        )));
        let [fr, fg, fb] = cfg.pwn_filament_color;
        let fe = cfg.pwn_filament_emissive;
        let fil_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(fr, fg, fb),
            emissive:   LinearRgba::new(fr * fe, fg * fe, fb * fe, 1.0),
            alpha_mode: AlphaMode::Add,
            unlit:      true,
            ..default()
        });

        for fi in 0..cfg.pwn_filament_count {
            let lon = fi as f32 / cfg.pwn_filament_count as f32 * std::f32::consts::TAU;
            let tilt = std::f32::consts::FRAC_PI_2 * 0.3
                + pseudo_hash(cfg.seed as f32 + fi as f32, 9.9) * std::f32::consts::FRAC_PI_2 * 0.5;

            for si in 0..cfg.pwn_filament_samples {
                let seed = pseudo_hash(fi as f32, si as f32 * 2.7);
                let fv = commands.spawn((
                    Mesh3d(fil_mesh.clone()),
                    MeshMaterial3d(fil_mat.clone()),
                    Transform::from_translation(Vec3::ZERO),
                    Visibility::default(),
                    NotShadowCaster,
                    PwnFilament { idx, filament_idx: fi, sample_idx: si, seed, lon, tilt },
                )).id();
                commands.entity(root).add_child(fv);
            }
        }
    }

    // ── Anneau de choc (termination shock) ────────────────────────────────
    if cfg.shock_ring_enabled {
        let [srr, srg, srb] = cfg.shock_ring_color;
        let sre = cfg.shock_ring_emissive;
        let shock_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(srr, srg, srb),
            emissive:   LinearRgba::new(srr * sre, srg * sre, srb * sre, 1.0),
            alpha_mode: AlphaMode::Add,
            unlit:      true,
            ..default()
        });
        let shock_mesh = meshes.add(Mesh::from(Cuboid::new(
            cfg.shock_ring_voxel_size,
            cfg.shock_ring_voxel_size * 0.4,
            cfg.shock_ring_voxel_size,
        )));

        for si in 0..cfg.shock_ring_count {
            let h1 = pseudo_hash(cfg.seed as f32 + 400.0, si as f32);
            let h2 = pseudo_hash(cfg.seed as f32 + 401.0, si as f32);
            let h3 = pseudo_hash(cfg.seed as f32 + 402.0, si as f32);

            let angle  = h1 * std::f32::consts::TAU;
            let radius = cfg.shock_ring_radius + (h2 * 2.0 - 1.0) * cfg.shock_ring_width * 0.5;
            let height = (h3 * 2.0 - 1.0) * cfg.shock_ring_thickness * 0.5;
            let seed   = pseudo_hash(h1, h3);

            let pos = Vec3::new(angle.cos() * radius, height, angle.sin() * radius);

            let sv = commands.spawn((
                Mesh3d(shock_mesh.clone()),
                MeshMaterial3d(shock_mat.clone()),
                Transform::from_translation(snap_grid(pos, cfg.shock_ring_voxel_size)),
                Visibility::default(),
                NotShadowCaster,
                ShockRingVoxel { idx, angle, radius, height, seed },
            )).id();
            commands.entity(root).add_child(sv);
        }
    }
}

// ─────────────────────────────────────────────
//  Orbite
// ─────────────────────────────────────────────

fn orbit_pulsars(
    time:       Res<Time>,
    res:        Res<PulsarRes>,
    mut root_q: Query<(&mut Transform, &PulsarRoot)>,
) {
    let t = time.elapsed_secs();
    for (mut tf, root) in &mut root_q {
        let Some(cfg) = res.pulsars.get(root.idx) else { continue; };
        if cfg.orbit_distance > 1.0 {
            let a = t * cfg.orbit_speed;
            tf.translation.x = a.cos() * cfg.orbit_distance;
            tf.translation.z = a.sin() * cfg.orbit_distance;
        }
    }
}

// ─────────────────────────────────────────────
//  Spin-down de Larmor + rotation du corps
// ─────────────────────────────────────────────

fn tick_pulsar_spin(
    time:       Res<Time>,
    res:        Res<PulsarRes>,
    mut spin:   ResMut<PulsarSpinState>,
    mut body_q: Query<(&mut Transform, &PulsarBody)>,
    mut pulse_events: EventWriter<PulseFireEvent>,
    root_q:     Query<(&GlobalTransform, &PulsarRoot)>,
) {
    let dt      = time.delta_secs();
    let elapsed = time.elapsed_secs();

    // Assure la taille des vecteurs
    for idx in 0..res.pulsars.len() {
        while spin.periods.len()      <= idx { spin.periods.push(res.pulsars[idx].period_initial); }
        while spin.angles.len()       <= idx { spin.angles.push(0.0); }
        while spin.pulse_timers.len() <= idx { spin.pulse_timers.push(0.0); }
        while spin.pulse_active.len() <= idx { spin.pulse_active.push(false); }
        while spin.pulse_radii.len()  <= idx { spin.pulse_radii.push(0.0); }
    }

    for (idx, cfg) in res.pulsars.iter().enumerate() {
        // Période courante après spin-down
        let period = current_period(cfg, elapsed);
        spin.periods[idx] = period;

        // Vitesse angulaire = TAU / P
        let omega = std::f32::consts::TAU / period.max(0.001);
        spin.angles[idx] = (spin.angles[idx] + omega * dt) % std::f32::consts::TAU;

        // Timer du pulse sphérique
        spin.pulse_timers[idx] += dt;
        if spin.pulse_timers[idx] >= period {
            spin.pulse_timers[idx] = 0.0;
            spin.pulse_active[idx] = true;
            spin.pulse_radii[idx]  = cfg.radius;

            let origin = root_q
                .iter()
                .find(|(_, r)| r.idx == idx)
                .map(|(gt, _)| gt.translation())
                .unwrap_or_default();

            pulse_events.send(PulseFireEvent { pulsar_idx: idx, origin, period });
        }

        // Expansion du pulse
        if spin.pulse_active[idx] {
            spin.pulse_radii[idx] += cfg.pulse_speed * dt;
            if spin.pulse_radii[idx] > cfg.pulse_max_radius {
                spin.pulse_active[idx] = false;
                spin.pulse_radii[idx]  = 0.0;
            }
        }
    }

    // Rotation du corps
    for (mut tf, body) in &mut body_q {
        let Some(_cfg) = res.pulsars.get(body.idx) else { continue; };
        if spin.angles.len() <= body.idx { continue; }
        let angle = spin.angles[body.idx];
        // Rotation axiale pure — très rapide
        tf.rotation = Quat::from_rotation_y(angle);
    }
}

// ─────────────────────────────────────────────
//  Animation faisceaux
// ─────────────────────────────────────────────

fn animate_beams(
    res:        Res<PulsarRes>,
    spin:       Res<PulsarSpinState>,
    mut beam_q: Query<(&BeamVoxel, &mut Transform, &mut Visibility)>,
) {
    for (bv, mut tf, mut vis) in &mut beam_q {
        let Some(cfg) = res.pulsars.get(bv.idx) else {
            *vis = Visibility::Hidden;
            continue;
        };
        if spin.angles.len() <= bv.idx { continue; }

        let spin_angle = spin.angles[bv.idx];
        // Axe magnétique courant
        let mag_axis = magnetic_axis(cfg, spin_angle);

        // Direction du faisceau : pôle + décalage conique
        let pole_dir = mag_axis * bv.pole;

        // Perpendiculaire à l'axe pour le cône
        let perp_base = if pole_dir.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
        let perp_a    = pole_dir.cross(perp_base).normalize();
        let perp_b    = pole_dir.cross(perp_a).normalize();

        // ring_t : 0 = axe, 1 = bord
        let ring_t        = bv.ring_idx as f32 / cfg.beam_cone_rings.max(1) as f32;
        let cone_angle    = ring_t * cfg.beam_width;
        let cone_phi      = bv.seed * std::f32::consts::TAU; // azimut dans le cône

        let beam_dir = (pole_dir
            + perp_a * cone_angle * cone_phi.cos()
            + perp_b * cone_angle * cone_phi.sin())
            .normalize();

        // Position le long du faisceau
        let sample_t = bv.sample_idx as f32 / cfg.beam_samples as f32;
        let dist     = cfg.radius + sample_t * cfg.beam_length;

        // Scintillement haute fréquence (interférence synchrotron)
        let flicker_seed = bv.seed * 17.3 + bv.sample_idx as f32;
        let _period = spin.periods.get(bv.idx).copied().unwrap_or(1.0);
        let flicker = (spin_angle * 8.0 + flicker_seed).sin() * 0.25 + 0.75;

        // Fade exponentiel avec la distance
        let fade = ((1.0 - sample_t).powf(0.6) * flicker).max(0.0);
        if fade < 0.02 {
            *vis = Visibility::Hidden;
            continue;
        }

        let pos = beam_dir * dist;
        tf.translation = snap_grid(pos, cfg.beam_voxel_size);
        tf.scale = Vec3::splat(fade * (1.0 - ring_t * 0.5));
        *vis = Visibility::Visible;
    }
}

// ─────────────────────────────────────────────
//  Tick pulse (déclenché par spin)
// ─────────────────────────────────────────────

fn tick_pulse_emission(
    res:        Res<PulsarRes>,
    spin:       Res<PulsarSpinState>,
    mut wave_q: Query<(&PulseWaveVoxel, &mut Transform, &mut Visibility)>,
) {
    for (pw, mut tf, mut vis) in &mut wave_q {
        let Some(cfg) = res.pulsars.get(pw.idx) else {
            *vis = Visibility::Hidden;
            continue;
        };
        if !cfg.pulse_enabled
            || spin.pulse_active.len() <= pw.idx
            || !spin.pulse_active[pw.idx]
        {
            *vis = Visibility::Hidden;
            continue;
        }

        let radius = spin.pulse_radii[pw.idx];
        let pos    = pw.dir * radius;
        tf.translation = snap_grid(pos, cfg.pulse_voxel_size);

        // Épaisseur et fade
        let life  = 1.0 - (radius / cfg.pulse_max_radius).clamp(0.0, 1.0);
        let scale = life * (0.5 + pw.seed * 0.8);
        tf.scale  = Vec3::splat(scale.max(0.05));
        *vis = Visibility::Visible;
    }
}

fn animate_pulse_wave(
    _res:   Res<PulsarRes>,
    _spin:  Res<PulsarSpinState>,
    // (déjà géré dans tick_pulse_emission — conservé pour extensibilité)
) {}

// ─────────────────────────────────────────────
//  Animation toroïde
// ─────────────────────────────────────────────

fn animate_torus(
    time:       Res<Time>,
    res:        Res<PulsarRes>,
    spin:       Res<PulsarSpinState>,
    mut tor_q:  Query<(&TorusVoxel, &mut Transform)>,
) {
    let t = time.elapsed_secs();

    for (tv, mut tf) in &mut tor_q {
        let Some(cfg) = res.pulsars.get(tv.idx) else { continue; };

        let spin_angle = spin.angles.get(tv.idx).copied().unwrap_or(0.0);
        let mag        = magnetic_axis(cfg, spin_angle);

        // Le tore est dans le plan perpendiculaire à l'axe magnétique
        // Rotation du tore propre
        let big_anim   = tv.big_angle + t * cfg.torus_rotation_speed;
        let small_anim = tv.small_angle + t * cfg.torus_rotation_speed * 3.2;

        // Position dans le repère du tore
        let r_tube = cfg.torus_tube_radius;
        let r_big  = cfg.torus_radius;

        let local_x = (r_big + r_tube * small_anim.cos()) * big_anim.cos();
        let local_y = r_tube * small_anim.sin();
        let local_z = (r_big + r_tube * small_anim.cos()) * big_anim.sin();
        let local   = Vec3::new(local_x, local_y, local_z);

        // Align le plan du tore avec le plan perpendiculaire à l'axe mag
        let up    = if mag.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
        let right = mag.cross(up).normalize();
        let fwd   = mag.cross(right).normalize();

        let world = right * local.x + mag * local.y + fwd * local.z;

        tf.translation = snap_grid(world, cfg.torus_voxel_size);

        // Shimmer
        let shimmer = 0.7 + (t * 2.0 + tv.seed * 6.28).sin() * 0.3;
        tf.scale = Vec3::splat(shimmer.max(0.05));
    }
}

// ─────────────────────────────────────────────
//  Animation PWN
// ─────────────────────────────────────────────

fn animate_pwn_voxels(
    time:      Res<Time>,
    res:       Res<PulsarRes>,
    mut pwn_q: Query<(&PwnVoxel, &mut Transform)>,
) {
    let t = time.elapsed_secs();

    for (pv, mut tf) in &mut pwn_q {
        let Some(cfg) = res.pulsars.get(pv.idx) else { continue; };

        let speed = cfg.pwn_drift_speed;
        let amp   = cfg.pwn_drift_amplitude;
        let s     = pv.seed;

        let drift = Vec3::new(
            (t * speed + s * 11.1).sin() * amp,
            (t * speed * 0.7 + s * 7.3).cos() * amp * 0.5,
            (t * speed * 0.9 + s * 5.7).sin() * amp * 0.8,
        );

        tf.translation = snap_grid(pv.origin + drift, cfg.pwn_voxel_size);

        // Pulsation liée à la distance (intérieur plus actif)
        let pulse = 1.0 + (t * (1.5 + pv.color_t) + s * std::f32::consts::TAU).sin()
            * 0.2 * (1.0 - pv.color_t * 0.6);
        tf.scale = Vec3::splat(pulse.max(0.05));
    }
}

fn animate_pwn_filaments(
    time:       Res<Time>,
    res:        Res<PulsarRes>,
    spin:       Res<PulsarSpinState>,
    mut fil_q:  Query<(&PwnFilament, &mut Transform, &mut Visibility)>,
) {
    let t = time.elapsed_secs();

    for (fv, mut tf, mut vis) in &mut fil_q {
        let Some(cfg) = res.pulsars.get(fv.idx) else {
            *vis = Visibility::Hidden;
            continue;
        };

        let spin_angle = spin.angles.get(fv.idx).copied().unwrap_or(0.0);

        // Les filaments tournent lentement avec le vent du pulsar
        let lon_anim   = fv.lon + spin_angle * 0.05
            + t * 0.04;

        let sample_t = fv.sample_idx as f32 / cfg.pwn_filament_samples.max(1) as f32;

        // Arc dipolaire dans la PWN (même équation que magnétar)
        let r_min = cfg.pwn_inner_radius;
        let r_max = cfg.pwn_outer_radius * 0.7;

        let lat_angle = (sample_t * std::f32::consts::PI - std::f32::consts::FRAC_PI_2)
            * fv.tilt / std::f32::consts::FRAC_PI_2;
        let cos_tilt = fv.tilt.cos().max(0.01);
        let r = (r_max * lat_angle.cos().powi(2) / (cos_tilt * cos_tilt))
            .clamp(r_min, r_max);

        let wave = (t * 0.3 + fv.seed * 9.0 + sample_t * 4.0).sin() * r * 0.04;
        let pos  = Vec3::new(
            (r + wave) * lat_angle.cos() * lon_anim.cos(),
            (r + wave) * lat_angle.sin(),
            (r + wave) * lat_angle.cos() * lon_anim.sin(),
        );

        tf.translation = snap_grid(pos, cfg.pwn_voxel_size * 0.5);

        // Opacité : max au milieu de l'arc
        let opacity = (sample_t * std::f32::consts::PI).sin();
        tf.scale    = Vec3::splat((opacity * 0.9).max(0.05));
        *vis = if opacity > 0.05 { Visibility::Visible } else { Visibility::Hidden };
    }
}

// ─────────────────────────────────────────────
//  Animation anneau de choc
// ─────────────────────────────────────────────

fn animate_shock_ring(
    time:        Res<Time>,
    res:         Res<PulsarRes>,
    spin:        Res<PulsarSpinState>,
    mut shock_q: Query<(&ShockRingVoxel, &mut Transform)>,
) {
    let t = time.elapsed_secs();

    for (sv, mut tf) in &mut shock_q {
        let Some(cfg) = res.pulsars.get(sv.idx) else { continue; };

        let spin_angle = spin.angles.get(sv.idx).copied().unwrap_or(0.0);
        let mag        = magnetic_axis(cfg, spin_angle);

        // L'anneau de choc est dans le plan équatorial magnétique
        let up    = if mag.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
        let right = mag.cross(up).normalize();
        let fwd   = mag.cross(right).normalize();

        // Ondulation de l'anneau
        let wave_r = (t * 2.5 + sv.angle * 3.0 + sv.seed * std::f32::consts::TAU).sin()
            * cfg.shock_ring_width * 0.12;
        let wave_h = (t * 1.8 + sv.angle * 2.0 + sv.seed * 4.7).sin()
            * cfg.shock_ring_thickness * 0.35;

        let r   = sv.radius + wave_r;
        let pos = right * (r * sv.angle.cos())
            + fwd   * (r * sv.angle.sin())
            + mag   * (sv.height + wave_h);

        tf.translation = snap_grid(pos, cfg.shock_ring_voxel_size);

        // Pulsation synchrotron : plus rapide sur le bord interne
        let r_norm  = ((sv.radius - cfg.shock_ring_radius) / cfg.shock_ring_width.max(1.0)
            + 0.5).clamp(0.0, 1.0);
        let p_speed = 3.0 + (1.0 - r_norm) * 4.0;
        let pulse   = 0.6 + (t * p_speed + sv.seed * std::f32::consts::TAU).sin().abs() * 0.6;
        tf.scale    = Vec3::splat(pulse.max(0.05));
    }
}

// ─────────────────────────────────────────────
//  Régénération à chaud
// ─────────────────────────────────────────────

fn reload_pulsars(
    mut commands:  Commands,
    mut events:    EventReader<ReloadAstre>,
    res:           Res<PulsarRes>,
    roots:         Query<(Entity, &PulsarRoot)>,
    mut meshes:    ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for ev in events.read() {
        let Ok((entity, root)) = roots.get(ev.0) else { continue };
        let idx = root.idx;
        let Some(cfg) = res.pulsars.get(idx) else { continue };
        if let Some(ec) = commands.get_entity(entity) { ec.despawn_recursive(); }
        build_pulsar(&mut commands, cfg, idx, &mut meshes, &mut materials);
    }
}

fn regenerate_pulsars(
    mut commands:  Commands,
    mut events:    EventReader<RegeneratePulsar>,
    res:           Res<PulsarRes>,
    mut spin:      ResMut<PulsarSpinState>,
    root_q:        Query<Entity, With<PulsarRoot>>,
    mut meshes:    ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut fired = false;
    for _ in events.read() { fired = true; }
    if !fired { return; }

    for entity in &root_q {
        if let Some(ec) = commands.get_entity(entity) { ec.despawn_recursive(); }
    }

    spin.periods.clear();
    spin.angles.clear();
    spin.pulse_timers.clear();
    spin.pulse_active.clear();
    spin.pulse_radii.clear();

    for (idx, cfg) in res.pulsars.iter().enumerate() {
        spin.periods.push(cfg.period_initial);
        spin.angles.push(0.0);
        spin.pulse_timers.push(0.0);
        spin.pulse_active.push(false);
        spin.pulse_radii.push(0.0);
        build_pulsar(&mut commands, cfg, idx, &mut meshes, &mut materials);
    }
}
