// ─────────────────────────────────────────────────────────────────────────
//  Panneau Multijoueur (F2 ou bouton « Multijoueur »)
//
//  - pseudo (champ texte) et couleur d'aura (palette) ;
//  - « Héberger une partie » ;
//  - parties trouvées automatiquement sur le réseau local (un clic = rejoindre) ;
//  - rejoindre par adresse IP (pour jouer par Internet).
// ─────────────────────────────────────────────────────────────────────────

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::ButtonState;
use bevy::prelude::*;

use crate::net::{Net, NetCommand, NetMode, MAX_NAME_LEN, NET_PORT};
use crate::settings::GameSettings;

const BG_DARK: Color = Color::srgba(0.06, 0.06, 0.10, 0.97);
const BG_FIELD: Color = Color::srgba(0.12, 0.12, 0.18, 1.0);
const BG_BUTTON: Color = Color::srgba(0.14, 0.14, 0.22, 1.0);
const ACCENT: Color = Color::srgb(0.3, 0.6, 1.0);
const TEXT_COLOR: Color = Color::srgb(0.9, 0.9, 0.95);
const TEXT_DIM: Color = Color::srgb(0.55, 0.55, 0.62);
const ERROR_COLOR: Color = Color::srgb(1.0, 0.45, 0.4);
const OK_COLOR: Color = Color::srgb(0.45, 0.9, 0.55);
const RED_SOFT: Color = Color::srgb(0.85, 0.35, 0.35);

/// Couleurs d'aura proposées.
pub const AURA_PALETTE: [[f32; 3]; 12] = [
    [0.20, 0.90, 1.00], // cyan
    [0.25, 0.45, 1.00], // bleu
    [0.60, 0.35, 1.00], // violet
    [1.00, 0.35, 0.80], // rose
    [1.00, 0.25, 0.25], // rouge
    [1.00, 0.55, 0.15], // orange
    [1.00, 0.90, 0.20], // jaune
    [0.60, 1.00, 0.20], // citron vert
    [0.20, 0.90, 0.40], // vert
    [0.10, 0.75, 0.65], // turquoise
    [0.95, 0.95, 1.00], // blanc
    [0.85, 0.65, 0.35], // or
];

pub struct NetUiPlugin;

impl Plugin for NetUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NetPanel>()
            .add_systems(Startup, setup_net_panel)
            // Avant Update : le menu Options (Échap) doit voir que le panneau a consommé la touche
            .add_systems(PreUpdate, toggle_net_panel)
            .add_systems(
                Update,
                (
                    handle_panel_buttons,
                    handle_text_input,
                    update_panel_visibility,
                    update_fields,
                    update_swatches,
                    update_status,
                    rebuild_lan_list,
                    rebuild_players_list,
                )
                    .chain(),
            );
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Field {
    Name,
    Address,
}

#[derive(Resource, Default)]
pub struct NetPanel {
    pub open: bool,
    pub focus: Option<Field>,
    /// Échap a été utilisé par ce panneau pendant cette frame.
    pub esc_consumed: bool,
    address: String,
    address_loaded: bool,
}

#[derive(Component)]
struct NetPanelRoot;

#[derive(Component)]
struct NetOpenButton;

#[derive(Component)]
struct CloseButton;

#[derive(Component)]
struct FieldBox(Field);

#[derive(Component)]
struct FieldText(Field);

#[derive(Component)]
struct Swatch(usize);

#[derive(Component)]
struct HostButton;

#[derive(Component)]
struct JoinButton;

#[derive(Component)]
struct LeaveButton;

#[derive(Component)]
struct LanEntry(String);

#[derive(Component)]
struct StatusText;

#[derive(Component)]
struct LanList;

#[derive(Component)]
struct PlayersList;

/// Section visible uniquement hors ligne (héberger / rejoindre).
#[derive(Component)]
struct OfflineSection;

/// Section visible uniquement en partie (joueurs + quitter).
#[derive(Component)]
struct OnlineSection;

// ─────────────────────────────────────────────────────────────────────────
//  Construction
// ─────────────────────────────────────────────────────────────────────────

fn text(label: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (Text::new(label.into()), TextFont { font_size: size, ..default() }, TextColor(color))
}

fn button(commands: &mut Commands, label: &str, border: Color, marker: impl Component) -> Entity {
    commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(12.0), Val::Px(7.0)),
                border: UiRect::all(Val::Px(2.0)),
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(BG_BUTTON),
            BorderColor(border),
            BorderRadius::all(Val::Px(6.0)),
            Button,
            marker,
        ))
        .with_child(text(label, 14.0, TEXT_COLOR))
        .id()
}

fn field(commands: &mut Commands, kind: Field, width: Val) -> Entity {
    commands
        .spawn((
            Node {
                width,
                height: Val::Px(30.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                border: UiRect::all(Val::Px(2.0)),
                align_items: AlignItems::Center,
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(BG_FIELD),
            BorderColor(TEXT_DIM),
            BorderRadius::all(Val::Px(5.0)),
            Button,
            FieldBox(kind),
        ))
        .with_child((text("", 14.0, TEXT_COLOR), FieldText(kind)))
        .id()
}

fn section_title(commands: &mut Commands, label: &str) -> Entity {
    commands.spawn(text(label, 12.0, TEXT_DIM)).id()
}

fn setup_net_panel(mut commands: Commands) {
    // Bouton d'ouverture, à gauche du bouton « Options »
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(130.0),
                top: Val::Px(20.0),
                padding: UiRect::axes(Val::Px(16.0), Val::Px(8.0)),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(BG_DARK),
            BorderColor(ACCENT),
            BorderRadius::all(Val::Px(6.0)),
            Button,
            NetOpenButton,
        ))
        .with_child(text("Multijoueur", 18.0, TEXT_COLOR));

    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(20.0),
                top: Val::Px(70.0),
                width: Val::Px(380.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(14.0)),
                row_gap: Val::Px(10.0),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(BG_DARK),
            BorderColor(ACCENT),
            BorderRadius::all(Val::Px(10.0)),
            Visibility::Hidden,
            NetPanelRoot,
        ))
        .id();

    // Titre + fermer
    let header = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            ..default()
        })
        .id();
    let title = commands.spawn(text("MULTIJOUEUR", 20.0, TEXT_COLOR)).id();
    let close = button(&mut commands, "X", RED_SOFT, CloseButton);
    commands.entity(header).add_children(&[title, close]);

    // Profil
    let name_title = section_title(&mut commands, "VOTRE PSEUDO");
    let name_field = field(&mut commands, Field::Name, Val::Percent(100.0));

    let color_title = section_title(&mut commands, "COULEUR DE VOTRE AURA");
    let palette = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_wrap: FlexWrap::Wrap,
            column_gap: Val::Px(6.0),
            row_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    for (i, c) in AURA_PALETTE.iter().enumerate() {
        let swatch = commands
            .spawn((
                Node {
                    width: Val::Px(24.0),
                    height: Val::Px(24.0),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(c[0], c[1], c[2])),
                BorderColor(Color::NONE),
                BorderRadius::all(Val::Px(12.0)),
                Button,
                Swatch(i),
            ))
            .id();
        commands.entity(palette).add_child(swatch);
    }

    // Statut
    let status = commands.spawn((text("", 13.0, TEXT_DIM), StatusText)).id();

    // ── Hors ligne : héberger / rejoindre ─────────────────────────────
    let offline = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(10.0),
                ..default()
            },
            OfflineSection,
        ))
        .id();
    let host = button(&mut commands, "Heberger une partie", OK_COLOR, HostButton);
    let host_hint = commands
        .spawn(text(
            "Les joueurs sur le meme reseau (meme box / wifi) vous verront automatiquement ci-dessous.",
            11.0,
            TEXT_DIM,
        ))
        .id();
    let lan_title = section_title(&mut commands, "PARTIES SUR LE RESEAU LOCAL (cliquer pour rejoindre)");
    let lan_list = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                ..default()
            },
            LanList,
        ))
        .id();
    let ip_title = section_title(&mut commands, "REJOINDRE PAR ADRESSE IP (par Internet)");
    let ip_row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            column_gap: Val::Px(6.0),
            align_items: AlignItems::Center,
            ..default()
        })
        .id();
    let ip_field = field(&mut commands, Field::Address, Val::Px(240.0));
    let join = button(&mut commands, "Rejoindre", ACCENT, JoinButton);
    commands.entity(ip_row).add_children(&[ip_field, join]);
    let ip_hint = commands
        .spawn(text(
            format!(
                "Par Internet, l'hote doit rediriger le port UDP {NET_PORT} de sa box vers son PC, puis vous donner son IP publique."
            ),
            11.0,
            TEXT_DIM,
        ))
        .id();
    commands
        .entity(offline)
        .add_children(&[host, host_hint, lan_title, lan_list, ip_title, ip_row, ip_hint]);

    // ── En partie : joueurs + quitter ─────────────────────────────────
    let online = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                display: Display::None,
                ..default()
            },
            OnlineSection,
        ))
        .id();
    let players_title = section_title(&mut commands, "JOUEURS");
    let players = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                ..default()
            },
            PlayersList,
        ))
        .id();
    let leave = button(&mut commands, "Quitter la partie", RED_SOFT, LeaveButton);
    commands.entity(online).add_children(&[players_title, players, leave]);

    let help = commands
        .spawn(text("F2 : ouvrir / fermer ce panneau", 11.0, TEXT_DIM))
        .id();

    commands.entity(root).add_children(&[
        header, name_title, name_field, color_title, palette, status, offline, online, help,
    ]);
}

// ─────────────────────────────────────────────────────────────────────────
//  Interaction
// ─────────────────────────────────────────────────────────────────────────

fn toggle_net_panel(
    keys: Res<ButtonInput<KeyCode>>,
    open_btn: Query<&Interaction, (Changed<Interaction>, With<NetOpenButton>)>,
    close_btn: Query<&Interaction, (Changed<Interaction>, With<CloseButton>)>,
    mut panel: ResMut<NetPanel>,
    mut net: ResMut<Net>,
    mut settings: ResMut<GameSettings>,
) {
    let clicked_open = open_btn.iter().any(|i| *i == Interaction::Pressed);
    let clicked_close = close_btn.iter().any(|i| *i == Interaction::Pressed);
    let key = keys.just_pressed(KeyCode::F2);
    let escape = keys.just_pressed(KeyCode::Escape) && panel.open && panel.focus.is_none();
    panel.esc_consumed = escape;

    if clicked_open || key {
        panel.open = !panel.open;
    } else if clicked_close || escape {
        panel.open = false;
    } else {
        return;
    }
    if !panel.open {
        commit_focus(&mut panel, &mut settings);
    }
    net.discovering = panel.open;
}

/// Quitte le champ en cours d'édition (et sauvegarde le pseudo).
fn commit_focus(panel: &mut NetPanel, settings: &mut GameSettings) {
    if panel.focus == Some(Field::Name) {
        let trimmed = settings.player_name.trim().to_string();
        settings.player_name = if trimmed.is_empty() { crate::settings::default_player_name() } else { trimmed };
        settings.save();
    }
    panel.focus = None;
}

fn handle_panel_buttons(
    fields: Query<(&Interaction, &FieldBox), Changed<Interaction>>,
    swatches: Query<(&Interaction, &Swatch), Changed<Interaction>>,
    host: Query<&Interaction, (Changed<Interaction>, With<HostButton>)>,
    join: Query<&Interaction, (Changed<Interaction>, With<JoinButton>)>,
    leave: Query<&Interaction, (Changed<Interaction>, With<LeaveButton>)>,
    lan: Query<(&Interaction, &LanEntry), Changed<Interaction>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut panel: ResMut<NetPanel>,
    mut settings: ResMut<GameSettings>,
    mut commands_out: EventWriter<NetCommand>,
) {
    if !panel.open {
        return;
    }
    let pressed = |i: &Interaction| *i == Interaction::Pressed;

    let mut clicked_field = None;
    for (i, f) in &fields {
        if pressed(i) {
            clicked_field = Some(f.0);
        }
    }
    if let Some(f) = clicked_field {
        if panel.focus != Some(f) {
            commit_focus(&mut panel, &mut settings);
            panel.focus = Some(f);
        }
    } else if mouse.just_pressed(MouseButton::Left) && panel.focus.is_some() {
        // Clic ailleurs : on quitte le champ
        commit_focus(&mut panel, &mut settings);
    }

    for (i, s) in &swatches {
        if pressed(i) {
            settings.aura_color = AURA_PALETTE[s.0];
            settings.save();
        }
    }
    if host.iter().any(pressed) {
        commands_out.send(NetCommand::Host);
    }
    if join.iter().any(pressed) {
        commit_focus(&mut panel, &mut settings);
        commands_out.send(NetCommand::Join(panel.address.clone()));
    }
    if leave.iter().any(pressed) {
        commands_out.send(NetCommand::Leave);
    }
    for (i, entry) in &lan {
        if pressed(i) {
            commands_out.send(NetCommand::Join(entry.0.clone()));
        }
    }
}

fn handle_text_input(
    mut events: EventReader<KeyboardInput>,
    mut panel: ResMut<NetPanel>,
    mut settings: ResMut<GameSettings>,
    mut commands_out: EventWriter<NetCommand>,
) {
    if !panel.address_loaded {
        panel.address = settings.last_join_address.clone();
        panel.address_loaded = true;
    }
    let Some(focus) = panel.focus else {
        events.clear();
        return;
    };
    for ev in events.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        match &ev.logical_key {
            Key::Enter => {
                commit_focus(&mut panel, &mut settings);
                if focus == Field::Address {
                    commands_out.send(NetCommand::Join(panel.address.clone()));
                }
                return;
            }
            Key::Escape => {
                commit_focus(&mut panel, &mut settings);
                return;
            }
            Key::Backspace => {
                match focus {
                    Field::Name => { settings.player_name.pop(); }
                    Field::Address => { panel.address.pop(); }
                }
            }
            Key::Space => {
                if focus == Field::Name && settings.player_name.chars().count() < MAX_NAME_LEN {
                    settings.player_name.push(' ');
                }
            }
            Key::Character(s) => {
                for c in s.chars() {
                    match focus {
                        Field::Name => {
                            if !c.is_control() && settings.player_name.chars().count() < MAX_NAME_LEN {
                                settings.player_name.push(c);
                            }
                        }
                        Field::Address => {
                            let allowed = c.is_ascii_alphanumeric() || ".:-_[]".contains(c);
                            if allowed && panel.address.len() < 64 {
                                panel.address.push(c);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  Rafraîchissement de l'affichage
// ─────────────────────────────────────────────────────────────────────────

fn update_panel_visibility(
    panel: Res<NetPanel>,
    net: Res<Net>,
    mut root: Query<&mut Visibility, With<NetPanelRoot>>,
    mut offline: Query<&mut Node, (With<OfflineSection>, Without<OnlineSection>)>,
    mut online: Query<&mut Node, (With<OnlineSection>, Without<OfflineSection>)>,
) {
    for mut vis in &mut root {
        let want = if panel.open { Visibility::Visible } else { Visibility::Hidden };
        if *vis != want {
            *vis = want;
        }
    }
    let in_game = matches!(net.mode(), NetMode::Hosting | NetMode::Connected);
    for mut node in &mut offline {
        let want = if in_game { Display::None } else { Display::Flex };
        if node.display != want {
            node.display = want;
        }
    }
    for mut node in &mut online {
        let want = if in_game { Display::Flex } else { Display::None };
        if node.display != want {
            node.display = want;
        }
    }
}

fn update_fields(
    time: Res<Time>,
    panel: Res<NetPanel>,
    settings: Res<GameSettings>,
    mut boxes: Query<(&FieldBox, &mut BorderColor)>,
    mut texts: Query<(&FieldText, &mut Text, &mut TextColor)>,
) {
    if !panel.open {
        return;
    }
    let caret = (time.elapsed_secs() * 2.0) as u32 % 2 == 0;
    for (fb, mut border) in &mut boxes {
        let c = if panel.focus == Some(fb.0) { ACCENT } else { TEXT_DIM };
        if border.0 != c {
            border.0 = c;
        }
    }
    for (ft, mut t, mut color) in &mut texts {
        let (value, placeholder) = match ft.0 {
            Field::Name => (settings.player_name.as_str(), "Votre pseudo"),
            Field::Address => (panel.address.as_str(), "ex. 192.168.1.20"),
        };
        let focused = panel.focus == Some(ft.0);
        let shown = if value.is_empty() && !focused {
            placeholder.to_string()
        } else if focused && caret {
            format!("{value}|")
        } else {
            value.to_string()
        };
        if t.0 != shown {
            t.0 = shown;
        }
        let c = if value.is_empty() && !focused { TEXT_DIM } else { TEXT_COLOR };
        if color.0 != c {
            color.0 = c;
        }
    }
}

fn update_swatches(settings: Res<GameSettings>, mut swatches: Query<(&Swatch, &mut BorderColor)>) {
    for (s, mut border) in &mut swatches {
        let selected = AURA_PALETTE[s.0] == settings.aura_color;
        let c = if selected { Color::WHITE } else { Color::NONE };
        if border.0 != c {
            border.0 = c;
        }
    }
}

fn update_status(net: Res<Net>, mut q: Query<(&mut Text, &mut TextColor), With<StatusText>>) {
    let color = if net.status_is_error {
        ERROR_COLOR
    } else if matches!(net.mode(), NetMode::Hosting | NetMode::Connected) {
        OK_COLOR
    } else {
        TEXT_DIM
    };
    let mut status = net.status.clone();
    if net.mode() == NetMode::Hosting {
        status.push_str(&format!("\nLes autres joueurs peuvent vous rejoindre (port UDP {NET_PORT})."));
    }
    for (mut t, mut c) in &mut q {
        if t.0 != status {
            t.0 = status.clone();
        }
        if c.0 != color {
            c.0 = color;
        }
    }
}

fn rebuild_lan_list(
    mut commands: Commands,
    net: Res<Net>,
    list: Query<Entity, With<LanList>>,
    mut last: Local<Option<String>>,
) {
    let signature: String = net
        .lan_games
        .iter()
        .map(|g| format!("{}|{}|{}|{};", g.addr, g.host, g.players, g.compatible))
        .collect();
    if last.as_deref() == Some(signature.as_str()) {
        return;
    }
    *last = Some(signature);
    let Ok(list) = list.get_single() else { return };
    commands.entity(list).despawn_descendants();

    if net.lan_games.is_empty() {
        let empty = commands
            .spawn(text("Recherche en cours... aucune partie trouvee pour l'instant.", 12.0, TEXT_DIM))
            .id();
        commands.entity(list).add_child(empty);
        return;
    }
    for g in &net.lan_games {
        let plural = if g.players > 1 { "s" } else { "" };
        let label = if g.compatible {
            format!("{}  -  {} joueur{plural}  -  {}", g.host, g.players, g.addr.ip())
        } else {
            format!("{}  -  version differente", g.host)
        };
        let entry = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(BG_BUTTON),
                BorderColor(if g.compatible { OK_COLOR } else { TEXT_DIM }),
                BorderRadius::all(Val::Px(5.0)),
                Button,
                LanEntry(g.addr.to_string()),
            ))
            .with_child(text(label, 13.0, if g.compatible { TEXT_COLOR } else { TEXT_DIM }))
            .id();
        commands.entity(list).add_child(entry);
    }
}

fn rebuild_players_list(
    mut commands: Commands,
    net: Res<Net>,
    settings: Res<GameSettings>,
    list: Query<Entity, With<PlayersList>>,
    mut last: Local<Option<String>>,
) {
    let mut players: Vec<(String, [f32; 3], bool)> =
        vec![(settings.player_name.clone(), settings.aura_color, true)];
    let mut others: Vec<_> = net.peers.values().map(|p| (p.name.clone(), p.color, false)).collect();
    others.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    players.extend(others);

    let signature: String = players
        .iter()
        .map(|(n, c, _)| format!("{n}|{:?};", c))
        .collect();
    if last.as_deref() == Some(signature.as_str()) {
        return;
    }
    *last = Some(signature);
    let Ok(list) = list.get_single() else { return };
    commands.entity(list).despawn_descendants();

    for (name, c, me) in players {
        let row = commands
            .spawn(Node {
                column_gap: Val::Px(8.0),
                align_items: AlignItems::Center,
                ..default()
            })
            .id();
        let dot = commands
            .spawn((
                Node { width: Val::Px(12.0), height: Val::Px(12.0), ..default() },
                BackgroundColor(Color::srgb(c[0], c[1], c[2])),
                BorderRadius::all(Val::Px(6.0)),
            ))
            .id();
        let label = if me { format!("{name} (vous)") } else { name };
        let t = commands.spawn(text(label, 13.0, TEXT_COLOR)).id();
        commands.entity(row).add_children(&[dot, t]);
        commands.entity(list).add_child(row);
    }
}
