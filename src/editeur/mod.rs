//! Éditeur de modèles voxel (0.12, `ROADMAP-0.12-editeur.md`) : personnages, vaisseaux, objets.
//!
//! - E0 : l'état du jeu `AppState` (Jeu / Editeur : en éditeur, le jeu ne lit plus le clavier ni la
//!   souris), ouverture à la **création du personnage** (premier lancement), depuis le menu et par
//!   `/editeur` ; le format `.ssvox` (`format.rs`), la bibliothèque `saves/modeles/`, l'import de
//!   Pixel World (`import.rs`).
//! - E1 : l'éditeur (`edit.rs` : outils, miroir, annuler, rayon, maillage ; `view.rs` : scène et
//!   caméra ; `panels.rs` : interface), porté de `VoxelEditorManager.cs`.
//! - E2 : palette OKLCH et matières (`palette.rs`).
//! - E4 : blocs de mouvement (`motion.rs` : blocs en données, placement, lecteur d'animations ;
//!   `Doc::place_block` ; gabarit, zones et aperçu dans `view.rs`).

pub mod edit;
pub mod format;
pub mod import;
pub mod motion;
pub mod palette;
pub mod panels;
pub mod view;

use bevy::prelude::*;
use std::path::{Path, PathBuf};

use edit::{Doc, Tool};
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
            .add_systems(Startup, (open_on_first_launch, view::setup_gizmos))
            .add_systems(Update, (open_editor, menu_button))
            .add_systems(OnEnter(AppState::Editeur), (view::enter_scene, on_enter).chain())
            .add_systems(OnExit(AppState::Editeur), (view::exit_scene, panels::show_game_ui))
            .add_systems(
                Update,
                (panels::typing, panels::escape, panels::actions, panels::replace_on_right_click, panels::scroll_panels, view::camera_input, view::tools_input, view::update_mesh, view::animate, view::ghost, panels::rebuild, panels::live_texts, view::draw)
                    .chain()
                    .run_if(in_state(AppState::Editeur)),
            )
            .add_systems(PostUpdate, panels::hide_game_ui.run_if(in_state(AppState::Editeur)))
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
    pub from: Vec<(Quat, Vec3)>,
    /// Avancement du mélange depuis `from` (0 à 1).
    pub blend: f32,
}

/// Bloc de mouvement en cours de pose : son index dans `Editor::blocks` et son orientation.
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
    pub scroll: [f32; 2],
    pub mirror: bool,
    pub grid: bool,
    pub cam: view::OrbitCam,
    /// Case pleine visée, case vide devant elle.
    pub hover: (Option<IVec3>, Option<IVec3>),
    pub last_cell: Option<IVec3>,
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
    /// Poids du fichier (recalculé de temps en temps).
    pub size_bytes: Option<usize>,
    size_tick: u32,
    /// Échap déjà utilisé cette image (fermer la saisie du nom).
    pub escape_used: bool,
    /// Blocs de mouvement (ceux du jeu, puis ceux de `saves/editeur/blocs/`).
    pub blocks: Vec<motion::BlockDef>,
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
            scroll: [0.0; 2],
            mirror: true,
            grid: true,
            cam: view::OrbitCam::default(),
            hover: (None, None),
            last_cell: None,
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
            size_tick: 0,
            escape_used: false,
            blocks: Vec::new(),
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

    /// Blocs de mouvement proposés pour le modèle ouvert (index dans `blocks`).
    pub fn blocks_for_doc(&self) -> Vec<usize> {
        let Some(d) = self.doc() else { return Vec::new() };
        let kind = d.model.kind.name().to_lowercase();
        (0..self.blocks.len()).filter(|i| self.blocks[*i].kinds.is_empty() || self.blocks[*i].kinds.iter().any(|k| k.to_lowercase() == kind)).collect()
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
        let scale = if self.blocks.get(i).is_some_and(|b| b.scalable) { "   Maj+molette : taille" } else { "" };
        self.say(format!("Clic : poser   Molette : tourner{scale}   X : miroir   Echap : annuler   (Ctrl+molette : zoom)"));
    }

    /// Lance l'aperçu (ou change d'animation) et vérifie les collisions.
    pub fn start_preview(&mut self, anim: &str) {
        self.placing = None;
        let from = match &self.preview {
            Some(_) => self.current_locals(),
            None => Vec::new(),
        };
        let Some(d) = self.doc() else { return };
        if d.model.zones.is_empty() {
            self.say("Aucune zone de mouvement : pose d'abord un bloc de mouvement.".into());
            return;
        }
        let colliding = motion::collisions(&d.model, &self.blocks, anim);
        let names: Vec<String> = colliding.iter().filter_map(|z| d.model.zones.get(*z).map(|z| z.name.clone())).collect();
        self.colliding = colliding;
        self.preview = Some(Preview { anim: anim.to_string(), t: 0.0, blend: if from.is_empty() { 1.0 } else { 0.0 }, from });
        self.say(if names.is_empty() {
            format!("Apercu : {anim}. P : arreter.")
        } else {
            format!("Attention : {} traverse le corps pendant \"{anim}\" (en rouge).", names.join(", "))
        });
    }

    pub fn stop_preview(&mut self) {
        if self.preview.take().is_some() {
            self.colliding.clear();
            self.ui_dirty = true;
        }
    }

    /// Mouvement propre des zones à cet instant de l'aperçu (mélange compris).
    pub fn current_locals(&self) -> Vec<(Quat, Vec3)> {
        let (Some(d), Some(p)) = (self.doc(), &self.preview) else { return Vec::new() };
        let now = motion::zone_locals(&d.model, &self.blocks, &p.anim, p.t);
        if p.blend < 1.0 && p.from.len() == now.len() {
            let f = p.blend * p.blend * (3.0 - 2.0 * p.blend);
            motion::blend(&p.from, &now, f)
        } else {
            now
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
        if let Some(m) = self.doc().map(|d| d.model.clone()) {
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
        self.say(msg);
    }

    /// Poids du fichier (Q4 : 10 Mo au plus), recalculé toutes les ~2 s.
    pub fn refresh_size(&mut self) {
        self.size_tick = self.size_tick.saturating_sub(1);
        if self.size_tick > 0 && self.size_bytes.is_some() {
            return;
        }
        self.size_tick = 120;
        self.size_bytes = self.doc().map(|d| match d.model.to_bytes() {
            Ok(b) => b.len(),
            Err(_) => format::MAX_FILE_BYTES + 1,
        });
    }

    /// Importe les JSON de Pixel World de `saves/import/`.
    pub fn import(&mut self) {
        let from = import_dir();
        let msg = match import_folder(&from, &library_dir()) {
            Ok(r) if r.models == 0 => format!("Aucun JSON de Pixel World dans {} : copiez-y vos modeles (.json).", from.display()),
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

/// Blocs de mouvement du joueur (un JSON par bloc, même format que `assets/editeur/blocs/`).
pub fn blocks_dir() -> PathBuf {
    crate::settings::data_dir().join("editeur").join("blocs")
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
    for path in rd.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|x| x == "json")) {
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
/// `SPACESPORE_EDITOR_DEMO=1`, l'éditeur s'ouvre sur un petit personnage de démonstration.
fn test_capture(mut commands: Commands, time: Res<Time>, mut done: Local<u8>, mut editor: ResMut<Editor>, state: Res<State<AppState>>, mut exit: EventWriter<AppExit>) {
    let Ok(path) = std::env::var("SPACESPORE_CAPTURE") else { return };
    let secs: f32 = std::env::var("SPACESPORE_CAPTURE_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(12.0);
    let t = time.elapsed_secs();
    if *done == 0 && std::env::var("SPACESPORE_EDITOR_DEMO").is_ok() && *state.get() == AppState::Editeur && t > secs * 0.5 {
        *done = 1;
        let mut m = format::Model::new("Demo", ModelKind::Personnage, None);
        m.race = Some(edit::RACES[0].to_string());
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
        let blocks_demo = std::env::var("SPACESPORE_EDITOR_DEMO").is_ok_and(|v| v == "blocs");
        if blocks_demo {
            let (blocks, _) = motion::load_blocks(None);
            let get = |id: &str| blocks.iter().find(|b| b.id == id).unwrap().clone();
            let p = motion::Placement { turn: 0, mirror: false, scale: 1 };
            let _ = doc.place_block(&get("bras"), IVec3::new(2, 19, 15), p, true);
            let _ = doc.place_block(&get("tete"), IVec3::new(6, 26, 16), p, false);
            let _ = doc.place_block(&get("queue"), IVec3::new(5, 11, 12), p, false);
        }
        editor.docs.push(doc);
        editor.overlay = None;
        let i = editor.docs.len() - 1;
        editor.select(i);
        if blocks_demo {
            editor.start_preview("marche");
        }
        // `SPACESPORE_EDITOR_DEMO=gabarit` : le gabarit d'une aile (tournée, reflétée) près du corps
        if std::env::var("SPACESPORE_EDITOR_DEMO").is_ok_and(|v| v == "gabarit") {
            editor.blocks = motion::load_blocks(None).0;
            if let Some(i) = editor.blocks.iter().position(|b| b.id == "aile") {
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
    if *done <= 1 && t > secs {
        *done = 2;
        commands.spawn(bevy::render::view::screenshot::Screenshot::primary_window()).observe(bevy::render::view::screenshot::save_to_disk(path));
    }
    if *done == 2 && t > secs + 3.0 {
        exit.send(AppExit::Success);
    }
}

/// En entrant : sans modèle ouvert, la fenêtre « Nouveau modèle » (création du personnage).
fn on_enter(mut editor: ResMut<Editor>) {
    editor.library = list_models(&library_dir());
    let (blocks, errors) = motion::load_blocks(Some(&blocks_dir()));
    editor.blocks = blocks;
    if !errors.is_empty() {
        warn!("Blocs de mouvement illisibles : {}", errors.join(" ; "));
        editor.say(format!("Blocs de mouvement illisibles : {}", errors.join(" ; ")));
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
