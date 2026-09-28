use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;
use crate::astre::{AstreLodRoot, ReloadAstre};

#[derive(Clone, Debug, PartialEq)]
pub enum DwarfType {
    /// Naine rouge  — M — froide, petite, très longue vie
    Red,
    /// Naine jaune  — G — type Soleil
    Yellow,
    /// Naine orange — K — intermédiaire
    Orange,
    /// Naine blanche — résidu compact, très dense, refroidissement lent
    White,
    /// Naine brune  — sous-stellaire, pas de fusion H, rayonnement IR
    Brown,
}

#[derive(Clone, Debug)]
pub struct DwarfStarConfig {
    pub position:           Vec3,
    pub orbit_distance:     f32,
    pub orbit_speed:        f32,
    pub spin_speed:         f32,
    pub dwarf_type:         DwarfType,
    pub radius:             f32,
    pub voxel_size:         f32,
    pub resolution:         u32,
    // Couleurs
    pub color_core:         [f32; 3],
    pub color_surface:      [f32; 3],
    pub emissive:           f32,
    // Refroidissement (naine blanche/brune uniquement)
    pub cooling_enabled:    bool,
    pub cooling_tau:        f32,
    pub cooling_floor:      f32,
    // Atmosphère IR (naine brune)
    pub atmosphere_enabled: bool,
    pub atmosphere_radius:  f32,
    pub atmosphere_count:   u32,
    pub atmosphere_voxel:   f32,
    pub atmosphere_color:   [f32; 3],
    pub atmosphere_emissive: f32,
    // Bandes naine brune (comme Jupiter mais stellaire)
    pub bands_enabled:      bool,
    pub band_count:         f32,
    pub band_colors:        Vec<[f32; 3]>,
    pub band_speed_eq:      f32,
    pub band_speed_pole:    f32,
    // Magnétosphère (naine rouge — très active)
    pub magneto_enabled:    bool,
    pub magneto_count:      u32,
    pub magneto_voxel:      f32,
    pub magneto_color:      [f32; 3],
    pub magneto_emissive:   f32,
    pub magneto_radius:     f32,
    // Éruptions stellaires (naine rouge — flares X extrêmes)
    pub xflare_enabled:     bool,
    pub xflare_count:       u32,
    pub xflare_samples:     u32,
    pub xflare_height:      f32,
    pub xflare_color:       [f32; 3],
    pub xflare_emissive:    f32,
    pub xflare_speed:       f32,
    pub xflare_interval:    f32,
    // Lumière
    pub light_intensity:    f32,
    pub light_range:        f32,
    pub seed:               u32,
}

impl DwarfStarConfig {
    pub fn red(seed: u32, pos: Vec3) -> Self {
        Self {
            position: pos, orbit_distance: 0.0, orbit_speed: 0.0, spin_speed: 0.3,
            dwarf_type: DwarfType::Red, radius: 45.0, voxel_size: 6.0, resolution: 16,
            color_core: [1.0, 0.45, 0.1], color_surface: [0.8, 0.2, 0.05], emissive: 5.0,
            cooling_enabled: false, cooling_tau: 1.0, cooling_floor: 0.0,
            atmosphere_enabled: false, atmosphere_radius: 0.0, atmosphere_count: 0,
            atmosphere_voxel: 6.0, atmosphere_color: [1.0,0.5,0.2], atmosphere_emissive: 1.0,
            bands_enabled: false, band_count: 0.0, band_colors: vec![], band_speed_eq: 0.0, band_speed_pole: 0.0,
            magneto_enabled: true, magneto_count: 180, magneto_voxel: 5.0,
            magneto_color: [0.8, 0.3, 1.0], magneto_emissive: 4.0, magneto_radius: 90.0,
            xflare_enabled: true, xflare_count: 4, xflare_samples: 40,
            xflare_height: 55.0, xflare_color: [1.0, 0.5, 0.2], xflare_emissive: 14.0,
            xflare_speed: 2.0, xflare_interval: 8.0,
            light_intensity: 800_000.0, light_range: 3000.0, seed,
        }
    }
    pub fn yellow(seed: u32, pos: Vec3) -> Self {
        Self {
            position: pos, orbit_distance: 0.0, orbit_speed: 0.0, spin_speed: 0.5,
            dwarf_type: DwarfType::Yellow, radius: 90.0, voxel_size: 8.0, resolution: 20,
            color_core: [1.0, 0.95, 0.7], color_surface: [1.0, 0.8, 0.35], emissive: 8.0,
            cooling_enabled: false, cooling_tau: 1.0, cooling_floor: 0.0,
            atmosphere_enabled: false, atmosphere_radius: 0.0, atmosphere_count: 0,
            atmosphere_voxel: 8.0, atmosphere_color: [1.0,0.9,0.5], atmosphere_emissive: 2.0,
            bands_enabled: false, band_count: 0.0, band_colors: vec![], band_speed_eq: 0.0, band_speed_pole: 0.0,
            magneto_enabled: false, magneto_count: 0, magneto_voxel: 6.0,
            magneto_color: [1.0,1.0,1.0], magneto_emissive: 0.0, magneto_radius: 0.0,
            xflare_enabled: false, xflare_count: 3, xflare_samples: 40,
            xflare_height: 70.0, xflare_color: [1.0,0.85,0.4], xflare_emissive: 10.0,
            xflare_speed: 1.2, xflare_interval: 15.0,
            light_intensity: 3_000_000.0, light_range: 6000.0, seed,
        }
    }
    pub fn white(seed: u32, pos: Vec3) -> Self {
        Self {
            position: pos, orbit_distance: 0.0, orbit_speed: 0.0, spin_speed: 2.5,
            dwarf_type: DwarfType::White, radius: 18.0, voxel_size: 3.0, resolution: 14,
            color_core: [0.9, 0.95, 1.0], color_surface: [0.7, 0.8, 1.0], emissive: 12.0,
            cooling_enabled: true, cooling_tau: 300.0, cooling_floor: 0.05,
            atmosphere_enabled: false, atmosphere_radius: 0.0, atmosphere_count: 0,
            atmosphere_voxel: 4.0, atmosphere_color: [0.7,0.8,1.0], atmosphere_emissive: 2.0,
            bands_enabled: false, band_count: 0.0, band_colors: vec![], band_speed_eq: 0.0, band_speed_pole: 0.0,
            magneto_enabled: false, magneto_count: 0, magneto_voxel: 4.0,
            magneto_color: [1.0,1.0,1.0], magneto_emissive: 0.0, magneto_radius: 0.0,
            xflare_enabled: false, xflare_count: 0, xflare_samples: 0,
            xflare_height: 0.0, xflare_color: [1.0,1.0,1.0], xflare_emissive: 0.0,
            xflare_speed: 0.0, xflare_interval: 0.0,
            light_intensity: 500_000.0, light_range: 2000.0, seed,
        }
    }
    pub fn brown(seed: u32, pos: Vec3) -> Self {
        Self {
            position: pos, orbit_distance: 0.0, orbit_speed: 0.0, spin_speed: 1.8,
            dwarf_type: DwarfType::Brown, radius: 65.0, voxel_size: 7.0, resolution: 18,
            color_core: [0.5, 0.3, 0.1], color_surface: [0.35, 0.2, 0.08], emissive: 0.8,
            cooling_enabled: true, cooling_tau: 600.0, cooling_floor: 0.02,
            atmosphere_enabled: true, atmosphere_radius: 40.0, atmosphere_count: 250,
            atmosphere_voxel: 7.0, atmosphere_color: [0.6,0.35,0.12], atmosphere_emissive: 0.5,
            bands_enabled: true, band_count: 5.0,
            band_colors: vec![[0.5,0.3,0.1],[0.4,0.22,0.07],[0.55,0.32,0.12],[0.38,0.2,0.06],[0.52,0.28,0.1]],
            band_speed_eq: 0.25, band_speed_pole: 0.06,
            magneto_enabled: false, magneto_count: 0, magneto_voxel: 6.0,
            magneto_color: [1.0,1.0,1.0], magneto_emissive: 0.0, magneto_radius: 0.0,
            xflare_enabled: false, xflare_count: 0, xflare_samples: 0,
            xflare_height: 0.0, xflare_color: [1.0,1.0,1.0], xflare_emissive: 0.0,
            xflare_speed: 0.0, xflare_interval: 0.0,
            light_intensity: 50_000.0, light_range: 800.0, seed,
        }
    }
}

pub const SPAWN_PROPS: crate::system_gen::SpawnProps = crate::system_gen::SpawnProps {
    category:  crate::system_gen::AstreCategory::Star,
    weight:    0.25,
    orbit_min: 0.0,
    orbit_max: 0.0,
};

pub fn generate_random(rng: &mut crate::system_gen::SeedRng, stars: &mut Vec<DwarfStarConfig>) {
    let seed = rng.u32();
    let roll = rng.f32();
    let config = if roll < 0.45 {
        DwarfStarConfig::red(seed, Vec3::ZERO)
    } else if roll < 0.70 {
        DwarfStarConfig {
            position: Vec3::ZERO,
            orbit_distance: 0.0,
            dwarf_type: DwarfType::Orange,
            radius: 65.0, voxel_size: 7.0, resolution: 18,
            color_core: [1.0, 0.7, 0.25], color_surface: [0.9, 0.5, 0.15],
            emissive: 6.5, light_intensity: 1_500_000.0, light_range: 4500.0,
            seed, ..DwarfStarConfig::red(seed, Vec3::ZERO)
        }
    } else {
        DwarfStarConfig::yellow(seed, Vec3::ZERO)
    };
    stars.push(config);
}

pub struct DwarfStarPlugin;
impl Plugin for DwarfStarPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DwarfStarRes>()
           .init_resource::<DwarfStarState>()
           .add_event::<RegenerateDwarfStar>()
           .add_systems(Startup, spawn_dwarfs)
           .add_systems(Update, (orbit_dwarfs, spin_dwarfs, tick_cooling_dwarf,
               animate_dwarf_surface, animate_magneto, animate_xflares,
               animate_dwarf_atmosphere, regenerate_dwarfs, reload_dwarfs).chain());
    }
}

#[derive(Resource)]
pub struct DwarfStarRes { pub stars: Vec<DwarfStarConfig> }
impl Default for DwarfStarRes { fn default() -> Self { Self { stars: vec![
    DwarfStarConfig::red(5, Vec3::new(2000.0, 0.0, 5000.0)),
    DwarfStarConfig::yellow(6, Vec3::new(2000.0, 0.0, 6500.0)),
    DwarfStarConfig::white(7, Vec3::new(2000.0, 0.0, 8000.0)),
    DwarfStarConfig::brown(8, Vec3::new(2000.0, 0.0, 9500.0)),
] } } }

#[derive(Resource, Default)]
pub struct DwarfStarState { pub ages: Vec<f32>, pub temps: Vec<f32>, pub spin_angles: Vec<f32> }

#[derive(Event)] pub struct RegenerateDwarfStar;
#[derive(Component)] pub struct DwarfRoot   { pub idx: usize }
#[derive(Component)] pub struct DwarfBody   { pub idx: usize }
#[derive(Component)] pub struct DwarfSurf   { pub idx: usize, pub lat: f32, pub lon: f32 }
#[derive(Component)] pub struct DwarfMagneto{ pub idx: usize, pub line: u32, pub sample: u32, pub lon: f32, pub tilt: f32, pub seed: f32 }
#[derive(Component)] pub struct DwarfXFlare { pub idx: usize, pub fi: u32, pub si: u32 }
#[derive(Component)] pub struct DwarfAtmo   { pub idx: usize, pub theta: f32, pub phi: f32, pub r: f32, pub seed: f32 }

fn ph(a:f32,b:f32)->f32{((a*12.9898+b*78.233).sin()*43758.5453).fract()}
fn snap(v:Vec3,g:f32)->Vec3{Vec3::new((v.x/g).round()*g,(v.y/g).round()*g,(v.z/g).round()*g)}
fn sph(lat:f32,lon:f32,r:f32)->Vec3{Vec3::new(r*lat.cos()*lon.cos(),r*lat.sin(),r*lat.cos()*lon.sin())}
fn lc(a:[f32;3],b:[f32;3],t:f32)->[f32;3]{let t=t.clamp(0.0,1.0);[a[0]+(b[0]-a[0])*t,a[1]+(b[1]-a[1])*t,a[2]+(b[2]-a[2])*t]}

fn spawn_dwarfs(mut cmd:Commands,res:Res<DwarfStarRes>,mut state:ResMut<DwarfStarState>,mut msh:ResMut<Assets<Mesh>>,mut mat:ResMut<Assets<StandardMaterial>>) {
    state.ages.clear(); state.temps.clear(); state.spin_angles.clear();
    for (i,cfg) in res.stars.iter().enumerate() {
        state.ages.push(0.0); state.temps.push(1.0); state.spin_angles.push(0.0);
        build_dwarf(&mut cmd,cfg,i,&mut msh,&mut mat);
    }
}

fn build_dwarf(cmd:&mut Commands,cfg:&DwarfStarConfig,idx:usize,msh:&mut ResMut<Assets<Mesh>>,mat:&mut ResMut<Assets<StandardMaterial>>) {
    let pos=if cfg.orbit_distance>1.0{Vec3::new(cfg.orbit_distance,0.0,0.0)}else{cfg.position};
    let root=cmd.spawn((Transform::from_translation(pos),Visibility::default(),DwarfRoot{idx},AstreLodRoot{cull_dist:3000.0, radius: cfg.radius, streamable: true, label: "DwarfStar"})).id();
    let body=cmd.spawn((Transform::IDENTITY,Visibility::default(),DwarfBody{idx})).id();
    cmd.entity(root).add_child(body);
    // Surface
    let vs=cfg.voxel_size; let rs=cfg.resolution;
    let smesh=msh.add(Mesh::from(Cuboid::new(vs,vs,vs)));
    let vstep=std::f32::consts::PI/rs as f32; let hstep=std::f32::consts::TAU/(rs*2) as f32;
    let [cr,cg,cb]=cfg.color_core; let ce=cfg.emissive;
    let smat=mat.add(StandardMaterial{base_color:Color::srgb(cr,cg,cb),emissive:LinearRgba::new(cr*ce,cg*ce,cb*ce,1.0),unlit:true,..default()});
    let mut lat=-std::f32::consts::FRAC_PI_2;
    while lat<=std::f32::consts::FRAC_PI_2 {
        let mut lon=0.0_f32;
        while lon<std::f32::consts::TAU {
            let n=ph(lat*7.0+cfg.seed as f32,lon*4.0)*0.1-0.05;
            let r=cfg.radius*(1.0+n);
            let v=cmd.spawn((Mesh3d(smesh.clone()),MeshMaterial3d(smat.clone()),Transform::from_translation(snap(sph(lat,lon,r),vs)),NotShadowCaster,DwarfSurf{idx,lat,lon})).id();
            cmd.entity(body).add_child(v);
            lon+=hstep;
        }
        lat+=vstep;
    }
    // Magnétosphère (naine rouge)
    if cfg.magneto_enabled {
        let [mr,mg,mb]=cfg.magneto_color; let me=cfg.magneto_emissive;
        let mm=mat.add(StandardMaterial{base_color:Color::srgb(mr,mg,mb),emissive:LinearRgba::new(mr*me,mg*me,mb*me,1.0),alpha_mode:AlphaMode::Add,unlit:true,..default()});
        let mmesh=msh.add(Mesh::from(Cuboid::new(cfg.magneto_voxel,cfg.magneto_voxel,cfg.magneto_voxel)));
        for li in 0..12u32 {
            let lon=li as f32/12.0*std::f32::consts::TAU;
            let tilt=0.2+ph(cfg.seed as f32+li as f32,5.0)*std::f32::consts::FRAC_PI_2*0.8;
            for si in 0..(cfg.magneto_count/12) {
                let seed=ph(li as f32*3.7,si as f32*2.3);
                let mv=cmd.spawn((Mesh3d(mmesh.clone()),MeshMaterial3d(mm.clone()),Transform::from_translation(Vec3::ZERO),Visibility::default(),NotShadowCaster,DwarfMagneto{idx,line:li,sample:si,lon,tilt,seed})).id();
                cmd.entity(root).add_child(mv);
            }
        }
    }
    // X-Flares
    if cfg.xflare_enabled {
        let [xr,xg,xb]=cfg.xflare_color; let xe=cfg.xflare_emissive;
        let xm=mat.add(StandardMaterial{base_color:Color::srgb(xr,xg,xb),emissive:LinearRgba::new(xr*xe,xg*xe,xb*xe,1.0),alpha_mode:AlphaMode::Add,unlit:true,..default()});
        let xmesh=msh.add(Mesh::from(Cuboid::new(5.0,5.0,5.0)));
        for fi in 0..cfg.xflare_count {
            for si in 0..cfg.xflare_samples {
                cmd.spawn((Mesh3d(xmesh.clone()),MeshMaterial3d(xm.clone()),Transform::from_translation(Vec3::ZERO).with_scale(Vec3::ZERO),Visibility::default(),NotShadowCaster,DwarfXFlare{idx,fi,si}));
            }
        }
    }
    // Atmosphère (naine brune)
    if cfg.atmosphere_enabled {
        let [ar,ag,ab]=cfg.atmosphere_color; let ae=cfg.atmosphere_emissive;
        let am=mat.add(StandardMaterial{base_color:Color::srgb(ar,ag,ab),emissive:LinearRgba::new(ar*ae,ag*ae,ab*ae,1.0),alpha_mode:AlphaMode::Add,unlit:true,..default()});
        let amesh=msh.add(Mesh::from(Cuboid::new(cfg.atmosphere_voxel,cfg.atmosphere_voxel,cfg.atmosphere_voxel)));
        for i in 0..cfg.atmosphere_count {
            let h1=ph(cfg.seed as f32+500.0,i as f32); let h2=ph(cfg.seed as f32+501.0,i as f32); let h3=ph(cfg.seed as f32+502.0,i as f32);
            let theta=h1*std::f32::consts::TAU; let phi=(h2*2.0-1.0).clamp(-1.0,1.0).acos();
            let r=cfg.radius+h3.sqrt()*cfg.atmosphere_radius;
            let av=cmd.spawn((Mesh3d(amesh.clone()),MeshMaterial3d(am.clone()),Transform::from_translation(Vec3::ZERO),Visibility::default(),NotShadowCaster,DwarfAtmo{idx,theta,phi,r,seed:ph(h1,h3)})).id();
            cmd.entity(root).add_child(av);
        }
    }
    let [lr,lg,lb]=cfg.color_core;
    cmd.spawn((PointLight{intensity:cfg.light_intensity * 100.0,range:cfg.light_range,color:Color::srgb(lr,lg,lb),shadows_enabled:false,..default()},Transform::IDENTITY));
}

fn orbit_dwarfs(time:Res<Time>,res:Res<DwarfStarRes>,mut q:Query<(&mut Transform,&DwarfRoot)>) {
    let t=time.elapsed_secs();
    for (mut tf,r) in &mut q{if let Some(cfg)=res.stars.get(r.idx){if cfg.orbit_distance>1.0{let a=t*cfg.orbit_speed;tf.translation.x=a.cos()*cfg.orbit_distance;tf.translation.z=a.sin()*cfg.orbit_distance;}}}
}
fn spin_dwarfs(time:Res<Time>,res:Res<DwarfStarRes>,mut state:ResMut<DwarfStarState>,mut q:Query<(&mut Transform,&DwarfBody)>) {
    let dt=time.delta_secs();
    for (mut tf,b) in &mut q{if let Some(cfg)=res.stars.get(b.idx){while state.spin_angles.len()<=b.idx{state.spin_angles.push(0.0);}state.spin_angles[b.idx]=(state.spin_angles[b.idx]+cfg.spin_speed*dt)%std::f32::consts::TAU;tf.rotation=Quat::from_rotation_y(state.spin_angles[b.idx]);}}
}
fn tick_cooling_dwarf(time:Res<Time>,res:Res<DwarfStarRes>,mut state:ResMut<DwarfStarState>) {
    let dt=time.delta_secs();
    for (i,cfg) in res.stars.iter().enumerate(){
        while state.ages.len()<=i{state.ages.push(0.0);} while state.temps.len()<=i{state.temps.push(1.0);}
        if cfg.cooling_enabled{state.ages[i]+=dt; state.temps[i]=((-state.ages[i]/cfg.cooling_tau.max(0.001)).exp()).max(cfg.cooling_floor);}
    }
}
fn animate_dwarf_surface(res:Res<DwarfStarRes>,state:Res<DwarfStarState>,q:Query<(&DwarfSurf,&MeshMaterial3d<StandardMaterial>)>,mut mats:ResMut<Assets<StandardMaterial>>) {
    for (sv,mh) in &q{
        let Some(cfg)=res.stars.get(sv.idx) else{continue;};
        let temp=state.temps.get(sv.idx).copied().unwrap_or(1.0);
        // Bandes naine brune
        let band_t=if cfg.bands_enabled&&!cfg.band_colors.is_empty(){
            let bt=((sv.lat*cfg.band_count).sin()*0.5+0.5)*(cfg.band_colors.len()-1) as f32;
            let lo=bt as usize; let hi=(lo+1).min(cfg.band_colors.len()-1);
            let frac=bt-lo as f32;
            let[r,g,b]=lc(cfg.band_colors[lo],cfg.band_colors[hi],frac);
            [r*temp,g*temp,b*temp]
        } else {
            let[r,g,b]=lc(cfg.color_surface,cfg.color_core,(sv.lat.abs()/std::f32::consts::FRAC_PI_2).powf(1.5));
            [r*temp,g*temp,b*temp]
        };
        let[r,g,b]=band_t; let ce=cfg.emissive*temp;
        if let Some(m)=mats.get_mut(&mh.0){m.base_color=Color::srgb(r,g,b);m.emissive=LinearRgba::new(r*ce,g*ce,b*ce,1.0);}
    }
}
fn animate_magneto(time:Res<Time>,res:Res<DwarfStarRes>,state:Res<DwarfStarState>,mut q:Query<(&DwarfMagneto,&mut Transform,&mut Visibility)>) {
    let t=time.elapsed_secs();
    for (mv,mut tf,mut vis) in &mut q {
        let Some(cfg)=res.stars.get(mv.idx) else{*vis=Visibility::Hidden;continue;};
        if !cfg.magneto_enabled{*vis=Visibility::Hidden;continue;}
        let spin=state.spin_angles.get(mv.idx).copied().unwrap_or(0.0);
        let n=cfg.magneto_count/12;
        let sample_t=mv.sample as f32/n.max(1) as f32;
        let lon_a=mv.lon+spin+t*0.15;
        let cos_t=mv.tilt.cos().max(0.01);
        let lat_a=(sample_t*std::f32::consts::PI-std::f32::consts::FRAC_PI_2)*mv.tilt/std::f32::consts::FRAC_PI_2;
        let r=(cfg.magneto_radius*lat_a.cos().powi(2)/(cos_t*cos_t)).clamp(cfg.radius,cfg.magneto_radius);
        let wave=(t*1.5+mv.seed*8.0+sample_t*5.0).sin()*r*0.04;
        let pos=Vec3::new((r+wave)*lat_a.cos()*lon_a.cos(),(r+wave)*lat_a.sin(),(r+wave)*lat_a.cos()*lon_a.sin());
        tf.translation=snap(pos,cfg.magneto_voxel);
        let op=(sample_t*std::f32::consts::PI).sin();
        tf.scale=Vec3::splat((op*0.8).max(0.02));
        *vis=if op>0.05{Visibility::Visible}else{Visibility::Hidden};
    }
}
fn animate_xflares(time:Res<Time>,res:Res<DwarfStarRes>,mut q:Query<(&DwarfXFlare,&mut Transform,&mut Visibility)>) {
    let t=time.elapsed_secs();
    for (fv,mut tf,mut vis) in &mut q {
        let Some(cfg)=res.stars.get(fv.idx) else{*vis=Visibility::Hidden;continue;};
        if !cfg.xflare_enabled{*vis=Visibility::Hidden;continue;}
        let base=cfg.seed as f32*5.1+fv.fi as f32*29.3;
        let cycle=cfg.xflare_interval; let off=ph(base,9.0)*cycle;
        let lt=((t*cfg.xflare_speed*0.5+off)%cycle).max(0.0);
        let active_frac=0.4; let life=if lt<cycle*active_frac{(lt/(cycle*active_frac)).powi(2)}else{0.0};
        if life<0.02{*vis=Visibility::Hidden;continue;}
        let frac=fv.si as f32/cfg.xflare_samples.max(1) as f32;
        if frac>life{*vis=Visibility::Hidden;continue;}
        let h1=ph(base,fv.si as f32*0.7); let h2=ph(base,fv.si as f32*1.3);
        let theta=h1*std::f32::consts::TAU; let phi=(h2*2.0-1.0).clamp(-1.0,1.0).acos();
        let dir=Vec3::new(phi.sin()*theta.cos(),phi.cos(),phi.sin()*theta.sin()).normalize();
        let height=cfg.radius+frac*cfg.xflare_height*life;
        tf.translation=snap(dir*height,5.0); tf.scale=Vec3::splat((frac*life).max(0.02));
        *vis=Visibility::Visible;
    }
}
fn animate_dwarf_atmosphere(time:Res<Time>,res:Res<DwarfStarRes>,state:Res<DwarfStarState>,mut q:Query<(&DwarfAtmo,&mut Transform)>) {
    let t=time.elapsed_secs();
    for (av,mut tf) in &mut q {
        let Some(cfg)=res.stars.get(av.idx) else{continue;};
        let temp=state.temps.get(av.idx).copied().unwrap_or(1.0);
        let th=av.theta+t*0.05*(1.0+av.seed*0.3);
        let pulse=1.0+(t*0.7+av.seed*std::f32::consts::TAU).sin()*0.15;
        let r=av.r*pulse;
        tf.translation=snap(Vec3::new(r*av.phi.sin()*th.cos(),r*av.phi.cos(),r*av.phi.sin()*th.sin()),cfg.atmosphere_voxel);
        let s=(0.3+temp*0.8)*(0.5+(t*1.1+av.seed*7.0).sin().abs()*0.5); tf.scale=Vec3::splat(s.max(0.02));
    }
}
fn reload_dwarfs(
    mut cmd: Commands,
    mut events: EventReader<ReloadAstre>,
    res: Res<DwarfStarRes>,
    roots: Query<(Entity, &DwarfRoot)>,
    xflares: Query<(Entity, &DwarfXFlare)>,
    mut msh: ResMut<Assets<Mesh>>,
    mut mat: ResMut<Assets<StandardMaterial>>,
) {
    for ev in events.read() {
        let Ok((entity, root)) = roots.get(ev.0) else { continue };
        let idx = root.idx;
        let Some(cfg) = res.stars.get(idx) else { continue };
        for (fe, fv) in &xflares { if fv.idx == idx { cmd.entity(fe).despawn_recursive(); } }
        cmd.entity(entity).despawn_recursive();
        build_dwarf(&mut cmd, cfg, idx, &mut msh, &mut mat);
    }
}

fn regenerate_dwarfs(mut cmd:Commands,mut ev:EventReader<RegenerateDwarfStar>,res:Res<DwarfStarRes>,mut state:ResMut<DwarfStarState>,rq:Query<Entity,With<DwarfRoot>>,mut msh:ResMut<Assets<Mesh>>,mut mat:ResMut<Assets<StandardMaterial>>) {
    let mut f=false;for _ in ev.read(){f=true;}if !f{return;}
    for e in &rq{cmd.entity(e).despawn_recursive();}
    state.ages.clear();state.temps.clear();state.spin_angles.clear();
    for (i,cfg) in res.stars.iter().enumerate(){state.ages.push(0.0);state.temps.push(1.0);state.spin_angles.push(0.0);build_dwarf(&mut cmd,cfg,i,&mut msh,&mut mat);}
}
