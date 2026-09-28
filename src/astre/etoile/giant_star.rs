use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;
use crate::astre::{AstreLodRoot, ReloadAstre};

#[derive(Clone,Debug,PartialEq)]
pub enum GiantType { Red, Orange, Blue, AGB }  // AGB = géante asymptotique (pré-nébuleuse)

#[derive(Clone,Debug)]
pub struct GiantStarConfig {
    pub position:           Vec3,
    pub orbit_distance:     f32,
    pub orbit_speed:        f32,
    pub spin_speed:         f32,
    pub giant_type:         GiantType,
    pub radius:             f32,
    pub voxel_size:         f32,
    pub resolution:         u32,
    pub color_core:         [f32;3],
    pub color_surface:      [f32;3],
    pub emissive:           f32,
    // Pulsation (géantes pulsantes — Mira, Cépheide)
    pub pulsation_enabled:  bool,
    pub pulsation_period:   f32,
    pub pulsation_amplitude: f32,
    // Enveloppe convective (énorme — couvre 80% de la géante rouge)
    pub convection_enabled: bool,
    pub convection_cells:   u32,
    pub convection_voxel:   f32,
    pub convection_color_hot: [f32;3],
    pub convection_color_cold: [f32;3],
    pub convection_emissive: f32,
    pub convection_speed:   f32,
    pub convection_depth:   f32,
    // Éjection de masse lente (vent géante)
    pub wind_enabled:       bool,
    pub wind_count:         u32,
    pub wind_voxel:         f32,
    pub wind_color:         [f32;3],
    pub wind_emissive:      f32,
    pub wind_speed:         f32,
    pub wind_radius:        f32,
    // Coquille AGB (précurseur nébuleuse planétaire)
    pub shell_enabled:      bool,
    pub shell_radius:       f32,
    pub shell_count:        u32,
    pub shell_voxel:        f32,
    pub shell_color:        [f32;3],
    pub shell_emissive:     f32,
    pub shell_thickness:    f32,
    // Couronne étendue
    pub corona_radius:      f32,
    pub corona_count:       u32,
    pub corona_voxel:       f32,
    pub corona_color:       [f32;3],
    pub corona_emissive:    f32,
    pub light_intensity:    f32,
    pub light_range:        f32,
    pub seed:               u32,
}

impl Default for GiantStarConfig {
    fn default() -> Self {
        Self {
            position:Vec3::ZERO,orbit_distance:0.0,orbit_speed:0.0,spin_speed:0.12,
            giant_type:GiantType::Red,radius:350.0,voxel_size:20.0,resolution:22,
            color_core:[1.0,0.55,0.15],color_surface:[0.85,0.3,0.05],emissive:6.0,
            pulsation_enabled:true,pulsation_period:8.0,pulsation_amplitude:0.07,
            convection_enabled:true,convection_cells:80,convection_voxel:22.0,
            convection_color_hot:[1.0,0.7,0.25],convection_color_cold:[0.7,0.25,0.05],
            convection_emissive:5.0,convection_speed:0.18,convection_depth:40.0,
            wind_enabled:true,wind_count:300,wind_voxel:14.0,
            wind_color:[0.8,0.45,0.12],wind_emissive:1.5,wind_speed:12.0,wind_radius:800.0,
            shell_enabled:false,shell_radius:600.0,shell_count:400,shell_voxel:16.0,
            shell_color:[0.7,0.5,0.9],shell_emissive:3.0,shell_thickness:50.0,
            corona_radius:250.0,corona_count:400,corona_voxel:18.0,
            corona_color:[1.0,0.7,0.35],corona_emissive:3.5,
            light_intensity:15_000_000.0,light_range:12000.0,seed:19,
        }
    }
}
impl GiantStarConfig {
    pub fn agb(seed:u32,pos:Vec3)->Self{Self{position:pos,giant_type:GiantType::AGB,radius:500.0,voxel_size:24.0,color_core:[0.9,0.6,0.8],color_surface:[0.7,0.3,0.6],emissive:7.0,pulsation_amplitude:0.14,shell_enabled:true,shell_radius:700.0,shell_count:600,shell_color:[0.6,0.4,0.9],shell_emissive:4.0,seed,..Default::default()}}
    pub fn blue(seed:u32,pos:Vec3)->Self{Self{position:pos,giant_type:GiantType::Blue,radius:250.0,voxel_size:16.0,color_core:[0.6,0.75,1.0],color_surface:[0.4,0.6,1.0],emissive:16.0,pulsation_enabled:false,convection_cells:40,wind_speed:40.0,wind_radius:1200.0,light_intensity:40_000_000.0,light_range:18000.0,seed,..Default::default()}}
}

pub const SPAWN_PROPS: crate::system_gen::SpawnProps = crate::system_gen::SpawnProps {
    category:  crate::system_gen::AstreCategory::Star,
    weight:    0.10,
    orbit_min: 0.0,
    orbit_max: 0.0,
};

pub fn generate_random(rng: &mut crate::system_gen::SeedRng, stars: &mut Vec<GiantStarConfig>) {
    let seed = rng.u32();
    let roll = rng.f32();
    let config = if roll < 0.60 {
        GiantStarConfig { position: Vec3::ZERO, seed, ..Default::default() }
    } else if roll < 0.85 {
        GiantStarConfig::blue(seed, Vec3::ZERO)
    } else {
        GiantStarConfig::agb(seed, Vec3::ZERO)
    };
    stars.push(config);
}

pub struct GiantStarPlugin;
impl Plugin for GiantStarPlugin {
    fn build(&self,app:&mut App){
        app.init_resource::<GiantStarRes>()
           .add_event::<RegenerateGiantStar>()
           .add_systems(Startup,spawn_giants)
           .add_systems(Update,(orbit_giants,animate_giant_pulse,animate_convection_giant,
               animate_giant_wind,animate_giant_shell,animate_giant_corona,regenerate_giants,reload_giants).chain());
    }
}

#[derive(Resource)]
pub struct GiantStarRes{pub stars:Vec<GiantStarConfig>}
impl Default for GiantStarRes{fn default()->Self{Self{stars:vec![
    GiantStarConfig{position:Vec3::new(0.0,0.0,9000.0),..Default::default()},
]}}}
#[derive(Event)] pub struct RegenerateGiantStar;

#[derive(Component)] pub struct GiantRoot    {pub idx:usize}
#[derive(Component)] pub struct GiantBody    {pub idx:usize}
#[derive(Component)] pub struct GiantConvCell{pub idx:usize,pub origin:Vec3,pub seed:f32,pub phase:f32}
#[derive(Component)] pub struct GiantWind    {pub idx:usize,pub dir:Vec3,pub t:f32,pub seed:f32}
#[derive(Component)] pub struct GiantShell   {pub idx:usize,pub dir:Vec3,pub seed:f32}
#[derive(Component)] pub struct GiantCorona  {pub idx:usize,pub theta:f32,pub phi:f32,pub r:f32,pub seed:f32}

fn ph(a:f32,b:f32)->f32{((a*12.9898+b*78.233).sin()*43758.5453).fract()}
fn snap(v:Vec3,g:f32)->Vec3{Vec3::new((v.x/g).round()*g,(v.y/g).round()*g,(v.z/g).round()*g)}
fn sph(lat:f32,lon:f32,r:f32)->Vec3{Vec3::new(r*lat.cos()*lon.cos(),r*lat.sin(),r*lat.cos()*lon.sin())}
fn lc(a:[f32;3],b:[f32;3],t:f32)->[f32;3]{let t=t.clamp(0.0,1.0);[a[0]+(b[0]-a[0])*t,a[1]+(b[1]-a[1])*t,a[2]+(b[2]-a[2])*t]}

fn spawn_giants(mut cmd:Commands,res:Res<GiantStarRes>,mut msh:ResMut<Assets<Mesh>>,mut mat:ResMut<Assets<StandardMaterial>>){
    for (i,cfg) in res.stars.iter().enumerate(){build_giant(&mut cmd,cfg,i,&mut msh,&mut mat);}
}
fn build_giant(cmd:&mut Commands,cfg:&GiantStarConfig,idx:usize,msh:&mut ResMut<Assets<Mesh>>,mat:&mut ResMut<Assets<StandardMaterial>>){
    let pos=if cfg.orbit_distance>1.0{Vec3::new(cfg.orbit_distance,0.0,0.0)}else{cfg.position};
    let root=cmd.spawn((Transform::from_translation(pos),Visibility::default(),GiantRoot{idx},AstreLodRoot{cull_dist:10000.0, radius: cfg.radius, streamable: true, label: "GiantStar"})).id();
    let body=cmd.spawn((Transform::IDENTITY,Visibility::default(),GiantBody{idx})).id();
    cmd.entity(root).add_child(body);
    let vs=cfg.voxel_size; let rs=cfg.resolution;
    let smesh=msh.add(Mesh::from(Cuboid::new(vs,vs,vs)));
    let vstep=std::f32::consts::PI/rs as f32; let hstep=std::f32::consts::TAU/(rs*2) as f32;
    let mut lat=-std::f32::consts::FRAC_PI_2;
    while lat<=std::f32::consts::FRAC_PI_2 {
        let mut lon=0.0_f32;
        while lon<std::f32::consts::TAU {
            let noise=ph(lat*5.0+cfg.seed as f32,lon*3.0)*0.12-0.06;
            let r=cfg.radius*(1.0+noise);
            let t2=(lat+std::f32::consts::FRAC_PI_2)/std::f32::consts::PI;
            let[sr,sg,sb]=lc(cfg.color_surface,cfg.color_core,t2);
            let se=cfg.emissive*(0.7+t2*0.5);
            let sm=mat.add(StandardMaterial{base_color:Color::srgb(sr,sg,sb),emissive:LinearRgba::new(sr*se,sg*se,sb*se,1.0),unlit:true,..default()});
            let v=cmd.spawn((Mesh3d(smesh.clone()),MeshMaterial3d(sm),Transform::from_translation(snap(sph(lat,lon,r),vs)),NotShadowCaster)).id();
            cmd.entity(body).add_child(v);
            lon+=hstep;
        }
        lat+=vstep;
    }
    // Cellules de convection géantes
    if cfg.convection_enabled {
        let gm=msh.add(Mesh::from(Cuboid::new(cfg.convection_voxel,cfg.convection_voxel,cfg.convection_voxel)));
        for i in 0..cfg.convection_cells {
            let h1=ph(cfg.seed as f32+100.0,i as f32);let h2=ph(cfg.seed as f32+101.0,i as f32);let h3=ph(cfg.seed as f32+102.0,i as f32);
            let theta=h1*std::f32::consts::TAU;let phi=(h2*2.0-1.0).clamp(-1.0,1.0).acos();
            let r=cfg.radius+h3*cfg.convection_depth;
            let o=Vec3::new(r*phi.sin()*theta.cos(),r*phi.cos(),r*phi.sin()*theta.sin());
            let[gr,gg,gb]=cfg.convection_color_hot;let ge=cfg.convection_emissive;
            let gmat=mat.add(StandardMaterial{base_color:Color::srgb(gr,gg,gb),emissive:LinearRgba::new(gr*ge,gg*ge,gb*ge,1.0),alpha_mode:AlphaMode::Add,unlit:true,..default()});
            let gv=cmd.spawn((Mesh3d(gm.clone()),MeshMaterial3d(gmat),Transform::from_translation(snap(o,cfg.convection_voxel)),Visibility::default(),NotShadowCaster,GiantConvCell{idx,origin:snap(o,cfg.convection_voxel),seed:ph(h1,h2),phase:h3*std::f32::consts::TAU})).id();
            cmd.entity(root).add_child(gv);
        }
    }
    // Vent
    if cfg.wind_enabled {
        let[wr,wg,wb]=cfg.wind_color;let we=cfg.wind_emissive;
        let wm=mat.add(StandardMaterial{base_color:Color::srgb(wr,wg,wb),emissive:LinearRgba::new(wr*we,wg*we,wb*we,1.0),alpha_mode:AlphaMode::Add,unlit:true,..default()});
        let wmesh=msh.add(Mesh::from(Cuboid::new(cfg.wind_voxel,cfg.wind_voxel,cfg.wind_voxel)));
        for i in 0..cfg.wind_count {
            let h1=ph(cfg.seed as f32+200.0,i as f32);let h2=ph(cfg.seed as f32+201.0,i as f32);
            let theta=h1*std::f32::consts::TAU;let phi=(h2*2.0-1.0).clamp(-1.0,1.0).acos();
            let dir=Vec3::new(phi.sin()*theta.cos(),phi.cos(),phi.sin()*theta.sin()).normalize();
            let wv=cmd.spawn((Mesh3d(wmesh.clone()),MeshMaterial3d(wm.clone()),Transform::from_translation(Vec3::ZERO),Visibility::default(),NotShadowCaster,GiantWind{idx,dir,t:ph(h1,h2),seed:ph(h2,h1)})).id();
            cmd.entity(root).add_child(wv);
        }
    }
    // Coquille AGB
    if cfg.shell_enabled {
        let[shr,shg,shb]=cfg.shell_color;let she=cfg.shell_emissive;
        let shm=mat.add(StandardMaterial{base_color:Color::srgb(shr,shg,shb),emissive:LinearRgba::new(shr*she,shg*she,shb*she,1.0),alpha_mode:AlphaMode::Add,unlit:true,..default()});
        let shmesh=msh.add(Mesh::from(Cuboid::new(cfg.shell_voxel,cfg.shell_voxel,cfg.shell_voxel)));
        for i in 0..cfg.shell_count {
            let h1=ph(cfg.seed as f32+300.0,i as f32);let h2=ph(cfg.seed as f32+301.0,i as f32);let h3=ph(cfg.seed as f32+302.0,i as f32);
            let theta=h1*std::f32::consts::TAU;let phi=(h2*2.0-1.0).clamp(-1.0,1.0).acos();
            let dir=Vec3::new(phi.sin()*theta.cos(),phi.cos(),phi.sin()*theta.sin()).normalize();
            let r=cfg.shell_radius+(h3*2.0-1.0)*cfg.shell_thickness*0.5;
            let shv=cmd.spawn((Mesh3d(shmesh.clone()),MeshMaterial3d(shm.clone()),Transform::from_translation(dir*r),Visibility::default(),NotShadowCaster,GiantShell{idx,dir,seed:ph(h1,h3)})).id();
            cmd.entity(root).add_child(shv);
        }
    }
    // Couronne
    let[cor,cog,cob]=cfg.corona_color;let coe=cfg.corona_emissive;
    let com=mat.add(StandardMaterial{base_color:Color::srgb(cor,cog,cob),emissive:LinearRgba::new(cor*coe,cog*coe,cob*coe,1.0),alpha_mode:AlphaMode::Add,unlit:true,..default()});
    let comesh=msh.add(Mesh::from(Cuboid::new(cfg.corona_voxel,cfg.corona_voxel,cfg.corona_voxel)));
    for i in 0..cfg.corona_count {
        let h1=ph(cfg.seed as f32+400.0,i as f32);let h2=ph(cfg.seed as f32+401.0,i as f32);let h3=ph(cfg.seed as f32+402.0,i as f32);
        let theta=h1*std::f32::consts::TAU;let phi=(h2*2.0-1.0).clamp(-1.0,1.0).acos();
        let r=cfg.radius+h3.sqrt()*cfg.corona_radius;
        let cov=cmd.spawn((Mesh3d(comesh.clone()),MeshMaterial3d(com.clone()),Transform::from_translation(Vec3::ZERO),Visibility::default(),NotShadowCaster,GiantCorona{idx,theta,phi,r,seed:ph(h1,h3)})).id();
        cmd.entity(root).add_child(cov);
    }
    let[lr,lg,lb]=cfg.color_core;
    cmd.spawn((PointLight{intensity:cfg.light_intensity * 100.0,range:cfg.light_range,color:Color::srgb(lr,lg,lb),shadows_enabled:true,..default()},Transform::IDENTITY));
}

fn orbit_giants(time:Res<Time>,res:Res<GiantStarRes>,mut q:Query<(&mut Transform,&GiantRoot)>){
    let t=time.elapsed_secs();
    for (mut tf,r) in &mut q{if let Some(cfg)=res.stars.get(r.idx){if cfg.orbit_distance>1.0{let a=t*cfg.orbit_speed;tf.translation.x=a.cos()*cfg.orbit_distance;tf.translation.z=a.sin()*cfg.orbit_distance;}}}
}
fn animate_giant_pulse(time:Res<Time>,res:Res<GiantStarRes>,mut q:Query<(&mut Transform,&GiantBody)>){
    let t=time.elapsed_secs();
    for (mut tf,b) in &mut q{if let Some(cfg)=res.stars.get(b.idx){
        tf.rotation=Quat::from_rotation_y(t*cfg.spin_speed);
        if cfg.pulsation_enabled{let p=1.0+(t*std::f32::consts::TAU/cfg.pulsation_period).sin()*cfg.pulsation_amplitude;tf.scale=Vec3::splat(p);}
    }}
}
fn animate_convection_giant(time:Res<Time>,res:Res<GiantStarRes>,mut q:Query<(&GiantConvCell,&mut Transform,&MeshMaterial3d<StandardMaterial>)>,mut mats:ResMut<Assets<StandardMaterial>>){
    let t=time.elapsed_secs();
    for (gv,mut tf,mh) in &mut q {
        let Some(cfg)=res.stars.get(gv.idx) else{continue;};
        let cycle=(t*cfg.convection_speed+gv.phase).sin()*0.5+0.5;
        let drift=Vec3::new((t*cfg.convection_speed*0.7+gv.seed*11.0).sin()*12.0,(t*cfg.convection_speed*0.5+gv.seed*7.3).cos()*6.0,(t*cfg.convection_speed*0.9+gv.seed*5.1).sin()*12.0);
        tf.translation=snap(gv.origin+drift,cfg.convection_voxel);
        let[hr,hg,hb]=cfg.convection_color_hot;let[cr,cg,cb]=cfg.convection_color_cold;
        let[rr,rg,rb]=lc([cr,cg,cb],[hr,hg,hb],cycle);
        let ge=cfg.convection_emissive*(0.4+cycle*0.8);
        if let Some(m)=mats.get_mut(&mh.0){m.base_color=Color::srgb(rr,rg,rb);m.emissive=LinearRgba::new(rr*ge,rg*ge,rb*ge,1.0);}
        let s=0.6+cycle*0.7; tf.scale=Vec3::splat(s);
    }
}
fn animate_giant_wind(time:Res<Time>,res:Res<GiantStarRes>,mut q:Query<(&GiantWind,&mut Transform,&mut Visibility)>){
    let t=time.elapsed_secs();
    for (wv,mut tf,mut vis) in &mut q {
        let Some(cfg)=res.stars.get(wv.idx) else{*vis=Visibility::Hidden;continue;};
        if !cfg.wind_enabled{*vis=Visibility::Hidden;continue;}
        let local_t=((t*cfg.wind_speed/cfg.wind_radius+wv.t)%1.0).max(0.0);
        let dist=cfg.radius+local_t*cfg.wind_radius;
        let jit=Vec3::new((t*0.2+wv.seed*7.0).sin(),(t*0.3+wv.seed*5.0).cos(),(t*0.25+wv.seed*9.0).sin())*dist*0.025;
        tf.translation=snap(wv.dir*dist+jit,cfg.wind_voxel);
        let fade=(1.0-local_t*0.8).max(0.02); tf.scale=Vec3::splat(fade);
        *vis=Visibility::Visible;
    }
}
fn animate_giant_shell(time:Res<Time>,res:Res<GiantStarRes>,mut q:Query<(&GiantShell,&mut Transform)>){
    let t=time.elapsed_secs();
    for (sv,mut tf) in &mut q {
        let Some(cfg)=res.stars.get(sv.idx) else{continue;};
        if !cfg.shell_enabled{continue;}
        let pulse=1.0+(t*0.3+sv.seed*std::f32::consts::TAU).sin()*0.05;
        let r=cfg.shell_radius*pulse;
        tf.translation=snap(sv.dir*r,cfg.shell_voxel);
        let s=0.5+(t*0.8+sv.seed*5.0).sin().abs()*0.5; tf.scale=Vec3::splat(s.max(0.02));
    }
}
fn animate_giant_corona(time:Res<Time>,res:Res<GiantStarRes>,mut q:Query<(&GiantCorona,&mut Transform)>){
    let t=time.elapsed_secs();
    for (cv,mut tf) in &mut q {
        let Some(cfg)=res.stars.get(cv.idx) else{continue;};
        let pulse=1.0+(t*0.7+cv.seed*std::f32::consts::TAU).sin()*0.2;
        let r=cv.r*pulse; let th=cv.theta+t*0.04;
        tf.translation=snap(Vec3::new(r*cv.phi.sin()*th.cos(),r*cv.phi.cos(),r*cv.phi.sin()*th.sin()),cfg.corona_voxel);
        let life=1.0-((r-cfg.radius)/cfg.corona_radius.max(1.0)).clamp(0.0,1.0);
        tf.scale=Vec3::splat((0.4+life*0.8).max(0.02));
    }
}
fn reload_giants(
    mut cmd: Commands,
    mut events: EventReader<ReloadAstre>,
    res: Res<GiantStarRes>,
    roots: Query<(Entity, &GiantRoot)>,
    mut msh: ResMut<Assets<Mesh>>,
    mut mat: ResMut<Assets<StandardMaterial>>,
) {
    for ev in events.read() {
        let Ok((entity, root)) = roots.get(ev.0) else { continue };
        let idx = root.idx;
        let Some(cfg) = res.stars.get(idx) else { continue };
        cmd.entity(entity).despawn_recursive();
        build_giant(&mut cmd, cfg, idx, &mut msh, &mut mat);
    }
}

fn regenerate_giants(mut cmd:Commands,mut ev:EventReader<RegenerateGiantStar>,res:Res<GiantStarRes>,rq:Query<Entity,With<GiantRoot>>,mut msh:ResMut<Assets<Mesh>>,mut mat:ResMut<Assets<StandardMaterial>>){
    let mut f=false;for _ in ev.read(){f=true;}if !f{return;}
    for e in &rq{cmd.entity(e).despawn_recursive();}
    for (i,cfg) in res.stars.iter().enumerate(){build_giant(&mut cmd,cfg,i,&mut msh,&mut mat);}
}
