//! Origine flottante : le monde se recentre sur le vaisseau.
//!
//! Les positions des entités sont des `f32` relatives à une origine absolue en `f64`
//! (voir `settings::origin`). Quand le vaisseau s'en éloigne de plus de `REBASE_DIST`, l'origine
//! le rejoint et tout ce qui est « monde » est décalé d'autant : la galaxie peut faire des
//! milliards d'unités sans que le terrain d'une planète, ou le vaisseau, perde en précision.
//!
//! Ce qui est décalé : toutes les entités racines (sans parent, hors interface et caméras 2D),
//! la pose mémorisée par l'atterrissage et la cible de la caméra. Ce qui ne bouge pas : les
//! positions absolues (systèmes, galaxies, trous de ver, joueurs distants), converties à la volée.

use bevy::prelude::*;

use crate::planet::{DistantGalaxyCore, FarStar, GalacticCore};
use crate::settings::{origin, set_origin, GameSettings};
use crate::ship::Ship;
use crate::surface::Surface;
use crate::CameraController;

/// Distance du vaisseau à l'origine au-delà de laquelle on recentre (précision de ~0,01 à 100 000).
pub const REBASE_DIST: f32 = 100_000.0;

pub struct OriginPlugin;

impl Plugin for OriginPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<OriginEpoch>().add_systems(First, rebase_origin);
    }
}

/// Nombre de recentrages effectués : ce qui garde des positions « monde » en mémoire (traînées,
/// caches) les jette quand ce compteur change.
#[derive(Resource, Default)]
pub struct OriginEpoch(pub u32);

#[allow(clippy::type_complexity)]
fn rebase_origin(
    mut epoch: ResMut<OriginEpoch>,
    travel: Res<crate::wormhole::WormholeTravel>,
    settings: Res<GameSettings>,
    mut surface: ResMut<Surface>,
    mut roots: Query<
        (&mut Transform, &mut GlobalTransform, Option<&FarStar>, Option<&GalacticCore>, Option<&DistantGalaxyCore>, Has<Ship>),
        (Without<Parent>, Without<Node>, Without<Camera2d>),
    >,
    mut cam_q: Query<&mut CameraController>,
) {
    // Pendant un voyage en trou de ver, la trajectoire est exprimée dans le repère courant
    if travel.active() {
        return;
    }
    let Some(delta) = roots.iter().find(|r| r.5).map(|r| r.0.translation) else { return };
    if !delta.is_finite() || delta.length() < REBASE_DIST {
        return;
    }

    set_origin(origin() + delta.as_dvec3());
    epoch.0 = epoch.0.wrapping_add(1);

    for (mut tf, mut gt, far_star, core, distant_core, _) in &mut roots {
        // Les positions qui viennent des réglages sont recalculées exactement (pas de dérive)
        let exact = if let Some(fs) = far_star {
            settings.systems.get(fs.sys_idx).map(|s| s.center())
        } else if core.is_some() {
            settings.galaxies.first().map(|g| g.center())
        } else if let Some(dc) = distant_core {
            settings.galaxies.get(dc.galaxy_id as usize).map(|g| g.center())
        } else {
            None
        };
        tf.translation = exact.unwrap_or(tf.translation - delta);
        let mut affine = gt.affine();
        affine.translation = tf.translation.into();
        *gt = GlobalTransform::from(affine);
    }
    for mut ctrl in &mut cam_q {
        ctrl.last_target_pos -= delta;
    }
    surface.shift(delta);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{to_abs, to_local};

    #[test]
    fn absolute_and_world_positions_round_trip_around_a_moving_origin() {
        let before = origin();
        // Une position à 800 millions d'unités du centre, vue depuis une origine toute proche
        let abs = bevy::math::DVec3::new(8.0e8 + 0.25, 1.0e5, -3.0e8);
        set_origin(abs - bevy::math::DVec3::new(120.0, 5.0, -30.0));
        let local = to_local(abs.as_vec3());
        // f32 absolu : ±32 à 8e8, mais la position monde garde la précision de l'origine
        assert!(local.length() < 200.0, "{local:?}");
        let back = to_abs(Vec3::new(120.0, 5.0, -30.0));
        assert!((back - abs).length() < 1e-6);
        set_origin(before);
    }
}
