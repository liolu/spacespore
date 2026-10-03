//! Scène de l'éditeur (E1) : la caméra du jeu passe sur un calque de rendu à part (le monde
//! disparaît sans être déchargé), une lumière à elle, le modèle, la grille et la case visée ;
//! caméra orbitale (clic droit = tourner, clic molette = déplacer, molette = zoom), outils à la
//! souris, raccourcis (AZERTY : les touches 1 à 4 sont & é " ').
//!
//! E4 : le modèle est maillé par zone de mouvement (`RigPart`), chaque zone posée par le lecteur
//! d'animations pendant l'aperçu (P) ; le gabarit blanc du bloc en cours de pose suit la souris et
//! joue son repos (`Ghost`) ; contours colorés des zones (rouges si elles traversent le corps).

use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::pbr::{DistanceFog, NotShadowCaster};
use bevy::prelude::*;
use bevy::render::view::RenderLayers;
use bevy::window::PrimaryWindow;

use super::edit::{self, Doc, Tool};
use super::format::Model;
use super::motion::{self, Placement};
use super::Editor;

/// Calque de rendu de l'éditeur.
pub const EDITOR_LAYER: usize = 7;

/// Traits de l'éditeur (grille, case visée), sur son calque.
#[derive(Default, Reflect, GizmoConfigGroup)]
pub struct EditorGizmos;

/// Ce qui appartient à la scène de l'éditeur (retiré en sortant).
#[derive(Component)]
pub struct EditorScene;

/// Une partie du modèle affiché : sa zone de mouvement (0 = corps fixe), une entité par matière.
#[derive(Component)]
pub struct RigPart(pub u8);

/// Le gabarit du bloc en cours de pose (une entité par zone et matière du gabarit).
#[derive(Component)]
pub struct Ghost(pub u8);

/// Rendus de l'éditeur : les quatre matières, le gabarit (blanc, ou rouge s'il manque de place).
#[derive(Resource)]
pub struct EditorMats {
    pub mats: [Handle<StandardMaterial>; 4],
    pub ghost: Handle<StandardMaterial>,
    pub ghost_bad: Handle<StandardMaterial>,
}

/// Rendu de chaque matière (E2) : mate, métal, verre, lumineuse.
pub fn material_of(k: usize) -> StandardMaterial {
    match k {
        1 => StandardMaterial { base_color: Color::WHITE, metallic: 0.85, perceptual_roughness: 0.3, reflectance: 0.6, ..default() },
        2 => StandardMaterial { base_color: Color::srgba(1.0, 1.0, 1.0, 0.42), alpha_mode: AlphaMode::Blend, perceptual_roughness: 0.05, reflectance: 0.5, ..default() },
        // Lumineuse : toujours sa pleine couleur, même dans l'ombre
        3 => StandardMaterial { base_color: Color::WHITE, unlit: true, ..default() },
        _ => StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.9, ..default() },
    }
}

/// Racine de la scène : le coin (0, 0, 0) de la grille.
#[derive(Component)]
pub struct EditorRoot;

/// Caméra orbitale autour d'un point de la grille.
#[derive(Clone, Copy, Debug)]
pub struct OrbitCam {
    pub pivot: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
}

impl Default for OrbitCam {
    fn default() -> Self {
        Self { pivot: Vec3::new(8.0, 8.0, 16.0), yaw: 0.8, pitch: 0.5, distance: 60.0 }
    }
}

impl OrbitCam {
    /// Cadrer tout le modèle (touche F).
    pub fn focus(&mut self, m: &Model) {
        let s = m.size.as_vec3();
        self.pivot = Vec3::new(s.x * 0.5, s.y * 0.35, s.z * 0.5);
        self.distance = s.max_element() * 1.9 + 4.0;
    }

    pub fn transform(&self, anchor: Vec3) -> Transform {
        let rot = Quat::from_euler(EulerRot::YXZ, self.yaw, -self.pitch, 0.0);
        let eye = anchor + self.pivot + rot * Vec3::new(0.0, 0.0, self.distance);
        Transform::from_translation(eye).looking_at(anchor + self.pivot, Vec3::Y)
    }
}

/// Ce que l'éditeur a changé sur la caméra du jeu (rendu en sortant).
#[derive(Default)]
pub struct SavedCamera {
    layers: Option<RenderLayers>,
    fog: Option<DistanceFog>,
    clear: Option<Color>,
    transform: Option<Transform>,
}

pub fn setup_gizmos(mut store: ResMut<GizmoConfigStore>) {
    let (config, _) = store.config_mut::<EditorGizmos>();
    config.render_layers = RenderLayers::layer(EDITOR_LAYER);
    config.line_width = 1.5;
}

#[allow(clippy::too_many_arguments)]
pub fn enter_scene(
    mut commands: Commands,
    mut editor: ResMut<Editor>,
    mut clear: ResMut<ClearColor>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    cam_q: Query<(Entity, &Transform, Option<&RenderLayers>, Option<&DistanceFog>), With<Camera3d>>,
) {
    let Ok((cam, tf, layers, fog)) = cam_q.get_single() else { return };
    // La caméra du jeu ne voit plus que le calque de l'éditeur (sans brouillard)
    editor.saved = SavedCamera { layers: layers.cloned(), fog: fog.cloned(), clear: Some(clear.0), transform: Some(*tf) };
    commands.entity(cam).insert(RenderLayers::layer(EDITOR_LAYER)).remove::<DistanceFog>();
    clear.0 = Color::srgb(0.1, 0.11, 0.16);
    // La scène, devant la caméra : le monde garde sa place (origine flottante, streaming)
    let anchor = tf.translation + *tf.forward() * 200.0;
    let layer = RenderLayers::layer(EDITOR_LAYER);
    commands.spawn((Transform::from_translation(anchor), Visibility::default(), EditorRoot, EditorScene, layer.clone()));
    let ghost = |a: f32, c: Color| StandardMaterial { base_color: c.with_alpha(a), alpha_mode: AlphaMode::Blend, unlit: true, ..default() };
    commands.insert_resource(EditorMats {
        mats: [0, 1, 2, 3].map(|k| materials.add(material_of(k))),
        ghost: materials.add(ghost(0.55, Color::WHITE)),
        ghost_bad: materials.add(ghost(0.55, Color::srgb(1.0, 0.35, 0.3))),
    });
    let _ = &mut meshes;
    commands.spawn((
        DirectionalLight { illuminance: 9_000.0, shadows_enabled: false, ..default() },
        Transform::from_rotation(Quat::from_euler(EulerRot::YXZ, -0.6, -0.9, 0.0)),
        EditorScene,
        layer,
    ));
    if let Some(d) = editor.doc_mut() {
        d.mesh_dirty = true;
    }
}

pub fn exit_scene(
    mut commands: Commands,
    mut editor: ResMut<Editor>,
    mut clear: ResMut<ClearColor>,
    scene: Query<Entity, With<EditorScene>>,
    mut cam_q: Query<(Entity, &mut Transform), With<Camera3d>>,
) {
    for e in &scene {
        commands.entity(e).try_despawn_recursive();
    }
    if let Some(d) = editor.doc_mut() {
        d.end();
    }
    let saved = std::mem::take(&mut editor.saved);
    if let Ok((cam, mut tf)) = cam_q.get_single_mut() {
        match saved.layers {
            Some(l) => commands.entity(cam).insert(l),
            None => commands.entity(cam).remove::<RenderLayers>(),
        };
        if let Some(f) = saved.fog {
            commands.entity(cam).insert(f);
        }
        if let Some(t) = saved.transform {
            *tf = t;
        }
    }
    if let Some(c) = saved.clear {
        clear.0 = c;
    }
}

/// Le curseur est-il sur un panneau de l'éditeur (pas dans la vue 3D) ?
pub fn over_ui(ui: &Query<&Interaction, With<super::panels::Blocks>>) -> bool {
    ui.iter().any(|i| *i != Interaction::None)
}

#[allow(clippy::too_many_arguments)]
pub fn camera_input(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut motion: EventReader<MouseMotion>,
    mut wheel: EventReader<MouseWheel>,
    ui: Query<&Interaction, With<super::panels::Blocks>>,
    mut editor: ResMut<Editor>,
    root: Query<&Transform, (With<EditorRoot>, Without<Camera3d>)>,
    mut cam_q: Query<&mut Transform, With<Camera3d>>,
) {
    let delta: Vec2 = motion.read().map(|m| m.delta).sum();
    let mut scroll: f32 = wheel.read().map(crate::ui::wheel_lines).sum();
    let free = !over_ui(&ui);
    // Pose d'un bloc : molette = tourner, Maj+molette = taille (Ctrl+molette : zoom)
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    if free && scroll != 0.0 && !ctrl {
        let scalable = editor.placing.and_then(|p| editor.blocks.get(p.block)).is_some_and(|b| b.scalable);
        if let Some(p) = editor.placing.as_mut() {
            let step = if scroll > 0.0 { 1 } else { -1 };
            if shift && scalable {
                p.place.scale = (p.place.scale as i32 + step).clamp(1, 4) as u8;
            } else if !shift {
                p.place.turn = (p.place.turn as i32 + step).rem_euclid(4) as u8;
            }
            scroll = 0.0;
            editor.ui_dirty = true;
        }
    }
    if buttons.just_pressed(MouseButton::Right) {
        editor.orbit_ok = free;
    }
    let orbit = editor.orbit_ok;
    let cam = &mut editor.cam;
    if buttons.pressed(MouseButton::Right) && orbit {
        cam.yaw -= delta.x * 0.006;
        cam.pitch = (cam.pitch + delta.y * 0.006).clamp(-1.5, 1.5);
    }
    if buttons.pressed(MouseButton::Middle) {
        let rot = Quat::from_euler(EulerRot::YXZ, cam.yaw, -cam.pitch, 0.0);
        let k = cam.distance * 0.0016;
        cam.pivot += (rot * Vec3::NEG_X * delta.x + rot * Vec3::Y * delta.y) * k;
    }
    if free && scroll != 0.0 {
        cam.distance = (cam.distance * (-scroll * 0.12).exp()).clamp(2.0, 4000.0);
    }
    let (Ok(r), Ok(mut tf)) = (root.get_single(), cam_q.get_single_mut()) else { return };
    *tf = editor.cam.transform(r.translation);
}

/// Case visée par la souris, outils, raccourcis.
#[allow(clippy::too_many_arguments)]
pub fn tools_input(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    viewport: Res<crate::graphics::ViewportScale>,
    ui: Query<&Interaction, With<super::panels::Blocks>>,
    cam_q: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    root: Query<&Transform, With<EditorRoot>>,
    mut editor: ResMut<Editor>,
) {
    if editor.typing() {
        return;
    }
    // ── Raccourcis ──
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    for (k, t) in [(KeyCode::Digit1, Tool::Add), (KeyCode::Digit2, Tool::Remove), (KeyCode::Digit3, Tool::Paint), (KeyCode::Digit4, Tool::Pick)] {
        if keys.just_pressed(k) && !ctrl {
            editor.tool = t;
        }
    }
    if keys.just_pressed(KeyCode::KeyX) && !ctrl && editor.placing.is_some() {
        if let Some(p) = editor.placing.as_mut() {
            p.place.mirror = !p.place.mirror;
        }
    } else if keys.just_pressed(KeyCode::KeyX) && !ctrl {
        editor.mirror = !editor.mirror;
        let on = if editor.mirror { "active" } else { "coupee" };
        editor.say(format!("Symetrie miroir {on} (X)."));
    }
    if keys.just_pressed(KeyCode::KeyP) && !ctrl {
        if editor.preview.is_some() {
            editor.stop_preview();
            editor.say("Apercu arrete.".into());
        } else {
            editor.start_preview("repos");
        }
    }
    if keys.just_pressed(KeyCode::KeyG) {
        editor.grid = !editor.grid;
    }
    if keys.just_pressed(KeyCode::KeyF) {
        editor.focus();
    }
    // Ctrl+Z (touche W en QWERTY physique sur un clavier AZERTY : les deux marchent)
    if ctrl && (keys.just_pressed(KeyCode::KeyW) || keys.just_pressed(KeyCode::KeyZ)) {
        editor.undo();
    }
    if ctrl && keys.just_pressed(KeyCode::KeyY) {
        editor.redo();
    }
    if ctrl && keys.just_pressed(KeyCode::KeyS) {
        editor.save();
    }
    if keys.just_pressed(KeyCode::ArrowLeft) || keys.just_pressed(KeyCode::ArrowRight) {
        let n = editor.docs.len();
        if n > 1 {
            let d = if keys.just_pressed(KeyCode::ArrowLeft) { n - 1 } else { 1 };
            let i = (editor.current + d) % n;
            editor.select(i);
        }
    }

    // ── Case visée ──
    // Pendant l'aperçu, le modèle bouge : pas d'outil
    if editor.overlay.is_some() || over_ui(&ui) || editor.docs.is_empty() || editor.preview.is_some() {
        editor.hover = (None, None);
        end_stroke(&mut editor, &buttons);
        return;
    }
    let (Ok(win), Ok((camera, cam_gt)), Ok(r)) = (windows.get_single(), cam_q.get_single(), root.get_single()) else { return };
    // Souris hors de la fenêtre : la dernière case visée reste
    let Some(cursor) = win.cursor_position() else { return };
    editor.hover = (None, None);
    let Ok(ray) = camera.viewport_to_world(cam_gt, viewport.to_viewport(cursor)) else { return };
    let origin = ray.origin - r.translation;
    let tool = editor.tool;
    let Some(doc) = editor.doc() else { return };
    let (hit, place) = edit::raycast(&doc.model, origin, *ray.direction, 4096);
    // Ajouter : la case vide devant ; les autres outils : la case pleine
    let target = if tool == Tool::Add { place } else { hit };
    editor.hover = (hit, place);

    // ── Souris ──
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    // Pose d'un bloc de mouvement : le gabarit devient des zones
    if let Some(p) = editor.placing {
        if buttons.just_pressed(MouseButton::Left) {
            if let (Some(at), Some(def)) = (place, editor.blocks.get(p.block).cloned()) {
                let mirror = editor.mirror;
                let r = editor.doc_mut().map(|d| d.place_block(&def, at, p.place, mirror));
                editor.placing = None;
                match r {
                    Some(Ok(n)) => editor.say(format!("{} pose : {n} zone(s) de mouvement. Peins, ajoute ou retire des blocs ; P : apercu.", def.name)),
                    Some(Err(e)) => editor.say(e),
                    None => {}
                }
            }
        }
        return;
    }
    if buttons.just_pressed(MouseButton::Left) {
        if shift || tool == Tool::Pick {
            let picked = hit.and_then(|p| editor.doc().and_then(|d| d.model.color_at(p)));
            if let Some(c) = picked {
                editor.set_color(c);
            }
            return;
        }
        if let Some(d) = editor.doc_mut() {
            d.begin();
        }
        editor.last_cell = None;
    }
    if buttons.pressed(MouseButton::Left) && !shift && tool != Tool::Pick {
        // Un trait : une seule action par case survolée
        if target.is_some() && target != editor.last_cell {
            editor.last_cell = target;
            let (color, mirror) = (editor.color, editor.mirror);
            if let Some(d) = editor.doc_mut() {
                d.apply(tool, hit, place, color, mirror);
            }
        }
    }
    end_stroke(&mut editor, &buttons);
}

fn end_stroke(editor: &mut Editor, buttons: &ButtonInput<MouseButton>) {
    if buttons.just_released(MouseButton::Left) {
        if let Some(d) = editor.doc_mut() {
            d.end();
        }
        editor.last_cell = None;
    }
}

/// Remaille le modèle quand il change (ou quand on change d'onglet) : une entité par zone de
/// mouvement et par matière, enfants de la racine.
pub fn update_mesh(
    mut commands: Commands,
    mut editor: ResMut<Editor>,
    old: Query<Entity, With<RigPart>>,
    root: Query<Entity, With<EditorRoot>>,
    mats: Option<Res<EditorMats>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let (Ok(root), Some(mats)) = (root.get_single(), mats) else { return };
    let dirty = editor.doc().is_none_or(|d| d.mesh_dirty);
    if !dirty {
        return;
    }
    for e in &old {
        commands.entity(e).try_despawn_recursive();
    }
    let Some(doc) = editor.doc_mut() else {
        editor.zone_boxes.clear();
        return;
    };
    doc.mesh_dirty = false;
    let parts = edit::build_parts(&doc.model);
    // Boîte de chaque zone (contours)
    let mut boxes = vec![(Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)); doc.model.zones.len()];
    for (p, z) in doc.model.zone_map.iter() {
        if let Some(b) = boxes.get_mut(z as usize - 1) {
            b.0 = b.0.min(p.as_vec3());
            b.1 = b.1.max(p.as_vec3() + Vec3::ONE);
        }
    }
    editor.zone_boxes = boxes;
    let layer = RenderLayers::layer(EDITOR_LAYER);
    commands.entity(root).with_children(|c| {
        for (zone, k, mesh) in parts {
            c.spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(mats.mats[k].clone()), Transform::IDENTITY, NotShadowCaster, RigPart(zone), layer.clone()));
        }
    });
}

/// Lecteur d'animations : pendant l'aperçu, chaque zone prend sa pose (mélange en douceur quand
/// on change d'animation) ; sinon, la pose neutre.
pub fn animate(time: Res<Time>, mut editor: ResMut<Editor>, mut q: Query<(&RigPart, &mut Transform)>) {
    let dt = time.delta_secs();
    if let Some(p) = editor.preview.as_mut() {
        p.t += dt;
        p.blend = (p.blend + dt / 0.35).min(1.0);
    }
    let pose = match editor.doc() {
        Some(d) if editor.preview.is_some() => motion::compose(&d.model, &editor.current_locals()),
        Some(d) => vec![Mat4::IDENTITY; d.model.zones.len() + 1],
        None => Vec::new(),
    };
    for (part, mut tf) in &mut q {
        let m = pose.get(part.0 as usize).copied().unwrap_or(Mat4::IDENTITY);
        let t = Transform::from_matrix(m);
        if *tf != t {
            *tf = t;
        }
    }
    editor.pose = pose;
}

/// Ce que montre le gabarit : le bloc, son orientation, la case visée, la symétrie.
#[derive(Default)]
pub struct GhostState {
    key: Option<(usize, Placement, IVec3, bool)>,
    /// Le gabarit posé dans un modèle vide (ses zones, pour l'animer).
    model: Option<Model>,
    t: f32,
}

/// Le gabarit blanc du bloc en cours de pose : il suit la case visée et joue son repos en boucle ;
/// rouge s'il sort de la grille ou recouvre des blocs.
#[allow(clippy::too_many_arguments)]
pub fn ghost(
    mut commands: Commands,
    time: Res<Time>,
    editor: Res<Editor>,
    mut state: Local<GhostState>,
    mut parts: Query<(Entity, &Ghost, &mut Transform)>,
    root: Query<Entity, With<EditorRoot>>,
    mats: Option<Res<EditorMats>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let (Ok(root), Some(mats)) = (root.get_single(), mats) else { return };
    let key = match (editor.placing, editor.hover.1, editor.doc()) {
        (Some(p), Some(at), Some(_)) => Some((p.block, p.place, at, editor.mirror)),
        _ => None,
    };
    if key != state.key || (key.is_some() && state.model.is_some() && parts.is_empty()) {
        state.key = key;
        state.model = None;
        for (e, _, _) in &parts {
            commands.entity(e).try_despawn_recursive();
        }
        let (Some((block, place, at, mirror)), Some(doc), Some(def)) = (key, editor.doc(), key.and_then(|k| editor.blocks.get(k.0))) else { return };
        let _ = block;
        // Le gabarit posé dans un modèle vide de la même grille
        let mut g = Doc::new(Model::new("gabarit", doc.model.kind, doc.model.category), None);
        g.model.size = doc.model.size;
        if g.place_block(def, at, place, mirror).is_err() {
            return;
        }
        let expected: usize = def.parts.iter().map(|p| motion::part_cells(p).len()).sum::<usize>() * (place.scale.max(1) as usize).pow(3);
        let instances = g.model.zones.len() / def.parts.len().max(1);
        let outside = g.model.voxels.count() < expected * instances;
        let covers = g.model.voxels.iter().any(|(p, _)| doc.model.voxels.get(p) != 0);
        let mat = if outside || covers { mats.ghost_bad.clone() } else { mats.ghost.clone() };
        let layer = RenderLayers::layer(EDITOR_LAYER);
        commands.entity(root).with_children(|c| {
            for (zone, _, mesh) in edit::build_parts(&g.model) {
                c.spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(mat.clone()), Transform::IDENTITY, NotShadowCaster, Ghost(zone), layer.clone()));
            }
        });
        state.model = Some(g.model);
        return;
    }
    // Le gabarit joue son repos
    state.t += time.delta_secs();
    let Some(m) = &state.model else { return };
    let pose = motion::compose(m, &motion::zone_locals(m, &editor.blocks, "repos", state.t));
    for (_, g, mut tf) in &mut parts {
        *tf = Transform::from_matrix(pose.get(g.0 as usize).copied().unwrap_or(Mat4::IDENTITY));
    }
}

/// Couleur du contour d'une zone (rouge : elle traverse le corps pendant l'aperçu).
fn zone_color(i: usize, colliding: bool) -> Color {
    if colliding {
        Color::srgb(1.0, 0.15, 0.1)
    } else {
        Color::hsl((i as f32 * 67.0 + 160.0) % 360.0, 0.85, 0.62)
    }
}

/// Grille du sol, boîte de la grille, plan du miroir, case visée.
pub fn draw(editor: Res<Editor>, root: Query<&Transform, With<EditorRoot>>, mut g: Gizmos<EditorGizmos>) {
    let (Ok(r), Some(doc)) = (root.get_single(), editor.doc()) else { return };
    let o = r.translation;
    let s = doc.model.size.as_vec3();
    if editor.grid {
        let step = (doc.model.size.max_element() / 64).max(1) as usize;
        let line = Color::srgba(1.0, 1.0, 1.0, 0.13);
        for x in (0..=doc.model.size.x as usize).step_by(step) {
            g.line(o + Vec3::new(x as f32, 0.0, 0.0), o + Vec3::new(x as f32, 0.0, s.z), line);
        }
        for z in (0..=doc.model.size.z as usize).step_by(step) {
            g.line(o + Vec3::new(0.0, 0.0, z as f32), o + Vec3::new(s.x, 0.0, z as f32), line);
        }
    }
    // Boîte de la grille
    g.cuboid(Transform::from_translation(o + s * 0.5).with_scale(s), Color::srgba(0.4, 0.7, 1.0, 0.45));
    // Plan du miroir (milieu en x)
    if editor.mirror {
        let x = s.x * 0.5;
        let c = Color::srgba(1.0, 0.4, 0.8, 0.5);
        g.line(o + Vec3::new(x, 0.0, 0.0), o + Vec3::new(x, 0.0, s.z), c);
        g.line(o + Vec3::new(x, 0.0, 0.0), o + Vec3::new(x, s.y, 0.0), c);
        g.line(o + Vec3::new(x, 0.0, s.z), o + Vec3::new(x, s.y, s.z), c);
    }
    // Contours des zones de mouvement (ils suivent leur pose)
    for (i, (lo, hi)) in editor.zone_boxes.iter().enumerate() {
        if lo.x > hi.x {
            continue;
        }
        let m = editor.pose.get(i + 1).copied().unwrap_or(Mat4::IDENTITY);
        let local = Mat4::from_scale_rotation_translation(*hi - *lo + Vec3::splat(0.06), Quat::IDENTITY, (*lo + *hi) * 0.5);
        g.cuboid(Transform::from_matrix(Mat4::from_translation(o) * m * local), zone_color(i, editor.colliding.contains(&i)));
    }
    if editor.placing.is_some() || editor.preview.is_some() {
        return;
    }
    // Case visée (vert : ajouter, rouge : retirer, jaune : peindre, blanc : pipette)
    let (hit, place) = editor.hover;
    let (cell, color) = match editor.tool {
        Tool::Add => (place, Color::srgb(0.3, 1.0, 0.4)),
        Tool::Remove => (hit, Color::srgb(1.0, 0.3, 0.3)),
        Tool::Paint => (hit, Color::srgb(1.0, 0.9, 0.3)),
        Tool::Pick => (hit, Color::WHITE),
    };
    if let Some(c) = cell {
        for p in if editor.mirror { edit::mirrored(&doc.model, c) } else { vec![c] } {
            g.cuboid(Transform::from_translation(o + p.as_vec3() + Vec3::splat(0.5)).with_scale(Vec3::splat(1.04)), color);
        }
    }
}

/// Un nouveau document : cadré, miroir selon le type.
pub fn open_doc(editor: &mut Editor, model: Model, path: Option<std::path::PathBuf>) {
    editor.mirror = edit::mirror_default(model.kind);
    editor.docs.push(Doc::new(model, path));
    let i = editor.docs.len() - 1;
    editor.select(i);
}
