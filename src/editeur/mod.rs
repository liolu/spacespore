//! Éditeur de modèles voxel (0.12, `roadmaps/fait/ROADMAP-0.12-editeur.md`) : personnages, vaisseaux, objets.
//!
//! - E0 : l'état du jeu `AppState` (Jeu / Editeur : en éditeur, le jeu ne lit plus le clavier ni la
//!   souris), ouverture à la **création du personnage** (premier lancement), depuis le menu et par
//!   `/editeur` ; le format `.ssvox` (`format.rs`), la bibliothèque `saves/modeles/`, l'import de
//!   Pixel World (`import.rs`).
//! - E1 : l'éditeur (`edit.rs` : outils, miroir, annuler, rayon, maillage ; `view.rs` : scène et
//!   caméra ; `panels.rs` : interface), porté de `VoxelEditorManager.cs`.
//! - E2 : palette OKLCH et matières (`palette.rs`).
//! - E3 : grilles jusqu'à 1024³ : chunks partagés (`format.rs`), annuler par chunk, outils de volume,
//!   remplissage, sélection, calques et coupe (`edit.rs`), maillage glouton par chunk hors du fil
//!   principal avec niveaux de détail (`mesh.rs`, `view::ChunkMeshes`).
//! - E4 : blocs de mouvement (`motion.rs` : blocs en données, placement, lecteur d'animations ;
//!   `Doc::place_block` ; gabarit, zones et aperçu dans `view.rs`).
//! - E5 : races (`races.rs`, 23 familles) et bibliothèque d'animations (`motion::Library`) ; au
//!   choix de la race, son rig s'anime à côté de la fenêtre « Nouveau modèle ».

pub mod custom;
pub mod defaults;
pub mod edit;
pub mod format;
pub mod import;
pub mod mesh;
pub mod motion;
pub mod palette;
pub mod races;
pub mod panels;
pub mod view;
pub mod vox;

use bevy::prelude::*;
use bevy::tasks::{block_on, futures_lite::future, AsyncComputeTaskPool, Task};
use std::path::{Path, PathBuf};

use edit::{Brush, Clip, ClipTransform, Doc, Shape, Tool};
use mesh::Cut;
use format::{Model, ModelKind, PaletteEntry, ShipCategory};

/// Le jeu ou l'éditeur.
#[derive(States, Default, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum AppState {
    #[default]
    Jeu,
    Editeur,
}

pub struct EditeurPlugin;

impl Plugin for EditeurPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AppState>()
            .init_resource::<Editor>()
            .add_event::<OpenEditor>()
            .init_gizmo_group::<view::EditorGizmos>()
            .init_resource::<view::ChunkMeshes>()
            .add_systems(Startup, (open_on_first_launch, view::setup_gizmos))
            .add_systems(Update, (open_editor, menu_button))
            .add_systems(OnEnter(AppState::Editeur), (view::enter_scene, on_enter).chain())
            .add_systems(OnExit(AppState::Editeur), (view::exit_scene, panels::show_game_ui))
            .add_systems(
                Update,
                (panels::typing, panels::escape, panels::actions, panels::replace_on_right_click, panels::scroll_panels, view::camera_input, view::tools_input, view::update_mesh, view::follow_light, view::animate, view::ghost, view::paste_ghost, panels::rebuild, panels::live_texts, view::draw)
                    .chain()
                    .run_if(in_state(AppState::Editeur)),
            )
            .add_systems(PostUpdate, panels::hide_game_ui.run_if(in_state(AppState::Editeur)))
            .add_systems(Update, first_character.run_if(in_state(AppState::Editeur)))
            .add_systems(Update, test_capture);
    }
}

/// Le jeu tourne (pas l'éditeur) : condition des systèmes qui lisent le clavier et la souris.
pub fn in_game(state: Option<Res<State<AppState>>>) -> bool {
    state.is_none_or(|s| *s.get() == AppState::Jeu)
}

/// Ouvrir l'éditeur sur un type de modèle.
#[derive(Event)]
pub struct OpenEditor(pub ModelKind);

/// Bouton « Éditeur de modèles » du menu du jeu (`ui.rs`).
#[derive(Component)]
pub struct EditorMenuButton;

/// Un modèle de la bibliothèque.
#[derive(Clone, Debug)]
pub struct LibraryEntry {
    pub path: PathBuf,
    pub name: String,
    pub kind: ModelKind,
    pub voxels: usize,
    pub bytes: usize,
}

/// Fenêtre ouverte au milieu de l'éditeur.
#[derive(Clone, Debug, PartialEq)]
pub enum Overlay {
    New,
    Library,
    /// Nom en cours de saisie.
    Rename(String),
    /// Couleur libre : code hexadécimal en cours de saisie.
    Hex(String),
    Tags,
}

/// Aperçu ▶ (touche P) : l'animation jouée, son temps, et la pose d'avant pour un changement en
/// douceur.
#[derive(Clone, Debug, Default)]
pub struct Preview {
    pub anim: String,
    pub t: f32,
    pub from: Vec<motion::Pose>,
    /// Avancement du mélange depuis `from` (0 à 1).
    pub blend: f32,
    /// Aperçu du rig de la race (fenêtre « Nouveau modèle »), pas de l'onglet.
    pub on_race: bool,
    /// Durée du passage depuis la pose d'avant (s) : plus longue entre deux états du vaisseau.
    pub blend_secs: f32,
    /// Arrêtée sur un instant (édition des images clés, E8).
    pub paused: bool,
}

/// Un outil de volume tiré à la souris : départ, arrivée, normale de la face de départ,
/// épaisseur ajoutée à la molette.
#[derive(Clone, Copy, Debug)]
pub struct Drag {
    pub tool: Tool,
    pub start: IVec3,
    pub end: IVec3,
    pub normal: IVec3,
    pub extrude: i32,
}

impl Drag {
    /// Le volume tracé (une sélection est une boîte).
    pub fn shape(&self) -> Option<Shape> {
        let (a, b, n, e) = (self.start, self.end, self.normal, self.extrude);
        let axis = if n.x != 0 { 0 } else if n.y != 0 { 1 } else { 2 };
        let up = n[axis].signum();
        Some(match self.tool {
            Tool::Box | Tool::Select => {
                let (mut lo, mut hi) = (a.min(b), a.max(b));
                if up > 0 {
                    hi[axis] += e;
                } else {
                    lo[axis] -= e;
                }
                Shape::Box { a: lo, b: hi }
            }
            Tool::Sphere => Shape::Sphere { c: a, r: (b - a).as_vec3().length() + e as f32 },
            Tool::Cylinder => {
                let mut d = (b - a).as_vec3();
                let along = d[axis].abs() as i32;
                d[axis] = 0.0;
                Shape::Cylinder { c: a, axis, r: d.length(), h: if up < 0 { -1 } else { 1 } * (1 + along + e) }
            }
            Tool::Line => Shape::Line { a, b },
            _ => return None,
        })
    }
}

/// Bloc de mouvement en cours de pose : son index dans `Editor::lib.blocks` et son orientation.
#[derive(Clone, Copy, Debug)]
pub struct Placing {
    pub block: usize,
    pub place: motion::Placement,
}

/// L'éditeur : onglets ouverts, outil, couleur, caméra, fenêtres.
#[derive(Resource)]
pub struct Editor {
    pub docs: Vec<Doc>,
    pub current: usize,
    pub tool: Tool,
    pub color: PaletteEntry,
    /// Les 16 dernières couleurs choisies.
    pub recent: Vec<PaletteEntry>,
    pub saturation: palette::Saturation,
    /// Palette thématique affichée.
    pub theme: usize,
    /// Le clic droit en cours a commencé dans la vue (tourner) et pas sur un panneau.
    pub orbit_ok: bool,
    /// Défilement du panneau de gauche (gardé quand l'interface est reconstruite).
    pub scroll: [f32; 3],
    pub mirror: bool,
    pub grid: bool,
    /// Lumière du jeu (un soleil fixe) au lieu de la lumière d'atelier qui suit la caméra.
    pub game_light: bool,
    pub cam: view::OrbitCam,
    /// Case pleine visée, case vide devant elle.
    pub hover: (Option<IVec3>, Option<IVec3>),
    pub last_cell: Option<IVec3>,
    /// Trait de l'outil Ajouter : le plan du premier bloc (case vide de départ, normale de la face
    /// visée). En glissant, on ne pose que sur ce plan, jamais sur un bloc posé pendant le trait.
    pub add_plane: Option<(IVec3, IVec3)>,
    pub overlay: Option<Overlay>,
    pub message: String,
    /// Ouvert pour la création du personnage (premier lancement).
    pub welcome: bool,
    pub library: Vec<LibraryEntry>,
    pub pending_delete: Option<usize>,
    pub new_kind: ModelKind,
    pub new_race: Option<usize>,
    pub new_category: ShipCategory,
    pub new_size: u32,
    pub saved: view::SavedCamera,
    pub ui_dirty: bool,
    pub hidden_ui: Vec<Entity>,
    /// Poids du fichier et nombre de blocs (recalculés hors du fil principal après un changement).
    pub size_bytes: Option<usize>,
    pub voxel_count: usize,
    size_tick: u32,
    size_rev: u64,
    size_task: Option<Task<(usize, usize)>>,
    /// Outils de volume : ajouter, retirer ou peindre ; tracé en cours.
    pub brush: Brush,
    pub drag: Option<Drag>,
    /// Sélection (boîte comprise), presse-papiers (et son numéro), collage en cours.
    pub selection: Option<(IVec3, IVec3)>,
    pub clip: Option<Clip>,
    pub clip_rev: u64,
    pub pasting: bool,
    /// Poussée simulée dans l'aperçu (0 à 1) ; hangar dont on prolonge le chemin.
    pub thrust: f32,
    pub path_edit: Option<usize>,
    /// Mode avancé (E8) : zone choisie, animation du modèle éditée et instant de la frise, pivot
    /// à poser d'un clic, animation à renommer.
    pub advanced: bool,
    pub sel_zone: Option<usize>,
    pub edit_anim: Option<String>,
    pub cursor: f32,
    pub pivot_pick: bool,
    pub rename_anim: Option<String>,
    /// Échap déjà utilisé cette image (fermer la saisie du nom).
    pub escape_used: bool,
    /// Blocs de mouvement, animations, races (ceux du jeu, puis ceux de `saves/editeur/`).
    pub lib: motion::Library,
    /// Le rig de la race choisie, animé pendant la fenêtre « Nouveau modèle ».
    pub race_view: Option<Doc>,
    /// Membres optionnels cochés pour la race choisie.
    pub new_options: Vec<bool>,
    pub placing: Option<Placing>,
    pub preview: Option<Preview>,
    /// Pose actuelle des zones (repère du modèle, index 0 = corps fixe).
    pub pose: Vec<Mat4>,
    /// Zones qui traversent le corps pendant l'aperçu.
    pub colliding: Vec<usize>,
    /// Boîte de chaque zone au repos (min, max), pour son contour.
    pub zone_boxes: Vec<(Vec3, Vec3)>,
}

impl Default for Editor {
    fn default() -> Self {
        Self {
            docs: Vec::new(),
            current: 0,
            tool: Tool::Add,
            color: panels::default_color(),
            recent: Vec::new(),
            saturation: palette::Saturation::Vif,
            theme: palette::THEMES.len() - 1,
            orbit_ok: true,
            scroll: [0.0; 3],
            mirror: true,
            grid: true,
            game_light: false,
            cam: view::OrbitCam::default(),
            hover: (None, None),
            last_cell: None,
            add_plane: None,
            overlay: None,
            message: String::new(),
            welcome: false,
            library: Vec::new(),
            pending_delete: None,
            new_kind: ModelKind::Personnage,
            new_race: None,
            new_category: ShipCategory::Chasseur,
            new_size: 32,
            saved: Default::default(),
            ui_dirty: true,
            hidden_ui: Vec::new(),
            size_bytes: None,
            voxel_count: 0,
            size_tick: 0,
            size_rev: u64::MAX,
            size_task: None,
            brush: Brush::Add,
            drag: None,
            selection: None,
            clip: None,
            clip_rev: 0,
            pasting: false,
            thrust: 0.6,
            path_edit: None,
            advanced: false,
            sel_zone: None,
            edit_anim: None,
            cursor: 0.0,
            pivot_pick: false,
            rename_anim: None,
            escape_used: false,
            lib: motion::Library::default(),
            race_view: None,
            new_options: Vec::new(),
            placing: None,
            preview: None,
            pose: Vec::new(),
            colliding: Vec::new(),
            zone_boxes: Vec::new(),
        }
    }
}

impl Editor {
    pub fn doc(&self) -> Option<&Doc> {
        self.docs.get(self.current)
    }

    pub fn doc_mut(&mut self) -> Option<&mut Doc> {
        self.docs.get_mut(self.current)
    }

    /// Une saisie de texte est en cours (les raccourcis ne comptent pas).
    pub fn typing(&self) -> bool {
        matches!(self.overlay, Some(Overlay::Rename(_)) | Some(Overlay::Hex(_)))
    }

    /// Le modèle affiché : le rig de la race pendant son choix, sinon l'onglet.
    pub fn shown(&self) -> Option<&Doc> {
        if self.race_shown() { self.race_view.as_ref() } else { self.doc() }
    }

    pub fn shown_mut(&mut self) -> Option<&mut Doc> {
        if self.race_shown() { self.race_view.as_mut() } else { self.doc_mut() }
    }

    /// Le rig de la race est-il affiché (choix de la race en cours) ?
    pub fn race_shown(&self) -> bool {
        self.overlay == Some(Overlay::New) && self.new_kind == ModelKind::Personnage && self.race_view.is_some()
    }

    /// Choisir une race (ou changer ses options) : son rig s'anime à côté.
    pub fn choose_race(&mut self, r: usize, options: Option<Vec<bool>>) {
        let Some(race) = self.lib.races.get(r).cloned() else { return };
        self.new_race = Some(r);
        self.new_options = options.unwrap_or_else(|| race.default_options());
        let anim = self.preview.as_ref().filter(|p| p.on_race).map(|p| p.anim.clone());
        self.stop_preview();
        self.race_view = Some(Doc::new(race.build(&race.name, &self.new_options), None));
        if let Some(m) = self.race_view.as_ref().map(|d| d.model.clone()) {
            self.cam.focus(&m);
        }
        let anim = anim.filter(|a| self.race_view.as_ref().is_some_and(|d| motion::model_anims(&d.model, &self.lib).contains(a)));
        self.start_preview(&anim.unwrap_or(race.preview));
        self.ui_dirty = true;
    }

    /// Blocs de mouvement proposés pour le modèle ouvert (index dans `lib.blocks`).
    pub fn blocks_for_doc(&self) -> Vec<usize> {
        let Some(d) = self.doc() else { return Vec::new() };
        let kind = d.model.kind.name().to_lowercase();
        (0..self.lib.blocks.len()).filter(|i| self.lib.blocks[*i].kinds.is_empty() || self.lib.blocks[*i].kinds.iter().any(|k| k.to_lowercase() == kind)).collect()
    }

    /// Choisir un bloc à poser (le gabarit suit la souris) ; le même bloc : arrêter.
    pub fn pick_block(&mut self, i: usize) {
        if self.placing.is_some_and(|p| p.block == i) {
            self.placing = None;
            self.say("Pose annulee.".into());
            return;
        }
        self.stop_preview();
        self.placing = Some(Placing { block: i, place: motion::Placement { turn: 0, mirror: false, scale: 1 } });
        let scale = if self.lib.blocks.get(i).is_some_and(|b| b.scalable) { "   Maj+molette : taille" } else { "" };
        self.say(format!("Clic : poser   Molette : tourner{scale}   X : miroir   Echap : annuler   (Ctrl+molette : zoom)"));
    }

    /// Lance l'aperçu (ou change d'animation) et vérifie les collisions.
    pub fn start_preview(&mut self, anim: &str) {
        self.placing = None;
        let from = match &self.preview {
            Some(_) => self.current_locals(),
            None => Vec::new(),
        };
        let on_race = self.race_shown();
        let Some(d) = self.shown() else { return };
        if d.model.zones.is_empty() {
            self.say("Aucune zone de mouvement : pose d'abord un bloc de mouvement.".into());
            return;
        }
        let colliding = motion::collisions(&d.model, &self.lib, anim);
        let names: Vec<String> = colliding.iter().filter_map(|z| d.model.zones.get(*z).map(|z| z.name.clone())).collect();
        self.colliding = colliding;
        let blend_secs = if anim.starts_with(motion::STATE_PREFIX) { 1.5 } else { 0.35 };
        self.preview = Some(Preview { anim: anim.to_string(), t: 0.0, blend: if from.is_empty() { 1.0 } else { 0.0 }, from, on_race, blend_secs, paused: false });
        if on_race {
            self.ui_dirty = true;
            return;
        }
        self.say(if names.is_empty() {
            format!("Apercu : {}. P : arreter.", motion::anim_label(&self.lib, anim))
        } else {
            format!("Attention : {} traverse le corps pendant \"{}\" (en rouge).", names.join(", "), motion::anim_label(&self.lib, anim))
        });
    }

    pub fn stop_preview(&mut self) {
        if self.preview.take().is_some() {
            self.colliding.clear();
            self.ui_dirty = true;
        }
    }

    /// Mouvement propre des zones à cet instant de l'aperçu (mélange compris).
    pub fn current_locals(&self) -> Vec<motion::Pose> {
        let (Some(d), Some(p)) = (self.shown(), &self.preview) else { return Vec::new() };
        let now = motion::zone_locals_with(&d.model, &self.lib, &p.anim, p.t, true, &self.inputs());
        if p.blend < 1.0 && p.from.len() == now.len() {
            let f = p.blend * p.blend * (3.0 - 2.0 * p.blend);
            motion::blend(&p.from, &now, f)
        } else {
            now
        }
    }

    /// Ce que le jeu donnera aux blocs pilotés (E7), simulé dans l'aperçu : la poussée choisie,
    /// une cible qui tourne autour du modèle, une direction de poussée qui oscille.
    pub fn inputs(&self) -> motion::Inputs {
        let (Some(d), Some(p)) = (self.shown(), &self.preview) else { return motion::Inputs::default() };
        let s = d.model.size.as_vec3();
        let t = p.t;
        let r = s.max_element() * 0.8;
        motion::Inputs {
            thrust: self.thrust,
            speed: self.thrust,
            maneuver: 0.5 + 0.5 * (t * 2.0).sin(),
            target: s * 0.5 + Vec3::new(r * (t * 0.4).cos(), r * (0.35 + 0.25 * (t * 0.3).sin()), r * (t * 0.4).sin()),
            steer: Vec3::new((t * 0.7).sin() * 0.4, (t * 0.5).cos() * 0.3, 1.0).normalize(),
        }
    }

    pub fn say(&mut self, msg: String) {
        self.message = msg;
        self.ui_dirty = true;
    }

    /// Onglet `i` : cadré, remaillé.
    pub fn select(&mut self, i: usize) {
        if let Some(d) = self.docs.get_mut(self.current) {
            d.end();
        }
        self.current = i.min(self.docs.len().saturating_sub(1));
        self.placing = None;
        self.selection = None;
        self.drag = None;
        self.pasting = false;
        self.size_rev = u64::MAX;
        self.stop_preview();
        if let Some(d) = self.docs.get_mut(self.current) {
            d.mesh_dirty = true;
        }
        self.focus();
        self.size_bytes = None;
        self.size_tick = 0;
        self.ui_dirty = true;
    }

    pub fn focus(&mut self) {
        if let Some(m) = self.shown().map(|d| d.model.clone()) {
            self.cam.focus(&m);
        }
    }

    pub fn undo(&mut self) {
        self.stop_preview();
        let done = self.doc_mut().is_some_and(|d| d.undo());
        self.say(if done { "Annule.".into() } else { "Rien a annuler.".into() });
    }

    pub fn redo(&mut self) {
        self.stop_preview();
        let done = self.doc_mut().is_some_and(|d| d.redo());
        self.say(if done { "Retabli.".into() } else { "Rien a retablir.".into() });
    }

    pub fn set_color(&mut self, c: PaletteEntry) {
        self.color = c;
        // Récentes : la plus récente devant, sans doublon, 16 au plus
        self.recent.retain(|r| *r != c);
        self.recent.insert(0, c);
        self.recent.truncate(16);
        self.ui_dirty = true;
    }

    /// Enregistre l'onglet : dans son fichier, sinon un nouveau dans la bibliothèque.
    pub fn save(&mut self) {
        let Some(d) = self.docs.get_mut(self.current) else { return };
        d.end();
        // Le fichier ne garde que les couleurs utilisées (l'onglet garde les siennes : l'historique
        // d'annulation en dépend)
        let mut saved = d.model.clone();
        saved.compact_palette();
        let r = match &d.path {
            Some(p) => saved.to_bytes().and_then(|b| std::fs::write(p, b).map_err(|e| e.to_string())).map(|_| p.clone()),
            None => save_model(&library_dir(), &saved),
        };
        let msg = match r {
            Ok(p) => {
                d.path = Some(p.clone());
                d.dirty = false;
                format!("Enregistre : {}", p.display())
            }
            Err(e) => format!("Non enregistre : {e}"),
        };
        self.size_bytes = None;
        self.size_tick = 0;
        self.size_rev = u64::MAX;
        self.say(msg);
    }

    /// Poids du fichier (Q4 : 10 Mo au plus) et nombre de blocs : recalculés hors du fil principal
    /// (le modèle est copié sans frais : chunks partagés) une seconde au plus après un changement.
    pub fn refresh_size(&mut self) {
        if let Some(t) = self.size_task.as_mut() {
            if let Some((bytes, count)) = block_on(future::poll_once(t)) {
                self.size_bytes = Some(bytes);
                self.voxel_count = count;
                self.size_task = None;
            }
            return;
        }
        self.size_tick = self.size_tick.saturating_sub(1);
        let Some(d) = self.doc() else { return };
        if self.size_tick > 0 || d.revision == self.size_rev {
            return;
        }
        let (model, rev) = (d.model.clone(), d.revision);
        self.size_rev = rev;
        self.size_tick = 60;
        self.size_task = Some(AsyncComputeTaskPool::get().spawn(async move {
            let bytes = model.to_bytes().map_or(format::MAX_FILE_BYTES + 1, |b| b.len());
            (bytes, model.voxels.count())
        }));
    }

    /// Choisir un outil (un tracé, un collage en cours s'arrêtent).
    pub fn set_tool(&mut self, t: Tool) {
        self.tool = t;
        self.drag = None;
        self.pasting = false;
        self.ui_dirty = true;
        if t.is_shape() && t != Tool::Select {
            self.say(format!("{} : tire a la souris ; molette pendant le trace : epaisseur. Mode : {}.", t.name(), self.brush.name()));
        } else if t == Tool::Select {
            self.say("Selection : tire une boite. Ctrl+C / Ctrl+X / Ctrl+V, Suppr, R : tourner (Maj+R : autre axe).".into());
        } else if t == Tool::Fill {
            self.say("Remplir : peindre ou retirer la region de meme couleur ; ajouter : remplir le plan vide vise.".into());
        }
    }

    /// Fin d'un tracé : le volume est posé (ou la sélection faite).
    pub fn finish_drag(&mut self) {
        let Some(d) = self.drag.take() else { return };
        let Some(shape) = d.shape() else { return };
        if d.tool == Tool::Select {
            let (lo, hi) = shape.bounds();
            let size = hi - lo + IVec3::ONE;
            self.selection = Some((lo, hi));
            self.say(format!("Selection {} x {} x {}.", size.x, size.y, size.z));
            return;
        }
        let (brush, color, mirror) = (self.brush, self.color, self.mirror);
        let t0 = std::time::Instant::now();
        let r = self.doc_mut().map(|doc| doc.apply_shape(shape, brush, color, mirror));
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        match r {
            Some(Ok(n)) => self.say(format!("{n} blocs ({}) en {ms:.0} ms. Ctrl+Z pour annuler.", brush.name())),
            Some(Err(e)) => self.say(e),
            None => {}
        }
    }

    /// Pot de peinture.
    pub fn flood(&mut self, start: IVec3, normal: IVec3) {
        let (brush, color) = (self.brush, self.color);
        let r = self.doc_mut().map(|d| d.flood(start, normal, brush, color, 4_000_000));
        match r {
            Some(Ok(n)) => self.say(format!("{n} blocs ({}).", brush.name())),
            Some(Err(e)) => self.say(e),
            None => {}
        }
    }

    /// Copier (ou couper) la sélection.
    pub fn copy_selection(&mut self, cut: bool) {
        let Some((lo, hi)) = self.selection else {
            self.say("Rien de selectionne (outil Selection, touche 0).".into());
            return;
        };
        let Some(d) = self.doc_mut() else { return };
        let clip = d.copy(lo, hi);
        if cut {
            d.end();
            d.clear(lo, hi);
        }
        let n = clip.count();
        self.clip = Some(clip);
        self.clip_rev += 1;
        self.say(format!("{n} blocs {}. Ctrl+V pour coller.", if cut { "coupes" } else { "copies" }));
    }

    /// Commencer un collage : le presse-papiers suit la souris.
    pub fn start_paste(&mut self) {
        if self.clip.is_none() {
            self.say("Presse-papiers vide (Ctrl+C sur une selection).".into());
            return;
        }
        self.pasting = true;
        self.placing = None;
        self.say("Coller : clic pour poser, R pour tourner, Echap pour annuler.".into());
    }

    /// Où irait le presse-papiers (coin bas), sous la souris : centré sur la case visée.
    pub fn paste_anchor(&self) -> Option<IVec3> {
        let (clip, at) = (self.clip.as_ref()?, self.hover.1.or(self.hover.0)?);
        let s = clip.model.size.as_ivec3();
        Some(at - IVec3::new(s.x / 2, 0, s.z / 2))
    }

    pub fn paste_here(&mut self) {
        let (Some(at), Some(clip)) = (self.paste_anchor(), self.clip.clone()) else { return };
        let r = self.doc_mut().map(|d| {
            d.end();
            d.paste(&clip, at)
        });
        self.pasting = false;
        let size = clip.model.size.as_ivec3();
        self.selection = Some((at, at + size - IVec3::ONE));
        match r {
            Some(Ok(n)) => self.say(format!("{n} blocs colles.")),
            Some(Err(e)) => self.say(e),
            None => {}
        }
    }

    pub fn delete_selection(&mut self) {
        let Some((lo, hi)) = self.selection else { return };
        let n = self.doc_mut().map_or(0, |d| {
            d.end();
            d.clear(lo, hi)
        });
        self.say(format!("{n} blocs effaces."));
    }

    /// Tourner / retourner : le presse-papiers pendant un collage, sinon la sélection sur place.
    pub fn transform(&mut self, t: ClipTransform) {
        if self.pasting {
            if let Some(c) = self.clip.take() {
                self.clip = Some(c.transformed(t));
                self.clip_rev += 1;
            }
            return;
        }
        let Some((lo, hi)) = self.selection else { return };
        let r = self.doc_mut().map(|d| d.transform_selection(lo, hi, t));
        match r {
            Some(Ok(b)) => {
                self.selection = Some(b);
                self.say("Selection tournee (Ctrl+Z pour annuler).".into());
            }
            Some(Err(e)) => self.say(e),
            None => {}
        }
    }

    /// Coupe : aucune -> y -> x -> z -> aucune (au milieu du modèle).
    pub fn cycle_cut(&mut self) {
        let Some(d) = self.doc_mut() else { return };
        let next = match d.cut.map(|c| c.axis) {
            None => Some(1),
            Some(1) => Some(0),
            Some(0) => Some(2),
            _ => None,
        };
        let cut = next.map(|axis| Cut { axis, pos: d.model.size.as_ivec3()[axis] / 2 });
        d.set_cut(cut);
        let msg = match cut {
            Some(c) => format!("Coupe en {} = {} (Page prec. / suiv. pour la deplacer, Maj : de 8 ; C : autre axe).", ["x", "y", "z"][c.axis], c.pos),
            None => "Coupe retiree.".into(),
        };
        self.say(msg);
    }

    pub fn move_cut(&mut self, by: i32) {
        let Some(d) = self.doc_mut() else { return };
        let Some(c) = d.cut else { return };
        let max = d.model.size.as_ivec3()[c.axis] - 1;
        let cut = Cut { axis: c.axis, pos: (c.pos + by).clamp(0, max) };
        d.set_cut(Some(cut));
        self.say(format!("Coupe en {} = {}.", ["x", "y", "z"][c.axis], cut.pos));
    }

    /// Édite l'animation `name` du modèle : l'aperçu s'arrête sur l'instant de la frise.
    pub fn edit_anim(&mut self, name: Option<String>) {
        self.edit_anim = name.clone();
        match name {
            Some(n) => {
                self.start_preview(&n);
                let t = self.cursor;
                if let Some(p) = self.preview.as_mut() {
                    p.paused = true;
                    p.t = t;
                }
            }
            None => self.stop_preview(),
        }
        self.ui_dirty = true;
    }

    /// Place l'aperçu (arrêté) sur l'instant de la frise.
    pub fn seek(&mut self, t: f32) {
        self.cursor = t.max(0.0);
        let anim = self.edit_anim.clone();
        match (&mut self.preview, anim) {
            (Some(p), Some(a)) if p.anim == a => {
                p.t = self.cursor;
                p.paused = true;
                p.blend = 1.0;
            }
            (_, Some(a)) => self.edit_anim(Some(a)),
            _ => {}
        }
        self.ui_dirty = true;
    }

    /// L'os de la zone choisie (images clés).
    pub fn sel_bone(&self) -> Option<String> {
        let z = self.sel_zone?;
        self.doc()?.model.zones.get(z).map(motion::bone)
    }

    /// Change les angles de la zone choisie à l'instant de la frise (crée l'image clé).
    pub fn key_angles(&mut self, f: impl FnOnce(&mut [f32; 3])) {
        let (Some(anim), Some(bone)) = (self.edit_anim.clone(), self.sel_bone()) else {
            self.say("Choisis une zone (liste a droite) et une animation du modele.".into());
            return;
        };
        let t = self.cursor;
        if let Some(d) = self.doc_mut() {
            d.edit_anims(|anims| {
                if let Some(a) = anims.get_mut(&anim) {
                    let mut angles = custom::angles_at(a, &bone, t);
                    f(&mut angles);
                    custom::set_key(a, &bone, t, angles);
                }
            });
        }
        self.seek(t);
    }

    /// Enregistre la zone choisie (et ses filles) en bloc de mouvement personnel.
    pub fn save_block(&mut self) {
        let (Some(z), Some(d)) = (self.sel_zone, self.doc()) else {
            self.say("Choisis d'abord une zone (liste a droite).".into());
            return;
        };
        let Some(zone) = d.model.zones.get(z) else { return };
        let name = format!("{} ({})", zone.name, d.model.name);
        let id = format!("perso_{}", file_stem(&name));
        let json = custom::block_json(&d.model, z, &id, &name);
        let dir = editor_dir().join("blocs");
        let path = dir.join(format!("{id}.json"));
        let r = std::fs::create_dir_all(&dir).map_err(|e| e.to_string()).and_then(|_| serde_json::to_string_pretty(&json).map_err(|e| e.to_string())).and_then(|t| std::fs::write(&path, t).map_err(|e| e.to_string()));
        match r {
            Ok(()) => {
                let (lib, _) = motion::Library::load(Some(&editor_dir()));
                self.lib = lib;
                self.say(format!("Bloc \"{name}\" enregistre ({}) : il est dans la liste des blocs de mouvement.", path.display()));
            }
            Err(e) => self.say(format!("Bloc non enregistre : {e}")),
        }
    }

    /// Exporte l'onglet en MagicaVoxel (`saves/export/<nom>.vox`).
    pub fn export_vox(&mut self) {
        let Some(d) = self.doc() else { return };
        let dir = export_dir();
        let path = dir.join(format!("{}.vox", file_stem(&d.model.name)));
        let bytes = vox::export(&d.model);
        let msg = match std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&path, bytes)) {
            Ok(()) => format!("Exporte pour MagicaVoxel : {}", path.display()),
            Err(e) => format!("Export impossible : {e}"),
        };
        self.say(msg);
    }

    /// Importe les JSON de Pixel World de `saves/import/`.
    pub fn import(&mut self) {
        let from = import_dir();
        let msg = match import_folder(&from, &library_dir()) {
            Ok(r) if r.models == 0 => format!("Aucun modele dans {} : copiez-y des .vox (MagicaVoxel) ou des .json (Pixel World).", from.display()),
            Ok(r) => format!("{} modele(s) importe(s), {} voxels ({} blocs inconnus, {} hors de la grille).", r.models, r.voxels, r.unknown, r.outside),
            Err(e) => {
                let _ = std::fs::create_dir_all(&from);
                format!("Import impossible : {e}. Copiez vos JSON de Pixel World dans {}.", from.display())
            }
        };
        self.library = list_models(&library_dir());
        self.say(msg);
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  Bibliothèque (`saves/modeles/`) et import (`saves/import/`)
// ─────────────────────────────────────────────────────────────────────────

pub fn library_dir() -> PathBuf {
    crate::settings::data_dir().join("modeles")
}

/// Données de l'éditeur ajoutées par le joueur : `blocs/`, `anims/`, `races/` (un JSON par
/// élément, même format que `assets/editeur/`).
pub fn editor_dir() -> PathBuf {
    crate::settings::data_dir().join("editeur")
}

/// Modèles exportés (MagicaVoxel `.vox`, E8).
pub fn export_dir() -> PathBuf {
    crate::settings::data_dir().join("export")
}

pub fn import_dir() -> PathBuf {
    crate::settings::data_dir().join("import")
}

/// Nom de fichier sûr (lettres, chiffres, tirets).
fn file_stem(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect();
    let s = s.trim_matches('-').to_string();
    if s.is_empty() { "modele".into() } else { s.chars().take(48).collect() }
}

/// Enregistre un modèle dans `dir` sans écraser un autre (« nom-2.ssvox »...).
pub fn save_model(dir: &Path, model: &Model) -> Result<PathBuf, String> {
    let bytes = model.to_bytes()?;
    std::fs::create_dir_all(dir).map_err(|e| format!("dossier {} : {e}", dir.display()))?;
    let stem = file_stem(&model.name);
    let mut path = dir.join(format!("{stem}.ssvox"));
    let mut k = 2;
    while path.exists() {
        path = dir.join(format!("{stem}-{k}.ssvox"));
        k += 1;
    }
    std::fs::write(&path, bytes).map_err(|e| format!("ecriture de {} : {e}", path.display()))?;
    Ok(path)
}

/// Les modèles d'un dossier (les fichiers illisibles sont ignorés).
pub fn list_models(dir: &Path) -> Vec<LibraryEntry> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<LibraryEntry> = rd
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "ssvox"))
        .filter_map(|path| {
            let bytes = std::fs::read(&path).ok()?;
            let m = Model::from_bytes(&bytes).ok()?;
            Some(LibraryEntry { path, name: m.name.clone(), kind: m.kind, voxels: m.voxels.count(), bytes: bytes.len() })
        })
        .collect();
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

/// Importe tous les JSON de Pixel World d'un dossier dans la bibliothèque.
pub fn import_folder(from: &Path, to: &Path) -> Result<import::ImportReport, String> {
    let rd = std::fs::read_dir(from).map_err(|_| format!("dossier {} introuvable", from.display()))?;
    let mut total = import::ImportReport::default();
    let mut errors = Vec::new();
    let files: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
    // MagicaVoxel (E8)
    for path in files.iter().filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("vox"))) {
        let name = path.file_stem().map_or("Modele".into(), |s| s.to_string_lossy().to_string());
        match std::fs::read(path).map_err(|e| e.to_string()).and_then(|b| vox::import(&b, &name)) {
            Ok(m) => {
                save_model(to, &m)?;
                total.models += 1;
                total.voxels += m.voxels.count();
            }
            Err(e) => errors.push(format!("{} : {e}", path.display())),
        }
    }
    for path in files.into_iter().filter(|p| p.extension().is_some_and(|x| x == "json")) {
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => {
                errors.push(format!("{} : {e}", path.display()));
                continue;
            }
        };
        match import::from_pixel_world(&text) {
            Ok((models, r)) => {
                for m in &models {
                    save_model(to, m)?;
                }
                total.models += r.models;
                total.voxels += r.voxels;
                total.unknown += r.unknown;
                total.outside += r.outside;
            }
            Err(e) => errors.push(format!("{} : {e}", path.display())),
        }
    }
    if total.models == 0 && !errors.is_empty() {
        return Err(errors.join(" ; "));
    }
    Ok(total)
}

// ─────────────────────────────────────────────────────────────────────────
//  Ouverture et fermeture
// ─────────────────────────────────────────────────────────────────────────

fn open_on_first_launch(settings: Res<crate::settings::GameSettings>, mut open: EventWriter<OpenEditor>, mut editor: ResMut<Editor>) {
    if settings.first_launch {
        editor.welcome = true;
        open.send(OpenEditor(ModelKind::Personnage));
    }
}

fn open_editor(mut events: EventReader<OpenEditor>, mut editor: ResMut<Editor>, mut next: ResMut<NextState<AppState>>, mut menu: ResMut<crate::ui::MenuState>) {
    for OpenEditor(kind) in events.read() {
        editor.new_kind = *kind;
        menu.open = false;
        info!("Editeur de modeles ouvert ({})", kind.name());
        next.set(AppState::Editeur);
    }
}

fn menu_button(interactions: Query<&Interaction, (Changed<Interaction>, With<EditorMenuButton>)>, mut open: EventWriter<OpenEditor>) {
    if interactions.iter().any(|i| *i == Interaction::Pressed) {
        open.send(OpenEditor(ModelKind::Personnage));
    }
}

/// Tests (développement) : `SPACESPORE_CAPTURE=fichier.png` fait une capture d'écran après
/// `SPACESPORE_CAPTURE_SECS` secondes (12 par défaut), puis ferme le jeu ; avec
/// Argument d'un mode de démonstration (`SPACESPORE_EDITOR_DEMO=<prefixe><argument>`).
fn demo_arg(prefix: &str) -> Option<String> {
    std::env::var("SPACESPORE_EDITOR_DEMO").ok()?.strip_prefix(prefix).map(str::to_string)
}

/// `SPACESPORE_EDITOR_DEMO=1`, l'éditeur s'ouvre sur un petit personnage de démonstration.
#[allow(clippy::too_many_arguments)]
fn test_capture(
    mut commands: Commands,
    time: Res<Time>,
    mut done: Local<u8>,
    mut frames: Local<(u32, f32, f32)>,
    mut editor: ResMut<Editor>,
    state: Res<State<AppState>>,
    mut exit: EventWriter<AppExit>,
) {
    let Ok(path) = std::env::var("SPACESPORE_CAPTURE") else { return };
    let secs: f32 = std::env::var("SPACESPORE_CAPTURE_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(12.0);
    let t = time.elapsed_secs();
    // Images par seconde pendant les 4 s avant la capture (règle 8), écrites à côté de la capture
    if *done == 1 && t > secs - 4.0 && t <= secs {
        frames.0 += 1;
        frames.1 += time.delta_secs();
        frames.2 = frames.2.max(time.delta_secs());
    }
    // `SPACESPORE_EDITOR_DEMO=vide:<categorie>` : grille vide d'une catégorie, un bloc à chaque coin
    // (C1 : toutes les grilles s'affichent) ; `fourni:<k>` : copie du modèle fourni k
    if *done == 0 && *state.get() == AppState::Editeur && t > secs * 0.3 {
        if let Some(c) = demo_arg("vide:") {
            *done = 1;
            let cat = ShipCategory::ALL.into_iter().find(|x| x.name().to_lowercase() == c).unwrap_or(ShipCategory::Corvette);
            let mut d = Doc::new(format::Model::new("Vide", ModelKind::Vaisseau, Some(cat)), None);
            let n = cat.grid() as i32 - 1;
            let i = d.model.color_index(PaletteEntry { rgb: [230, 120, 60], material: format::Material::Mate }).unwrap();
            for x in [0, n] {
                for y in [0, n] {
                    for z in [0, n] {
                        d.set(IVec3::new(x, y, z), i, false);
                    }
                }
            }
            editor.overlay = None;
            editor.race_view = None;
            editor.docs.push(d);
            let k = editor.docs.len() - 1;
            editor.select(k);
        } else if let Some(k) = demo_arg("fourni:").and_then(|k| k.parse::<usize>().ok()) {
            *done = 1;
            editor.lib = motion::Library::load(None).0;
            editor.open_provided(k);
        }
    }
    // `SPACESPORE_EDITOR_DEMO=croiseur` : le croiseur 512³ de démonstration
    let demo = std::env::var("SPACESPORE_EDITOR_DEMO").unwrap_or_default();
    if *done == 0 && matches!(demo.as_str(), "croiseur" | "hangar" | "vaisseau") && *state.get() == AppState::Editeur && t > secs * 0.3 {
        *done = 1;
        let t0 = std::time::Instant::now();
        editor.lib = motion::Library::load(None).0;
        let get = |lib: &motion::Library, id: &str| lib.block(id).cloned().unwrap();
        let p0 = motion::Placement { turn: 0, mirror: false, scale: 1 };
        let mut d = if demo == "vaisseau" {
            // `vaisseau` : un chasseur et ses blocs de vaisseau, état « combat »
            let mut d = Doc::new(format::Model::new("Chasseur de demo", ModelKind::Vaisseau, Some(format::ShipCategory::Chasseur)), None);
            let hull = PaletteEntry { rgb: [120, 125, 135], material: format::Material::Metal };
            let _ = d.apply_shape(Shape::Box { a: IVec3::new(26, 20, 12), b: IVec3::new(37, 27, 50) }, Brush::Add, hull, false);
            let _ = d.apply_shape(Shape::Box { a: IVec3::new(29, 28, 38), b: IVec3::new(34, 30, 46) }, Brush::Add, hull, false);
            let lib = editor.lib.clone();
            for (id, at, place, mirror) in [
                ("ailes_x", IVec3::new(38, 23, 25), p0, true),
                ("propulseur", IVec3::new(31, 23, 11), motion::Placement { scale: 2, ..p0 }, false),
                ("tourelle", IVec3::new(31, 28, 30), motion::Placement { scale: 2, ..p0 }, false),
                ("train", IVec3::new(29, 19, 20), motion::Placement { scale: 2, ..p0 }, true),
                ("radar", IVec3::new(31, 31, 40), p0, false),
                ("feu", IVec3::new(57, 24, 30), p0, true),
                ("tuyere_orientable", IVec3::new(28, 24, 11), p0, true),
            ] {
                let _ = d.place_block(&get(&lib, id), at, place, mirror);
            }
            d
        } else {
            let mut d = edit::demo_cruiser();
            if demo == "hangar" {
                // `hangar` : un hangar à chasseur sur le flanc du croiseur, entrée en cours
                let _ = d.place_block(&get(&editor.lib, "hangar_chasseur"), IVec3::new(256 + 70, 180, 150), motion::Placement { turn: 1, ..p0 }, false);
            }
            d
        };
        d.end();
        info!("demo construite en {:.0} ms", t0.elapsed().as_secs_f64() * 1000.0);
        editor.docs.push(d);
        editor.overlay = None;
        let i = editor.docs.len() - 1;
        editor.select(i);
        if std::env::var("SPACESPORE_EDITOR_CUT").is_ok() {
            editor.cycle_cut();
        }
        let anim = match demo.as_str() {
            "hangar" => Some((motion::HANGAR_IN, 4.5)),
            "vaisseau" => Some(("etat:combat", 3.0)),
            _ => None,
        };
        if let Some((a, at)) = anim {
            editor.start_preview(a);
            // L'instant voulu de l'animation tombe au moment de la capture
            if let Some(p) = editor.preview.as_mut() {
                p.t = at - (secs - t);
                p.blend = 1.0;
            }
        }
    }
    if *done == 0 && std::env::var("SPACESPORE_EDITOR_DEMO").is_ok_and(|v| !v.starts_with("race:") && !v.starts_with("vide:") && !v.starts_with("fourni:") && !matches!(v.as_str(), "croiseur" | "hangar" | "vaisseau")) && *state.get() == AppState::Editeur && t > secs * 0.5 {
        *done = 1;
        let mut m = format::Model::new("Demo", ModelKind::Personnage, None);
        m.race = Some("Humanoide".to_string());
        let mut doc = Doc::new(m, None);
        let palette = edit::starter_palette();
        // Un petit bonhomme : jambes, corps, bras, tête (symétrie miroir)
        let mut put = |x0: i32, y0: i32, z0: i32, x1: i32, y1: i32, z1: i32, c: PaletteEntry| {
            for x in x0..=x1 {
                for y in y0..=y1 {
                    for z in z0..=z1 {
                        let i = doc.model.color_index(c).unwrap();
                        doc.set(IVec3::new(x, y, z), i, true);
                    }
                }
            }
        };
        put(5, 0, 14, 6, 9, 16, palette[30]);
        put(4, 10, 13, 7, 19, 17, palette[8]);
        put(2, 11, 14, 3, 18, 16, palette[10]);
        put(5, 20, 13, 7, 25, 18, palette[3]);
        put(6, 22, 18, 6, 22, 18, palette[38]);
        // Matières (E2) : visière de verre, épaulettes de métal, ceinture lumineuse
        put(5, 23, 18, 7, 23, 18, PaletteEntry { rgb: [120, 200, 255], material: format::Material::Verre });
        put(2, 19, 13, 3, 19, 17, PaletteEntry { rgb: [212, 175, 55], material: format::Material::Metal });
        put(4, 10, 17, 7, 10, 17, PaletteEntry { rgb: [255, 60, 200], material: format::Material::Lumineuse });
        // Blocs de mouvement (E4) : `SPACESPORE_EDITOR_DEMO=blocs` pose des bras, une tête et une
        // queue, lance l'aperçu « marche » et montre le gabarit d'une aile
        let blocks_demo = std::env::var("SPACESPORE_EDITOR_DEMO").is_ok_and(|v| v == "blocs" || v == "avance");
        if blocks_demo {
            let (lib, _) = motion::Library::load(None);
            let get = |id: &str| lib.block(id).unwrap().clone();
            let p = motion::Placement { turn: 0, mirror: false, scale: 1 };
            let _ = doc.place_block(&get("bras"), IVec3::new(2, 19, 15), p, true);
            let _ = doc.place_block(&get("tete"), IVec3::new(6, 26, 16), p, false);
            let _ = doc.place_block(&get("queue"), IVec3::new(5, 11, 12), p, false);
        }
        editor.docs.push(doc);
        editor.overlay = None;
        let i = editor.docs.len() - 1;
        editor.select(i);
        if std::env::var("SPACESPORE_EDITOR_DEMO").is_ok_and(|v| v == "avance") {
            // `avance` : mode avancé, une animation du modèle en cours d'édition (E8)
            editor.lib = motion::Library::load(None).0;
            editor.advanced = true;
            editor.sel_zone = Some(0);
            if let Some(d) = editor.doc_mut() {
                d.edit_anims(|anims| {
                    let mut a = format::ModelAnim { duration: 2.0, keys: Default::default() };
                    custom::set_key(&mut a, "bras_g", 0.0, [0.0; 3]);
                    custom::set_key(&mut a, "bras_g", 1.0, [0.0, 0.0, -120.0]);
                    custom::set_key(&mut a, "bras_g", 2.0, [0.0; 3]);
                    anims.insert("lever le bras".into(), a);
                });
            }
            editor.cursor = 1.0;
            editor.edit_anim(Some("lever le bras".into()));
        } else if blocks_demo {
            editor.start_preview("marcher");
        }
        // `SPACESPORE_EDITOR_DEMO=gabarit` : le gabarit d'une aile (tournée, reflétée) près du corps
        if std::env::var("SPACESPORE_EDITOR_DEMO").is_ok_and(|v| v == "gabarit") {
            editor.lib = motion::Library::load(None).0;
            if let Some(i) = editor.lib.blocks.iter().position(|b| b.id == "aile") {
                editor.pick_block(i);
                if let Some(p) = editor.placing.as_mut() {
                    p.place.turn = 1;
                }
            }
            editor.hover = (Some(IVec3::new(4, 18, 13)), Some(IVec3::new(4, 18, 12)));
        }
        editor.hover = (Some(IVec3::new(7, 19, 15)), Some(IVec3::new(8, 19, 15)));
        if let Ok(v) = std::env::var("SPACESPORE_EDITOR_SCROLL") {
            editor.scroll[0] = v.parse().unwrap_or(0.0);
        }
    }
    // `SPACESPORE_EDITOR_DEMO=race:<id>` : la fenêtre « Nouveau modèle » sur cette race (aperçu)
    if *done == 0 && *state.get() == AppState::Editeur && t > secs * 0.5 {
        if let Some(id) = std::env::var("SPACESPORE_EDITOR_DEMO").ok().and_then(|v| v.strip_prefix("race:").map(str::to_string)) {
            *done = 1;
            editor.lib = motion::Library::load(None).0;
            editor.overlay = Some(Overlay::New);
            editor.new_kind = ModelKind::Personnage;
            if let Some(r) = editor.lib.races.iter().position(|r| r.id == id) {
                let all = std::env::var("SPACESPORE_RACE_OPTIONS").is_ok().then(|| vec![true; editor.lib.races[r].options.len()]);
                editor.choose_race(r, all);
                if let Ok(a) = std::env::var("SPACESPORE_RACE_ANIM") {
                    editor.start_preview(&a);
                }
            }
        }
    }
    if *done <= 1 && t > secs {
        *done = 2;
        if frames.0 > 0 {
            let fps = format!("{:.1} images/s en moyenne, pire image {:.1} ms ({} images)
", frames.0 as f32 / frames.1, frames.2 * 1000.0, frames.0);
            info!("{fps}");
            let _ = std::fs::write(format!("{path}.txt"), fps);
        }
        commands.spawn(bevy::render::view::screenshot::Screenshot::primary_window()).observe(bevy::render::view::screenshot::save_to_disk(path));
    }
    if *done == 2 && t > secs + 3.0 {
        exit.send(AppExit::Success);
    }
}

/// Création du personnage (E7) : le premier personnage enregistré devient celui du joueur.
fn first_character(mut editor: ResMut<Editor>, mut settings: ResMut<crate::settings::GameSettings>, mut models: ResMut<crate::models::GameModels>) {
    if settings.character_model.is_some() {
        return;
    }
    let Some(d) = editor.doc().filter(|d| d.model.kind == ModelKind::Personnage && !d.dirty) else { return };
    let Some(path) = d.path.clone() else { return };
    let rel = path.strip_prefix(crate::settings::data_dir()).map(|p| p.to_path_buf()).unwrap_or(path);
    settings.character_model = Some(rel.to_string_lossy().replace('\\', "/"));
    settings.save();
    models.reload = true;
    let name = d.model.name.clone();
    editor.say(format!("\"{name}\" sera ton personnage en jeu (bouton \"Utiliser comme mon personnage\" pour en changer)."));
}

/// En entrant : sans modèle ouvert, la fenêtre « Nouveau modèle » (création du personnage).
fn on_enter(mut editor: ResMut<Editor>) {
    editor.library = list_models(&library_dir());
    let (lib, errors) = motion::Library::load(Some(&editor_dir()));
    editor.lib = lib;
    if !errors.is_empty() {
        warn!("Donnees de l'editeur illisibles : {}", errors.join(" ; "));
        editor.say(format!("Fichiers illisibles : {}", errors.join(" ; ")));
    }
    if editor.docs.is_empty() {
        editor.overlay = Some(Overlay::New);
    } else {
        editor.focus();
    }
    editor.ui_dirty = true;
}

#[cfg(test)]
mod tests {
    use super::*;
    use format::{Material, PaletteEntry};

    fn temp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("spacespore-editeur-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn library_saves_lists_and_never_overwrites() {
        let dir = temp_dir("lib");
        let mut m = Model::new("Mon Vaisseau !", ModelKind::Vaisseau, Some(ShipCategory::Corvette));
        let c = m.color_index(PaletteEntry { rgb: [1, 2, 3], material: Material::Metal }).unwrap();
        m.voxels.set(bevy::math::IVec3::new(5, 5, 5), c);
        let a = save_model(&dir, &m).unwrap();
        let b = save_model(&dir, &m).unwrap();
        assert_ne!(a, b);
        assert!(a.file_name().unwrap().to_string_lossy().starts_with("mon-vaisseau"));
        let list = list_models(&dir);
        assert_eq!(list.len(), 2);
        assert_eq!((list[0].kind, list[0].voxels), (ModelKind::Vaisseau, 1));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Les vrais modèles de Pixel World (sur la machine de développement) :
    /// `cargo test --release real_pixel_world -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn real_pixel_world_models_import() {
        let from = PathBuf::from(r"D:\... logiciel\unity\Pixel world\VoxelModels");
        let to = temp_dir("real");
        for sub in [from.clone(), from.join("ai_samples")] {
            let r = import_folder(&sub, &to).unwrap();
            println!("{} : {r:?}", sub.display());
            assert!(r.models > 0 && r.unknown == 0, "{r:?}");
        }
        let list = list_models(&to);
        let total: usize = list.iter().map(|e| e.bytes).sum();
        println!("{} modeles, {:.1} Ko au total", list.len(), total as f64 / 1024.0);
        for e in list.iter().take(8) {
            println!("  {} : {} voxels, {} octets", e.name, e.voxels, e.bytes);
        }
        let _ = std::fs::remove_dir_all(&to);
    }

    #[test]
    fn import_folder_converts_pixel_world_files() {
        let (from, to) = (temp_dir("import-from"), temp_dir("import-to"));
        std::fs::create_dir_all(&from).unwrap();
        std::fs::write(from.join("arbre.json"), r#"{ "name": "Arbre", "size": [3, 5, 3], "voxels": [[1, 0, 1, "Wood"], [1, 1, 1, "Wood"], [1, 2, 1, "Leaves"]] }"#).unwrap();
        std::fs::write(from.join("casse.json"), "{ pas du json").unwrap();
        let r = import_folder(&from, &to).unwrap();
        assert_eq!((r.models, r.voxels), (1, 3));
        let list = list_models(&to);
        assert_eq!(list.len(), 1);
        assert_eq!((list[0].name.as_str(), list[0].kind), ("Arbre", ModelKind::Autre));
        let _ = std::fs::remove_dir_all(&from);
        let _ = std::fs::remove_dir_all(&to);
    }
}
