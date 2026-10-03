//! Interface de l'éditeur (E1) : outils et couleurs à gauche, onglets en haut, modèle à droite,
//! barre d'état en bas, et les fenêtres « Nouveau modèle », « Bibliothèque », « Renommer »,
//! « Étiquettes ». L'interface du jeu est masquée pendant l'édition.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::ButtonState;
use bevy::prelude::*;

use super::edit::{self, Brush, ClipTransform, Tool, TAG_GROUPS};
use super::format::{Material, Model, ModelKind, PaletteEntry, ShipCategory, CHARACTER_GRID, MAX_FILE_BYTES, OTHER_MAX_GRID};
use super::palette;
use super::{library_dir, list_models, save_model, AppState, Editor, Overlay};

const PANEL: Color = Color::srgba(0.05, 0.06, 0.1, 0.94);
const BUTTON: Color = Color::srgb(0.1, 0.13, 0.2);
const ACCENT: Color = Color::srgb(0.35, 0.65, 1.0);
const ON: Color = Color::srgb(1.0, 0.8, 0.3);
const TEXT: Color = Color::srgb(0.88, 0.92, 1.0);
const DIM: Color = Color::srgb(0.6, 0.66, 0.75);

/// Un panneau de l'éditeur : la souris y est hors de la vue 3D.
#[derive(Component)]
pub struct Blocks;

/// Panneau défilant : 0 = gauche (outils), 1 = droite (modèle et zones), 2 = choix de la race.
#[derive(Component)]
pub struct ScrollPanel(pub usize);

/// La molette fait défiler le panneau sous la souris.
pub fn scroll_panels(mut wheel: EventReader<bevy::input::mouse::MouseWheel>, mut editor: ResMut<Editor>, mut q: Query<(&Interaction, &mut ScrollPosition, &ScrollPanel)>) {
    let lines: f32 = wheel.read().map(crate::ui::wheel_lines).sum();
    for (i, mut s, panel) in &mut q {
        let k = panel.0.min(2);
        // La mise en page ramène la position dans le contenu : on garde la vraie
        if (editor.scroll[k] - s.offset_y).abs() > 0.5 {
            editor.scroll[k] = s.offset_y;
        }
        if *i == Interaction::None || lines == 0.0 {
            continue;
        }
        s.offset_y = (s.offset_y - lines * 40.0).clamp(0.0, 2_000.0);
        editor.scroll[k] = s.offset_y;
    }
}

/// Racine de l'interface de l'éditeur.
#[derive(Component)]
pub struct EditorUi;

#[derive(Component)]
pub enum Live {
    Status,
    Info,
    Rename,
}

/// Ce que fait un bouton.
#[derive(Component, Clone, Copy, PartialEq)]
pub enum Act {
    Tool(Tool),
    Mirror,
    Grid,
    Undo,
    Redo,
    Focus,
    /// Choisir une couleur (de la grille, d'un thème, des récentes).
    Pick(PaletteEntry),
    /// Une couleur du modèle (index de palette) : clic = la choisir, clic droit = la remplacer.
    ModelColor(u8),
    Material(Material),
    Saturation(palette::Saturation),
    Theme(usize),
    Hex,
    Tab(usize),
    CloseTab,
    New,
    Library,
    Save,
    Quit,
    Rename,
    Tags,
    CloseOverlay,
    NewKind(ModelKind),
    NewRace(usize),
    NewCategory(ShipCategory),
    NewSize(u32),
    Create,
    Open(usize),
    Duplicate(usize),
    Delete(usize),
    Tag(&'static str),
    RenameOk,
    Import,
    /// Bloc de mouvement à poser (index dans `Editor::blocks`).
    Block(usize),
    /// Retirer une zone de mouvement (ses blocs restent).
    RemoveZone(usize),
    /// Aperçu ▶ (P).
    Preview,
    /// Animation jouée par l'aperçu (index dans `motion::model_anims`).
    Anim(usize),
    /// Cocher / décocher un membre optionnel de la race choisie.
    RaceOption(usize),
    /// Mode des outils de volume.
    Brush(Brush),
    /// Sélection et presse-papiers.
    Clip(ClipAct),
    /// Calque courant, calque montré / caché, nouveau calque.
    Layer(u8),
    LayerVisible(u8),
    AddLayer,
    /// Coupe : axe (aucune = `None`), déplacer.
    Cut(Option<usize>),
    CutMove(i32),
}

/// Ce qu'on fait de la sélection.
#[derive(Clone, Copy, PartialEq)]
pub enum ClipAct {
    Copy,
    Cut,
    Paste,
    Delete,
    Transform(ClipTransform),
    Deselect,
}

fn button(p: &mut ChildBuilder, label: &str, act: Act, on: bool) {
    p.spawn((
        Button,
        Node { padding: UiRect::axes(Val::Px(9.0), Val::Px(5.0)), border: UiRect::all(Val::Px(2.0)), ..default() },
        BackgroundColor(BUTTON),
        BorderColor(if on { ON } else { ACCENT }),
        BorderRadius::all(Val::Px(5.0)),
        act,
    ))
    .with_child((Text::new(label), TextFont { font_size: 14.0, ..default() }, TextColor(TEXT)));
}

fn label(p: &mut ChildBuilder, s: &str, size: f32, color: Color) {
    p.spawn((Text::new(s), TextFont { font_size: size, ..default() }, TextColor(color)));
}

fn wrap(p: &mut ChildBuilder, f: impl FnOnce(&mut ChildBuilder)) {
    p.spawn(Node { flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap, column_gap: Val::Px(6.0), row_gap: Val::Px(6.0), ..default() }).with_children(f);
}

fn panel(node: Node) -> impl Bundle {
    (node, BackgroundColor(PANEL), BorderRadius::all(Val::Px(8.0)), Interaction::default(), Blocks)
}

/// Pastille de couleur (avec la lettre de sa matière).
fn chip(w: &mut ChildBuilder, e: &PaletteEntry, act: Act, on: bool) {
    swatch_button(w, e, act, on, 22.0, 22.0, true);
}

/// Case de la grande palette.
fn cell(w: &mut ChildBuilder, e: &PaletteEntry, act: Act, on: bool) {
    swatch_button(w, e, act, on, 17.0, 12.0, false);
}

fn swatch_button(w: &mut ChildBuilder, e: &PaletteEntry, act: Act, on: bool, width: f32, height: f32, mark: bool) {
    let letter = match e.material {
        Material::Mate => "",
        Material::Metal => "M",
        Material::Verre => "V",
        Material::Lumineuse => "L",
    };
    let mut b = w.spawn((
        Button,
        Node { width: Val::Px(width), height: Val::Px(height), border: UiRect::all(Val::Px(if on { 2.0 } else { if mark { 1.0 } else { 0.0 } })), justify_content: JustifyContent::Center, ..default() },
        BackgroundColor(swatch(e)),
        BorderColor(if on { ON } else { Color::srgba(0.0, 0.0, 0.0, 0.5) }),
        act,
    ));
    if mark && !letter.is_empty() {
        b.with_child((Text::new(letter), TextFont { font_size: 11.0, ..default() }, TextColor(Color::srgba(0.0, 0.0, 0.0, 0.8))));
    }
}

/// Couleur d'affichage d'une entrée de palette.
fn swatch(e: &super::format::PaletteEntry) -> Color {
    Color::srgb_u8(e.rgb[0], e.rgb[1], e.rgb[2])
}

/// Reconstruit l'interface quand l'état change (onglet, outil, fenêtre...).
pub fn rebuild(mut commands: Commands, mut editor: ResMut<Editor>, old: Query<Entity, With<EditorUi>>) {
    if !editor.ui_dirty {
        return;
    }
    editor.ui_dirty = false;
    for e in &old {
        commands.entity(e).try_despawn_recursive();
    }
    let ed = &*editor;
    commands
        .spawn((Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, GlobalZIndex(60), EditorUi))
        .with_children(|root| {
            // ── Gauche : outils et couleurs (défile à la molette) ──
            root.spawn(panel(Node {
                position_type: PositionType::Absolute,
                left: Val::Px(8.0),
                top: Val::Px(8.0),
                bottom: Val::Px(40.0),
                width: Val::Px(250.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                padding: UiRect::all(Val::Px(10.0)),
                overflow: Overflow::scroll_y(),
                ..default()
            }))
            .insert((ScrollPosition { offset_x: 0.0, offset_y: ed.scroll[0] }, ScrollPanel(0)))
            .with_children(|p| {
                label(p, "EDITEUR", 20.0, ACCENT);
                wrap(p, |w| {
                    for t in Tool::ALL {
                        button(w, t.name(), Act::Tool(t), ed.tool == t);
                    }
                });
                label(p, "Volumes (molette pendant le trace : epaisseur)", 13.0, DIM);
                wrap(p, |w| {
                    for t in Tool::VOLUME {
                        button(w, t.name(), Act::Tool(t), ed.tool == t);
                    }
                });
                if Tool::VOLUME.contains(&ed.tool) && ed.tool != Tool::Select {
                    wrap(p, |w| {
                        label(w, "Mode :", 13.0, DIM);
                        for b in Brush::ALL {
                            button(w, b.name(), Act::Brush(b), ed.brush == b);
                        }
                    });
                }
                if ed.tool == Tool::Select || ed.selection.is_some() || ed.pasting {
                    label(p, "Selection", 13.0, DIM);
                    wrap(p, |w| {
                        button(w, "Copier (Ctrl+C)", Act::Clip(ClipAct::Copy), false);
                        button(w, "Couper (Ctrl+X)", Act::Clip(ClipAct::Cut), false);
                        button(w, "Coller (Ctrl+V)", Act::Clip(ClipAct::Paste), ed.pasting);
                        button(w, "Effacer (Suppr)", Act::Clip(ClipAct::Delete), false);
                        for (a, n) in ["x", "y", "z"].iter().enumerate() {
                            button(w, &format!("Tourner {n}{}", if a == 1 { " (R)" } else { "" }), Act::Clip(ClipAct::Transform(ClipTransform::Turn(a))), false);
                        }
                        for (a, n) in ["x", "y", "z"].iter().enumerate() {
                            button(w, &format!("Retourner {n}"), Act::Clip(ClipAct::Transform(ClipTransform::Mirror(a))), false);
                        }
                        button(w, "Deselectionner", Act::Clip(ClipAct::Deselect), false);
                    });
                }
                wrap(p, |w| {
                    button(w, "Miroir (X)", Act::Mirror, ed.mirror);
                    button(w, "Grille (G)", Act::Grid, ed.grid);
                    button(w, "Cadrer (F)", Act::Focus, false);
                });
                wrap(p, |w| {
                    button(w, "Annuler (Ctrl+Z)", Act::Undo, false);
                    button(w, "Retablir (Ctrl+Y)", Act::Redo, false);
                });
                if let Some(d) = ed.doc() {
                    label(p, "Calques (clic : calque courant)", 13.0, DIM);
                    for (k, l) in d.model.layer_list().iter().enumerate() {
                        p.spawn(Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(6.0), ..default() }).with_children(|r| {
                            button(r, if l.visible { "Vu" } else { "Cache" }, Act::LayerVisible(k as u8), !l.visible);
                            button(r, &l.name, Act::Layer(k as u8), d.layer as usize == k);
                        });
                    }
                    wrap(p, |w| button(w, "+ Calque", Act::AddLayer, false));
                    let axis = d.cut.map(|c| c.axis);
                    wrap(p, |w| {
                        label(w, "Coupe (C) :", 13.0, DIM);
                        button(w, "aucune", Act::Cut(None), axis.is_none());
                        for (a, n) in ["x", "y", "z"].iter().enumerate() {
                            button(w, n, Act::Cut(Some(a)), axis == Some(a));
                        }
                        if let Some(c) = d.cut {
                            button(w, "-", Act::CutMove(-1), false);
                            label(w, &c.pos.to_string(), 14.0, ON);
                            button(w, "+", Act::CutMove(1), false);
                        }
                    });
                }
                let blocks = ed.blocks_for_doc();
                if !blocks.is_empty() {
                    label(p, "Blocs de mouvement", 13.0, DIM);
                    wrap(p, |w| {
                        for i in blocks {
                            button(w, &ed.lib.blocks[i].name, Act::Block(i), ed.placing.is_some_and(|x| x.block == i));
                        }
                    });
                    if let Some(x) = ed.placing {
                        let scale = if ed.lib.blocks.get(x.block).is_some_and(|b| b.scalable) { format!(", taille {}", x.place.scale) } else { String::new() };
                        let mirror = if x.place.mirror { ", reflete" } else { "" };
                        label(p, &format!("Molette : tourner ({} quart(s){scale}{mirror})", x.place.turn), 13.0, ON);
                    }
                }
                label(p, &format!("Couleur : {} ({})", hex(&ed.color.rgb), ed.color.material.name()), 14.0, TEXT);
                p.spawn((Node { width: Val::Px(226.0), height: Val::Px(20.0), ..default() }, BackgroundColor(swatch(&ed.color)), BorderRadius::all(Val::Px(4.0))));
                wrap(p, |w| {
                    for m in [Material::Mate, Material::Metal, Material::Verre, Material::Lumineuse] {
                        button(w, m.name(), Act::Material(m), ed.color.material == m);
                    }
                    button(w, "Hex...", Act::Hex, false);
                });
                if !ed.recent.is_empty() {
                    label(p, "Recentes", 13.0, DIM);
                    wrap(p, |w| {
                        for e in &ed.recent {
                            chip(w, e, Act::Pick(*e), *e == ed.color);
                        }
                    });
                }
                label(p, "Palettes", 13.0, DIM);
                wrap(p, |w| {
                    for (i, (name, _)) in palette::THEMES.iter().enumerate() {
                        button(w, name, Act::Theme(i), ed.theme == i);
                    }
                });
                wrap(p, |w| {
                    for e in palette::theme(ed.theme) {
                        chip(w, &e, Act::Pick(e), e == ed.color);
                    }
                });
                if let Some(d) = ed.doc() {
                    label(p, "Couleurs du modele (clic droit : remplacer partout)", 13.0, DIM);
                    wrap(p, |w| {
                        for i in palette::sorted(&d.model.palette) {
                            let e = d.model.palette[i];
                            chip(w, &e, Act::ModelColor(i as u8 + 1), e == ed.color);
                        }
                    });
                }
            });

            // ── Bas : la grande palette OKLCH (cachée pendant le choix de la race) ──
            if !ed.race_shown() {
                root.spawn(panel(Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(266.0),
                    right: Val::Px(276.0),
                    bottom: Val::Px(40.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(4.0),
                    padding: UiRect::all(Val::Px(8.0)),
                    ..default()
                }))
                .with_children(|p| {
                    wrap(p, |w| {
                        label(w, "Palette (36 teintes x 12 clartes)", 13.0, DIM);
                        for s in palette::Saturation::ALL {
                            button(w, s.name(), Act::Saturation(s), ed.saturation == s);
                        }
                    });
                    for row in (0..palette::LIGHTS).rev() {
                        p.spawn(Node { flex_direction: FlexDirection::Row, ..default() }).with_children(|r| {
                            for col in 0..palette::HUES {
                                let rgb = palette::grid_color(ed.saturation, col, row);
                                let e = PaletteEntry { rgb, material: ed.color.material };
                                cell(r, &e, Act::Pick(e), e == ed.color);
                            }
                        });
                    }
                    p.spawn(Node { flex_direction: FlexDirection::Row, margin: UiRect::top(Val::Px(3.0)), ..default() }).with_children(|r| {
                        for i in 0..palette::GREYS {
                            let e = PaletteEntry { rgb: palette::grey(i), material: ed.color.material };
                            cell(r, &e, Act::Pick(e), e == ed.color);
                        }
                    });
                });

            }

            // ── Haut : onglets et fichiers ──
            root.spawn(panel(Node {
                position_type: PositionType::Absolute,
                left: Val::Px(266.0),
                right: Val::Px(276.0),
                top: Val::Px(8.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(6.0),
                row_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(8.0)),
                ..default()
            }))
            .with_children(|p| {
                for (i, d) in ed.docs.iter().enumerate() {
                    let star = if d.dirty { " *" } else { "" };
                    button(p, &format!("{}{star}", d.model.name), Act::Tab(i), i == ed.current);
                }
                if !ed.docs.is_empty() {
                    button(p, "Fermer l'onglet", Act::CloseTab, false);
                }
                button(p, "Nouveau", Act::New, false);
                button(p, "Bibliotheque", Act::Library, false);
                button(p, "Enregistrer (Ctrl+S)", Act::Save, false);
                button(p, "Retour au jeu (Echap)", Act::Quit, false);
            });

            // ── Droite : le modèle ──
            root.spawn(panel(Node {
                position_type: PositionType::Absolute,
                right: Val::Px(8.0),
                top: Val::Px(8.0),
                bottom: Val::Px(40.0),
                width: Val::Px(260.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                padding: UiRect::all(Val::Px(10.0)),
                overflow: Overflow::scroll_y(),
                ..default()
            }))
            .insert((ScrollPosition { offset_x: 0.0, offset_y: ed.scroll[1] }, ScrollPanel(1)))
            .with_children(|p| {
                label(p, "MODELE", 18.0, ACCENT);
                p.spawn((Text::new(""), TextFont { font_size: 14.0, ..default() }, TextColor(TEXT), Live::Info));
                if !ed.docs.is_empty() {
                    wrap(p, |w| {
                        button(w, "Renommer", Act::Rename, false);
                        button(w, "Etiquettes", Act::Tags, false);
                    });
                }
                if let Some(d) = ed.doc() {
                    let zones = &d.model.zones;
                    label(p, &format!("ZONES DE MOUVEMENT ({})", zones.len()), 15.0, ACCENT);
                    if zones.is_empty() {
                        label(p, "Choisis un bloc de mouvement a gauche et pose-le.", 13.0, DIM);
                    } else {
                        wrap(p, |w| button(w, if ed.preview.is_some() { "Arreter l'apercu (P)" } else { "> Apercu (P)" }, Act::Preview, ed.preview.is_some()));
                        if let Some(pv) = &ed.preview {
                            wrap(p, |w| {
                                for (k, a) in super::motion::model_anims(&d.model, &ed.lib).iter().enumerate() {
                                    button(w, a, Act::Anim(k), *a == pv.anim);
                                }
                            });
                        }
                        for (i, z) in zones.iter().enumerate().take(64) {
                            let depth = std::iter::successors(z.parent, |k| zones.get(*k as usize).and_then(|x| x.parent)).take(8).count();
                            p.spawn(Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(6.0), align_items: AlignItems::Center, ..default() }).with_children(|r| {
                                let color = if ed.colliding.contains(&i) { Color::srgb(1.0, 0.35, 0.3) } else { TEXT };
                                label(r, &format!("{}{}", "  ".repeat(depth), z.name), 13.0, color);
                                button(r, "Retirer", Act::RemoveZone(i), false);
                            });
                        }
                        if zones.len() > 64 {
                            label(p, &format!("... et {} autres", zones.len() - 64), 13.0, DIM);
                        }
                    }
                }
            });

            // ── Bas : état ──
            root.spawn(panel(Node {
                position_type: PositionType::Absolute,
                left: Val::Px(8.0),
                right: Val::Px(8.0),
                bottom: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)),
                ..default()
            }))
            .with_child((Text::new(""), TextFont { font_size: 14.0, ..default() }, TextColor(TEXT), Live::Status));

            if let Some(o) = &ed.overlay {
                overlay(root, ed, o);
            }
        });
}

fn hex(rgb: &[u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2])
}

/// Fenêtre au centre de l'écran.
fn overlay(root: &mut ChildBuilder, ed: &Editor, o: &Overlay) {
    // Choix de la race : la fenêtre à gauche, le rig animé à droite
    let side = *o == Overlay::New && ed.new_kind == ModelKind::Personnage;
    root.spawn(panel(Node {
        position_type: PositionType::Absolute,
        left: if side { Val::Px(8.0) } else { Val::Percent(50.0) },
        top: Val::Px(if side { 8.0 } else { 70.0 }),
        width: Val::Px(if side { 640.0 } else { 760.0 }),
        bottom: if side { Val::Px(40.0) } else { Val::Auto },
        margin: UiRect::left(Val::Px(if side { 0.0 } else { -380.0 })),
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(10.0),
        padding: UiRect::all(Val::Px(16.0)),
        border: UiRect::all(Val::Px(2.0)),
        overflow: if side { Overflow::scroll_y() } else { Overflow::DEFAULT },
        ..default()
    }))
    .insert((BorderColor(ACCENT), ScrollPosition { offset_x: 0.0, offset_y: if side { ed.scroll[2] } else { 0.0 } }, ScrollPanel(2)))
    .with_children(|p| match o {
        Overlay::New => {
            label(p, "NOUVEAU MODELE", 20.0, ACCENT);
            if ed.welcome {
                label(p, "Bienvenue ! Cree ton personnage : choisis ta race, puis construis-le bloc par bloc.", 15.0, ON);
            }
            wrap(p, |w| {
                for k in ModelKind::ALL {
                    button(w, k.name(), Act::NewKind(k), ed.new_kind == k);
                }
            });
            match ed.new_kind {
                ModelKind::Personnage => {
                    label(p, &format!("Race (grille {} x {} x {}) : tu recois son squelette en blocs blancs deja animes", CHARACTER_GRID.x, CHARACTER_GRID.y, CHARACTER_GRID.z), 14.0, DIM);
                    wrap(p, |w| {
                        for (i, r) in ed.lib.races.iter().enumerate() {
                            button(w, &r.name, Act::NewRace(i), ed.new_race == Some(i));
                        }
                    });
                    if let Some(r) = ed.new_race.and_then(|r| ed.lib.races.get(r)) {
                        label(p, &format!("{} - {} ({})", r.name, r.races.join(", "), r.locomotion), 14.0, ON);
                        if !r.options.is_empty() {
                            label(p, "Membres optionnels", 13.0, DIM);
                            wrap(p, |w| {
                                for (k, o) in r.options.iter().enumerate() {
                                    let on = ed.new_options.get(k).copied().unwrap_or(false);
                                    button(w, &format!("{} {}", if on { "[x]" } else { "[ ]" }, o.name), Act::RaceOption(k), on);
                                }
                            });
                        }
                        if let (Some(d), Some(pv)) = (ed.race_view.as_ref(), &ed.preview) {
                            label(p, "Apercu", 13.0, DIM);
                            wrap(p, |w| {
                                for (k, a) in super::motion::model_anims(&d.model, &ed.lib).iter().enumerate() {
                                    let name = ed.lib.anim(a).map_or(a.as_str(), |x| x.name.as_str());
                                    button(w, name, Act::Anim(k), *a == pv.anim);
                                }
                            });
                        }
                    }
                }
                ModelKind::Vaisseau => {
                    label(p, "Categorie (taille de la grille)", 14.0, DIM);
                    wrap(p, |w| {
                        for c in ShipCategory::ALL {
                            button(w, &format!("{} {}³", c.name(), c.grid()), Act::NewCategory(c), ed.new_category == c);
                        }
                    });
                }
                ModelKind::Autre => {
                    label(p, &format!("Taille de la grille (jusqu'a {m}³)", m = OTHER_MAX_GRID), 14.0, DIM);
                    wrap(p, |w| {
                        for s in [8, 16, 32, 48, 64] {
                            button(w, &format!("{s}³"), Act::NewSize(s), ed.new_size == s);
                        }
                    });
                }
            }
            wrap(p, |w| {
                button(w, "Creer", Act::Create, true);
                if !ed.docs.is_empty() || !ed.welcome {
                    button(w, "Annuler", Act::CloseOverlay, false);
                }
            });
        }
        Overlay::Library => {
            label(p, &format!("BIBLIOTHEQUE ({})", library_dir().display()), 18.0, ACCENT);
            if ed.library.is_empty() {
                label(p, "Aucun modele enregistre. Pour importer : copiez des JSON de Pixel World dans saves/import/.", 14.0, DIM);
            }
            for (i, e) in ed.library.iter().enumerate().take(18) {
                p.spawn(Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(8.0), align_items: AlignItems::Center, ..default() }).with_children(|r| {
                    label(r, &format!("{}  -  {}, {} voxels, {:.1} Ko", e.name, e.kind.name(), e.voxels, e.bytes as f64 / 1024.0), 14.0, TEXT);
                    button(r, "Ouvrir", Act::Open(i), false);
                    button(r, "Dupliquer", Act::Duplicate(i), false);
                    let sure = ed.pending_delete == Some(i);
                    button(r, if sure { "Confirmer la suppression" } else { "Supprimer" }, Act::Delete(i), sure);
                });
            }
            if ed.library.len() > 18 {
                label(p, &format!("... et {} autres", ed.library.len() - 18), 13.0, DIM);
            }
            wrap(p, |w| {
                button(w, "Importer Pixel World (saves/import/)", Act::Import, false);
                button(w, "Fermer", Act::CloseOverlay, false);
            });
        }
        Overlay::Hex(_) => {
            label(p, "COULEUR LIBRE : code hexadecimal (ex. FF8000), Entree : valider", 18.0, ACCENT);
            p.spawn((Text::new(""), TextFont { font_size: 18.0, ..default() }, TextColor(ON), Live::Rename));
            wrap(p, |w| {
                button(w, "Valider", Act::RenameOk, true);
                button(w, "Annuler", Act::CloseOverlay, false);
            });
        }
        Overlay::Rename(_) => {
            label(p, "RENOMMER (Entree : valider, Echap : annuler)", 18.0, ACCENT);
            p.spawn((Text::new(""), TextFont { font_size: 18.0, ..default() }, TextColor(ON), Live::Rename));
            wrap(p, |w| {
                button(w, "Valider", Act::RenameOk, true);
                button(w, "Annuler", Act::CloseOverlay, false);
            });
        }
        Overlay::Tags => {
            label(p, "ETIQUETTES", 18.0, ACCENT);
            let tags = ed.doc().map(|d| d.model.tags.clone()).unwrap_or_default();
            for (group, list) in TAG_GROUPS {
                label(p, group, 14.0, DIM);
                wrap(p, |w| {
                    for t in list.iter() {
                        button(w, t, Act::Tag(t), tags.iter().any(|x| x == t));
                    }
                });
            }
            wrap(p, |w| button(w, "Fermer", Act::CloseOverlay, false));
        }
    });
}

/// Textes qui changent à chaque image.
pub fn live_texts(mut editor: ResMut<Editor>, chunks: Res<super::view::ChunkMeshes>, mut q: Query<(&Live, &mut Text)>) {
    editor.refresh_size();
    let ed = &*editor;
    for (which, mut t) in &mut q {
        let s = match which {
            Live::Status => {
                let at = match (ed.tool, ed.hover) {
                    (Tool::Add, (_, Some(p))) | (_, (Some(p), _)) => format!("case {} {} {}   ", p.x, p.y, p.z),
                    _ => String::new(),
                };
                let msg = if ed.message.is_empty() { "Clic gauche : outil   Clic droit : tourner   Clic molette : deplacer   Molette : zoom   Fleches : onglets".to_string() } else { ed.message.clone() };
                format!("{at}{msg}")
            }
            Live::Info => match ed.doc() {
                Some(d) => {
                    let m = &d.model;
                    let what = match (m.kind, &m.race, m.category) {
                        (ModelKind::Personnage, Some(r), _) => format!("Personnage - {r}"),
                        (ModelKind::Vaisseau, _, Some(c)) => format!("Vaisseau - {}", c.name()),
                        (k, _, _) => k.name().to_string(),
                    };
                    let size = ed.size_bytes.map_or("?".into(), |b| format!("{:.2}", b as f64 / (1024.0 * 1024.0)));
                    let full = ed.size_bytes.is_some_and(|b| b > MAX_FILE_BYTES);
                    format!(
                        "{}\n{what}\nGrille {} x {} x {}\n{} voxels, {} couleurs, {} zones\nFichier : {size} / 10 Mo{}\nEtiquettes : {}\n{}{}",
                        m.name,
                        m.size.x,
                        m.size.y,
                        m.size.z,
                        ed.voxel_count,
                        m.palette.len(),
                        m.zones.len(),
                        if full { "  TROP LOURD" } else { "" },
                        if m.tags.is_empty() { "aucune".into() } else { m.tags.join(", ") },
                        d.path.as_ref().map_or("pas encore enregistre".into(), |p| p.file_name().map_or(String::new(), |f| f.to_string_lossy().to_string())),
                        if chunks.busy() > 0 { format!("\nMaillage : {} chunks en attente", chunks.busy()) } else { String::new() },
                    )
                }
                None => "Aucun modele ouvert : Nouveau ou Bibliotheque.".into(),
            },
            Live::Rename => match &ed.overlay {
                Some(Overlay::Rename(s)) | Some(Overlay::Hex(s)) => format!("{s}_"),
                _ => String::new(),
            },
        };
        if t.0 != s {
            t.0 = s;
        }
    }
}

/// Saisie du nouveau nom (clavier logique : AZERTY compris).
pub fn typing(mut events: EventReader<KeyboardInput>, mut editor: ResMut<Editor>) {
    let (mut name, hex_mode) = match editor.overlay.clone() {
        Some(Overlay::Rename(s)) => (s, false),
        Some(Overlay::Hex(s)) => (s, true),
        _ => {
            events.clear();
            return;
        }
    };
    let mut done = None;
    for ev in events.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        match &ev.logical_key {
            Key::Character(c) => {
                if name.chars().count() < 40 {
                    name.push_str(c);
                }
            }
            Key::Space => name.push(' '),
            Key::Backspace => {
                name.pop();
            }
            Key::Enter => done = Some(true),
            Key::Escape => done = Some(false),
            _ => {}
        }
    }
    match done {
        Some(true) if hex_mode => editor.apply_hex(&name),
        Some(true) => editor.rename(name),
        Some(false) => {
            editor.overlay = None;
            editor.ui_dirty = true;
            editor.escape_used = true;
        }
        None if hex_mode => editor.overlay = Some(Overlay::Hex(name)),
        None => editor.overlay = Some(Overlay::Rename(name)),
    }
}

/// Échap : ferme la fenêtre ouverte, sinon revient au jeu.
pub fn escape(keys: Res<ButtonInput<KeyCode>>, mut editor: ResMut<Editor>, mut next: ResMut<NextState<AppState>>) {
    if !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    if std::mem::take(&mut editor.escape_used) {
        return;
    }
    if editor.placing.take().is_some() {
        editor.say("Pose annulee.".into());
        return;
    }
    if editor.drag.take().is_some() || std::mem::take(&mut editor.pasting) {
        editor.say("Annule.".into());
        return;
    }
    if editor.overlay.is_none() && editor.selection.take().is_some() {
        editor.say("Selection retiree.".into());
        return;
    }
    if editor.preview.as_ref().is_some_and(|p| !p.on_race) {
        editor.stop_preview();
        editor.say("Apercu arrete.".into());
        return;
    }
    if editor.overlay.is_some() && !(editor.welcome && editor.docs.is_empty()) {
        editor.overlay = None;
        editor.ui_dirty = true;
    } else {
        next.set(AppState::Jeu);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn actions(interactions: Query<(&Interaction, &Act), Changed<Interaction>>, mut editor: ResMut<Editor>, mut next: ResMut<NextState<AppState>>) {
    for (i, act) in &interactions {
        if *i != Interaction::Pressed {
            continue;
        }
        let ed = &mut *editor;
        ed.ui_dirty = true;
        if !matches!(act, Act::Delete(_)) {
            ed.pending_delete = None;
        }
        match *act {
            Act::Tool(t) => ed.set_tool(t),
            Act::Brush(b) => ed.brush = b,
            Act::Clip(c) => match c {
                ClipAct::Copy => ed.copy_selection(false),
                ClipAct::Cut => ed.copy_selection(true),
                ClipAct::Paste => ed.start_paste(),
                ClipAct::Delete => ed.delete_selection(),
                ClipAct::Transform(t) => ed.transform(t),
                ClipAct::Deselect => {
                    ed.selection = None;
                    ed.pasting = false;
                }
            },
            Act::Layer(k) => {
                if let Some(d) = ed.doc_mut() {
                    d.layer = k;
                }
            }
            Act::LayerVisible(k) => {
                if let Some(d) = ed.doc_mut() {
                    d.toggle_layer(k);
                }
            }
            Act::AddLayer => {
                if let Some(d) = ed.doc_mut() {
                    d.add_layer();
                }
            }
            Act::Cut(axis) => {
                if let Some(d) = ed.doc_mut() {
                    let cut = axis.map(|a| super::mesh::Cut { axis: a, pos: d.cut.filter(|c| c.axis == a).map_or(d.model.size.as_ivec3()[a] / 2, |c| c.pos) });
                    d.set_cut(cut);
                }
            }
            Act::CutMove(by) => ed.move_cut(by),
            Act::Mirror => ed.mirror = !ed.mirror,
            Act::Grid => ed.grid = !ed.grid,
            Act::Undo => ed.undo(),
            Act::Redo => ed.redo(),
            Act::Focus => ed.focus(),
            Act::Pick(c) => ed.set_color(c),
            Act::ModelColor(k) => {
                if let Some(c) = ed.doc().and_then(|d| d.model.palette.get(k as usize - 1).copied()) {
                    ed.set_color(c);
                }
            }
            Act::Material(m) => {
                let c = PaletteEntry { rgb: ed.color.rgb, material: m };
                ed.set_color(c);
            }
            Act::Saturation(s) => ed.saturation = s,
            Act::Theme(t) => ed.theme = t,
            Act::Hex => ed.overlay = Some(Overlay::Hex(hex(&ed.color.rgb))),
            Act::Tab(k) => ed.select(k),
            Act::CloseTab => ed.close_tab(),
            Act::New => {
                ed.overlay = Some(Overlay::New);
            }
            Act::Library => {
                ed.library = list_models(&library_dir());
                ed.overlay = Some(Overlay::Library);
            }
            Act::Save => ed.save(),
            Act::Quit => next.set(AppState::Jeu),
            Act::Rename => {
                let name = ed.doc().map(|d| d.model.name.clone()).unwrap_or_default();
                ed.overlay = Some(Overlay::Rename(name));
            }
            Act::Tags => ed.overlay = Some(Overlay::Tags),
            Act::CloseOverlay => ed.overlay = None,
            Act::NewKind(k) => ed.new_kind = k,
            Act::NewRace(r) => ed.choose_race(r, None),
            Act::RaceOption(k) => {
                if let Some(r) = ed.new_race {
                    let mut o = ed.new_options.clone();
                    if let Some(x) = o.get_mut(k) {
                        *x = !*x;
                    }
                    ed.choose_race(r, Some(o));
                }
            }
            Act::NewCategory(c) => ed.new_category = c,
            Act::NewSize(s) => ed.new_size = s,
            Act::Create => ed.create(),
            Act::Open(k) => ed.open_library(k),
            Act::Duplicate(k) => ed.duplicate(k),
            Act::Delete(k) => ed.delete(k),
            Act::Tag(t) => {
                if let Some(d) = ed.doc_mut() {
                    if let Some(pos) = d.model.tags.iter().position(|x| x == t) {
                        d.model.tags.remove(pos);
                    } else {
                        d.model.tags.push(t.to_string());
                    }
                    d.dirty = true;
                }
            }
            Act::Import => ed.import(),
            Act::Block(k) => ed.pick_block(k),
            Act::RemoveZone(z) => {
                ed.stop_preview();
                let name = ed.doc().and_then(|d| d.model.zones.get(z)).map(|z| z.name.clone()).unwrap_or_default();
                if let Some(d) = ed.doc_mut() {
                    d.remove_zone(z);
                }
                ed.say(format!("Zone \"{name}\" retiree : ses blocs restent (Ctrl+Z pour annuler)."));
            }
            Act::Preview => {
                if ed.preview.is_some() {
                    ed.stop_preview();
                } else {
                    ed.start_preview("repos");
                }
            }
            Act::Anim(k) => {
                let anim = ed.shown().and_then(|d| super::motion::model_anims(&d.model, &ed.lib).get(k).cloned());
                if let Some(a) = anim {
                    ed.start_preview(&a);
                }
            }
            Act::RenameOk => match ed.overlay.clone() {
                Some(Overlay::Rename(name)) => ed.rename(name),
                Some(Overlay::Hex(code)) => ed.apply_hex(&code),
                _ => {}
            },
        }
    }
}

/// Masque l'interface du jeu (on la rend en sortant).
pub fn hide_game_ui(
    mut editor: ResMut<Editor>,
    mut roots: Query<(Entity, &mut Visibility), (With<Node>, Without<Parent>, Without<EditorUi>, Without<TargetCamera>)>,
) {
    for (e, mut v) in &mut roots {
        if *v != Visibility::Hidden {
            if !editor.hidden_ui.contains(&e) {
                editor.hidden_ui.push(e);
            }
            *v = Visibility::Hidden;
        }
    }
}

pub fn show_game_ui(mut editor: ResMut<Editor>, mut q: Query<&mut Visibility, (With<Node>, Without<EditorUi>)>, mut commands: Commands, ui: Query<Entity, With<EditorUi>>) {
    for e in std::mem::take(&mut editor.hidden_ui) {
        if let Ok(mut v) = q.get_mut(e) {
            *v = Visibility::Inherited;
        }
    }
    for e in &ui {
        commands.entity(e).try_despawn_recursive();
    }
}

/// Nom de modèle unique parmi les onglets.
pub fn fresh_name(base: &str, taken: &[String]) -> String {
    let mut k = 1;
    loop {
        let n = format!("{base} {k}");
        if !taken.contains(&n) {
            return n;
        }
        k += 1;
    }
}

impl Editor {
    /// Crée le modèle choisi dans « Nouveau modèle ».
    fn create(&mut self) {
        if self.new_kind == ModelKind::Personnage && self.new_race.is_none() {
            self.say("Choisis d'abord une race.".into());
            return;
        }
        let mut taken: Vec<String> = self.docs.iter().map(|d| d.model.name.clone()).collect();
        taken.extend(list_models(&library_dir()).into_iter().map(|e| e.name));
        let category = (self.new_kind == ModelKind::Vaisseau).then_some(self.new_category);
        let race = self.new_race.and_then(|r| self.lib.races.get(r)).cloned();
        let base = match (self.new_kind, &race) {
            (ModelKind::Personnage, Some(r)) => r.races.first().cloned().unwrap_or_else(|| r.name.clone()),
            (ModelKind::Vaisseau, _) => self.new_category.name().to_string(),
            _ => "Objet".to_string(),
        };
        let name = fresh_name(&base, &taken);
        let mut m = match (self.new_kind, &race) {
            // Le rig de la race, en blocs blancs, avec les membres choisis
            (ModelKind::Personnage, Some(r)) => r.build(&name, &self.new_options),
            _ => Model::new(&name, self.new_kind, category),
        };
        if self.new_kind == ModelKind::Autre {
            m.size = UVec3::splat(self.new_size);
        }
        let m_zones = m.zones.len();
        super::view::open_doc(self, m, None);
        self.overlay = None;
        self.welcome = false;
        self.race_view = None;
        self.say(if m_zones > 0 {
            "Nouveau personnage : peins ou remplace les blocs blancs, ajoute des blocs (ils suivent la zone touchee). P : apercu.".into()
        } else {
            "Nouveau modele : clic gauche pour poser des blocs. Ctrl+S pour enregistrer.".into()
        });
    }

    fn open_library(&mut self, k: usize) {
        let Some(e) = self.library.get(k).cloned() else { return };
        if let Some(i) = self.docs.iter().position(|d| d.path.as_ref() == Some(&e.path)) {
            self.select(i);
            self.overlay = None;
            return;
        }
        match std::fs::read(&e.path).map_err(|x| x.to_string()).and_then(|b| Model::from_bytes(&b)) {
            Ok(m) => {
                super::view::open_doc(self, m, Some(e.path.clone()));
                self.overlay = None;
                self.say(format!("Ouvert : {}", e.name));
            }
            Err(x) => self.say(format!("Impossible d'ouvrir {} : {x}", e.name)),
        }
    }

    fn duplicate(&mut self, k: usize) {
        let Some(e) = self.library.get(k).cloned() else { return };
        let r = std::fs::read(&e.path).map_err(|x| x.to_string()).and_then(|b| Model::from_bytes(&b)).and_then(|mut m| {
            m.name = format!("{} (copie)", m.name);
            save_model(&library_dir(), &m)
        });
        self.say(match r {
            Ok(p) => format!("Copie enregistree : {}", p.display()),
            Err(x) => x,
        });
        self.library = list_models(&library_dir());
    }

    fn delete(&mut self, k: usize) {
        if self.pending_delete != Some(k) {
            self.pending_delete = Some(k);
            return;
        }
        self.pending_delete = None;
        let Some(e) = self.library.get(k).cloned() else { return };
        let msg = match std::fs::remove_file(&e.path) {
            Ok(()) => {
                // L'onglet ouvert reste, sans fichier (on peut le réenregistrer)
                for d in &mut self.docs {
                    if d.path.as_ref() == Some(&e.path) {
                        d.path = None;
                        d.dirty = true;
                    }
                }
                format!("Supprime : {}", e.name)
            }
            Err(x) => format!("Suppression impossible : {x}"),
        };
        self.say(msg);
        self.library = list_models(&library_dir());
    }

    fn apply_hex(&mut self, code: &str) {
        self.overlay = None;
        self.ui_dirty = true;
        self.escape_used = true;
        match palette::parse_hex(code) {
            Some(rgb) => {
                let c = PaletteEntry { rgb, material: self.color.material };
                self.set_color(c);
            }
            None => self.say(format!("Code de couleur invalide : \"{code}\" (6 chiffres hexadecimaux, ex. FF8000).")),
        }
    }

    fn rename(&mut self, name: String) {
        let name = name.trim().to_string();
        self.overlay = None;
        self.ui_dirty = true;
        self.escape_used = true;
        if name.is_empty() {
            return;
        }
        if let Some(d) = self.doc_mut() {
            d.model.name = name;
            d.dirty = true;
        }
    }

    fn close_tab(&mut self) {
        if self.docs.is_empty() {
            return;
        }
        let d = self.docs.remove(self.current);
        if d.dirty {
            self.say(format!("\"{}\" ferme sans enregistrer les derniers changements.", d.model.name));
        }
        let i = self.current.min(self.docs.len().saturating_sub(1));
        if !self.docs.is_empty() {
            self.select(i);
        }
    }
}

/// Couleur de départ de l'éditeur.
pub fn default_color() -> super::format::PaletteEntry {
    edit::starter_palette()[0]
}

/// Clic droit sur une couleur du modèle : elle est remplacée partout par la couleur choisie.
pub fn replace_on_right_click(buttons: Res<ButtonInput<MouseButton>>, chips: Query<(&Interaction, &Act)>, mut editor: ResMut<Editor>) {
    if !buttons.just_pressed(MouseButton::Right) {
        return;
    }
    let Some(k) = chips.iter().find_map(|(i, a)| match (i, a) {
        (Interaction::Hovered | Interaction::Pressed, Act::ModelColor(k)) => Some(*k),
        _ => None,
    }) else {
        return;
    };
    let to = editor.color;
    let n = editor.doc_mut().map_or(0, |d| d.replace_color(k, to));
    editor.say(if n > 0 { format!("{n} voxels repeints d'un coup (Ctrl+Z pour annuler).") } else { "Rien a remplacer (meme couleur).".into() });
}
