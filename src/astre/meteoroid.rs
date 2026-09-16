use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;

// ─────────────────────────────────────────────
//  Types de composition
// ─────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub enum MeteoroidComposition {
    /// Roche silicatée (chondrite)
    Rocky,
    /// Métal pur (fer-nickel)
    Metallic,
    /// Glace + poussière (cométaire)
    Icy,
    /// Carbone + matière organique
    Carbonaceous,
    /// Mélange roche + métal (pallasite)
    Stony_Iron,
    /// Mélange glace + roche
    Icy_Rocky,
}

impl MeteoroidComposition {
    /// Couleur principale selon la composition
    pub fn base_color(&self) -> [f32; 3] {
        match self {
            Self::Rocky        => [0.55, 0.48, 0.40],
            Self::Metallic     => [0.72, 0.70, 0.65],
            Self::Icy          => [0.78, 0.88, 0.95],
            Self::Carbonaceous => [0.18, 0.16, 0.14],
            Self::Stony_Iron   => [0.60, 0.55, 0.45],
            Self::Icy_Rocky    => [0.62, 0.68, 0.72],
        }
    }

    /// Couleur des veines internes (exposées à la fragmentation)
    pub fn vein_color(&self) -> [f32; 3] {
        match self {
            Self::Rocky        => [0.70, 0.60, 0.50],
            Self::Metallic     => [0.90, 0.88, 0.80],
            Self::Icy          => [0.92, 0.96, 1.00],
            Self::Carbonaceous => [0.28, 0.24, 0.20],
            Self::Stony_Iron   => [0.80, 0.75, 0.55],
            Self::Icy_Rocky    => [0.80, 0.88, 0.92],
        }
    }

    /// Couleur de l'explosion à l'impact
    pub fn explosion_color(&self) -> [f32; 3] {
        match self {
            Self::Rocky        => [1.0, 0.55, 0.15],
            Self::Metallic     => [1.0, 0.85, 0.30],
            Self::Icy          => [0.70, 0.90, 1.00],
            Self::Carbonaceous => [0.60, 0.40, 0.10],
            Self::Stony_Iron   => [1.0, 0.70, 0.20],
            Self::Icy_Rocky    => [0.80, 0.95, 1.00],
        }
    }

    /// Résistance à la fragmentation (0 = fragile, 1 = résistant)
    pub fn toughness(&self) -> f32 {
        match self {
            Self::Rocky        => 0.55,
            Self::Metallic     => 0.90,
            Self::Icy          => 0.20,
            Self::Carbonaceous => 0.35,
            Self::Stony_Iron   => 0.72,
            Self::Icy_Rocky    => 0.30,
        }
    }

    /// Emissivité au moment du l'impact
    pub fn impact_emissive(&self) -> f32 {
        match self {
            Self::Rocky        => 8.0,
            Self::Metallic     => 15.0,
            Self::Icy          => 5.0,
            Self::Carbonaceous => 4.0,
            Self::Stony_Iron   => 12.0,
            Self::Icy_Rocky    => 6.0,
        }
    }
}

// ─────────────────────────────────────────────
//  Formes procédurales
// ─────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub enum MeteoroidShape {
    /// Sphère déformée (astéroïde typique)
    DeformedSphere {
        deform_strength: f32,
        deform_octaves:  u32,
    },
    /// Ellipsoïde (cigare ou galette)
    Ellipsoid {
        scale_x: f32,
        scale_y: f32,
        scale_z: f32,
    },
    /// Double lobe (contact binary — deux corps fusionnés)
    DoubleLobe {
        lobe_separation: f32,
        lobe_ratio:      f32,   // rapport taille lobe2 / lobe1
    },
    /// Roche plate (fragment d'impact)
    Slab {
        flatness: f32,   // 0=sphère, 1=très plat
    },
    /// Forme irrégulière anguleuse (fragment frais)
    Angular {
        spike_count:    u32,
        spike_strength: f32,
    },
    /// Tore creux (très rare, instable)
    Toroidal {
        hole_ratio: f32,   // rayon du trou / rayon total
    },
    /// Forme d'os (deux masses reliées par un col fin)
    Dumbbell {
        neck_ratio: f32,   // rayon du col / rayon des extrémités
    },
}

// ─────────────────────────────────────────────
//  Config
// ─────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct MeteoroidConfig {
    // --- Position & trajectoire ---
    pub position:               Vec3,
    pub velocity:               Vec3,
    /// Vitesse de rotation propre (rad/s) autour de chaque axe
    pub spin:                   Vec3,
    /// Si true : tumbling chaotique (rotation multi-axe)
    pub tumbling:               bool,

    // --- Taille & forme ---
    pub radius:                 f32,
    pub voxel_size:             f32,
    pub shape:                  MeteoroidShape,

    // --- Composition ---
    pub composition:            MeteoroidComposition,

    // --- Surface ---
    /// Cratères d'impact sur la surface (0 = lisse, 1 = très cratérisé)
    pub crater_density:         f32,
    pub crater_depth:           f32,
    /// Veines visibles à la surface
    pub vein_enabled:           bool,
    pub vein_count:             u32,
    pub vein_width:             f32,

    // --- Poussière & halo ---
    pub dust_enabled:           bool,
    pub dust_voxel_size:        f32,
    pub dust_count:             u32,
    pub dust_color:             [f32; 3],
    pub dust_emissive:          f32,
    pub dust_radius:            f32,
    pub dust_drift_speed:       f32,

    // --- Dégazage (glace/carbonaceous) ---
    pub outgassing_enabled:     bool,
    pub outgassing_count:       u32,
    pub outgassing_voxel_size:  f32,
    pub outgassing_color:       [f32; 3],
    pub outgassing_emissive:    f32,
    pub outgassing_jet_count:   u32,
    pub outgassing_jet_length:  f32,
    pub outgassing_speed:       f32,

    // --- Fragmentation & impact ---
    pub fragment_enabled:       bool,
    /// Seuil de vitesse d'impact (u/s) au-delà duquel fragmentation
    pub fragment_threshold:     f32,
    /// Nombre de fragments produits
    pub fragment_count:         u32,
    pub fragment_min_size:      f32,
    pub fragment_max_size:      f32,
    /// Vitesse d'éjection des fragments
    pub fragment_eject_speed:   f32,
    /// Durée de vie des fragments (s) avant despawn
    pub fragment_lifetime:      f32,

    // --- Explosion d'impact ---
    pub explosion_enabled:      bool,
    pub explosion_voxel_count:  u32,
    pub explosion_voxel_size:   f32,
    pub explosion_max_radius:   f32,
    pub explosion_speed:        f32,
    pub explosion_duration:     f32,

    // --- LOD (distance pour simplifier le mesh) ---
    pub lod_distance_full:      f32,
    pub lod_distance_low:       f32,

    // --- Seed ---
    pub seed:                   u32,
}

impl Default for MeteoroidConfig {
    fn default() -> Self {
        Self {
            position:               Vec3::new(500.0, 0.0, 0.0),
            velocity:               Vec3::new(-5.0, 0.2, 0.1),
            spin:                   Vec3::new(0.3, 0.7, 0.15),
            tumbling:               true,

            radius:                 40.0,
            voxel_size:             7.0,
            shape: MeteoroidShape::DeformedSphere {
                deform_strength: 0.35,
                deform_octaves:  4,
            },
            composition:            MeteoroidComposition::Rocky,

            crater_density:         0.45,
            crater_depth:           0.3,
            vein_enabled:           true,
            vein_count:             8,
            vein_width:             1.5,

            dust_enabled:           true,
            dust_voxel_size:        5.0,
            dust_count:             80,
            dust_color:             [0.60, 0.55, 0.48],
            dust_emissive:          0.2,
            dust_radius:            20.0,
            dust_drift_speed:       0.12,

            outgassing_enabled:     false,
            outgassing_count:       60,
            outgassing_voxel_size:  4.0,
            outgassing_color:       [0.8, 0.9, 1.0],
            outgassing_emissive:    1.5,
            outgassing_jet_count:   3,
            outgassing_jet_length:  50.0,
            outgassing_speed:       0.8,

            fragment_enabled:       true,
            fragment_threshold:     50.0,
            fragment_count:         12,
            fragment_min_size:      3.0,
            fragment_max_size:      15.0,
            fragment_eject_speed:   80.0,
            fragment_lifetime:      8.0,

            explosion_enabled:      true,
            explosion_voxel_count:  150,
            explosion_voxel_size:   8.0,
            explosion_max_radius:   200.0,
            explosion_speed:        140.0,
            explosion_duration:     3.5,

            lod_distance_full:      300.0,
            lod_distance_low:       900.0,

            seed:                   41,
        }
    }
}

// ─────────────────────────────────────────────
//  Présets de config par composition
// ─────────────────────────────────────────────

impl MeteoroidConfig {
    pub fn rocky(seed: u32, radius: f32, pos: Vec3) -> Self {
        Self {
            seed, radius, position: pos,
            composition: MeteoroidComposition::Rocky,
            shape: MeteoroidShape::DeformedSphere { deform_strength: 0.30, deform_octaves: 4 },
            crater_density: 0.5, vein_enabled: true,
            ..Default::default()
        }
    }

    pub fn metallic(seed: u32, radius: f32, pos: Vec3) -> Self {
        Self {
            seed, radius, position: pos,
            composition: MeteoroidComposition::Metallic,
            shape: MeteoroidShape::Ellipsoid { scale_x: 1.4, scale_y: 0.75, scale_z: 0.9 },
            crater_density: 0.2,
            dust_enabled: false,
            fragment_count: 6,
            ..Default::default()
        }
    }

    pub fn icy(seed: u32, radius: f32, pos: Vec3) -> Self {
        Self {
            seed, radius, position: pos,
            composition: MeteoroidComposition::Icy,
            shape: MeteoroidShape::DeformedSphere { deform_strength: 0.15, deform_octaves: 3 },
            outgassing_enabled: true,
            crater_density: 0.1,
            fragment_count: 18,
            vein_count: 4,
            ..Default::default()
        }
    }

    pub fn carbonaceous(seed: u32, radius: f32, pos: Vec3) -> Self {
        Self {
            seed, radius, position: pos,
            composition: MeteoroidComposition::Carbonaceous,
            shape: MeteoroidShape::Angular { spike_count: 12, spike_strength: 0.25 },
            crater_density: 0.6,
            outgassing_enabled: true,
            vein_enabled: false,
            ..Default::default()
        }
    }

    pub fn stony_iron(seed: u32, radius: f32, pos: Vec3) -> Self {
        Self {
            seed, radius, position: pos,
            composition: MeteoroidComposition::Stony_Iron,
            shape: MeteoroidShape::DoubleLobe { lobe_separation: 0.6, lobe_ratio: 0.7 },
            crater_density: 0.35,
            vein_count: 14,
            ..Default::default()
        }
    }

    pub fn icy_rocky(seed: u32, radius: f32, pos: Vec3) -> Self {
        Self {
            seed, radius, position: pos,
            composition: MeteoroidComposition::Icy_Rocky,
            shape: MeteoroidShape::Dumbbell { neck_ratio: 0.45 },
            outgassing_enabled: true,
            crater_density: 0.25,
            ..Default::default()
        }
    }
}

// ─────────────────────────────────────────────
//  Plugin
// ─────────────────────────────────────────────

pub struct MeteoroidPlugin;

impl Plugin for MeteoroidPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<MeteoroidRes>()
            .add_event::<RegenerateMeteoroid>()
            .add_event::<MeteoroidImpactEvent>()
            .add_event::<TriggerFragmentation>()
            .add_systems(Startup, spawn_meteoroids)
            .add_systems(Update, (
                move_meteoroids,
                tumble_meteoroids,
                animate_dust_halo,
                animate_outgassing,
                detect_fragmentation,
                animate_fragments,
                animate_explosion,
                regenerate_meteoroids,
            ).chain());
    }
}

// ─────────────────────────────────────────────
//  Resource & Events
// ─────────────────────────────────────────────

#[derive(Resource)]
pub struct MeteoroidRes {
    pub meteoroids: Vec<MeteoroidConfig>,
}

impl Default for MeteoroidRes {
    fn default() -> Self {
        Self {
            meteoroids: vec![
                MeteoroidConfig::rocky(11, 40.0, Vec3::new(500.0, 20.0, 0.0)),
                MeteoroidConfig::icy(22, 28.0, Vec3::new(-400.0, 0.0, 200.0)),
                MeteoroidConfig::metallic(33, 25.0, Vec3::new(0.0, 100.0, 600.0)),
                MeteoroidConfig::carbonaceous(44, 35.0, Vec3::new(300.0, -50.0, -300.0)),
                MeteoroidConfig::stony_iron(55, 32.0, Vec3::new(-600.0, 30.0, 100.0)),
                MeteoroidConfig::icy_rocky(66, 22.0, Vec3::new(100.0, 200.0, -500.0)),
            ],
        }
    }
}

#[derive(Event)]
pub struct RegenerateMeteoroid;

/// Émis quand un météoroïde se fragmente ou frappe quelque chose
#[derive(Event)]
pub struct MeteoroidImpactEvent {
    pub meteoroid_idx: usize,
    pub position:      Vec3,
    pub velocity:      Vec3,
    pub composition:   MeteoroidComposition,
}

/// Déclenche la fragmentation d'un météoroïde spécifique
#[derive(Event)]
pub struct TriggerFragmentation {
    pub meteoroid_idx: usize,
    pub impact_pos:    Vec3,
}

// ─────────────────────────────────────────────
//  Composants ECS
// ─────────────────────────────────────────────

#[derive(Component)]
pub struct MeteoroidRoot {
    pub idx: usize,
}

/// Vélocité du météoroïde (déplacement indépendant)
#[derive(Component)]
pub struct MeteoroidVelocity(pub Vec3);

/// Rotation chaotique (tumbling)
#[derive(Component)]
pub struct MeteoroidTumble {
    pub spin: Vec3,
    pub phase: Vec3,
}

/// Voxel de corps principal
#[derive(Component)]
pub struct MeteoroidBodyVoxel {
    pub idx:    usize,
    /// Position locale originale (pour QPO / déformation)
    pub origin: Vec3,
    /// Facteur de surface (0 = intérieur, 1 = surface)
    pub surf_t: f32,
    /// true si c'est une veine
    pub is_vein: bool,
    pub seed:   f32,
}

/// Voxel de poussière orbitale
#[derive(Component)]
pub struct MeteoroidDust {
    pub idx:    usize,
    pub origin: Vec3,
    pub theta:  f32,
    pub phi:    f32,
    pub r:      f32,
    pub seed:   f32,
}

/// Voxel de dégazage (jet de glace/gaz)
#[derive(Component)]
pub struct OutgassingVoxel {
    pub idx:       usize,
    pub jet_idx:   u32,
    pub sample_t:  f32,
    pub jet_dir:   Vec3,
    pub seed:      f32,
}

/// Fragment éjecté après impact
#[derive(Component)]
pub struct MeteoroidFragment {
    pub idx:           usize,
    pub velocity:      Vec3,
    pub spin:          Vec3,
    pub lifetime:      f32,
    pub max_lifetime:  f32,
    pub composition:   MeteoroidComposition,
}

/// Voxel d'explosion d'impact
#[derive(Component)]
pub struct ExplosionVoxel {
    pub idx:       usize,
    pub dir:       Vec3,
    pub seed:      f32,
    pub speed:     f32,
    pub elapsed:   f32,
    pub duration:  f32,
    pub max_radius: f32,
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

/// Retourne true si le point local `p` est à l'intérieur de la forme
/// et la distance normalisée [0,1] à la surface (0=centre, 1=surface)
fn shape_sdf(p: Vec3, cfg: &MeteoroidConfig) -> (bool, f32) {
    let r     = cfg.radius;
    let seed  = cfg.seed as f32;

    match &cfg.shape {
        MeteoroidShape::DeformedSphere { deform_strength, deform_octaves } => {
            // FBM procédural via hashing (pas de dépendance noise)
            let pn    = p.normalize();
            let mut n = 0.0_f32;
            let mut amp = 1.0_f32;
            let mut freq = 1.0_f32;
            for oct in 0..*deform_octaves {
                let sx = pn.x * freq + seed * 0.1 + oct as f32 * 3.7;
                let sy = pn.y * freq + seed * 0.2 + oct as f32 * 7.1;
                let sz = pn.z * freq + seed * 0.3 + oct as f32 * 5.3;
                n += pseudo_hash(sx + sy, sz) * 2.0 - 1.0;
                n *= amp;
                amp  *= 0.5;
                freq *= 2.0;
            }
            let deformed_r = r * (1.0 + n * deform_strength);
            let dist = p.length();
            let surf_t = (dist / deformed_r.max(0.01)).clamp(0.0, 1.0);
            (dist <= deformed_r, surf_t)
        }

        MeteoroidShape::Ellipsoid { scale_x, scale_y, scale_z } => {
            let scaled = Vec3::new(p.x / scale_x, p.y / scale_y, p.z / scale_z);
            let dist   = scaled.length() / r;
            (dist <= 1.0, dist.clamp(0.0, 1.0))
        }

        MeteoroidShape::DoubleLobe { lobe_separation, lobe_ratio } => {
            let sep    = r * lobe_separation;
            let r1     = r;
            let r2     = r * lobe_ratio;
            let p1     = Vec3::new(-sep * 0.5, 0.0, 0.0);
            let p2     = Vec3::new( sep * 0.5, 0.0, 0.0);
            let d1     = (p - p1).length();
            let d2     = (p - p2).length();
            let in1    = d1 <= r1;
            let in2    = d2 <= r2;
            let surf_t = ((d1 / r1).min(d2 / r2)).clamp(0.0, 1.0);
            (in1 || in2, surf_t)
        }

        MeteoroidShape::Slab { flatness } => {
            let half_h = r * (1.0 - flatness.clamp(0.0, 0.95));
            let r_xz   = r * (1.0 + flatness * 0.5);
            let in_xz  = (p.x * p.x + p.z * p.z).sqrt() <= r_xz;
            let in_y   = p.y.abs() <= half_h;
            let surf_t = ((p.x * p.x + p.z * p.z).sqrt() / r_xz)
                .max(p.y.abs() / half_h.max(0.01))
                .clamp(0.0, 1.0);
            (in_xz && in_y, surf_t)
        }

        MeteoroidShape::Angular { spike_count, spike_strength } => {
            // Base sphère + pics anguleux selon hash directionnel
            let pn    = p.normalize();
            let dist  = p.length();
            // Déformation par pics
            let mut max_spike = 0.0_f32;
            for i in 0..*spike_count {
                let sh1 = pseudo_hash(seed + i as f32, 1.0);
                let sh2 = pseudo_hash(seed + i as f32, 2.0);
                let theta = sh1 * std::f32::consts::TAU;
                let phi   = (sh2 * 2.0 - 1.0).clamp(-1.0, 1.0).acos();
                let spike_dir = Vec3::new(
                    phi.sin() * theta.cos(),
                    phi.cos(),
                    phi.sin() * theta.sin(),
                );
                let alignment = pn.dot(spike_dir).max(0.0).powf(8.0);
                max_spike    = max_spike.max(alignment);
            }
            let deformed_r = r * (1.0 + max_spike * spike_strength);
            let surf_t     = (dist / deformed_r.max(0.01)).clamp(0.0, 1.0);
            (dist <= deformed_r, surf_t)
        }

        MeteoroidShape::Toroidal { hole_ratio } => {
            let r_big  = r;
            let r_tube = r * (1.0 - hole_ratio) * 0.5;
            let xz     = (p.x * p.x + p.z * p.z).sqrt();
            let d      = ((xz - r_big * hole_ratio).powi(2) + p.y * p.y).sqrt();
            let surf_t = (d / r_tube.max(0.01)).clamp(0.0, 1.0);
            (d <= r_tube, surf_t)
        }

        MeteoroidShape::Dumbbell { neck_ratio } => {
            let half_l  = r * 0.85;
            let r_end   = r * 0.55;
            let r_neck  = r_end * neck_ratio;
            let p1      = Vec3::new(0.0,  half_l, 0.0);
            let p2      = Vec3::new(0.0, -half_l, 0.0);
            let d1      = (p - p1).length();
            let d2      = (p - p2).length();
            // Col : cylindre d'épaisseur r_neck entre p1 et p2
            let along   = p.y.clamp(-half_l, half_l);
            let xz_dist = (p.x * p.x + p.z * p.z).sqrt();
            let neck_t  = 1.0 - (p.y / half_l).abs();
            let r_at_y  = r_neck + (r_end - r_neck) * (1.0 - neck_t).powi(2);
            let in_neck = xz_dist <= r_at_y && p.y.abs() <= half_l;
            let in_end1 = d1 <= r_end;
            let in_end2 = d2 <= r_end;
            let inside  = in_neck || in_end1 || in_end2;
            let surf_t  = (d1 / r_end).min(d2 / r_end).min(xz_dist / r_at_y.max(0.01))
                .clamp(0.0, 1.0);
            (inside, surf_t)
        }
    }
}

/// Déformation de surface par cratère (retourne un multiplicateur radial)
fn crater_deform(p_norm: Vec3, cfg: &MeteoroidConfig) -> f32 {
    if cfg.crater_density <= 0.0 { return 0.0; }
    let n_craters = (cfg.crater_density * 20.0) as u32;
    let mut total = 0.0_f32;

    for i in 0..n_craters {
        let ch1 = pseudo_hash(cfg.seed as f32 + 100.0 + i as f32, 1.0);
        let ch2 = pseudo_hash(cfg.seed as f32 + 100.0 + i as f32, 2.0);
        let ch3 = pseudo_hash(cfg.seed as f32 + 100.0 + i as f32, 3.0);

        let theta   = ch1 * std::f32::consts::TAU;
        let phi     = (ch2 * 2.0 - 1.0).clamp(-1.0, 1.0).acos();
        let c_dir   = Vec3::new(phi.sin() * theta.cos(), phi.cos(), phi.sin() * theta.sin());
        let c_r     = 0.05 + ch3 * 0.25;   // rayon angulaire du cratère

        let alignment = p_norm.dot(c_dir).clamp(-1.0, 1.0).acos();
        if alignment < c_r {
            let t     = alignment / c_r;
            // Profil de cratère : dépression douce au centre, rempart sur les bords
            let rim   = (1.0 - t * t).sin() * 0.3;
            let depth = -(1.0 - t * t).powi(2) * cfg.crater_depth;
            total    += depth + rim;
        }
    }
    total
}

// ─────────────────────────────────────────────
//  Spawn
// ─────────────────────────────────────────────

fn spawn_meteoroids(
    mut commands:  Commands,
    res:           Res<MeteoroidRes>,
    mut meshes:    ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (idx, cfg) in res.meteoroids.iter().enumerate() {
        build_meteoroid(&mut commands, cfg, idx, &mut meshes, &mut materials);
    }
}

fn build_meteoroid(
    commands:  &mut Commands,
    cfg:       &MeteoroidConfig,
    idx:       usize,
    meshes:    &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) {
    let root = commands.spawn((
        Transform::from_translation(cfg.position),
        Visibility::default(),
        MeteoroidRoot { idx },
        MeteoroidVelocity(cfg.velocity),
        MeteoroidTumble {
            spin:  cfg.spin,
            phase: Vec3::new(
                pseudo_hash(cfg.seed as f32, 1.0) * std::f32::consts::TAU,
                pseudo_hash(cfg.seed as f32, 2.0) * std::f32::consts::TAU,
                pseudo_hash(cfg.seed as f32, 3.0) * std::f32::consts::TAU,
            ),
        },
    )).id();

    // ── Corps principal ────────────────────────────────────────────────────
    let [br, bg, bb] = cfg.composition.base_color();
    let [vr, vg, vb] = cfg.composition.vein_color();
    let voxel_mesh = meshes.add(Mesh::from(Cuboid::new(
        cfg.voxel_size, cfg.voxel_size, cfg.voxel_size,
    )));

    let body_mat = materials.add(StandardMaterial {
        base_color:          Color::srgb(br, bg, bb),
        perceptual_roughness: 0.9,
        ..default()
    });
    let vein_mat = materials.add(StandardMaterial {
        base_color:          Color::srgb(vr, vg, vb),
        perceptual_roughness: 0.6,
        emissive:            LinearRgba::new(vr * 0.5, vg * 0.5, vb * 0.5, 1.0),
        ..default()
    });

    // Grille 3D — on teste chaque point dans la SDF de la forme
    let steps = (cfg.radius * 2.0 / cfg.voxel_size) as i32 + 2;
    let mut voxel_idx = 0u32;

    for gx in -steps..=steps {
        for gy in -steps..=steps {
            for gz in -steps..=steps {
                let p = Vec3::new(gx as f32, gy as f32, gz as f32) * cfg.voxel_size;

                let (inside, surf_t) = shape_sdf(p, cfg);
                if !inside { continue; }

                // Déformation de cratère sur la surface
                let p_norm     = p.normalize_or_zero();
                let crater_off = crater_deform(p_norm, cfg) * cfg.radius;
                let p_deformed = p + p_norm * crater_off;
                let (still_in, _) = shape_sdf(p_deformed, cfg);
                if !still_in && surf_t > 0.85 { continue; }

                // Détection des veines
                let is_vein = if cfg.vein_enabled && cfg.vein_count > 0 {
                    let mut vein = false;
                    for vi in 0..cfg.vein_count {
                        let vh1 = pseudo_hash(cfg.seed as f32 + vi as f32 * 17.3, 8.0);
                        let vh2 = pseudo_hash(cfg.seed as f32 + vi as f32 * 17.3, 9.0);
                        let vein_theta = vh1 * std::f32::consts::TAU;
                        let vein_phi   = (vh2 * 2.0 - 1.0).clamp(-1.0, 1.0).acos();
                        let vein_dir = Vec3::new(
                            vein_phi.sin() * vein_theta.cos(),
                            vein_phi.cos(),
                            vein_phi.sin() * vein_theta.sin(),
                        );
                        // Distance à la ligne de la veine
                        let t_proj = p_norm.dot(vein_dir).clamp(0.0, 1.0);
                        let closest = vein_dir * t_proj;
                        let vein_dist = (p_norm - closest).length();
                        if vein_dist < cfg.vein_width / cfg.radius {
                            vein = true;
                            break;
                        }
                    }
                    vein
                } else {
                    false
                };

                let seed_v = pseudo_hash(voxel_idx as f32, 3.14);
                voxel_idx += 1;

                let mat = if is_vein { vein_mat.clone() } else { body_mat.clone() };
                let snapped = snap_grid(p, cfg.voxel_size);

                let v = commands.spawn((
                    Mesh3d(voxel_mesh.clone()),
                    MeshMaterial3d(mat),
                    Transform::from_translation(snapped),
                    NotShadowCaster,
                    MeteoroidBodyVoxel { idx, origin: snapped, surf_t, is_vein, seed: seed_v },
                )).id();
                commands.entity(root).add_child(v);
            }
        }
    }

    // ── Halo de poussière ──────────────────────────────────────────────────
    if cfg.dust_enabled {
        let [dr, dg, db] = cfg.dust_color;
        let de = cfg.dust_emissive;
        let dust_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(dr, dg, db),
            emissive:   LinearRgba::new(dr * de, dg * de, db * de, 1.0),
            alpha_mode: AlphaMode::Add,
            unlit:      true,
            ..default()
        });
        let dust_mesh = meshes.add(Mesh::from(Cuboid::new(
            cfg.dust_voxel_size, cfg.dust_voxel_size, cfg.dust_voxel_size,
        )));

        for di in 0..cfg.dust_count {
            let h1 = pseudo_hash(cfg.seed as f32 + 200.0, di as f32);
            let h2 = pseudo_hash(cfg.seed as f32 + 201.0, di as f32);
            let h3 = pseudo_hash(cfg.seed as f32 + 202.0, di as f32);

            let theta = h1 * std::f32::consts::TAU;
            let phi   = (h2 * 2.0 - 1.0).clamp(-1.0, 1.0).acos();
            let r     = cfg.radius + h3.sqrt() * cfg.dust_radius;
            let origin = Vec3::new(
                r * phi.sin() * theta.cos(),
                r * phi.cos(),
                r * phi.sin() * theta.sin(),
            );

            let dv = commands.spawn((
                Mesh3d(dust_mesh.clone()),
                MeshMaterial3d(dust_mat.clone()),
                Transform::from_translation(snap_grid(origin, cfg.dust_voxel_size)),
                Visibility::default(),
                NotShadowCaster,
                MeteoroidDust { idx, origin: snap_grid(origin, cfg.dust_voxel_size), theta, phi, r, seed: pseudo_hash(h1, h2) },
            )).id();
            commands.entity(root).add_child(dv);
        }
    }

    // ── Dégazage ───────────────────────────────────────────────────────────
    if cfg.outgassing_enabled {
        let [or_, og, ob] = cfg.outgassing_color;
        let oe = cfg.outgassing_emissive;
        let out_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(or_, og, ob),
            emissive:   LinearRgba::new(or_ * oe, og * oe, ob * oe, 1.0),
            alpha_mode: AlphaMode::Add,
            unlit:      true,
            ..default()
        });
        let out_mesh = meshes.add(Mesh::from(Cuboid::new(
            cfg.outgassing_voxel_size, cfg.outgassing_voxel_size, cfg.outgassing_voxel_size,
        )));

        for ji in 0..cfg.outgassing_jet_count {
            let jh1 = pseudo_hash(cfg.seed as f32 + 300.0 + ji as f32, 1.0);
            let jh2 = pseudo_hash(cfg.seed as f32 + 300.0 + ji as f32, 2.0);
            let jtheta = jh1 * std::f32::consts::TAU;
            let jphi   = (jh2 * 2.0 - 1.0).clamp(-1.0, 1.0).acos();
            let jet_dir = Vec3::new(
                jphi.sin() * jtheta.cos(),
                jphi.cos(),
                jphi.sin() * jtheta.sin(),
            ).normalize();

            for si in 0..cfg.outgassing_count {
                let sample_t = si as f32 / cfg.outgassing_count as f32;
                let seed     = pseudo_hash(ji as f32, si as f32 * 1.73);

                let ov = commands.spawn((
                    Mesh3d(out_mesh.clone()),
                    MeshMaterial3d(out_mat.clone()),
                    Transform::from_translation(Vec3::ZERO),
                    Visibility::default(),
                    NotShadowCaster,
                    OutgassingVoxel { idx, jet_idx: ji, sample_t, jet_dir, seed },
                )).id();
                commands.entity(root).add_child(ov);
            }
        }
    }
}

// ─────────────────────────────────────────────
//  Déplacement
// ─────────────────────────────────────────────

fn move_meteoroids(
    time:      Res<Time>,
    mut root_q: Query<(&mut Transform, &MeteoroidVelocity), With<MeteoroidRoot>>,
) {
    let dt = time.delta_secs();
    for (mut tf, vel) in &mut root_q {
        tf.translation += vel.0 * dt;
    }
}

fn tumble_meteoroids(
    time:       Res<Time>,
    res:        Res<MeteoroidRes>,
    mut root_q: Query<(&mut Transform, &MeteoroidTumble, &MeteoroidRoot)>,
) {
    let t = time.elapsed_secs();
    for (mut tf, tumble, root) in &mut root_q {
        let Some(cfg) = res.meteoroids.get(root.idx) else { continue; };
        if !cfg.tumbling { continue; }

        // Rotation de Euler indépendante sur chaque axe + phase initiale
        let rx = Quat::from_rotation_x(t * tumble.spin.x + tumble.phase.x);
        let ry = Quat::from_rotation_y(t * tumble.spin.y + tumble.phase.y);
        let rz = Quat::from_rotation_z(t * tumble.spin.z + tumble.phase.z);
        tf.rotation = rx * ry * rz;
    }
}

// ─────────────────────────────────────────────
//  Animation poussière
// ─────────────────────────────────────────────

fn animate_dust_halo(
    time:      Res<Time>,
    res:       Res<MeteoroidRes>,
    mut dust_q: Query<(&MeteoroidDust, &mut Transform)>,
) {
    let t = time.elapsed_secs();
    for (dv, mut tf) in &mut dust_q {
        let Some(cfg) = res.meteoroids.get(dv.idx) else { continue; };

        let speed = cfg.dust_drift_speed;
        let theta_a = dv.theta + t * speed * (1.0 + dv.seed * 0.5);
        let drift   = Vec3::new(
            (t * speed * 0.7 + dv.seed * 11.3).sin() * cfg.dust_radius * 0.08,
            (t * speed * 0.5 + dv.seed * 7.7).cos()  * cfg.dust_radius * 0.05,
            (t * speed * 0.9 + dv.seed * 5.1).sin()  * cfg.dust_radius * 0.06,
        );

        let pos = Vec3::new(
            dv.r * dv.phi.sin() * theta_a.cos(),
            dv.r * dv.phi.cos(),
            dv.r * dv.phi.sin() * theta_a.sin(),
        ) + drift;

        tf.translation = snap_grid(pos, cfg.dust_voxel_size);
        let shimmer = 0.5 + (t * 1.4 + dv.seed * std::f32::consts::TAU).sin().abs() * 0.6;
        tf.scale    = Vec3::splat(shimmer.max(0.05));
    }
}

// ─────────────────────────────────────────────
//  Animation dégazage
// ─────────────────────────────────────────────

fn animate_outgassing(
    time:      Res<Time>,
    res:       Res<MeteoroidRes>,
    mut out_q: Query<(&OutgassingVoxel, &mut Transform, &mut Visibility)>,
) {
    let t = time.elapsed_secs();
    for (ov, mut tf, mut vis) in &mut out_q {
        let Some(cfg) = res.meteoroids.get(ov.idx) else { *vis = Visibility::Hidden; continue; };
        if !cfg.outgassing_enabled { *vis = Visibility::Hidden; continue; }

        // Animation en boucle : le voxel remonte le long du jet
        let local_t = ((t * cfg.outgassing_speed + ov.sample_t) % 1.0).max(0.0);
        let dist    = cfg.radius + local_t * cfg.outgassing_jet_length;

        // Cône d'expansion + agitation latérale
        let spread_angle = ov.seed * std::f32::consts::TAU + t * 0.5;
        let spread_r     = local_t * cfg.radius * 0.25;
        let perp = if ov.jet_dir.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
        let right = ov.jet_dir.cross(perp).normalize();
        let fwd   = ov.jet_dir.cross(right).normalize();

        let pos = ov.jet_dir * dist
            + right * (spread_angle.cos() * spread_r)
            + fwd   * (spread_angle.sin() * spread_r);

        tf.translation = snap_grid(pos, cfg.outgassing_voxel_size);

        let fade = (1.0 - local_t).max(0.05);
        tf.scale = Vec3::splat(fade * (0.4 + ov.seed * 0.6));
        *vis = Visibility::Visible;
    }
}

// ─────────────────────────────────────────────
//  Détection fragmentation
// ─────────────────────────────────────────────

fn detect_fragmentation(
    mut commands:      Commands,
    res:               Res<MeteoroidRes>,
    root_q:            Query<(Entity, &GlobalTransform, &MeteoroidVelocity, &MeteoroidRoot)>,
    mut frag_events:   EventWriter<TriggerFragmentation>,
    mut impact_events: EventWriter<MeteoroidImpactEvent>,
    mut meshes:        ResMut<Assets<Mesh>>,
    mut materials:     ResMut<Assets<StandardMaterial>>,
    // NOTE : ajoute ici une query sur le monde voxel pour détecter les collisions
    // Pour l'instant, le trigger est manuel via TriggerFragmentation event
    mut trig_q:        EventReader<TriggerFragmentation>,
) {
    for ev in trig_q.read() {
        let Some(cfg) = res.meteoroids.get(ev.meteoroid_idx) else { continue; };

        // Trouver la root entity
        let Some((root_e, root_gt, vel, _)) = root_q.iter()
            .find(|(_, _, _, r)| r.idx == ev.meteoroid_idx)
        else { continue; };

        let impact_pos  = ev.impact_pos;
        let impact_vel  = vel.0;

        impact_events.send(MeteoroidImpactEvent {
            meteoroid_idx: ev.meteoroid_idx,
            position:      impact_pos,
            velocity:      impact_vel,
            composition:   cfg.composition.clone(),
        });

        // Spawn fragments
        if cfg.fragment_enabled {
            spawn_fragments(&mut commands, cfg, ev.meteoroid_idx, impact_pos, impact_vel, &mut meshes, &mut materials);
        }

        // Spawn explosion
        if cfg.explosion_enabled {
            spawn_explosion(&mut commands, cfg, ev.meteoroid_idx, impact_pos, &mut meshes, &mut materials);
        }

        // Despawn le météoroïde
        commands.entity(root_e).despawn_recursive();
    }
}

fn spawn_fragments(
    commands:  &mut Commands,
    cfg:       &MeteoroidConfig,
    idx:       usize,
    pos:       Vec3,
    base_vel:  Vec3,
    meshes:    &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) {
    let [br, bg, bb] = cfg.composition.base_color();
    let [vr, vg, vb] = cfg.composition.vein_color();

    for fi in 0..cfg.fragment_count {
        let h1 = pseudo_hash(cfg.seed as f32 + fi as f32, 10.0);
        let h2 = pseudo_hash(cfg.seed as f32 + fi as f32, 11.0);
        let h3 = pseudo_hash(cfg.seed as f32 + fi as f32, 12.0);
        let h4 = pseudo_hash(cfg.seed as f32 + fi as f32, 13.0);
        let h5 = pseudo_hash(cfg.seed as f32 + fi as f32, 14.0);

        // Direction d'éjection aléatoire (hémisphère de l'impact)
        let theta = h1 * std::f32::consts::TAU;
        let phi   = (h2 * 2.0 - 1.0).clamp(-1.0, 1.0).acos();
        let eject_dir = Vec3::new(phi.sin() * theta.cos(), phi.cos(), phi.sin() * theta.sin());
        let speed     = cfg.fragment_eject_speed * (0.4 + h3 * 0.8);
        let frag_vel  = base_vel * 0.3 + eject_dir * speed;

        let size = cfg.fragment_min_size + h4 * (cfg.fragment_max_size - cfg.fragment_min_size);
        let frag_spin = Vec3::new(
            pseudo_hash(h1, h5) * 4.0 - 2.0,
            pseudo_hash(h2, h5) * 4.0 - 2.0,
            pseudo_hash(h3, h5) * 4.0 - 2.0,
        );

        // Couleur : mélange surface + veine exposée
        let exposed = h5;
        let [fr, fg_, fb] = lerp_color([br, bg, bb], [vr, vg, vb], exposed);
        let fe = cfg.composition.impact_emissive() * exposed * 0.5;

        let frag_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(fr, fg_, fb),
            emissive:   LinearRgba::new(fr * fe, fg_ * fe, fb * fe, 1.0),
            perceptual_roughness: 0.85,
            ..default()
        });

        // Forme anguleuse pour les fragments
        let sx = size * (0.6 + pseudo_hash(h1, 3.3) * 0.8);
        let sy = size * (0.5 + pseudo_hash(h2, 3.3) * 0.7);
        let sz = size * (0.55 + pseudo_hash(h3, 3.3) * 0.75);
        let frag_mesh = meshes.add(Mesh::from(Cuboid::new(sx, sy, sz)));

        let offset = eject_dir * cfg.radius * 0.3;
        commands.spawn((
            Mesh3d(frag_mesh),
            MeshMaterial3d(frag_mat),
            Transform::from_translation(pos + offset),
            Visibility::default(),
            NotShadowCaster,
            MeteoroidFragment {
                idx,
                velocity:      frag_vel,
                spin:          frag_spin,
                lifetime:      0.0,
                max_lifetime:  cfg.fragment_lifetime * (0.5 + h5 * 0.8),
                composition:   cfg.composition.clone(),
            },
        ));
    }
}

fn spawn_explosion(
    commands:  &mut Commands,
    cfg:       &MeteoroidConfig,
    idx:       usize,
    pos:       Vec3,
    meshes:    &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) {
    let [er, eg, eb] = cfg.composition.explosion_color();
    let ee = cfg.composition.impact_emissive();

    let exp_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(er, eg, eb),
        emissive:   LinearRgba::new(er * ee, eg * ee, eb * ee, 1.0),
        alpha_mode: AlphaMode::Add,
        unlit:      true,
        ..default()
    });
    let exp_mesh = meshes.add(Mesh::from(Cuboid::new(
        cfg.explosion_voxel_size, cfg.explosion_voxel_size, cfg.explosion_voxel_size,
    )));

    for ei in 0..cfg.explosion_voxel_count {
        let h1 = pseudo_hash(cfg.seed as f32 + 500.0, ei as f32);
        let h2 = pseudo_hash(cfg.seed as f32 + 501.0, ei as f32);
        let h3 = pseudo_hash(cfg.seed as f32 + 502.0, ei as f32);

        let theta = h1 * std::f32::consts::TAU;
        let phi   = (h2 * 2.0 - 1.0).clamp(-1.0, 1.0).acos();
        let dir   = Vec3::new(phi.sin() * theta.cos(), phi.cos(), phi.sin() * theta.sin());
        let speed = cfg.explosion_speed * (0.5 + h3 * 0.8);

        commands.spawn((
            Mesh3d(exp_mesh.clone()),
            MeshMaterial3d(exp_mat.clone()),
            Transform::from_translation(pos),
            Visibility::default(),
            NotShadowCaster,
            ExplosionVoxel {
                idx,
                dir: dir.normalize(),
                seed: pseudo_hash(h1, h2),
                speed,
                elapsed: 0.0,
                duration: cfg.explosion_duration,
                max_radius: cfg.explosion_max_radius,
            },
        ));
    }
}

// ─────────────────────────────────────────────
//  Animation fragments
// ─────────────────────────────────────────────

fn animate_fragments(
    time:       Res<Time>,
    mut frag_q: Query<(Entity, &mut Transform, &mut MeteoroidFragment)>,
    mut commands: Commands,
) {
    let dt = time.delta_secs();
    for (entity, mut tf, mut frag) in &mut frag_q {
        frag.lifetime += dt;

        if frag.lifetime >= frag.max_lifetime {
            commands.entity(entity).despawn_recursive();
            continue;
        }

        // Déplacement + rotation tumbling
        tf.translation += frag.velocity * dt;
        // Friction légère (ralentissement dans l'espace — peut être retiré)
        frag.velocity  *= 1.0 - dt * 0.05;

        let t = frag.lifetime;
        let rx = Quat::from_rotation_x(t * frag.spin.x);
        let ry = Quat::from_rotation_y(t * frag.spin.y);
        let rz = Quat::from_rotation_z(t * frag.spin.z);
        tf.rotation = rx * ry * rz;

        // Fade progressif vers la fin de vie
        let life_t = (frag.lifetime / frag.max_lifetime).clamp(0.0, 1.0);
        let fade   = (1.0 - life_t * life_t).max(0.05);
        tf.scale   = Vec3::splat(fade);
    }
}

// ─────────────────────────────────────────────
//  Animation explosion
// ─────────────────────────────────────────────

fn animate_explosion(
    time:      Res<Time>,
    mut exp_q: Query<(Entity, &mut Transform, &mut Visibility, &mut ExplosionVoxel)>,
    mut commands: Commands,
) {
    let dt = time.delta_secs();
    for (entity, mut tf, mut vis, mut ev) in &mut exp_q {
        ev.elapsed += dt;

        if ev.elapsed >= ev.duration {
            commands.entity(entity).despawn_recursive();
            continue;
        }

        let radius = ev.speed * ev.elapsed;
        if radius > ev.max_radius {
            commands.entity(entity).despawn_recursive();
            continue;
        }

        // Jitter de position (turbulence de l'explosion)
        let jitter = (ev.seed * std::f32::consts::TAU + ev.elapsed * 12.0).sin()
            * radius * 0.04;
        let pos    = ev.dir * (radius + jitter);
        tf.translation = snap_grid(pos, 4.0);

        // Fade + scale
        let life  = 1.0 - (ev.elapsed / ev.duration).clamp(0.0, 1.0);
        let scale = life * (0.3 + ev.seed * 0.8);
        tf.scale  = Vec3::splat(scale.max(0.02));
        *vis      = Visibility::Visible;
    }
}

// ─────────────────────────────────────────────
//  Régénération à chaud
// ─────────────────────────────────────────────

fn regenerate_meteoroids(
    mut commands:  Commands,
    mut events:    EventReader<RegenerateMeteoroid>,
    res:           Res<MeteoroidRes>,
    root_q:        Query<Entity, With<MeteoroidRoot>>,
    frag_q:        Query<Entity, With<MeteoroidFragment>>,
    exp_q:         Query<Entity, With<ExplosionVoxel>>,
    mut meshes:    ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut fired = false;
    for _ in events.read() { fired = true; }
    if !fired { return; }

    for e in &root_q  { commands.entity(e).despawn_recursive(); }
    for e in &frag_q  { commands.entity(e).despawn_recursive(); }
    for e in &exp_q   { commands.entity(e).despawn_recursive(); }

    for (idx, cfg) in res.meteoroids.iter().enumerate() {
        build_meteoroid(&mut commands, cfg, idx, &mut meshes, &mut materials);
    }
}
