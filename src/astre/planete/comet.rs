use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;

// ─────────────────────────────────────────────
//  Config procédurale
// ─────────────────────────────────────────────

/// Toutes les options d'une comète — à brancher sur GameSettings plus tard.
#[derive(Clone, Debug)]
pub struct CometConfig {
    // --- Orbite elliptique ---
    /// Distance au périhélie (point le plus proche de l'étoile)
    pub perihelion:         f32,
    /// Distance à l'aphélie (point le plus loin)
    pub aphelion:           f32,
    /// Vitesse orbitale de base au périhélie
    pub orbit_speed:        f32,
    /// Inclinaison de l'orbite en radians
    pub inclination:        f32,
    /// Argument du périhélie (rotation de l'ellipse dans le plan orbital)
    pub arg_perihelion:     f32,
    /// Nœud ascendant (rotation du plan orbital)
    pub ascending_node:     f32,

    // --- Noyau ---
    pub nucleus_radius:     f32,
    pub nucleus_voxel_size: f32,
    /// Couleur de la roche du noyau (gris-bleuté, glace)
    pub nucleus_color:      [f32; 3],
    pub nucleus_emissive:   f32,
    /// Rotation propre du noyau (rad/s)
    pub nucleus_spin:       f32,

    // --- Chevelure (coma) ---
    /// Rayon de la chevelure de gaz autour du noyau
    pub coma_radius:        f32,
    pub coma_voxel_size:    f32,
    pub coma_voxel_count:   u32,
    pub coma_color:         [f32; 3],
    pub coma_emissive:      f32,
    /// Opacité de la chevelure
    pub coma_opacity:       f32,

    // --- Queue de poussière ---
    pub dust_tail_length:   f32,
    /// Largeur max de la queue à son extrémité
    pub dust_tail_width:    f32,
    pub dust_voxel_size:    f32,
    pub dust_voxel_count:   u32,
    pub dust_color:         [f32; 3],
    pub dust_emissive:      f32,
    /// Courbure de la queue de poussière (0 = droite, 1 = très courbée)
    pub dust_curve:         f32,

    // --- Queue ionique (plasma, droite, pointe vers l'étoile) ---
    pub ion_tail_length:    f32,
    pub ion_tail_width:     f32,
    pub ion_voxel_size:     f32,
    pub ion_voxel_count:    u32,
    pub ion_color:          [f32; 3],
    pub ion_emissive:       f32,

    // --- Jets de gaz du noyau ---
    pub jet_count:          u32,
    pub jet_voxel_size:     f32,
    pub jet_samples:        u32,
    pub jet_length:         f32,
    pub jet_spread:         f32,
    pub jet_color:          [f32; 3],
    pub jet_emissive:       f32,
    pub jet_speed:          f32,

    // --- Animation ---
    pub coma_drift_speed:   f32,
    pub coma_drift_amp:     f32,
    pub tail_shimmer_speed: f32,
    pub tail_shimmer_amp:   f32,

    // --- Seed ---
    pub seed:               u32,
}

impl Default for CometConfig {
    fn default() -> Self {
        Self {
            perihelion:         300.0,
            aphelion:           3000.0,
            orbit_speed:        0.06,
            inclination:        0.35,
            arg_perihelion:     0.8,
            ascending_node:     1.2,

            nucleus_radius:     18.0,
            nucleus_voxel_size: 6.0,
            nucleus_color:      [0.55, 0.58, 0.62],
            nucleus_emissive:   0.4,
            nucleus_spin:       0.15,

            coma_radius:        80.0,
            coma_voxel_size:    12.0,
            coma_voxel_count:   200,
            coma_color:         [0.5, 0.85, 0.95],
            coma_emissive:      1.8,
            coma_opacity:       0.6,

            dust_tail_length:   900.0,
            dust_tail_width:    120.0,
            dust_voxel_size:    18.0,
            dust_voxel_count:   300,
            dust_color:         [0.85, 0.72, 0.45],
            dust_emissive:      0.9,
            dust_curve:         0.35,

            ion_tail_length:    1400.0,
            ion_tail_width:     40.0,
            ion_voxel_size:     10.0,
            ion_voxel_count:    250,
            ion_color:          [0.25, 0.55, 1.0],
            ion_emissive:       3.5,

            jet_count:          4,
            jet_voxel_size:     5.0,
            jet_samples:        32,
            jet_length:         55.0,
            jet_spread:         0.25,
            jet_color:          [0.7, 0.95, 1.0],
            jet_emissive:       4.0,
            jet_speed:          1.8,

            coma_drift_speed:   0.6,
            coma_drift_amp:     4.0,
            tail_shimmer_speed: 1.1,
            tail_shimmer_amp:   0.18,

            seed:               7,
        }
    }
}

// ─────────────────────────────────────────────
//  Plugin
// ─────────────────────────────────────────────

pub struct CometPlugin;

impl Plugin for CometPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<CometRes>()
            .add_event::<RegenerateComet>()
            .add_systems(Startup, spawn_comets)
            .add_systems(Update, (
                orbit_comets,
                orient_tails,
                animate_coma,
                animate_tails,
                animate_jets,
                regenerate_comets,
            ));
    }
}

// ─────────────────────────────────────────────
//  Resource & Events
// ─────────────────────────────────────────────

#[derive(Resource)]
pub struct CometRes {
    pub comets: Vec<CometConfig>,
}

impl Default for CometRes {
    fn default() -> Self {
        Self { comets: vec![CometConfig::default()] }
    }
}

#[derive(Event)]
pub struct RegenerateComet;

// ─────────────────────────────────────────────
//  Composants ECS
// ─────────────────────────────────────────────

/// Racine de la comète — suit l'orbite
#[derive(Component)]
pub struct CometRoot {
    pub idx: usize,
}

/// Noyau rocheux/glacé, tourne sur lui-même
#[derive(Component)]
pub struct CometNucleus {
    pub comet_idx: usize,
}

/// Voxel de chevelure (coma) — dérive autour du noyau
#[derive(Component)]
pub struct CometComaVoxel {
    pub comet_idx: usize,
    pub local_origin: Vec3,   // position de repos en local
    pub drift_seed:   f32,
}

/// Voxel de queue de poussière — position fixe en local, shimmer
#[derive(Component)]
pub struct DustTailVoxel {
    pub comet_idx:    usize,
    pub local_origin: Vec3,
    pub drift_seed:   f32,
    /// t ∈ [0,1] : position le long de la queue (0 = noyau, 1 = bout)
    pub t:            f32,
}

/// Voxel de queue ionique — réorienté vers l'étoile chaque frame
#[derive(Component)]
pub struct IonTailVoxel {
    pub comet_idx: usize,
    /// t ∈ [0,1] le long de la queue
    pub t:         f32,
    pub drift_seed: f32,
    /// Décalage perpendiculaire normalisé ∈ [-1,1]
    pub perp_offset: f32,
}

/// Voxel de jet de gaz partant du noyau
#[derive(Component)]
pub struct CometJetVoxel {
    pub comet_idx:  usize,
    pub jet_idx:    u32,
    pub sample_idx: u32,
}

// ─────────────────────────────────────────────
//  Helpers math
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

/// Position sur une orbite elliptique de Kepler (approximation)
/// t_norm ∈ [0, TAU] — angle moyen
fn ellipse_position(cfg: &CometConfig, mean_anomaly: f32) -> Vec3 {
    let a = (cfg.perihelion + cfg.aphelion) * 0.5; // demi-grand axe
    let e = (cfg.aphelion - cfg.perihelion) / (cfg.aphelion + cfg.perihelion); // excentricité

    // Résolution de l'équation de Kepler par itération (Newton-Raphson)
    let mut eccentric = mean_anomaly;
    for _ in 0..8 {
        eccentric -= (eccentric - e * eccentric.sin() - mean_anomaly)
            / (1.0 - e * eccentric.cos());
    }

    // Position dans le plan orbital
    let x_orb = a * (eccentric.cos() - e);
    let y_orb = a * (1.0 - e * e).sqrt() * eccentric.sin();

    // Vitesse angulaire variable (2ème loi de Kepler)
    // pas besoin explicitement — la position suffit

    // Rotation : argument du périhélie → nœud ascendant → inclinaison
    let cos_w = cfg.arg_perihelion.cos();
    let sin_w = cfg.arg_perihelion.sin();
    let cos_o = cfg.ascending_node.cos();
    let sin_o = cfg.ascending_node.sin();
    let cos_i = cfg.inclination.cos();
    let sin_i = cfg.inclination.sin();

    // Passage plan orbital → plan de référence (3D)
    let x3 = (cos_o * cos_w - sin_o * sin_w * cos_i) * x_orb
            + (-cos_o * sin_w - sin_o * cos_w * cos_i) * y_orb;
    let y3 = (sin_o * cos_w + cos_o * sin_w * cos_i) * x_orb
            + (-sin_o * sin_w + cos_o * cos_w * cos_i) * y_orb;
    let z3 = (sin_w * sin_i) * x_orb + (cos_w * sin_i) * y_orb;

    Vec3::new(x3, z3, y3) // Y-up Bevy
}

/// Vitesse angulaire de Kepler — plus rapide au périhélie
fn kepler_speed(cfg: &CometConfig, mean_anomaly: f32) -> f32 {
    let a = (cfg.perihelion + cfg.aphelion) * 0.5;
    let e = (cfg.aphelion - cfg.perihelion) / (cfg.aphelion + cfg.perihelion);
    let mut eccentric = mean_anomaly;
    for _ in 0..8 {
        eccentric -= (eccentric - e * eccentric.sin() - mean_anomaly)
            / (1.0 - e * eccentric.cos());
    }
    // Approximation de la vitesse angulaire (loi des aires)
    let r = a * (1.0 - e * eccentric.cos());
    cfg.orbit_speed * (a / r.max(1.0)).sqrt()
}

// ─────────────────────────────────────────────
//  Spawn
// ─────────────────────────────────────────────

fn spawn_comets(
    mut commands:  Commands,
    comet_res:     Res<CometRes>,
    mut meshes:    ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (idx, cfg) in comet_res.comets.iter().enumerate() {
        build_comet(&mut commands, cfg, idx, &mut meshes, &mut materials);
    }
}

fn build_comet(
    commands:  &mut Commands,
    cfg:       &CometConfig,
    idx:       usize,
    meshes:    &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) {
    let initial_pos = ellipse_position(cfg, 0.0);

    // Racine
    let root = commands.spawn((
        Transform::from_translation(initial_pos),
        Visibility::default(),
        CometRoot { idx },
    )).id();

    // ── Noyau ──────────────────────────────────────────────────────────────
    let [nr, ng, nb] = cfg.nucleus_color;
    let ne = cfg.nucleus_emissive;

    let nucleus_mat = materials.add(StandardMaterial {
        base_color:          Color::srgb(nr, ng, nb),
        emissive:            LinearRgba::new(nr * ne, ng * ne, nb * ne, 1.0),
        perceptual_roughness: 0.92,
        unlit:               false,
        ..default()
    });

    // Noyau voxelisé : cube 3x3x3 de voxels pour la forme irrégulière
    let nv = cfg.nucleus_voxel_size;
    let nucleus_mesh = meshes.add(Mesh::from(Cuboid::new(nv, nv, nv)));

    // Forme irrégulière : on supprime certains coins selon la seed
    let nucleus_entity = commands.spawn((
        Transform::IDENTITY,
        Visibility::default(),
        CometNucleus { comet_idx: idx },
    )).id();
    commands.entity(root).add_child(nucleus_entity);

    for gx in -1_i32..=1 {
        for gy in -1_i32..=1 {
            for gz in -1_i32..=1 {
                let corner_seed = pseudo_hash(
                    cfg.seed as f32 + gx as f32 * 3.1,
                    gy as f32 * 7.7 + gz as f32 * 13.3,
                );
                // Supprime ~25% des coins pour l'aspect irrégulier
                if corner_seed < 0.25 { continue; }

                let pos = Vec3::new(gx as f32, gy as f32, gz as f32) * nv;
                let scale_jitter = 0.7 + corner_seed * 0.55;

                let voxel = commands.spawn((
                    Mesh3d(nucleus_mesh.clone()),
                    MeshMaterial3d(nucleus_mat.clone()),
                    Transform::from_translation(pos)
                        .with_scale(Vec3::splat(scale_jitter)),
                    NotShadowCaster,
                )).id();
                commands.entity(nucleus_entity).add_child(voxel);
            }
        }
    }

    // ── Chevelure (coma) ───────────────────────────────────────────────────
    let [cr, cg, cb] = cfg.coma_color;
    let ce = cfg.coma_emissive;

    let coma_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(cr, cg, cb, cfg.coma_opacity),
        emissive:   LinearRgba::new(cr * ce, cg * ce, cb * ce, 1.0),
        alpha_mode: AlphaMode::Add,
        unlit:      true,
        ..default()
    });
    let coma_mesh = meshes.add(Mesh::from(
        Cuboid::new(cfg.coma_voxel_size, cfg.coma_voxel_size, cfg.coma_voxel_size)
    ));

    for i in 0..cfg.coma_voxel_count {
        let h1 = pseudo_hash(cfg.seed as f32 + i as f32, 1.0);
        let h2 = pseudo_hash(cfg.seed as f32 + i as f32, 2.0);
        let h3 = pseudo_hash(cfg.seed as f32 + i as f32, 3.0);

        let theta = h1 * std::f32::consts::TAU;
        let phi   = (h2 * 2.0 - 1.0).clamp(-1.0, 1.0).acos();
        // Distribution concentrée vers le centre
        let r     = h3.powf(0.5) * cfg.coma_radius;

        let origin = Vec3::new(
            r * phi.sin() * theta.cos(),
            r * phi.cos(),
            r * phi.sin() * theta.sin(),
        );
        let snapped = snap_grid(origin, cfg.coma_voxel_size);
        let drift_seed = pseudo_hash(cfg.seed as f32, i as f32 * 1.73);

        let v = commands.spawn((
            Mesh3d(coma_mesh.clone()),
            MeshMaterial3d(coma_mat.clone()),
            Transform::from_translation(snapped),
            Visibility::default(),
            NotShadowCaster,
            CometComaVoxel { comet_idx: idx, local_origin: snapped, drift_seed },
        )).id();
        commands.entity(root).add_child(v);
    }

    // ── Queue de poussière ─────────────────────────────────────────────────
    let [dr, dg, db] = cfg.dust_color;
    let de = cfg.dust_emissive;

    let dust_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(dr, dg, db, 0.75),
        emissive:   LinearRgba::new(dr * de, dg * de, db * de, 1.0),
        alpha_mode: AlphaMode::Add,
        unlit:      true,
        ..default()
    });
    let dust_mesh = meshes.add(Mesh::from(
        Cuboid::new(cfg.dust_voxel_size, cfg.dust_voxel_size, cfg.dust_voxel_size)
    ));

    for i in 0..cfg.dust_voxel_count {
        let h1 = pseudo_hash(cfg.seed as f32 + i as f32, 10.0);
        let h2 = pseudo_hash(cfg.seed as f32 + i as f32, 11.0);
        let h3 = pseudo_hash(cfg.seed as f32 + i as f32, 12.0);

        // t : progression le long de la queue
        let t = h1;
        // Largeur croissante + légère courbure latérale
        let half_w = t * cfg.dust_tail_width * 0.5;
        let perp    = (h2 * 2.0 - 1.0) * half_w;
        let vert    = (h3 * 2.0 - 1.0) * half_w * 0.4;

        // Courbure vers le bas/côté (la poussière suit la trajectoire orbitale)
        let curve_offset = t * t * cfg.dust_curve * cfg.dust_tail_width * 0.5;

        // En local : +Z = direction de la queue (sera réorienté dans orient_tails)
        let origin = Vec3::new(
            perp + curve_offset,
            vert,
            t * cfg.dust_tail_length,
        );
        let snapped = snap_grid(origin, cfg.dust_voxel_size);
        let drift_seed = pseudo_hash(cfg.seed as f32 + 500.0, i as f32 * 2.31);

        let v = commands.spawn((
            Mesh3d(dust_mesh.clone()),
            MeshMaterial3d(dust_mat.clone()),
            Transform::from_translation(snapped),
            Visibility::default(),
            NotShadowCaster,
            DustTailVoxel { comet_idx: idx, local_origin: snapped, drift_seed, t },
        )).id();
        commands.entity(root).add_child(v);
    }

    // ── Queue ionique ──────────────────────────────────────────────────────
    let [ir, ig, ib] = cfg.ion_color;
    let iemit = cfg.ion_emissive;

    let ion_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(ir, ig, ib, 0.6),
        emissive:   LinearRgba::new(ir * iemit, ig * iemit, ib * iemit, 1.0),
        alpha_mode: AlphaMode::Add,
        unlit:      true,
        ..default()
    });
    let ion_mesh = meshes.add(Mesh::from(
        Cuboid::new(cfg.ion_voxel_size, cfg.ion_voxel_size, cfg.ion_voxel_size)
    ));

    for i in 0..cfg.ion_voxel_count {
        let h1 = pseudo_hash(cfg.seed as f32 + i as f32, 20.0);
        let h2 = pseudo_hash(cfg.seed as f32 + i as f32, 21.0);

        let t            = h1;
        let perp_offset  = h2 * 2.0 - 1.0; // [-1, 1] normalisé
        let drift_seed   = pseudo_hash(cfg.seed as f32 + 1000.0, i as f32 * 3.17);

        // Position initiale fictive — réorientée chaque frame dans orient_tails
        let origin = Vec3::new(
            perp_offset * cfg.ion_tail_width * 0.5 * t,
            0.0,
            t * cfg.ion_tail_length,
        );
        let snapped = snap_grid(origin, cfg.ion_voxel_size);

        let v = commands.spawn((
            Mesh3d(ion_mesh.clone()),
            MeshMaterial3d(ion_mat.clone()),
            Transform::from_translation(snapped),
            Visibility::default(),
            NotShadowCaster,
            IonTailVoxel { comet_idx: idx, t, drift_seed, perp_offset },
        )).id();
        commands.entity(root).add_child(v);
    }

    // ── Jets de gaz ────────────────────────────────────────────────────────
    let [jr, jg, jb] = cfg.jet_color;
    let je = cfg.jet_emissive;

    let jet_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(jr, jg, jb),
        emissive:   LinearRgba::new(jr * je, jg * je, jb * je, 1.0),
        alpha_mode: AlphaMode::Add,
        unlit:      true,
        ..default()
    });
    let jet_mesh = meshes.add(Mesh::from(
        Cuboid::new(cfg.jet_voxel_size, cfg.jet_voxel_size, cfg.jet_voxel_size)
    ));

    for fi in 0..cfg.jet_count {
        for si in 0..cfg.jet_samples {
            commands.spawn((
                Mesh3d(jet_mesh.clone()),
                MeshMaterial3d(jet_mat.clone()),
                Transform::from_translation(Vec3::ZERO).with_scale(Vec3::ZERO),
                Visibility::default(),
                NotShadowCaster,
                CometJetVoxel { comet_idx: idx, jet_idx: fi, sample_idx: si },
            ));
        }
    }
}

// ─────────────────────────────────────────────
//  Système d'orbite
// ─────────────────────────────────────────────

fn orbit_comets(
    time:      Res<Time>,
    comet_res: Res<CometRes>,
    mut root_q: Query<(&mut Transform, &CometRoot)>,
) {
    let t = time.elapsed_secs();

    for (mut tf, root) in &mut root_q {
        let Some(cfg) = comet_res.comets.get(root.idx) else { continue; };

        // Angle moyen qui avance dans le temps, vitesse variable
        let mean_anomaly = (t * cfg.orbit_speed
            + root.idx as f32 * std::f32::consts::TAU / 3.0)
            % std::f32::consts::TAU;

        tf.translation = ellipse_position(cfg, mean_anomaly);
    }
}

// ─────────────────────────────────────────────
//  Orientation des queues (direction anti-soleil)
// ─────────────────────────────────────────────

fn orient_tails(
    time:        Res<Time>,
    comet_res:   Res<CometRes>,
    root_q:      Query<(&GlobalTransform, &CometRoot)>,
    mut dust_q:  Query<(&DustTailVoxel, &mut Transform)>,
    mut ion_q:   Query<(&IonTailVoxel,  &mut Transform), Without<DustTailVoxel>>,
) {
    let t = time.elapsed_secs();

    for (dust, mut tf) in &mut dust_q {
        let Some(cfg) = comet_res.comets.get(dust.comet_idx) else { continue; };
        let comet_pos = root_q
            .iter()
            .find(|(_, r)| r.idx == dust.comet_idx)
            .map(|(gt, _)| gt.translation())
            .unwrap_or_default();

        // Direction anti-soleil (le soleil est à l'origine)
        let anti_sun = if comet_pos.length() > 0.01 {
            comet_pos.normalize()
        } else {
            Vec3::Z
        };

        // La queue de poussière est légèrement inclinée par rapport à anti-sun
        // (elle suit un peu le mouvement orbital de la comète)
        let mean_anomaly = (t * cfg.orbit_speed
            + dust.comet_idx as f32 * std::f32::consts::TAU / 3.0)
            % std::f32::consts::TAU;
        let next_pos = ellipse_position(cfg, mean_anomaly + 0.01);
        let vel_dir = (next_pos - comet_pos).normalize_or_zero();

        // Queue poussière = mélange anti-sun + direction orbitale inversée
        let dust_dir = (anti_sun * (1.0 - cfg.dust_curve) + (-vel_dir) * cfg.dust_curve).normalize_or_zero();
        let up = if dust_dir.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
        let right = dust_dir.cross(up).normalize();
        let up_perp = dust_dir.cross(right).normalize();

        let snapped_origin = dust.local_origin;
        let world_pos = dust_dir * snapped_origin.z
            + right    * snapped_origin.x
            + up_perp  * snapped_origin.y;

        tf.translation = snap_grid(world_pos, cfg.dust_voxel_size);
    }

    // Queue ionique : strictement anti-soleil
    for (ion, mut tf) in &mut ion_q {
        let Some(cfg) = comet_res.comets.get(ion.comet_idx) else { continue; };
        let comet_pos = root_q
            .iter()
            .find(|(_, r)| r.idx == ion.comet_idx)
            .map(|(gt, _)| gt.translation())
            .unwrap_or_default();

        let anti_sun = if comet_pos.length() > 0.01 {
            comet_pos.normalize()
        } else {
            Vec3::Z
        };

        let up    = if anti_sun.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
        let right = anti_sun.cross(up).normalize();
        let up_p  = anti_sun.cross(right).normalize();

        let perp_w = ion.perp_offset * cfg.ion_tail_width * 0.5 * ion.t;
        let world_pos = anti_sun * (ion.t * cfg.ion_tail_length)
            + right * perp_w;

        tf.translation = snap_grid(world_pos, cfg.ion_voxel_size);
    }
}

// ─────────────────────────────────────────────
//  Animation coma
// ─────────────────────────────────────────────

fn animate_coma(
    time:       Res<Time>,
    comet_res:  Res<CometRes>,
    mut coma_q: Query<(&CometComaVoxel, &mut Transform)>,
) {
    let t = time.elapsed_secs();

    for (vx, mut tf) in &mut coma_q {
        let Some(cfg) = comet_res.comets.get(vx.comet_idx) else { continue; };

        let s     = vx.drift_seed;
        let speed = cfg.coma_drift_speed;
        let amp   = cfg.coma_drift_amp;

        let drift = Vec3::new(
            (t * speed + s * 11.3).sin() * amp,
            (t * speed * 0.8 + s * 7.7).cos() * amp * 0.6,
            (t * speed * 0.9 + s * 5.1).sin() * amp * 0.8,
        );

        tf.translation = snap_grid(vx.local_origin + drift, cfg.coma_voxel_size);

        // Pulsation de taille légère
        let pulse = 1.0 + (t * speed * 1.5 + s * std::f32::consts::TAU).sin() * 0.12;
        tf.scale  = Vec3::splat(pulse);
    }
}

// ─────────────────────────────────────────────
//  Animation queues (shimmer)
// ─────────────────────────────────────────────

fn animate_tails(
    time:       Res<Time>,
    comet_res:  Res<CometRes>,
    mut dust_q: Query<(&DustTailVoxel, &mut Transform), Without<IonTailVoxel>>,
    mut ion_q:  Query<(&IonTailVoxel,  &mut Transform), Without<DustTailVoxel>>,
) {
    let t = time.elapsed_secs();

    for (vx, mut tf) in &mut dust_q {
        let Some(cfg) = comet_res.comets.get(vx.comet_idx) else { continue; };
        let shimmer = 1.0
            + (t * cfg.tail_shimmer_speed + vx.drift_seed * std::f32::consts::TAU).sin()
            * cfg.tail_shimmer_amp;
        // Scale decroissant vers le bout de la queue
        let size_falloff = (1.0 - vx.t * 0.7).max(0.2);
        tf.scale = Vec3::splat(shimmer * size_falloff);
    }

    for (vx, mut tf) in &mut ion_q {
        let Some(cfg) = comet_res.comets.get(vx.comet_idx) else { continue; };
        let shimmer = 1.0
            + (t * cfg.tail_shimmer_speed * 1.6 + vx.drift_seed * std::f32::consts::TAU).sin()
            * cfg.tail_shimmer_amp * 1.4;
        let size_falloff = (1.0 - vx.t * 0.8).max(0.15);
        tf.scale = Vec3::splat(shimmer * size_falloff);
    }
}

// ─────────────────────────────────────────────
//  Rotation du noyau
// ─────────────────────────────────────────────

// Note : la rotation est gérée dans orbit_comets via le Transform du CometNucleus.
// On ajoute un système dédié pour ne pas polluer orbit_comets.

// ─────────────────────────────────────────────
//  Animation jets
// ─────────────────────────────────────────────

fn animate_jets(
    time:       Res<Time>,
    comet_res:  Res<CometRes>,
    root_q:     Query<(&GlobalTransform, &CometRoot)>,
    mut jet_q:  Query<(&CometJetVoxel, &mut Transform, &mut Visibility)>,
) {
    let t = time.elapsed_secs();

    for (jv, mut tf, mut vis) in &mut jet_q {
        let Some(cfg) = comet_res.comets.get(jv.comet_idx) else {
            *vis = Visibility::Hidden;
            continue;
        };

        let comet_pos = root_q
            .iter()
            .find(|(_, r)| r.idx == jv.comet_idx)
            .map(|(gt, _)| gt.translation())
            .unwrap_or_default();

        // Jets plus actifs au périhélie (proche du soleil)
        let dist = comet_pos.length();
        let activity = (1.0 - (dist - cfg.perihelion) / (cfg.aphelion - cfg.perihelion).max(1.0))
            .clamp(0.0, 1.0);
        let active_samples = (cfg.jet_samples as f32 * activity) as u32;

        if jv.sample_idx >= active_samples.max(4) {
            *vis = Visibility::Hidden;
            continue;
        }

        // Direction unique par jet, sur la surface du noyau
        let base_seed  = cfg.seed as f32 * 7.3 + jv.jet_idx as f32 * 37.1;
        let jet_theta  = pseudo_hash(base_seed, 1.0) * std::f32::consts::TAU;
        let jet_phi    = (pseudo_hash(base_seed, 2.0) * 2.0 - 1.0).clamp(-1.0, 1.0).acos();

        let jet_dir = Vec3::new(
            jet_phi.sin() * jet_theta.cos(),
            jet_phi.cos(),
            jet_phi.sin() * jet_theta.sin(),
        ).normalize();

        // Animation de cycle (montée / descente) par jet
        let cycle_dur  = 1.5 + pseudo_hash(base_seed, 3.0) * 2.0;
        let offset     = pseudo_hash(base_seed, 4.0) * cycle_dur;
        let local_t    = ((t * cfg.jet_speed + offset) % cycle_dur) / cycle_dur;

        let life = if local_t < 0.5 { local_t * 2.0 } else { (1.0 - local_t) * 2.0 };

        if life < 0.05 {
            *vis = Visibility::Hidden;
            continue;
        }

        let frac = jv.sample_idx as f32 / cfg.jet_samples as f32;
        if frac > life { *vis = Visibility::Hidden; continue; }

        // Étalement du jet (cône)
        let spread_seed_a = pseudo_hash(base_seed + jv.sample_idx as f32, 5.0);
        let spread_seed_b = pseudo_hash(base_seed + jv.sample_idx as f32, 6.0);
        let spread_theta  = spread_seed_a * std::f32::consts::TAU;
        let spread_r      = spread_seed_b * cfg.jet_spread;

        let perp = if jet_dir.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
        let right = jet_dir.cross(perp).normalize();
        let up    = jet_dir.cross(right).normalize();

        let tip_offset = right * spread_theta.cos() * spread_r * cfg.jet_length
                       + up    * spread_theta.sin() * spread_r * cfg.jet_length;

        let nucleus_surface = jet_dir * cfg.nucleus_radius;
        let jet_tip         = jet_dir * cfg.jet_length + tip_offset;

        // Position interpolée sur la ligne noyau → extrémité
        let pos = nucleus_surface.lerp(jet_tip, frac);
        let snapped = snap_grid(pos, cfg.jet_voxel_size.max(1.0));

        *vis = Visibility::Visible;
        tf.translation = snapped;
        let s = (1.0 - frac * 0.7) * life;
        tf.scale = Vec3::splat(s.max(0.05));
    }
}

// ─────────────────────────────────────────────
//  Régénération à chaud
// ─────────────────────────────────────────────

fn regenerate_comets(
    mut commands:  Commands,
    mut events:    EventReader<RegenerateComet>,
    comet_res:     Res<CometRes>,
    root_q:        Query<Entity, With<CometRoot>>,
    jet_q:         Query<Entity, With<CometJetVoxel>>,
    mut meshes:    ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut fired = false;
    for _ in events.read() { fired = true; }
    if !fired { return; }

    for entity in &root_q  { commands.entity(entity).despawn_recursive(); }
    for entity in &jet_q   { commands.entity(entity).despawn_recursive(); }

    for (idx, cfg) in comet_res.comets.iter().enumerate() {
        build_comet(&mut commands, cfg, idx, &mut meshes, &mut materials);
    }
}
