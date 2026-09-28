use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;
use crate::astre::{AstreLodRoot, ReloadAstre};

// ── Config ────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct StarConfig {
    pub position:           Vec3,
    pub orbit_distance:     f32,
    pub orbit_speed:        f32,
    pub spin_speed:         f32,
    pub radius:             f32,
    pub surface_voxel_size: f32,
    pub surface_resolution: u32,
    // Couleurs
    pub color_pole:         [f32; 3],
    pub color_equator:      [f32; 3],
    pub surface_emissive:   f32,
    // Taches solaires
    pub sunspot_count:      u32,
    pub sunspot_color:      [f32; 3],
    pub sunspot_emissive:   f32,
    pub sunspot_speed:      f32,
    // Granulation convective (cellules de Bénard)
    pub granule_enabled:    bool,
    pub granule_count:      u32,
    pub granule_voxel_size: f32,
    pub granule_color_hot:  [f32; 3],
    pub granule_color_cold: [f32; 3],
    pub granule_emissive:   f32,
    pub granule_speed:      f32,
    // Couronne
    pub corona_enabled:     bool,
    pub corona_radius:      f32,
    pub corona_voxel_size:  f32,
    pub corona_count:       u32,
    pub corona_color:       [f32; 3],
    pub corona_emissive:    f32,
    pub corona_pulse_speed: f32,
    // Éruptions (flares)
    pub flare_count:        u32,
    pub flare_samples:      u32,
    pub flare_height:       f32,
    pub flare_color:        [f32; 3],
    pub flare_emissive:     f32,
    pub flare_speed:        f32,
    // Lumière
    pub light_intensity:    f32,
    pub light_range:        f32,
    pub light_color:        [f32; 3],
    pub seed:               u32,
}

impl Default for StarConfig {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO, orbit_distance: 0.0, orbit_speed: 0.0, spin_speed: 0.6,
            radius: 120.0, surface_voxel_size: 10.0, surface_resolution: 24,
            color_pole: [1.0, 0.95, 0.8], color_equator: [1.0, 0.75, 0.3],
            surface_emissive: 8.0,
            sunspot_count: 12, sunspot_color: [0.35, 0.2, 0.05], sunspot_emissive: 0.5, sunspot_speed: 0.05,
            granule_enabled: true, granule_count: 200, granule_voxel_size: 8.0,
            granule_color_hot: [1.0, 0.9, 0.6], granule_color_cold: [0.8, 0.5, 0.15],
            granule_emissive: 5.0, granule_speed: 0.4,
            corona_enabled: true, corona_radius: 60.0, corona_voxel_size: 8.0,
            corona_count: 300, corona_color: [1.0, 0.85, 0.5], corona_emissive: 4.0, corona_pulse_speed: 0.8,
            flare_count: 5, flare_samples: 48, flare_height: 80.0,
            flare_color: [1.0, 0.7, 0.2], flare_emissive: 12.0, flare_speed: 1.2,
            light_intensity: 4_000_000.0, light_range: 8000.0, light_color: [1.0, 0.95, 0.8],
            seed: 7,
        }
    }
}

// ── Plugin ────────────────────────────────────────────────────────────────

pub const SPAWN_PROPS: crate::system_gen::SpawnProps = crate::system_gen::SpawnProps {
    category:  crate::system_gen::AstreCategory::Star,
    weight:    0.15,
    orbit_min: 0.0,
    orbit_max: 0.0,
};

pub fn generate_random(rng: &mut crate::system_gen::SeedRng, stars: &mut Vec<StarConfig>) {
    stars.push(StarConfig {
        position: Vec3::ZERO,
        orbit_distance: 0.0,
        seed: rng.u32(),
        ..Default::default()
    });
}

pub struct StarPlugin;
impl Plugin for StarPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StarRes>()
           .add_event::<RegenerateStar>()
           .add_systems(Startup, spawn_stars)
           .add_systems(Update, (orbit_stars, spin_stars, animate_granules,
               animate_corona, animate_flares, animate_sunspots, regenerate_stars, reload_stars).chain());
    }
}

#[derive(Resource)]
pub struct StarRes { pub stars: Vec<StarConfig> }
impl Default for StarRes { fn default() -> Self { Self { stars: vec![
    StarConfig { position: Vec3::new(0.0, 0.0, 5000.0), seed: 7, ..Default::default() },
] } } }

#[derive(Event)] pub struct RegenerateStar;

#[derive(Component)] pub struct StarRoot       { pub idx: usize }
#[derive(Component)] pub struct StarBody        { pub idx: usize }
#[derive(Component)] pub struct StarSurface     { pub idx: usize, pub lat: f32, pub lon: f32 }
#[derive(Component)] pub struct StarSunspot     { pub idx: usize, pub seed: f32, pub lat: f32, pub lon: f32 }
#[derive(Component)] pub struct StarGranule     { pub idx: usize, pub origin: Vec3, pub seed: f32, pub phase: f32 }
#[derive(Component)] pub struct StarCorona      { pub idx: usize, pub theta: f32, pub phi: f32, pub r: f32, pub seed: f32 }
#[derive(Component)] pub struct StarFlare       { pub idx: usize, pub flare_idx: u32, pub sample_idx: u32 }

// ── Helpers ───────────────────────────────────────────────────────────────

fn ph(a: f32, b: f32) -> f32 { ((a*12.9898+b*78.233).sin()*43758.5453).fract() }
fn snap(v: Vec3, g: f32) -> Vec3 { Vec3::new((v.x/g).round()*g,(v.y/g).round()*g,(v.z/g).round()*g) }
fn sph(lat: f32, lon: f32, r: f32) -> Vec3 { Vec3::new(r*lat.cos()*lon.cos(),r*lat.sin(),r*lat.cos()*lon.sin()) }
fn lc(a:[f32;3],b:[f32;3],t:f32)->[f32;3]{let t=t.clamp(0.0,1.0);[a[0]+(b[0]-a[0])*t,a[1]+(b[1]-a[1])*t,a[2]+(b[2]-a[2])*t]}

// ── Spawn ─────────────────────────────────────────────────────────────────

fn spawn_stars(mut cmd: Commands, res: Res<StarRes>, mut msh: ResMut<Assets<Mesh>>, mut mat: ResMut<Assets<StandardMaterial>>) {
    for (i,cfg) in res.stars.iter().enumerate() { build_star(&mut cmd,cfg,i,&mut msh,&mut mat); }
}

fn build_star(cmd:&mut Commands,cfg:&StarConfig,idx:usize,msh:&mut ResMut<Assets<Mesh>>,mat:&mut ResMut<Assets<StandardMaterial>>) {
    let pos = if cfg.orbit_distance>1.0{Vec3::new(cfg.orbit_distance,0.0,0.0)}else{cfg.position};
    let root=cmd.spawn((Transform::from_translation(pos),Visibility::default(),StarRoot{idx},AstreLodRoot{cull_dist:5000.0, radius: cfg.radius, streamable: true, label: "VoxelStar"})).id();
    let body=cmd.spawn((Transform::IDENTITY,Visibility::default(),StarBody{idx})).id();
    cmd.entity(root).add_child(body);

    // Surface
    let vs=cfg.surface_voxel_size;
    let smesh=msh.add(Mesh::from(Cuboid::new(vs,vs,vs)));
    let res_s=cfg.surface_resolution;
    let vstep=std::f32::consts::PI/res_s as f32;
    let hstep=std::f32::consts::TAU/(res_s*2) as f32;
    let mut lat=-std::f32::consts::FRAC_PI_2;
    while lat<=std::f32::consts::FRAC_PI_2 {
        let mut lon=0.0_f32;
        while lon<std::f32::consts::TAU {
            let t=(lat+std::f32::consts::FRAC_PI_2)/std::f32::consts::PI;
            let noise=ph(lat*8.0+cfg.seed as f32, lon*4.0)*0.08-0.04;
            let r=cfg.radius*(1.0+noise);
            let [sr,sg,sb]=lc(cfg.color_equator,cfg.color_pole,t);
            let se=cfg.surface_emissive;
            let smat=mat.add(StandardMaterial{base_color:Color::srgb(sr,sg,sb),emissive:LinearRgba::new(sr*se,sg*se,sb*se,1.0),unlit:true,..default()});
            let v=cmd.spawn((Mesh3d(smesh.clone()),MeshMaterial3d(smat),Transform::from_translation(snap(sph(lat,lon,r),vs)),NotShadowCaster,StarSurface{idx,lat,lon})).id();
            cmd.entity(body).add_child(v);
            lon+=hstep;
        }
        lat+=vstep;
    }

    // Taches
    let sp_mesh=msh.add(Mesh::from(Cuboid::new(vs*1.3,vs*0.4,vs*1.3)));
    let [spr,spg,spb]=cfg.sunspot_color; let spe=cfg.sunspot_emissive;
    let sp_mat=mat.add(StandardMaterial{base_color:Color::srgb(spr,spg,spb),emissive:LinearRgba::new(spr*spe,spg*spe,spb*spe,1.0),unlit:true,..default()});
    for i in 0..cfg.sunspot_count {
        let h1=ph(cfg.seed as f32+i as f32,1.0); let h2=ph(cfg.seed as f32+i as f32,2.0);
        let lat=(h1*2.0-1.0).clamp(-1.0,1.0).asin()*0.6;
        let lon=h2*std::f32::consts::TAU;
        let pos=snap(sph(lat,lon,cfg.radius+vs*0.1),vs);
        let sp=cmd.spawn((Mesh3d(sp_mesh.clone()),MeshMaterial3d(sp_mat.clone()),Transform::from_translation(pos),NotShadowCaster,StarSunspot{idx,seed:ph(h1,h2),lat,lon})).id();
        cmd.entity(body).add_child(sp);
    }

    // Granules convectives
    if cfg.granule_enabled {
        let gm=msh.add(Mesh::from(Cuboid::new(cfg.granule_voxel_size,cfg.granule_voxel_size,cfg.granule_voxel_size)));
        for i in 0..cfg.granule_count {
            let h1=ph(cfg.seed as f32+200.0,i as f32); let h2=ph(cfg.seed as f32+201.0,i as f32); let h3=ph(cfg.seed as f32+202.0,i as f32);
            let theta=h1*std::f32::consts::TAU; let phi=(h2*2.0-1.0).clamp(-1.0,1.0).acos();
            let r=cfg.radius+h3*cfg.granule_voxel_size*0.5;
            let o=Vec3::new(r*phi.sin()*theta.cos(),r*phi.cos(),r*phi.sin()*theta.sin());
            let [gr,gg,gb]=cfg.granule_color_hot; let ge=cfg.granule_emissive;
            let gmat=mat.add(StandardMaterial{base_color:Color::srgb(gr,gg,gb),emissive:LinearRgba::new(gr*ge,gg*ge,gb*ge,1.0),alpha_mode:AlphaMode::Add,unlit:true,..default()});
            let gv=cmd.spawn((Mesh3d(gm.clone()),MeshMaterial3d(gmat),Transform::from_translation(snap(o,cfg.granule_voxel_size)),Visibility::default(),NotShadowCaster,StarGranule{idx,origin:snap(o,cfg.granule_voxel_size),seed:ph(h1,h2),phase:h3*std::f32::consts::TAU})).id();
            cmd.entity(root).add_child(gv);
        }
    }

    // Couronne
    if cfg.corona_enabled {
        let [cr,cg,cb]=cfg.corona_color; let ce=cfg.corona_emissive;
        let cmat=mat.add(StandardMaterial{base_color:Color::srgb(cr,cg,cb),emissive:LinearRgba::new(cr*ce,cg*ce,cb*ce,1.0),alpha_mode:AlphaMode::Add,unlit:true,..default()});
        let cm=msh.add(Mesh::from(Cuboid::new(cfg.corona_voxel_size,cfg.corona_voxel_size,cfg.corona_voxel_size)));
        for i in 0..cfg.corona_count {
            let h1=ph(cfg.seed as f32+300.0,i as f32); let h2=ph(cfg.seed as f32+301.0,i as f32); let h3=ph(cfg.seed as f32+302.0,i as f32);
            let theta=h1*std::f32::consts::TAU; let phi=(h2*2.0-1.0).clamp(-1.0,1.0).acos();
            let r=cfg.radius+h3.sqrt()*cfg.corona_radius;
            let cv=cmd.spawn((Mesh3d(cm.clone()),MeshMaterial3d(cmat.clone()),Transform::from_translation(Vec3::ZERO),Visibility::default(),NotShadowCaster,StarCorona{idx,theta,phi,r,seed:ph(h1,h3)})).id();
            cmd.entity(root).add_child(cv);
        }
    }

    // Flares
    let fm=msh.add(Mesh::from(Cuboid::new(6.0,6.0,6.0)));
    let [fr,fg,fb]=cfg.flare_color; let fe=cfg.flare_emissive;
    let fmat=mat.add(StandardMaterial{base_color:Color::srgb(fr,fg,fb),emissive:LinearRgba::new(fr*fe,fg*fe,fb*fe,1.0),alpha_mode:AlphaMode::Add,unlit:true,..default()});
    for fi in 0..cfg.flare_count {
        for si in 0..cfg.flare_samples {
            cmd.spawn((Mesh3d(fm.clone()),MeshMaterial3d(fmat.clone()),Transform::from_translation(Vec3::ZERO).with_scale(Vec3::ZERO),Visibility::default(),NotShadowCaster,StarFlare{idx,flare_idx:fi,sample_idx:si}));
        }
    }

    // Lumière
    let [lr,lg,lb]=cfg.light_color;
    cmd.spawn((PointLight{intensity:cfg.light_intensity * 100.0,range:cfg.light_range,color:Color::srgb(lr,lg,lb),shadows_enabled:true,..default()},Transform::IDENTITY));
}

// ── Systèmes ──────────────────────────────────────────────────────────────

fn orbit_stars(time:Res<Time>,res:Res<StarRes>,mut q:Query<(&mut Transform,&StarRoot)>) {
    let t=time.elapsed_secs();
    for (mut tf,r) in &mut q { if let Some(cfg)=res.stars.get(r.idx) { if cfg.orbit_distance>1.0 { let a=t*cfg.orbit_speed; tf.translation.x=a.cos()*cfg.orbit_distance; tf.translation.z=a.sin()*cfg.orbit_distance; } } }
}
fn spin_stars(time:Res<Time>,res:Res<StarRes>,mut q:Query<(&mut Transform,&StarBody)>) {
    let t=time.elapsed_secs();
    for (mut tf,b) in &mut q { if let Some(cfg)=res.stars.get(b.idx) { tf.rotation=Quat::from_rotation_y(t*cfg.spin_speed); } }
}

fn animate_sunspots(time:Res<Time>,res:Res<StarRes>,mut q:Query<(&StarSunspot,&mut Transform)>) {
    let t=time.elapsed_secs();
    for (sp,mut tf) in &mut q {
        let Some(cfg)=res.stars.get(sp.idx) else { continue; };
        let lon=sp.lon+t*cfg.sunspot_speed*(1.0-sp.lat.abs()/std::f32::consts::FRAC_PI_2*0.3);
        let pulse=1.0+(t*1.2+sp.seed*6.28).sin()*0.08;
        let r=cfg.radius+cfg.surface_voxel_size*0.05;
        tf.translation=snap(sph(sp.lat,lon,r),cfg.surface_voxel_size);
        tf.scale=Vec3::splat(pulse);
    }
}

fn animate_granules(time:Res<Time>,res:Res<StarRes>,mut q:Query<(&StarGranule,&mut Transform,&MeshMaterial3d<StandardMaterial>)>,mut mats:ResMut<Assets<StandardMaterial>>) {
    let t=time.elapsed_secs();
    for (gv,mut tf,mh) in &mut q {
        let Some(cfg)=res.stars.get(gv.idx) else { continue; };
        let cycle=(t*cfg.granule_speed+gv.phase).sin()*0.5+0.5;
        let drift=Vec3::new((t*cfg.granule_speed*0.7+gv.seed*11.0).sin()*4.0,(t*cfg.granule_speed*0.5+gv.seed*7.3).cos()*2.0,(t*cfg.granule_speed*0.9+gv.seed*5.1).sin()*4.0);
        tf.translation=snap(gv.origin+drift,cfg.granule_voxel_size);
        let [hr,hg,hb]=cfg.granule_color_hot; let [cr,cg,cb]=cfg.granule_color_cold;
        let [rr,rg,rb]=lc([cr,cg,cb],[hr,hg,hb],cycle);
        let ge=cfg.granule_emissive*(0.5+cycle*0.7);
        if let Some(m)=mats.get_mut(&mh.0){m.base_color=Color::srgb(rr,rg,rb);m.emissive=LinearRgba::new(rr*ge,rg*ge,rb*ge,1.0);}
        let scale=0.7+cycle*0.5; tf.scale=Vec3::splat(scale);
    }
}

fn animate_corona(time:Res<Time>,res:Res<StarRes>,mut q:Query<(&StarCorona,&mut Transform)>) {
    let t=time.elapsed_secs();
    for (cv,mut tf) in &mut q {
        let Some(cfg)=res.stars.get(cv.idx) else { continue; };
        let pulse=1.0+(t*cfg.corona_pulse_speed+cv.seed*std::f32::consts::TAU).sin()*0.25;
        let r=cv.r*pulse;
        let th=cv.theta+t*0.07;
        tf.translation=snap(Vec3::new(r*cv.phi.sin()*th.cos(),r*cv.phi.cos(),r*cv.phi.sin()*th.sin()),cfg.corona_voxel_size);
        let s=0.4+(t*cfg.corona_pulse_speed*1.3+cv.seed*7.0).sin().abs()*0.7;
        tf.scale=Vec3::splat(s.max(0.05));
    }
}

fn animate_flares(time:Res<Time>,res:Res<StarRes>,mut q:Query<(&StarFlare,&mut Transform,&mut Visibility)>) {
    let t=time.elapsed_secs();
    for (fv,mut tf,mut vis) in &mut q {
        let Some(cfg)=res.stars.get(fv.idx) else { *vis=Visibility::Hidden; continue; };
        let base=cfg.seed as f32*7.3+fv.flare_idx as f32*37.1;
        let rise=2.0+ph(base,1.0)*3.0; let hold=0.5+ph(base,2.0)*1.5; let fall=2.0+ph(base,3.0)*3.0; let pause=1.0+ph(base,4.0)*4.0;
        let total=rise+hold+fall+pause; let off=ph(base,5.0)*total;
        let lt=((t*cfg.flare_speed+off)%total).max(0.0);
        let life=if lt<rise{lt/rise}else if lt<rise+hold{1.0}else if lt<rise+hold+fall{1.0-(lt-rise-hold)/fall}else{0.0};
        if life<0.01{*vis=Visibility::Hidden;continue;}
        let frac=fv.sample_idx as f32/cfg.flare_samples as f32;
        if frac>life{*vis=Visibility::Hidden;continue;}
        let h1=ph(base,fv.sample_idx as f32*0.7); let h2=ph(base,fv.sample_idx as f32*1.3);
        let theta=h1*std::f32::consts::TAU; let phi=(h2*2.0-1.0).clamp(-1.0,1.0).acos();
        let dir=Vec3::new(phi.sin()*theta.cos(),phi.cos(),phi.sin()*theta.sin()).normalize();
        let height=cfg.flare_height*life;
        let perp=if dir.y.abs()>0.9{dir.cross(Vec3::X).normalize()}else{dir.cross(Vec3::Y).normalize()};
        let spread=15.0*0.5; let sd=dir+perp*spread/cfg.radius.max(1.0); let ed=dir-perp*spread/cfg.radius.max(1.0);
        let s=dir*(cfg.radius+height); let e=sd.normalize()*cfg.radius; let c=ed.normalize()*cfg.radius;
        let inv=1.0-frac; let pt=e*inv*inv+s*2.0*inv*frac+c*frac*frac;
        tf.translation=snap(pt,6.0); tf.scale=Vec3::splat((frac).max(0.05));
        *vis=Visibility::Visible;
    }
}

fn reload_stars(
    mut cmd: Commands,
    mut events: EventReader<ReloadAstre>,
    res: Res<StarRes>,
    roots: Query<(Entity, &StarRoot)>,
    flares: Query<(Entity, &StarFlare)>,
    mut msh: ResMut<Assets<Mesh>>,
    mut mat: ResMut<Assets<StandardMaterial>>,
) {
    for ev in events.read() {
        let Ok((entity, root)) = roots.get(ev.0) else { continue };
        let idx = root.idx;
        let Some(cfg) = res.stars.get(idx) else { continue };
        for (fe, fv) in &flares { if fv.idx == idx { cmd.entity(fe).despawn_recursive(); } }
        cmd.entity(entity).despawn_recursive();
        build_star(&mut cmd, cfg, idx, &mut msh, &mut mat);
    }
}

fn regenerate_stars(mut cmd:Commands,mut ev:EventReader<RegenerateStar>,res:Res<StarRes>,rq:Query<Entity,With<StarRoot>>,mut msh:ResMut<Assets<Mesh>>,mut mat:ResMut<Assets<StandardMaterial>>) {
    let mut f=false; for _ in ev.read(){f=true;} if !f{return;}
    for e in &rq{cmd.entity(e).despawn_recursive();}
    for (i,cfg) in res.stars.iter().enumerate(){build_star(&mut cmd,cfg,i,&mut msh,&mut mat);}
}
