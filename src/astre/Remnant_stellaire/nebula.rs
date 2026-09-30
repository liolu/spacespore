use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;
use crate::astre::{AstreLodRoot, ReloadAstre};
use noise::{NoiseFn, Perlin, Fbm, MultiFractal};

// ─────────────────────────────────────────────
//  Constantes de base (overridables via config)
// ─────────────────────────────────────────────

/// Nombre max de voxels de gaz spawned par frame lors du build
const NEBULA_BUILD_BATCH: usize = 512;

/// Grille de snap pour les voxels (esthétique voxel)
const NEBULA_SNAP_GRID: f32 = 8.0;

// ─────────────────────────────────────────────
//  Config procédurale de la nébuleuse
// ─────────────────────────────────────────────

/// Toutes les options de génération — à brancher sur GameSettings plus tard.
#[derive(Clone, Debug)]
pub struct NebulaConfig {
    // --- Position & taille ---
    pub position:           Vec3,
    pub radius:             f32,   // rayon global du volume

    // --- Seed & noise ---
    pub seed:               u32,
    /// Fréquence du bruit de forme principale (plus petit = plus grand nuage)
    pub shape_frequency:    f64,
    /// Fréquence du bruit de détail / filaments
    pub detail_frequency:   f64,
    /// Octaves du bruit fractal
    pub octaves:            usize,
    /// Seuil de densité : valeur de bruit au-delà de laquelle un voxel existe
    pub density_threshold:  f64,
    /// Force du warp de domaine (torsion des filaments, 0 = aucun)
    pub warp_strength:      f32,

    // --- Couleurs ---
    /// Couleur intérieure des nuages denses (gaz chaud)
    pub color_core:         [f32; 3],
    /// Couleur des bords diffus (nébuleuse froide)
    pub color_edge:         [f32; 3],
    /// Couleur des filaments (zones de haute fréquence)
    pub color_filament:     [f32; 3],
    /// Émissivité globale (intensité de la lueur)
    pub emissive_strength:  f32,

    // --- Géométrie voxels ---
    /// Taille de base des voxels de gaz dense
    pub voxel_size_core:    f32,
    /// Taille de base des voxels diffus (bords)
    pub voxel_size_edge:    f32,
    /// Résolution de l'échantillonnage (nombre de points par axe)
    pub resolution:         u32,

    // --- Filaments ---
    /// Active les filaments haute fréquence
    pub filaments_enabled:  bool,
    /// Seuil séparé pour les filaments (souvent plus haut que density_threshold)
    pub filament_threshold: f64,
    /// Taille des voxels filaments
    pub voxel_size_filament: f32,

    // --- Étoiles internes ---
    /// Nombre d'étoiles nées dans la nébuleuse
    pub inner_stars_count:  u32,
    /// Rayon des étoiles internes (visuelles, pas de lumière lourde)
    pub inner_star_radius:  f32,
    /// Couleur des étoiles internes
    pub inner_star_color:   [f32; 3],
    /// Émissivité des étoiles internes
    pub inner_star_emissive: f32,

    // --- Animation ---
    /// Vitesse de dérive lente des voxels (effet nébuleuse vivante)
    pub drift_speed:        f32,
    /// Amplitude de la dérive
    pub drift_amplitude:    f32,
    /// Active le scintillement des voxels de bord
    pub shimmer_enabled:    bool,
    pub shimmer_speed:      f32,
    pub shimmer_amplitude:  f32,
}

impl Default for NebulaConfig {
    fn default() -> Self {
        Self {
            position:            Vec3::new(0.0, 0.0, -2000.0),
            radius:              600.0,
            seed:                42,
            shape_frequency:     0.003,
            detail_frequency:    0.012,
            octaves:             5,
            density_threshold:   0.08,
            warp_strength:       80.0,
            color_core:          [0.6, 0.15, 0.9],   // violet chaud
            color_edge:          [0.05, 0.35, 0.8],  // bleu froid
            color_filament:      [1.0, 0.6, 0.2],    // orange filaments
            emissive_strength:   3.5,
            voxel_size_core:     24.0,
            voxel_size_edge:     40.0,
            resolution:          48,
            filaments_enabled:   true,
            filament_threshold:  0.35,
            voxel_size_filament: 10.0,
            inner_stars_count:   12,
            inner_star_radius:   6.0,
            inner_star_color:    [1.0, 0.9, 0.7],
            inner_star_emissive: 8.0,
            drift_speed:         0.04,
            drift_amplitude:     5.0,
            shimmer_enabled:     true,
            shimmer_speed:       1.2,
            shimmer_amplitude:   0.15,
        }
    }
}

// ─────────────────────────────────────────────
//  Plugin
// ─────────────────────────────────────────────

pub struct NebulaPlugin;

impl Plugin for NebulaPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<NebulaRes>()
            .add_event::<RegenerateNebula>()
            .add_systems(Startup, spawn_nebula)
            .add_systems(Update, (
                animate_nebula_voxels,
                regenerate_nebula,
                reload_nebula_stream,
            ));
    }
}

// ─────────────────────────────────────────────
//  Resource & Events
// ─────────────────────────────────────────────

/// Resource globale — config courante de la nébuleuse.
/// À remplacer par un champ dans GameSettings plus tard.
#[derive(Resource)]
pub struct NebulaRes {
    pub enabled: bool,
    pub config: NebulaConfig,
}

impl Default for NebulaRes {
    fn default() -> Self {
        Self { enabled: true, config: NebulaConfig { position: Vec3::new(0.0, 0.0, -12000.0), ..Default::default() } }
    }
}

#[derive(Event)]
pub struct RegenerateNebula;

// ─────────────────────────────────────────────
//  Composants ECS
// ─────────────────────────────────────────────

#[derive(Component)]
pub struct NebulaRoot;

/// Voxel de gaz dense (centre de la nébuleuse)
#[derive(Component)]
pub struct NebulaVoxelCore {
    pub origin:     Vec3,   // position de base (avant animation)
    pub drift_seed: f32,    // seed perso pour la dérive
}

/// Voxel de bord diffus
#[derive(Component)]
pub struct NebulaVoxelEdge {
    pub origin:     Vec3,
    pub drift_seed: f32,
}

/// Voxel filament
#[derive(Component)]
pub struct NebulaVoxelFilament {
    pub origin:     Vec3,
    pub drift_seed: f32,
}

/// Étoile interne à la nébuleuse
#[derive(Component)]
pub struct NebulaInnerStar;

// ─────────────────────────────────────────────
//  Helpers bruit / math
// ─────────────────────────────────────────────

fn pseudo_hash(a: f32, b: f32) -> f32 {
    ((a * 12.9898 + b * 78.233).sin() * 43758.5453).fract()
}

fn snap(v: Vec3, grid: f32) -> Vec3 {
    Vec3::new(
        (v.x / grid).round() * grid,
        (v.y / grid).round() * grid,
        (v.z / grid).round() * grid,
    )
}

/// Retourne (density_shape, density_detail, warp_offset)
/// density_shape ∈ [-1, 1], density_detail ∈ [-1, 1]
fn sample_noise(
    fbm_shape:  &Fbm<Perlin>,
    fbm_detail: &Fbm<Perlin>,
    perlin_warp: &Perlin,
    p:          Vec3,
    cfg:        &NebulaConfig,
) -> (f64, f64) {
    let px = p.x as f64;
    let py = p.y as f64;
    let pz = p.z as f64;

    // Domain warp — tord l'espace pour créer des filaments courbes
    let wx = perlin_warp.get([px * cfg.shape_frequency, py * cfg.shape_frequency, pz * cfg.shape_frequency]);
    let wy = perlin_warp.get([px * cfg.shape_frequency + 3.7, py * cfg.shape_frequency, pz * cfg.shape_frequency]);
    let wz = perlin_warp.get([px * cfg.shape_frequency, py * cfg.shape_frequency + 7.1, pz * cfg.shape_frequency]);

    let w = cfg.warp_strength as f64;
    let warped = [px + wx * w, py + wy * w, pz + wz * w];

    let s = fbm_shape.get([
        warped[0] * cfg.shape_frequency,
        warped[1] * cfg.shape_frequency,
        warped[2] * cfg.shape_frequency,
    ]);

    let d = fbm_detail.get([
        warped[0] * cfg.detail_frequency,
        warped[1] * cfg.detail_frequency,
        warped[2] * cfg.detail_frequency,
    ]);

    (s, d)
}

/// Atténuation sphérique douce (1 au centre → 0 à la surface)
fn sphere_falloff(dist_norm: f32) -> f32 {
    let t = (1.0 - dist_norm).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t) // smoothstep
}

/// Interpole entre deux couleurs [r,g,b] selon t ∈ [0,1]
fn lerp_color(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

// ─────────────────────────────────────────────
//  Spawn principal
// ─────────────────────────────────────────────

fn spawn_nebula(
    mut commands:  Commands,
    nebula_res:    Res<NebulaRes>,
    mut meshes:    ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if !nebula_res.enabled {
        return;
    }
    build_nebula(&mut commands, &nebula_res.config, &mut meshes, &mut materials);
}

fn build_nebula(
    commands:  &mut Commands,
    cfg:       &NebulaConfig,
    meshes:    &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) {
    // --- Bruits ---
    let fbm_shape: Fbm<Perlin> = Fbm::<Perlin>::new(cfg.seed)
        .set_octaves(cfg.octaves)
        .set_frequency(1.0)
        .set_lacunarity(2.0)
        .set_persistence(0.5);

    let fbm_detail: Fbm<Perlin> = Fbm::<Perlin>::new(cfg.seed + 1)
        .set_octaves((cfg.octaves + 1).min(8))
        .set_frequency(1.0)
        .set_lacunarity(2.1)
        .set_persistence(0.45);

    let perlin_warp = Perlin::new(cfg.seed + 2);

    // --- Root entity ---
    let root = commands.spawn((
        Transform::from_translation(cfg.position),
        Visibility::default(),
        NebulaRoot,
        AstreLodRoot { cull_dist: 10000.0, radius: cfg.radius, streamable: true, label: "Nebula" },
    )).id();

    // --- Matériaux ---
    let [cr, cg, cb] = cfg.color_core;
    let [er, eg, eb] = cfg.color_edge;
    let [fr, fg, fb] = cfg.color_filament;
    let es = cfg.emissive_strength;

    let mat_core = materials.add(StandardMaterial {
        base_color: Color::srgb(cr, cg, cb),
        emissive:   LinearRgba::new(cr * es, cg * es, cb * es, 1.0),
        alpha_mode: AlphaMode::Add,
        unlit:      true,
        ..default()
    });

    let mat_edge = materials.add(StandardMaterial {
        base_color: Color::srgba(er, eg, eb, 0.55),
        emissive:   LinearRgba::new(er * es * 0.4, eg * es * 0.4, eb * es * 0.4, 1.0),
        alpha_mode: AlphaMode::Add,
        unlit:      true,
        ..default()
    });

    let mat_filament = materials.add(StandardMaterial {
        base_color: Color::srgb(fr, fg, fb),
        emissive:   LinearRgba::new(fr * es * 1.5, fg * es * 1.5, fb * es * 1.5, 1.0),
        alpha_mode: AlphaMode::Add,
        unlit:      true,
        ..default()
    });

    // --- Meshes voxels ---
    let mesh_core     = meshes.add(Mesh::from(Cuboid::new(cfg.voxel_size_core, cfg.voxel_size_core, cfg.voxel_size_core)));
    let mesh_edge     = meshes.add(Mesh::from(Cuboid::new(cfg.voxel_size_edge, cfg.voxel_size_edge, cfg.voxel_size_edge)));
    let mesh_filament = meshes.add(Mesh::from(Cuboid::new(cfg.voxel_size_filament, cfg.voxel_size_filament, cfg.voxel_size_filament)));

    let res = cfg.resolution;
    let _step = cfg.radius * 2.0 / res as f32;

    let mut voxel_idx: u32 = 0;

    for xi in 0..res {
        for yi in 0..res {
            for zi in 0..res {
                let lx = (xi as f32 + 0.5) / res as f32 * 2.0 - 1.0;
                let ly = (yi as f32 + 0.5) / res as f32 * 2.0 - 1.0;
                let lz = (zi as f32 + 0.5) / res as f32 * 2.0 - 1.0;

                let dist_norm = (lx * lx + ly * ly + lz * lz).sqrt();
                if dist_norm > 1.0 { continue; } // hors sphère

                let world_p = Vec3::new(lx, ly, lz) * cfg.radius;
                let (s_shape, s_detail) = sample_noise(
                    &fbm_shape, &fbm_detail, &perlin_warp, world_p, cfg
                );

                let falloff = sphere_falloff(dist_norm);
                // Combine shape + falloff : le centre dense, les bords érodés
                let effective_density = s_shape * falloff as f64;

                if effective_density < cfg.density_threshold { continue; }

                // Filament : zones de bruit détail élevé + density proche du seuil
                let is_filament = cfg.filaments_enabled
                    && s_detail > cfg.filament_threshold
                    && effective_density < cfg.density_threshold * 3.0;

                // Dense / diffus selon distance et bruit
                let is_core = effective_density > cfg.density_threshold * 2.5 && dist_norm < 0.6;

                let snapped = snap(world_p, NEBULA_SNAP_GRID);
                let drift_seed = pseudo_hash(voxel_idx as f32, 99.7);
                voxel_idx += 1;

                let voxel = if is_filament {
                    commands.spawn((
                        Mesh3d(mesh_filament.clone()),
                        MeshMaterial3d(mat_filament.clone()),
                        Transform::from_translation(snapped),
                        Visibility::default(),
                        NotShadowCaster,
                        NebulaVoxelFilament { origin: snapped, drift_seed },
                    )).id()
                } else if is_core {
                    commands.spawn((
                        Mesh3d(mesh_core.clone()),
                        MeshMaterial3d(mat_core.clone()),
                        Transform::from_translation(snapped),
                        Visibility::default(),
                        NotShadowCaster,
                        NebulaVoxelCore { origin: snapped, drift_seed },
                    )).id()
                } else {
                    commands.spawn((
                        Mesh3d(mesh_edge.clone()),
                        MeshMaterial3d(mat_edge.clone()),
                        Transform::from_translation(snapped),
                        Visibility::default(),
                        NotShadowCaster,
                        NebulaVoxelEdge { origin: snapped, drift_seed },
                    )).id()
                };

                commands.entity(root).add_child(voxel);
            }
        }
    }

    // --- Étoiles internes ---
    spawn_inner_stars(commands, cfg, meshes, materials, root);
}

// ─────────────────────────────────────────────
//  Étoiles internes
// ─────────────────────────────────────────────

fn spawn_inner_stars(
    commands:  &mut Commands,
    cfg:       &NebulaConfig,
    meshes:    &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    root:      Entity,
) {
    let [sr, sg, sb] = cfg.inner_star_color;
    let se = cfg.inner_star_emissive;

    let star_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        emissive:   LinearRgba::new(sr * se, sg * se, sb * se, 1.0),
        alpha_mode: AlphaMode::Add,
        unlit:      true,
        ..default()
    });
    let star_mesh = meshes.add(Mesh::from(Cuboid::new(
        cfg.inner_star_radius, cfg.inner_star_radius, cfg.inner_star_radius,
    )));

    for i in 0..cfg.inner_stars_count {
        // Distribution sphérique uniforme via hash
        let h1 = pseudo_hash(cfg.seed as f32 + i as f32, 1.0);
        let h2 = pseudo_hash(cfg.seed as f32 + i as f32, 2.0);
        let h3 = pseudo_hash(cfg.seed as f32 + i as f32, 3.0);

        let theta = h1 * std::f32::consts::TAU;
        let phi   = (h2 * 2.0 - 1.0).clamp(-1.0, 1.0).acos();
        // Les étoiles tendent vers le centre de la nébuleuse
        let r     = h3.powf(0.4) * cfg.radius * 0.65;

        let pos = Vec3::new(
            r * phi.sin() * theta.cos(),
            r * phi.cos(),
            r * phi.sin() * theta.sin(),
        );
        let snapped = snap(pos, NEBULA_SNAP_GRID);

        // Petite lumière ponctuelle sur chaque étoile interne
        let light_intensity = se * 80_000.0;
        let light = commands.spawn((
            PointLight {
                intensity:     light_intensity,
                range:         cfg.radius * 0.3,
                color:         Color::srgb(sr, sg, sb),
                shadows_enabled: false, // perf : pas d'ombres pour les étoiles de nébuleuse
                ..default()
            },
            Transform::from_translation(snapped),
        )).id();

        let star = commands.spawn((
            Mesh3d(star_mesh.clone()),
            MeshMaterial3d(star_mat.clone()),
            Transform::from_translation(snapped),
            Visibility::default(),
            NotShadowCaster,
            NebulaInnerStar,
        )).id();

        commands.entity(root).add_child(star);
        commands.entity(root).add_child(light);
    }
}

// ─────────────────────────────────────────────
//  Animation
// ─────────────────────────────────────────────

fn animate_nebula_voxels(
    time:     Res<Time>,
    nebula:   Res<NebulaRes>,
    mut core_q:     Query<(&NebulaVoxelCore,     &mut Transform), Without<NebulaVoxelEdge>>,
    mut edge_q:     Query<(&NebulaVoxelEdge,     &mut Transform), Without<NebulaVoxelCore>>,
    mut filament_q: Query<(&NebulaVoxelFilament, &mut Transform),
        (Without<NebulaVoxelCore>, Without<NebulaVoxelEdge>)>,
) {
    let cfg = &nebula.config;
    let t   = time.elapsed_secs();

    // Core : dérive lente et régulière
    for (vx, mut tf) in &mut core_q {
        let s = vx.drift_seed;
        let drift = Vec3::new(
            (t * cfg.drift_speed + s * 11.3).sin() * cfg.drift_amplitude,
            (t * cfg.drift_speed + s * 7.7).cos()  * cfg.drift_amplitude * 0.6,
            (t * cfg.drift_speed + s * 5.1).sin()  * cfg.drift_amplitude * 0.8,
        );
        tf.translation = snap(vx.origin + drift, NEBULA_SNAP_GRID);
    }

    // Edge : dérive + shimmer (scale pulsant)
    for (vx, mut tf) in &mut edge_q {
        let s = vx.drift_seed;
        let drift = Vec3::new(
            (t * cfg.drift_speed * 0.7 + s * 13.1).sin() * cfg.drift_amplitude * 1.4,
            (t * cfg.drift_speed * 0.7 + s * 9.3).cos()  * cfg.drift_amplitude,
            (t * cfg.drift_speed * 0.7 + s * 6.7).sin()  * cfg.drift_amplitude * 1.2,
        );
        tf.translation = snap(vx.origin + drift, NEBULA_SNAP_GRID);

        if cfg.shimmer_enabled {
            let shimmer = 1.0 + (t * cfg.shimmer_speed + s * std::f32::consts::TAU).sin()
                * cfg.shimmer_amplitude;
            tf.scale = Vec3::splat(shimmer);
        }
    }

    // Filaments : dérive rapide en spirale
    for (vx, mut tf) in &mut filament_q {
        let s = vx.drift_seed;
        let speed = cfg.drift_speed * 2.5;
        let drift = Vec3::new(
            (t * speed + s * 17.3).sin() * cfg.drift_amplitude * 0.5,
            (t * speed + s * 11.9).cos() * cfg.drift_amplitude * 0.3,
            (t * speed + s * 8.3).sin()  * cfg.drift_amplitude * 0.5,
        );
        tf.translation = snap(vx.origin + drift, NEBULA_SNAP_GRID);
    }
}

// ─────────────────────────────────────────────
//  Régénération à chaud
// ─────────────────────────────────────────────

fn reload_nebula_stream(
    mut commands:  Commands,
    mut events:    EventReader<ReloadAstre>,
    nebula_res:    Res<NebulaRes>,
    roots:         Query<Entity, With<NebulaRoot>>,
    mut meshes:    ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for ev in events.read() {
        let Ok(entity) = roots.get(ev.0) else { continue };
        commands.entity(entity).despawn_recursive();
        if nebula_res.enabled {
            build_nebula(&mut commands, &nebula_res.config, &mut meshes, &mut materials);
        }
    }
}

fn regenerate_nebula(
    mut commands:   Commands,
    mut events:     EventReader<RegenerateNebula>,
    nebula_res:     Res<NebulaRes>,
    root_q:         Query<Entity, With<NebulaRoot>>,
    mut meshes:     ResMut<Assets<Mesh>>,
    mut materials:  ResMut<Assets<StandardMaterial>>,
) {
    let mut fired = false;
    for _ in events.read() { fired = true; }
    if !fired { return; }

    for entity in &root_q {
        commands.entity(entity).despawn_recursive();
    }

    if nebula_res.enabled {
        build_nebula(&mut commands, &nebula_res.config, &mut meshes, &mut materials);
    }
}
