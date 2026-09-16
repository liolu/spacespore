use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;

#[derive(Clone,Debug,PartialEq)]
pub enum SupergiantClass { RedA, RedB, BlueA, BlueB, YellowHypergiant }

#[derive(Clone,Debug)]
pub struct SupergiantConfig {
    pub position:Vec3, pub orbit_distance:f32, pub orbit_speed:f32, pub spin_speed:f32,
    pub class:SupergiantClass,
    pub radius:f32, pub voxel_size:f32, pub resolution:u32,
    pub color_core:[f32;3], pub color_surface:[f32;3], pub emissive:f32,
    pub pulsation_period:f32, pub pulsation_amplitude:f32,
    pub convection_cells:u32, pub convection_voxel:f32,
    pub convection_hot:[f32;3], pub convection_cold:[f32;3], pub convection_emissive:f32, pub convection_speed:f32,
    pub wind_count:u32, pub wind_voxel:f32, pub wind_color:[f32;3], pub wind_emissive:f32, pub wind_speed:f32, pub wind_radius:f32,
    pub ejecta_enabled:bool, pub ejecta_count:u32, pub ejecta_voxel:f32, pub ejecta_color:[f32;3], pub ejecta_emissive:f32, pub ejecta_radius:f32,
    pub corona_count:u32, pub corona_voxel:f32, pub corona_color:[f32;3], pub corona_emissive:f32, pub corona_radius:f32,
    pub bipolar_nebula:bool, pub nebula_count:u32, pub nebula_voxel:f32, pub nebula_color:[f32;3], pub nebula_emissive:f32, pub nebula_length:f32,
    pub light_intensity:f32, pub light_range:f32, pub seed:u32,
}
impl Default for SupergiantConfig {
    fn default()->Self{Self{
        position:Vec3::ZERO,orbit_distance:0.0,orbit_speed:0.0,spin_speed:0.06,
        class:SupergiantClass::RedA,radius:700.0,voxel_size:30.0,resolution:24,
        color_core:[1.0,0.55,0.1],color_surface:[0.8,0.25,0.05],emissive:7.0,
        pulsation_period:14.0,pulsation_amplitude:0.05,
        convection_cells:60,convection_voxel:30.0,convection_hot:[1.0,0.65,0.2],convection_cold:[0.7,0.2,0.04],convection_emissive:5.0,convection_speed:0.1,
        wind_count:400,wind_voxel:16.0,wind_color:[0.75,0.42,0.1],wind_emissive:1.2,wind_speed:8.0,wind_radius:2000.0,
        ejecta_enabled:true,ejecta_count:200,ejecta_voxel:18.0,ejecta_color:[0.8,0.5,0.15],ejecta_emissive:2.5,ejecta_radius:1200.0,
        corona_count:500,corona_voxel:22.0,corona_color:[1.0,0.7,0.3],corona_emissive:3.5,corona_radius:500.0,
        bipolar_nebula:false,nebula_count:0,nebula_voxel:20.0,nebula_color:[0.8,0.5,0.9],nebula_emissive:3.0,nebula_length:0.0,
        light_intensity:80_000_000.0,light_range:25000.0,seed:23,
    }}
}
impl SupergiantConfig {
    pub fn blue(seed:u32,pos:Vec3)->Self{Self{position:pos,class:SupergiantClass::BlueA,radius:400.0,voxel_size:22.0,color_core:[0.6,0.75,1.0],color_surface:[0.4,0.6,1.0],emissive:20.0,wind_speed:50.0,wind_radius:3000.0,light_intensity:200_000_000.0,light_range:40000.0,seed,..Default::default()}}
    pub fn lbv(seed:u32,pos:Vec3)->Self{Self{position:pos,class:SupergiantClass::YellowHypergiant,radius:550.0,voxel_size:26.0,color_core:[1.0,0.9,0.4],color_surface:[0.9,0.75,0.25],emissive:14.0,ejecta_radius:2500.0,bipolar_nebula:true,nebula_count:300,nebula_voxel:20.0,nebula_length:2000.0,nebula_color:[0.7,0.6,0.9],nebula_emissive:4.0,seed,..Default::default()}}
}

pub struct SupergiantPlugin;
impl Plugin for SupergiantPlugin {
    fn build(&self,app:&mut App){
        app.init_resource::<SupergiantRes>().add_event::<RegenerateSupergiant>()
           .add_systems(Startup,spawn_supergiants)
           .add_systems(Update,(orbit_sg,animate_sg_body,animate_sg_convection,animate_sg_wind,animate_sg_ejecta,animate_sg_corona,animate_bipolar_nebula,regenerate_sg).chain());
    }
}
#[derive(Resource)] pub struct SupergiantRes{pub stars:Vec<SupergiantConfig>}
impl Default for SupergiantRes{fn default()->Self{Self{stars:vec![SupergiantConfig::default()]}}}
#[derive(Event)] pub struct RegenerateSupergiant;
#[derive(Component)] pub struct SgRoot    {pub idx:usize}
#[derive(Component)] pub struct SgBody    {pub idx:usize}
#[derive(Component)] pub struct SgConv    {pub idx:usize,pub origin:Vec3,pub seed:f32,pub phase:f32}
#[derive(Component)] pub struct SgWind    {pub idx:usize,pub dir:Vec3,pub t:f32,pub seed:f32}
#[derive(Component)] pub struct SgEjecta  {pub idx:usize,pub dir:Vec3,pub t:f32,pub seed:f32}
#[derive(Component)] pub struct SgCorona  {pub idx:usize,pub theta:f32,pub phi:f32,pub r:f32,pub seed:f32}
#[derive(Component)] pub struct SgNebula  {pub idx:usize,pub pole:f32,pub t:f32,pub perp:f32,pub seed:f32}

fn ph(a:f32,b:f32)->f32{((a*12.9898+b*78.233).sin()*43758.5453).fract()}
fn snap(v:Vec3,g:f32)->Vec3{Vec3::new((v.x/g).round()*g,(v.y/g).round()*g,(v.z/g).round()*g)}
fn sph(lat:f32,lon:f32,r:f32)->Vec3{Vec3::new(r*lat.cos()*lon.cos(),r*lat.sin(),r*lat.cos()*lon.sin())}
fn lc(a:[f32;3],b:[f32;3],t:f32)->[f32;3]{let t=t.clamp(0.0,1.0);[a[0]+(b[0]-a[0])*t,a[1]+(b[1]-a[1])*t,a[2]+(b[2]-a[2])*t]}

fn spawn_supergiants(mut cmd:Commands,res:Res<SupergiantRes>,mut msh:ResMut<Assets<Mesh>>,mut mat:ResMut<Assets<StandardMaterial>>){
    for (i,cfg) in res.stars.iter().enumerate(){build_sg(&mut cmd,cfg,i,&mut msh,&mut mat);}
}
fn build_sg(cmd:&mut Commands,cfg:&SupergiantConfig,idx:usize,msh:&mut ResMut<Assets<Mesh>>,mat:&mut ResMut<Assets<StandardMaterial>>){
    let pos=if cfg.orbit_distance>1.0{Vec3::new(cfg.orbit_distance,0.0,0.0)}else{cfg.position};
    let root=cmd.spawn((Transform::from_translation(pos),Visibility::default(),SgRoot{idx})).id();
    let body=cmd.spawn((Transform::IDENTITY,Visibility::default(),SgBody{idx})).id();
    cmd.entity(root).add_child(body);
    let vs=cfg.voxel_size; let rs=cfg.resolution;
    let smesh=msh.add(Mesh::from(Cuboid::new(vs,vs,vs)));
    let vstep=std::f32::consts::PI/rs as f32; let hstep=std::f32::consts::TAU/(rs*2) as f32;
    let mut lat=-std::f32::consts::FRAC_PI_2;
    while lat<=std::f32::consts::FRAC_PI_2 {
        let mut lon=0.0_f32;
        while lon<std::f32::consts::TAU {
            let noise=ph(lat*4.0+cfg.seed as f32,lon*2.5)*0.15-0.075;
            let r=cfg.radius*(1.0+noise);
            let t2=(lat+std::f32::consts::FRAC_PI_2)/std::f32::consts::PI;
            let[sr,sg,sb]=lc(cfg.color_surface,cfg.color_core,t2);
            let se=cfg.emissive*(0.6+t2*0.6);
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
        let r=cfg.radius+h3*cfg.convection_voxel*1.5;
        let o=Vec3::new(r*phi.sin()*theta.cos(),r*phi.cos(),r*phi.sin()*theta.sin());
        let[gr,gg,gb]=cfg.convection_hot;let ge=cfg.convection_emissive;
        let gmat=mat.add(StandardMaterial{base_color:Color::srgb(gr,gg,gb),emissive:LinearRgba::new(gr*ge,gg*ge,gb*ge,1.0),alpha_mode:AlphaMode::Add,unlit:true,..default()});
        let gv=cmd.spawn((Mesh3d(gm.clone()),MeshMaterial3d(gmat),Transform::from_translation(snap(o,cfg.convection_voxel)),Visibility::default(),NotShadowCaster,SgConv{idx,origin:snap(o,cfg.convection_voxel),seed:ph(h1,h2),phase:h3*std::f32::consts::TAU})).id();
        cmd.entity(root).add_child(gv);
    }
    // Vent + éjecta
    let wm=mat.add(StandardMaterial{base_color:Color::srgb(cfg.wind_color[0],cfg.wind_color[1],cfg.wind_color[2]),emissive:LinearRgba::new(cfg.wind_color[0]*cfg.wind_emissive,cfg.wind_color[1]*cfg.wind_emissive,cfg.wind_color[2]*cfg.wind_emissive,1.0),alpha_mode:AlphaMode::Add,unlit:true,..default()});
    let wmesh=msh.add(Mesh::from(Cuboid::new(cfg.wind_voxel,cfg.wind_voxel,cfg.wind_voxel)));
    for i in 0..cfg.wind_count {
        let h1=ph(cfg.seed as f32+200.0,i as f32);let h2=ph(cfg.seed as f32+201.0,i as f32);
        let theta=h1*std::f32::consts::TAU;let phi=(h2*2.0-1.0).clamp(-1.0,1.0).acos();
        let dir=Vec3::new(phi.sin()*theta.cos(),phi.cos(),phi.sin()*theta.sin()).normalize();
        let wv=cmd.spawn((Mesh3d(wmesh.clone()),MeshMaterial3d(wm.clone()),Transform::from_translation(Vec3::ZERO),Visibility::default(),NotShadowCaster,SgWind{idx,dir,t:ph(h1,h2),seed:ph(h2,h1)})).id();
        cmd.entity(root).add_child(wv);
    }
    if cfg.ejecta_enabled {
        let em=mat.add(StandardMaterial{base_color:Color::srgb(cfg.ejecta_color[0],cfg.ejecta_color[1],cfg.ejecta_color[2]),emissive:LinearRgba::new(cfg.ejecta_color[0]*cfg.ejecta_emissive,cfg.ejecta_color[1]*cfg.ejecta_emissive,cfg.ejecta_color[2]*cfg.ejecta_emissive,1.0),alpha_mode:AlphaMode::Add,unlit:true,..default()});
        let emesh=msh.add(Mesh::from(Cuboid::new(cfg.ejecta_voxel,cfg.ejecta_voxel,cfg.ejecta_voxel)));
        for i in 0..cfg.ejecta_count {
            let h1=ph(cfg.seed as f32+300.0,i as f32);let h2=ph(cfg.seed as f32+301.0,i as f32);
            let theta=h1*std::f32::consts::TAU;let phi=(h2*2.0-1.0).clamp(-1.0,1.0).acos();
            let dir=Vec3::new(phi.sin()*theta.cos(),phi.cos(),phi.sin()*theta.sin()).normalize();
            let ev=cmd.spawn((Mesh3d(emesh.clone()),MeshMaterial3d(em.clone()),Transform::from_translation(Vec3::ZERO),Visibility::default(),NotShadowCaster,SgEjecta{idx,dir,t:ph(h1,h2),seed:ph(h2,h1)})).id();
            cmd.entity(root).add_child(ev);
        }
    }
    // Couronne
    let com=mat.add(StandardMaterial{base_color:Color::srgb(cfg.corona_color[0],cfg.corona_color[1],cfg.corona_color[2]),emissive:LinearRgba::new(cfg.corona_color[0]*cfg.corona_emissive,cfg.corona_color[1]*cfg.corona_emissive,cfg.corona_color[2]*cfg.corona_emissive,1.0),alpha_mode:AlphaMode::Add,unlit:true,..default()});
    let comesh=msh.add(Mesh::from(Cuboid::new(cfg.corona_voxel,cfg.corona_voxel,cfg.corona_voxel)));
    for i in 0..cfg.corona_count {
        let h1=ph(cfg.seed as f32+400.0,i as f32);let h2=ph(cfg.seed as f32+401.0,i as f32);let h3=ph(cfg.seed as f32+402.0,i as f32);
        let theta=h1*std::f32::consts::TAU;let phi=(h2*2.0-1.0).clamp(-1.0,1.0).acos();
        let r=cfg.radius+h3.sqrt()*cfg.corona_radius;
        let cov=cmd.spawn((Mesh3d(comesh.clone()),MeshMaterial3d(com.clone()),Transform::from_translation(Vec3::ZERO),Visibility::default(),NotShadowCaster,SgCorona{idx,theta,phi,r,seed:ph(h1,h3)})).id();
        cmd.entity(root).add_child(cov);
    }
    // Nébuleuse bipolaire (LBV/Eta Carinae)
    if cfg.bipolar_nebula {
        let nm=mat.add(StandardMaterial{base_color:Color::srgb(cfg.nebula_color[0],cfg.nebula_color[1],cfg.nebula_color[2]),emissive:LinearRgba::new(cfg.nebula_color[0]*cfg.nebula_emissive,cfg.nebula_color[1]*cfg.nebula_emissive,cfg.nebula_color[2]*cfg.nebula_emissive,1.0),alpha_mode:AlphaMode::Add,unlit:true,..default()});
        let nmesh=msh.add(Mesh::from(Cuboid::new(cfg.nebula_voxel,cfg.nebula_voxel,cfg.nebula_voxel)));
        for pole in [1.0_f32,-1.0_f32] {
            for i in 0..cfg.nebula_count {
                let h1=ph(cfg.seed as f32+500.0+pole*20.0,i as f32);let h2=ph(cfg.seed as f32+501.0+pole*20.0,i as f32);
                let nv=cmd.spawn((Mesh3d(nmesh.clone()),MeshMaterial3d(nm.clone()),Transform::from_translation(Vec3::ZERO),Visibility::default(),NotShadowCaster,SgNebula{idx,pole,t:h1,perp:h2,seed:ph(h1,h2)})).id();
                cmd.entity(root).add_child(nv);
            }
        }
    }
    let[lr,lg,lb]=cfg.color_core;
    cmd.spawn((PointLight{intensity:cfg.light_intensity * 100.0,range:cfg.light_range,color:Color::srgb(lr,lg,lb),shadows_enabled:true,..default()},Transform::IDENTITY));
}

fn orbit_sg(time:Res<Time>,res:Res<SupergiantRes>,mut q:Query<(&mut Transform,&SgRoot)>){let t=time.elapsed_secs();for (mut tf,r) in &mut q{if let Some(cfg)=res.stars.get(r.idx){if cfg.orbit_distance>1.0{let a=t*cfg.orbit_speed;tf.translation.x=a.cos()*cfg.orbit_distance;tf.translation.z=a.sin()*cfg.orbit_distance;}}}}
fn animate_sg_body(time:Res<Time>,res:Res<SupergiantRes>,mut q:Query<(&mut Transform,&SgBody)>){let t=time.elapsed_secs();for (mut tf,b) in &mut q{if let Some(cfg)=res.stars.get(b.idx){tf.rotation=Quat::from_rotation_y(t*cfg.spin_speed);let p=1.0+(t*std::f32::consts::TAU/cfg.pulsation_period).sin()*cfg.pulsation_amplitude;tf.scale=Vec3::splat(p);}}}
fn animate_sg_convection(time:Res<Time>,res:Res<SupergiantRes>,mut q:Query<(&SgConv,&mut Transform,&MeshMaterial3d<StandardMaterial>)>,mut mats:ResMut<Assets<StandardMaterial>>){
    let t=time.elapsed_secs();
    for (gv,mut tf,mh) in &mut q{let Some(cfg)=res.stars.get(gv.idx) else{continue;};let cycle=(t*cfg.convection_speed+gv.phase).sin()*0.5+0.5;let drift=Vec3::new((t*cfg.convection_speed*0.7+gv.seed*11.0).sin()*18.0,(t*cfg.convection_speed*0.5+gv.seed*7.3).cos()*9.0,(t*cfg.convection_speed*0.9+gv.seed*5.1).sin()*18.0);tf.translation=snap(gv.origin+drift,cfg.convection_voxel);let[hr,hg,hb]=cfg.convection_hot;let[cr,cg,cb]=cfg.convection_cold;let[rr,rg,rb]=lc([cr,cg,cb],[hr,hg,hb],cycle);let ge=cfg.convection_emissive*(0.4+cycle*0.8);if let Some(m)=mats.get_mut(&mh.0){m.base_color=Color::srgb(rr,rg,rb);m.emissive=LinearRgba::new(rr*ge,rg*ge,rb*ge,1.0);}let s=0.5+cycle*0.8;tf.scale=Vec3::splat(s);}
}
fn animate_sg_wind(time:Res<Time>,res:Res<SupergiantRes>,mut q:Query<(&SgWind,&mut Transform,&mut Visibility)>){let t=time.elapsed_secs();for (wv,mut tf,mut vis) in &mut q{let Some(cfg)=res.stars.get(wv.idx) else{*vis=Visibility::Hidden;continue;};let local_t=((t*cfg.wind_speed/cfg.wind_radius+wv.t)%1.0).max(0.0);let dist=cfg.radius+local_t*cfg.wind_radius;let jit=Vec3::new((t*0.15+wv.seed*7.0).sin(),(t*0.2+wv.seed*5.0).cos(),(t*0.18+wv.seed*9.0).sin())*dist*0.03;tf.translation=snap(wv.dir*dist+jit,cfg.wind_voxel);let fade=(1.0-local_t*0.75).max(0.02);tf.scale=Vec3::splat(fade);*vis=Visibility::Visible;}}
fn animate_sg_ejecta(time:Res<Time>,res:Res<SupergiantRes>,mut q:Query<(&SgEjecta,&mut Transform,&mut Visibility)>){let t=time.elapsed_secs();for (ev,mut tf,mut vis) in &mut q{let Some(cfg)=res.stars.get(ev.idx) else{*vis=Visibility::Hidden;continue;};if !cfg.ejecta_enabled{*vis=Visibility::Hidden;continue;}let local_t=((t*cfg.wind_speed*0.5/cfg.ejecta_radius+ev.t)%1.0).max(0.0);let dist=cfg.radius+local_t*cfg.ejecta_radius;let jit=Vec3::new((t*0.1+ev.seed*6.0).sin(),(t*0.12+ev.seed*4.0).cos(),(t*0.11+ev.seed*8.0).sin())*dist*0.04;tf.translation=snap(ev.dir*dist+jit,cfg.ejecta_voxel);let fade=(1.0-local_t*0.85).max(0.02);let pulse=0.6+(t*0.6+ev.seed*std::f32::consts::TAU).sin().abs()*0.5;tf.scale=Vec3::splat(fade*pulse);*vis=Visibility::Visible;}}
fn animate_sg_corona(time:Res<Time>,res:Res<SupergiantRes>,mut q:Query<(&SgCorona,&mut Transform)>){let t=time.elapsed_secs();for (cv,mut tf) in &mut q{let Some(cfg)=res.stars.get(cv.idx) else{continue;};let pulse=1.0+(t*0.6+cv.seed*std::f32::consts::TAU).sin()*0.22;let r=cv.r*pulse;let th=cv.theta+t*0.03;tf.translation=snap(Vec3::new(r*cv.phi.sin()*th.cos(),r*cv.phi.cos(),r*cv.phi.sin()*th.sin()),cfg.corona_voxel);let life=1.0-((r-cfg.radius)/cfg.corona_radius.max(1.0)).clamp(0.0,1.0);tf.scale=Vec3::splat((0.3+life*0.9).max(0.02));}}
fn animate_bipolar_nebula(time:Res<Time>,res:Res<SupergiantRes>,mut q:Query<(&SgNebula,&mut Transform,&mut Visibility)>){let t=time.elapsed_secs();for (nv,mut tf,mut vis) in &mut q{let Some(cfg)=res.stars.get(nv.idx) else{*vis=Visibility::Hidden;continue;};if !cfg.bipolar_nebula{*vis=Visibility::Hidden;continue;}let local_t=((t*0.2+nv.t)%1.0).max(0.0);let cone=local_t*cfg.nebula_length;let cw=local_t*cfg.corona_radius*0.8;let pa=nv.perp*std::f32::consts::TAU+t*0.15;tf.translation=snap(Vec3::new(pa.cos()*cw,nv.pole*(cfg.radius+cone),pa.sin()*cw),cfg.nebula_voxel);let fade=(1.0-local_t*0.8).max(0.02);let pulse=0.5+(t*0.8+nv.seed*5.0).sin().abs()*0.6;tf.scale=Vec3::splat(fade*pulse);*vis=Visibility::Visible;}}
fn regenerate_sg(mut cmd:Commands,mut ev:EventReader<RegenerateSupergiant>,res:Res<SupergiantRes>,rq:Query<Entity,With<SgRoot>>,mut msh:ResMut<Assets<Mesh>>,mut mat:ResMut<Assets<StandardMaterial>>){let mut f=false;for _ in ev.read(){f=true;}if !f{return;}for e in &rq{cmd.entity(e).despawn_recursive();}for (i,cfg) in res.stars.iter().enumerate(){build_sg(&mut cmd,cfg,i,&mut msh,&mut mat);}}
