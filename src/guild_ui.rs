// ─────────────────────────────────────────────────────────────────────────
//  Panneau Guilde (touche G ou bouton « Guilde » du panneau Multijoueur)
//
//  Sans guilde : créer la sienne (nom, tag, emblème, couleur) ou demander à
//  rejoindre une guilde existante.
//  Dans une guilde : membres et rôles, demandes d'adhésion, relations avec
//  les autres guildes, départ et dissolution. Seuls les boutons auxquels
//  son rang donne droit sont affichés (voir `guild.rs`).
// ─────────────────────────────────────────────────────────────────────────

use bevy::prelude::*;

use crate::diplomacy::{combine, faction_key, Relation};
use crate::guild::{self, has_rank, my_role, GuildRecord, Guilds, Role, EMBLEM_COUNT, MAX_MEMBERS};
use crate::net::Net;
use crate::net_ui::{
    button, change_relation, field, section_title, text, Field, NetPanel, ACCENT, AURA_PALETTE, BG_BUTTON, BG_DARK,
    CODE_COLOR, OK_COLOR, RED_SOFT, TEXT_COLOR, TEXT_DIM,
};
use crate::settings::GameSettings;

/// Délai pour confirmer la dissolution (secondes).
const CONFIRM_SECS: f64 = 6.0;

pub struct GuildUiPlugin;

impl Plugin for GuildUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GuildDraft>()
            .add_systems(Startup, setup_guild_panel)
            .add_systems(Update, (handle_guild_actions, rebuild_guild_panel, announce_requests).chain());
    }
}

/// Choix en cours dans le panneau (le nom et le tag sont dans `NetPanel`).
#[derive(Resource, Default)]
struct GuildDraft {
    emblem: u8,
    color: usize,
    /// Dissolution demandée : à confirmer avant cette heure.
    confirm_dissolve: Option<f64>,
}

#[derive(Component)]
struct GuildPanelRoot;

#[derive(Component)]
struct GuildPanelBody;

#[derive(Component, Clone)]
enum GuildAction {
    PickEmblem(u8),
    PickColor(usize),
    Create,
    Request(u64),
    CancelRequest,
    /// Accepter une demande ou ajouter directement (identifiant, pseudo).
    Add(u64, String),
    Refuse(u64, String),
    SetRole(u64, String, Role),
    Kick(u64, String),
    Transfer(u64, String),
    AskPromotion(bool),
    Leave,
    Dissolve,
    ConfirmDissolve,
    CancelDissolve,
    /// (faction, nom affiché, nouvelle position de ma guilde)
    Relation(String, String, Relation),
}

fn setup_guild_panel(mut commands: Commands) {
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(395.0),
                top: Val::Px(70.0),
                width: Val::Px(440.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(14.0)),
                row_gap: Val::Px(6.0),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(BG_DARK),
            BorderColor(ACCENT),
            BorderRadius::all(Val::Px(10.0)),
            Visibility::Hidden,
            GuildPanelRoot,
        ))
        .id();
    let header = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            ..default()
        })
        .id();
    let title = commands.spawn(text("GUILDE", 20.0, TEXT_COLOR)).id();
    let hint = commands.spawn(text("G ou Echap : fermer", 11.0, TEXT_DIM)).id();
    commands.entity(header).add_children(&[title, hint]);
    let body = commands
        .spawn((
            Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(5.0), ..default() },
            GuildPanelBody,
        ))
        .id();
    commands.entity(root).add_children(&[header, body]);
}

// ─────────────────────────────────────────────────────────────────────────
//  Briques d'affichage
// ─────────────────────────────────────────────────────────────────────────

/// Emblème de guilde : une forme simple dessinée à la couleur de la guilde.
pub fn emblem(commands: &mut Commands, kind: u8, color: Color, size: f32) -> Entity {
    let round = kind % 2 == 0 && kind < 6;
    let radius = if round { BorderRadius::all(Val::Percent(50.0)) } else { BorderRadius::all(Val::Px(2.0)) };
    let hollow = kind >= 2;
    let thickness = (size * 0.2).max(2.0);
    let mut node = Node {
        width: Val::Px(size),
        height: Val::Px(size),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..default()
    };
    let outer = if kind == 7 {
        commands.spawn(node).id()
    } else if hollow {
        node.border = UiRect::all(Val::Px(thickness));
        commands.spawn((node, BorderColor(color), radius)).id()
    } else {
        commands.spawn((node, BackgroundColor(color), radius)).id()
    };
    let bar = |w: f32, h: f32| Node {
        position_type: PositionType::Absolute,
        width: Val::Px(w),
        height: Val::Px(h),
        ..default()
    };
    let inner = size - 2.0 * thickness;
    match kind {
        // Anneau ou cadre avec un point au centre
        4 | 5 => {
            let dot = commands
                .spawn((Node { width: Val::Px(inner * 0.5), height: Val::Px(inner * 0.5), ..default() }, BackgroundColor(color), radius))
                .id();
            commands.entity(outer).add_child(dot);
        }
        // Cadre barré
        6 => {
            let line = commands.spawn((bar(inner, thickness), BackgroundColor(color))).id();
            commands.entity(outer).add_child(line);
        }
        // Croix
        7 => {
            let h = commands.spawn((bar(size, thickness * 1.3), BackgroundColor(color))).id();
            let v = commands.spawn((bar(thickness * 1.3, size), BackgroundColor(color))).id();
            commands.entity(outer).add_children(&[h, v]);
        }
        _ => {}
    }
    outer
}

fn rgb(c: [f32; 3]) -> Color {
    Color::srgb(c[0], c[1], c[2])
}

fn row(commands: &mut Commands) -> Entity {
    commands
        .spawn(Node {
            width: Val::Percent(100.0),
            column_gap: Val::Px(6.0),
            row_gap: Val::Px(3.0),
            align_items: AlignItems::Center,
            flex_wrap: FlexWrap::Wrap,
            ..default()
        })
        .id()
}

fn small_button(commands: &mut Commands, label: &str, color: Color, action: GuildAction) -> Entity {
    commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(7.0), Val::Px(2.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(BG_BUTTON),
            BorderColor(color),
            BorderRadius::all(Val::Px(5.0)),
            Button,
            action,
        ))
        .with_child(text(label, 11.0, color))
        .id()
}

// ─────────────────────────────────────────────────────────────────────────
//  Construction du contenu
// ─────────────────────────────────────────────────────────────────────────

fn rebuild_guild_panel(
    mut commands: Commands,
    time: Res<Time>,
    panel: Res<NetPanel>,
    settings: Res<GameSettings>,
    guilds: Res<Guilds>,
    net: Res<Net>,
    mut draft: ResMut<GuildDraft>,
    mut root: Query<&mut Visibility, With<GuildPanelRoot>>,
    body: Query<Entity, With<GuildPanelBody>>,
    mut last: Local<Option<String>>,
) {
    for mut vis in &mut root {
        let want = if panel.guild_open { Visibility::Visible } else { Visibility::Hidden };
        if *vis != want {
            *vis = want;
        }
    }
    if !panel.guild_open {
        draft.confirm_dissolve = None;
        return;
    }
    let now = time.elapsed_secs_f64();
    if draft.confirm_dissolve.is_some_and(|until| now > until) {
        draft.confirm_dissolve = None;
    }

    // Reconstruit seulement quand quelque chose d'affiché a changé
    let mut known: Vec<(u64, u64)> = guilds.known.values().map(|g| (g.id, g.rev)).collect();
    known.sort();
    let mut peers: Vec<_> = net.peers.values().map(|p| (p.status.pid, p.name.clone(), p.gid, p.status.req)).collect();
    peers.sort();
    let signature = format!(
        "{:?}|{}|{:?}|{known:?}|{peers:?}|{}|{}|{}",
        settings.guild, settings.player_id, guilds.request, draft.emblem, draft.color, draft.confirm_dissolve.is_some(),
    );
    if last.as_deref() == Some(signature.as_str()) {
        return;
    }
    *last = Some(signature);
    let Ok(body) = body.get_single() else { return };
    commands.entity(body).despawn_descendants();

    let children = match &settings.guild {
        Some(g) => build_member_view(&mut commands, g, &settings, &guilds, &net, &draft),
        None => build_outsider_view(&mut commands, &guilds, &net, &draft),
    };
    commands.entity(body).add_children(&children);
}

/// Joueur sans guilde : création, et liste des guildes à rejoindre.
fn build_outsider_view(commands: &mut Commands, guilds: &Guilds, net: &Net, draft: &GuildDraft) -> Vec<Entity> {
    let mut out = Vec::new();
    let color = rgb(AURA_PALETTE[draft.color % AURA_PALETTE.len()]);

    out.push(section_title(commands, "CREER UNE GUILDE (vous en serez le Chef)"));
    out.push(field(commands, Field::GuildName, Val::Percent(100.0)));
    let tag_row = row(commands);
    let tag_field = field(commands, Field::GuildTag, Val::Px(90.0));
    let tag_hint = commands.spawn(text("Tag : 1 a 5 lettres ou chiffres,\naffiche [TAG] devant les pseudos.", 11.0, TEXT_DIM)).id();
    commands.entity(tag_row).add_children(&[tag_field, tag_hint]);
    out.push(tag_row);

    out.push(section_title(commands, "EMBLEME"));
    let emblems = row(commands);
    for kind in 0..EMBLEM_COUNT {
        let selected = kind == draft.emblem % EMBLEM_COUNT;
        let cell = commands
            .spawn((
                Node {
                    padding: UiRect::all(Val::Px(5.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(BG_BUTTON),
                BorderColor(if selected { Color::WHITE } else { Color::NONE }),
                BorderRadius::all(Val::Px(6.0)),
                Button,
                GuildAction::PickEmblem(kind),
            ))
            .id();
        let icon = emblem(commands, kind, color, 24.0);
        commands.entity(cell).add_child(icon);
        commands.entity(emblems).add_child(cell);
    }
    out.push(emblems);

    out.push(section_title(commands, "COULEUR"));
    let palette = row(commands);
    for (i, c) in AURA_PALETTE.iter().enumerate() {
        let selected = i == draft.color % AURA_PALETTE.len();
        let swatch = commands
            .spawn((
                Node { width: Val::Px(24.0), height: Val::Px(24.0), border: UiRect::all(Val::Px(2.0)), ..default() },
                BackgroundColor(rgb(*c)),
                BorderColor(if selected { Color::WHITE } else { Color::NONE }),
                BorderRadius::all(Val::Px(12.0)),
                Button,
                GuildAction::PickColor(i),
            ))
            .id();
        commands.entity(palette).add_child(swatch);
    }
    out.push(palette);
    out.push(button(commands, "Creer la guilde", OK_COLOR, GuildAction::Create));

    out.push(section_title(commands, "REJOINDRE UNE GUILDE"));
    let mut list: Vec<&GuildRecord> = guilds.known.values().filter(|g| !g.dissolved).collect();
    list.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    if list.is_empty() {
        out.push(commands.spawn(text("Aucune guilde connue parmi les joueurs connectes.", 12.0, TEXT_DIM)).id());
    }
    for g in list {
        let line = row(commands);
        let icon = emblem(commands, g.emblem, rgb(g.color), 18.0);
        let online = net.peers.values().filter(|p| p.gid == g.id).count();
        let label = commands
            .spawn(text(
                format!("[{}] {}  ({}/{MAX_MEMBERS}, {online} en ligne)", g.tag, g.name, g.members.len()),
                13.0,
                rgb(g.color),
            ))
            .id();
        let action = if guilds.request.is_some_and(|r| r.0 == g.id) {
            small_button(commands, "Annuler ma demande", RED_SOFT, GuildAction::CancelRequest)
        } else {
            small_button(commands, "Demander a rejoindre", ACCENT, GuildAction::Request(g.id))
        };
        commands.entity(line).add_children(&[icon, label, action]);
        out.push(line);
    }
    out.push(
        commands
            .spawn(text(
                "Une demande doit etre acceptee par un Officier, un Sous-chef\nou le Chef de la guilde, connecte en meme temps que vous.",
                11.0,
                TEXT_DIM,
            ))
            .id(),
    );
    out
}

/// Membre d'une guilde : gestion selon son rang.
fn build_member_view(
    commands: &mut Commands,
    g: &GuildRecord,
    settings: &GameSettings,
    guilds: &Guilds,
    net: &Net,
    draft: &GuildDraft,
) -> Vec<Entity> {
    let mut out = Vec::new();
    let me = settings.player_id;
    let rank = g.role_of(me).unwrap_or(Role::Member);
    let color = rgb(g.color);

    // En-tête : emblème, nom, mon rôle
    let head = row(commands);
    let icon = emblem(commands, g.emblem, color, 30.0);
    let name = commands.spawn(text(format!("[{}] {}", g.tag, g.name), 19.0, color)).id();
    commands.entity(head).add_children(&[icon, name]);
    out.push(head);
    out.push(
        commands
            .spawn(text(format!("Votre role : {}   -   {}/{MAX_MEMBERS} membres", rank.label(), g.members.len()), 12.0, TEXT_DIM))
            .id(),
    );

    // Membres, du plus haut rang au plus bas
    out.push(section_title(commands, "MEMBRES"));
    let mut members: Vec<_> = g.members.iter().collect();
    members.sort_by(|a, b| b.role.cmp(&a.role).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    for m in members {
        let line = row(commands);
        let online = m.id == me || net.peers.values().any(|p| p.status.pid == m.id);
        let who = if m.id == me { format!("{} (vous)", m.name) } else { m.name.clone() };
        let label = commands.spawn(text(who, 13.0, if online { TEXT_COLOR } else { TEXT_DIM })).id();
        let role = commands.spawn(text(m.role.label(), 12.0, CODE_COLOR)).id();
        commands.entity(line).add_children(&[label, role]);
        if !online {
            let off = commands.spawn(text("hors ligne", 11.0, TEXT_DIM)).id();
            commands.entity(line).add_child(off);
        }
        if m.wants_promotion {
            let ask = commands.spawn(text("demande une promotion", 11.0, OK_COLOR)).id();
            commands.entity(line).add_child(ask);
        }
        // Boutons : seulement sur un rang strictement inférieur au mien
        if m.id != me && rank > m.role {
            if rank >= Role::Deputy {
                if let Some(up) = m.role.up().filter(|up| *up < rank) {
                    let label = format!("Promouvoir {}", up.label());
                    let b = small_button(commands, &label, OK_COLOR, GuildAction::SetRole(m.id, m.name.clone(), up));
                    commands.entity(line).add_child(b);
                }
                if let Some(down) = m.role.down() {
                    let label = format!("Retrograder {}", down.label());
                    let b = small_button(commands, &label, CODE_COLOR, GuildAction::SetRole(m.id, m.name.clone(), down));
                    commands.entity(line).add_child(b);
                }
            }
            if rank >= Role::Officer {
                let b = small_button(commands, "Exclure", RED_SOFT, GuildAction::Kick(m.id, m.name.clone()));
                commands.entity(line).add_child(b);
            }
            if rank == Role::Chief {
                let b = small_button(commands, "Nommer Chef", ACCENT, GuildAction::Transfer(m.id, m.name.clone()));
                commands.entity(line).add_child(b);
            }
        }
        out.push(line);
    }

    // Demandes et ajouts : Officier et plus
    if rank >= Role::Officer {
        let mut outsiders: Vec<_> = net.peers.values().filter(|p| p.gid == 0 && p.status.pid != 0).collect();
        outsiders.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        let requests: Vec<_> = outsiders
            .iter()
            .filter(|p| p.status.req == g.id && !g.refused.contains(&p.status.pid))
            .collect();
        if !requests.is_empty() {
            out.push(section_title(commands, "DEMANDES POUR REJOINDRE"));
        }
        for p in &requests {
            let line = row(commands);
            let label = commands.spawn(text(p.name.clone(), 13.0, TEXT_COLOR)).id();
            let yes = small_button(commands, "Accepter", OK_COLOR, GuildAction::Add(p.status.pid, p.name.clone()));
            let no = small_button(commands, "Refuser", RED_SOFT, GuildAction::Refuse(p.status.pid, p.name.clone()));
            commands.entity(line).add_children(&[label, yes, no]);
            out.push(line);
        }
        let others: Vec<_> = outsiders.iter().filter(|p| p.status.req != g.id).collect();
        if !others.is_empty() {
            out.push(section_title(commands, "JOUEURS SANS GUILDE CONNECTES"));
        }
        for p in &others {
            let line = row(commands);
            let label = commands.spawn(text(p.name.clone(), 13.0, TEXT_COLOR)).id();
            let add = small_button(commands, "Ajouter a la guilde", ACCENT, GuildAction::Add(p.status.pid, p.name.clone()));
            commands.entity(line).add_children(&[label, add]);
            out.push(line);
        }
    }

    // Relations avec les autres guildes (connectées, ou déjà déclarées)
    out.push(section_title(commands, "RELATIONS AVEC LES AUTRES GUILDES"));
    let my_key = faction_key(g.id, "");
    let can_set = rank >= Role::Deputy;
    // (faction, nom, couleur du nom, emblème, ce qu'elle a déclaré envers nous si on le sait)
    let mut factions: Vec<(String, String, Color, Option<u8>, Option<Relation>)> = guilds
        .known
        .values()
        .filter(|o| !o.dissolved && o.id != g.id)
        .map(|o| {
            (faction_key(o.id, ""), format!("[{}] {}", o.tag, o.name), rgb(o.color), Some(o.emblem), Some(o.declared(&my_key)))
        })
        .collect();
    for r in &g.relations {
        if r.key.starts_with("g:") && !factions.iter().any(|f| f.0 == r.key) {
            factions.push((r.key.clone(), format!("{} (hors ligne)", r.label), TEXT_DIM, None, None));
        }
    }
    factions.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));
    if factions.is_empty() {
        out.push(commands.spawn(text("Aucune autre guilde connue pour l'instant.", 12.0, TEXT_DIM)).id());
    }
    for (key, label, name_color, icon, theirs) in factions {
        let mine = g.declared(&key);
        let effective = combine(false, mine, theirs.unwrap_or(Relation::Neutral));
        let line = row(commands);
        if let Some(kind) = icon {
            let icon = emblem(commands, kind, name_color, 16.0);
            commands.entity(line).add_child(icon);
        }
        let name = commands.spawn(text(label.clone(), 13.0, name_color)).id();
        let status = commands.spawn(text(effective.label().to_uppercase(), 13.0, effective.color())).id();
        commands.entity(line).add_children(&[name, status]);
        let note = match (mine, theirs) {
            (Relation::Ally, Some(Relation::Neutral)) | (Relation::Ally, None) => Some("alliance proposee, en attente de leur accord"),
            (Relation::Neutral, Some(Relation::Ally)) => Some("ils proposent une alliance"),
            (Relation::Neutral | Relation::Ally, Some(Relation::Enemy)) => Some("ils vous ont declare la guerre"),
            _ => None,
        };
        if let Some(note) = note {
            let n = commands.spawn(text(note, 11.0, TEXT_DIM)).id();
            commands.entity(line).add_child(n);
        }
        if can_set {
            for choice in [Relation::Ally, Relation::Neutral, Relation::Enemy] {
                if choice != mine {
                    let b = small_button(
                        commands,
                        choice.label(),
                        choice.color(),
                        GuildAction::Relation(key.clone(), label.clone(), choice),
                    );
                    commands.entity(line).add_child(b);
                }
            }
        }
        out.push(line);
    }
    out.push(
        commands
            .spawn(text(
                if can_set {
                    "Allie : attaques impossibles entre les deux guildes (si les deux sont d'accord).\nNeutre et Ennemi : attaques possibles."
                } else {
                    "Les relations sont decidees par le Chef et les Sous-chefs.\nAllie : attaques impossibles. Neutre et Ennemi : attaques possibles."
                },
                11.0,
                TEXT_DIM,
            ))
            .id(),
    );

    // Actions personnelles
    let actions = row(commands);
    commands.entity(actions).insert(Node {
        width: Val::Percent(100.0),
        column_gap: Val::Px(6.0),
        row_gap: Val::Px(4.0),
        margin: UiRect::top(Val::Px(8.0)),
        flex_wrap: FlexWrap::Wrap,
        ..default()
    });
    if rank == Role::Member {
        let wants = g.member(me).is_some_and(|m| m.wants_promotion);
        let (label, c) = if wants { ("Annuler ma demande de promotion", CODE_COLOR) } else { ("Demander une promotion", OK_COLOR) };
        let b = small_button(commands, label, c, GuildAction::AskPromotion(!wants));
        commands.entity(actions).add_child(b);
    }
    let leave_label = if rank == Role::Chief && g.members.len() > 1 {
        "Quitter la guilde (un autre membre deviendra Chef)"
    } else {
        "Quitter la guilde"
    };
    let leave = small_button(commands, leave_label, RED_SOFT, GuildAction::Leave);
    commands.entity(actions).add_child(leave);
    if rank == Role::Chief {
        if draft.confirm_dissolve.is_some() {
            let yes = small_button(commands, "CONFIRMER : dissoudre definitivement", RED_SOFT, GuildAction::ConfirmDissolve);
            let no = small_button(commands, "Annuler", TEXT_COLOR, GuildAction::CancelDissolve);
            commands.entity(actions).add_children(&[yes, no]);
        } else {
            let b = small_button(commands, "Dissoudre la guilde", RED_SOFT, GuildAction::Dissolve);
            commands.entity(actions).add_child(b);
        }
    }
    out.push(actions);
    out
}

// ─────────────────────────────────────────────────────────────────────────
//  Actions
// ─────────────────────────────────────────────────────────────────────────

fn handle_guild_actions(
    time: Res<Time>,
    buttons: Query<(&Interaction, &GuildAction), Changed<Interaction>>,
    mut panel: ResMut<NetPanel>,
    mut draft: ResMut<GuildDraft>,
    mut settings: ResMut<GameSettings>,
    mut guilds: ResMut<Guilds>,
    mut net: ResMut<Net>,
) {
    let now = time.elapsed_secs_f64();
    let pressed: Vec<GuildAction> =
        buttons.iter().filter(|(i, _)| **i == Interaction::Pressed).map(|(_, a)| a.clone()).collect();
    for action in pressed {
        let (settings, guilds, net) = (&mut *settings, &mut *guilds, &mut *net);
        match action {
            GuildAction::PickEmblem(kind) => draft.emblem = kind,
            GuildAction::PickColor(i) => draft.color = i,
            GuildAction::Create => {
                let color = AURA_PALETTE[draft.color % AURA_PALETTE.len()];
                if guild::create(settings, guilds, net, now, &panel.guild_name, &panel.guild_tag, draft.emblem, color) {
                    panel.guild_name.clear();
                    panel.guild_tag.clear();
                    panel.focus = None;
                }
            }
            GuildAction::Request(gid) => {
                if let Some(g) = guilds.active(gid) {
                    let text = format!("Demande envoyee a la guilde [{}] {}.", g.tag, g.name);
                    guilds.request = Some((gid, g.rev));
                    net.notify(&text, now);
                }
            }
            GuildAction::CancelRequest => guilds.request = None,
            GuildAction::Add(pid, name) => {
                guild::edit(settings, guilds, net, now, |g, me| {
                    g.add_member(me, pid, &name)?;
                    Ok(Some(format!("{name} rejoint la guilde.")))
                });
            }
            GuildAction::Refuse(pid, name) => {
                guild::edit(settings, guilds, net, now, |g, me| {
                    g.refuse(me, pid)?;
                    Ok(Some(format!("Demande de {name} refusee.")))
                });
            }
            GuildAction::SetRole(pid, name, role) => {
                guild::edit(settings, guilds, net, now, |g, me| {
                    g.set_role(me, pid, role)?;
                    Ok(Some(format!("{name} est maintenant {}.", role.label())))
                });
            }
            GuildAction::Kick(pid, name) => {
                guild::edit(settings, guilds, net, now, |g, me| {
                    g.kick(me, pid)?;
                    Ok(Some(format!("{name} est exclu de la guilde.")))
                });
            }
            GuildAction::Transfer(pid, name) => {
                guild::edit(settings, guilds, net, now, |g, me| {
                    g.transfer(me, pid)?;
                    Ok(Some(format!("{name} est le nouveau Chef. Vous devenez Sous-chef.")))
                });
            }
            GuildAction::AskPromotion(wants) => {
                guild::edit(settings, guilds, net, now, |g, me| {
                    g.ask_promotion(me, wants)?;
                    Ok(wants.then(|| "Demande de promotion envoyee.".to_string()))
                });
            }
            GuildAction::Leave => guild::leave(settings, guilds, net, now),
            GuildAction::Dissolve => {
                if my_role(settings) == Some(Role::Chief) {
                    draft.confirm_dissolve = Some(now + CONFIRM_SECS);
                }
            }
            GuildAction::CancelDissolve => draft.confirm_dissolve = None,
            GuildAction::ConfirmDissolve => {
                if draft.confirm_dissolve.take().is_some() {
                    guild::edit(settings, guilds, net, now, |g, me| {
                        let name = g.name.clone();
                        g.dissolve(me)?;
                        Ok(Some(format!("La guilde {name} est dissoute.")))
                    });
                }
            }
            GuildAction::Relation(key, label, relation) => {
                change_relation(settings, guilds, net, now, &key, &label, relation);
            }
        }
    }
}

/// Prévient les Officiers (et plus) quand un joueur demande à rejoindre la guilde.
fn announce_requests(
    time: Res<Time>,
    settings: Res<GameSettings>,
    mut net: ResMut<Net>,
    mut seen: Local<Vec<u64>>,
) {
    let Some(g) = settings.guild.as_ref().filter(|_| has_rank(&settings, Role::Officer)) else {
        seen.clear();
        return;
    };
    let requests: Vec<(u64, String)> = net
        .peers
        .values()
        .filter(|p| p.gid == 0 && p.status.req == g.id && !g.refused.contains(&p.status.pid))
        .map(|p| (p.status.pid, p.name.clone()))
        .collect();
    let now = time.elapsed_secs_f64();
    for (pid, name) in &requests {
        if !seen.contains(pid) {
            net.notify(&format!("{name} demande a rejoindre la guilde (G pour repondre)."), now);
        }
    }
    *seen = requests.into_iter().map(|r| r.0).collect();
}
