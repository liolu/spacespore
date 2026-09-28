use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;
use crate::astre::{AstreLodRoot, ReloadAstre};

/// Hypergéantes — les plus grandes étoiles connues (R136a1, UY Scuti, VY CMa…)
#[derive(Clone,Debug,PartialEq)]
pub enum HypergiantType {
    /// Hypergéante rouge — cool, énorme (UY Scuti, VY CMa)
    Red,
    /// Hypergéante bleue lumineuse — LBV (Eta Carinae, P Cygni)
    BlueLuminous,
    /// Wolf-Rayet — cœur nu, vent extrême, ionisation totale
    WolfRayet,
}

#[derive(Clone,Debug)]
pub struct HypergiantConfig {
    pub position:Vec3, pub orbit_distance:f32, pub orbit_speed:f32, pub spin_speed:f32,
    pub hg_type:HypergiantType,
    pub radius:f32, pub voxel_size:f32, pub resolution:u32,
    pub color_core:[f32;3], pub color_surface:[f32;3], pub emissive:f32,
    pub pulsation_period:f32, pub pulsation_amplitude:f32,
    // Cellules de convection géantes (taille = fraction du rayon solaire)
    pub convection_cells:u32, pub convection_voxel:f32,
    pub convection_hot:[f32;3], pub convection_cold:[f32;3], pub convection_emissive:f32, pub convection_speed:f32,
    // Vent extrême (taux de perte de masse ~1e-4 M_sol/an)
    pub wind_count:u32, pub wind_voxel:f32, pub wind_color:[f32;3], pub wind_emissive:f32, pub wind_speed:f32, pub wind_radius:f32,
    // Coquilles multiples d'éjecta (éruptions historiques)
    pub shells:Vec<(f32,f32,[f32;3],f32)>,  // (radius, thickness, color, emissive)
    pub shell_count_per:u32, pub shell_voxel:f32,
    // Nébuleuse homuncule (Eta Carinae) ou lobes bipolaires
    pub homunculus_enabled:bool, pub homunculus_length:f32, pub homunculus_width:f32,
    pub homunculus_count:u32, pub homunculus_voxel:f32,
    pub homunculus_color:[f32;3], pub homunculus_emissive:f32,
    // Ionisation totale (Wolf-Rayet) — lignes d'émission He II, C IV
    pub ionization_enabled:bool, pub ionization_count:u32, pub ionization_voxel:f32,
    pub ionization_color:[f32;3], pub ionization_emissive:f32, pub ionization_radius:f32,
    // Couronne hyper-étendue
    pub corona_count:u32, pub corona_voxel:f32, pub corona_color:[f32;3], pub corona_emissive:f32, pub corona_radius:f32,
    pub light_intensity:f32, pub light_range:f32, pub seed:u32,
}
impl Default for HypergiantConfig {
    fn default()->Self{Self{
        position:Vec3::ZERO,orbit_distance:0.0,orbit_speed:0.0,spin_speed:0.03,
        hg_type:HypergiantType::Red,radius:1400.0,voxel_size:50.0,resolution:24,
        color_core:[1.0,0.5,0.1],color_surface:[0.75,0.2,0.04],emissive:8.0,
        pulsation_period:20.0,pulsation_amplitude:0.06,
        convection_cells:40,convection_voxel:55.0,convection_hot:[1.0,0.6,0.18],convection_cold:[0.65,0.18,0.04],convection_emissive:6.0,convection_speed:0.06,
        wind_count:600,wind_voxel:20.0,wind_color:[0.7,0.38,0.1],wind_emissive:1.5,wind_speed:5.0,wind_radius:5000.0,
        shells:vec![(2500.0,80.0,[0.8,0.45,0.15],2.0),(3800.0,120.0,[0.6,0.3,0.5],1.5)],
        shell_count_per:300,shell_voxel:22.0,
        homunculus_enabled:false,homunculus_length:3000.0,homunculus_width:600.0,
        homunculus_count:0,homunculus_voxel:24.0,homunculus_color:[0.8,0.6,0.9],homunculus_emissive:4.0,
        ionization_enabled:false,ionization_count:0,ionization_voxel:16.0,ionization_color:[0.3,0.8,1.0],ionization_emissive:8.0,ionization_radius:0.0,
        corona_count:600,corona_voxel:30.0,corona_color:[1.0,0.65,0.28],corona_emissive:4.0,corona_radius:1000.0,
        light_intensity:500_000_000.0,light_range:60000.0,seed:31,
    }}
}
impl HypergiantConfig {
    pub fn eta_carinae(seed:u32,pos:Vec3)->Self{Self{position:pos,hg_type:HypergiantType::BlueLuminous,radius:800.0,voxel_size:35.0,color_core:[0.7,0.8,1.0],color_surface:[0.5,0.65,1.0],emissive:18.0,pulsation_amplitude:0.1,wind_speed:30.0,wind_radius:8000.0,homunculus_enabled:true,homunculus_length:4000.0,homunculus_width:1000.0,homunculus_count:500,light_intensity:2_000_000_000.0,light_range:100000.0,seed,..Default::default()}}
    pub fn wolf_rayet(seed:u32,pos:Vec3)->Self{Self{position:pos,hg_type:HypergiantType::WolfRayet,radius:300.0,voxel_size:18.0,color_core:[0.4,0.7,1.0],color_surface:[0.2,0.5,1.0],emissive:25.0,wind_speed:80.0,wind_radius:10000.0,shells:vec![],ionization_enabled:true,ionization_count:400,ionization_voxel:14.0,ionization_color:[0.2,0.9,1.0],ionization_emissive:12.0,ionization_radius:1500.0,light_intensity:3_000_000_000.0,light_range:120000.0,seed,..Default::default()}}
}

pub const SPAWN_PROPS: crate::system_gen::SpawnProps = crate::system_gen::SpawnProps {
    category:  crate::system_gen::AstreCategory::Star,
    weight:    0.04,
    orbit_min: 0.0,
    orbit_max: 0.0,
};

pub fn generate_random(rng: &mut crate::system_gen::SeedRng, stars: &mut Vec<HypergiantConfig>) {
    let seed = rng.u32();
    let roll = rng.f32();
    let config = if roll < 0.50 {
        HypergiantConfig { position: Vec3::ZERO, seed, ..Default::default() }
    } else if roll < 0.80 {
        HypergiantConfig::eta_carinae(seed, Vec3::ZERO)
    } else {
        HypergiantConfig::wolf_rayet(seed, Vec3::ZERO)
    };
    stars.push(config);
}

pub struct HypergiantPlugin;
impl Plugin for HypergiantPlugin {
    fn build(&self,app:&mut App){
        app.init_resource::<HypergiantRes>().add_event::<RegenerateHypergiant>()
           .add_systems(Startup,spawn_hypergiants)
           .add_systems(Update,(orbit_hg,animate_hg_body,animate_hg_convection,animate_hg_wind,animate_hg_shells,animate_homunculus,animate_ionization,animate_hg_corona,regenerate_hg,reload_hg).chain());
    }
}
#[derive(Resource)] pub struct HypergiantRes{pub stars:Vec<HypergiantConfig>}
impl Default for HypergiantRes{fn default()->Self{Self{stars:vec![
    HypergiantConfig{position:Vec3::new(0.0,0.0,16000.0),..Default::default()},
]}}}
#[derive(Event)] pub struct RegenerateHypergiant;
#[derive(Component)] pub struct HgRoot      {pub idx:usize}
#[derive(Component)] pub struct HgBody      {pub idx:usize}
#[derive(Component)] pub struct HgConv      {pub idx:usize,pub origin:Vec3,pub seed:f32,pub phase:f32}
#[derive(Component)] pub struct HgWind      {pub idx:usize,pub dir:Vec3,pub t:f32,pub seed:f32}
#[derive(Component)] pub struct HgShell     {pub idx:usize,pub shell_idx:usize,pub dir:Vec3,pub seed:f32}
#[derive(Component)] pub struct HgHomunculus{pub idx:usize,pub pole:f32,pub t:f32,pub perp:f32,pub seed:f32}
#[derive(Component)] pub struct HgIonize    {pub idx:usize,pub theta:f32,pub phi:f32,pub r:f32,pub seed:f32}
#[derive(Component)] pub struct HgCorona    {pub idx:usize,pub theta:f32,pub phi:f32,pub r:f32,pub seed:f32}

fn ph(a:f32,b:f32)->f32{((a*12.9898+b*78.233).sin()*43758.5453).fract()}
fn snap(v:Vec3,g:f32)->Vec3{Vec3::new((v.x/g).round()*g,(v.y/g).round()*g,(v.z/g).round()*g)}
fn sph(lat:f32,lon:f32,r:f32)->Vec3{Vec3::new(r*lat.cos()*lon.cos(),r*lat.sin(),r*lat.cos()*lon.sin())}
fn lc(a:[f32;3],b:[f32;3],t:f32)->[f32;3]{let t=t.clamp(0.0,1.0);[a[0]+(b[0]-a[0])*t,a[1]+(b[1]-a[1])*t,a[2]+(b[2]-a[2])*t]}

fn spawn_hypergiants(mut cmd:Commands,res:Res<HypergiantRes>,mut msh:ResMut<Assets<Mesh>>,mut mat:ResMut<Assets<StandardMaterial>>){
    for (i,cfg) in res.stars.iter().enumerate(){build_hg(&mut cmd,cfg,i,&mut msh,&mut mat);}
}
fn build_hg(cmd:&mut Commands,cfg:&HypergiantConfig,idx:usize,msh:&mut ResMut<Assets<Mesh>>,mat:&mut ResMut<Assets<StandardMaterial>>){
    let pos=if cfg.orbit_distance>1.0{Vec3::new(cfg.orbit_distance,0.0,0.0)}else{cfg.position};
    let root=cmd.spawn((Transform::from_translation(pos),Visibility::default(),HgRoot{idx},AstreLodRoot{cull_dist:25000.0, radius: cfg.radius, streamable: true, label: "Hypergiant"})).id();
    let body=cmd.spawn((Transform::IDENTITY,Visibility::default(),HgBody{idx})).id();
    cmd.entity(root).add_child(body);
    let vs=cfg.voxel_size;let rs=cfg.resolution;
    let smesh=msh.add(Mesh::from(Cuboid::new(vs,vs,vs)));
    let vstep=std::f32::consts::PI/rs as f32;let hstep=std::f32::consts::TAU/(rs*2) as f32;
    let mut lat=-std::f32::consts::FRAC_PI_2;
    while lat<=std::f32::consts::FRAC_PI_2 {
        let mut lon=0.0_f32;
        while lon<std::f32::consts::TAU {
            let noise=ph(lat*3.5+cfg.seed as f32,lon*2.0)*0.18-0.09;
            let r=cfg.radius*(1.0+noise);
            let t2=(lat+std::f32::consts::FRAC_PI_2)/std::f32::consts::PI;
            let[sr,sg,sb]=lc(cfg.color_surface,cfg.color_core,t2);
            let se=cfg.emissive*(0.55+t2*0.65);
            let sm=mat.add(StandardMaterial{base_color:Color::srgb(sr,sg,sb),emissive:LinearRgba::new(sr*se,sg*se,sb*se,1.0),unlit:true,..default()});
            let v=cmd.spawn((Mesh3d(smesh.clone()),MeshMaterial3d(sm),Transform::from_translation(snap(sph(lat,lon,r),vs)),NotShadowCaster)).id();
            cmd.entity(body).add_child(v);
            lon+=hstep;
        }
        lat+=vstep;
    }
    // Convection
    let gm=msh.add(Mesh::from(Cuboid::new(cfg.convection_voxel,cfg.convection_voxel,cfg.convection_voxel)));
    for i in 0..cfg.convection_cells {
        let h1=ph(cfg.seed as f32+100.0,i as f32);let h2=ph(cfg.seed as f32+101.0,i as f32);let h3=ph(cfg.seed as f32+102.0,i as f32);
        let theta=h1*std::f32::consts::TAU;let phi=(h2*2.0-1.0).clamp(-1.0,1.0).acos();
        let r=cfg.radius+h3*cfg.convection_voxel*2.0;
        let o=Vec3::new(r*phi.sin()*theta.cos(),r*phi.cos(),r*phi.sin()*theta.sin());
        let[gr,gg,gb]=cfg.convection_hot;let ge=cfg.convection_emissive;
        let gmat=mat.add(StandardMaterial{base_color:Color::srgb(gr,gg,gb),emissive:LinearRgba::new(gr*ge,gg*ge,gb*ge,1.0),alpha_mode:AlphaMode::Add,unlit:true,..default()});
        let gv=cmd.spawn((Mesh3d(gm.clone()),MeshMaterial3d(gmat),Transform::from_translation(snap(o,cfg.convection_voxel)),Visibility::default(),NotShadowCaster,HgConv{idx,origin:snap(o,cfg.convection_voxel),seed:ph(h1,h2),phase:h3*std::f32::consts::TAU})).id();
        cmd.entity(root).add_child(gv);
    }
    // Vent
    let wm=mat.add(StandardMaterial{base_color:Color::srgb(cfg.wind_color[0],cfg.wind_color[1],cfg.wind_color[2]),emissive:LinearRgba::new(cfg.wind_color[0]*cfg.wind_emissive,cfg.wind_color[1]*cfg.wind_emissive,cfg.wind_color[2]*cfg.wind_emissive,1.0),alpha_mode:AlphaMode::Add,unlit:true,..default()});
    let wmesh=msh.add(Mesh::from(Cuboid::new(cfg.wind_voxel,cfg.wind_voxel,cfg.wind_voxel)));
    for i in 0..cfg.wind_count {
        let h1=ph(cfg.seed as f32+200.0,i as f32);let h2=ph(cfg.seed as f32+201.0,i as f32);
        let theta=h1*std::f32::consts::TAU;let phi=(h2*2.0-1.0).clamp(-1.0,1.0).acos();
        let dir=Vec3::new(phi.sin()*theta.cos(),phi.cos(),phi.sin()*theta.sin()).normalize();
        let wv=cmd.spawn((Mesh3d(wmesh.clone()),MeshMaterial3d(wm.clone()),Transform::from_translation(Vec3::ZERO),Visibility::default(),NotShadowCaster,HgWind{idx,dir,t:ph(h1,h2),seed:ph(h2,h1)})).id();
        cmd.entity(root).add_child(wv);
    }
    // Coquilles multiples
    let sv_mesh=msh.add(Mesh::from(Cuboid::new(cfg.shell_voxel,cfg.shell_voxel,cfg.shell_voxel)));
    for (si,(shell_r,shell_t,shell_col,shell_e)) in cfg.shells.iter().enumerate() {
        let[shr,shg,shb]=*shell_col;
        let shm=mat.add(StandardMaterial{base_color:Color::srgb(shr,shg,shb),emissive:LinearRgba::new(shr*shell_e,shg*shell_e,shb*shell_e,1.0),alpha_mode:AlphaMode::Add,unlit:true,..default()});
        for i in 0..cfg.shell_count_per {
            let h1=ph(cfg.seed as f32+300.0+si as f32*50.0,i as f32);let h2=ph(cfg.seed as f32+301.0+si as f32*50.0,i as f32);let h3=ph(cfg.seed as f32+302.0+si as f32*50.0,i as f32);
            let theta=h1*std::f32::consts::TAU;let phi=(h2*2.0-1.0).clamp(-1.0,1.0).acos();
            let dir=Vec3::new(phi.sin()*theta.cos(),phi.cos(),phi.sin()*theta.sin()).normalize();
            let r=shell_r+(h3*2.0-1.0)*shell_t*0.5;
            let shv=cmd.spawn((Mesh3d(sv_mesh.clone()),MeshMaterial3d(shm.clone()),Transform::from_translation(dir*r),Visibility::default(),NotShadowCaster,HgShell{idx,shell_idx:si,dir,seed:ph(h1,h3)})).id();
            cmd.entity(root).add_child(shv);
        }
    }
    // Homuncule
    if cfg.homunculus_enabled {
        let hm=mat.add(StandardMaterial{base_color:Color::srgb(cfg.homunculus_color[0],cfg.homunculus_color[1],cfg.homunculus_color[2]),emissive:LinearRgba::new(cfg.homunculus_color[0]*cfg.homunculus_emissive,cfg.homunculus_color[1]*cfg.homunculus_emissive,cfg.homunculus_color[2]*cfg.homunculus_emissive,1.0),alpha_mode:AlphaMode::Add,unlit:true,..default()});
        let hmesh=msh.add(Mesh::from(Cuboid::new(cfg.homunculus_voxel,cfg.homunculus_voxel,cfg.homunculus_voxel)));
        for pole in [1.0_f32,-1.0_f32] {
            for i in 0..cfg.homunculus_count {
                let h1=ph(cfg.seed as f32+400.0+pole*30.0,i as f32);let h2=ph(cfg.seed as f32+401.0+pole*30.0,i as f32);
                let hv=cmd.spawn((Mesh3d(hmesh.clone()),MeshMaterial3d(hm.clone()),Transform::from_translation(Vec3::ZERO),Visibility::default(),NotShadowCaster,HgHomunculus{idx,pole,t:h1,perp:h2,seed:ph(h1,h2)})).id();
                cmd.entity(root).add_child(hv);
            }
        }
    }
    // Ionisation Wolf-Rayet
    if cfg.ionization_enabled {
        let im=mat.add(StandardMaterial{base_color:Color::srgb(cfg.ionization_color[0],cfg.ionization_color[1],cfg.ionization_color[2]),emissive:LinearRgba::new(cfg.ionization_color[0]*cfg.ionization_emissive,cfg.ionization_color[1]*cfg.ionization_emissive,cfg.ionization_color[2]*cfg.ionization_emissive,1.0),alpha_mode:AlphaMode::Add,unlit:true,..default()});
        let imesh=msh.add(Mesh::from(Cuboid::new(cfg.ionization_voxel,cfg.ionization_voxel,cfg.ionization_voxel)));
        for i in 0..cfg.ionization_count {
            let h1=ph(cfg.seed as f32+500.0,i as f32);let h2=ph(cfg.seed as f32+501.0,i as f32);let h3=ph(cfg.seed as f32+502.0,i as f32);
            let theta=h1*std::f32::consts::TAU;let phi=(h2*2.0-1.0).clamp(-1.0,1.0).acos();
            let r=cfg.radius+h3.sqrt()*cfg.ionization_radius;
            let iv=cmd.spawn((Mesh3d(imesh.clone()),MeshMaterial3d(im.clone()),Transform::from_translation(Vec3::ZERO),Visibility::default(),NotShadowCaster,HgIonize{idx,theta,phi,r,seed:ph(h1,h3)})).id();
            cmd.entity(root).add_child(iv);
        }
    }
    // Couronne
    let com=mat.add(StandardMaterial{base_color:Color::srgb(cfg.corona_color[0],cfg.corona_color[1],cfg.corona_color[2]),emissive:LinearRgba::new(cfg.corona_color[0]*cfg.corona_emissive,cfg.corona_color[1]*cfg.corona_emissive,cfg.corona_color[2]*cfg.corona_emissive,1.0),alpha_mode:AlphaMode::Add,unlit:true,..default()});
    let comesh=msh.add(Mesh::from(Cuboid::new(cfg.corona_voxel,cfg.corona_voxel,cfg.corona_voxel)));
    for i in 0..cfg.corona_count {
        let h1=ph(cfg.seed as f32+600.0,i as f32);let h2=ph(cfg.seed as f32+601.0,i as f32);let h3=ph(cfg.seed as f32+602.0,i as f32);
        let theta=h1*std::f32::consts::TAU;let phi=(h2*2.0-1.0).clamp(-1.0,1.0).acos();
        let r=cfg.radius+h3.sqrt()*cfg.corona_radius;
        let cov=cmd.spawn((Mesh3d(comesh.clone()),MeshMaterial3d(com.clone()),Transform::from_translation(Vec3::ZERO),Visibility::default(),NotShadowCaster,HgCorona{idx,theta,phi,r,seed:ph(h1,h3)})).id();
        cmd.entity(root).add_child(cov);
    }
    let[lr,lg,lb]=cfg.color_core;
    cmd.spawn((PointLight{intensity:cfg.light_intensity * 100.0,range:cfg.light_range,color:Color::srgb(lr,lg,lb),shadows_enabled:true,..default()},Transform::IDENTITY));
}

fn orbit_hg(time:Res<Time>,res:Res<HypergiantRes>,mut q:Query<(&mut Transform,&HgRoot)>){let t=time.elapsed_secs();for (mut tf,r) in &mut q{if let Some(cfg)=res.stars.get(r.idx){if cfg.orbit_distance>1.0{let a=t*cfg.orbit_speed;tf.translation.x=a.cos()*cfg.orbit_distance;tf.translation.z=a.sin()*cfg.orbit_distance;}}}}
fn animate_hg_body(time:Res<Time>,res:Res<HypergiantRes>,mut q:Query<(&mut Transform,&HgBody)>){let t=time.elapsed_secs();for (mut tf,b) in &mut q{if let Some(cfg)=res.stars.get(b.idx){tf.rotation=Quat::from_rotation_y(t*cfg.spin_speed);let p=1.0+(t*std::f32::consts::TAU/cfg.pulsation_period).sin()*cfg.pulsation_amplitude;tf.scale=Vec3::splat(p);}}}
fn animate_hg_convection(time:Res<Time>,res:Res<HypergiantRes>,mut q:Query<(&HgConv,&mut Transform,&MeshMaterial3d<StandardMaterial>)>,mut mats:ResMut<Assets<StandardMaterial>>){
    let t=time.elapsed_secs();
    for (gv,mut tf,mh) in &mut q{let Some(cfg)=res.stars.get(gv.idx) else{continue;};let cycle=(t*cfg.convection_speed+gv.phase).sin()*0.5+0.5;let drift=Vec3::new((t*cfg.convection_speed*0.7+gv.seed*11.0).sin()*30.0,(t*cfg.convection_speed*0.5+gv.seed*7.3).cos()*15.0,(t*cfg.convection_speed*0.9+gv.seed*5.1).sin()*30.0);tf.translation=snap(gv.origin+drift,cfg.convection_voxel);let[hr,hg,hb]=cfg.convection_hot;let[cr,cg,cb]=cfg.convection_cold;let[rr,rg,rb]=lc([cr,cg,cb],[hr,hg,hb],cycle);let ge=cfg.convection_emissive*(0.3+cycle*0.9);if let Some(m)=mats.get_mut(&mh.0){m.base_color=Color::srgb(rr,rg,rb);m.emissive=LinearRgba::new(rr*ge,rg*ge,rb*ge,1.0);}let s=0.4+cycle*0.9;tf.scale=Vec3::splat(s);}
}
fn animate_hg_wind(time:Res<Time>,res:Res<HypergiantRes>,mut q:Query<(&HgWind,&mut Transform,&mut Visibility)>){let t=time.elapsed_secs();for (wv,mut tf,mut vis) in &mut q{let Some(cfg)=res.stars.get(wv.idx) else{*vis=Visibility::Hidden;continue;};let local_t=((t*cfg.wind_speed/cfg.wind_radius+wv.t)%1.0).max(0.0);let dist=cfg.radius+local_t*cfg.wind_radius;let jit=Vec3::new((t*0.1+wv.seed*7.0).sin(),(t*0.12+wv.seed*5.0).cos(),(t*0.11+wv.seed*9.0).sin())*dist*0.035;tf.translation=snap(wv.dir*dist+jit,cfg.wind_voxel);let fade=(1.0-local_t*0.7).max(0.02);tf.scale=Vec3::splat(fade);*vis=Visibility::Visible;}}
fn animate_hg_shells(time:Res<Time>,res:Res<HypergiantRes>,mut q:Query<(&HgShell,&mut Transform)>){let t=time.elapsed_secs();for (sv,mut tf) in &mut q{let Some(cfg)=res.stars.get(sv.idx) else{continue;};if let Some((shell_r,_,_,_))=cfg.shells.get(sv.shell_idx){let pulse=1.0+(t*0.2+sv.seed*std::f32::consts::TAU).sin()*0.04;let r=shell_r*pulse;tf.translation=snap(sv.dir*r,cfg.shell_voxel);let s=0.5+(t*0.5+sv.seed*5.0).sin().abs()*0.5;tf.scale=Vec3::splat(s.max(0.02));}}}
fn animate_homunculus(time:Res<Time>,res:Res<HypergiantRes>,mut q:Query<(&HgHomunculus,&mut Transform,&mut Visibility)>){let t=time.elapsed_secs();for (hv,mut tf,mut vis) in &mut q{let Some(cfg)=res.stars.get(hv.idx) else{*vis=Visibility::Hidden;continue;};if !cfg.homunculus_enabled{*vis=Visibility::Hidden;continue;}let lt=((t*0.12+hv.t)%1.0).max(0.0);let along=cfg.radius+lt*cfg.homunculus_length;let cw=lt.powf(0.4)*cfg.homunculus_width*0.5;let pa=hv.perp*std::f32::consts::TAU+t*0.08;tf.translation=snap(Vec3::new(pa.cos()*cw,hv.pole*along,pa.sin()*cw),cfg.homunculus_voxel);let fade=(1.0-lt*0.75).max(0.02);let pulse=0.4+(t*0.6+hv.seed*5.0).sin().abs()*0.7;tf.scale=Vec3::splat(fade*pulse);*vis=Visibility::Visible;}}
fn animate_ionization(time:Res<Time>,res:Res<HypergiantRes>,mut q:Query<(&HgIonize,&mut Transform)>){let t=time.elapsed_secs();for (iv,mut tf) in &mut q{let Some(cfg)=res.stars.get(iv.idx) else{continue;};if !cfg.ionization_enabled{continue;}let pulse=1.0+(t*2.5+iv.seed*std::f32::consts::TAU).sin()*0.3;let r=iv.r*pulse;let th=iv.theta+t*0.2;tf.translation=snap(Vec3::new(r*iv.phi.sin()*th.cos(),r*iv.phi.cos(),r*iv.phi.sin()*th.sin()),cfg.ionization_voxel);let life=1.0-((r-cfg.radius)/cfg.ionization_radius.max(1.0)).clamp(0.0,1.0);let s=0.3+(t*3.0+iv.seed*7.0).sin().abs()*0.8*life;tf.scale=Vec3::splat(s.max(0.02));}}
fn animate_hg_corona(time:Res<Time>,res:Res<HypergiantRes>,mut q:Query<(&HgCorona,&mut Transform)>){let t=time.elapsed_secs();for (cv,mut tf) in &mut q{let Some(cfg)=res.stars.get(cv.idx) else{continue;};let pulse=1.0+(t*0.45+cv.seed*std::f32::consts::TAU).sin()*0.2;let r=cv.r*pulse;let th=cv.theta+t*0.02;tf.translation=snap(Vec3::new(r*cv.phi.sin()*th.cos(),r*cv.phi.cos(),r*cv.phi.sin()*th.sin()),cfg.corona_voxel);let life=1.0-((r-cfg.radius)/cfg.corona_radius.max(1.0)).clamp(0.0,1.0);tf.scale=Vec3::splat((0.3+life*1.0).max(0.02));}}
fn reload_hg(
    mut cmd: Commands,
    mut events: EventReader<ReloadAstre>,
    res: Res<HypergiantRes>,
    roots: Query<(Entity, &HgRoot)>,
    mut msh: ResMut<Assets<Mesh>>,
    mut mat: ResMut<Assets<StandardMaterial>>,
) {
    for ev in events.read() {
        let Ok((entity, root)) = roots.get(ev.0) else { continue };
        let idx = root.idx;
        let Some(cfg) = res.stars.get(idx) else { continue };
        cmd.entity(entity).despawn_recursive();
        build_hg(&mut cmd, cfg, idx, &mut msh, &mut mat);
    }
}

fn regenerate_hg(mut cmd:Commands,mut ev:EventReader<RegenerateHypergiant>,res:Res<HypergiantRes>,rq:Query<Entity,With<HgRoot>>,mut msh:ResMut<Assets<Mesh>>,mut mat:ResMut<Assets<StandardMaterial>>){let mut f=false;for _ in ev.read(){f=true;}if !f{return;}for e in &rq{cmd.entity(e).despawn_recursive();}for (i,cfg) in res.stars.iter().enumerate(){build_hg(&mut cmd,cfg,i,&mut msh,&mut mat);}}
