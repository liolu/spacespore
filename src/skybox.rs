//! Skybox générée (0.13 C2) : le ciel étoilé est celui de la vraie galaxie, vu du système où l'on
//! est. Une cubemap de 1 024 x 1 024 par face est calculée en arrière-plan (étoiles voisines avec leur
//! couleur et leur éclat, bande de la Voie lactée tirée de la forme réelle de la galaxie, voiles de
//! gaz et poussière, autres galaxies) et refaite quand on change de système. Un dôme (`SkyMaterial`,
//! `sky_dome.wgsl`) la dessine à l'arrière-plan, sans toucher à la profondeur : les étoiles proches
//! restent de vrais objets cliquables, devant elle.
//!
//! Le dôme dessine aussi la couleur du ciel (celle de `ClearColor`) : le jour, elle efface les
//! étoiles ; les nuages, sous terre et sous l'eau les voilent ; au sol la galaxie apparaît la nuit.

#![allow(dead_code)]

use bevy::asset::{load_internal_asset, RenderAssetUsages};
use bevy::pbr::{NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;
use bevy::render::primitives::Aabb;
use bevy::render::render_resource::{AsBindGroup, Extent3d, RenderPipelineDescriptor, ShaderRef, ShaderType, SpecializedMeshPipelineError, TextureDimension, TextureFormat, TextureViewDescriptor, TextureViewDimension};
use bevy::render::view::NoFrustumCulling;
use bevy::tasks::{block_on, futures_lite::future, AsyncComputeTaskPool, Task};

use crate::galaxy_shape::Rng;
use crate::settings::{GalaxyConfig, GameSettings};
use crate::{CameraController, ZoomLevel};

pub const SKY_SHADER: Handle<Shader> = Handle::weak_from_u128(0x5ea_f00d_0a7e_c0de_1234_5678_9ad0);

/// Côté d'une face de la cubemap (pixels).
pub const SKY_SIZE: usize = 1024;

// ─────────────────────────────────────────────────────────────────────────
//  Géométrie de la cubemap (convention des cartes cubiques des cartes graphiques)
// ─────────────────────────────────────────────────────────────────────────

/// Direction (non normalisée) du point (`a`, `b`) de la face `face` ; `a`, `b` dans [-1, 1] (a vers la
/// droite de l'image, b vers le bas). Faces : +X, -X, +Y, -Y, +Z, -Z.
pub fn face_dir(face: usize, a: f32, b: f32) -> Vec3 {
    match face {
        0 => Vec3::new(1.0, -b, -a),
        1 => Vec3::new(-1.0, -b, a),
        2 => Vec3::new(a, 1.0, b),
        3 => Vec3::new(a, -1.0, -b),
        4 => Vec3::new(a, -b, 1.0),
        _ => Vec3::new(-a, -b, -1.0),
    }
}

/// Face et coordonnées (a, b) dans [-1, 1] d'une direction.
pub fn dir_to_face(d: Vec3) -> (usize, f32, f32) {
    let ad = d.abs();
    if ad.x >= ad.y && ad.x >= ad.z {
        if d.x > 0.0 { (0, -d.z / ad.x, -d.y / ad.x) } else { (1, d.z / ad.x, -d.y / ad.x) }
    } else if ad.y >= ad.z {
        if d.y > 0.0 { (2, d.x / ad.y, d.z / ad.y) } else { (3, d.x / ad.y, -d.z / ad.y) }
    } else if d.z > 0.0 {
        (4, d.x / ad.z, -d.y / ad.z)
    } else {
        (5, -d.x / ad.z, -d.y / ad.z)
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  Calcul de la cubemap (hors du fil principal)
// ─────────────────────────────────────────────────────────────────────────

/// Ce qu'il faut pour calculer le ciel d'un système : tout est copié (le calcul est asynchrone).
#[derive(Clone)]
pub struct SkyInput {
    /// Position ABSOLUE du système d'où l'on regarde.
    pub viewer: Vec3,
    /// Sa galaxie (la bande vient de sa forme), et les autres.
    pub galaxy: Option<GalaxyConfig>,
    pub others: Vec<GalaxyConfig>,
    /// Étoiles réelles : position absolue, couleur, lumens.
    pub stars: Vec<(Vec3, [f32; 3], f32)>,
    pub seed: u32,
    pub size: usize,
}

/// Étoiles représentées par l'ensemble des tirages de la bande (leur nombre fixe la luminosité
/// du fond diffus par rapport aux étoiles réelles).
const BAND_STARS: f32 = 3.0e6;

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn to_srgb(x: f32) -> f32 {
    x.clamp(0.0, 1.0).powf(1.0 / 2.2)
}

struct Faces {
    size: usize,
    px: Vec<[f32; 3]>,
}

impl Faces {
    fn new(size: usize) -> Self {
        Self { size, px: vec![[0.0; 3]; 6 * size * size] }
    }

    /// Ajoute une gaussienne de rayon `sigma` (pixels) centrée sur la direction `d` ; déborde sur les faces voisines.
    fn splat(&mut self, d: Vec3, color: [f32; 3], sigma: f32) {
        let n = self.size as f32;
        let (face, a, b) = dir_to_face(d);
        let (fx, fy) = ((a + 1.0) * 0.5 * n, (b + 1.0) * 0.5 * n);
        let reach = (sigma * 2.6).ceil().max(1.0) as i32;
        let norm = 1.0 / (2.0 * std::f32::consts::PI * sigma * sigma);
        for dy in -reach..=reach {
            for dx in -reach..=reach {
                let (x, y) = (fx.floor() as i32 + dx, fy.floor() as i32 + dy);
                let w = (-(((x as f32 + 0.5 - fx).powi(2) + (y as f32 + 0.5 - fy).powi(2)) / (2.0 * sigma * sigma))).exp() * norm;
                if w < 1e-4 {
                    continue;
                }
                let (f2, x2, y2) = if x >= 0 && y >= 0 && x < self.size as i32 && y < self.size as i32 {
                    (face, x as usize, y as usize)
                } else {
                    // Hors de la face : on retrouve la face voisine par la direction du pixel
                    let (pa, pb) = ((x as f32 + 0.5) / n * 2.0 - 1.0, (y as f32 + 0.5) / n * 2.0 - 1.0);
                    let (f3, a3, b3) = dir_to_face(face_dir(face, pa, pb));
                    (f3, (((a3 + 1.0) * 0.5 * n) as usize).min(self.size - 1), (((b3 + 1.0) * 0.5 * n) as usize).min(self.size - 1))
                };
                let p = &mut self.px[(f2 * self.size + y2) * self.size + x2];
                p[0] += color[0] * w;
                p[1] += color[1] * w;
                p[2] += color[2] * w;
            }
        }
    }

    fn at(&self, face: usize, x: usize, y: usize) -> [f32; 3] {
        self.px[(face * self.size + y) * self.size + x]
    }
}

/// Flou en boîte (deux passes séparables) sur chaque face.
fn blur(f: &Faces, radius: usize, passes: usize) -> Faces {
    let n = f.size;
    let mut cur = Faces { size: n, px: f.px.clone() };
    for _ in 0..passes {
        for horizontal in [true, false] {
            let src = Faces { size: n, px: cur.px.clone() };
            for face in 0..6 {
                for y in 0..n {
                    for x in 0..n {
                        let (mut r, mut g, mut b, mut c) = (0.0, 0.0, 0.0, 0.0);
                        for k in -(radius as i32)..=radius as i32 {
                            let (xx, yy) = if horizontal { (x as i32 + k, y as i32) } else { (x as i32, y as i32 + k) };
                            if xx < 0 || yy < 0 || xx >= n as i32 || yy >= n as i32 {
                                continue;
                            }
                            let p = src.at(face, xx as usize, yy as usize);
                            r += p[0];
                            g += p[1];
                            b += p[2];
                            c += 1.0;
                        }
                        cur.px[(face * n + y) * n + x] = [r / c, g / c, b / c];
                    }
                }
            }
        }
    }
    cur
}

/// Couleur d'un tirage de la bande d'après sa place dans la galaxie : noyau jaune, bras bleutés.
fn band_color(rel: f32, mix: f32) -> [f32; 3] {
    let core = smoothstep(0.35, 0.0, rel);
    let warm = [1.0, 0.82, 0.55];
    let cool = [0.62, 0.74, 1.0];
    let white = [1.0, 0.97, 0.92];
    let arm = [cool[0] + (white[0] - cool[0]) * mix, cool[1] + (white[1] - cool[1]) * mix, cool[2] + (white[2] - cool[2]) * mix];
    [arm[0] + (warm[0] - arm[0]) * core, arm[1] + (warm[1] - arm[1]) * core, arm[2] + (warm[2] - arm[2]) * core]
}

/// Cubemap RGBA8 (sRGB) du ciel décrit par `input` : 6 faces de `size` x `size`, les unes après les autres.
pub fn render_cube(input: &SkyInput) -> Vec<u8> {
    let size = input.size;
    let half = (size / 2).max(8);

    // ── Bande de la galaxie : tirages de la forme réelle, vus de l'intérieur ──
    let mut band = Faces::new(half);
    let mut density = 0.0f32;
    if let Some(g) = &input.galaxy {
        let shape = g.shape();
        let n_samples = (half * half) as f32 * 1.6;
        let threads = 6usize;
        let per = (n_samples as usize / threads).max(1);
        let parts: Vec<Faces> = std::thread::scope(|sc| {
            let handles: Vec<_> = (0..threads)
                .map(|t| {
                    let shape = &shape;
                    sc.spawn(move || {
                        let mut f = Faces::new(half);
                        let mut rng = Rng::new(input.seed.wrapping_mul(2654435761).wrapping_add(t as u32 * 7919 + 1));
                        for _ in 0..per {
                            let p = if rng.f() < shape.structure_share { shape.sample_structure(&mut rng, 0.0) } else { shape.sample_background(&mut rng) };
                            let rel_pos = g.abs_center + g.tilt * p - input.viewer;
                            let d = rel_pos.length();
                            // Trop près : ce sont les vraies étoiles, dessinées à part
                            if d < 0.05 * g.radius {
                                continue;
                            }
                            let u = rng.f();
                            let lum = 0.35 + 2.2 * u * u * u;
                            // (un tirage tout près ferait un bloc blanc : son éclat est plafonné)
                            let k = (g.radius / d).powi(2).min(120.0) * lum * (BAND_STARS / (per * threads) as f32) * 4.0e-4;
                            let color = band_color(p.length() / g.radius, rng.f());
                            f.splat(rel_pos / d, [color[0] * k, color[1] * k, color[2] * k], 0.85);
                        }
                        f
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });
        for part in &parts {
            for (a, b) in band.px.iter_mut().zip(&part.px) {
                a[0] += b[0];
                a[1] += b[1];
                a[2] += b[2];
            }
        }
        density = 1.0;
    }
    let soft = blur(&band, 3, 2);

    // Niveau de la bande : le 97e centile du fond diffus donne le réglage de l'exposition
    let mut lum: Vec<f32> = soft.px.iter().map(|p| (p[0] + p[1] + p[2]) / 3.0).collect();
    lum.sort_by(|a, b| a.total_cmp(b));
    let p97 = lum.get(lum.len() * 999 / 1000).copied().unwrap_or(1.0).max(1e-9);
    let gain = 0.55 / p97;
    let p50 = lum.get(lum.len() / 2).copied().unwrap_or(0.0);

    // ── Pixels : bande, poussière, nébuleuses, autres galaxies ──
    let mut out = Faces::new(size);
    let seed = input.seed;
    // Autres galaxies : direction, taille angulaire, axes de l'ellipse, couleur
    struct Far {
        dir: Vec3,
        rho: f32,
        minor: Vec3,
        major: Vec3,
        q: f32,
        color: [f32; 3],
        bright: f32,
    }
    let fars: Vec<Far> = input
        .others
        .iter()
        .filter_map(|g| {
            let rel = g.abs_center - input.viewer;
            let d = rel.length();
            if d < 1.0 {
                return None;
            }
            let dir = rel / d;
            let normal = g.tilt * Vec3::Y;
            let q = dir.dot(normal).abs().max(0.18);
            let minor = (normal - dir * normal.dot(dir)).normalize_or(Vec3::Y);
            let major = dir.cross(minor).normalize_or(Vec3::X);
            let rho = (g.radius / d).min(0.6);
            let h = crate::settings::pseudo_rand(g.seed ^ 0x9E37);
            let color = if h < 0.5 { [0.75, 0.82, 1.0] } else { [1.0, 0.88, 0.7] };
            Some(Far { dir, rho: rho.max(0.0016), minor, major, q, color, bright: 1.0 / (1.0 + 18.0 * rho) })
        })
        .collect();
    let rows = size;
    std::thread::scope(|sc| {
        let chunks: Vec<(usize, &mut [[f32; 3]])> = out.px.chunks_mut(size * size).enumerate().collect();
        let fars = &fars;
        for (face, buf) in chunks {
            sc.spawn(move || {
                for y in 0..rows {
                    for x in 0..size {
                        let (a, b) = ((x as f32 + 0.5) / size as f32 * 2.0 - 1.0, (y as f32 + 0.5) / size as f32 * 2.0 - 1.0);
                        let dir = face_dir(face, a, b).normalize();
                        // Pas de bande de galaxie dessinee (la ligne blanche est retiree) : seulement un tres leger voile
                        let mut c = [0.002f32; 3];
                        // Autres galaxies
                        for f in fars {
                            let cosv = dir.dot(f.dir);
                            if cosv < 0.0 {
                                continue;
                            }
                            let off = dir - f.dir * cosv;
                            let (u, v) = (off.dot(f.major) / f.rho, off.dot(f.minor) / (f.rho * f.q));
                            let r2 = u * u + v * v;
                            if r2 < 16.0 {
                                let s = f.bright * 0.9 * (1.0 + r2 * 2.0).powf(-1.5) * (1.0 / (1.0 + 4.0 * f.q.min(1.0).powi(0)));
                                for k in 0..3 {
                                    c[k] += f.color[k] * s;
                                }
                            }
                        }
                        buf[y * size + x] = c;
                    }
                }
            });
        }
    });

    // ── Fond d'étoiles : des centaines de milliers de petites étoiles lointaines, de couleurs variées ──
    {
        let mut rng = Rng::new(seed ^ 0x57A2);
        let count = (size * size) as f32 * 0.24;
        for _ in 0..count as usize {
            let (u, v, w) = (rng.f() * 2.0 - 1.0, rng.f() * 2.0 - 1.0, rng.f() * 2.0 - 1.0);
            let d = Vec3::new(u, v, w);
            let l = d.length();
            if !(0.05..=1.0).contains(&l) {
                continue;
            }
            // Température : surtout blanches et bleutées, des jaunes, quelques orangées
            let t = rng.f();
            let color = if t < 0.5 { [0.8, 0.88, 1.0] } else if t < 0.78 { [1.0, 0.96, 0.82] } else if t < 0.93 { [1.0, 0.82, 0.58] } else { [1.0, 0.6, 0.45] };
            let m = rng.f();
            let k = 0.08 + 1.5 * m.powi(6);
            let sigma = 0.5 + 0.4 * m.powi(6);
            out.splat(d / l, [color[0] * k, color[1] * k, color[2] * k], sigma);
        }
    }

    // ── Étoiles réelles : un point par étoile, éclat selon son flux ──
    let (scale, mean_l) = match &input.galaxy {
        Some(g) if !input.stars.is_empty() => (g.radius, input.stars.iter().map(|s| s.2).sum::<f32>() / input.stars.len() as f32),
        _ => (1.0, 1.0),
    };
    for (pos, color, lumens) in &input.stars {
        let rel = *pos - input.viewer;
        let d = rel.length();
        if d < 1.0 {
            continue;
        }
        let flux = (lumens / mean_l.max(1.0)) * (scale / d).powi(2) * 4.0e-4 * gain * (BAND_STARS / 1.0e5);
        // Éclat -> taille du point (les brillantes débordent un peu) et intensité
        let sigma = 0.8 + 0.35 * (1.0 + flux).ln().min(4.0);
        let k = flux.min(40.0) + 0.05;
        out.splat(rel / d, [color[0] * k, color[1] * k, color[2] * k], sigma);
    }
    let _ = (density, p50);

    // ── Tons : exposition douce, gamma, un grain contre les bandes ──
    let mut rng = Rng::new(seed ^ 0xA5A5);
    let mut bytes = Vec::with_capacity(out.px.len() * 4);
    for p in &out.px {
        let mut rgb = [0u8; 3];
        for k in 0..3 {
            let x = 1.0 - (-p[k]).exp();
            let v = to_srgb(x) * 255.0 + (rng.f() - 0.5) * 1.2;
            rgb[k] = v.clamp(0.0, 255.0) as u8;
        }
        bytes.extend_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
    }
    bytes
}

// ─────────────────────────────────────────────────────────────────────────
//  Matériau du dôme
// ─────────────────────────────────────────────────────────────────────────

#[derive(ShaderType, Clone, Copy, Debug, Default)]
pub struct SkyParams {
    /// Couleur du ciel (clair de lune, jour, brouillard...) ; w = 1.
    pub sky: Vec4,
    /// x : part des étoiles visibles (0..1), y : éclat des étoiles.
    pub stars: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct SkyMaterial {
    #[texture(0, dimension = "cube")]
    #[sampler(1)]
    pub cube: Handle<Image>,
    #[uniform(2)]
    pub params: SkyParams,
}

impl Material for SkyMaterial {
    fn vertex_shader() -> ShaderRef {
        SKY_SHADER.into()
    }

    fn fragment_shader() -> ShaderRef {
        SKY_SHADER.into()
    }

    // À l'arrière-plan : on ne cache que le vide, et rien n'est écrit dans la profondeur
    fn specialize(
        _pipeline: &bevy::pbr::MaterialPipeline<Self>,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &bevy::render::mesh::MeshVertexBufferLayoutRef,
        _key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        if let Some(ds) = descriptor.depth_stencil.as_mut() {
            ds.depth_write_enabled = false;
            ds.depth_compare = bevy::render::render_resource::CompareFunction::GreaterEqual;
        }
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  Plugin : calcul au changement de système, dôme qui suit la caméra
// ─────────────────────────────────────────────────────────────────────────

pub struct SkyboxPlugin;

impl Plugin for SkyboxPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, SKY_SHADER, "sky_dome.wgsl", Shader::from_wgsl);
        app.add_plugins(MaterialPlugin::<SkyMaterial>::default())
            .init_resource::<SkyState>()
            .add_systems(Startup, spawn_dome)
            .add_systems(Update, (first_sky, watch_system, finish_sky, sky_params).chain())
            .add_systems(PostUpdate, follow_camera.before(TransformSystem::TransformPropagate));
    }
}

#[derive(Resource, Default)]
pub struct SkyState {
    /// Système d'où le ciel a été calculé (ou est en calcul).
    pub system: Option<usize>,
    task: Option<(usize, Task<Vec<u8>>)>,
    pub image: Handle<Image>,
    pub ready: bool,
    last_start: f64,
    /// Durée du dernier calcul (s), pour les tests.
    pub took: f32,
    started: Option<std::time::Instant>,
    /// Point de vue du dernier ciel et sa galaxie : un voisin proche donne le meme ciel, on ne recalcule pas.
    viewer: Option<(Vec3, usize)>,
}

#[derive(Component)]
struct SkyDome;

fn cube_image(bytes: Vec<u8>, size: usize) -> Image {
    let mut image = Image::new(Extent3d { width: size as u32, height: size as u32, depth_or_array_layers: 6 }, TextureDimension::D2, bytes, TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::RENDER_WORLD);
    image.texture_view_descriptor = Some(TextureViewDescriptor { dimension: Some(TextureViewDimension::Cube), ..default() });
    image
}

fn spawn_dome(mut commands: Commands, mut images: ResMut<Assets<Image>>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<SkyMaterial>>, mut state: ResMut<SkyState>) {
    // Ciel noir en attendant le premier calcul
    let black = images.add(cube_image(vec![0, 0, 0, 255].repeat(6), 1));
    state.image = black.clone();
    let mat = materials.add(SkyMaterial { cube: black, params: SkyParams { sky: Vec4::new(0.0, 0.0, 0.0, 1.0), stars: Vec4::new(1.0, 1.0, 0.0, 0.0) } });
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(50.0).mesh().ico(2).unwrap())),
        MeshMaterial3d(mat),
        Transform::default(),
        Visibility::Inherited,
        NoFrustumCulling,
        NotShadowCaster,
        NotShadowReceiver,
        Aabb::from_min_max(Vec3::splat(-1.0e9), Vec3::splat(1.0e9)),
        SkyDome,
    ));
}

/// Le dôme suit la caméra : il n'a pas de profondeur propre (voir `sky_dome.wgsl`).
fn follow_camera(cam_q: Query<&Transform, (With<Camera3d>, Without<SkyDome>)>, mut dome: Query<&mut Transform, With<SkyDome>>) {
    let (Ok(cam), Ok(mut tf)) = (cam_q.get_single(), dome.get_single_mut()) else { return };
    tf.translation = cam.translation;
}

/// Le tout premier ciel est calcule avant la premiere image (le jeu s'ouvre avec ses etoiles, pas sur du noir).
fn first_sky(
    mut done: Local<bool>,
    settings: Res<GameSettings>,
    cam_q: Query<&Transform, With<Camera3d>>,
    ship_q: Query<&Transform, (With<crate::ship::Ship>, Without<Camera3d>)>,
    mut state: ResMut<SkyState>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<SkyMaterial>>,
    dome: Query<&MeshMaterial3d<SkyMaterial>, With<SkyDome>>,
) {
    if *done {
        return;
    }
    *done = true;
    let here = ship_q.get_single().map(|t| t.translation).or_else(|_| cam_q.get_single().map(|t| t.translation)).unwrap_or(Vec3::ZERO);
    let Some((si, input)) = sky_input(&settings, here) else { return };
    let started = std::time::Instant::now();
    let bytes = render_cube(&input);
    state.viewer = Some((input.viewer, settings.systems.get(si).map_or(0, |s| s.galaxy_id as usize)));
    state.system = Some(si);
    state.ready = true;
    state.took = started.elapsed().as_secs_f32();
    let handle = images.add(cube_image(bytes, SKY_SIZE));
    state.image = handle.clone();
    info!("Ciel : premier ciel pret en {:.1} s (systeme {si})", state.took);
    if let Ok(m) = dome.get_single() {
        if let Some(mat) = materials.get_mut(&m.0) {
            mat.cube = handle;
        }
    }
}

/// Système le plus proche de `here` (monde) et ce qu'il faut pour calculer son ciel.
fn sky_input(settings: &GameSettings, here: Vec3) -> Option<(usize, SkyInput)> {
    let mut best: Option<(usize, f32)> = None;
    for (i, s) in settings.systems.iter() {
        let d = (s.center() - here).length_squared();
        if best.is_none_or(|(_, b)| d < b) {
            best = Some((i, d));
        }
    }
    let (si, _) = best?;
    let sys = settings.systems.get(si)?;
    let viewer = sys.abs_center();
    let gid = sys.galaxy_id as usize;
    let galaxy = settings.galaxies.get(gid).cloned();
    let mut others: Vec<(f32, GalaxyConfig)> = settings
        .galaxies
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != gid)
        .map(|(_, g)| (g.radius / (g.abs_center - viewer).length().max(1.0), g.clone()))
        .collect();
    others.sort_by(|a, b| b.0.total_cmp(&a.0));
    let others: Vec<GalaxyConfig> = others.into_iter().take(48).map(|(_, g)| g).collect();
    let mut stars: Vec<(f32, Vec3, [f32; 3], f32)> = settings
        .systems
        .iter()
        .filter(|(i, _)| *i != si)
        .filter_map(|(_, s)| {
            let st = s.stars.first()?;
            Some(((s.abs_center() - viewer).length_squared(), s.abs_center(), [st.light_color_r, st.light_color_g, st.light_color_b], st.lumens()))
        })
        .collect();
    stars.sort_by(|a, b| a.0.total_cmp(&b.0));
    let stars: Vec<(Vec3, [f32; 3], f32)> = stars.into_iter().skip(40).map(|(_, p, c, l)| (p, c, l)).collect();
    Some((si, SkyInput { viewer, galaxy, others, stars, seed: (si as u32).wrapping_mul(2246822519) ^ settings.world_seed as u32, size: SKY_SIZE }))
}

/// Système le plus proche de la caméra (parmi ceux qui existent) : quand il change, le ciel est refait.
fn watch_system(time: Res<Time>, settings: Res<GameSettings>, cam_q: Query<&Transform, With<Camera3d>>, mut state: ResMut<SkyState>) {
    let now = time.elapsed_secs_f64();
    // Le premier ciel part tout de suite ; les suivants, au plus un toutes les 5 s
    if state.task.is_some() || (state.ready && now - state.last_start < 5.0) {
        return;
    }
    let Ok(cam) = cam_q.get_single() else { return };
    let here = cam.translation;
    let mut best: Option<(usize, f32)> = None;
    for (i, s) in settings.systems.iter() {
        let d = (s.center() - here).length_squared();
        if best.is_none_or(|(_, b)| d < b) {
            best = Some((i, d));
        }
    }
    let Some((si, _)) = best else { return };
    if state.system == Some(si) {
        return;
    }
    let Some(sys) = settings.systems.get(si) else { return };
    let viewer = sys.abs_center();
    let gid = sys.galaxy_id as usize;
    if let Some((prev, pg)) = state.viewer {
        if pg == gid && (viewer - prev).length() < 1.5e9 {
            state.system = Some(si);
            return;
        }
    }
    state.viewer = Some((viewer, gid));
    let galaxy = settings.galaxies.get(gid).cloned();
    // Les autres galaxies : les 48 qui paraissent les plus grandes (10 000 au total, la plupart ne sont que des points)
    let mut others: Vec<(f32, GalaxyConfig)> = settings
        .galaxies
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != gid)
        .map(|(_, g)| (g.radius / (g.abs_center - viewer).length().max(1.0), g.clone()))
        .collect();
    others.sort_by(|a, b| b.0.total_cmp(&a.0));
    let others: Vec<GalaxyConfig> = others.into_iter().take(48).map(|(_, g)| g).collect();
    // Les étoiles voisines (une cinquantaine) sont de vrais objets : seules les autres sont dessinées
    let mut stars: Vec<(f32, Vec3, [f32; 3], f32)> = settings
        .systems
        .iter()
        .filter(|(i, _)| *i != si)
        .filter_map(|(_, s)| {
            let st = s.stars.first()?;
            Some(((s.abs_center() - viewer).length_squared(), s.abs_center(), [st.light_color_r, st.light_color_g, st.light_color_b], st.lumens()))
        })
        .collect();
    stars.sort_by(|a, b| a.0.total_cmp(&b.0));
    let stars: Vec<(Vec3, [f32; 3], f32)> = stars.into_iter().skip(40).map(|(_, p, c, l)| (p, c, l)).collect();
    let input = SkyInput { viewer, galaxy, others, stars, seed: (si as u32).wrapping_mul(2246822519) ^ settings.world_seed as u32, size: SKY_SIZE };
    info!("Ciel : calcul depuis le systeme {si} ({} etoiles, {} autres galaxies)", input.stars.len(), input.others.len());
    state.last_start = now;
    state.started = Some(std::time::Instant::now());
    state.task = Some((si, AsyncComputeTaskPool::get().spawn(async move { render_cube(&input) })));
}

fn finish_sky(mut state: ResMut<SkyState>, mut images: ResMut<Assets<Image>>, mut materials: ResMut<Assets<SkyMaterial>>, dome: Query<&MeshMaterial3d<SkyMaterial>, With<SkyDome>>) {
    let Some((si, task)) = state.task.as_mut() else { return };
    let si = *si;
    let Some(bytes) = block_on(future::poll_once(task)) else { return };
    state.task = None;
    state.system = Some(si);
    state.took = state.started.map_or(0.0, |t| t.elapsed().as_secs_f32());
    let handle = images.add(cube_image(bytes, SKY_SIZE));
    state.image = handle.clone();
    state.ready = true;
    info!("Ciel : pret en {:.1} s", state.took);
    if let Ok(m) = dome.get_single() {
        if let Some(mat) = materials.get_mut(&m.0) {
            mat.cube = handle;
        }
    }
}

/// Couleur du ciel, étoiles visibles : effacées par le ciel de jour, voilées par les nuages, éteintes
/// sous l'eau et sous terre, absentes quand la galaxie se voit en vrai (zooms lointains).
#[allow(clippy::too_many_arguments)]
fn sky_params(
    clear: Res<ClearColor>,
    surface: Res<crate::surface::Surface>,
    weather: Res<crate::weather::WeatherNow>,
    under: Res<crate::water::Underwater>,
    zoom: Res<ZoomLevel>,
    ctrl_q: Query<&CameraController>,
    mut materials: ResMut<Assets<SkyMaterial>>,
    dome: Query<&MeshMaterial3d<SkyMaterial>, With<SkyDome>>,
) {
    let Ok(m) = dome.get_single() else { return };
    let Some(mat) = materials.get_mut(&m.0) else { return };
    let sky = clear.0.to_linear();
    let luma = (sky.red + sky.green + sky.blue) / 3.0;
    // Le jour efface les étoiles ; la nuit et dans l'espace elles se voient
    let mut vis = 1.0 - smoothstep(0.012, 0.18, luma);
    // Les nuages et la brume les voilent
    vis *= 1.0 - 0.9 * weather.sample.cloud.clamp(0.0, 1.0) * surface.air();
    vis *= 1.0 - 0.8 * weather.sample.fog.clamp(0.0, 1.0);
    if under.active() || surface.underground() > 0.4 {
        vis = 0.0;
    }
    // Les zooms lointains montrent la vraie galaxie : plus de fond
    vis *= match *zoom {
        ZoomLevel::Planet | ZoomLevel::System => 1.0,
        ZoomLevel::Sector => 0.6,
        _ => 0.0,
    };
    let _ = ctrl_q;
    mat.params.sky = Vec4::new(sky.red, sky.green, sky.blue, 1.0);
    mat.params.stars = Vec4::new(vis, 1.0, 0.0, 0.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cube_faces_round_trip() {
        for face in 0..6 {
            for (a, b) in [(0.0, 0.0), (0.7, -0.3), (-0.9, 0.9), (0.2, 0.95)] {
                let d = face_dir(face, a, b);
                let (f2, a2, b2) = dir_to_face(d);
                assert_eq!(f2, face, "{a} {b}");
                assert!((a2 - a).abs() < 1e-5 && (b2 - b).abs() < 1e-5, "face {face} : {a} {b} -> {a2} {b2}");
            }
        }
        // Les axes tombent sur la bonne face
        assert_eq!(dir_to_face(Vec3::X).0, 0);
        assert_eq!(dir_to_face(Vec3::NEG_Y).0, 3);
        assert_eq!(dir_to_face(Vec3::NEG_Z).0, 5);
    }

    #[test]
    fn a_splat_keeps_its_light_even_across_an_edge() {
        let mut f = Faces::new(64);
        // Pile sur l'arête entre +X et +Z
        f.splat(Vec3::new(1.0, 0.0, 1.0).normalize(), [1.0, 1.0, 1.0], 1.5);
        let total: f32 = f.px.iter().map(|p| p[0]).sum();
        assert!((total - 1.0).abs() < 0.08, "lumiere {total}");
        let faces_lit: Vec<usize> = (0..6).filter(|fc| (0..64 * 64).any(|i| f.px[fc * 64 * 64 + i][0] > 1e-3)).collect();
        assert!(faces_lit.contains(&0) && faces_lit.contains(&4), "{faces_lit:?}");
    }

    fn input(size: usize) -> SkyInput {
        let galaxies = crate::settings::default_galaxies(crate::settings::DEFAULT_WORLD_SEED);
        let g = galaxies[0].clone();
        let viewer = g.abs_center + Vec3::new(g.radius * 0.45, 0.0, 0.0);
        SkyInput { viewer, galaxy: Some(g), others: galaxies[1..6].to_vec(), stars: vec![(viewer + Vec3::new(3.0e7, 1.0e6, 0.0), [1.0, 0.9, 0.7], 5.0e18), (viewer + Vec3::new(-1.0e8, 5.0e7, 2.0e7), [0.7, 0.8, 1.0], 9.0e18)], seed: 5, size }
    }

    #[test]
    fn the_sky_is_deterministic_and_has_a_band_and_stars() {
        let a = render_cube(&input(96));
        let b = render_cube(&input(96));
        assert_eq!(a, b, "meme ciel pour le meme systeme");
        assert_eq!(a.len(), 6 * 96 * 96 * 4);
        let lum: Vec<f32> = a.chunks(4).map(|p| (p[0] as f32 + p[1] as f32 + p[2] as f32) / 3.0).collect();
        let mean = lum.iter().sum::<f32>() / lum.len() as f32;
        let max = lum.iter().cloned().fold(0.0, f32::max);
        assert!(mean > 3.0 && mean < 160.0, "ciel ni noir ni blanc : {mean}");
        assert!(max > 200.0, "des etoiles brillantes : {max}");
    }

    #[test]
    fn another_viewpoint_gives_another_sky() {
        let mut other = input(64);
        let a = render_cube(&other);
        other.viewer += Vec3::new(0.0, 0.0, other.galaxy.as_ref().unwrap().radius * 0.3);
        let b = render_cube(&other);
        let diff = a.iter().zip(&b).filter(|(x, y)| x != y).count();
        // Plus de bande de galaxie : seules les etoiles reelles et les galaxies lointaines changent avec le lieu
        assert!(diff > 0, "le ciel change avec le lieu : {diff}");
    }

    /// Mesure (règle 18) : `cargo test --release bench_sky -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn bench_sky() {
        let t = std::time::Instant::now();
        let bytes = render_cube(&input(SKY_SIZE));
        println!("ciel {SKY_SIZE}x6 : {:.2} s, {} Mo", t.elapsed().as_secs_f32(), bytes.len() >> 20);
    }
}
