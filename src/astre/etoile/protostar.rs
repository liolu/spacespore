use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;
use crate::astre::{AstreLodRoot, ReloadAstre};

#[derive(Clone, Debug)]
pub struct ProtostarConfig {
    pub position:           Vec3,
    pub orbit_distance:     f32,
    pub orbit_speed:        f32,
    pub radius:             f32,
    pub voxel_size:         f32,
    pub resolution:         u32,
    // Température (0=froid/rouge, 1=chaud/blanc)
    pub temperature:        f32,
    // Couleurs
    pub color_core:         [f32; 3],
    pub color_envelope:     [f32; 3],
    pub emissive_core:      f32,
    pub emissive_envelope:  f32,
    // Enveloppe de gaz/poussière
    pub envelope_enabled:   bool,
    pub envelope_radius:    f32,
    pub envelope_count:     u32,
    pub envelope_voxel_size: f32,
    pub envelope_color:     [f32; 3],
    pub envelope_emissive:  f32,
    pub envelope_opacity:   f32,
    // Disque protoplanétaire
    pub disk_enabled:       bool,
    pub disk_inner:         f32,
    pub disk_outer:         f32,
    pub disk_thickness:     f32,
    pub disk_count:         u32,
    pub disk_voxel_size:    f32,
    pub disk_color_inner:   [f32; 3],
    pub disk_color_outer:   [f32; 3],
    pub disk_emissive:      f32,
    pub disk_speed:         f32,
    // Jets bipolaires de Herbig-Haro
    pub jet_enabled:        bool,
    pub jet_length:         f32,
    pub jet_width:          f32,
    pub jet_voxel_size:     f32,
    pub jet_count:          u32,
    pub jet_color:          [f32; 3],
    pub jet_emissive:       f32,
    pub jet_speed:          f32,
    pub jet_knot_count:     u32,    // nœuds de Herbig-Haro le long du jet
    // Accrétion (chute de matière sur la protoétoile)
    pub accretion_enabled:  bool,
    pub accretion_count:    u32,
    pub accretion_color:    [f32; 3],
    pub accretion_emissive: f32,
    pub accretion_speed:    f32,
    // Pulsations
    pub pulse_speed:        f32,
    pub pulse_amplitude:    f32,
    pub seed:               u32,
}

impl Default for ProtostarConfig {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO, orbit_distance: 0.0, orbit_speed: 0.0,
            radius: 60.0, voxel_size: 8.0, resolution: 16,
            temperature: 0.35,
            color_core: [1.0, 0.6, 0.2], color_envelope: [0.7, 0.35, 0.1],
            emissive_core: 6.0, emissive_envelope: 2.5,
            envelope_enabled: true, envelope_radius: 180.0, envelope_count: 500,
            envelope_voxel_size: 12.0, envelope_color: [0.45, 0.25, 0.1],
            envelope_emissive: 1.2, envelope_opacity: 0.5,
            disk_enabled: true, disk_inner: 70.0, disk_outer: 350.0,
            disk_thickness: 40.0, disk_count: 600, disk_voxel_size: 10.0,
            disk_color_inner: [0.9, 0.55, 0.15], disk_color_outer: [0.3, 0.18, 0.08],
            disk_emissive: 2.5, disk_speed: 0.08,
            jet_enabled: true, jet_length: 700.0, jet_width: 45.0,
            jet_voxel_size: 8.0, jet_count: 160, jet_color: [0.4, 0.75, 1.0],
            jet_emissive: 7.0, jet_speed: 1.4, jet_knot_count: 5,
            accretion_enabled: true, accretion_count: 80,
            accretion_color: [1.0, 0.65, 0.2], accretion_emissive: 4.0, accretion_speed: 0.6,
            pulse_speed: 0.25, pulse_amplitude: 0.08,
            seed: 17,
        }
    }
}

pub struct ProtostarPlugin;
impl Plugin for ProtostarPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ProtostarRes>()
           .add_event::<RegenerateProtostar>()
           .add_systems(Startup, spawn_protostars)
           .add_systems(Update, (orbit_protostars, animate_pulse, animate_envelope,
               animate_disk, animate_jets, animate_accretion, regenerate_protostars, reload_protostars).chain());
    }
}

#[derive(Resource)]
pub struct ProtostarRes { pub stars: Vec<ProtostarConfig> }
impl Default for ProtostarRes { fn default() -> Self { Self { stars: vec![
    ProtostarConfig { position: Vec3::new(0.0, 0.0, 7000.0), ..Default::default() },
] } } }
#[derive(Event)] pub struct RegenerateProtostar;

#[derive(Component)] pub struct ProtostarRoot   { pub idx: usize }
#[derive(Component)] pub struct ProtostarCore   { pub idx: usize }
#[derive(Component)] pub struct ProtoEnvelope   { pub idx: usize, pub theta: f32, pub phi: f32, pub r: f32, pub seed: f32 }
#[derive(Component)] pub struct ProtoDiskVoxel  { pub idx: usize, pub angle: f32, pub radius: f32, pub height: f32, pub seed: f32, pub ct: f32 }
#[derive(Component)] pub struct ProtoJetVoxel   { pub idx: usize, pub pole: f32, pub t: f32, pub knot: u32, pub perp: f32, pub seed: f32 }
#[derive(Component)] pub struct ProtoAccretion  { pub idx: usize, pub t: f32, pub dir: Vec3, pub seed: f32 }

fn ph(a:f32,b:f32)->f32{((a*12.9898+b*78.233).sin()*43758.5453).fract()}
fn snap(v:Vec3,g:f32)->Vec3{Vec3::new((v.x/g).round()*g,(v.y/g).round()*g,(v.z/g).round()*g)}
fn sph(lat:f32,lon:f32,r:f32)->Vec3{Vec3::new(r*lat.cos()*lon.cos(),r*lat.sin(),r*lat.cos()*lon.sin())}
fn lc(a:[f32;3],b:[f32;3],t:f32)->[f32;3]{let t=t.clamp(0.0,1.0);[a[0]+(b[0]-a[0])*t,a[1]+(b[1]-a[1])*t,a[2]+(b[2]-a[2])*t]}

fn spawn_protostars(mut cmd:Commands,res:Res<ProtostarRes>,mut msh:ResMut<Assets<Mesh>>,mut mat:ResMut<Assets<StandardMaterial>>) {
    for (i,cfg) in res.stars.iter().enumerate(){build_protostar(&mut cmd,cfg,i,&mut msh,&mut mat);}
}

fn build_protostar(cmd:&mut Commands,cfg:&ProtostarConfig,idx:usize,msh:&mut ResMut<Assets<Mesh>>,mat:&mut ResMut<Assets<StandardMaterial>>) {
    let pos=if cfg.orbit_distance>1.0{Vec3::new(cfg.orbit_distance,0.0,0.0)}else{cfg.position};
    let root=cmd.spawn((Transform::from_translation(pos),Visibility::default(),ProtostarRoot{idx},AstreLodRoot{cull_dist:6000.0, radius: cfg.radius, streamable: true, label: "Protostar"})).id();
    // Noyau voxelisé
    let core_e=cmd.spawn((Transform::IDENTITY,Visibility::default(),ProtostarCore{idx})).id();
    cmd.entity(root).add_child(core_e);
    let vs=cfg.voxel_size; let rs=cfg.resolution;
    let vstep=std::f32::consts::PI/rs as f32; let hstep=std::f32::consts::TAU/(rs*2) as f32;
    let smesh=msh.add(Mesh::from(Cuboid::new(vs,vs,vs)));
    let mut lat=-std::f32::consts::FRAC_PI_2;
    while lat<=std::f32::consts::FRAC_PI_2 {
        let mut lon=0.0_f32;
        while lon<std::f32::consts::TAU {
            let noise=ph(lat*6.0+cfg.seed as f32,lon*3.0)*0.15-0.07;
            let r=cfg.radius*(1.0+noise);
            let lt=(lat+std::f32::consts::FRAC_PI_2)/std::f32::consts::PI;
            let [cr,cg,cb]=lc(cfg.color_core,cfg.color_envelope,lt*0.5);
            let ce=cfg.emissive_core*(0.7+lt*0.6);
            let sm=mat.add(StandardMaterial{base_color:Color::srgb(cr,cg,cb),emissive:LinearRgba::new(cr*ce,cg*ce,cb*ce,1.0),unlit:true,..default()});
            let v=cmd.spawn((Mesh3d(smesh.clone()),MeshMaterial3d(sm),Transform::from_translation(snap(sph(lat,lon,r),vs)),NotShadowCaster)).id();
            cmd.entity(core_e).add_child(v);
            lon+=hstep;
        }
        lat+=vstep;
    }
    // Enveloppe
    if cfg.envelope_enabled {
        let [er,eg,eb]=cfg.envelope_color; let ee=cfg.envelope_emissive;
        let em=mat.add(StandardMaterial{base_color:Color::srgba(er,eg,eb,cfg.envelope_opacity),emissive:LinearRgba::new(er*ee,eg*ee,eb*ee,1.0),alpha_mode:AlphaMode::Add,unlit:true,..default()});
        let emesh=msh.add(Mesh::from(Cuboid::new(cfg.envelope_voxel_size,cfg.envelope_voxel_size,cfg.envelope_voxel_size)));
        for i in 0..cfg.envelope_count {
            let h1=ph(cfg.seed as f32+100.0,i as f32); let h2=ph(cfg.seed as f32+101.0,i as f32); let h3=ph(cfg.seed as f32+102.0,i as f32);
            let theta=h1*std::f32::consts::TAU; let phi=(h2*2.0-1.0).clamp(-1.0,1.0).acos();
            let r=cfg.radius+h3.sqrt()*(cfg.envelope_radius-cfg.radius);
            let ev=cmd.spawn((Mesh3d(emesh.clone()),MeshMaterial3d(em.clone()),Transform::from_translation(Vec3::ZERO),Visibility::default(),NotShadowCaster,ProtoEnvelope{idx,theta,phi,r,seed:ph(h1,h2)})).id();
            cmd.entity(root).add_child(ev);
        }
    }
    // Disque protoplanétaire
    if cfg.disk_enabled {
        let dmesh=msh.add(Mesh::from(Cuboid::new(cfg.disk_voxel_size,cfg.disk_voxel_size*0.35,cfg.disk_voxel_size)));
        for i in 0..cfg.disk_count {
            let h1=ph(cfg.seed as f32+200.0,i as f32); let h2=ph(cfg.seed as f32+201.0,i as f32); let h3=ph(cfg.seed as f32+202.0,i as f32); let h4=ph(cfg.seed as f32+203.0,i as f32);
            let angle=h1*std::f32::consts::TAU;
            let radius=cfg.disk_inner+h2.powf(0.55)*(cfg.disk_outer-cfg.disk_inner);
            let height=(h3*2.0-1.0)*cfg.disk_thickness*0.5*(1.0-(radius-cfg.disk_inner)/(cfg.disk_outer-cfg.disk_inner)*0.5);
            let ct=((radius-cfg.disk_inner)/(cfg.disk_outer-cfg.disk_inner)).clamp(0.0,1.0);
            let [dr,dg,db]=lc(cfg.disk_color_inner,cfg.disk_color_outer,ct);
            let de=cfg.disk_emissive*(1.0-ct*0.6);
            let dm=mat.add(StandardMaterial{base_color:Color::srgb(dr,dg,db),emissive:LinearRgba::new(dr*de,dg*de,db*de,1.0),alpha_mode:AlphaMode::Add,unlit:true,..default()});
            let pos=Vec3::new(angle.cos()*radius,height,angle.sin()*radius);
            let dv=cmd.spawn((Mesh3d(dmesh.clone()),MeshMaterial3d(dm),Transform::from_translation(snap(pos,cfg.disk_voxel_size)),Visibility::default(),NotShadowCaster,ProtoDiskVoxel{idx,angle,radius,height,seed:h4,ct})).id();
            cmd.entity(root).add_child(dv);
        }
    }
    // Jets HH
    if cfg.jet_enabled {
        let [jr,jg,jb]=cfg.jet_color; let je=cfg.jet_emissive;
        let jm=mat.add(StandardMaterial{base_color:Color::srgb(jr,jg,jb),emissive:LinearRgba::new(jr*je,jg*je,jb*je,1.0),alpha_mode:AlphaMode::Add,unlit:true,..default()});
        let jmesh=msh.add(Mesh::from(Cuboid::new(cfg.jet_voxel_size,cfg.jet_voxel_size,cfg.jet_voxel_size)));
        for pole in [1.0_f32,-1.0_f32] {
            for i in 0..cfg.jet_count {
                let h1=ph(cfg.seed as f32+300.0+pole*20.0,i as f32); let h2=ph(cfg.seed as f32+301.0+pole*20.0,i as f32);
                let knot=(h1*cfg.jet_knot_count as f32) as u32 % cfg.jet_knot_count.max(1);
                let jv=cmd.spawn((Mesh3d(jmesh.clone()),MeshMaterial3d(jm.clone()),Transform::from_translation(Vec3::ZERO).with_scale(Vec3::ZERO),Visibility::default(),NotShadowCaster,ProtoJetVoxel{idx,pole,t:h1,knot,perp:h2,seed:ph(h1,h2)})).id();
                cmd.entity(root).add_child(jv);
            }
        }
    }
    // Accrétion
    if cfg.accretion_enabled {
        let [acr,acg,acb]=cfg.accretion_color; let ace=cfg.accretion_emissive;
        let am=mat.add(StandardMaterial{base_color:Color::srgb(acr,acg,acb),emissive:LinearRgba::new(acr*ace,acg*ace,acb*ace,1.0),alpha_mode:AlphaMode::Add,unlit:true,..default()});
        let amesh=msh.add(Mesh::from(Cuboid::new(cfg.voxel_size*0.8,cfg.voxel_size*0.8,cfg.voxel_size*0.8)));
        for i in 0..cfg.accretion_count {
            let h1=ph(cfg.seed as f32+400.0,i as f32); let h2=ph(cfg.seed as f32+401.0,i as f32);
            let theta=h1*std::f32::consts::TAU; let phi=(h2*2.0-1.0).clamp(-1.0,1.0).acos();
            let dir=Vec3::new(phi.sin()*theta.cos(),phi.cos(),phi.sin()*theta.sin()).normalize();
            let av=cmd.spawn((Mesh3d(amesh.clone()),MeshMaterial3d(am.clone()),Transform::from_translation(Vec3::ZERO),Visibility::default(),NotShadowCaster,ProtoAccretion{idx,t:ph(h1,h2),dir,seed:ph(h2,h1)})).id();
            cmd.entity(root).add_child(av);
        }
    }
}

fn orbit_protostars(time:Res<Time>,res:Res<ProtostarRes>,mut q:Query<(&mut Transform,&ProtostarRoot)>) {
    let t=time.elapsed_secs();
    for (mut tf,r) in &mut q { if let Some(cfg)=res.stars.get(r.idx){if cfg.orbit_distance>1.0{let a=t*cfg.orbit_speed;tf.translation.x=a.cos()*cfg.orbit_distance;tf.translation.z=a.sin()*cfg.orbit_distance;}} }
}
fn animate_pulse(time:Res<Time>,res:Res<ProtostarRes>,mut q:Query<(&mut Transform,&ProtostarCore)>) {
    let t=time.elapsed_secs();
    for (mut tf,c) in &mut q { if let Some(cfg)=res.stars.get(c.idx){let p=1.0+(t*cfg.pulse_speed).sin()*cfg.pulse_amplitude; tf.scale=Vec3::splat(p);} }
}
fn animate_envelope(time:Res<Time>,res:Res<ProtostarRes>,mut q:Query<(&ProtoEnvelope,&mut Transform)>) {
    let t=time.elapsed_secs();
    for (ev,mut tf) in &mut q {
        let Some(cfg)=res.stars.get(ev.idx) else{continue;};
        let pulse=1.0+(t*cfg.pulse_speed*0.6+ev.seed*std::f32::consts::TAU).sin()*0.18;
        let r=ev.r*pulse; let th=ev.theta+t*0.04*(1.0+ev.seed*0.3);
        tf.translation=snap(Vec3::new(r*ev.phi.sin()*th.cos(),r*ev.phi.cos(),r*ev.phi.sin()*th.sin()),cfg.envelope_voxel_size);
        let s=0.4+(t*0.9+ev.seed*6.0).sin().abs()*0.7; tf.scale=Vec3::splat(s.max(0.05));
    }
}
fn animate_disk(time:Res<Time>,res:Res<ProtostarRes>,mut q:Query<(&ProtoDiskVoxel,&mut Transform)>) {
    let t=time.elapsed_secs();
    for (dv,mut tf) in &mut q {
        let Some(cfg)=res.stars.get(dv.idx) else{continue;};
        let kepler=cfg.disk_speed*(cfg.disk_inner/dv.radius.max(1.0)).sqrt();
        let angle=dv.angle+t*kepler;
        let turb=(t*1.4+dv.seed*std::f32::consts::TAU).sin()*5.0*(1.0-dv.ct);
        tf.translation=snap(Vec3::new(angle.cos()*dv.radius,dv.height+turb,angle.sin()*dv.radius),cfg.disk_voxel_size);
        let s=(0.5+dv.ct*0.7)*(1.0+(t*2.0+dv.seed*5.0).sin()*0.12); tf.scale=Vec3::splat(s.max(0.05));
    }
}
fn animate_jets(time:Res<Time>,res:Res<ProtostarRes>,mut q:Query<(&ProtoJetVoxel,&mut Transform,&mut Visibility)>) {
    let t=time.elapsed_secs();
    for (jv,mut tf,mut vis) in &mut q {
        let Some(cfg)=res.stars.get(jv.idx) else{*vis=Visibility::Hidden;continue;};
        if !cfg.jet_enabled{*vis=Visibility::Hidden;continue;}
        // Position le long du jet avec nœuds HH
        let local_t=((t*cfg.jet_speed*0.3+jv.t)%1.0).max(0.0);
        let knot_t=(local_t*cfg.jet_knot_count as f32)%1.0;
        // Les nœuds sont des zones de choc — brillance accrue
        let knot_bright=(knot_t*std::f32::consts::TAU).sin().abs();
        let dist=cfg.radius+local_t*cfg.jet_length;
        let cone=local_t*cfg.jet_width*0.5;
        let pa=jv.perp*std::f32::consts::TAU+t*0.4;
        tf.translation=snap(Vec3::new(pa.cos()*cone,jv.pole*dist,pa.sin()*cone),cfg.jet_voxel_size);
        let fade=(1.0-local_t*0.8).max(0.05)*(0.5+knot_bright*0.6);
        tf.scale=Vec3::splat(fade.max(0.02)); *vis=Visibility::Visible;
    }
}
fn animate_accretion(time:Res<Time>,res:Res<ProtostarRes>,mut q:Query<(&ProtoAccretion,&mut Transform,&mut Visibility)>) {
    let t=time.elapsed_secs();
    for (av,mut tf,mut vis) in &mut q {
        let Some(cfg)=res.stars.get(av.idx) else{*vis=Visibility::Hidden;continue;};
        if !cfg.accretion_enabled{*vis=Visibility::Hidden;continue;}
        let local_t=((t*cfg.accretion_speed+av.t)%1.0).max(0.0);
        let from_r=cfg.disk_inner+av.seed*(cfg.disk_outer-cfg.disk_inner);
        let to_r=cfg.radius+cfg.voxel_size*0.5;
        let r=from_r+(to_r-from_r)*local_t;
        tf.translation=snap(av.dir*r,cfg.voxel_size*0.8);
        let fade=(1.0-local_t*0.7).max(0.1); tf.scale=Vec3::splat(fade);
        *vis=Visibility::Visible;
    }
}
fn reload_protostars(
    mut cmd: Commands,
    mut events: EventReader<ReloadAstre>,
    res: Res<ProtostarRes>,
    roots: Query<(Entity, &ProtostarRoot)>,
    mut msh: ResMut<Assets<Mesh>>,
    mut mat: ResMut<Assets<StandardMaterial>>,
) {
    for ev in events.read() {
        let Ok((entity, root)) = roots.get(ev.0) else { continue };
        let idx = root.idx;
        let Some(cfg) = res.stars.get(idx) else { continue };
        cmd.entity(entity).despawn_recursive();
        build_protostar(&mut cmd, cfg, idx, &mut msh, &mut mat);
    }
}

fn regenerate_protostars(mut cmd:Commands,mut ev:EventReader<RegenerateProtostar>,res:Res<ProtostarRes>,rq:Query<Entity,With<ProtostarRoot>>,mut msh:ResMut<Assets<Mesh>>,mut mat:ResMut<Assets<StandardMaterial>>) {
    let mut f=false;for _ in ev.read(){f=true;}if !f{return;}
    for e in &rq{cmd.entity(e).despawn_recursive();}
    for (i,cfg) in res.stars.iter().enumerate(){build_protostar(&mut cmd,cfg,i,&mut msh,&mut mat);}
}
