use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;
use crate::astre::{AstreLodRoot, ReloadAstre};

// ─────────────────────────────────────────────
//  Config
// ─────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct MagnetarConfig {
    // --- Position & orbite ---
    pub position:               Vec3,
    pub orbit_distance:         f32,
    pub orbit_speed:            f32,
    /// Vitesse de rotation propre (rad/s) — très rapide pour un magnétar
    pub spin_speed:             f32,
    /// Inclinaison de l'axe magnétique par rapport à l'axe de rotation (rad)
    pub magnetic_tilt:          f32,

    // --- Corps stellaire ---
    pub radius:                 f32,
    pub surface_voxel_size:     f32,
    pub surface_resolution:     u32,
    pub color_equator:          [f32; 3],
    pub color_pole:             [f32; 3],
    pub surface_emissive:       f32,
    /// Craquements de surface (hotspots brillants qui migrent)
    pub crust_quake_count:      u32,
    pub crust_quake_emissive:   f32,
    pub crust_quake_color:      [f32; 3],
    pub crust_quake_speed:      f32,

    // --- Champ magnétique (arcs de voxels) ---
    pub field_line_count:       u32,     // nombre de lignes de champ
    pub field_line_samples:     u32,     // voxels par ligne
    pub field_voxel_size:       f32,
    pub field_radius:           f32,     // rayon max des arcs
    pub field_color_north:      [f32; 3],
    pub field_color_south:      [f32; 3],
    pub field_emissive:         f32,
    pub field_speed:            f32,     // vitesse d'animation des arcs

    // --- Zone d'interaction magnétique ---
    /// Rayon dans lequel les voxels du monde sont affectés
    pub magnetic_influence:     f32,
    /// Force de répulsion au pôle nord
    pub repulsion_force:        f32,
    /// Force d'attraction au pôle sud
    pub attraction_force:       f32,
    /// Vitesse max imposée aux voxels affectés
    pub max_magnetic_velocity:  f32,

    // --- Burst gamma continu ---
    pub continuous_burst:       bool,
    pub burst_voxel_size:       f32,
    pub burst_voxel_count:      u32,
    pub burst_color:            [f32; 3],
    pub burst_emissive:         f32,
    pub burst_radius:           f32,     // rayon de l'halo continu
    pub burst_pulse_speed:      f32,

    // --- Sursaut SGR (périodique) ---
    pub sgr_enabled:            bool,
    pub sgr_interval:           f32,     // secondes entre deux sursauts
    pub sgr_duration:           f32,     // durée d'un sursaut
    pub sgr_wave_count:         u32,     // voxels de l'onde de choc
    pub sgr_wave_voxel_size:    f32,
    pub sgr_color:              [f32; 3],
    pub sgr_emissive:           f32,
    pub sgr_max_radius:         f32,     // rayon max de l'onde de choc
    pub sgr_speed:              f32,     // vitesse d'expansion

    // --- Seed ---
    pub seed:                   u32,
}

impl Default for MagnetarConfig {
    fn default() -> Self {
        Self {
            position:               Vec3::ZERO,
            orbit_distance:         0.0,
            orbit_speed:            0.0,
            spin_speed:             3.5,
            magnetic_tilt:          0.3,

            radius:                 35.0,
            surface_voxel_size:     5.0,
            surface_resolution:     20,
            color_equator:          [0.15, 0.25, 0.85],
            color_pole:             [0.9,  0.5,  1.0 ],
            surface_emissive:       4.0,
            crust_quake_count:      8,
            crust_quake_emissive:   18.0,
            crust_quake_color:      [1.0, 0.8, 0.3],
            crust_quake_speed:      0.4,

            field_line_count:       24,
            field_line_samples:     40,
            field_voxel_size:       6.0,
            field_radius:           280.0,
            field_color_north:      [0.3, 0.6, 1.0],
            field_color_south:      [1.0, 0.3, 0.5],
            field_emissive:         5.0,
            field_speed:            0.8,

            magnetic_influence:     600.0,
            repulsion_force:        120.0,
            attraction_force:       80.0,
            max_magnetic_velocity:  250.0,

            continuous_burst:       true,
            burst_voxel_size:       8.0,
            burst_voxel_count:      300,
            burst_color:            [0.7, 0.3, 1.0],
            burst_emissive:         9.0,
            burst_radius:           90.0,
            burst_pulse_speed:      2.2,

            sgr_enabled:            true,
            sgr_interval:           12.0,
            sgr_duration:           3.5,
            sgr_wave_count:         200,
            sgr_wave_voxel_size:    14.0,
            sgr_color:              [1.0, 0.95, 0.5],
            sgr_emissive:           22.0,
            sgr_max_radius:         900.0,
            sgr_speed:              280.0,

            seed:                   17,
        }
    }
}

// ─────────────────────────────────────────────
//  Plugin
// ─────────────────────────────────────────────

pub struct MagnetarPlugin;

impl Plugin for MagnetarPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<MagnetarRes>()
            .add_event::<RegenerateMagnetar>()
            .add_event::<SgrBurstEvent>()
            .add_systems(Startup, spawn_magnetars)
            .add_systems(Update, (
                orbit_magnetars,
                spin_magnetar_body,
                animate_field_lines,
                animate_continuous_burst,
                tick_sgr_bursts,
                animate_sgr_wave,
                apply_magnetic_field,
                animate_crust_quakes,
                regenerate_magnetars,
                reload_magnetars,
            ).chain());
    }
}

// ─────────────────────────────────────────────
//  Resource & Events
// ─────────────────────────────────────────────

#[derive(Resource)]
pub struct MagnetarRes {
    pub magnetars: Vec<MagnetarConfig>,
}

impl Default for MagnetarRes {
    fn default() -> Self {
        Self { magnetars: vec![MagnetarConfig { position: Vec3::new(-2000.0, 0.0, -5000.0), ..Default::default() }] }
    }
}

#[derive(Event)]
pub struct RegenerateMagnetar;

/// Émis à chaque déclenchement d'un sursaut SGR.
/// Utilise-le pour audio, effets caméra, dégâts, etc.
#[derive(Event)]
pub struct SgrBurstEvent {
    pub magnetar_idx: usize,
    pub origin:       Vec3,
}

// ─────────────────────────────────────────────
//  Composants ECS
// ─────────────────────────────────────────────

/// Racine du magnétar
#[derive(Component)]
pub struct MagnetarRoot {
    pub idx: usize,
}

/// Corps stellaire (tourne sur lui-même)
#[derive(Component)]
pub struct MagnetarBody {
    pub idx: usize,
}

/// Voxel de surface
#[derive(Component)]
pub struct MagnetarSurface {
    pub idx:   usize,
    pub lat:   f32,
    pub lon:   f32,
}

/// Craquement de croûte (hotspot migrant)
#[derive(Component)]
pub struct CrustQuake {
    pub idx:       usize,
    pub quake_idx: u32,
    pub seed:      f32,
}

/// Voxel d'une ligne de champ magnétique
#[derive(Component)]
pub struct FieldLineVoxel {
    pub idx:        usize,
    pub line_idx:   u32,
    pub sample_idx: u32,
    /// Longitude de la ligne sur la surface (rad)
    pub lon:        f32,
    /// Inclinaison de la ligne (0 = équateur, PI/2 = pôle)
    pub tilt:       f32,
    pub seed:       f32,
}

/// Halo de rayonnement continu
#[derive(Component)]
pub struct BurstHaloVoxel {
    pub idx:   usize,
    pub theta: f32,
    pub phi:   f32,
    pub r:     f32,
    pub seed:  f32,
}

/// Onde de choc SGR
#[derive(Component)]
pub struct SgrWaveVoxel {
    pub idx:       usize,
    /// Direction de propagation normalisée
    pub dir:       Vec3,
    pub seed:      f32,
    /// Rayon courant de l'onde (animé)
    pub wave_idx:  u32,
}

/// Timer interne du magnétar (SGR)
#[derive(Component)]
pub struct MagnetarTimer {
    pub idx:            usize,
    /// Temps depuis le dernier burst
    pub time_since_sgr: f32,
    /// true si un burst SGR est en cours
    pub sgr_active:     bool,
    pub sgr_elapsed:    f32,
}

/// Marqueur : voxel du monde affectable par le champ magnétique.
/// Ajoute ce composant + MagneticPolarity sur les entités concernées.
#[derive(Component)]
pub struct MagneticallyAffectable;

/// Polarité magnétique d'un voxel du monde
#[derive(Component, Clone, Copy)]
pub enum MagneticPolarity {
    /// Repoussé par le pôle nord, attiré par le sud
    Positive,
    /// Attiré par le pôle nord, repoussé par le sud
    Negative,
    /// Neutre : attrait faible uniforme
    Neutral,
}

/// Vélocité magnétique accumulée sur un voxel du monde
#[derive(Component, Default)]
pub struct MagneticVelocity(pub Vec3);

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

/// Position d'une ligne de champ dipolaire (coordonnées sphériques)
/// r = R * cos²(lat) — équation d'une ligne de champ dipolaire
fn dipole_line_pos(
    sample: f32,  // ∈ [0, 1] le long de la ligne
    lon:    f32,
    tilt:   f32,
    r_max:  f32,
    radius: f32,
) -> Vec3 {
    // Paramétrage dipolaire : lat varie de -tilt à +tilt en passant par 0
    let lat = (sample * std::f32::consts::PI - std::f32::consts::FRAC_PI_2) * tilt
        / std::f32::consts::FRAC_PI_2;

    // r = r_max * cos²(lat) / cos²(tilt) — normalisé pour toucher la surface
    let cos_tilt = tilt.cos().max(0.01);
    let r = (r_max * lat.cos() * lat.cos() / (cos_tilt * cos_tilt))
        .clamp(radius, r_max);

    // Point sur l'arc
    let x_arc = r * lat.cos() * lon.cos();
    let y_arc = r * lat.sin();
    let z_arc = r * lat.cos() * lon.sin();

    Vec3::new(x_arc, y_arc, z_arc)
}

/// Pôle nord du magnétar dans l'espace local (tenu compte de l'inclinaison magnétique)
fn magnetic_north(cfg: &MagnetarConfig, spin_angle: f32) -> Vec3 {
    // L'axe magnétique tourne avec le spin mais est incliné de magnetic_tilt
    let tilt = cfg.magnetic_tilt;
    Vec3::new(
        tilt.sin() * spin_angle.cos(),
        tilt.cos(),
        tilt.sin() * spin_angle.sin(),
    ).normalize()
}

// ─────────────────────────────────────────────
//  Spawn
// ─────────────────────────────────────────────

fn spawn_magnetars(
    mut commands:  Commands,
    res:           Res<MagnetarRes>,
    mut meshes:    ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (idx, cfg) in res.magnetars.iter().enumerate() {
        build_magnetar(&mut commands, cfg, idx, &mut meshes, &mut materials);
    }
}

fn build_magnetar(
    commands:  &mut Commands,
    cfg:       &MagnetarConfig,
    idx:       usize,
    meshes:    &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) {
    let init_pos = if cfg.orbit_distance > 1.0 {
        Vec3::new(cfg.orbit_distance, 0.0, 0.0)
    } else {
        cfg.position
    };

    // Racine
    let root = commands.spawn((
        Transform::from_translation(init_pos),
        Visibility::default(),
        MagnetarRoot { idx },
        AstreLodRoot { cull_dist: 5000.0, radius: cfg.radius, streamable: true, label: "Magnetar" },
        MagnetarTimer { idx, time_since_sgr: 0.0, sgr_active: false, sgr_elapsed: 0.0 },
    )).id();

    // ── Corps stellaire ────────────────────────────────────────────────────
    let body = commands.spawn((
        Transform::IDENTITY,
        Visibility::default(),
        MagnetarBody { idx },
    )).id();
    commands.entity(root).add_child(body);

    let res_s = cfg.surface_resolution;
    let vstep  = std::f32::consts::PI / res_s as f32;
    let hstep  = std::f32::consts::TAU / (res_s * 2) as f32;
    let surf_mesh = meshes.add(Mesh::from(Cuboid::new(
        cfg.surface_voxel_size, cfg.surface_voxel_size, cfg.surface_voxel_size,
    )));

    let mut lat = -std::f32::consts::FRAC_PI_2;
    while lat <= std::f32::consts::FRAC_PI_2 {
        let mut lon = 0.0_f32;
        while lon < std::f32::consts::TAU {
            // Couleur : interpolation pôle→équateur selon latitude
            let lat_norm = (lat + std::f32::consts::FRAC_PI_2) / std::f32::consts::PI;
            let pole_t   = (1.0 - (lat_norm * 2.0 - 1.0).abs()).powf(2.0);
            let [sr, sg, sb] = lerp_color(cfg.color_equator, cfg.color_pole, 1.0 - pole_t);
            let se = cfg.surface_emissive;

            let surf_mat = materials.add(StandardMaterial {
                base_color: Color::srgb(sr, sg, sb),
                emissive:   LinearRgba::new(sr * se, sg * se, sb * se, 1.0),
                unlit:      true,
                ..default()
            });

            let pos = sphere_pos(lat, lon, cfg.radius);
            let snapped = snap_grid(pos, cfg.surface_voxel_size);

            let v = commands.spawn((
                Mesh3d(surf_mesh.clone()),
                MeshMaterial3d(surf_mat),
                Transform::from_translation(snapped),
                NotShadowCaster,
                MagnetarSurface { idx, lat, lon },
            )).id();
            commands.entity(body).add_child(v);

            lon += hstep;
        }
        lat += vstep;
    }

    // ── Craquements de croûte (hotspots) ──────────────────────────────────
    let quake_mesh = meshes.add(Mesh::from(Cuboid::new(
        cfg.surface_voxel_size * 1.4,
        cfg.surface_voxel_size * 1.4,
        cfg.surface_voxel_size * 1.4,
    )));

    for qi in 0..cfg.crust_quake_count {
        let seed = pseudo_hash(cfg.seed as f32, qi as f32 * 3.7);
        let [qr, qg, qb] = cfg.crust_quake_color;
        let qe = cfg.crust_quake_emissive;

        let quake_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(qr, qg, qb),
            emissive:   LinearRgba::new(qr * qe, qg * qe, qb * qe, 1.0),
            alpha_mode: AlphaMode::Add,
            unlit:      true,
            ..default()
        });

        // Position initiale — sera réanimée chaque frame
        let q = commands.spawn((
            Mesh3d(quake_mesh.clone()),
            MeshMaterial3d(quake_mat),
            Transform::from_translation(Vec3::Y * cfg.radius),
            Visibility::default(),
            NotShadowCaster,
            CrustQuake { idx, quake_idx: qi, seed },
        )).id();
        commands.entity(body).add_child(q);
    }

    // ── Lignes de champ magnétique ─────────────────────────────────────────
    let fmesh = meshes.add(Mesh::from(Cuboid::new(
        cfg.field_voxel_size, cfg.field_voxel_size, cfg.field_voxel_size,
    )));

    for li in 0..cfg.field_line_count {
        // Longitude répartie sur le cercle + légère variation
        let lon_base = li as f32 / cfg.field_line_count as f32 * std::f32::consts::TAU;
        let seed     = pseudo_hash(cfg.seed as f32 + li as f32, 5.5);
        let lon      = lon_base + (seed - 0.5) * 0.3;

        // Inclinaison de la ligne : varie de quasi-équatorial à quasi-polaire
        let tilt_t   = (li as f32 / cfg.field_line_count as f32 * 3.0).fract();
        let tilt     = 0.15 + tilt_t * std::f32::consts::FRAC_PI_2 * 0.9;

        for si in 0..cfg.field_line_samples {
            let sample_t = si as f32 / cfg.field_line_samples as f32;

            // Couleur : nord → sud selon la progression
            let [fr, fg, fb] = lerp_color(cfg.field_color_north, cfg.field_color_south, sample_t);
            let fe = cfg.field_emissive * (0.4 + (sample_t * std::f32::consts::PI).sin() * 0.6);

            let field_mat = materials.add(StandardMaterial {
                base_color: Color::srgb(fr, fg, fb),
                emissive:   LinearRgba::new(fr * fe, fg * fe, fb * fe, 1.0),
                alpha_mode: AlphaMode::Add,
                unlit:      true,
                ..default()
            });

            let pos = dipole_line_pos(sample_t, lon, tilt, cfg.field_radius, cfg.radius);
            let snapped = snap_grid(pos, cfg.field_voxel_size);

            let fv = commands.spawn((
                Mesh3d(fmesh.clone()),
                MeshMaterial3d(field_mat),
                Transform::from_translation(snapped),
                Visibility::default(),
                NotShadowCaster,
                FieldLineVoxel { idx, line_idx: li, sample_idx: si, lon, tilt, seed },
            )).id();
            commands.entity(root).add_child(fv);
        }
    }

    // ── Halo de rayonnement continu ────────────────────────────────────────
    if cfg.continuous_burst {
        let [br, bg, bb] = cfg.burst_color;
        let be = cfg.burst_emissive;
        let burst_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(br, bg, bb),
            emissive:   LinearRgba::new(br * be, bg * be, bb * be, 1.0),
            alpha_mode: AlphaMode::Add,
            unlit:      true,
            ..default()
        });
        let burst_mesh = meshes.add(Mesh::from(Cuboid::new(
            cfg.burst_voxel_size, cfg.burst_voxel_size, cfg.burst_voxel_size,
        )));

        for bi in 0..cfg.burst_voxel_count {
            let h1 = pseudo_hash(cfg.seed as f32 + 400.0, bi as f32);
            let h2 = pseudo_hash(cfg.seed as f32 + 401.0, bi as f32);
            let h3 = pseudo_hash(cfg.seed as f32 + 402.0, bi as f32);

            let theta = h1 * std::f32::consts::TAU;
            let phi   = (h2 * 2.0 - 1.0).clamp(-1.0, 1.0).acos();
            let r     = cfg.radius + h3.sqrt() * cfg.burst_radius;

            let bv = commands.spawn((
                Mesh3d(burst_mesh.clone()),
                MeshMaterial3d(burst_mat.clone()),
                Transform::from_translation(Vec3::ZERO),
                Visibility::default(),
                NotShadowCaster,
                BurstHaloVoxel { idx, theta, phi, r, seed: h1 },
            )).id();
            commands.entity(root).add_child(bv);
        }
    }

    // ── Onde de choc SGR (spawn une fois, réutilisée) ─────────────────────
    if cfg.sgr_enabled {
        let [wr, wg, wb] = cfg.sgr_color;
        let we = cfg.sgr_emissive;
        let wave_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(wr, wg, wb),
            emissive:   LinearRgba::new(wr * we, wg * we, wb * we, 1.0),
            alpha_mode: AlphaMode::Add,
            unlit:      true,
            ..default()
        });
        let wave_mesh = meshes.add(Mesh::from(Cuboid::new(
            cfg.sgr_wave_voxel_size,
            cfg.sgr_wave_voxel_size,
            cfg.sgr_wave_voxel_size,
        )));

        for wi in 0..cfg.sgr_wave_count {
            let h1 = pseudo_hash(cfg.seed as f32 + 500.0, wi as f32);
            let h2 = pseudo_hash(cfg.seed as f32 + 501.0, wi as f32);

            let theta = h1 * std::f32::consts::TAU;
            let phi   = (h2 * 2.0 - 1.0).clamp(-1.0, 1.0).acos();
            let dir   = Vec3::new(
                phi.sin() * theta.cos(),
                phi.cos(),
                phi.sin() * theta.sin(),
            ).normalize();

            let wv = commands.spawn((
                Mesh3d(wave_mesh.clone()),
                MeshMaterial3d(wave_mat.clone()),
                Transform::from_translation(Vec3::ZERO).with_scale(Vec3::ZERO),
                Visibility::Hidden,
                NotShadowCaster,
                SgrWaveVoxel { idx, dir, seed: pseudo_hash(h1, h2), wave_idx: wi },
            )).id();
            commands.entity(root).add_child(wv);
        }
    }
}

// ─────────────────────────────────────────────
//  Orbite
// ─────────────────────────────────────────────

fn orbit_magnetars(
    time:       Res<Time>,
    res:        Res<MagnetarRes>,
    mut root_q: Query<(&mut Transform, &MagnetarRoot)>,
) {
    let t = time.elapsed_secs();
    for (mut tf, root) in &mut root_q {
        let Some(cfg) = res.magnetars.get(root.idx) else { continue; };
        if cfg.orbit_distance > 1.0 {
            let angle = t * cfg.orbit_speed;
            tf.translation.x = angle.cos() * cfg.orbit_distance;
            tf.translation.z = angle.sin() * cfg.orbit_distance;
        }
    }
}

// ─────────────────────────────────────────────
//  Rotation du corps stellaire
// ─────────────────────────────────────────────

fn spin_magnetar_body(
    time:      Res<Time>,
    res:       Res<MagnetarRes>,
    mut body_q: Query<(&mut Transform, &MagnetarBody)>,
) {
    let t = time.elapsed_secs();
    for (mut tf, body) in &mut body_q {
        let Some(cfg) = res.magnetars.get(body.idx) else { continue; };
        tf.rotation = Quat::from_rotation_y(t * cfg.spin_speed);
    }
}

// ─────────────────────────────────────────────
//  Animation des craquements de croûte
// ─────────────────────────────────────────────

fn animate_crust_quakes(
    time:       Res<Time>,
    res:        Res<MagnetarRes>,
    mut quake_q: Query<(&CrustQuake, &mut Transform, &mut Visibility)>,
) {
    let t = time.elapsed_secs();

    for (cq, mut tf, mut vis) in &mut quake_q {
        let Some(cfg) = res.magnetars.get(cq.idx) else { continue; };

        let s = cq.seed;
        let speed = cfg.crust_quake_speed;

        // Chaque hotspot migre lentement sur la surface
        let lat = (t * speed * 0.31 + s * 5.7).sin() * std::f32::consts::FRAC_PI_2;
        let lon = (t * speed * 0.19 + s * 11.3) * std::f32::consts::TAU;

        let pos = sphere_pos(lat, lon, cfg.radius + cfg.surface_voxel_size * 0.6);
        tf.translation = snap_grid(pos, cfg.surface_voxel_size);

        // Pulsation intense et irrégulière
        let flicker = (t * 7.0 + s * 23.0).sin() * 0.5
            + (t * 13.0 + s * 7.0).sin() * 0.3
            + (t * 31.0 + s * 41.0).sin() * 0.2;
        let scale = (0.4 + flicker.abs() * 1.2).max(0.05);
        tf.scale  = Vec3::splat(scale);

        // Clignote aléatoirement
        *vis = if flicker.abs() > 0.1 { Visibility::Visible } else { Visibility::Hidden };
    }
}

// ─────────────────────────────────────────────
//  Animation des lignes de champ
// ─────────────────────────────────────────────

fn animate_field_lines(
    time:       Res<Time>,
    res:        Res<MagnetarRes>,
    root_q:     Query<(&GlobalTransform, &MagnetarRoot)>,
    mut field_q: Query<(&FieldLineVoxel, &mut Transform, &mut Visibility)>,
) {
    let t = time.elapsed_secs();

    for (fv, mut tf, mut vis) in &mut field_q {
        let Some(cfg) = res.magnetars.get(fv.idx) else { continue; };

        let _magnetar_pos = root_q
            .iter()
            .find(|(_, r)| r.idx == fv.idx)
            .map(|(gt, _)| gt.translation())
            .unwrap_or_default();

        // L'axe magnétique tourne avec le spin
        let spin_angle = t * cfg.spin_speed;

        // Longitude animée : les lignes tournent avec le spin + ondulation
        let wave_offset = (t * cfg.field_speed + fv.seed * std::f32::consts::TAU).sin() * 0.15;
        let lon_anim    = fv.lon + spin_angle + wave_offset;

        let sample_t = fv.sample_idx as f32 / (cfg.field_line_samples as f32 - 1.0);

        // Ondulation de l'arc : les lignes de champ vibrent légèrement
        let vib = (t * cfg.field_speed * 2.0 + fv.seed * 9.1 + sample_t * 5.0).sin()
            * cfg.field_radius * 0.025;

        let mut pos = dipole_line_pos(sample_t, lon_anim, fv.tilt, cfg.field_radius, cfg.radius);
        // Ajoute la vibration perpendiculairement
        let perp = Vec3::new(-lon_anim.sin(), 0.0, lon_anim.cos());
        pos += perp * vib;

        tf.translation = snap_grid(pos, cfg.field_voxel_size);

        // Opacité maximale au milieu de l'arc, nulle aux extrémités
        let opacity = (sample_t * std::f32::consts::PI).sin();
        let scale   = (opacity * (0.7 + fv.seed * 0.6)).max(0.05);
        tf.scale    = Vec3::splat(scale);

        *vis = if opacity > 0.05 { Visibility::Visible } else { Visibility::Hidden };
    }
}

// ─────────────────────────────────────────────
//  Animation halo continu
// ─────────────────────────────────────────────

fn animate_continuous_burst(
    time:       Res<Time>,
    res:        Res<MagnetarRes>,
    mut halo_q: Query<(&BurstHaloVoxel, &mut Transform)>,
) {
    let t = time.elapsed_secs();

    for (bv, mut tf) in &mut halo_q {
        let Some(cfg) = res.magnetars.get(bv.idx) else { continue; };

        let speed  = cfg.burst_pulse_speed;
        // Pulsation radiale : les voxels du halo respirent vers l'extérieur
        let pulse  = 1.0 + (t * speed + bv.seed * std::f32::consts::TAU).sin() * 0.22;
        let r_anim = bv.r * pulse;

        let _dir = Vec3::new(
            bv.phi.sin() * bv.theta.cos(),
            bv.phi.cos(),
            bv.phi.sin() * bv.theta.sin(),
        );

        // Légère rotation du halo
        let theta_anim = bv.theta + t * 0.15;
        let dir_anim   = Vec3::new(
            bv.phi.sin() * theta_anim.cos(),
            bv.phi.cos(),
            bv.phi.sin() * theta_anim.sin(),
        );

        let pos = dir_anim * r_anim;
        tf.translation = snap_grid(pos, cfg.burst_voxel_size);

        // Scale pulsant + variation par voxel
        let s = 0.5 + (t * speed * 1.3 + bv.seed * 7.0).sin().abs() * 0.8;
        tf.scale = Vec3::splat(s.max(0.05));
    }
}

// ─────────────────────────────────────────────
//  Tick SGR (timer + déclenchement)
// ─────────────────────────────────────────────

fn tick_sgr_bursts(
    time:       Res<Time>,
    res:        Res<MagnetarRes>,
    root_q:     Query<(&GlobalTransform, &MagnetarRoot)>,
    mut timer_q: Query<&mut MagnetarTimer>,
    mut sgr_events: EventWriter<SgrBurstEvent>,
) {
    let dt = time.delta_secs();

    for mut timer in &mut timer_q {
        let Some(cfg) = res.magnetars.get(timer.idx) else { continue; };
        if !cfg.sgr_enabled { continue; }

        if timer.sgr_active {
            timer.sgr_elapsed += dt;
            if timer.sgr_elapsed >= cfg.sgr_duration {
                timer.sgr_active   = false;
                timer.sgr_elapsed  = 0.0;
                timer.time_since_sgr = 0.0;
            }
        } else {
            timer.time_since_sgr += dt;
            if timer.time_since_sgr >= cfg.sgr_interval {
                timer.sgr_active  = true;
                timer.sgr_elapsed = 0.0;

                let origin = root_q
                    .iter()
                    .find(|(_, r)| r.idx == timer.idx)
                    .map(|(gt, _)| gt.translation())
                    .unwrap_or_default();

                sgr_events.send(SgrBurstEvent { magnetar_idx: timer.idx, origin });
            }
        }
    }
}

// ─────────────────────────────────────────────
//  Animation onde de choc SGR
// ─────────────────────────────────────────────

fn animate_sgr_wave(
    res:        Res<MagnetarRes>,
    timer_q:    Query<&MagnetarTimer>,
    mut wave_q: Query<(&SgrWaveVoxel, &mut Transform, &mut Visibility)>,
) {
    for (wv, mut tf, mut vis) in &mut wave_q {
        let Some(cfg) = res.magnetars.get(wv.idx) else {
            *vis = Visibility::Hidden;
            continue;
        };

        // Récupère l'état du timer pour ce magnétar
        let timer = timer_q.iter().find(|t| t.idx == wv.idx);
        let (sgr_active, sgr_elapsed) = timer
            .map(|t| (t.sgr_active, t.sgr_elapsed))
            .unwrap_or((false, 0.0));

        if !sgr_active {
            *vis = Visibility::Hidden;
            continue;
        }

        // Rayon courant de l'onde
        let radius = cfg.sgr_speed * sgr_elapsed;

        if radius > cfg.sgr_max_radius {
            *vis = Visibility::Hidden;
            continue;
        }

        // Chaque voxel suit sa direction propre + léger jitter
        let jitter = (wv.seed * std::f32::consts::TAU).sin() * radius * 0.05;
        let pos    = wv.dir * (radius + jitter);
        tf.translation = snap_grid(pos, cfg.sgr_wave_voxel_size);

        // Fade en fin de burst
        let life  = (1.0 - radius / cfg.sgr_max_radius).clamp(0.0, 1.0);
        // Épaisseur de l'anneau : voxels en coque fine
        let shell_t = (sgr_elapsed / cfg.sgr_duration).clamp(0.0, 1.0);
        let scale = life * (0.5 + shell_t * 1.5);
        tf.scale  = Vec3::splat(scale.max(0.05));

        *vis = Visibility::Visible;
    }
}

// ─────────────────────────────────────────────
//  Champ magnétique sur les voxels du monde
// ─────────────────────────────────────────────

/// Applique répulsion/attraction selon la polarité du voxel
/// et la direction par rapport aux pôles magnétiques.
fn apply_magnetic_field(
    time:       Res<Time>,
    res:        Res<MagnetarRes>,
    root_q:     Query<(&GlobalTransform, &MagnetarRoot)>,
    mut voxel_q: Query<
        (&GlobalTransform, &MagneticPolarity, &mut MagneticVelocity, &mut Transform),
        With<MagneticallyAffectable>,
    >,
) {
    let dt = time.delta_secs();
    let t  = time.elapsed_secs();

    for (hole_gt, hole_root) in &root_q {
        let Some(cfg) = res.magnetars.get(hole_root.idx) else { continue; };

        let mag_pos   = hole_gt.translation();
        let spin_angle = t * cfg.spin_speed;
        let north_dir  = magnetic_north(cfg, spin_angle);
        let south_dir  = -north_dir;

        // Positions des pôles sur la surface
        let north_pole = mag_pos + north_dir * cfg.radius;
        let south_pole = mag_pos + south_dir * cfg.radius;

        for (vox_gt, polarity, mut mag_vel, mut tf) in &mut voxel_q {
            let vox_pos  = vox_gt.translation();
            let dist     = vox_pos.distance(mag_pos);

            if dist > cfg.magnetic_influence { continue; }

            // Falloff quadratique
            let falloff = (1.0 - dist / cfg.magnetic_influence).clamp(0.0, 1.0).powi(2);

            let to_north = (north_pole - vox_pos).normalize_or_zero();
            let to_south = (south_pole - vox_pos).normalize_or_zero();

            let force = match polarity {
                MagneticPolarity::Positive => {
                    // Repoussé par le nord, attiré par le sud
                    -to_north * cfg.repulsion_force + to_south * cfg.attraction_force
                }
                MagneticPolarity::Negative => {
                    // Attiré par le nord, repoussé par le sud
                    to_north * cfg.attraction_force - to_south * cfg.repulsion_force
                }
                MagneticPolarity::Neutral => {
                    // Faible attraction vers le centre
                    (mag_pos - vox_pos).normalize_or_zero()
                        * (cfg.attraction_force * 0.15)
                }
            };

            mag_vel.0 = (mag_vel.0 + force * falloff * dt)
                .clamp_length_max(cfg.max_magnetic_velocity);

            // Amortissement progressif (pas de dérive infinie)
            mag_vel.0 *= 1.0 - dt * 0.8;

            tf.translation += mag_vel.0 * dt;
        }
    }
}

// ─────────────────────────────────────────────
//  Régénération à chaud
// ─────────────────────────────────────────────

fn reload_magnetars(
    mut commands:  Commands,
    mut events:    EventReader<ReloadAstre>,
    res:           Res<MagnetarRes>,
    roots:         Query<(Entity, &MagnetarRoot)>,
    mut meshes:    ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for ev in events.read() {
        let Ok((entity, root)) = roots.get(ev.0) else { continue };
        let idx = root.idx;
        let Some(cfg) = res.magnetars.get(idx) else { continue };
        if let Some(ec) = commands.get_entity(entity) { ec.despawn_recursive(); }
        build_magnetar(&mut commands, cfg, idx, &mut meshes, &mut materials);
    }
}

fn regenerate_magnetars(
    mut commands:  Commands,
    mut events:    EventReader<RegenerateMagnetar>,
    res:           Res<MagnetarRes>,
    root_q:        Query<Entity, With<MagnetarRoot>>,
    mut meshes:    ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut fired = false;
    for _ in events.read() { fired = true; }
    if !fired { return; }

    for entity in &root_q {
        if let Some(ec) = commands.get_entity(entity) { ec.despawn_recursive(); }
    }
    for (idx, cfg) in res.magnetars.iter().enumerate() {
        build_magnetar(&mut commands, cfg, idx, &mut meshes, &mut materials);
    }
}
