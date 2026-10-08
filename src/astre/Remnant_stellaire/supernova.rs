use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;
use crate::astre::{AstreLodRoot, ReloadAstre};

#[derive(Clone, Debug, PartialEq)]
pub enum SupernovaType { TypeIa, TypeII }

#[derive(Clone, Debug, PartialEq)]
pub enum SupernovaPhase {
    Progenitor,
    FlashPeak  { elapsed: f32 },
    Fireball   { elapsed: f32 },
    Plateau    { elapsed: f32 },
    Remnant    { elapsed: f32 },
}

#[derive(Clone, Debug)]
pub struct SupernovaConfig {
    pub position:                   Vec3,
    pub orbit_distance:             f32,
    pub orbit_speed:                f32,
    pub sn_type:                    SupernovaType,
    pub start_exploding:            bool,
    pub flash_duration:             f32,
    pub fireball_duration:          f32,
    pub plateau_duration:           f32,
    pub progenitor_radius:          f32,
    pub progenitor_voxel_size:      f32,
    pub progenitor_resolution:      u32,
    pub progenitor_color:           [f32; 3],
    pub progenitor_emissive:        f32,
    pub flash_voxel_size:           f32,
    pub flash_voxel_count:          u32,
    pub flash_color:                [f32; 3],
    pub flash_emissive:             f32,
    pub flash_max_radius:           f32,
    pub flash_speed:                f32,
    pub fireball_voxel_size:        f32,
    pub fireball_voxel_count:       u32,
    pub fireball_color_core:        [f32; 3],
    pub fireball_color_edge:        [f32; 3],
    pub fireball_emissive:          f32,
    pub fireball_max_radius:        f32,
    pub fireball_speed:             f32,
    pub fireball_polar_boost:       f32,
    pub fireball_turbulence:        f32,
    pub jet_enabled:                bool,
    pub jet_length:                 f32,
    pub jet_width:                  f32,
    pub jet_voxel_size:             f32,
    pub jet_voxel_count:            u32,
    pub jet_color:                  [f32; 3],
    pub jet_emissive:               f32,
    pub jet_speed:                  f32,
    pub blast_voxel_size:           f32,
    pub blast_voxel_count:          u32,
    pub blast_color:                [f32; 3],
    pub blast_emissive:             f32,
    pub blast_max_radius:           f32,
    pub blast_speed:                f32,
    pub blast_thickness:            f32,
    pub echo_enabled:               bool,
    pub echo_voxel_size:            f32,
    pub echo_voxel_count:           u32,
    pub echo_color:                 [f32; 3],
    pub echo_emissive:              f32,
    pub echo_max_radius:            f32,
    pub echo_speed:                 f32,
    pub echo_lead:                  f32,
    pub ionization_enabled:         bool,
    pub ionization_voxel_size:      f32,
    pub ionization_voxel_count:     u32,
    pub ionization_color:           [f32; 3],
    pub ionization_emissive:        f32,
    pub ionization_width:           f32,
    pub snr_enabled:                bool,
    pub snr_inner_radius:           f32,
    pub snr_outer_radius:           f32,
    pub snr_voxel_size:             f32,
    pub snr_voxel_count:            u32,
    pub snr_color_inner:            [f32; 3],
    pub snr_color_outer:            [f32; 3],
    pub snr_emissive:               f32,
    pub snr_drift_speed:            f32,
    pub snr_drift_amplitude:        f32,
    pub snr_filament_count:         u32,
    pub snr_filament_samples:       u32,
    pub snr_filament_color:         [f32; 3],
    pub snr_filament_emissive:      f32,
    pub snr_expansion_speed:        f32,
    pub remnant_core_enabled:       bool,
    pub remnant_core_radius:        f32,
    pub remnant_core_color:         [f32; 3],
    pub remnant_core_emissive:      f32,
    pub seed:                       u32,
}

impl SupernovaConfig {
    pub fn type_ia(seed: u32, pos: Vec3) -> Self {
        Self {
            position: pos, orbit_distance: 0.0, orbit_speed: 0.0,
            sn_type: SupernovaType::TypeIa, start_exploding: false,
            flash_duration: 4.0, fireball_duration: 20.0, plateau_duration: 60.0,
            progenitor_radius: 30.0, progenitor_voxel_size: 5.0, progenitor_resolution: 14,
            progenitor_color: [0.7, 0.8, 1.0], progenitor_emissive: 6.0,
            flash_voxel_size: 10.0, flash_voxel_count: 300,
            flash_color: [1.0, 1.0, 0.95], flash_emissive: 35.0,
            flash_max_radius: 600.0, flash_speed: 280.0,
            fireball_voxel_size: 14.0, fireball_voxel_count: 800,
            fireball_color_core: [1.0, 0.9, 0.5], fireball_color_edge: [0.8, 0.3, 0.05],
            fireball_emissive: 14.0, fireball_max_radius: 1200.0, fireball_speed: 85.0,
            fireball_polar_boost: 0.0, fireball_turbulence: 18.0,
            jet_enabled: false, jet_length: 0.0, jet_width: 0.0,
            jet_voxel_size: 8.0, jet_voxel_count: 0,
            jet_color: [1.0, 1.0, 1.0], jet_emissive: 0.0, jet_speed: 0.0,
            blast_voxel_size: 12.0, blast_voxel_count: 500,
            blast_color: [0.9, 0.6, 0.2], blast_emissive: 8.0,
            blast_max_radius: 1800.0, blast_speed: 60.0, blast_thickness: 40.0,
            echo_enabled: true, echo_voxel_size: 10.0, echo_voxel_count: 400,
            echo_color: [0.9, 0.85, 0.7], echo_emissive: 4.0,
            echo_max_radius: 2400.0, echo_speed: 320.0, echo_lead: 120.0,
            ionization_enabled: true, ionization_voxel_size: 8.0, ionization_voxel_count: 350,
            ionization_color: [0.3, 0.7, 1.0], ionization_emissive: 6.0, ionization_width: 30.0,
            snr_enabled: true, snr_inner_radius: 200.0, snr_outer_radius: 700.0,
            snr_voxel_size: 16.0, snr_voxel_count: 600,
            snr_color_inner: [0.9, 0.5, 0.1], snr_color_outer: [0.2, 0.1, 0.5],
            snr_emissive: 2.5, snr_drift_speed: 0.08, snr_drift_amplitude: 10.0,
            snr_filament_count: 16, snr_filament_samples: 35,
            snr_filament_color: [1.0, 0.7, 0.3], snr_filament_emissive: 4.0,
            snr_expansion_speed: 0.4,
            remnant_core_enabled: false, remnant_core_radius: 0.0,
            remnant_core_color: [1.0, 1.0, 1.0], remnant_core_emissive: 0.0,
            seed,
        }
    }

    pub fn type_ii(seed: u32, pos: Vec3) -> Self {
        Self {
            position: pos, orbit_distance: 0.0, orbit_speed: 0.0,
            sn_type: SupernovaType::TypeII, start_exploding: false,
            flash_duration: 6.0, fireball_duration: 30.0, plateau_duration: 90.0,
            progenitor_radius: 80.0, progenitor_voxel_size: 8.0, progenitor_resolution: 16,
            progenitor_color: [0.9, 0.35, 0.1], progenitor_emissive: 4.0,
            flash_voxel_size: 12.0, flash_voxel_count: 350,
            flash_color: [1.0, 0.98, 0.9], flash_emissive: 28.0,
            flash_max_radius: 700.0, flash_speed: 240.0,
            fireball_voxel_size: 16.0, fireball_voxel_count: 1000,
            fireball_color_core: [1.0, 0.85, 0.4], fireball_color_edge: [0.65, 0.15, 0.05],
            fireball_emissive: 12.0, fireball_max_radius: 1500.0, fireball_speed: 65.0,
            fireball_polar_boost: 0.55, fireball_turbulence: 28.0,
            jet_enabled: true, jet_length: 900.0, jet_width: 60.0,
            jet_voxel_size: 10.0, jet_voxel_count: 200,
            jet_color: [0.5, 0.8, 1.0], jet_emissive: 10.0, jet_speed: 160.0,
            blast_voxel_size: 14.0, blast_voxel_count: 600,
            blast_color: [0.85, 0.45, 0.1], blast_emissive: 7.0,
            blast_max_radius: 2200.0, blast_speed: 50.0, blast_thickness: 60.0,
            echo_enabled: true, echo_voxel_size: 12.0, echo_voxel_count: 450,
            echo_color: [0.85, 0.75, 0.55], echo_emissive: 3.5,
            echo_max_radius: 3000.0, echo_speed: 310.0, echo_lead: 150.0,
            ionization_enabled: true, ionization_voxel_size: 10.0, ionization_voxel_count: 400,
            ionization_color: [0.2, 0.6, 1.0], ionization_emissive: 7.0, ionization_width: 45.0,
            snr_enabled: true, snr_inner_radius: 300.0, snr_outer_radius: 900.0,
            snr_voxel_size: 18.0, snr_voxel_count: 700,
            snr_color_inner: [0.8, 0.4, 0.1], snr_color_outer: [0.15, 0.1, 0.45],
            snr_emissive: 3.0, snr_drift_speed: 0.06, snr_drift_amplitude: 14.0,
            snr_filament_count: 22, snr_filament_samples: 40,
            snr_filament_color: [0.9, 0.6, 0.25], snr_filament_emissive: 5.0,
            snr_expansion_speed: 0.3,
            remnant_core_enabled: true, remnant_core_radius: 12.0,
            remnant_core_color: [0.5, 0.75, 1.0], remnant_core_emissive: 18.0,
            seed,
        }
    }
}

pub struct SupernovaPlugin;
impl Plugin for SupernovaPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<SupernovaRes>()
            .init_resource::<SupernovaState>()
            .add_event::<RegenerateSupernova>()
            .add_event::<TriggerExplosion>()
            .add_event::<SupernovaPhaseChanged>()
            .add_systems(Startup, spawn_supernovae)
            .add_systems(Update, (
                orbit_supernovae,
                tick_phase,
                animate_progenitor,
                animate_flash,
                animate_fireball,
                animate_jet,
                animate_blast_wave,
                animate_light_echo,
                animate_ionization_front,
                animate_snr_voxels,
                animate_snr_filaments,
                animate_remnant_core,
                handle_trigger_explosion,
                regenerate_supernovae,
                reload_supernovae,
            ).chain());
    }
}

#[derive(Resource)]
pub struct SupernovaRes { pub supernovae: Vec<SupernovaConfig> }
impl Default for SupernovaRes {
    fn default() -> Self {
        Self { supernovae: vec![SupernovaConfig::type_ii(42, Vec3::new(0.0, 0.0, -9000.0))] }
    }
}

#[derive(Resource, Default)]
pub struct SupernovaState { pub phases: Vec<SupernovaPhase> }

#[derive(Event)] pub struct RegenerateSupernova;
#[derive(Event)] pub struct TriggerExplosion { pub idx: usize }
#[derive(Event)] pub struct SupernovaPhaseChanged { pub idx: usize, pub new_phase: String }

#[derive(Component)] pub struct SupernovaRoot   { pub idx: usize }
#[derive(Component)] pub struct ProgenitorVoxel { pub idx: usize, pub lat: f32, pub lon: f32 }
#[derive(Component)] pub struct FlashVoxel      { pub idx: usize, pub dir: Vec3, pub seed: f32 }
#[derive(Component)] pub struct FireballVoxel   { pub idx: usize, pub dir: Vec3, pub seed: f32, pub color_t: f32, pub polar_t: f32 }
#[derive(Component)] pub struct SnovaJetVoxel   { pub idx: usize, pub pole: f32, pub t: f32, pub perp_seed: f32, pub seed: f32 }
#[derive(Component)] pub struct BlastWaveVoxel  { pub idx: usize, pub dir: Vec3, pub seed: f32 }
#[derive(Component)] pub struct LightEchoVoxel  { pub idx: usize, pub dir: Vec3, pub seed: f32, pub r_off: f32 }
#[derive(Component)] pub struct IonizationVoxel { pub idx: usize, pub dir: Vec3, pub seed: f32 }
#[derive(Component)] pub struct SnrVoxel        { pub idx: usize, pub origin: Vec3, pub seed: f32, pub color_t: f32, pub r0: f32 }
#[derive(Component)] pub struct SnrFilament     { pub idx: usize, pub filament_idx: u32, pub sample_idx: u32, pub lon: f32, pub tilt: f32, pub seed: f32 }
#[derive(Component)] pub struct RemnantCore     { pub idx: usize }

fn pseudo_hash(a: f32, b: f32) -> f32 {
    ((a * 12.9898 + b * 78.233).sin() * 43758.5453).fract()
}
fn snap_grid(v: Vec3, grid: f32) -> Vec3 {
    Vec3::new((v.x/grid).round()*grid, (v.y/grid).round()*grid, (v.z/grid).round()*grid)
}
fn lerp_color(a: [f32;3], b: [f32;3], t: f32) -> [f32;3] {
    let t = t.clamp(0.0,1.0);
    [a[0]+(b[0]-a[0])*t, a[1]+(b[1]-a[1])*t, a[2]+(b[2]-a[2])*t]
}
fn sphere_pos(lat: f32, lon: f32, r: f32) -> Vec3 {
    Vec3::new(r*lat.cos()*lon.cos(), r*lat.sin(), r*lat.cos()*lon.sin())
}
fn rand_sphere(h1: f32, h2: f32) -> Vec3 {
    let theta = h1 * std::f32::consts::TAU;
    let phi   = (h2*2.0-1.0).clamp(-1.0,1.0).acos();
    Vec3::new(phi.sin()*theta.cos(), phi.cos(), phi.sin()*theta.sin()).normalize()
}
fn phase_elapsed(state: &SupernovaState, idx: usize) -> f32 {
    match state.phases.get(idx) {
        Some(SupernovaPhase::FlashPeak{elapsed})|Some(SupernovaPhase::Fireball{elapsed})|
        Some(SupernovaPhase::Plateau{elapsed})|Some(SupernovaPhase::Remnant{elapsed}) => *elapsed,
        _ => 0.0,
    }
}

fn spawn_supernovae(
    mut commands: Commands, res: Res<SupernovaRes>, mut state: ResMut<SupernovaState>,
    mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>,
) {
    state.phases.clear();
    for (idx, cfg) in res.supernovae.iter().enumerate() {
        state.phases.push(if cfg.start_exploding {
            SupernovaPhase::FlashPeak{elapsed:0.0}
        } else {
            SupernovaPhase::Progenitor
        });
        build_supernova(&mut commands, cfg, idx, &mut meshes, &mut materials);
    }
}

fn build_supernova(
    commands: &mut Commands, cfg: &SupernovaConfig, idx: usize,
    meshes: &mut ResMut<Assets<Mesh>>, materials: &mut ResMut<Assets<StandardMaterial>>,
) {
    let init_pos = if cfg.orbit_distance > 1.0 { Vec3::new(cfg.orbit_distance,0.0,0.0) } else { cfg.position };
    let root = commands.spawn((Transform::from_translation(init_pos), Visibility::default(), SupernovaRoot{idx}, AstreLodRoot{cull_dist:10000.0, radius: cfg.blast_max_radius.max(cfg.progenitor_radius), streamable: true, label: "Supernova"})).id();

    // ── Progéniteur ───────────────────────────────────────────────────────
    {
        let [pr,pg,pb] = cfg.progenitor_color; let pe = cfg.progenitor_emissive;
        let mat = materials.add(StandardMaterial { base_color: Color::srgb(pr,pg,pb),
            emissive: LinearRgba::new(pr*pe,pg*pe,pb*pe,1.0), unlit:true, ..default() });
        let mesh = meshes.add(Mesh::from(Cuboid::new(cfg.progenitor_voxel_size,cfg.progenitor_voxel_size,cfg.progenitor_voxel_size)));
        let res_s = cfg.progenitor_resolution;
        let vstep = std::f32::consts::PI / res_s as f32;
        let hstep = std::f32::consts::TAU / (res_s*2) as f32;
        let mut lat = -std::f32::consts::FRAC_PI_2;
        while lat <= std::f32::consts::FRAC_PI_2 {
            let mut lon = 0.0_f32;
            while lon < std::f32::consts::TAU {
                let deform = pseudo_hash(lat*10.0+cfg.seed as f32, lon*5.0)*0.12-0.06;
                let r = cfg.progenitor_radius*(1.0+deform);
                let v = commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(mat.clone()),
                    Transform::from_translation(snap_grid(sphere_pos(lat,lon,r), cfg.progenitor_voxel_size)),
                    Visibility::default(), NotShadowCaster, ProgenitorVoxel{idx,lat,lon})).id();
                commands.entity(root).add_child(v);
                lon += hstep;
            }
            lat += vstep;
        }
    }

    // ── Flash ─────────────────────────────────────────────────────────────
    {
        let [fr,fg,fb]=cfg.flash_color; let fe=cfg.flash_emissive;
        let mat=materials.add(StandardMaterial{base_color:Color::srgb(fr,fg,fb),
            emissive:LinearRgba::new(fr*fe,fg*fe,fb*fe,1.0),alpha_mode:AlphaMode::Add,unlit:true,..default()});
        let mesh=meshes.add(Mesh::from(Cuboid::new(cfg.flash_voxel_size,cfg.flash_voxel_size,cfg.flash_voxel_size)));
        for i in 0..cfg.flash_voxel_count {
            let h1=pseudo_hash(cfg.seed as f32+10.0,i as f32);
            let h2=pseudo_hash(cfg.seed as f32+11.0,i as f32);
            let dir=rand_sphere(h1,h2);
            commands.spawn((Mesh3d(mesh.clone()),MeshMaterial3d(mat.clone()),
                Transform::from_translation(Vec3::ZERO).with_scale(Vec3::ZERO),
                Visibility::Hidden,NotShadowCaster,FlashVoxel{idx,dir,seed:pseudo_hash(h1,h2)}));
        }
    }

    // ── Boule de feu ──────────────────────────────────────────────────────
    {
        let mesh=meshes.add(Mesh::from(Cuboid::new(cfg.fireball_voxel_size,cfg.fireball_voxel_size,cfg.fireball_voxel_size)));
        for i in 0..cfg.fireball_voxel_count {
            let h1=pseudo_hash(cfg.seed as f32+20.0,i as f32);
            let h2=pseudo_hash(cfg.seed as f32+21.0,i as f32);
            let h3=pseudo_hash(cfg.seed as f32+22.0,i as f32);
            let dir=rand_sphere(h1,h2);
            let color_t=h3;
            let [rr,rg,rb]=lerp_color(cfg.fireball_color_core,cfg.fireball_color_edge,color_t);
            let re=cfg.fireball_emissive*(1.0-color_t*0.65);
            let mat=materials.add(StandardMaterial{base_color:Color::srgb(rr,rg,rb),
                emissive:LinearRgba::new(rr*re,rg*re,rb*re,1.0),alpha_mode:AlphaMode::Add,unlit:true,..default()});
            commands.spawn((Mesh3d(mesh.clone()),MeshMaterial3d(mat),
                Transform::from_translation(Vec3::ZERO).with_scale(Vec3::ZERO),
                Visibility::Hidden,NotShadowCaster,
                FireballVoxel{idx,dir,seed:pseudo_hash(h1,h3),color_t,polar_t:dir.y.abs()}));
        }
    }

    // ── Jets Type II ──────────────────────────────────────────────────────
    if cfg.jet_enabled {
        let [jr,jg,jb]=cfg.jet_color; let je=cfg.jet_emissive;
        let mat=materials.add(StandardMaterial{base_color:Color::srgb(jr,jg,jb),
            emissive:LinearRgba::new(jr*je,jg*je,jb*je,1.0),alpha_mode:AlphaMode::Add,unlit:true,..default()});
        let mesh=meshes.add(Mesh::from(Cuboid::new(cfg.jet_voxel_size,cfg.jet_voxel_size,cfg.jet_voxel_size)));
        for pole in [1.0_f32,-1.0_f32] {
            for i in 0..cfg.jet_voxel_count {
                let h1=pseudo_hash(cfg.seed as f32+30.0+pole*20.0,i as f32);
                let h2=pseudo_hash(cfg.seed as f32+31.0+pole*20.0,i as f32);
                commands.spawn((Mesh3d(mesh.clone()),MeshMaterial3d(mat.clone()),
                    Transform::from_translation(Vec3::ZERO).with_scale(Vec3::ZERO),
                    Visibility::Hidden,NotShadowCaster,
                    SnovaJetVoxel{idx,pole,t:h1,perp_seed:h2,seed:pseudo_hash(h1,h2)}));
            }
        }
    }

    // ── Onde de choc ──────────────────────────────────────────────────────
    {
        let [br,bg,bb]=cfg.blast_color; let be=cfg.blast_emissive;
        let mat=materials.add(StandardMaterial{base_color:Color::srgb(br,bg,bb),
            emissive:LinearRgba::new(br*be,bg*be,bb*be,1.0),alpha_mode:AlphaMode::Add,unlit:true,..default()});
        let mesh=meshes.add(Mesh::from(Cuboid::new(cfg.blast_voxel_size,cfg.blast_voxel_size,cfg.blast_voxel_size)));
        for i in 0..cfg.blast_voxel_count {
            let h1=pseudo_hash(cfg.seed as f32+40.0,i as f32);
            let h2=pseudo_hash(cfg.seed as f32+41.0,i as f32);
            commands.spawn((Mesh3d(mesh.clone()),MeshMaterial3d(mat.clone()),
                Transform::from_translation(Vec3::ZERO).with_scale(Vec3::ZERO),
                Visibility::Hidden,NotShadowCaster,
                BlastWaveVoxel{idx,dir:rand_sphere(h1,h2),seed:pseudo_hash(h1,h2)}));
        }
    }

    // ── Light echo ────────────────────────────────────────────────────────
    if cfg.echo_enabled {
        let [er,eg,eb]=cfg.echo_color; let ee=cfg.echo_emissive;
        let mat=materials.add(StandardMaterial{base_color:Color::srgb(er,eg,eb),
            emissive:LinearRgba::new(er*ee,eg*ee,eb*ee,1.0),alpha_mode:AlphaMode::Add,unlit:true,..default()});
        let mesh=meshes.add(Mesh::from(Cuboid::new(cfg.echo_voxel_size,cfg.echo_voxel_size,cfg.echo_voxel_size)));
        for i in 0..cfg.echo_voxel_count {
            let h1=pseudo_hash(cfg.seed as f32+50.0,i as f32);
            let h2=pseudo_hash(cfg.seed as f32+51.0,i as f32);
            let h3=pseudo_hash(cfg.seed as f32+52.0,i as f32);
            commands.spawn((Mesh3d(mesh.clone()),MeshMaterial3d(mat.clone()),
                Transform::from_translation(Vec3::ZERO).with_scale(Vec3::ZERO),
                Visibility::Hidden,NotShadowCaster,
                LightEchoVoxel{idx,dir:rand_sphere(h1,h2),seed:pseudo_hash(h1,h3),r_off:(h3*2.0-1.0)*cfg.echo_lead*0.5}));
        }
    }

    // ── Front d'ionisation ─────────────────────────────────────────────────
    if cfg.ionization_enabled {
        let [ir,ig,ib]=cfg.ionization_color; let ie=cfg.ionization_emissive;
        let mat=materials.add(StandardMaterial{base_color:Color::srgb(ir,ig,ib),
            emissive:LinearRgba::new(ir*ie,ig*ie,ib*ie,1.0),alpha_mode:AlphaMode::Add,unlit:true,..default()});
        let mesh=meshes.add(Mesh::from(Cuboid::new(cfg.ionization_voxel_size,cfg.ionization_voxel_size,cfg.ionization_voxel_size)));
        for i in 0..cfg.ionization_voxel_count {
            let h1=pseudo_hash(cfg.seed as f32+60.0,i as f32);
            let h2=pseudo_hash(cfg.seed as f32+61.0,i as f32);
            commands.spawn((Mesh3d(mesh.clone()),MeshMaterial3d(mat.clone()),
                Transform::from_translation(Vec3::ZERO).with_scale(Vec3::ZERO),
                Visibility::Hidden,NotShadowCaster,
                IonizationVoxel{idx,dir:rand_sphere(h1,h2),seed:pseudo_hash(h1,h2)}));
        }
    }

    // ── SNR ───────────────────────────────────────────────────────────────
    if cfg.snr_enabled {
        let mesh=meshes.add(Mesh::from(Cuboid::new(cfg.snr_voxel_size,cfg.snr_voxel_size,cfg.snr_voxel_size)));
        for i in 0..cfg.snr_voxel_count {
            let h1=pseudo_hash(cfg.seed as f32+70.0,i as f32);
            let h2=pseudo_hash(cfg.seed as f32+71.0,i as f32);
            let h3=pseudo_hash(cfg.seed as f32+72.0,i as f32);
            let h4=pseudo_hash(cfg.seed as f32+73.0,i as f32);
            let dir=rand_sphere(h1,h2);
            let r=cfg.snr_inner_radius+h3.powf(0.6)*(cfg.snr_outer_radius-cfg.snr_inner_radius);
            let color_t=((r-cfg.snr_inner_radius)/(cfg.snr_outer_radius-cfg.snr_inner_radius)).clamp(0.0,1.0);
            let [rr,rg,rb]=lerp_color(cfg.snr_color_inner,cfg.snr_color_outer,color_t);
            let re=cfg.snr_emissive*(1.0-color_t*0.5);
            let mat=materials.add(StandardMaterial{base_color:Color::srgb(rr,rg,rb),
                emissive:LinearRgba::new(rr*re,rg*re,rb*re,1.0),alpha_mode:AlphaMode::Add,unlit:true,..default()});
            let origin=snap_grid(dir*r, cfg.snr_voxel_size);
            commands.spawn((Mesh3d(mesh.clone()),MeshMaterial3d(mat),
                Transform::from_translation(origin),Visibility::Hidden,NotShadowCaster,
                SnrVoxel{idx,origin,seed:h4,color_t,r0:r}));
        }
        // filaments SNR
        let fil_mesh=meshes.add(Mesh::from(Cuboid::new(cfg.snr_voxel_size*0.5,cfg.snr_voxel_size*0.5,cfg.snr_voxel_size*0.5)));
        let [fr,fg,fb]=cfg.snr_filament_color; let fe=cfg.snr_filament_emissive;
        let fil_mat=materials.add(StandardMaterial{base_color:Color::srgb(fr,fg,fb),
            emissive:LinearRgba::new(fr*fe,fg*fe,fb*fe,1.0),alpha_mode:AlphaMode::Add,unlit:true,..default()});
        for fi in 0..cfg.snr_filament_count {
            let lon=fi as f32/cfg.snr_filament_count as f32*std::f32::consts::TAU;
            let tilt=0.2+pseudo_hash(cfg.seed as f32+fi as f32,77.7)*std::f32::consts::FRAC_PI_2*0.8;
            for si in 0..cfg.snr_filament_samples {
                let seed=pseudo_hash(fi as f32*3.7,si as f32*2.3);
                commands.spawn((Mesh3d(fil_mesh.clone()),MeshMaterial3d(fil_mat.clone()),
                    Transform::from_translation(Vec3::ZERO),Visibility::Hidden,NotShadowCaster,
                    SnrFilament{idx,filament_idx:fi,sample_idx:si,lon,tilt,seed}));
            }
        }
    }

    // ── Résidu compact ────────────────────────────────────────────────────
    if cfg.remnant_core_enabled {
        let [cr,cg,cb]=cfg.remnant_core_color; let ce=cfg.remnant_core_emissive;
        let mat=materials.add(StandardMaterial{base_color:Color::srgb(cr,cg,cb),
            emissive:LinearRgba::new(cr*ce,cg*ce,cb*ce,1.0),unlit:true,..default()});
        let r=cfg.remnant_core_radius;
        let mesh=meshes.add(Mesh::from(Cuboid::new(r,r,r)));
        commands.spawn((Mesh3d(mesh),MeshMaterial3d(mat),Transform::IDENTITY,Visibility::Hidden,NotShadowCaster,RemnantCore{idx}));
    }
}

fn orbit_supernovae(time:Res<Time>,res:Res<SupernovaRes>,mut root_q:Query<(&mut Transform,&SupernovaRoot)>) {
    let t=time.elapsed_secs();
    for (mut tf,root) in &mut root_q {
        let Some(cfg)=res.supernovae.get(root.idx) else { continue; };
        if cfg.orbit_distance>1.0 {
            let a=t*cfg.orbit_speed;
            tf.translation.x=a.cos()*cfg.orbit_distance;
            tf.translation.z=a.sin()*cfg.orbit_distance;
        }
    }
}

fn tick_phase(time:Res<Time>,res:Res<SupernovaRes>,mut state:ResMut<SupernovaState>,mut ev:EventWriter<SupernovaPhaseChanged>) {
    let dt=time.delta_secs();
    for (idx,cfg) in res.supernovae.iter().enumerate() {
        while state.phases.len()<=idx { state.phases.push(SupernovaPhase::Progenitor); }
        let next=match &state.phases[idx] {
            SupernovaPhase::Progenitor => None,
            SupernovaPhase::FlashPeak{elapsed} => {
                let e=elapsed+dt;
                if e>=cfg.flash_duration { ev.send(SupernovaPhaseChanged{idx,new_phase:"Fireball".into()}); Some(SupernovaPhase::Fireball{elapsed:0.0}) }
                else { Some(SupernovaPhase::FlashPeak{elapsed:e}) }
            }
            SupernovaPhase::Fireball{elapsed} => {
                let e=elapsed+dt;
                if e>=cfg.fireball_duration { ev.send(SupernovaPhaseChanged{idx,new_phase:"Plateau".into()}); Some(SupernovaPhase::Plateau{elapsed:0.0}) }
                else { Some(SupernovaPhase::Fireball{elapsed:e}) }
            }
            SupernovaPhase::Plateau{elapsed} => {
                let e=elapsed+dt;
                if e>=cfg.plateau_duration { ev.send(SupernovaPhaseChanged{idx,new_phase:"Remnant".into()}); Some(SupernovaPhase::Remnant{elapsed:0.0}) }
                else { Some(SupernovaPhase::Plateau{elapsed:e}) }
            }
            SupernovaPhase::Remnant{elapsed} => Some(SupernovaPhase::Remnant{elapsed:elapsed+dt}),
        };
        if let Some(p)=next { state.phases[idx]=p; }
    }
}

fn handle_trigger_explosion(mut events:EventReader<TriggerExplosion>,mut state:ResMut<SupernovaState>,mut ev:EventWriter<SupernovaPhaseChanged>) {
    for e in events.read() {
        while state.phases.len()<=e.idx { state.phases.push(SupernovaPhase::Progenitor); }
        if state.phases[e.idx]==SupernovaPhase::Progenitor {
            state.phases[e.idx]=SupernovaPhase::FlashPeak{elapsed:0.0};
            ev.send(SupernovaPhaseChanged{idx:e.idx,new_phase:"FlashPeak".into()});
        }
    }
}

fn animate_progenitor(time:Res<Time>,state:Res<SupernovaState>,mut q:Query<(&ProgenitorVoxel,&mut Transform,&mut Visibility)>) {
    let t=time.elapsed_secs();
    for (pv,mut tf,mut vis) in &mut q {
        let visible=matches!(state.phases.get(pv.idx),Some(SupernovaPhase::Progenitor));
        *vis=if visible{Visibility::Visible}else{Visibility::Hidden};
        if visible { let p=1.0+(t*0.4+pv.lat*3.0+pv.lon*1.5).sin()*0.06; tf.scale=Vec3::splat(p); }
    }
}

fn animate_flash(res:Res<SupernovaRes>,state:Res<SupernovaState>,mut q:Query<(&FlashVoxel,&mut Transform,&mut Visibility)>) {
    for (fv,mut tf,mut vis) in &mut q {
        let Some(cfg)=res.supernovae.get(fv.idx) else { *vis=Visibility::Hidden; continue; };
        if !matches!(state.phases.get(fv.idx),Some(SupernovaPhase::FlashPeak{..})) { *vis=Visibility::Hidden; continue; }
        let e=phase_elapsed(&state,fv.idx);
        let r=cfg.flash_speed*e;
        if r>cfg.flash_max_radius { *vis=Visibility::Hidden; continue; }
        let j=(fv.seed*std::f32::consts::TAU+e*20.0).sin()*r*0.03;
        tf.translation=snap_grid(fv.dir*(r+j),cfg.flash_voxel_size);
        let life=1.0-(r/cfg.flash_max_radius).clamp(0.0,1.0);
        let sh=0.6+(e*15.0+fv.seed*9.0).sin()*0.4;
        tf.scale=Vec3::splat((life*sh).max(0.02)); *vis=Visibility::Visible;
    }
}

fn animate_fireball(time:Res<Time>,res:Res<SupernovaRes>,state:Res<SupernovaState>,mut q:Query<(&FireballVoxel,&mut Transform,&mut Visibility)>) {
    let t=time.elapsed_secs();
    for (fv,mut tf,mut vis) in &mut q {
        let Some(cfg)=res.supernovae.get(fv.idx) else { *vis=Visibility::Hidden; continue; };
        let active=matches!(state.phases.get(fv.idx),Some(SupernovaPhase::Fireball{..})|Some(SupernovaPhase::Plateau{..}));
        if !active { *vis=Visibility::Hidden; continue; }
        let e=phase_elapsed(&state,fv.idx);
        let boost=1.0+fv.polar_t*cfg.fireball_polar_boost;
        let base_r=(cfg.fireball_speed*e*boost).min(cfg.fireball_max_radius);
        let turb=(t*1.2+fv.seed*std::f32::consts::TAU).sin()*cfg.fireball_turbulence;
        tf.translation=snap_grid(fv.dir*(base_r+turb).max(0.0),cfg.fireball_voxel_size);
        let size=0.4+(1.0-fv.color_t)*0.9;
        let pf=if let Some(SupernovaPhase::Plateau{elapsed:pe})=state.phases.get(fv.idx){(1.0-pe/cfg.plateau_duration).clamp(0.0,1.0).powf(0.5)}else{1.0};
        tf.scale=Vec3::splat((size*pf).max(0.02)); *vis=Visibility::Visible;
    }
}

fn animate_jet(time:Res<Time>,res:Res<SupernovaRes>,state:Res<SupernovaState>,mut q:Query<(&SnovaJetVoxel,&mut Transform,&mut Visibility)>) {
    let t=time.elapsed_secs();
    for (jv,mut tf,mut vis) in &mut q {
        let Some(cfg)=res.supernovae.get(jv.idx) else { *vis=Visibility::Hidden; continue; };
        if !cfg.jet_enabled { *vis=Visibility::Hidden; continue; }
        let active=matches!(state.phases.get(jv.idx),Some(SupernovaPhase::Fireball{..})|Some(SupernovaPhase::Plateau{..}));
        if !active { *vis=Visibility::Hidden; continue; }
        let e=phase_elapsed(&state,jv.idx);
        let max_d=(cfg.jet_speed*e).min(cfg.jet_length);
        let dist=jv.t*max_d;
        let cw=jv.t*cfg.jet_width*0.5;
        let pa=jv.perp_seed*std::f32::consts::TAU+t*0.6;
        tf.translation=snap_grid(Vec3::new(pa.cos()*cw, jv.pole*dist, pa.sin()*cw),cfg.jet_voxel_size);
        let fade=(1.0-jv.t*0.75).max(0.05);
        let sh=0.7+(t*3.5+jv.seed*5.0).sin()*0.3;
        tf.scale=Vec3::splat((fade*sh).max(0.02)); *vis=Visibility::Visible;
    }
}

fn animate_blast_wave(res:Res<SupernovaRes>,state:Res<SupernovaState>,mut q:Query<(&BlastWaveVoxel,&mut Transform,&mut Visibility)>) {
    for (bv,mut tf,mut vis) in &mut q {
        let Some(cfg)=res.supernovae.get(bv.idx) else { *vis=Visibility::Hidden; continue; };
        let active=matches!(state.phases.get(bv.idx),
            Some(SupernovaPhase::Fireball{..})|Some(SupernovaPhase::Plateau{..})|Some(SupernovaPhase::Remnant{..}));
        if !active { *vis=Visibility::Hidden; continue; }
        let e=phase_elapsed(&state,bv.idx);
        let r=(cfg.blast_speed*e).min(cfg.blast_max_radius);
        if r<1.0 { *vis=Visibility::Hidden; continue; }
        let j=(bv.seed*std::f32::consts::TAU).sin()*cfg.blast_thickness*0.5;
        tf.translation=snap_grid(bv.dir*(r+j),cfg.blast_voxel_size);
        let life=1.0-(r/cfg.blast_max_radius).clamp(0.0,1.0);
        let sh=0.5+(e*2.0+bv.seed*8.0).sin().abs()*0.5;
        tf.scale=Vec3::splat((life*sh*1.2).max(0.02)); *vis=Visibility::Visible;
    }
}

fn animate_light_echo(res:Res<SupernovaRes>,state:Res<SupernovaState>,mut q:Query<(&LightEchoVoxel,&mut Transform,&mut Visibility)>) {
    for (ev,mut tf,mut vis) in &mut q {
        let Some(cfg)=res.supernovae.get(ev.idx) else { *vis=Visibility::Hidden; continue; };
        if !cfg.echo_enabled { *vis=Visibility::Hidden; continue; }
        if matches!(state.phases.get(ev.idx),Some(SupernovaPhase::Progenitor)|None) { *vis=Visibility::Hidden; continue; }
        let e=phase_elapsed(&state,ev.idx);
        let r=(cfg.echo_speed*e+cfg.echo_lead+ev.r_off).clamp(0.0,cfg.echo_max_radius);
        if r<1.0 { *vis=Visibility::Hidden; continue; }
        tf.translation=snap_grid(ev.dir*r,cfg.echo_voxel_size);
        let life=1.0-(r/cfg.echo_max_radius).clamp(0.0,1.0);
        let sh=0.4+(e*1.3+ev.seed*6.0).sin().abs()*0.4;
        tf.scale=Vec3::splat((life*sh).max(0.02)); *vis=Visibility::Visible;
    }
}

fn animate_ionization_front(res:Res<SupernovaRes>,state:Res<SupernovaState>,mut q:Query<(&IonizationVoxel,&mut Transform,&mut Visibility)>) {
    for (iv,mut tf,mut vis) in &mut q {
        let Some(cfg)=res.supernovae.get(iv.idx) else { *vis=Visibility::Hidden; continue; };
        if !cfg.ionization_enabled { *vis=Visibility::Hidden; continue; }
        if matches!(state.phases.get(iv.idx),Some(SupernovaPhase::Progenitor)|None) { *vis=Visibility::Hidden; continue; }
        let e=phase_elapsed(&state,iv.idx);
        let blast_r=(cfg.blast_speed*e).min(cfg.blast_max_radius);
        if blast_r<1.0 { *vis=Visibility::Hidden; continue; }
        let j=(iv.seed*std::f32::consts::TAU).sin()*cfg.ionization_width;
        let r=(blast_r+j+cfg.ionization_width*0.5).max(0.0);
        tf.translation=snap_grid(iv.dir*r,cfg.ionization_voxel_size);
        let sh=0.3+(e*4.0+iv.seed*std::f32::consts::TAU).sin().abs()*0.7;
        let life=1.0-(blast_r/cfg.blast_max_radius).clamp(0.0,1.0);
        tf.scale=Vec3::splat((sh*life*1.3).max(0.02)); *vis=Visibility::Visible;
    }
}

fn animate_snr_voxels(time:Res<Time>,res:Res<SupernovaRes>,state:Res<SupernovaState>,mut q:Query<(&SnrVoxel,&mut Transform,&mut Visibility)>) {
    let t=time.elapsed_secs();
    for (sv,mut tf,mut vis) in &mut q {
        let Some(cfg)=res.supernovae.get(sv.idx) else { *vis=Visibility::Hidden; continue; };
        if !cfg.snr_enabled||!matches!(state.phases.get(sv.idx),Some(SupernovaPhase::Remnant{..})) { *vis=Visibility::Hidden; continue; }
        let e=phase_elapsed(&state,sv.idx);
        let expansion=1.0+e*cfg.snr_expansion_speed*0.001;
        let drift=Vec3::new(
            (t*cfg.snr_drift_speed+sv.seed*11.0).sin()*cfg.snr_drift_amplitude,
            (t*cfg.snr_drift_speed*0.7+sv.seed*7.3).cos()*cfg.snr_drift_amplitude*0.5,
            (t*cfg.snr_drift_speed*0.9+sv.seed*5.9).sin()*cfg.snr_drift_amplitude*0.8,
        );
        tf.translation=snap_grid(sv.origin*expansion+drift,cfg.snr_voxel_size);
        let p=1.0+(t*0.6+sv.seed*std::f32::consts::TAU).sin()*0.12;
        let sz=(0.4+(1.0-sv.color_t)*0.8)*p;
        tf.scale=Vec3::splat(sz.max(0.02)); *vis=Visibility::Visible;
    }
}

fn animate_snr_filaments(time:Res<Time>,res:Res<SupernovaRes>,state:Res<SupernovaState>,mut q:Query<(&SnrFilament,&mut Transform,&mut Visibility)>) {
    let t=time.elapsed_secs();
    for (fv,mut tf,mut vis) in &mut q {
        let Some(cfg)=res.supernovae.get(fv.idx) else { *vis=Visibility::Hidden; continue; };
        if !cfg.snr_enabled||!matches!(state.phases.get(fv.idx),Some(SupernovaPhase::Remnant{..})) { *vis=Visibility::Hidden; continue; }
        let e=phase_elapsed(&state,fv.idx);
        let expansion=1.0+e*cfg.snr_expansion_speed*0.0008;
        let sample_t=fv.sample_idx as f32/cfg.snr_filament_samples.max(1) as f32;
        let lon_a=fv.lon+t*0.025;
        let cos_tilt=fv.tilt.cos().max(0.01);
        let lat_ang=(sample_t*std::f32::consts::PI-std::f32::consts::FRAC_PI_2)*fv.tilt/std::f32::consts::FRAC_PI_2;
        let r_arc=(cfg.snr_outer_radius*lat_ang.cos().powi(2)/(cos_tilt*cos_tilt)).clamp(cfg.snr_inner_radius,cfg.snr_outer_radius);
        let wave=(t*0.4+fv.seed*8.0+sample_t*5.0).sin()*r_arc*0.03;
        let r=(r_arc+wave)*expansion;
        let pos=Vec3::new(r*lat_ang.cos()*lon_a.cos(), r*lat_ang.sin(), r*lat_ang.cos()*lon_a.sin());
        tf.translation=snap_grid(pos,cfg.snr_voxel_size*0.5);
        let op=(sample_t*std::f32::consts::PI).sin();
        tf.scale=Vec3::splat((op*0.85).max(0.02));
        *vis=if op>0.04{Visibility::Visible}else{Visibility::Hidden};
    }
}

fn animate_remnant_core(time:Res<Time>,res:Res<SupernovaRes>,state:Res<SupernovaState>,mut q:Query<(&RemnantCore,&mut Transform,&mut Visibility)>) {
    let t=time.elapsed_secs();
    for (rc,mut tf,mut vis) in &mut q {
        let Some(cfg)=res.supernovae.get(rc.idx) else { *vis=Visibility::Hidden; continue; };
        if !cfg.remnant_core_enabled||!matches!(state.phases.get(rc.idx),Some(SupernovaPhase::Remnant{..})) { *vis=Visibility::Hidden; continue; }
        tf.rotation=Quat::from_rotation_y(t*4.5)*Quat::from_rotation_x(t*1.2);
        tf.scale=Vec3::splat(1.0+(t*6.0).sin()*0.15);
        *vis=Visibility::Visible;
    }
}

fn reload_supernovae(
    mut commands: Commands,
    mut events: EventReader<ReloadAstre>,
    res: Res<SupernovaRes>,
    roots: Query<(Entity, &SupernovaRoot)>,
    flash_q: Query<(Entity, &FlashVoxel)>,
    fireball_q: Query<(Entity, &FireballVoxel)>,
    jet_q: Query<(Entity, &SnovaJetVoxel)>,
    blast_q: Query<(Entity, &BlastWaveVoxel)>,
    echo_q: Query<(Entity, &LightEchoVoxel)>,
    ion_q: Query<(Entity, &IonizationVoxel)>,
    snr_q: Query<(Entity, &SnrVoxel)>,
    fil_q: Query<(Entity, &SnrFilament)>,
    core_q: Query<(Entity, &RemnantCore)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for ev in events.read() {
        let Ok((entity, root)) = roots.get(ev.0) else { continue };
        let idx = root.idx;
        let Some(cfg) = res.supernovae.get(idx) else { continue };
        for (e, v) in &flash_q { if v.idx == idx { if let Some(ec) = commands.get_entity(e) { ec.despawn_recursive(); } } }
        for (e, v) in &fireball_q { if v.idx == idx { if let Some(ec) = commands.get_entity(e) { ec.despawn_recursive(); } } }
        for (e, v) in &jet_q { if v.idx == idx { if let Some(ec) = commands.get_entity(e) { ec.despawn_recursive(); } } }
        for (e, v) in &blast_q { if v.idx == idx { if let Some(ec) = commands.get_entity(e) { ec.despawn_recursive(); } } }
        for (e, v) in &echo_q { if v.idx == idx { if let Some(ec) = commands.get_entity(e) { ec.despawn_recursive(); } } }
        for (e, v) in &ion_q { if v.idx == idx { if let Some(ec) = commands.get_entity(e) { ec.despawn_recursive(); } } }
        for (e, v) in &snr_q { if v.idx == idx { if let Some(ec) = commands.get_entity(e) { ec.despawn_recursive(); } } }
        for (e, v) in &fil_q { if v.idx == idx { if let Some(ec) = commands.get_entity(e) { ec.despawn_recursive(); } } }
        for (e, v) in &core_q { if v.idx == idx { if let Some(ec) = commands.get_entity(e) { ec.despawn_recursive(); } } }
        if let Some(ec) = commands.get_entity(entity) { ec.despawn_recursive(); }
        build_supernova(&mut commands, cfg, idx, &mut meshes, &mut materials);
    }
}

fn regenerate_supernovae(
    mut commands:Commands, mut events:EventReader<RegenerateSupernova>,
    res:Res<SupernovaRes>, mut state:ResMut<SupernovaState>,
    root_q:Query<Entity,With<SupernovaRoot>>,
    mut meshes:ResMut<Assets<Mesh>>, mut materials:ResMut<Assets<StandardMaterial>>,
) {
    let mut fired=false; for _ in events.read() { fired=true; } if !fired { return; }
    for e in &root_q { if let Some(ec) = commands.get_entity(e) { ec.despawn_recursive(); } }
    state.phases.clear();
    for (idx,cfg) in res.supernovae.iter().enumerate() {
        state.phases.push(if cfg.start_exploding{SupernovaPhase::FlashPeak{elapsed:0.0}}else{SupernovaPhase::Progenitor});
        build_supernova(&mut commands,cfg,idx,&mut meshes,&mut materials);
    }
}
