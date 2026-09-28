use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;
use crate::astre::{AstreLodRoot, ReloadAstre};

// ─────────────────────────────────────────────
//  Config
// ─────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct BlackHoleConfig {
    // --- Position & orbite ---
    pub position:               Vec3,
    /// Si > 0 le trou noir orbite autour de l'origine
    pub orbit_distance:         f32,
    pub orbit_speed:            f32,

    // --- Physique ---
    /// Masse gravitationnelle (G * M, unités de jeu)
    pub mass:                   f32,
    /// Rayon de l'horizon des événements — absorption instantanée
    pub event_horizon:          f32,
    /// Rayon d'influence max (au-delà : aucun effet)
    pub influence_radius:       f32,
    /// Force tangentielle (rotation de la matière autour du trou noir)
    pub angular_force:          f32,
    /// Vitesse max autorisée sur un voxel attiré
    pub max_velocity:           f32,

    // --- Zones de déformation ---
    /// Distance de début de spaghettification
    pub spaghetti_radius:       f32,
    /// Distance d'arrachement fort
    pub tear_radius:            f32,

    // --- Spaghettification ---
    /// Facteur d'étirement max avant absorption
    pub spaghetti_max_scale:    f32,
    /// Vitesse d'étirement
    pub spaghetti_speed:        f32,

    // --- Rendu du trou noir ---
    pub core_voxel_size:        f32,
    /// Couleur du halo photonique
    pub photon_ring_color:      [f32; 3],
    pub photon_ring_emissive:   f32,
    pub photon_ring_radius:     f32,
    pub photon_ring_count:      u32,
    pub photon_ring_width:      f32,

    // --- Disque d'accrétion ---
    pub accretion_inner:        f32,
    pub accretion_outer:        f32,
    pub accretion_thickness:    f32,
    pub accretion_voxel_size:   f32,
    pub accretion_count:        u32,
    /// Couleurs du dégradé interne → externe [chaud → froid]
    pub accretion_color_inner:  [f32; 3],
    pub accretion_color_outer:  [f32; 3],
    pub accretion_emissive:     f32,
    pub accretion_speed:        f32,
    /// Turbulence de la hauteur des voxels du disque
    pub accretion_turbulence:   f32,

    // --- Jets polaires ---
    pub jets_enabled:           bool,
    pub jet_length:             f32,
    pub jet_width:              f32,
    pub jet_voxel_size:         f32,
    pub jet_count:              u32,
    pub jet_color:              [f32; 3],
    pub jet_emissive:           f32,
    pub jet_speed:              f32,

    // --- Seed ---
    pub seed:                   u32,
}

impl Default for BlackHoleConfig {
    fn default() -> Self {
        Self {
            position:               Vec3::ZERO,
            orbit_distance:         0.0,
            orbit_speed:            0.0,

            mass:                   18_000.0,
            event_horizon:          40.0,
            influence_radius:       1800.0,
            angular_force:          0.55,
            max_velocity:           420.0,

            spaghetti_radius:       160.0,
            tear_radius:            80.0,

            spaghetti_max_scale:    6.5,
            spaghetti_speed:        1.4,

            core_voxel_size:        8.0,
            photon_ring_color:      [0.8, 0.5, 1.0],
            photon_ring_emissive:   12.0,
            photon_ring_radius:     52.0,
            photon_ring_count:      180,
            photon_ring_width:      6.0,

            accretion_inner:        65.0,
            accretion_outer:        380.0,
            accretion_thickness:    22.0,
            accretion_voxel_size:   12.0,
            accretion_count:        900,
            accretion_color_inner:  [1.0, 0.6, 0.1],
            accretion_color_outer:  [0.4, 0.1, 0.6],
            accretion_emissive:     5.0,
            accretion_speed:        0.18,
            accretion_turbulence:   8.0,

            jets_enabled:           true,
            jet_length:             700.0,
            jet_width:              30.0,
            jet_voxel_size:         10.0,
            jet_count:              120,
            jet_color:              [0.5, 0.8, 1.0],
            jet_emissive:           8.0,
            jet_speed:              2.2,

            seed:                   13,
        }
    }
}

// ─────────────────────────────────────────────
//  Plugin
// ─────────────────────────────────────────────

pub struct BlackHolePlugin;

impl Plugin for BlackHolePlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<BlackHoleRes>()
            .add_event::<RegenerateBlackHole>()
            .add_event::<VoxelAbsorbedEvent>()
            .add_systems(Startup, spawn_black_holes)
            .add_systems(Update, (
                orbit_black_holes,
                apply_gravity_field,
                update_spaghettification,
                animate_accretion_disk,
                animate_photon_ring,
                animate_jets,
                absorb_horizon_voxels,
                regenerate_black_holes,
                reload_black_holes,
            ).chain());
    }
}

// ─────────────────────────────────────────────
//  Resource & Events
// ─────────────────────────────────────────────

#[derive(Resource)]
pub struct BlackHoleRes {
    pub holes: Vec<BlackHoleConfig>,
}

impl Default for BlackHoleRes {
    fn default() -> Self {
        Self { holes: vec![BlackHoleConfig { position: Vec3::new(0.0, 0.0, -5000.0), ..Default::default() }] }
    }
}

#[derive(Event)]
pub struct RegenerateBlackHole;

/// Émis quand un voxel franchit l'horizon des événements.
/// Utilise-le dans d'autres systèmes (score, effets, audio…)
#[derive(Event)]
pub struct VoxelAbsorbedEvent {
    pub entity:     Entity,
    pub hole_idx:   usize,
    pub world_pos:  Vec3,
}

// ─────────────────────────────────────────────
//  Composants ECS
// ─────────────────────────────────────────────

/// Racine du trou noir — suit l'orbite
#[derive(Component)]
pub struct BlackHoleRoot {
    pub idx: usize,
}

/// Voxel du disque d'accrétion
#[derive(Component)]
pub struct AccretionVoxel {
    pub hole_idx:    usize,
    pub angle:       f32,
    pub radius:      f32,
    pub base_height: f32,
    pub seed:        f32,
    /// Couleur interpolée (0 = intérieur chaud, 1 = extérieur froid)
    pub color_t:     f32,
}

/// Voxel de l'anneau photonique
#[derive(Component)]
pub struct PhotonRingVoxel {
    pub hole_idx: usize,
    pub angle:    f32,
    pub seed:     f32,
}

/// Voxel de jet polaire
#[derive(Component)]
pub struct JetVoxel {
    pub hole_idx:  usize,
    /// +1 pôle nord, -1 pôle sud
    pub pole:      f32,
    /// t ∈ [0,1] le long du jet
    pub t:         f32,
    pub seed:      f32,
    pub perp_seed: f32,
}

// ── Composants pour les voxels du monde affectés ──────────────────────────

/// Ajouté automatiquement à tout voxel entrant dans le champ gravitationnel
#[derive(Component)]
pub struct GravityAffected {
    pub hole_idx:  usize,
    pub velocity:  Vec3,
    /// Phase de vie dans le champ
    pub phase:     GravityPhase,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum GravityPhase {
    /// Attraction normale
    Attracted,
    /// Dans la zone de spaghettification
    Spaghettifying {
        /// Direction d'étirement (vers le trou noir)
        stretch_dir: Vec3,
        /// Facteur d'étirement courant
        stretch:     f32,
    },
    /// Dans la zone d'arrachement — plus de retour en arrière possible
    Tearing,
    /// Franchit l'horizon — sera absorbé ce frame
    Absorbing,
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

/// Gravité newtonienne clampée — évite les singularités
fn gravity_force(cfg: &BlackHoleConfig, dist: f32) -> f32 {
    let d = dist.max(cfg.event_horizon * 0.5);
    (cfg.mass / (d * d)).min(cfg.max_velocity * 4.0)
}

/// Atténuation douce de l'influence [0→1] selon la distance
fn influence_falloff(dist: f32, max_dist: f32) -> f32 {
    let t = (1.0 - dist / max_dist).clamp(0.0, 1.0);
    t * t
}

// ─────────────────────────────────────────────
//  Spawn
// ─────────────────────────────────────────────

fn spawn_black_holes(
    mut commands:  Commands,
    res:           Res<BlackHoleRes>,
    mut meshes:    ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (idx, cfg) in res.holes.iter().enumerate() {
        build_black_hole(&mut commands, cfg, idx, &mut meshes, &mut materials);
    }
}

fn build_black_hole(
    commands:  &mut Commands,
    cfg:       &BlackHoleConfig,
    idx:       usize,
    meshes:    &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) {
    let pos = if cfg.orbit_distance > 1.0 {
        Vec3::new(cfg.orbit_distance, 0.0, 0.0)
    } else {
        cfg.position
    };

    let root = commands.spawn((
        Transform::from_translation(pos),
        Visibility::default(),
        BlackHoleRoot { idx },
        AstreLodRoot { cull_dist: 10000.0, radius: cfg.influence_radius, streamable: true, label: "BlackHole" },
    )).id();

    // ── Cœur — sphère de voxels noirs absolus ─────────────────────────────
    let core_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.0, 0.0, 0.0),
        emissive:   LinearRgba::NONE,
        unlit:      true,
        ..default()
    });
    let vs = cfg.core_voxel_size;
    let core_mesh = meshes.add(Mesh::from(Cuboid::new(vs, vs, vs)));

    // Sphère voxelisée pour l'horizon
    let steps = (cfg.event_horizon / vs) as i32 + 1;
    for gx in -steps..=steps {
        for gy in -steps..=steps {
            for gz in -steps..=steps {
                let p = Vec3::new(gx as f32, gy as f32, gz as f32) * vs;
                if p.length() > cfg.event_horizon { continue; }
                let core = commands.spawn((
                    Mesh3d(core_mesh.clone()),
                    MeshMaterial3d(core_mat.clone()),
                    Transform::from_translation(p),
                    NotShadowCaster,
                )).id();
                commands.entity(root).add_child(core);
            }
        }
    }

    // ── Anneau photonique ──────────────────────────────────────────────────
    let [pr, pg, pb] = cfg.photon_ring_color;
    let pe = cfg.photon_ring_emissive;
    let photon_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(pr, pg, pb),
        emissive:   LinearRgba::new(pr * pe, pg * pe, pb * pe, 1.0),
        alpha_mode: AlphaMode::Add,
        unlit:      true,
        ..default()
    });
    let photon_mesh = meshes.add(Mesh::from(Cuboid::new(
        cfg.photon_ring_width,
        cfg.photon_ring_width * 0.3,
        cfg.photon_ring_width,
    )));

    for i in 0..cfg.photon_ring_count {
        let angle = i as f32 / cfg.photon_ring_count as f32 * std::f32::consts::TAU;
        let seed  = pseudo_hash(cfg.seed as f32, i as f32);
        let r     = cfg.photon_ring_radius + (seed * 2.0 - 1.0) * cfg.photon_ring_width;
        let h     = (seed * 2.0 - 1.0) * cfg.photon_ring_width * 0.4;
        let pos_p = Vec3::new(angle.cos() * r, h, angle.sin() * r);

        let pv = commands.spawn((
            Mesh3d(photon_mesh.clone()),
            MeshMaterial3d(photon_mat.clone()),
            Transform::from_translation(snap_grid(pos_p, cfg.photon_ring_width)),
            Visibility::default(),
            NotShadowCaster,
            PhotonRingVoxel { hole_idx: idx, angle, seed },
        )).id();
        commands.entity(root).add_child(pv);
    }

    // ── Disque d'accrétion ─────────────────────────────────────────────────
    let accr_mesh = meshes.add(Mesh::from(Cuboid::new(
        cfg.accretion_voxel_size,
        cfg.accretion_voxel_size * 0.35,
        cfg.accretion_voxel_size,
    )));

    for i in 0..cfg.accretion_count {
        let h1 = pseudo_hash(cfg.seed as f32 + 100.0, i as f32);
        let h2 = pseudo_hash(cfg.seed as f32 + 101.0, i as f32);
        let h3 = pseudo_hash(cfg.seed as f32 + 102.0, i as f32);
        let h4 = pseudo_hash(cfg.seed as f32 + 103.0, i as f32);

        let angle  = h1 * std::f32::consts::TAU;
        let radius = cfg.accretion_inner
            + h2.powf(0.6) * (cfg.accretion_outer - cfg.accretion_inner);
        let height = (h3 * 2.0 - 1.0) * cfg.accretion_thickness * 0.5;
        let seed   = h4;

        // Couleur : rouge-orange au centre, violet froid à l'extérieur
        let color_t = ((radius - cfg.accretion_inner)
            / (cfg.accretion_outer - cfg.accretion_inner))
            .clamp(0.0, 1.0);
        let [ar, ag, ab] = lerp_color(
            cfg.accretion_color_inner,
            cfg.accretion_color_outer,
            color_t,
        );
        let ae = cfg.accretion_emissive * (1.0 - color_t * 0.6);

        let accr_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(ar, ag, ab),
            emissive:   LinearRgba::new(ar * ae, ag * ae, ab * ae, 1.0),
            alpha_mode: AlphaMode::Add,
            unlit:      true,
            ..default()
        });

        let pos_a = Vec3::new(angle.cos() * radius, height, angle.sin() * radius);
        let snapped = snap_grid(pos_a, cfg.accretion_voxel_size);

        let av = commands.spawn((
            Mesh3d(accr_mesh.clone()),
            MeshMaterial3d(accr_mat),
            Transform::from_translation(snapped),
            Visibility::default(),
            NotShadowCaster,
            AccretionVoxel { hole_idx: idx, angle, radius, base_height: height, seed, color_t },
        )).id();
        commands.entity(root).add_child(av);
    }

    // ── Jets polaires ──────────────────────────────────────────────────────
    if cfg.jets_enabled {
        let [jr, jg, jb] = cfg.jet_color;
        let je = cfg.jet_emissive;
        let jet_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(jr, jg, jb),
            emissive:   LinearRgba::new(jr * je, jg * je, jb * je, 1.0),
            alpha_mode: AlphaMode::Add,
            unlit:      true,
            ..default()
        });
        let jet_mesh = meshes.add(Mesh::from(Cuboid::new(
            cfg.jet_voxel_size, cfg.jet_voxel_size, cfg.jet_voxel_size,
        )));

        for pole in [1.0_f32, -1.0_f32] {
            for i in 0..cfg.jet_count {
                let h1 = pseudo_hash(cfg.seed as f32 + 200.0 + pole * 50.0, i as f32);
                let h2 = pseudo_hash(cfg.seed as f32 + 201.0 + pole * 50.0, i as f32);
                let t  = h1;
                let perp_seed = h2;
                let seed = pseudo_hash(h1, h2);

                let jv = commands.spawn((
                    Mesh3d(jet_mesh.clone()),
                    MeshMaterial3d(jet_mat.clone()),
                    Transform::from_translation(Vec3::ZERO),
                    Visibility::default(),
                    NotShadowCaster,
                    JetVoxel { hole_idx: idx, pole, t, seed, perp_seed },
                )).id();
                commands.entity(root).add_child(jv);
            }
        }
    }
}

// ─────────────────────────────────────────────
//  Orbite
// ─────────────────────────────────────────────

fn orbit_black_holes(
    time:       Res<Time>,
    res:        Res<BlackHoleRes>,
    mut root_q: Query<(&mut Transform, &BlackHoleRoot)>,
) {
    let t = time.elapsed_secs();
    for (mut tf, root) in &mut root_q {
        let Some(cfg) = res.holes.get(root.idx) else { continue; };
        if cfg.orbit_distance > 1.0 {
            let angle = t * cfg.orbit_speed;
            tf.translation.x = angle.cos() * cfg.orbit_distance;
            tf.translation.z = angle.sin() * cfg.orbit_distance;
        }
    }
}

// ─────────────────────────────────────────────
//  Champ gravitationnel — affecte TOUS les voxels du monde
// ─────────────────────────────────────────────

/// Ce système parcourt tous les voxels qui ont un Transform
/// et leur ajoute GravityAffected s'ils sont dans le rayon d'influence.
/// Les voxels du monde jouable doivent avoir le composant `WorldVoxel`
/// ou tout autre marqueur — ici on utilise un marqueur générique.
///
/// IMPORTANT : pour intégrer avec planet.rs/comet.rs/nebula.rs,
/// ajoute le composant `AffectableByGravity` sur les entités concernées.
fn apply_gravity_field(
    time:       Res<Time>,
    res:        Res<BlackHoleRes>,
    root_q:     Query<(&GlobalTransform, &BlackHoleRoot)>,
    mut target_q: Query<
        (Entity, &GlobalTransform, Option<&mut GravityAffected>),
        With<AffectableByGravity>,
    >,
    mut commands: Commands,
) {
    let dt = time.delta_secs();

    for (hole_gt, hole_root) in &root_q {
        let Some(cfg) = res.holes.get(hole_root.idx) else { continue; };
        let hole_pos = hole_gt.translation();

        for (entity, voxel_gt, maybe_affected) in target_q.iter_mut() {
            let voxel_pos = voxel_gt.translation();
            let to_hole   = hole_pos - voxel_pos;
            let dist      = to_hole.length();

            if dist > cfg.influence_radius { continue; }
            if dist < 0.01 { continue; }

            let radial  = to_hole.normalize();
            // Composante tangentielle : rotation autour de Y
            let tangent = radial.cross(Vec3::Y).normalize_or_zero();

            let falloff  = influence_falloff(dist, cfg.influence_radius);
            let grav     = gravity_force(cfg, dist) * falloff;

            let accel    = radial * grav + tangent * cfg.angular_force * grav.sqrt();

            // Détermine la phase
            let phase = if dist <= cfg.event_horizon {
                GravityPhase::Absorbing
            } else if dist <= cfg.tear_radius {
                GravityPhase::Tearing
            } else if dist <= cfg.spaghetti_radius {
                GravityPhase::Spaghettifying {
                    stretch_dir: radial,
                    stretch:     1.0
                        + (1.0 - (dist - cfg.event_horizon)
                            / (cfg.spaghetti_radius - cfg.event_horizon))
                            * cfg.spaghetti_max_scale,
                }
            } else {
                GravityPhase::Attracted
            };

            if let Some(mut affected) = maybe_affected {
                // Mise à jour de la vélocité
                affected.velocity = (affected.velocity + accel * dt)
                    .clamp_length_max(cfg.max_velocity);
                affected.phase = phase;
                affected.hole_idx = hole_root.idx;
            } else {
                // Premier contact avec le champ
                commands.entity(entity).insert(GravityAffected {
                    hole_idx: hole_root.idx,
                    velocity: accel * dt,
                    phase,
                });
            }
        }
    }
}

/// Marqueur — pose ce composant sur tout voxel que le trou noir peut affecter.
/// Exemples : PlanetChunk, AsteroidVoxel, WorldBlock, etc.
#[derive(Component)]
pub struct AffectableByGravity;

// ─────────────────────────────────────────────
//  Spaghettification + déplacement
// ─────────────────────────────────────────────

fn update_spaghettification(
    time:     Res<Time>,
    res:      Res<BlackHoleRes>,
    mut q:    Query<(Entity, &mut Transform, &mut GravityAffected)>,
    mut absorbed_events: EventWriter<VoxelAbsorbedEvent>,
    mut commands: Commands,
) {
    let dt = time.delta_secs();

    for (entity, mut tf, mut affected) in q.iter_mut() {
        let Some(cfg) = res.holes.get(affected.hole_idx) else { continue; };

        match affected.phase {
            GravityPhase::Absorbing => {
                absorbed_events.send(VoxelAbsorbedEvent {
                    entity,
                    hole_idx: affected.hole_idx,
                    world_pos: tf.translation,
                });
                commands.entity(entity).despawn_recursive();
            }

            GravityPhase::Tearing => {
                // Déplacement + shrink rapide (matière arrachée)
                tf.translation += affected.velocity * dt;
                let shrink = (tf.scale.x - dt * cfg.spaghetti_speed * 2.0).max(0.05);
                tf.scale = Vec3::splat(shrink);
            }

            GravityPhase::Spaghettifying { stretch_dir, stretch } => {
                // Déplacement
                tf.translation += affected.velocity * dt;

                // Étirement : allonge dans la direction du trou noir,
                // compresse perpendiculairement (conservation du volume approx.)
                let s = stretch.min(cfg.spaghetti_max_scale);
                let compress = (1.0 / s.sqrt()).max(0.08);

                // Calcul de la rotation d'étirement
                let current_forward = tf.rotation * Vec3::Z;
                if current_forward.dot(stretch_dir) < 0.999 {
                    let rot = Quat::from_rotation_arc(
                        current_forward.normalize_or_zero(),
                        stretch_dir,
                    );
                    tf.rotation = tf.rotation.slerp(rot, dt * cfg.spaghetti_speed * 3.0);
                }

                tf.scale = Vec3::new(compress, compress, s);
            }

            GravityPhase::Attracted => {
                // Déplacement simple
                tf.translation += affected.velocity * dt;
                // Retour progressif à l'échelle normale si relâché
                tf.scale = tf.scale.lerp(Vec3::ONE, dt * 2.0);
            }
        }
    }
}

// ─────────────────────────────────────────────
//  Absorption à l'horizon
// ─────────────────────────────────────────────

/// Sécurité supplémentaire : supprime tout voxel AffectableByGravity
/// qui se retrouverait physiquement à l'intérieur de l'horizon.
fn absorb_horizon_voxels(
    res:      Res<BlackHoleRes>,
    root_q:   Query<(&GlobalTransform, &BlackHoleRoot)>,
    voxel_q:  Query<(Entity, &GlobalTransform), With<AffectableByGravity>>,
    mut absorbed: EventWriter<VoxelAbsorbedEvent>,
    mut commands: Commands,
) {
    for (hole_gt, hole_root) in &root_q {
        let Some(cfg) = res.holes.get(hole_root.idx) else { continue; };
        let hole_pos  = hole_gt.translation();

        for (entity, voxel_gt) in &voxel_q {
            let dist = voxel_gt.translation().distance(hole_pos);
            if dist < cfg.event_horizon {
                absorbed.send(VoxelAbsorbedEvent {
                    entity,
                    hole_idx: hole_root.idx,
                    world_pos: voxel_gt.translation(),
                });
                commands.entity(entity).despawn_recursive();
            }
        }
    }
}

// ─────────────────────────────────────────────
//  Animation disque d'accrétion
// ─────────────────────────────────────────────

fn animate_accretion_disk(
    time:       Res<Time>,
    res:        Res<BlackHoleRes>,
    mut disk_q: Query<(&AccretionVoxel, &mut Transform)>,
) {
    let t = time.elapsed_secs();

    for (av, mut tf) in &mut disk_q {
        let Some(cfg) = res.holes.get(av.hole_idx) else { continue; };

        // Vitesse différentielle de Kepler : plus proche = plus vite
        let kepler_speed = cfg.accretion_speed
            * (cfg.accretion_inner / av.radius.max(1.0)).sqrt();
        let angle = av.angle + t * kepler_speed;

        // Turbulence verticale
        let turb = (t * 1.3 + av.seed * std::f32::consts::TAU).sin()
            * cfg.accretion_turbulence
            * (1.0 - av.color_t); // plus turbulent au centre

        let pos = Vec3::new(
            angle.cos() * av.radius,
            av.base_height + turb,
            angle.sin() * av.radius,
        );

        tf.translation = snap_grid(pos, cfg.accretion_voxel_size);

        // Scale pulsant — les voxels internes pulsent plus vite
        let pulse_speed = 2.0 + (1.0 - av.color_t) * 4.0;
        let pulse = 1.0 + (t * pulse_speed + av.seed * std::f32::consts::TAU).sin() * 0.14;
        // Les voxels extérieurs sont plus grands
        let size_factor = 0.5 + av.color_t * 0.8;
        tf.scale = Vec3::splat(pulse * size_factor);
    }
}

// ─────────────────────────────────────────────
//  Animation anneau photonique
// ─────────────────────────────────────────────

fn animate_photon_ring(
    time:        Res<Time>,
    res:         Res<BlackHoleRes>,
    mut photon_q: Query<(&PhotonRingVoxel, &mut Transform)>,
) {
    let t = time.elapsed_secs();

    for (pv, mut tf) in &mut photon_q {
        let Some(cfg) = res.holes.get(pv.hole_idx) else { continue; };

        // Rotation très rapide (photons piégés en orbite)
        let angle = pv.angle + t * 2.8;
        let r     = cfg.photon_ring_radius
            + (t * 1.7 + pv.seed * std::f32::consts::TAU).sin()
            * cfg.photon_ring_width * 0.3;
        let h = (t * 2.1 + pv.seed * std::f32::consts::TAU).cos()
            * cfg.photon_ring_width * 0.2;

        let pos = Vec3::new(angle.cos() * r, h, angle.sin() * r);
        tf.translation = snap_grid(pos, cfg.photon_ring_width);

        // Scintillement intense
        let shimmer = 0.6 + (t * 5.0 + pv.seed * 9.0).sin() * 0.4;
        tf.scale = Vec3::splat(shimmer.max(0.1));
    }
}

// ─────────────────────────────────────────────
//  Animation jets polaires
// ─────────────────────────────────────────────

fn animate_jets(
    time:      Res<Time>,
    res:       Res<BlackHoleRes>,
    mut jet_q: Query<(&JetVoxel, &mut Transform, &mut Visibility)>,
) {
    let t = time.elapsed_secs();

    for (jv, mut tf, mut vis) in &mut jet_q {
        let Some(cfg) = res.holes.get(jv.hole_idx) else {
            *vis = Visibility::Hidden;
            continue;
        };

        // Le voxel remonte le long du jet en boucle
        let cycle = cfg.jet_length / cfg.jet_speed;
        let local_t = ((t * cfg.jet_speed / cfg.jet_length + jv.t) % 1.0).max(0.0);

        // Distance depuis le centre
        let dist_along = local_t * cfg.jet_length;

        // Élargissement conique du jet
        let cone_width = (local_t * cfg.jet_width).min(cfg.jet_width);
        let perp_angle = jv.perp_seed * std::f32::consts::TAU
            + t * cfg.jet_speed * 0.3;
        let perp_x = perp_angle.cos() * cone_width * 0.5;
        let perp_z = perp_angle.sin() * cone_width * 0.5;

        let pos = Vec3::new(
            perp_x,
            jv.pole * (cfg.event_horizon + dist_along),
            perp_z,
        );

        tf.translation = snap_grid(pos, cfg.jet_voxel_size);

        // Fade en bout de jet
        let fade = (1.0 - local_t).max(0.05);
        tf.scale = Vec3::splat(fade * (0.8 + (t * 3.0 + jv.seed * 6.0).sin() * 0.2));

        *vis = if fade > 0.08 { Visibility::Visible } else { Visibility::Hidden };
    }
}

// ─────────────────────────────────────────────
//  Régénération à chaud
// ─────────────────────────────────────────────

fn reload_black_holes(
    mut commands:  Commands,
    mut events:    EventReader<ReloadAstre>,
    res:           Res<BlackHoleRes>,
    roots:         Query<(Entity, &BlackHoleRoot)>,
    mut meshes:    ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for ev in events.read() {
        let Ok((entity, root)) = roots.get(ev.0) else { continue };
        let idx = root.idx;
        let Some(cfg) = res.holes.get(idx) else { continue };
        commands.entity(entity).despawn_recursive();
        build_black_hole(&mut commands, cfg, idx, &mut meshes, &mut materials);
    }
}

fn regenerate_black_holes(
    mut commands:  Commands,
    mut events:    EventReader<RegenerateBlackHole>,
    res:           Res<BlackHoleRes>,
    root_q:        Query<Entity, With<BlackHoleRoot>>,
    mut meshes:    ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut fired = false;
    for _ in events.read() { fired = true; }
    if !fired { return; }

    for entity in &root_q {
        commands.entity(entity).despawn_recursive();
    }

    for (idx, cfg) in res.holes.iter().enumerate() {
        build_black_hole(&mut commands, cfg, idx, &mut meshes, &mut materials);
    }
}
