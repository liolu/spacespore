//! Trous noirs des galaxies (0.13 V1), d'après le modèle `trou_noir` (Kerr, macroquad) adapté au jeu.
//!
//! Chaque trou noir central (`GalacticCore`, `DistantGalaxyCore`) reçoit une sphère de lentille : un
//! maillage dont le shader `black_hole.wgsl` suit les rayons lumineux dans la métrique de Kerr
//! (géodésiques nulles en coordonnées de Kerr-Schild), du bord de la sphère vers le trou noir :
//! - l'horizon n'est pas dessiné : seule son ombre se voit (les rayons qui y tombent ne ramènent rien) ;
//! - un rayon qui ressort de la sphère va chercher le **vrai fond** du jeu dans sa nouvelle direction :
//!   l'image déjà rendue derrière le trou noir (texture de transmission de Bevy : étoiles, galaxies,
//!   bras, skybox), sinon la skybox fixe (`skybox.rs`) hors de l'écran. D'où la lentille, l'anneau
//!   d'Einstein et l'image dédoublée, sans rien d'inventé ;
//! - disque d'accrétion semi-transparent en shader (plus chaud au centre, effet Doppler, décalage
//!   gravitationnel, le dessus et le dessous du disque visibles au-dessus et sous l'ombre) ;
//! - jets polaires relativistes ;
//! - une étoile aspirée : un courant de gaz en spirale qui part d'une étoile compagne et tombe sur le disque.
//!
//! Unités du shader : rayon de Schwarzschild = 1 (M = 0,5). Un trou noir de rayon `core_radius` (réglages
//! de la galaxie) a une ombre de ~2,6 unités : 1 unité = `core_radius / 2,6`. La sphère de lentille fait
//! `LENS_RADIUS` unités (au-delà, la déviation est invisible).

use bevy::asset::load_internal_asset;
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey, NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;
use bevy::render::mesh::MeshVertexBufferLayoutRef;
use bevy::render::render_resource::{AsBindGroup, RenderPipelineDescriptor, ShaderRef, ShaderType, SpecializedMeshPipelineError};

use crate::planet::{DistantGalaxyCore, GalacticCore};
use crate::settings::GameSettings;

pub const BLACK_HOLE_SHADER: Handle<Shader> = Handle::weak_from_u128(0xb1ac_4b01_e5ad_e000_7e11_0a3d_9c4f_2b81);

/// Rayon de la sphère de lentille, en rayons de Schwarzschild.
pub const LENS_RADIUS: f32 = 70.0;
/// Rayon apparent de l'ombre, en rayons de Schwarzschild (~2,6 pour un trou noir en rotation rapide).
const SHADOW_UNITS: f32 = 2.6;
/// Spin a/M des trous noirs (Kerr rapide, comme le modèle).
const SPIN: f32 = 0.9;
/// Rayon de l'orbite de l'étoile aspirée (rayons de Schwarzschild).
const STAR_ORBIT: f32 = 34.0;

/// Taille d'un rayon de Schwarzschild (unités du jeu) pour un trou noir de rayon `core_radius`.
pub fn unit_of(core_radius: f32) -> f32 {
    core_radius / SHADOW_UNITS
}

pub struct BlackHoleFxPlugin;

impl Plugin for BlackHoleFxPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, BLACK_HOLE_SHADER, "black_hole.wgsl", Shader::from_wgsl);
        // Pas de pré-passe : la profondeur est écrite par le shader (celle du centre), pas par la sphère
        app.add_plugins(MaterialPlugin::<BlackHoleMaterial> { prepass_enabled: false, shadows_enabled: false, ..default() })
            .add_systems(Update, attach_lenses)
            .add_systems(PostUpdate, update_lenses.after(bevy::transform::TransformSystem::TransformPropagate));
    }
}

#[derive(ShaderType, Clone, Copy, Debug, Default)]
pub struct HoleParams {
    /// xyz : centre (monde), w : un rayon de Schwarzschild en unités du jeu.
    pub center: Vec4,
    /// Repère du trou noir : x, y dans le plan du disque, z = axe de rotation (monde).
    pub ax: Vec4,
    pub ay: Vec4,
    pub az: Vec4,
    /// x : spin a (unités du shader), y : temps (s), z : rayon de la sphère de lentille, w : éclat.
    pub misc: Vec4,
    /// x : orbite de l'étoile aspirée, y : sa phase, z : 1 si elle existe, w : graine.
    pub star: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct BlackHoleMaterial {
    #[uniform(0)]
    pub p: HoleParams,
    /// La skybox fixe : le fond hors de l'écran.
    #[texture(1, dimension = "cube")]
    #[sampler(2)]
    pub sky: Handle<Image>,
}

impl Material for BlackHoleMaterial {
    fn fragment_shader() -> ShaderRef {
        BLACK_HOLE_SHADER.into()
    }

    /// Lit l'image déjà rendue derrière lui (le vrai fond) : passe « transmissive », après les objets opaques.
    fn reads_view_transmission_texture(&self) -> bool {
        true
    }

    // La sphère se voit de dehors (faces avant) comme de dedans (faces arrière) : le shader garde la bonne
    fn specialize(
        _pipeline: &MaterialPipeline<Self>,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

/// Sphère de lentille d'un trou noir, et la galaxie à qui il appartient.
#[derive(Component)]
pub struct BlackHoleLens {
    pub galaxy_id: u32,
}

/// Un trou noir central qui vient d'être créé reçoit sa sphère de lentille et son matériau.
#[allow(clippy::type_complexity)]
fn attach_lenses(
    mut commands: Commands,
    settings: Res<GameSettings>,
    sky: Res<crate::skybox::SkyState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<BlackHoleMaterial>>,
    mut mesh: Local<Option<Handle<Mesh>>>,
    cores: Query<(Entity, Option<&DistantGalaxyCore>, Has<GalacticCore>, &Transform), (Or<(With<GalacticCore>, With<DistantGalaxyCore>)>, Without<BlackHoleLens>)>,
) {
    for (e, distant, _main, tf) in &cores {
        let gid = distant.map_or(0, |d| d.galaxy_id);
        let Some(g) = settings.galaxies.get(gid as usize) else { continue };
        let unit = unit_of(g.core_radius);
        let sphere = mesh.get_or_insert_with(|| meshes.add(Sphere::new(1.0).mesh().ico(5).unwrap())).clone();
        let mat = materials.add(BlackHoleMaterial { p: params(g, unit, tf.translation, 0.0, gid), sky: sky.image.clone() });
        commands.entity(e).try_insert((
            Mesh3d(sphere),
            MeshMaterial3d(mat),
            Transform::from_translation(tf.translation).with_scale(Vec3::splat(LENS_RADIUS * unit)),
            NotShadowCaster,
            NotShadowReceiver,
            BlackHoleLens { galaxy_id: gid },
        ));
    }
}

/// Paramètres du shader d'un trou noir de la galaxie `g` posé en `center` (monde).
fn params(g: &crate::settings::GalaxyConfig, unit: f32, center: Vec3, t: f32, gid: u32) -> HoleParams {
    // Le disque est dans le plan de la galaxie : son axe de rotation est la normale de la galaxie
    let az = (g.tilt * Vec3::Y).normalize_or(Vec3::Y);
    let ax = (g.tilt * Vec3::X).normalize_or(Vec3::X);
    let ay = az.cross(ax);
    let seed = crate::settings::pseudo_rand(gid.wrapping_mul(977).wrapping_add(13));
    HoleParams {
        center: center.extend(unit),
        ax: ax.extend(0.0),
        ay: ay.extend(0.0),
        az: az.extend(0.0),
        misc: Vec4::new(SPIN * 0.5, t, LENS_RADIUS, 1.0),
        // L'étoile aspirée fait le tour en une dizaine de minutes ; une galaxie sur trois n'en a pas
        star: Vec4::new(STAR_ORBIT, seed * std::f32::consts::TAU + t * 0.01, if seed < 0.67 { 1.0 } else { 0.0 }, seed),
    }
}

/// Chaque image : centre (origine flottante), temps et ciel des trous noirs.
fn update_lenses(
    time: Res<Time>,
    settings: Res<GameSettings>,
    sky: Res<crate::skybox::SkyState>,
    lenses: Query<(&BlackHoleLens, &GlobalTransform, &MeshMaterial3d<BlackHoleMaterial>)>,
    mut materials: ResMut<Assets<BlackHoleMaterial>>,
) {
    // Temps gardé petit (précision du f32 dans le shader)
    let t = (time.elapsed_secs_f64() % 3600.0) as f32;
    for (lens, gt, mat) in &lenses {
        let Some(g) = settings.galaxies.get(lens.galaxy_id as usize) else { continue };
        let Some(m) = materials.get_mut(&mat.0) else { continue };
        m.p = params(g, unit_of(g.core_radius), gt.translation(), t, lens.galaxy_id);
        if m.sky != sky.image {
            m.sky = sky.image.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// L'ombre a la taille de l'ancien trou noir, la lentille va bien au-delà, et le vaisseau stationne
    /// hors du disque d'accrétion (14 unités) mais dans la lentille.
    #[test]
    fn black_hole_scales() {
        let r = 9_000_000.0;
        let u = unit_of(r);
        assert!((u * SHADOW_UNITS - r).abs() < 1.0);
        assert!(LENS_RADIUS * u > 20.0 * r);
        let hover = r * crate::CORE_HOVER_RADII;
        assert!(hover > 15.0 * u && hover < LENS_RADIUS * u, "{} unités", hover / u);
    }
}
