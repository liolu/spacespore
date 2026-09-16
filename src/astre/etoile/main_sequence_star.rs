use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;

/// Classes spectrales O B A F G K M
#[derive(Clone, Debug, PartialEq)]
pub enum SpectralClass { O, B, A, F, G, K, M }

impl SpectralClass {
    pub fn color(&self) -> [f32; 3] {
        match self {
            Self::O => [0.6, 0.7, 1.0],
            Self::B => [0.7, 0.8, 1.0],
            Self::A => [0.9, 0.92, 1.0],
            Self::F => [1.0, 0.98, 0.85],
            Self::G => [1.0, 0.92, 0.65],
            Self::K => [1.0, 0.75, 0.35],
            Self::M => [1.0, 0.4, 0.1],
        }
    }
    pub fn emissive(&self) -> f32 {
        match self { Self::O=>22.0, Self::B=>18.0, Self::A=>14.0, Self::F=>10.0, Self::G=>8.0, Self::K=>6.0, Self::M=>4.0 }
    }
    pub fn radius_factor(&self) -> f32 {
        match self { Self::O=>7.0, Self::B=>4.5, Self::A=>2.5, Self::F=>1.4, Self::G=>1.0, Self::K=>0.75, Self::M=>0.4 }
    }
    pub fn light_intensity(&self) -> f32 {
        match self { Self::O=>50_000_000.0, Self::B=>20_000_000.0, Self::A=>8_000_000.0, Self::F=>5_000_000.0, Self::G=>3_000_000.0, Self::K=>1_500_000.0, Self::M=>600_000.0 }
    }
}

#[derive(Clone, Debug)]
pub struct MainSequenceConfig {
    pub position:           Vec3,
    pub orbit_distance:     f32,
    pub orbit_speed:        f32,
    pub spin_speed:         f32,
    pub spectral_class:     SpectralClass,
    pub base_radius:        f32,   // multiplié par spectral_class.radius_factor()
    pub voxel_size:         f32,
    pub resolution:         u32,
    // Zones différentiées
    pub photosphere_color:  [f32; 3],   // override ou [0,0,0] = auto
    pub chromosphere_enabled: bool,
    pub chromosphere_height:  f32,
    pub chromosphere_count:   u32,
    pub chromosphere_color:   [f32; 3],
    pub chromosphere_emissive: f32,
    // Granulation convective
    pub granule_enabled:    bool,
    pub granule_count:      u32,
    pub granule_voxel:      f32,
    pub granule_speed:      f32,
    // Zone de convection visible (arcs)
    pub convection_arcs:    bool,
    pub arc_count:          u32,
    pub arc_samples:        u32,
    pub arc_height:         f32,
    pub arc_color:          [f32; 3],
    pub arc_emissive:       f32,
    pub arc_speed:          f32,
    // Couronne + vent solaire
    pub corona_radius:      f32,
    pub corona_count:       u32,
    pub corona_voxel:       f32,
    pub corona_color:       [f32; 3],
    pub corona_emissive:    f32,
    // Vent solaire (particules sortantes)
    pub wind_enabled:       bool,
    pub wind_count:         u32,
    pub wind_voxel:         f32,
    pub wind_color:         [f32; 3],
    pub wind_emissive:      f32,
    pub wind_speed:         f32,
    pub wind_max_radius:    f32,
    // Taches
    pub sunspot_count:      u32,
    pub sunspot_color:      [f32; 3],
    pub sunspot_speed:      f32,
    pub seed:               u32,
}

impl Default for MainSequenceConfig {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO, orbit_distance: 0.0, orbit_speed: 0.0, spin_speed: 0.5,
            spectral_class: SpectralClass::G, base_radius: 100.0,
            voxel_size: 9.0, resolution: 22,
            photosphere_color: [0.0,0.0,0.0],
            chromosphere_enabled: true, chromosphere_height: 15.0, chromosphere_count: 250,
            chromosphere_color: [1.0, 0.5, 0.15], chromosphere_emissive: 5.0,
            granule_enabled: true, granule_count: 220, granule_voxel: 8.0, granule_speed: 0.35,
            convection_arcs: true, arc_count: 8, arc_samples: 50,
            arc_height: 60.0, arc_color: [1.0, 0.65, 0.2], arc_emissive: 9.0, arc_speed: 0.9,
            corona_radius: 80.0, corona_count: 350, corona_voxel: 9.0,
            corona_color: [1.0, 0.88, 0.55], corona_emissive: 5.0,
            wind_enabled: true, wind_count: 200, wind_voxel: 7.0,
            wind_color: [0.9, 0.85, 0.7], wind_emissive: 1.5,
            wind_speed: 60.0, wind_max_radius: 600.0,
            sunspot_count: 10, sunspot_color: [0.3, 0.18, 0.05], sunspot_speed: 0.06,
            seed: 9,
        }
    }
}

pub struct MainSequencePlugin;
impl Plugin for MainSequencePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MainSequenceRes>()
           .add_event::<RegenerateMainSequence>()
           .add_systems(Startup, spawn_ms)
           .add_systems(Update, (orbit_ms, spin_ms, animate_ms_granules, animate_chromosphere,
               animate_corona_ms, animate_convection_arcs, animate_solar_wind,
               animate_sunspots_ms, regenerate_ms).chain());
    }
}

#[derive(Resource)]
pub struct MainSequenceRes { pub stars: Vec<MainSequenceConfig> }
impl Default for MainSequenceRes { fn default() -> Self { Self { stars: vec![MainSequenceConfig::default()] } } }
#[derive(Event)] pub struct RegenerateMainSequence;

#[derive(Component)] pub struct MsRoot      { pub idx: usize }
#[derive(Component)] pub struct MsBody      { pub idx: usize }
#[derive(Component)] pub struct MsSurf      { pub idx: usize, pub lat: f32, pub lon: f32 }
#[derive(Component)] pub struct MsChromos   { pub idx: usize, pub theta: f32, pub phi: f32, pub r: f32, pub seed: f32 }
#[derive(Component)] pub struct MsGranule   { pub idx: usize, pub origin: Vec3, pub seed: f32, pub phase: f32 }
#[derive(Component)] pub struct MsCorona    { pub idx: usize, pub theta: f32, pub phi: f32, pub r: f32, pub seed: f32 }
#[derive(Component)] pub struct MsArc       { pub idx: usize, pub ai: u32, pub si: u32 }
#[derive(Component)] pub struct SolarWind   { pub idx: usize, pub dir: Vec3, pub t: f32, pub seed: f32 }
#[derive(Component)] pub struct MsSunspot   { pub idx: usize, pub lat: f32, pub lon: f32, pub seed: f32 }

fn ph(a:f32,b:f32)->f32{((a*12.9898+b*78.233).sin()*43758.5453).fract()}
fn snap(v:Vec3,g:f32)->Vec3{Vec3::new((v.x/g).round()*g,(v.y/g).round()*g,(v.z/g).round()*g)}
fn sph(lat:f32,lon:f32,r:f32)->Vec3{Vec3::new(r*lat.cos()*lon.cos(),r*lat.sin(),r*lat.cos()*lon.sin())}
fn lc(a:[f32;3],b:[f32;3],t:f32)->[f32;3]{let t=t.clamp(0.0,1.0);[a[0]+(b[0]-a[0])*t,a[1]+(b[1]-a[1])*t,a[2]+(b[2]-a[2])*t]}

fn spawn_ms(mut cmd:Commands,res:Res<MainSequenceRes>,mut msh:ResMut<Assets<Mesh>>,mut mat:ResMut<Assets<StandardMaterial>>) {
    for (i,cfg) in res.stars.iter().enumerate(){build_ms(&mut cmd,cfg,i,&mut msh,&mut mat);}
}

fn build_ms(cmd:&mut Commands,cfg:&MainSequenceConfig,idx:usize,msh:&mut ResMut<Assets<Mesh>>,mat:&mut ResMut<Assets<StandardMaterial>>) {
    let pos=if cfg.orbit_distance>1.0{Vec3::new(cfg.orbit_distance,0.0,0.0)}else{cfg.position};
    let root=cmd.spawn((Transform::from_translation(pos),Visibility::default(),MsRoot{idx})).id();
    let body=cmd.spawn((Transform::IDENTITY,Visibility::default(),MsBody{idx})).id();
    cmd.entity(root).add_child(body);
    let r=cfg.base_radius*cfg.spectral_class.radius_factor();
    let auto_col=cfg.spectral_class.color();
    let col=if cfg.photosphere_color==[0.0,0.0,0.0]{auto_col}else{cfg.photosphere_color};
    let em=cfg.spectral_class.emissive();
    let vs=cfg.voxel_size; let rs=cfg.resolution;
    let smesh=msh.add(Mesh::from(Cuboid::new(vs,vs,vs)));
    let vstep=std::f32::consts::PI/rs as f32; let hstep=std::f32::consts::TAU/(rs*2) as f32;
    let mut lat=-std::f32::consts::FRAC_PI_2;
    while lat<=std::f32::consts::FRAC_PI_2 {
        let mut lon=0.0_f32;
        while lon<std::f32::consts::TAU {
            let noise=ph(lat*7.0+cfg.seed as f32,lon*4.0)*0.07-0.035;
            let rv=r*(1.0+noise);
            let lat_t=(lat+std::f32::consts::FRAC_PI_2)/std::f32::consts::PI;
            let limb=1.0-(1.0-lat_t.min(1.0-lat_t)*2.0).abs().powf(0.4)*0.35;
            let[sr,sg,sb]=col; let[cr,cg,cb]=[sr*limb,sg*limb,sb*limb];
            let sm=mat.add(StandardMaterial{base_color:Color::srgb(cr,cg,cb),emissive:LinearRgba::new(cr*em,cg*em,cb*em,1.0),unlit:true,..default()});
            let v=cmd.spawn((Mesh3d(smesh.clone()),MeshMaterial3d(sm),Transform::from_translation(snap(sph(lat,lon,rv),vs)),NotShadowCaster,MsSurf{idx,lat,lon})).id();
            cmd.entity(body).add_child(v);
            lon+=hstep;
        }
        lat+=vstep;
    }
    // Taches
    let sp_mesh=msh.add(Mesh::from(Cuboid::new(vs*1.4,vs*0.3,vs*1.4)));
    let[spr,spg,spb]=cfg.sunspot_color;
    let sp_mat=mat.add(StandardMaterial{base_color:Color::srgb(spr,spg,spb),emissive:LinearRgba::new(spr*0.5,spg*0.5,spb*0.5,1.0),unlit:true,..default()});
    for i in 0..cfg.sunspot_count {
        let h1=ph(cfg.seed as f32+i as f32,1.0); let h2=ph(cfg.seed as f32+i as f32,2.0);
        let slat=(h1*2.0-1.0).clamp(-1.0,1.0).asin()*0.65; let slon=h2*std::f32::consts::TAU;
        let sp=cmd.spawn((Mesh3d(sp_mesh.clone()),MeshMaterial3d(sp_mat.clone()),Transform::from_translation(snap(sph(slat,slon,r+vs*0.1),vs)),NotShadowCaster,MsSunspot{idx,lat:slat,lon:slon,seed:ph(h1,h2)})).id();
        cmd.entity(body).add_child(sp);
    }
    // Chromosphère
    if cfg.chromosphere_enabled {
        let[chr,chg,chb]=cfg.chromosphere_color; let che=cfg.chromosphere_emissive;
        let chm=mat.add(StandardMaterial{base_color:Color::srgb(chr,chg,chb),emissive:LinearRgba::new(chr*che,chg*che,chb*che,1.0),alpha_mode:AlphaMode::Add,unlit:true,..default()});
        let chmesh=msh.add(Mesh::from(Cuboid::new(vs*0.8,vs*0.8,vs*0.8)));
        for i in 0..cfg.chromosphere_count {
            let h1=ph(cfg.seed as f32+10.0,i as f32); let h2=ph(cfg.seed as f32+11.0,i as f32); let h3=ph(cfg.seed as f32+12.0,i as f32);
            let theta=h1*std::f32::consts::TAU; let phi=(h2*2.0-1.0).clamp(-1.0,1.0).acos();
            let rv=r+h3*cfg.chromosphere_height;
            let cv=cmd.spawn((Mesh3d(chmesh.clone()),MeshMaterial3d(chm.clone()),Transform::from_translation(Vec3::ZERO),Visibility::default(),NotShadowCaster,MsChromos{idx,theta,phi,r:rv,seed:ph(h1,h2)})).id();
            cmd.entity(root).add_child(cv);
        }
    }
    // Granules
    if cfg.granule_enabled {
        let gm=msh.add(Mesh::from(Cuboid::new(cfg.granule_voxel,cfg.granule_voxel,cfg.granule_voxel)));
        for i in 0..cfg.granule_count {
            let h1=ph(cfg.seed as f32+20.0,i as f32); let h2=ph(cfg.seed as f32+21.0,i as f32); let h3=ph(cfg.seed as f32+22.0,i as f32);
            let theta=h1*std::f32::consts::TAU; let phi=(h2*2.0-1.0).clamp(-1.0,1.0).acos();
            let rv=r+h3*cfg.granule_voxel*0.5;
            let o=Vec3::new(rv*phi.sin()*theta.cos(),rv*phi.cos(),rv*phi.sin()*theta.sin());
            let[gr,gg,gb]=col; let ge=em*0.7;
            let gmat=mat.add(StandardMaterial{base_color:Color::srgb(gr,gg,gb),emissive:LinearRgba::new(gr*ge,gg*ge,gb*ge,1.0),alpha_mode:AlphaMode::Add,unlit:true,..default()});
            let gv=cmd.spawn((Mesh3d(gm.clone()),MeshMaterial3d(gmat),Transform::from_translation(snap(o,cfg.granule_voxel)),Visibility::default(),NotShadowCaster,MsGranule{idx,origin:snap(o,cfg.granule_voxel),seed:ph(h1,h2),phase:h3*std::f32::consts::TAU})).id();
            cmd.entity(root).add_child(gv);
        }
    }
    // Couronne
    let[cor,cog,cob]=cfg.corona_color; let coe=cfg.corona_emissive;
    let com=mat.add(StandardMaterial{base_color:Color::srgb(cor,cog,cob),emissive:LinearRgba::new(cor*coe,cog*coe,cob*coe,1.0),alpha_mode:AlphaMode::Add,unlit:true,..default()});
    let comesh=msh.add(Mesh::from(Cuboid::new(cfg.corona_voxel,cfg.corona_voxel,cfg.corona_voxel)));
    for i in 0..cfg.corona_count {
        let h1=ph(cfg.seed as f32+30.0,i as f32); let h2=ph(cfg.seed as f32+31.0,i as f32); let h3=ph(cfg.seed as f32+32.0,i as f32);
        let theta=h1*std::f32::consts::TAU; let phi=(h2*2.0-1.0).clamp(-1.0,1.0).acos();
        let rv=r+h3.sqrt()*cfg.corona_radius;
        let cov=cmd.spawn((Mesh3d(comesh.clone()),MeshMaterial3d(com.clone()),Transform::from_translation(Vec3::ZERO),Visibility::default(),NotShadowCaster,MsCorona{idx,theta,phi,r:rv,seed:ph(h1,h3)})).id();
        cmd.entity(root).add_child(cov);
    }
    // Arcs de convection
    if cfg.convection_arcs {
        let[ar2,ag2,ab2]=cfg.arc_color; let ae2=cfg.arc_emissive;
        let am2=mat.add(StandardMaterial{base_color:Color::srgb(ar2,ag2,ab2),emissive:LinearRgba::new(ar2*ae2,ag2*ae2,ab2*ae2,1.0),alpha_mode:AlphaMode::Add,unlit:true,..default()});
        let amesh2=msh.add(Mesh::from(Cuboid::new(5.0,5.0,5.0)));
        for ai in 0..cfg.arc_count { for si in 0..cfg.arc_samples {
            cmd.spawn((Mesh3d(amesh2.clone()),MeshMaterial3d(am2.clone()),Transform::from_translation(Vec3::ZERO).with_scale(Vec3::ZERO),Visibility::default(),NotShadowCaster,MsArc{idx,ai,si}));
        }}
    }
    // Vent solaire
    if cfg.wind_enabled {
        let[wr,wg,wb]=cfg.wind_color; let we=cfg.wind_emissive;
        let wm=mat.add(StandardMaterial{base_color:Color::srgb(wr,wg,wb),emissive:LinearRgba::new(wr*we,wg*we,wb*we,1.0),alpha_mode:AlphaMode::Add,unlit:true,..default()});
        let wmesh=msh.add(Mesh::from(Cuboid::new(cfg.wind_voxel,cfg.wind_voxel,cfg.wind_voxel)));
        for i in 0..cfg.wind_count {
            let h1=ph(cfg.seed as f32+50.0,i as f32); let h2=ph(cfg.seed as f32+51.0,i as f32);
            let theta=h1*std::f32::consts::TAU; let phi=(h2*2.0-1.0).clamp(-1.0,1.0).acos();
            let dir=Vec3::new(phi.sin()*theta.cos(),phi.cos(),phi.sin()*theta.sin()).normalize();
            let wv=cmd.spawn((Mesh3d(wmesh.clone()),MeshMaterial3d(wm.clone()),Transform::from_translation(Vec3::ZERO),Visibility::default(),NotShadowCaster,SolarWind{idx,dir,t:ph(h1,h2),seed:ph(h2,h1)})).id();
            cmd.entity(root).add_child(wv);
        }
    }
    let li=cfg.spectral_class.light_intensity(); let[lr,lg,lb]=col;
    cmd.spawn((PointLight{intensity:li * 100.0,range:li.sqrt()*8.0,color:Color::srgb(lr,lg,lb),shadows_enabled:true,..default()},Transform::IDENTITY));
}

fn orbit_ms(time:Res<Time>,res:Res<MainSequenceRes>,mut q:Query<(&mut Transform,&MsRoot)>) {
    let t=time.elapsed_secs();
    for (mut tf,r) in &mut q{if let Some(cfg)=res.stars.get(r.idx){if cfg.orbit_distance>1.0{let a=t*cfg.orbit_speed;tf.translation.x=a.cos()*cfg.orbit_distance;tf.translation.z=a.sin()*cfg.orbit_distance;}}}
}
fn spin_ms(time:Res<Time>,res:Res<MainSequenceRes>,mut q:Query<(&mut Transform,&MsBody)>) {
    let t=time.elapsed_secs();
    for (mut tf,b) in &mut q{if let Some(cfg)=res.stars.get(b.idx){tf.rotation=Quat::from_rotation_y(t*cfg.spin_speed);}}
}
fn animate_ms_granules(time:Res<Time>,res:Res<MainSequenceRes>,mut q:Query<(&MsGranule,&mut Transform)>) {
    let t=time.elapsed_secs();
    for (gv,mut tf) in &mut q {
        let Some(cfg)=res.stars.get(gv.idx) else{continue;};
        let cycle=(t*cfg.granule_speed+gv.phase).sin()*0.5+0.5;
        let drift=Vec3::new((t*cfg.granule_speed*0.7+gv.seed*11.0).sin()*4.5,(t*cfg.granule_speed*0.5+gv.seed*7.3).cos()*2.5,(t*cfg.granule_speed*0.9+gv.seed*5.1).sin()*4.5);
        tf.translation=snap(gv.origin+drift,cfg.granule_voxel);
        let s=0.6+cycle*0.6; tf.scale=Vec3::splat(s);
    }
}
fn animate_chromosphere(time:Res<Time>,res:Res<MainSequenceRes>,mut q:Query<(&MsChromos,&mut Transform)>) {
    let t=time.elapsed_secs();
    for (cv,mut tf) in &mut q {
        let Some(cfg)=res.stars.get(cv.idx) else{continue;};
        let r=cfg.base_radius*cfg.spectral_class.radius_factor();
        let pulse=1.0+(t*1.2+cv.seed*std::f32::consts::TAU).sin()*0.2;
        let rv=cv.r*pulse; let th=cv.theta+t*0.1;
        tf.translation=snap(Vec3::new(rv*cv.phi.sin()*th.cos(),rv*cv.phi.cos(),rv*cv.phi.sin()*th.sin()),cfg.voxel_size*0.8);
        let op=(cv.r-r)/cfg.chromosphere_height.max(1.0);
        let s=(1.0-op)*0.8; tf.scale=Vec3::splat(s.max(0.03));
    }
}
fn animate_corona_ms(time:Res<Time>,res:Res<MainSequenceRes>,mut q:Query<(&MsCorona,&mut Transform)>) {
    let t=time.elapsed_secs();
    for (cv,mut tf) in &mut q {
        let Some(cfg)=res.stars.get(cv.idx) else{continue;};
        let r=cfg.base_radius*cfg.spectral_class.radius_factor();
        let pulse=1.0+(t*0.9+cv.seed*std::f32::consts::TAU).sin()*0.22;
        let rv=(cv.r-r)*pulse+r; let th=cv.theta+t*0.06;
        tf.translation=snap(Vec3::new(rv*cv.phi.sin()*th.cos(),rv*cv.phi.cos(),rv*cv.phi.sin()*th.sin()),cfg.corona_voxel);
        let life=1.0-((rv-r)/cfg.corona_radius.max(1.0)).clamp(0.0,1.0);
        let s=0.3+life*0.8; tf.scale=Vec3::splat(s.max(0.02));
    }
}
fn animate_convection_arcs(time:Res<Time>,res:Res<MainSequenceRes>,mut q:Query<(&MsArc,&mut Transform,&mut Visibility)>) {
    let t=time.elapsed_secs();
    for (av,mut tf,mut vis) in &mut q {
        let Some(cfg)=res.stars.get(av.idx) else{*vis=Visibility::Hidden;continue;};
        if !cfg.convection_arcs{*vis=Visibility::Hidden;continue;}
        let r=cfg.base_radius*cfg.spectral_class.radius_factor();
        let base=cfg.seed as f32*6.1+av.ai as f32*41.3;
        let cycle=3.0+ph(base,1.0)*5.0; let off=ph(base,2.0)*cycle;
        let lt=((t*cfg.arc_speed*0.4+off)%cycle).max(0.0);
        let life=if lt<cycle*0.5{(lt/(cycle*0.5)).powi(2)}else{((cycle-lt)/(cycle*0.5)).powi(2)};
        if life<0.05{*vis=Visibility::Hidden;continue;}
        let frac=av.si as f32/cfg.arc_samples.max(1) as f32;
        let h1=ph(base,av.si as f32*0.7); let h2=ph(base,av.si as f32*1.4);
        let theta=ph(base,3.0)*std::f32::consts::TAU; let phi=(ph(base,4.0)*2.0-1.0).clamp(-1.0,1.0).acos();
        let foot=Vec3::new(r*phi.sin()*theta.cos(),r*phi.cos(),r*phi.sin()*theta.sin());
        let apex=foot+foot.normalize()*cfg.arc_height*life;
        let off2=foot*ph(base,5.0)*0.3;
        let inv=1.0-frac; let pt=foot*inv*inv+apex*2.0*inv*frac+off2*frac*frac;
        tf.translation=snap(pt,5.0); tf.scale=Vec3::splat((life*(1.0-frac*0.5)).max(0.02));
        *vis=Visibility::Visible;
    }
}
fn animate_solar_wind(time:Res<Time>,res:Res<MainSequenceRes>,mut q:Query<(&SolarWind,&mut Transform,&mut Visibility)>) {
    let t=time.elapsed_secs();
    for (wv,mut tf,mut vis) in &mut q {
        let Some(cfg)=res.stars.get(wv.idx) else{*vis=Visibility::Hidden;continue;};
        if !cfg.wind_enabled{*vis=Visibility::Hidden;continue;}
        let r=cfg.base_radius*cfg.spectral_class.radius_factor();
        let local_t=((t*cfg.wind_speed/cfg.wind_max_radius+wv.t)%1.0).max(0.0);
        let dist=r+local_t*cfg.wind_max_radius;
        let jitter=Vec3::new((t*0.3+wv.seed*7.0).sin(),(t*0.4+wv.seed*5.0).cos(),(t*0.35+wv.seed*9.0).sin())*dist*0.02;
        tf.translation=snap(wv.dir*dist+jitter,cfg.wind_voxel);
        let fade=(1.0-local_t*0.85).max(0.02); tf.scale=Vec3::splat(fade);
        *vis=Visibility::Visible;
    }
}
fn animate_sunspots_ms(time:Res<Time>,res:Res<MainSequenceRes>,mut q:Query<(&MsSunspot,&mut Transform)>) {
    let t=time.elapsed_secs();
    for (sv,mut tf) in &mut q {
        let Some(cfg)=res.stars.get(sv.idx) else{continue;};
        let r=cfg.base_radius*cfg.spectral_class.radius_factor();
        let diff=1.0-sv.lat.abs()/std::f32::consts::FRAC_PI_2*0.25;
        let lon=sv.lon+t*cfg.sunspot_speed*diff;
        let pulse=1.0+(t*1.5+sv.seed*6.28).sin()*0.1;
        tf.translation=snap(sph(sv.lat,lon,r+cfg.voxel_size*0.1),cfg.voxel_size);
        tf.scale=Vec3::splat(pulse);
    }
}
fn regenerate_ms(mut cmd:Commands,mut ev:EventReader<RegenerateMainSequence>,res:Res<MainSequenceRes>,rq:Query<Entity,With<MsRoot>>,mut msh:ResMut<Assets<Mesh>>,mut mat:ResMut<Assets<StandardMaterial>>) {
    let mut f=false;for _ in ev.read(){f=true;}if !f{return;}
    for e in &rq{cmd.entity(e).despawn_recursive();}
    for (i,cfg) in res.stars.iter().enumerate(){build_ms(&mut cmd,cfg,i,&mut msh,&mut mat);}
}
