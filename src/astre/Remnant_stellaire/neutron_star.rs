use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;
use crate::astre::{AstreLodRoot, ReloadAstre};

// ─────────────────────────────────────────────
//  Mode de l'étoile à neutrons
// ─────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub enum NeutronStarMode {
    /// Étoile isolée, refroidissement pur
    Isolated,
    /// Système binaire X : accrétion depuis un compagnon
    BinaryX {
        /// Distance du compagnon
        companion_distance: f32,
        /// Rayon du compagnon
        companion_radius:   f32,
        /// Couleur du compagnon
        companion_color:    [f32; 3],
        /// Vitesse orbitale du binaire
        binary_orbit_speed: f32,
        /// Débit d'accrétion (voxels/s)
        accretion_rate:     f32,
        /// Rayon du disque d'accrétion
        disk_inner:         f32,
        disk_outer:         f32,
    },
    /// Les deux disponibles via paramètre runtime
    Both {
        companion_distance: f32,
        companion_radius:   f32,
        companion_color:    [f32; 3],
        binary_orbit_speed: f32,
        accretion_rate:     f32,
        disk_inner:         f32,
        disk_outer:         f32,
        /// true = mode BinaryX actif, false = Isolated
        active_binary:      bool,
    },
}

// ─────────────────────────────────────────────
//  Config
// ─────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct NeutronStarConfig {
    // --- Position & orbite ---
    pub position:               Vec3,
    pub orbit_distance:         f32,
    pub orbit_speed:            f32,
    pub spin_speed:             f32,

    // --- Mode ---
    pub mode:                   NeutronStarMode,

    // --- Corps stellaire ---
    pub radius:                 f32,
    pub surface_voxel_size:     f32,
    pub surface_resolution:     u32,

    // --- Refroidissement thermique (cooling curve) ---
    /// Température initiale (normalisée 0–1, 1 = max chaud)
    pub temp_initial:           f32,
    /// Constante de refroidissement (τ en secondes de jeu)
    /// T(t) = T0 * exp(-t / tau_cool)
    pub tau_cool:               f32,
    /// Température résiduelle min (jamais 0 — neutrons dégénérés)
    pub temp_floor:             f32,
    /// Couleur à T max (blanc-bleu très chaud)
    pub color_hot:              [f32; 3],
    /// Couleur à T min (rouge sombre)
    pub color_cold:             [f32; 3],
    pub surface_emissive_hot:   f32,
    pub surface_emissive_cold:  f32,

    // --- Oscillations sismiques (QPO / starquakes) ---
    pub qpo_enabled:            bool,
    /// Fréquences des modes d'oscillation (en Hz de jeu)
    pub qpo_frequencies:        Vec<f32>,
    /// Amplitude des déplacements de surface (en unités voxel)
    pub qpo_amplitude:          f32,
    /// Starquakes : déchirures aléatoires de la croûte
    pub starquake_enabled:      bool,
    pub starquake_interval:     f32,    // s entre deux starquakes
    pub starquake_duration:     f32,
    pub starquake_count:        u32,    // fissures simultanées
    pub starquake_voxel_size:   f32,
    pub starquake_color:        [f32; 3],
    pub starquake_emissive:     f32,

    // --- Atmosphère de neutrons (fine couche superficielle) ---
    pub atmosphere_enabled:     bool,
    pub atmosphere_thickness:   f32,
    pub atmosphere_voxel_size:  f32,
    pub atmosphere_count:       u32,
    pub atmosphere_color:       [f32; 3],
    pub atmosphere_emissive:    f32,
    pub atmosphere_opacity:     f32,

    // --- Halo de rayonnement X thermique ---
    pub xray_halo_enabled:      bool,
    pub xray_halo_radius:       f32,
    pub xray_halo_count:        u32,
    pub xray_halo_voxel_size:   f32,
    pub xray_halo_color:        [f32; 3],
    pub xray_halo_emissive:     f32,
    pub xray_pulse_speed:       f32,

    // --- Disque d'accrétion binaire ---
    pub disk_voxel_size:        f32,
    pub disk_count:             u32,
    pub disk_color_inner:       [f32; 3],
    pub disk_color_outer:       [f32; 3],
    pub disk_emissive:          f32,
    pub disk_thickness:         f32,
    pub disk_rotation_speed:    f32,
    pub disk_turbulence:        f32,

    // --- Jet d'accrétion (colonne polaire) ---
    pub accretion_column_enabled: bool,
    pub column_length:          f32,
    pub column_width:           f32,
    pub column_voxel_size:      f32,
    pub column_count:           u32,
    pub column_color:           [f32; 3],
    pub column_emissive:        f32,
    pub column_speed:           f32,

    // --- Flux d'accrétion compagnon → étoile ---
    pub stream_voxel_size:      f32,
    pub stream_count:           u32,
    pub stream_color:           [f32; 3],
    pub stream_emissive:        f32,

    // --- Seed ---
    pub seed:                   u32,
}

impl Default for NeutronStarConfig {
    fn default() -> Self {
        Self {
            position:               Vec3::ZERO,
            orbit_distance:         0.0,
            orbit_speed:            0.0,
            spin_speed:             1.8,

            mode: NeutronStarMode::Both {
                companion_distance: 320.0,
                companion_radius:   55.0,
                companion_color:    [1.0, 0.65, 0.2],
                binary_orbit_speed: 0.025,
                accretion_rate:     60.0,
                disk_inner:         50.0,
                disk_outer:         220.0,
                active_binary:      true,
            },

            radius:                 22.0,
            surface_voxel_size:     3.5,
            surface_resolution:     16,

            temp_initial:           1.0,
            tau_cool:               120.0,
            temp_floor:             0.08,
            color_hot:              [0.75, 0.88, 1.0],
            color_cold:             [0.5, 0.05, 0.02],
            surface_emissive_hot:   12.0,
            surface_emissive_cold:  0.8,

            qpo_enabled:            true,
            qpo_frequencies:        vec![0.7, 1.3, 2.1, 3.8],
            qpo_amplitude:          2.5,
            starquake_enabled:      true,
            starquake_interval:     18.0,
            starquake_duration:     2.5,
            starquake_count:        6,
            starquake_voxel_size:   5.0,
            starquake_color:        [1.0, 0.7, 0.2],
            starquake_emissive:     16.0,

            atmosphere_enabled:     true,
            atmosphere_thickness:   8.0,
            atmosphere_voxel_size:  4.0,
            atmosphere_count:       180,
            atmosphere_color:       [0.6, 0.8, 1.0],
            atmosphere_emissive:    3.5,
            atmosphere_opacity:     0.45,

            xray_halo_enabled:      true,
            xray_halo_radius:       65.0,
            xray_halo_count:        220,
            xray_halo_voxel_size:   6.0,
            xray_halo_color:        [0.5, 0.7, 1.0],
            xray_halo_emissive:     5.0,
            xray_pulse_speed:       1.4,

            disk_voxel_size:        10.0,
            disk_count:             700,
            disk_color_inner:       [1.0, 0.6, 0.1],
            disk_color_outer:       [0.35, 0.1, 0.55],
            disk_emissive:          4.5,
            disk_thickness:         16.0,
            disk_rotation_speed:    0.14,
            disk_turbulence:        5.0,

            accretion_column_enabled: true,
            column_length:          80.0,
            column_width:           8.0,
            column_voxel_size:      5.0,
            column_count:           80,
            column_color:           [1.0, 0.85, 0.5],
            column_emissive:        12.0,
            column_speed:           1.6,

            stream_voxel_size:      7.0,
            stream_count:           120,
            stream_color:           [0.9, 0.55, 0.15],
            stream_emissive:        3.5,

            seed:                   23,
        }
    }
}

// ─────────────────────────────────────────────
//  Plugin
// ─────────────────────────────────────────────

pub struct NeutronStarPlugin;

impl Plugin for NeutronStarPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<NeutronStarRes>()
            .init_resource::<NeutronStarState>()
            .add_event::<RegenerateNeutronStar>()
            .add_event::<StarquakeEvent>()
            .add_systems(Startup, spawn_neutron_stars)
            .add_systems(Update, (
                orbit_neutron_stars,
                spin_neutron_star,
                tick_cooling,
                update_surface_temperature,
                animate_atmosphere,
                animate_xray_halo,
                tick_qpo_oscillations,
                tick_starquakes,
                animate_starquake_fissures,
                animate_accretion_disk,
                animate_accretion_column,
                animate_accretion_stream,
                orbit_companion,
                regenerate_neutron_stars,
                reload_neutron_stars,
            ).chain());
    }
}

// ─────────────────────────────────────────────
//  Resource & Events
// ─────────────────────────────────────────────

#[derive(Resource)]
pub struct NeutronStarRes {
    pub stars: Vec<NeutronStarConfig>,
}

impl Default for NeutronStarRes {
    fn default() -> Self {
        Self { stars: vec![NeutronStarConfig { position: Vec3::new(0.0, 0.0, -7000.0), ..Default::default() }] }
    }
}

/// État dynamique runtime par étoile
#[derive(Resource, Default)]
pub struct NeutronStarState {
    /// Température courante normalisée [0,1] par étoile
    pub temperatures:       Vec<f32>,
    /// Temps de jeu depuis le spawn (pour cooling)
    pub ages:               Vec<f32>,
    /// Angle de spin courant
    pub spin_angles:        Vec<f32>,
    /// Timer starquake
    pub starquake_timers:   Vec<f32>,
    pub starquake_active:   Vec<bool>,
    pub starquake_elapsed:  Vec<f32>,
    /// QPO : phase courante par fréquence
    pub qpo_phases:         Vec<Vec<f32>>,
}

#[derive(Event)]
pub struct RegenerateNeutronStar;

/// Émis à chaque starquake — utile pour effets, audio, dégâts
#[derive(Event)]
pub struct StarquakeEvent {
    pub star_idx:   usize,
    pub origin:     Vec3,
    pub magnitude:  f32,
}

// ─────────────────────────────────────────────
//  Composants ECS
// ─────────────────────────────────────────────

#[derive(Component)]
pub struct NeutronStarRoot {
    pub idx: usize,
}

/// Corps stellaire (tourne)
#[derive(Component)]
pub struct NeutronStarBody {
    pub idx: usize,
}

/// Voxel de surface — couleur mise à jour par cooling
#[derive(Component)]
pub struct NeutronSurface {
    pub idx: usize,
    pub lat: f32,
    pub lon: f32,
}

/// Voxel d'atmosphère
#[derive(Component)]
pub struct NeutronAtmosphere {
    pub idx:    usize,
    pub origin: Vec3,
    pub seed:   f32,
    pub theta:  f32,
    pub phi:    f32,
}

/// Halo X thermique
#[derive(Component)]
pub struct XrayHaloVoxel {
    pub idx:   usize,
    pub theta: f32,
    pub phi:   f32,
    pub r:     f32,
    pub seed:  f32,
}

/// Fissure de starquake
#[derive(Component)]
pub struct StarquakeFissure {
    pub idx:        usize,
    pub fissure_idx: u32,
    pub seed:       f32,
    pub lat:        f32,
    pub lon:        f32,
}

/// Voxel du disque d'accrétion
#[derive(Component)]
pub struct AccretionDiskVoxel {
    pub idx:     usize,
    pub angle:   f32,
    pub radius:  f32,
    pub height:  f32,
    pub seed:    f32,
    pub color_t: f32,
}

/// Voxel de la colonne d'accrétion polaire
#[derive(Component)]
pub struct AccretionColumnVoxel {
    pub idx:        usize,
    pub pole:       f32,   // +1 nord, -1 sud
    pub t:          f32,   // ∈ [0,1] le long de la colonne
    pub perp_seed:  f32,
    pub seed:       f32,
}

/// Flux d'accrétion compagnon → étoile
#[derive(Component)]
pub struct AccretionStream {
    pub idx:    usize,
    pub t:      f32,   // ∈ [0,1] de compagnon à étoile
    pub seed:   f32,
    pub offset: Vec2,  // décalage perpendiculaire
}

/// Compagnon stellaire (binaire X)
#[derive(Component)]
pub struct CompanionStar {
    pub idx: usize,
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

/// Température normalisée selon la cooling curve
/// T(t) = max(T_floor, T0 * exp(-t / tau))
fn cooled_temp(cfg: &NeutronStarConfig, age: f32) -> f32 {
    let t = cfg.temp_initial * (-age / cfg.tau_cool.max(0.001)).exp();
    t.max(cfg.temp_floor)
}

/// Extrait les paramètres binaires selon le mode
fn binary_params(cfg: &NeutronStarConfig) -> Option<(f32, f32, [f32; 3], f32, f32, f32, f32)> {
    match &cfg.mode {
        NeutronStarMode::BinaryX {
            companion_distance, companion_radius, companion_color,
            binary_orbit_speed, accretion_rate, disk_inner, disk_outer,
        } => Some((
            *companion_distance, *companion_radius, *companion_color,
            *binary_orbit_speed, *accretion_rate, *disk_inner, *disk_outer,
        )),
        NeutronStarMode::Both {
            companion_distance, companion_radius, companion_color,
            binary_orbit_speed, accretion_rate, disk_inner, disk_outer,
            active_binary,
        } if *active_binary => Some((
            *companion_distance, *companion_radius, *companion_color,
            *binary_orbit_speed, *accretion_rate, *disk_inner, *disk_outer,
        )),
        _ => None,
    }
}

fn is_binary_active(cfg: &NeutronStarConfig) -> bool {
    binary_params(cfg).is_some()
}

// ─────────────────────────────────────────────
//  Spawn
// ─────────────────────────────────────────────

fn spawn_neutron_stars(
    mut commands:  Commands,
    res:           Res<NeutronStarRes>,
    mut state:     ResMut<NeutronStarState>,
    mut meshes:    ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    state.temperatures.clear();
    state.ages.clear();
    state.spin_angles.clear();
    state.starquake_timers.clear();
    state.starquake_active.clear();
    state.starquake_elapsed.clear();
    state.qpo_phases.clear();

    for (idx, cfg) in res.stars.iter().enumerate() {
        state.temperatures.push(cfg.temp_initial);
        state.ages.push(0.0);
        state.spin_angles.push(0.0);
        state.starquake_timers.push(0.0);
        state.starquake_active.push(false);
        state.starquake_elapsed.push(0.0);
        state.qpo_phases.push(vec![0.0; cfg.qpo_frequencies.len()]);

        build_neutron_star(&mut commands, cfg, idx, &mut meshes, &mut materials);
    }
}

fn build_neutron_star(
    commands:  &mut Commands,
    cfg:       &NeutronStarConfig,
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
        NeutronStarRoot { idx },
        AstreLodRoot { cull_dist: 5000.0, radius: cfg.radius, streamable: true, label: "NeutronStar" },
    )).id();

    // ── Corps stellaire ────────────────────────────────────────────────────
    let body = commands.spawn((
        Transform::IDENTITY,
        Visibility::default(),
        NeutronStarBody { idx },
    )).id();
    commands.entity(root).add_child(body);

    let res_s   = cfg.surface_resolution;
    let vstep   = std::f32::consts::PI / res_s as f32;
    let hstep   = std::f32::consts::TAU / (res_s * 2) as f32;
    let sv_mesh = meshes.add(Mesh::from(Cuboid::new(
        cfg.surface_voxel_size, cfg.surface_voxel_size, cfg.surface_voxel_size,
    )));

    // Surface — couleur initiale à T max
    let [hr, hg, hb] = cfg.color_hot;
    let he = cfg.surface_emissive_hot;
    let surf_mat_hot = materials.add(StandardMaterial {
        base_color: Color::srgb(hr, hg, hb),
        emissive:   LinearRgba::new(hr * he, hg * he, hb * he, 1.0),
        unlit:      true,
        ..default()
    });

    let mut lat = -std::f32::consts::FRAC_PI_2;
    while lat <= std::f32::consts::FRAC_PI_2 {
        let mut lon = 0.0_f32;
        while lon < std::f32::consts::TAU {
            let pos = sphere_pos(lat, lon, cfg.radius);
            let v = commands.spawn((
                Mesh3d(sv_mesh.clone()),
                MeshMaterial3d(surf_mat_hot.clone()),
                Transform::from_translation(snap_grid(pos, cfg.surface_voxel_size)),
                NotShadowCaster,
                NeutronSurface { idx, lat, lon },
            )).id();
            commands.entity(body).add_child(v);
            lon += hstep;
        }
        lat += vstep;
    }

    // ── Atmosphère ─────────────────────────────────────────────────────────
    if cfg.atmosphere_enabled {
        let [ar, ag, ab] = cfg.atmosphere_color;
        let ae = cfg.atmosphere_emissive;
        let atm_mat = materials.add(StandardMaterial {
            base_color: Color::srgba(ar, ag, ab, cfg.atmosphere_opacity),
            emissive:   LinearRgba::new(ar * ae, ag * ae, ab * ae, 1.0),
            alpha_mode: AlphaMode::Add,
            unlit:      true,
            ..default()
        });
        let atm_mesh = meshes.add(Mesh::from(Cuboid::new(
            cfg.atmosphere_voxel_size,
            cfg.atmosphere_voxel_size,
            cfg.atmosphere_voxel_size,
        )));

        for ai in 0..cfg.atmosphere_count {
            let h1 = pseudo_hash(cfg.seed as f32 + 10.0, ai as f32);
            let h2 = pseudo_hash(cfg.seed as f32 + 11.0, ai as f32);
            let h3 = pseudo_hash(cfg.seed as f32 + 12.0, ai as f32);

            let theta = h1 * std::f32::consts::TAU;
            let phi   = (h2 * 2.0 - 1.0).clamp(-1.0, 1.0).acos();
            let r     = cfg.radius + h3 * cfg.atmosphere_thickness;

            let origin = Vec3::new(
                r * phi.sin() * theta.cos(),
                r * phi.cos(),
                r * phi.sin() * theta.sin(),
            );

            let av = commands.spawn((
                Mesh3d(atm_mesh.clone()),
                MeshMaterial3d(atm_mat.clone()),
                Transform::from_translation(snap_grid(origin, cfg.atmosphere_voxel_size)),
                Visibility::default(),
                NotShadowCaster,
                NeutronAtmosphere {
                    idx, origin: snap_grid(origin, cfg.atmosphere_voxel_size),
                    seed: pseudo_hash(h1, h2), theta, phi,
                },
            )).id();
            commands.entity(root).add_child(av);
        }
    }

    // ── Halo X ────────────────────────────────────────────────────────────
    if cfg.xray_halo_enabled {
        let [xr, xg, xb] = cfg.xray_halo_color;
        let xe = cfg.xray_halo_emissive;
        let xray_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(xr, xg, xb),
            emissive:   LinearRgba::new(xr * xe, xg * xe, xb * xe, 1.0),
            alpha_mode: AlphaMode::Add,
            unlit:      true,
            ..default()
        });
        let xray_mesh = meshes.add(Mesh::from(Cuboid::new(
            cfg.xray_halo_voxel_size,
            cfg.xray_halo_voxel_size,
            cfg.xray_halo_voxel_size,
        )));

        for xi in 0..cfg.xray_halo_count {
            let h1 = pseudo_hash(cfg.seed as f32 + 20.0, xi as f32);
            let h2 = pseudo_hash(cfg.seed as f32 + 21.0, xi as f32);
            let h3 = pseudo_hash(cfg.seed as f32 + 22.0, xi as f32);

            let theta = h1 * std::f32::consts::TAU;
            let phi   = (h2 * 2.0 - 1.0).clamp(-1.0, 1.0).acos();
            let r     = cfg.radius + cfg.atmosphere_thickness
                + h3.sqrt() * (cfg.xray_halo_radius - cfg.radius - cfg.atmosphere_thickness);

            let xv = commands.spawn((
                Mesh3d(xray_mesh.clone()),
                MeshMaterial3d(xray_mat.clone()),
                Transform::from_translation(Vec3::ZERO),
                Visibility::default(),
                NotShadowCaster,
                XrayHaloVoxel { idx, theta, phi, r, seed: pseudo_hash(h1, h3) },
            )).id();
            commands.entity(root).add_child(xv);
        }
    }

    // ── Fissures de starquake (spawned, cachées au départ) ────────────────
    if cfg.starquake_enabled {
        let sq_mesh = meshes.add(Mesh::from(Cuboid::new(
            cfg.starquake_voxel_size,
            cfg.starquake_voxel_size,
            cfg.starquake_voxel_size,
        )));
        let [sqr, sqg, sqb] = cfg.starquake_color;
        let sqe = cfg.starquake_emissive;
        let sq_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(sqr, sqg, sqb),
            emissive:   LinearRgba::new(sqr * sqe, sqg * sqe, sqb * sqe, 1.0),
            alpha_mode: AlphaMode::Add,
            unlit:      true,
            ..default()
        });

        for fi in 0..cfg.starquake_count {
            let seed = pseudo_hash(cfg.seed as f32 + 30.0, fi as f32);
            let lat  = pseudo_hash(seed, 1.0) * std::f32::consts::PI - std::f32::consts::FRAC_PI_2;
            let lon  = pseudo_hash(seed, 2.0) * std::f32::consts::TAU;

            let fv = commands.spawn((
                Mesh3d(sq_mesh.clone()),
                MeshMaterial3d(sq_mat.clone()),
                Transform::from_translation(Vec3::ZERO).with_scale(Vec3::ZERO),
                Visibility::Hidden,
                NotShadowCaster,
                StarquakeFissure { idx, fissure_idx: fi, seed, lat, lon },
            )).id();
            commands.entity(root).add_child(fv);
        }
    }

    // ── Disque d'accrétion (binaire) ──────────────────────────────────────
    if is_binary_active(cfg) {
        if let Some((_, _, _, _, _, disk_inner, disk_outer)) = binary_params(cfg) {
            let disk_mesh = meshes.add(Mesh::from(Cuboid::new(
                cfg.disk_voxel_size,
                cfg.disk_voxel_size * 0.3,
                cfg.disk_voxel_size,
            )));

            for di in 0..cfg.disk_count {
                let h1 = pseudo_hash(cfg.seed as f32 + 40.0, di as f32);
                let h2 = pseudo_hash(cfg.seed as f32 + 41.0, di as f32);
                let h3 = pseudo_hash(cfg.seed as f32 + 42.0, di as f32);
                let h4 = pseudo_hash(cfg.seed as f32 + 43.0, di as f32);

                let angle   = h1 * std::f32::consts::TAU;
                let radius  = disk_inner + h2.powf(0.5) * (disk_outer - disk_inner);
                let height  = (h3 * 2.0 - 1.0) * cfg.disk_thickness * 0.5;
                let color_t = ((radius - disk_inner) / (disk_outer - disk_inner)).clamp(0.0, 1.0);

                let [dr, dg, db] = lerp_color(cfg.disk_color_inner, cfg.disk_color_outer, color_t);
                let de = cfg.disk_emissive * (1.0 - color_t * 0.55);

                let disk_mat = materials.add(StandardMaterial {
                    base_color: Color::srgb(dr, dg, db),
                    emissive:   LinearRgba::new(dr * de, dg * de, db * de, 1.0),
                    alpha_mode: AlphaMode::Add,
                    unlit:      true,
                    ..default()
                });

                let pos = Vec3::new(angle.cos() * radius, height, angle.sin() * radius);
                let dv = commands.spawn((
                    Mesh3d(disk_mesh.clone()),
                    MeshMaterial3d(disk_mat),
                    Transform::from_translation(snap_grid(pos, cfg.disk_voxel_size)),
                    Visibility::default(),
                    NotShadowCaster,
                    AccretionDiskVoxel { idx, angle, radius, height, seed: h4, color_t },
                )).id();
                commands.entity(root).add_child(dv);
            }
        }

        // ── Colonne d'accrétion polaire ────────────────────────────────────
        if cfg.accretion_column_enabled {
            let col_mesh = meshes.add(Mesh::from(Cuboid::new(
                cfg.column_voxel_size,
                cfg.column_voxel_size,
                cfg.column_voxel_size,
            )));
            let [cor, cog, cob] = cfg.column_color;
            let coe = cfg.column_emissive;
            let col_mat = materials.add(StandardMaterial {
                base_color: Color::srgb(cor, cog, cob),
                emissive:   LinearRgba::new(cor * coe, cog * coe, cob * coe, 1.0),
                alpha_mode: AlphaMode::Add,
                unlit:      true,
                ..default()
            });

            for pole in [1.0_f32, -1.0_f32] {
                for ci in 0..cfg.column_count {
                    let h1 = pseudo_hash(cfg.seed as f32 + 50.0 + pole * 10.0, ci as f32);
                    let h2 = pseudo_hash(cfg.seed as f32 + 51.0 + pole * 10.0, ci as f32);
                    let t  = h1;
                    let cv = commands.spawn((
                        Mesh3d(col_mesh.clone()),
                        MeshMaterial3d(col_mat.clone()),
                        Transform::from_translation(Vec3::ZERO),
                        Visibility::default(),
                        NotShadowCaster,
                        AccretionColumnVoxel { idx, pole, t, perp_seed: h2, seed: pseudo_hash(h1, h2) },
                    )).id();
                    commands.entity(root).add_child(cv);
                }
            }
        }

        // ── Flux d'accrétion compagnon → étoile ───────────────────────────
        let stream_mesh = meshes.add(Mesh::from(Cuboid::new(
            cfg.stream_voxel_size,
            cfg.stream_voxel_size,
            cfg.stream_voxel_size,
        )));
        let [str_r, str_g, str_b] = cfg.stream_color;
        let str_e = cfg.stream_emissive;
        let stream_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(str_r, str_g, str_b),
            emissive:   LinearRgba::new(str_r * str_e, str_g * str_e, str_b * str_e, 1.0),
            alpha_mode: AlphaMode::Add,
            unlit:      true,
            ..default()
        });

        for si in 0..cfg.stream_count {
            let h1 = pseudo_hash(cfg.seed as f32 + 60.0, si as f32);
            let h2 = pseudo_hash(cfg.seed as f32 + 61.0, si as f32);
            let h3 = pseudo_hash(cfg.seed as f32 + 62.0, si as f32);
            let t  = h1;
            let offset = Vec2::new(h2 * 2.0 - 1.0, h3 * 2.0 - 1.0);

            let sv = commands.spawn((
                Mesh3d(stream_mesh.clone()),
                MeshMaterial3d(stream_mat.clone()),
                Transform::from_translation(Vec3::ZERO),
                Visibility::default(),
                NotShadowCaster,
                AccretionStream { idx, t, seed: pseudo_hash(h2, h3), offset },
            )).id();
            commands.entity(root).add_child(sv);
        }

        // ── Compagnon stellaire ────────────────────────────────────────────
        if let Some((comp_dist, comp_r, comp_color, _, _, _, _)) = binary_params(cfg) {
            let [cr, cg, cb] = comp_color;
            let comp_mat = materials.add(StandardMaterial {
                base_color: Color::srgb(cr, cg, cb),
                emissive:   LinearRgba::new(cr * 2.0, cg * 2.0, cb * 2.0, 1.0),
                perceptual_roughness: 0.8,
                unlit: false,
                ..default()
            });
            let comp_mesh = meshes.add(Mesh::from(Sphere::new(comp_r)));

            let comp = commands.spawn((
                Mesh3d(comp_mesh),
                MeshMaterial3d(comp_mat),
                Transform::from_translation(Vec3::new(comp_dist, 0.0, 0.0)),
                Visibility::default(),
                NotShadowCaster,
                CompanionStar { idx },
            )).id();
            commands.entity(root).add_child(comp);
        }
    }
}

// ─────────────────────────────────────────────
//  Orbite & Spin
// ─────────────────────────────────────────────

fn orbit_neutron_stars(
    time:       Res<Time>,
    res:        Res<NeutronStarRes>,
    mut root_q: Query<(&mut Transform, &NeutronStarRoot)>,
) {
    let t = time.elapsed_secs();
    for (mut tf, root) in &mut root_q {
        let Some(cfg) = res.stars.get(root.idx) else { continue; };
        if cfg.orbit_distance > 1.0 {
            let a = t * cfg.orbit_speed;
            tf.translation.x = a.cos() * cfg.orbit_distance;
            tf.translation.z = a.sin() * cfg.orbit_distance;
        }
    }
}

fn spin_neutron_star(
    time:       Res<Time>,
    res:        Res<NeutronStarRes>,
    mut state:  ResMut<NeutronStarState>,
    mut body_q: Query<(&mut Transform, &NeutronStarBody)>,
) {
    let dt = time.delta_secs();
    for (mut tf, body) in &mut body_q {
        let Some(cfg) = res.stars.get(body.idx) else { continue; };
        while state.spin_angles.len() <= body.idx { state.spin_angles.push(0.0); }
        state.spin_angles[body.idx] =
            (state.spin_angles[body.idx] + cfg.spin_speed * dt) % std::f32::consts::TAU;
        tf.rotation = Quat::from_rotation_y(state.spin_angles[body.idx]);
    }
}

// ─────────────────────────────────────────────
//  Cooling curve
// ─────────────────────────────────────────────

fn tick_cooling(
    time:    Res<Time>,
    res:     Res<NeutronStarRes>,
    mut state: ResMut<NeutronStarState>,
) {
    let dt = time.delta_secs();
    for (idx, cfg) in res.stars.iter().enumerate() {
        while state.ages.len()         <= idx { state.ages.push(0.0); }
        while state.temperatures.len() <= idx { state.temperatures.push(cfg.temp_initial); }

        state.ages[idx] += dt;
        state.temperatures[idx] = cooled_temp(cfg, state.ages[idx]);
    }
}

/// Met à jour la couleur de chaque voxel de surface selon la température courante
fn update_surface_temperature(
    res:      Res<NeutronStarRes>,
    state:    Res<NeutronStarState>,
    surf_q:   Query<(&NeutronSurface, &MeshMaterial3d<StandardMaterial>)>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    for (ns, mat_handle) in &surf_q {
        let Some(cfg) = res.stars.get(ns.idx) else { continue; };
        let temp = state.temperatures.get(ns.idx).copied().unwrap_or(cfg.temp_initial);

        // Variation locale : les pôles sont plus chauds (chauffage magnétique)
        let pole_boost = ns.lat.abs() / std::f32::consts::FRAC_PI_2;
        let local_temp = (temp + pole_boost * 0.15 * temp).min(1.0);

        let [r, g, b] = lerp_color(cfg.color_cold, cfg.color_hot, local_temp);
        let emissive  = cfg.surface_emissive_cold
            + local_temp * (cfg.surface_emissive_hot - cfg.surface_emissive_cold);

        if let Some(mat) = mats.get_mut(&mat_handle.0) {
            mat.base_color = Color::srgb(r, g, b);
            mat.emissive   = LinearRgba::new(r * emissive, g * emissive, b * emissive, 1.0);
        }
    }
}

// ─────────────────────────────────────────────
//  Atmosphère
// ─────────────────────────────────────────────

fn animate_atmosphere(
    time:      Res<Time>,
    res:       Res<NeutronStarRes>,
    state:     Res<NeutronStarState>,
    mut atm_q: Query<(&NeutronAtmosphere, &mut Transform)>,
) {
    let t = time.elapsed_secs();
    for (av, mut tf) in &mut atm_q {
        let Some(cfg) = res.stars.get(av.idx) else { continue; };
        let temp = state.temperatures.get(av.idx).copied().unwrap_or(1.0);

        // L'atmosphère pulse plus vite quand l'étoile est chaude
        let pulse_speed = 0.8 + temp * 2.5;
        let pulse = 1.0 + (t * pulse_speed + av.seed * std::f32::consts::TAU).sin() * 0.18;

        // Légère dérive tangentielle
        let theta_anim = av.theta + t * 0.08 * (1.0 + av.seed);
        let r = cfg.radius + cfg.atmosphere_thickness * (0.3 + av.seed * 0.7)
            + (t * 0.4 + av.seed * 7.0).sin() * cfg.atmosphere_thickness * 0.15;

        let pos = Vec3::new(
            r * av.phi.sin() * theta_anim.cos(),
            r * av.phi.cos(),
            r * av.phi.sin() * theta_anim.sin(),
        );

        tf.translation = snap_grid(pos, cfg.atmosphere_voxel_size);
        tf.scale = Vec3::splat((pulse * temp.max(0.1)).max(0.05));
    }
}

// ─────────────────────────────────────────────
//  Halo X
// ─────────────────────────────────────────────

fn animate_xray_halo(
    time:       Res<Time>,
    res:        Res<NeutronStarRes>,
    state:      Res<NeutronStarState>,
    mut xray_q: Query<(&XrayHaloVoxel, &mut Transform, &mut Visibility)>,
) {
    let t = time.elapsed_secs();
    for (xv, mut tf, mut vis) in &mut xray_q {
        let Some(cfg) = res.stars.get(xv.idx) else { continue; };
        let temp = state.temperatures.get(xv.idx).copied().unwrap_or(1.0);

        // Halo disparaît progressivement avec le refroidissement
        if temp < cfg.temp_floor * 2.0 {
            *vis = Visibility::Hidden;
            continue;
        }

        let pulse = 1.0 + (t * cfg.xray_pulse_speed + xv.seed * std::f32::consts::TAU).sin()
            * 0.3 * temp;
        let r = xv.r * pulse;

        let pos = Vec3::new(
            r * xv.phi.sin() * xv.theta.cos(),
            r * xv.phi.cos(),
            r * xv.phi.sin() * xv.theta.sin(),
        );

        tf.translation = snap_grid(pos, cfg.xray_halo_voxel_size);
        tf.scale = Vec3::splat((0.3 + temp * 0.9).max(0.05));
        *vis = Visibility::Visible;
    }
}

// ─────────────────────────────────────────────
//  QPO (oscillations quasi-périodiques)
// ─────────────────────────────────────────────

/// Retourne le déplacement QPO total (somme de N modes)
fn qpo_displacement(cfg: &NeutronStarConfig, state: &NeutronStarState, idx: usize) -> Vec3 {
    if !cfg.qpo_enabled { return Vec3::ZERO; }
    let phases = match state.qpo_phases.get(idx) {
        Some(p) => p,
        None    => return Vec3::ZERO,
    };

    let mut disp = Vec3::ZERO;
    for (i, &freq) in cfg.qpo_frequencies.iter().enumerate() {
        let phase = phases.get(i).copied().unwrap_or(0.0);
        // Chaque mode oscille dans une direction différente
        let axis = Vec3::new(
            (i as f32 * 2.3 + 1.0).sin(),
            (i as f32 * 3.7).cos(),
            (i as f32 * 1.9 + 0.5).sin(),
        ).normalize();
        disp += axis * phase.sin() * cfg.qpo_amplitude
            / (i as f32 + 1.0).sqrt(); // modes supérieurs moins intenses
    }
    disp
}

fn tick_qpo_oscillations(
    time:     Res<Time>,
    res:      Res<NeutronStarRes>,
    mut state: ResMut<NeutronStarState>,
) {
    let dt = time.delta_secs();
    for (idx, cfg) in res.stars.iter().enumerate() {
        if !cfg.qpo_enabled { continue; }
        while state.qpo_phases.len() <= idx {
            state.qpo_phases.push(vec![0.0; cfg.qpo_frequencies.len()]);
        }
        let phases = &mut state.qpo_phases[idx];
        while phases.len() < cfg.qpo_frequencies.len() { phases.push(0.0); }
        for (i, &freq) in cfg.qpo_frequencies.iter().enumerate() {
            phases[i] += freq * dt * std::f32::consts::TAU;
        }
    }
}

// ─────────────────────────────────────────────
//  Starquakes
// ─────────────────────────────────────────────

fn tick_starquakes(
    time:    Res<Time>,
    res:     Res<NeutronStarRes>,
    mut state: ResMut<NeutronStarState>,
    root_q:  Query<(&GlobalTransform, &NeutronStarRoot)>,
    mut sq_events: EventWriter<StarquakeEvent>,
) {
    let dt = time.delta_secs();
    for (idx, cfg) in res.stars.iter().enumerate() {
        if !cfg.starquake_enabled { continue; }

        while state.starquake_timers.len()  <= idx { state.starquake_timers.push(0.0); }
        while state.starquake_active.len()  <= idx { state.starquake_active.push(false); }
        while state.starquake_elapsed.len() <= idx { state.starquake_elapsed.push(0.0); }

        if state.starquake_active[idx] {
            state.starquake_elapsed[idx] += dt;
            if state.starquake_elapsed[idx] >= cfg.starquake_duration {
                state.starquake_active[idx]  = false;
                state.starquake_elapsed[idx] = 0.0;
                state.starquake_timers[idx]  = 0.0;
            }
        } else {
            state.starquake_timers[idx] += dt;
            if state.starquake_timers[idx] >= cfg.starquake_interval {
                state.starquake_active[idx] = true;
                let origin = root_q
                    .iter()
                    .find(|(_, r)| r.idx == idx)
                    .map(|(gt, _)| gt.translation())
                    .unwrap_or_default();
                let magnitude = 0.5 + pseudo_hash(idx as f32, state.ages.get(idx).copied().unwrap_or(0.0)) * 0.5;
                sq_events.send(StarquakeEvent { star_idx: idx, origin, magnitude });
            }
        }
    }
}

fn animate_starquake_fissures(
    res:      Res<NeutronStarRes>,
    state:    Res<NeutronStarState>,
    mut fq:   Query<(&StarquakeFissure, &mut Transform, &mut Visibility)>,
) {
    for (fv, mut tf, mut vis) in &mut fq {
        let Some(cfg) = res.stars.get(fv.idx) else { *vis = Visibility::Hidden; continue; };

        let active  = state.starquake_active.get(fv.idx).copied().unwrap_or(false);
        let elapsed = state.starquake_elapsed.get(fv.idx).copied().unwrap_or(0.0);

        if !active { *vis = Visibility::Hidden; continue; }

        // Progression de la fissure : s'ouvre puis se referme
        let life = if elapsed < cfg.starquake_duration * 0.3 {
            elapsed / (cfg.starquake_duration * 0.3)
        } else {
            1.0 - (elapsed - cfg.starquake_duration * 0.3)
                / (cfg.starquake_duration * 0.7)
        }.clamp(0.0, 1.0);

        // Position sur la surface + QPO displacement
        let qpo = qpo_displacement(cfg, &state, fv.idx);
        let base_pos = sphere_pos(fv.lat, fv.lon, cfg.radius);
        let pos = base_pos + qpo * life;

        tf.translation = snap_grid(pos, cfg.starquake_voxel_size);

        // Éclat brillant et scale
        let flicker = (elapsed * 18.0 + fv.seed * 9.0).sin() * 0.4 + 0.6;
        tf.scale = Vec3::splat((life * flicker * 1.8).max(0.05));
        *vis = Visibility::Visible;
    }
}

// ─────────────────────────────────────────────
//  Disque d'accrétion
// ─────────────────────────────────────────────

fn animate_accretion_disk(
    time:      Res<Time>,
    res:       Res<NeutronStarRes>,
    mut disk_q: Query<(&AccretionDiskVoxel, &mut Transform)>,
) {
    let t = time.elapsed_secs();
    for (dv, mut tf) in &mut disk_q {
        let Some(cfg) = res.stars.get(dv.idx) else { continue; };
        if !is_binary_active(cfg) { continue; }

        // Vitesse de Kepler différentielle
        if let Some((_, _, _, _, _, disk_inner, _)) = binary_params(cfg) {
            let kepler = cfg.disk_rotation_speed
                * (disk_inner / dv.radius.max(1.0)).sqrt();
            let angle  = dv.angle + t * kepler;

            let turb = (t * 1.8 + dv.seed * std::f32::consts::TAU).sin()
                * cfg.disk_turbulence * (1.0 - dv.color_t);

            let pos = Vec3::new(
                angle.cos() * dv.radius,
                dv.height + turb,
                angle.sin() * dv.radius,
            );

            tf.translation = snap_grid(pos, cfg.disk_voxel_size);

            let pulse = 1.0 + (t * (2.0 + dv.color_t * 3.0) + dv.seed * 6.28).sin() * 0.15;
            let size  = (0.4 + dv.color_t * 0.8) * pulse;
            tf.scale  = Vec3::splat(size.max(0.05));
        }
    }
}

// ─────────────────────────────────────────────
//  Colonne d'accrétion polaire
// ─────────────────────────────────────────────

fn animate_accretion_column(
    time:      Res<Time>,
    res:       Res<NeutronStarRes>,
    state:     Res<NeutronStarState>,
    mut col_q: Query<(&AccretionColumnVoxel, &mut Transform, &mut Visibility)>,
) {
    let t = time.elapsed_secs();
    for (cv, mut tf, mut vis) in &mut col_q {
        let Some(cfg) = res.stars.get(cv.idx) else { *vis = Visibility::Hidden; continue; };
        if !is_binary_active(cfg) || !cfg.accretion_column_enabled {
            *vis = Visibility::Hidden;
            continue;
        }

        let spin_angle = state.spin_angles.get(cv.idx).copied().unwrap_or(0.0);

        // Colonne alignée sur le pôle magnétique (légère inclinaison)
        let tilt = 0.15_f32;
        let pole_dir = Vec3::new(
            tilt.sin() * spin_angle.cos(),
            tilt.cos(),
            tilt.sin() * spin_angle.sin(),
        ).normalize() * cv.pole;

        // Le voxel monte le long de la colonne en boucle
        let local_t = ((t * cfg.column_speed + cv.t) % 1.0).max(0.0);
        let dist = cfg.radius + local_t * cfg.column_length;

        // Étalement conique
        let cone_w = local_t * cfg.column_width * 0.5;
        let perp_angle = cv.perp_seed * std::f32::consts::TAU + t * cfg.column_speed * 0.4;
        let perp_base = if pole_dir.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
        let right = pole_dir.cross(perp_base).normalize();
        let fwd   = pole_dir.cross(right).normalize();

        let pos = pole_dir * dist
            + right * (perp_angle.cos() * cone_w)
            + fwd   * (perp_angle.sin() * cone_w);

        tf.translation = snap_grid(pos, cfg.column_voxel_size);

        let fade = (1.0 - local_t * 0.8).max(0.1);
        tf.scale = Vec3::splat(fade * (0.7 + cv.seed * 0.5));
        *vis = Visibility::Visible;
    }
}

// ─────────────────────────────────────────────
//  Flux d'accrétion compagnon → étoile
// ─────────────────────────────────────────────

fn animate_accretion_stream(
    time:        Res<Time>,
    res:         Res<NeutronStarRes>,
    mut stream_q: Query<(&AccretionStream, &mut Transform, &mut Visibility)>,
    comp_q:      Query<(&GlobalTransform, &CompanionStar)>,
    root_q:      Query<(&GlobalTransform, &NeutronStarRoot)>,
) {
    let t = time.elapsed_secs();
    for (sv, mut tf, mut vis) in &mut stream_q {
        let Some(cfg) = res.stars.get(sv.idx) else { *vis = Visibility::Hidden; continue; };
        if !is_binary_active(cfg) { *vis = Visibility::Hidden; continue; }

        // Positions en world space
        let star_pos = root_q
            .iter()
            .find(|(_, r)| r.idx == sv.idx)
            .map(|(gt, _)| gt.translation())
            .unwrap_or_default();
        let comp_pos = comp_q
            .iter()
            .find(|(_, c)| c.idx == sv.idx)
            .map(|(gt, _)| gt.translation())
            .unwrap_or(star_pos + Vec3::X * 300.0);

        // Flux : Lagrange L1 — point de transfert de masse
        // Simplifié : courbe de Bézier avec courbure orbitale
        let mid = (star_pos + comp_pos) * 0.5
            + Vec3::Y * (comp_pos - star_pos).length() * 0.12;

        // Animation : t avance dans le temps (voxel glisse vers l'étoile)
        let local_t = ((sv.t - t * 0.3 * cfg.disk_rotation_speed).rem_euclid(1.0)).clamp(0.0, 1.0);

        let inv  = 1.0 - local_t;
        let pos  = comp_pos * inv * inv + mid * 2.0 * inv * local_t + star_pos * local_t * local_t;

        // Décalage perpendiculaire (épaisseur du flux)
        let stream_dir = (star_pos - comp_pos).normalize_or_zero();
        let perp = if stream_dir.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
        let right = stream_dir.cross(perp).normalize();
        let width_scale = cfg.stream_voxel_size * 1.5;

        let final_pos = pos
            + right * sv.offset.x * width_scale
            + Vec3::Y * sv.offset.y * width_scale * 0.4;

        tf.translation = snap_grid(final_pos - star_pos, cfg.stream_voxel_size);

        let scale = 0.5 + (1.0 - local_t) * 0.8
            + (t * 2.0 + sv.seed * 5.0).sin() * 0.1;
        tf.scale = Vec3::splat(scale.max(0.05));
        *vis = Visibility::Visible;
    }
}

// ─────────────────────────────────────────────
//  Orbite du compagnon (binaire)
// ─────────────────────────────────────────────

fn orbit_companion(
    time:      Res<Time>,
    res:       Res<NeutronStarRes>,
    mut comp_q: Query<(&mut Transform, &CompanionStar)>,
) {
    let t = time.elapsed_secs();
    for (mut tf, comp) in &mut comp_q {
        let Some(cfg) = res.stars.get(comp.idx) else { continue; };
        if let Some((dist, _, _, speed, _, _, _)) = binary_params(cfg) {
            let angle = t * speed;
            tf.translation.x = angle.cos() * dist;
            tf.translation.z = angle.sin() * dist;
        }
    }
}

// ─────────────────────────────────────────────
//  Régénération à chaud
// ─────────────────────────────────────────────

fn reload_neutron_stars(
    mut commands:  Commands,
    mut events:    EventReader<ReloadAstre>,
    res:           Res<NeutronStarRes>,
    roots:         Query<(Entity, &NeutronStarRoot)>,
    mut meshes:    ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for ev in events.read() {
        let Ok((entity, root)) = roots.get(ev.0) else { continue };
        let idx = root.idx;
        let Some(cfg) = res.stars.get(idx) else { continue };
        commands.entity(entity).despawn_recursive();
        build_neutron_star(&mut commands, cfg, idx, &mut meshes, &mut materials);
    }
}

fn regenerate_neutron_stars(
    mut commands:  Commands,
    mut events:    EventReader<RegenerateNeutronStar>,
    res:           Res<NeutronStarRes>,
    mut state:     ResMut<NeutronStarState>,
    root_q:        Query<Entity, With<NeutronStarRoot>>,
    mut meshes:    ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut fired = false;
    for _ in events.read() { fired = true; }
    if !fired { return; }

    for entity in &root_q {
        commands.entity(entity).despawn_recursive();
    }

    state.temperatures.clear();
    state.ages.clear();
    state.spin_angles.clear();
    state.starquake_timers.clear();
    state.starquake_active.clear();
    state.starquake_elapsed.clear();
    state.qpo_phases.clear();

    for (idx, cfg) in res.stars.iter().enumerate() {
        state.temperatures.push(cfg.temp_initial);
        state.ages.push(0.0);
        state.spin_angles.push(0.0);
        state.starquake_timers.push(0.0);
        state.starquake_active.push(false);
        state.starquake_elapsed.push(0.0);
        state.qpo_phases.push(vec![0.0; cfg.qpo_frequencies.len()]);
        build_neutron_star(&mut commands, cfg, idx, &mut meshes, &mut materials);
    }
}
