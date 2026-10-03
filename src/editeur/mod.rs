//! Éditeur de modèles voxel (0.12, `ROADMAP-0.12-editeur.md`) : personnages, vaisseaux, objets.
//!
//! Phase E0 (fondations) :
//! - l'état du jeu `AppState` (Jeu / Editeur) : en éditeur, le jeu ne réagit plus au clavier ni à la
//!   souris ; Échap ramène au jeu ;
//! - l'éditeur s'ouvre de lui-même à la **création du personnage** (premier lancement : aucune
//!   sauvegarde), sur le type Personnage, et depuis le menu du jeu (bouton « Éditeur de modèles ») ou
//!   `/editeur` ;
//! - le format `.ssvox` (`format.rs`), la bibliothèque `saves/modeles/` et l'import des modèles de
//!   Pixel World (`import.rs`, depuis `saves/import/`).
//!
//! L'édition elle-même (grille, caméra, outils) vient en E1.

pub mod format;
pub mod import;

use bevy::prelude::*;
use std::path::{Path, PathBuf};

use format::{Model, ModelKind, ShipCategory, CHARACTER_GRID, OTHER_MAX_GRID};

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
            .init_resource::<EditorSession>()
            .add_event::<OpenEditor>()
            .add_systems(Startup, open_on_first_launch)
            .add_systems(Update, (open_editor, menu_button))
            .add_systems(OnEnter(AppState::Editeur), (refresh_library, spawn_screen).chain())
            .add_systems(OnExit(AppState::Editeur), despawn_screen)
            .add_systems(Update, (screen_buttons, refresh_texts, leave_editor).chain().run_if(in_state(AppState::Editeur)));
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
    /// Fichier (ouvrir, renommer, supprimer : E1).
    #[allow(dead_code)]
    pub path: PathBuf,
    pub name: String,
    pub kind: ModelKind,
    pub voxels: usize,
    pub bytes: usize,
}

/// Ce que l'éditeur affiche.
#[derive(Resource, Default)]
pub struct EditorSession {
    pub kind: ModelKind,
    pub category: Option<ShipCategory>,
    pub library: Vec<LibraryEntry>,
    pub message: String,
    /// Ouvert pour la création du personnage (premier lancement).
    pub welcome: bool,
}

// ─────────────────────────────────────────────────────────────────────────
//  Bibliothèque (`saves/modeles/`) et import (`saves/import/`)
// ─────────────────────────────────────────────────────────────────────────

pub fn library_dir() -> PathBuf {
    crate::settings::data_dir().join("modeles")
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

fn open_on_first_launch(settings: Res<crate::settings::GameSettings>, mut open: EventWriter<OpenEditor>, mut session: ResMut<EditorSession>) {
    if settings.first_launch {
        session.welcome = true;
        open.send(OpenEditor(ModelKind::Personnage));
    }
}

fn open_editor(mut events: EventReader<OpenEditor>, mut session: ResMut<EditorSession>, mut next: ResMut<NextState<AppState>>, mut menu: ResMut<crate::ui::MenuState>) {
    for OpenEditor(kind) in events.read() {
        session.kind = *kind;
        if *kind == ModelKind::Vaisseau && session.category.is_none() {
            session.category = Some(ShipCategory::Chasseur);
        }
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

fn refresh_library(mut session: ResMut<EditorSession>) {
    session.library = list_models(&library_dir());
}

fn leave_editor(keys: Res<ButtonInput<KeyCode>>, mut next: ResMut<NextState<AppState>>, mut session: ResMut<EditorSession>, quit: Query<&Interaction, (Changed<Interaction>, With<QuitButton>)>) {
    if keys.just_pressed(KeyCode::Escape) || quit.iter().any(|i| *i == Interaction::Pressed) {
        session.welcome = false;
        next.set(AppState::Jeu);
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  Écran de l'éditeur (E0 : choix du type, bibliothèque, import)
// ─────────────────────────────────────────────────────────────────────────

const BG: Color = Color::srgba(0.03, 0.04, 0.07, 0.97);
const PANEL: Color = Color::srgb(0.08, 0.1, 0.15);
const ACCENT: Color = Color::srgb(0.35, 0.65, 1.0);
const TEXT: Color = Color::srgb(0.88, 0.92, 1.0);
const DIM: Color = Color::srgb(0.6, 0.66, 0.75);

#[derive(Component)]
struct EditorScreen;

#[derive(Component, Clone, Copy, PartialEq)]
enum ScreenButton {
    Kind(ModelKind),
    Category(ShipCategory),
    New,
    Import,
}

#[derive(Component)]
struct QuitButton;

#[derive(Component)]
enum ScreenText {
    Header,
    Grid,
    Library,
    Message,
}

fn button(p: &mut ChildBuilder, label: &str, marker: impl Bundle) {
    p.spawn((
        Button,
        Node { padding: UiRect::axes(Val::Px(14.0), Val::Px(7.0)), border: UiRect::all(Val::Px(2.0)), ..default() },
        BackgroundColor(PANEL),
        BorderColor(ACCENT),
        BorderRadius::all(Val::Px(6.0)),
        marker,
    ))
    .with_child((Text::new(label), TextFont { font_size: 16.0, ..default() }, TextColor(TEXT)));
}

fn text(p: &mut ChildBuilder, size: f32, color: Color, marker: ScreenText) {
    p.spawn((Text::new(""), TextFont { font_size: size, ..default() }, TextColor(color), marker));
}

fn row(p: &mut ChildBuilder, f: impl FnOnce(&mut ChildBuilder)) {
    p.spawn(Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(10.0), flex_wrap: FlexWrap::Wrap, ..default() }).with_children(f);
}

fn spawn_screen(mut commands: Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(36.0)),
                row_gap: Val::Px(16.0),
                ..default()
            },
            BackgroundColor(BG),
            GlobalZIndex(50),
            // Les clics ne traversent pas vers le jeu
            Interaction::default(),
            EditorScreen,
        ))
        .with_children(|p| {
            p.spawn((Text::new("EDITEUR DE MODELES"), TextFont { font_size: 30.0, ..default() }, TextColor(ACCENT)));
            text(p, 17.0, TEXT, ScreenText::Header);
            row(p, |r| {
                for k in ModelKind::ALL {
                    button(r, k.name(), ScreenButton::Kind(k));
                }
            });
            row(p, |r| {
                for c in ShipCategory::ALL {
                    button(r, &format!("{} ({}³)", c.name(), c.grid()), ScreenButton::Category(c));
                }
            });
            text(p, 15.0, DIM, ScreenText::Grid);
            row(p, |r| {
                button(r, "Nouveau modele", ScreenButton::New);
                button(r, "Importer Pixel World", ScreenButton::Import);
                button(r, "Retour au jeu (Echap)", QuitButton);
            });
            text(p, 15.0, Color::srgb(1.0, 0.85, 0.5), ScreenText::Message);
            text(p, 15.0, TEXT, ScreenText::Library);
        });
}

fn despawn_screen(mut commands: Commands, q: Query<Entity, With<EditorScreen>>) {
    for e in &q {
        commands.entity(e).try_despawn_recursive();
    }
}

fn grid_text(kind: ModelKind, category: Option<ShipCategory>) -> String {
    match kind {
        ModelKind::Personnage => format!(
            "Personnage : grille fixe {} de large x {} de haut x {} de long (queues, centaures, lamias). La race et son squelette anime arrivent en E1 et E5.",
            CHARACTER_GRID.x, CHARACTER_GRID.y, CHARACTER_GRID.z
        ),
        ModelKind::Vaisseau => {
            let c = category.unwrap_or(ShipCategory::Chasseur);
            format!("Vaisseau {} : grille {g} x {g} x {g} (stockage creux par blocs de 32, 10 Mo au plus par fichier).", c.name(), g = c.grid())
        }
        ModelKind::Autre => format!("Autre (objet, arme, meuble, decor) : grille libre jusqu'a {m} x {m} x {m}.", m = OTHER_MAX_GRID),
    }
}

#[allow(clippy::too_many_arguments)]
fn screen_buttons(
    interactions: Query<(&Interaction, &ScreenButton), Changed<Interaction>>,
    mut session: ResMut<EditorSession>,
) {
    for (i, b) in &interactions {
        if *i != Interaction::Pressed {
            continue;
        }
        match *b {
            ScreenButton::Kind(k) => {
                session.kind = k;
                if k == ModelKind::Vaisseau && session.category.is_none() {
                    session.category = Some(ShipCategory::Chasseur);
                }
            }
            ScreenButton::Category(c) => {
                session.kind = ModelKind::Vaisseau;
                session.category = Some(c);
            }
            ScreenButton::New => {
                let n = session.library.iter().filter(|e| e.kind == session.kind).count() + 1;
                let name = format!("{} {n}", session.kind.name());
                let model = Model::new(&name, session.kind, (session.kind == ModelKind::Vaisseau).then_some(session.category.unwrap_or(ShipCategory::Chasseur)));
                session.message = match save_model(&library_dir(), &model) {
                    Ok(p) => format!("Modele vide \"{name}\" cree : {}. L'edition arrive en E1.", p.display()),
                    Err(e) => e,
                };
                session.library = list_models(&library_dir());
            }
            ScreenButton::Import => {
                let from = import_dir();
                session.message = match import_folder(&from, &library_dir()) {
                    Ok(r) if r.models == 0 => format!("Aucun JSON de Pixel World dans {} : copiez-y vos modeles (.json).", from.display()),
                    Ok(r) => format!(
                        "{} modele(s) importe(s), {} voxels ({} blocs inconnus, {} hors de la grille).",
                        r.models, r.voxels, r.unknown, r.outside
                    ),
                    Err(e) => {
                        let _ = std::fs::create_dir_all(&from);
                        format!("Import impossible : {e}. Copiez vos JSON de Pixel World dans {}.", from.display())
                    }
                };
                session.library = list_models(&library_dir());
            }
        }
    }
}

fn refresh_texts(
    session: Res<EditorSession>,
    mut texts: Query<(&ScreenText, &mut Text)>,
    mut buttons: Query<(&ScreenButton, &mut BorderColor)>,
) {
    if !session.is_changed() {
        return;
    }
    for (which, mut t) in &mut texts {
        let s = match which {
            ScreenText::Header => {
                if session.welcome {
                    "Bienvenue ! Cree ton personnage : choisis le type de modele, puis construis-le (l'edition arrive en E1).".to_string()
                } else {
                    "Choisis le type de modele.".to_string()
                }
            }
            ScreenText::Grid => grid_text(session.kind, session.category),
            ScreenText::Message => session.message.clone(),
            ScreenText::Library => {
                let mut lines = vec![format!("Bibliotheque ({}) : {} modele(s)", library_dir().display(), session.library.len())];
                for e in session.library.iter().take(14) {
                    lines.push(format!("  {}  -  {}, {} voxels, {:.1} Ko", e.name, e.kind.name(), e.voxels, e.bytes as f64 / 1024.0));
                }
                if session.library.len() > 14 {
                    lines.push(format!("  ... et {} autres", session.library.len() - 14));
                }
                lines.join("\n")
            }
        };
        if t.0 != s {
            t.0 = s;
        }
    }
    for (b, mut border) in &mut buttons {
        let on = match *b {
            ScreenButton::Kind(k) => k == session.kind,
            ScreenButton::Category(c) => session.kind == ModelKind::Vaisseau && session.category == Some(c),
            _ => false,
        };
        border.0 = if on { Color::srgb(1.0, 0.8, 0.3) } else { ACCENT };
    }
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
