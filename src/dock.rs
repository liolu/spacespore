//! Amarrage dans les hangars des porte-vaisseaux (E7, §5.1 de `ROADMAP-0.12-editeur.md`).
//!
//! Dans l'espace, près du vaisseau d'un autre joueur dont le modèle a un hangar libre à notre
//! taille (Q8 : croiseur = chasseurs ; capital = chasseurs, corvettes, cargos jusqu'à la frégate),
//! H lance l'**entrée** : la porte s'ouvre, notre vaisseau suit le chemin du hangar jusqu'à sa place
//! (à la taille relative des deux modèles), la porte se ferme ; amarrés, nous suivons le
//! porte-vaisseau. H de nouveau : la **sortie**, puis le vaisseau est rendu au pilote. L'état part
//! sur le réseau (`net::Net::dock`) : chez tous, la porte s'ouvre et le vaisseau entre.

use bevy::prelude::*;

use crate::editeur::format::ShipCategory;
use crate::editeur::motion::{self, HANGAR_SECS};
use crate::models::{fit_transform, Fit, GameModels};
use crate::net::Net;
use crate::net_models::DockState;
use crate::ship::Ship;

pub struct DockPlugin;

impl Plugin for DockPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (test_carrier, dock_keys.run_if(crate::editeur::in_game)).chain())
            .add_systems(PostUpdate, follow_carrier.before(bevy::transform::TransformSystem::TransformPropagate));
    }
}

/// Touche d'amarrage.
pub const DOCK_KEY: KeyCode = KeyCode::KeyH;

/// Rang d'une catégorie (du chasseur au capital).
fn rank(c: ShipCategory) -> usize {
    ShipCategory::ALL.iter().position(|x| *x == c).unwrap_or(0)
}

/// Un hangar libre du porte-vaisseau `carrier` (joueur) pour un vaisseau de catégorie `mine` :
/// son index. Libre = aucun autre joueur n'y est.
pub fn free_hangar(net: &Net, models: &GameModels, carrier: u32, mine: ShipCategory) -> Option<u8> {
    let peer = net.peers.get(&carrier)?;
    let l = models.peek(&peer.look.ship_key())?;
    let taken = |i: usize| net.peers.values().any(|p| p.look.dock.is_some_and(|d| d.carrier == carrier && d.hangar as usize == i));
    l.model.hangars.iter().enumerate().find(|(i, h)| rank(h.category) >= rank(mine) && !taken(*i)).map(|(i, _)| i as u8)
}

/// Catégorie de mon vaisseau (son modèle ; chasseur par défaut).
fn my_category(models: &GameModels) -> ShipCategory {
    models.peek(&models.ship).and_then(|l| l.model.category).unwrap_or(ShipCategory::Chasseur)
}

/// H : s'amarrer au porte-vaisseau le plus proche, ou en sortir.
#[allow(clippy::too_many_arguments)]
fn dock_keys(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    menu: Res<crate::ui::MenuState>,
    panel: Res<crate::net_ui::NetPanel>,
    surface: Res<crate::surface::Surface>,
    models: Res<GameModels>,
    mut net: ResMut<Net>,
    ship_q: Query<&Transform, With<Ship>>,
) {
    let now = time.elapsed_secs_f64();
    let dt = time.delta_secs();
    // La séquence avance
    if let Some(d) = net.dock.as_mut() {
        d.t += dt;
        if d.phase == 0 && d.t >= HANGAR_SECS {
            *d = DockState { phase: 1, t: 0.0, ..*d };
        } else if d.phase == 2 && d.t >= HANGAR_SECS {
            net.dock = None;
            net.notify("Sorti du hangar.", now);
        }
    }
    // Le porte-vaisseau est parti (ou détruit) : on est libéré
    if let Some(d) = net.dock {
        if net.peers.get(&d.carrier).is_none_or(|p| p.status.hp == 0) {
            net.dock = None;
        }
    }
    if !keys.just_pressed(DOCK_KEY) || menu.open || panel.focus.is_some() || surface.active() {
        return;
    }
    match net.dock {
        Some(d) if d.phase == 1 => {
            net.dock = Some(DockState { phase: 2, t: 0.0, ..d });
            net.notify("Sortie du hangar...", now);
        }
        Some(_) => {}
        None => {
            let Ok(me) = ship_q.get_single() else { return };
            let mine = my_category(&models);
            // Le plus proche à portée (quelques longueurs de son modèle)
            let best = net
                .peers
                .iter()
                .filter(|(_, p)| p.status.hp > 0)
                .filter_map(|(id, p)| {
                    let l = models.peek(&p.look.ship_key())?;
                    let reach = me.scale.x * crate::models::icon_length(l) * 6.0;
                    let d = p.pos().distance(me.translation);
                    (d < reach && !l.model.hangars.is_empty()).then_some((*id, d))
                })
                .min_by(|a, b| a.1.total_cmp(&b.1));
            let Some((carrier, _)) = best else {
                net.notify("Aucun porte-vaisseau a portee (il faut le vaisseau d'un autre joueur avec un hangar).", now);
                return;
            };
            match free_hangar(&net, &models, carrier, mine) {
                Some(h) => {
                    net.dock = Some(DockState { carrier, hangar: h, phase: 0, t: 0.0 });
                    let cargo = net.peers.get(&carrier).and_then(|p| models.peek(&p.look.ship_key())).and_then(|l| l.model.hangars.get(h as usize)).is_some_and(|x| x.cargo);
                    net.notify(if cargo { "Entree dans la soute a cargos... (dechargement : avec l'economie, 0.14)" } else { "Entree dans le hangar..." }, now);
                }
                None => net.notify("Pas de hangar libre a votre taille dans ce vaisseau.", now),
            }
        }
    }
}

/// Tests (développement) : `SPACESPORE_TEST_PEER=marcheur` (un centaure à pied à côté du nôtre) ou
/// `croiseur|capital` met à 3 s un faux joueur avec ce
/// vaisseau devant le nôtre ; `SPACESPORE_TEST_DOCK=<s>` lance l'amarrage à cet instant.
fn test_carrier(
    time: Res<Time>,
    models: Res<GameModels>,
    surface: Res<crate::surface::Surface>,
    mut net: ResMut<Net>,
    ship_q: Query<&Transform, With<Ship>>,
    mut step: Local<u8>,
) {
    let Ok(kind) = std::env::var("SPACESPORE_TEST_PEER") else { return };
    let t = time.elapsed_secs_f64();
    let Ok(me) = ship_q.get_single() else { return };
    // `marcheur` : un centaure qui marche à côté de notre personnage
    if kind == "marcheur" {
        if let Some((body, mut local, _)) = surface.walker_state() {
            local.translation += local.rotation * Vec3::X * local.scale.x * 3.0;
            let walk = crate::net_models::WalkState::new(body, local, "trot");
            let look = crate::net_models::Looks { chr: Some(crate::models::ModelKey::Default("perso:centaure".into())), walk, ..Default::default() };
            net.test_peer(9998, "Centaure", me.translation, me.rotation, look);
        }
        return;
    }
    if *step == 0 && t > 3.0 {
        *step = 1;
        let look = crate::net_models::Looks { ship: Some(crate::models::ModelKey::Default(format!("vaisseau:{kind}"))), ss: "vol".into(), ..Default::default() };
        let pos = me.translation + me.forward() * me.scale.x * 40.0 + me.right() * me.scale.x * 25.0;
        net.test_peer(9999, "Porte-vaisseau", pos, me.rotation, look);
    }
    // Le faux joueur reste près de nous (il ne se déplace pas tout seul)
    if let Some(at) = std::env::var("SPACESPORE_TEST_DOCK").ok().and_then(|s| s.parse::<f64>().ok()) {
        if *step == 1 && t > at {
            if let Some(h) = free_hangar(&net, &models, 9999, my_category(&models)) {
                *step = 2;
                net.dock = Some(DockState { carrier: 9999, hangar: h, phase: 0, t: 0.0 });
            }
        }
    }
}

/// Pendant un amarrage, notre vaisseau suit le hangar du porte-vaisseau (la caméra le suit).
#[allow(clippy::type_complexity)]
fn follow_carrier(
    net: Res<Net>,
    models: Res<GameModels>,
    carriers: Query<(&Transform, &crate::net::RemoteShip), Without<Ship>>,
    mut ship_q: Query<&mut Transform, (With<Ship>, Without<Camera3d>)>,
    mut cam_q: Query<&mut Transform, (With<Camera3d>, Without<Ship>, Without<crate::net::RemoteShip>)>,
) {
    let Some(d) = net.dock else { return };
    let Some((carrier_tf, _)) = carriers.iter().find(|(_, r)| r.id == d.carrier) else { return };
    let (Some(peer), Ok(mut ship)) = (net.peers.get(&d.carrier), ship_q.get_single_mut()) else { return };
    let (Some(lc), Some(lm)) = (models.peek(&peer.look.ship_key()), models.peek(&models.ship)) else { return };
    let Some(h) = lc.model.hangars.get(d.hangar as usize) else { return };
    // Repère du modèle du porte-vaisseau -> monde
    let to_world = carrier_tf.compute_matrix() * fit_transform(lc, Fit::Ship).compute_matrix();
    let (pos, dir) = match d.phase {
        0 => motion::hangar_ship(h, true, d.t),
        2 => motion::hangar_ship(h, false, d.t),
        _ => Some((Vec3::from_array(h.slot), Vec3::from_array(h.facing))),
    }
    .unwrap_or((Vec3::from_array(h.slot), Vec3::from_array(h.facing)));
    // Notre taille : celle de notre modèle dans les voxels du porte-vaisseau (même échelle)
    let per_voxel = to_world.transform_vector3(Vec3::X).length();
    let scale = lm.size().max_element() * per_voxel / crate::models::icon_length(lm);
    let lift = Vec3::Y * lm.size().y * 0.5;
    let world_pos = to_world.transform_point3(pos + lift);
    let world_dir = to_world.transform_vector3(dir).normalize_or(Vec3::NEG_Z);
    let up = to_world.transform_vector3(Vec3::Y).normalize_or(Vec3::Y);
    let before = ship.translation;
    *ship = Transform::from_translation(world_pos).looking_to(world_dir, up).with_scale(Vec3::splat(scale));
    if let Ok(mut cam) = cam_q.get_single_mut() {
        cam.translation += ship.translation - before;
    }
}

/// L'animation d'un porte-vaisseau pendant qu'un vaisseau entre ou sort d'un de ses hangars :
/// (séquence, instant), à jouer à la place de son état.
pub fn carrier_sequence(net: &Net, carrier: u32) -> Option<(&'static str, f32)> {
    let mine = net.dock.filter(|d| d.carrier == carrier);
    let theirs = net.peers.values().filter_map(|p| p.look.dock).find(|d| d.carrier == carrier);
    let d = mine.or(theirs)?;
    match d.phase {
        0 => Some((motion::HANGAR_IN, d.t)),
        2 => Some((motion::HANGAR_OUT, d.t)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn categories_are_ranked() {
        assert!(rank(ShipCategory::Capital) > rank(ShipCategory::Fregate));
        assert_eq!(rank(ShipCategory::Chasseur), 0);
    }
}
