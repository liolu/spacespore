//! Scène de l'éditeur (E1) : la caméra du jeu passe sur un calque de rendu à part (le monde
//! disparaît sans être déchargé), une lumière à elle, le modèle, la grille et la case visée ;
//! caméra orbitale (clic droit = tourner, clic molette = déplacer, molette = zoom), outils à la
//! souris, raccourcis (AZERTY : les touches 1 à 4 sont & é " ').
//!
//! E3 : maillage par chunk (`ChunkMeshes`, `mesh.rs`), au niveau de détail de sa distance, hors du
//! fil principal ; outils de volume tirés à la souris (molette pendant le tracé : épaisseur),
//! remplissage, sélection (copier, couper, coller, tourner, retourner), coupe (C, Page préc./suiv.).
//!
//! E4 : le modèle est maillé par zone de mouvement (`RigPart`), chaque zone posée par le lecteur
//! d'animations pendant l'aperçu (P) ; le gabarit blanc du bloc en cours de pose suit la souris et
//! joue son repos (`Ghost`) ; contours colorés des zones (rouges si elles traversent le corps).

use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::pbr::{DistanceFog, NotShadowCaster};
use bevy::prelude::*;
use bevy::render::view::RenderLayers;
use bevy::tasks::{block_on, futures_lite::future, AsyncComputeTaskPool, Task};
use bevy::window::PrimaryWindow;
use std::collections::{HashMap, HashSet};

use super::edit::{self, Brush, ClipTransform, Doc, Shape, Tool};
use super::format::CHUNK;
use super::mesh::ChunkJob;
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

/// Ce qu'on va coller (presse-papiers), qui suit la souris.
#[derive(Component)]
pub struct PasteGhost;

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
    /// Décalage du modèle vers la droite de l'écran (fraction de la distance).
    pub shift: f32,
}

impl Default for OrbitCam {
    fn default() -> Self {
        Self { pivot: Vec3::new(8.0, 8.0, 16.0), yaw: 0.8, pitch: 0.5, distance: 60.0, shift: 0.0 }
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
        let look = anchor + self.pivot + rot * Vec3::NEG_X * self.distance * self.shift;
        let eye = look + rot * Vec3::new(0.0, 0.0, self.distance);
        Transform::from_translation(eye).looking_at(look, Vec3::Y)
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
    commands.insert_resource(ChunkMeshes::default());
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
    if scroll != 0.0 && !ctrl {
        if let Some(d) = editor.drag.as_mut() {
            d.extrude = (d.extrude + if scroll > 0.0 { 1 } else { -1 }).clamp(0, 1024);
            scroll = 0.0;
        }
    }
    if free && scroll != 0.0 && !ctrl {
        let scalable = editor.placing.and_then(|p| editor.lib.blocks.get(p.block)).is_some_and(|b| b.scalable);
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
    editor.cam.shift = if editor.race_shown() { 0.24 } else { 0.0 };
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
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    for (k, t) in [
        (KeyCode::Digit1, Tool::Add),
        (KeyCode::Digit2, Tool::Remove),
        (KeyCode::Digit3, Tool::Paint),
        (KeyCode::Digit4, Tool::Pick),
        (KeyCode::Digit5, Tool::Box),
        (KeyCode::Digit6, Tool::Sphere),
        (KeyCode::Digit7, Tool::Cylinder),
        (KeyCode::Digit8, Tool::Line),
        (KeyCode::Digit9, Tool::Fill),
        (KeyCode::Digit0, Tool::Select),
    ] {
        if keys.just_pressed(k) && !ctrl {
            editor.set_tool(t);
        }
    }
    // Sélection : copier, couper, coller, effacer, tourner
    if ctrl && keys.just_pressed(KeyCode::KeyC) {
        editor.copy_selection(false);
    }
    if ctrl && keys.just_pressed(KeyCode::KeyX) {
        editor.copy_selection(true);
    }
    if ctrl && keys.just_pressed(KeyCode::KeyV) {
        editor.start_paste();
    }
    if keys.just_pressed(KeyCode::Delete) {
        editor.delete_selection();
    }
    if keys.just_pressed(KeyCode::KeyR) && !ctrl {
        editor.transform(ClipTransform::Turn(if shift { 0 } else { 1 }));
    }
    // Coupe : C change d'axe (aucune, y, x, z), Page préc. / suiv. la déplace (Maj : de 8)
    if keys.just_pressed(KeyCode::KeyC) && !ctrl {
        editor.cycle_cut();
    }
    for (k, d) in [(KeyCode::PageUp, 1), (KeyCode::PageDown, -1)] {
        if keys.just_pressed(k) {
            editor.move_cut(if shift { d * 8 } else { d });
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
        if buttons.just_released(MouseButton::Left) {
            editor.finish_drag();
        }
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
    let (hit, place) = edit::raycast(&doc.model, &doc.view(), origin, *ray.direction, 8192);
    // Ajouter : la case vide devant ; les autres outils : la case pleine
    let target = if tool == Tool::Add { place } else { hit };
    editor.hover = (hit, place);

    // ── Souris ──
    // Pose d'un bloc de mouvement : le gabarit devient des zones
    if let Some(p) = editor.placing {
        if buttons.just_pressed(MouseButton::Left) {
            if let (Some(at), Some(def)) = (place, editor.lib.blocks.get(p.block).cloned()) {
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
    // Chemin d'un hangar : clic = un point de plus au bout extérieur
    if let Some(i) = editor.path_edit {
        if buttons.just_pressed(MouseButton::Left) {
            if let Some(c) = place.or(hit) {
                if let Some(d) = editor.doc_mut() {
                    d.extend_path(i, c.as_vec3() + Vec3::splat(0.5));
                }
                editor.say("Point ajoute au chemin (Echap : fini).".into());
            }
        }
        return;
    }
    // Coller : le presse-papiers suit la souris, clic = le poser
    if editor.pasting {
        if buttons.just_pressed(MouseButton::Left) {
            editor.paste_here();
        }
        return;
    }
    // Outils de volume : tirer de la case de départ à celle d'arrivée
    if tool.is_shape() && !shift {
        let cell = match (tool, editor.brush) {
            (Tool::Select, _) => hit.or(place),
            (_, Brush::Add) => place,
            _ => hit,
        };
        if buttons.just_pressed(MouseButton::Left) {
            if let Some(c) = cell {
                let normal = match (hit, place) {
                    (Some(h), Some(p)) if (p - h).abs().max_element() == 1 => p - h,
                    _ => IVec3::Y,
                };
                editor.drag = Some(super::Drag { tool, start: c, end: c, normal, extrude: 0 });
            }
        }
        if let (Some(d), Some(c)) = (editor.drag.as_mut(), cell) {
            d.end = c;
        }
        if buttons.just_released(MouseButton::Left) {
            editor.finish_drag();
        }
        return;
    }
    if tool == Tool::Fill && !shift {
        if buttons.just_pressed(MouseButton::Left) {
            let normal = match (hit, place) {
                (Some(h), Some(p)) => p - h,
                _ => IVec3::Y,
            };
            let start = if editor.brush == Brush::Add { place } else { hit };
            if let Some(s) = start {
                editor.flood(s, normal);
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

/// Maillages des chunks affichés (E3) : entités par chunk, niveau de détail, maillages en cours.
#[derive(Resource, Default)]
pub struct ChunkMeshes {
    /// Ce qui est affiché : (rig de la race ?, onglet, nombre d'onglets).
    shown: Option<(bool, usize, usize)>,
    entities: HashMap<IVec3, Vec<Entity>>,
    lod: HashMap<IVec3, i32>,
    pending: HashSet<IVec3>,
    tasks: HashMap<IVec3, (i32, Task<Vec<(u8, usize, Mesh)>>)>,
    lod_timer: f32,
}

impl ChunkMeshes {
    /// Chunks en attente ou en cours de maillage.
    pub fn busy(&self) -> usize {
        self.pending.len() + self.tasks.len()
    }
}

/// Niveau de détail d'un chunk à la distance `d` (cases) : une case de maillage pour 1, 2 ou 4
/// voxels quand un voxel fait moins d'un pixel ; `now` = niveau actuel (écart de 10 % pour ne pas
/// hésiter à la limite).
fn lod_for(d: f32, now: i32) -> i32 {
    let raw = |d: f32| if d > LOD_FAR { 4 } else if d > LOD_NEAR { 2 } else { 1 };
    let want = raw(d);
    if want != now && raw(d * 1.1) == want && raw(d * 0.9) == want { want } else if want == now { now } else { now.clamp(1, 4) }
}

const LOD_NEAR: f32 = 1200.0;
const LOD_FAR: f32 = 2400.0;
/// Chunks maillés tout de suite (pose d'un bloc : sans attendre une image).
const SYNC_CHUNKS: usize = 6;
/// Maillages lancés au plus par image.
const TASKS_PER_FRAME: usize = 48;

fn spawn_chunk(commands: &mut Commands, root: Entity, mats: &EditorMats, meshes: &mut Assets<Mesh>, parts: Vec<(u8, usize, Mesh)>) -> Vec<Entity> {
    let layer = RenderLayers::layer(EDITOR_LAYER);
    let mut out = Vec::with_capacity(parts.len());
    commands.entity(root).with_children(|c| {
        for (zone, k, mesh) in parts {
            out.push(c.spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(mats.mats[k].clone()), Transform::IDENTITY, NotShadowCaster, RigPart(zone), layer.clone())).id());
        }
    });
    out
}

/// Remaille les chunks changés (ou tout, en changeant d'onglet), au bon niveau de détail : les
/// premiers tout de suite, les autres hors du fil principal ; une entité par chunk, zone de
/// mouvement et matière, enfants de la racine.
#[allow(clippy::too_many_arguments)]
pub fn update_mesh(
    mut commands: Commands,
    time: Res<Time>,
    mut editor: ResMut<Editor>,
    root: Query<Entity, With<EditorRoot>>,
    mats: Option<Res<EditorMats>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut st: ResMut<ChunkMeshes>,
) {
    let (Ok(root), Some(mats)) = (root.get_single(), mats) else { return };
    let st = &mut *st;
    // Passage du rig de la race à l'onglet (ou l'inverse) : tout est refait
    let key = (editor.race_shown(), editor.current, editor.docs.len());
    if st.shown != Some(key) {
        st.shown = Some(key);
        for (_, es) in st.entities.drain() {
            for e in es {
                commands.entity(e).try_despawn_recursive();
            }
        }
        st.lod.clear();
        st.pending.clear();
        st.tasks.clear();
        if editor.preview.as_ref().is_some_and(|p| p.on_race != key.0) {
            editor.stop_preview();
        }
        editor.focus();
        if let Some(d) = editor.shown_mut() {
            d.mesh_dirty = true;
        }
    }
    let eye = editor.cam.transform(Vec3::ZERO).translation;
    let Some(doc) = editor.shown_mut() else {
        editor.zone_boxes.clear();
        return;
    };
    let mut boxes_dirty = false;
    if doc.mesh_dirty {
        doc.mesh_dirty = false;
        st.pending.extend(doc.model.voxels.keys());
        st.pending.extend(st.entities.keys().copied());
        doc.dirty_chunks.clear();
        boxes_dirty = true;
    }
    if !doc.dirty_chunks.is_empty() {
        st.pending.extend(doc.dirty_chunks.drain());
        boxes_dirty |= !doc.model.zones.is_empty();
    }
    let dist = |c: IVec3| ((c * CHUNK).as_vec3() + Vec3::splat(CHUNK as f32 * 0.5)).distance(eye);
    // Niveau de détail selon la distance (vérifié 3 fois par seconde)
    st.lod_timer += time.delta_secs();
    if st.lod_timer > 0.33 {
        st.lod_timer = 0.0;
        for (c, now) in st.lod.iter() {
            if lod_for(dist(*c), *now) != *now {
                st.pending.insert(*c);
            }
        }
    }
    let view = doc.view();
    // Les plus proches d'abord ; peu de chunks : tout de suite
    let mut todo: Vec<IVec3> = st.pending.drain().collect();
    todo.sort_by(|a, b| dist(*a).total_cmp(&dist(*b)));
    let sync = todo.len() <= SYNC_CHUNKS && st.tasks.is_empty();
    let pool = AsyncComputeTaskPool::get();
    let mut launched = 0;
    for c in todo {
        let step = lod_for(dist(c), st.lod.get(&c).copied().unwrap_or(1));
        let job = ChunkJob::new(&doc.model, c, step, view);
        if sync {
            let parts = job.run();
            for e in st.entities.remove(&c).unwrap_or_default() {
                commands.entity(e).try_despawn_recursive();
            }
            st.entities.insert(c, spawn_chunk(&mut commands, root, &mats, &mut meshes, parts));
            st.lod.insert(c, step);
        } else if launched < TASKS_PER_FRAME {
            st.tasks.insert(c, (step, pool.spawn(async move { job.run() })));
            launched += 1;
        } else {
            st.pending.insert(c);
        }
    }
    // Maillages finis
    let done: Vec<IVec3> = st.tasks.iter_mut().filter_map(|(c, (_, t))| block_on(future::poll_once(t)).map(|parts| (*c, parts))).map(|(c, parts)| {
        for e in st.entities.remove(&c).unwrap_or_default() {
            commands.entity(e).try_despawn_recursive();
        }
        let es = spawn_chunk(&mut commands, root, &mats, &mut meshes, parts);
        st.entities.insert(c, es);
        c
    }).collect();
    for c in done {
        if let Some((step, _)) = st.tasks.remove(&c) {
            st.lod.insert(c, step);
        }
    }
    st.entities.retain(|_, es| !es.is_empty());
    if boxes_dirty {
        // Boîte de chaque zone (contours)
        let mut boxes = vec![(Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)); doc.model.zones.len()];
        for (p, z) in doc.model.zone_map.iter() {
            if let Some(b) = boxes.get_mut(z as usize - 1) {
                b.0 = b.0.min(p.as_vec3());
                b.1 = b.1.max(p.as_vec3() + Vec3::ONE);
            }
        }
        editor.zone_boxes = boxes;
    }
}

/// Lecteur d'animations : pendant l'aperçu, chaque zone prend sa pose (mélange en douceur quand
/// on change d'animation) ; sinon, la pose neutre.
pub fn animate(time: Res<Time>, mut editor: ResMut<Editor>, mut q: Query<(&RigPart, &mut Transform)>) {
    let dt = time.delta_secs();
    if let Some(p) = editor.preview.as_mut() {
        p.t += dt;
        p.blend = (p.blend + dt / p.blend_secs.max(0.05)).min(1.0);
    }
    let pose = match editor.shown() {
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
        let (Some((block, place, at, mirror)), Some(doc), Some(def)) = (key, editor.doc(), key.and_then(|k| editor.lib.blocks.get(k.0))) else { return };
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
    let pose = motion::compose(m, &motion::zone_locals(m, &editor.lib, "repos", state.t, false));
    for (_, g, mut tf) in &mut parts {
        *tf = Transform::from_matrix(pose.get(g.0 as usize).copied().unwrap_or(Mat4::IDENTITY));
    }
}

/// Le presse-papiers qui suit la souris pendant un collage (maillé une fois ; au-delà de 400 000
/// blocs, seule sa boîte est dessinée).
#[allow(clippy::too_many_arguments)]
pub fn paste_ghost(
    mut commands: Commands,
    editor: Res<Editor>,
    mut built: Local<Option<u64>>,
    mut parts: Query<(Entity, &mut Transform, &mut Visibility), With<PasteGhost>>,
    root: Query<Entity, With<EditorRoot>>,
    mats: Option<Res<EditorMats>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let (Ok(root), Some(mats)) = (root.get_single(), mats) else { return };
    let want = editor.pasting.then_some(editor.clip_rev);
    if *built != want || (want.is_some() && parts.is_empty() && built.is_some()) {
        for (e, _, _) in &parts {
            commands.entity(e).try_despawn_recursive();
        }
        *built = want;
        if let (Some(_), Some(clip)) = (want, editor.clip.as_ref()) {
            if clip.count() <= 400_000 {
                let layer = RenderLayers::layer(EDITOR_LAYER);
                commands.entity(root).with_children(|c| {
                    for (_, _, mesh) in edit::build_parts(&clip.model) {
                        c.spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(mats.ghost.clone()), Transform::IDENTITY, Visibility::Hidden, NotShadowCaster, PasteGhost, layer.clone()));
                    }
                });
            }
        }
        return;
    }
    let at = editor.paste_anchor();
    for (_, mut tf, mut vis) in &mut parts {
        match at {
            Some(a) => {
                tf.translation = a.as_vec3();
                *vis = Visibility::Inherited;
            }
            None => *vis = Visibility::Hidden,
        }
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
    let (Ok(r), Some(doc)) = (root.get_single(), editor.shown()) else { return };
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
    // Hangars : chemin d'entrée (du dehors à la place), place, et le vaisseau qui entre ou sort
    let hangar_seq = editor.preview.as_ref().filter(|p| p.anim == motion::HANGAR_IN || p.anim == motion::HANGAR_OUT).map(|p| (p.anim == motion::HANGAR_IN, p.t));
    for (i, h) in doc.model.hangars.iter().enumerate() {
        let col = if editor.path_edit == Some(i) { Color::srgb(1.0, 0.9, 0.2) } else if h.cargo { Color::srgb(1.0, 0.6, 0.2) } else { Color::srgb(0.4, 0.9, 1.0) };
        let pts: Vec<Vec3> = h.path.iter().map(|p| o + Vec3::from_array(*p)).collect();
        g.linestrip(pts.iter().copied(), col);
        for q in &pts {
            g.sphere(Isometry3d::from_translation(*q), 1.5, col);
        }
        let grid = h.category.grid() as f32;
        let size = Vec3::new(grid * 0.5, grid * 0.25, grid * 0.8);
        let face = |dir: Vec3| Quat::from_rotation_arc(Vec3::Z, dir.normalize_or(Vec3::Z));
        let slot = Vec3::from_array(h.slot);
        g.cuboid(Transform { translation: o + slot + Vec3::Y * size.y * 0.5, rotation: face(Vec3::from_array(h.facing)), scale: size }, col.with_alpha(0.35));
        if let Some((entering, t)) = hangar_seq {
            if let Some((pos, dir)) = motion::hangar_ship(h, entering, t) {
                g.cuboid(Transform { translation: o + pos + Vec3::Y * size.y * 0.5, rotation: face(dir), scale: size }, col);
            }
        }
    }
    // Coupe : le plan
    if let Some(c) = doc.cut {
        let (u, v) = ((c.axis + 1) % 3, (c.axis + 2) % 3);
        let mut a = Vec3::ZERO;
        a[c.axis] = c.pos as f32 + 1.0;
        let corner = |i: f32, j: f32| {
            let mut p = a;
            p[u] = i * s[u];
            p[v] = j * s[v];
            o + p
        };
        let col = Color::srgba(1.0, 0.55, 0.1, 0.8);
        g.linestrip([corner(0.0, 0.0), corner(1.0, 0.0), corner(1.0, 1.0), corner(0.0, 1.0), corner(0.0, 0.0)], col);
    }
    // Sélection
    if let Some((lo, hi)) = editor.selection {
        let size = (hi - lo + IVec3::ONE).as_vec3();
        g.cuboid(Transform::from_translation(o + lo.as_vec3() + size * 0.5).with_scale(size + Vec3::splat(0.08)), Color::srgb(1.0, 0.85, 0.2));
    }
    // Collage : la boîte du presse-papiers sous la souris
    if let (true, Some(clip), Some(at)) = (editor.pasting, editor.clip.as_ref(), editor.paste_anchor()) {
        let size = clip.model.size.as_vec3();
        g.cuboid(Transform::from_translation(o + at.as_vec3() + size * 0.5).with_scale(size + Vec3::splat(0.08)), Color::srgb(0.3, 1.0, 0.8));
    }
    // Tracé en cours
    if let Some(shape) = editor.drag.and_then(|d| d.shape()) {
        let col = match editor.drag.map(|d| d.tool) {
            Some(Tool::Select) => Color::srgb(1.0, 0.85, 0.2),
            _ => match editor.brush {
                Brush::Add => Color::srgb(0.3, 1.0, 0.4),
                Brush::Remove => Color::srgb(1.0, 0.3, 0.3),
                Brush::Paint => Color::srgb(1.0, 0.9, 0.3),
            },
        };
        match shape {
            Shape::Sphere { c, r } => {
                g.sphere(Isometry3d::from_translation(o + c.as_vec3() + Vec3::splat(0.5)), r + 0.5, col);
            }
            Shape::Line { a, b } => {
                g.line(o + a.as_vec3() + Vec3::splat(0.5), o + b.as_vec3() + Vec3::splat(0.5), col);
            }
            _ => {
                let (lo, hi) = shape.bounds();
                let size = (hi - lo + IVec3::ONE).as_vec3();
                g.cuboid(Transform::from_translation(o + lo.as_vec3() + size * 0.5).with_scale(size + Vec3::splat(0.04)), col);
            }
        }
    }
    if editor.placing.is_some() || editor.preview.is_some() || editor.pasting || editor.drag.is_some() {
        return;
    }
    // Case visée (vert : ajouter, rouge : retirer, jaune : peindre, blanc : pipette)
    let (hit, place) = editor.hover;
    let (cell, color) = match (editor.tool, editor.brush) {
        (Tool::Add, _) => (place, Color::srgb(0.3, 1.0, 0.4)),
        (Tool::Remove, _) => (hit, Color::srgb(1.0, 0.3, 0.3)),
        (Tool::Paint, _) => (hit, Color::srgb(1.0, 0.9, 0.3)),
        (Tool::Pick, _) | (Tool::Select, _) => (hit, Color::WHITE),
        (_, Brush::Add) => (place, Color::srgb(0.3, 1.0, 0.4)),
        (_, Brush::Remove) => (hit, Color::srgb(1.0, 0.3, 0.3)),
        (_, Brush::Paint) => (hit, Color::srgb(1.0, 0.9, 0.3)),
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
