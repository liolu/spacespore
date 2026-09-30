// ─────────────────────────────────────────────────────────────────────────
//  PNJ des territoires : icône dans le monde + interface de discussion (E)
//
//  Chaque faction PNJ a un représentant alien à son étoile capitale, marqué par
//  une icône. Quand la cible de la caméra est une étoile (ou un corps) du
//  territoire, E ouvre la discussion : dialogues à gauche, alien à droite.
//  Le menu : dialogue, shop (par groupes), vente, échange, missions, infos.
//  Tout passe par l'économie du joueur (`economy.rs`) : crédits, soute,
//  réputation et missions.
// ─────────────────────────────────────────────────────────────────────────

use bevy::prelude::*;

use crate::economy::{self, Economy, GoodId, Mission, MissionKind, Tier, GOODS, GROUPS};
use crate::galaxy_fx::NpcTerritories;
use crate::net::Net;
use crate::net_ui::{button, text, NetPanel, ACCENT, BG_DARK, OK_COLOR, RED_SOFT, TEXT_COLOR, TEXT_DIM};
use crate::settings::GameSettings;
use crate::ui::{CameraTarget, MenuState};
use crate::{target_system, CameraController, StarId, StarRoot, ZoomLevel};

/// Nombre maximal d'icônes affichées en même temps.
const ICON_POOL: usize = 24;
const ICON_SIZE: f32 = 26.0;
const ALIEN_GREEN: Color = Color::srgb(0.45, 0.85, 0.35);
const QTYS: [u32; 3] = [1, 10, 50];

pub struct NpcUiPlugin;

impl Plugin for NpcUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NpcDialog>()
            .add_systems(Startup, setup_npc_ui)
            .add_systems(Update, (update_icons, update_prompt, open_close_dialog, handle_actions, rebuild_dialog).chain());
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Screen {
    Menu,
    Talk,
    Shop,
    Sell,
    Trade,
    Missions,
    Info,
}

#[derive(Resource)]
struct NpcDialog {
    open: bool,
    faction: usize,
    screen: Screen,
    /// Groupe du shop déplié.
    shop_group: Option<usize>,
    /// Réplique en cours.
    line: usize,
    /// Index dans `QTYS`.
    qty: usize,
    /// Remise négociée (0 à 0,1) ; une seule tentative par visite.
    haggle: f64,
    haggled: bool,
    /// Système ciblé à l'ouverture (étoile que l'on peut acheter).
    sys: Option<usize>,
    /// Variante de l'échange proposé.
    trade_seed: usize,
    /// Dernier résultat d'action affiché au joueur.
    feedback: Option<(String, bool)>,
    dirty: bool,
}

impl Default for NpcDialog {
    fn default() -> Self {
        Self {
            open: false,
            faction: 0,
            screen: Screen::Menu,
            shop_group: None,
            line: 0,
            qty: 0,
            haggle: 0.0,
            haggled: false,
            sys: None,
            trade_seed: 0,
            feedback: None,
            dirty: false,
        }
    }
}

#[derive(Component)]
struct NpcIcon(usize);
#[derive(Component)]
struct NpcPrompt;
#[derive(Component)]
struct DialogRoot;
#[derive(Component)]
struct DialogBody;

#[derive(Component, Clone, Copy)]
enum NpcAction {
    Go(Screen),
    Group(usize),
    NextLine,
    CycleQty,
    Haggle,
    Buy(GoodId),
    Sell(GoodId, bool),
    AcceptTrade,
    ModifyTrade,
    TakeMission(usize),
    BuyStar,
    Leave,
}

/// Échange proposé : le PNJ donne `give` contre `take` pris dans la soute.
struct TradeOffer {
    give: (GoodId, u32),
    take: (GoodId, u32),
}

fn trade_offer(faction: usize, seed: usize, eco: &Economy) -> Option<TradeOffer> {
    let owned: Vec<(GoodId, u32)> = eco.inventory.iter().map(|(&g, &n)| (g, n)).collect();
    if owned.is_empty() {
        return None;
    }
    let (tg, have) = owned[economy::pick(faction, seed, 1, owned.len())];
    let take_qty = have.min(5 + economy::pick(faction, seed, 2, 20) as u32).max(1);
    // Le PNJ offre du carburant ou une ressource, pour une valeur proche (90 à 115 %)
    let candidates: Vec<GoodId> = (0..GOODS.len()).filter(|&g| (GOODS[g].1 == 3 || GOODS[g].1 == 4) && g != tg).collect();
    let gg = candidates[economy::pick(faction, seed, 3, candidates.len())];
    let ratio = 0.9 + economy::pick(faction, seed, 4, 26) as f64 / 100.0;
    let value = Economy::base_value(tg, take_qty) as f64 * ratio;
    let give_qty = ((value / GOODS[gg].2 as f64).round() as u32).max(1);
    Some(TradeOffer { give: (gg, give_qty), take: (tg, take_qty) })
}

fn mission_text(m: &Mission, npcs: &NpcTerritories, settings: &GameSettings) -> String {
    format!("{}  -  {} cr, +{} rep", m.describe(npcs, settings), m.reward, m.rep)
}

fn greeting(name: &str, faction: usize, line: usize, tier: Tier) -> String {
    match tier {
        Tier::Enemy => return "Toi ! Apres ce que tu as fait a mon peuple, tu oses te montrer ? Va-t'en.".to_string(),
        Tier::Wary => return "Je me souviens de toi... Fais vite, je n'ai pas confiance.".to_string(),
        Tier::Friend | Tier::Ally => {
            return format!("Mon ami ! {name} t'accueille toujours avec joie. Que puis-je faire pour toi ?")
        }
        _ => {}
    }
    let lines = [
        format!("Salutations, voyageur. Ici {name}. Que puis-je faire pour toi ?"),
        "Les etoiles sont calmes ces jours-ci... pour l'instant. Reste prudent au-dela de nos frontieres.".to_string(),
        "Nos freres anciens disaient que chaque etoile chante. Tu l'entends, toi aussi ?".to_string(),
        "Nous ne connaissons pas ton visage, mais ta reputation, elle, commence a circuler.".to_string(),
    ];
    lines[(faction + line) % lines.len()].clone()
}

fn setup_npc_ui(mut commands: Commands) {
    // Icônes du monde (réserve réutilisée d'une image à l'autre)
    for i in 0..ICON_POOL {
        commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(ICON_SIZE),
                    height: Val::Px(ICON_SIZE),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.05, 0.12, 0.05, 0.85)),
                BorderColor(ALIEN_GREEN),
                BorderRadius::all(Val::Percent(50.0)),
                Visibility::Hidden,
                NpcIcon(i),
            ))
            .with_children(|p| {
                p.spawn(text("!", 16.0, ALIEN_GREEN));
            });
    }

    // Invite « E »
    commands.spawn((
        text("", 16.0, OK_COLOR),
        Node { position_type: PositionType::Absolute, bottom: Val::Px(90.0), left: Val::Percent(50.0), margin: UiRect::left(Val::Px(-130.0)), ..default() },
        Visibility::Hidden,
        NpcPrompt,
    ));

    // Fenêtre de discussion : dialogues à gauche, alien à droite
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                padding: UiRect::all(Val::Px(40.0)),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.03, 0.55)),
            Visibility::Hidden,
            GlobalZIndex(50),
            DialogRoot,
        ))
        .id();
    let left = commands
        .spawn((
            Node {
                width: Val::Px(560.0),
                max_height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(18.0)),
                row_gap: Val::Px(8.0),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(BG_DARK),
            BorderColor(ALIEN_GREEN),
            BorderRadius::all(Val::Px(10.0)),
            DialogBody,
        ))
        .id();
    let right = spawn_alien(&mut commands);
    commands.entity(root).add_children(&[left, right]);
}

/// Un alien dessiné avec des formes : grosse tête, grands yeux noirs, petit corps.
fn spawn_alien(commands: &mut Commands) -> Entity {
    let skin = ALIEN_GREEN;
    let dark = Color::srgb(0.03, 0.05, 0.03);
    let eye = |commands: &mut Commands, tilt: f32| {
        commands
            .spawn((
                Node { width: Val::Px(62.0), height: Val::Px(92.0), ..default() },
                BackgroundColor(dark),
                BorderRadius::all(Val::Percent(50.0)),
                Transform::from_rotation(Quat::from_rotation_z(tilt)),
            ))
            .with_children(|p| {
                p.spawn((
                    Node { position_type: PositionType::Absolute, left: Val::Px(14.0), top: Val::Px(14.0), width: Val::Px(12.0), height: Val::Px(18.0), ..default() },
                    BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.75)),
                    BorderRadius::all(Val::Percent(50.0)),
                ));
            })
            .id()
    };
    let eyes = commands
        .spawn(Node { column_gap: Val::Px(40.0), margin: UiRect::top(Val::Px(70.0)), ..default() })
        .id();
    let (e1, e2) = (eye(commands, 0.45), eye(commands, -0.45));
    commands.entity(eyes).add_children(&[e1, e2]);
    let mouth = commands
        .spawn((
            Node { width: Val::Px(46.0), height: Val::Px(6.0), margin: UiRect::top(Val::Px(34.0)), ..default() },
            BackgroundColor(dark),
            BorderRadius::all(Val::Px(3.0)),
        ))
        .id();
    let head = commands
        .spawn((
            Node { width: Val::Px(260.0), height: Val::Px(330.0), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, ..default() },
            BackgroundColor(skin),
            BorderRadius { top_left: Val::Percent(50.0), top_right: Val::Percent(50.0), bottom_left: Val::Percent(38.0), bottom_right: Val::Percent(38.0) },
        ))
        .id();
    commands.entity(head).add_children(&[eyes, mouth]);
    let neck = commands.spawn((Node { width: Val::Px(54.0), height: Val::Px(26.0), ..default() }, BackgroundColor(Color::srgb(0.36, 0.7, 0.28)))).id();
    let body = commands
        .spawn((
            Node { width: Val::Px(230.0), height: Val::Px(150.0), ..default() },
            BackgroundColor(Color::srgb(0.18, 0.22, 0.42)),
            BorderRadius { top_left: Val::Percent(45.0), top_right: Val::Percent(45.0), bottom_left: Val::Px(0.0), bottom_right: Val::Px(0.0) },
        ))
        .id();
    let col = commands
        .spawn(Node { width: Val::Px(300.0), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, margin: UiRect::right(Val::Px(40.0)), ..default() })
        .id();
    commands.entity(col).add_children(&[head, neck, body]);
    col
}

/// Faction dont l'étoile ciblée fait partie du territoire.
fn targeted_faction(
    target: &CameraTarget,
    star_q: &Query<&StarId, With<StarRoot>>,
    npcs: &NpcTerritories,
) -> Option<usize> {
    let sys = target_system(&target.0, star_q)??;
    npcs.faction_index_of(sys)
}

fn update_icons(
    zoom: Res<ZoomLevel>,
    settings: Res<GameSettings>,
    npcs: Res<NpcTerritories>,
    cam_q: Query<(&Camera, &GlobalTransform, &CameraController)>,
    mut icons: Query<(&NpcIcon, &mut Node, &mut Visibility, &mut BorderColor)>,
    dialog: Res<NpcDialog>,
) {
    let Ok((camera, cam_tf, ctrl)) = cam_q.get_single() else { return };
    let cam_pos = cam_tf.translation();
    let window = (ctrl.distance * 1.5).clamp(60_000.0, 6_000_000.0);
    // Capitales proches, les plus proches d'abord
    let mut near: Vec<(f32, usize)> = Vec::new();
    if *zoom != ZoomLevel::Planet && !dialog.open {
        for (i, f) in npcs.factions.iter().enumerate() {
            let Some(sys) = f.stars.first().and_then(|&s| settings.systems.get(s)) else { continue };
            let d = cam_pos.distance(sys.center());
            if d <= window {
                near.push((d, i));
            }
        }
        near.sort_by(|a, b| a.0.total_cmp(&b.0));
    }
    for (icon, mut node, mut vis, mut border) in &mut icons {
        let shown = near.get(icon.0).and_then(|&(_, fi)| {
            let f = &npcs.factions[fi];
            let sys = settings.systems.get(*f.stars.first()?)?;
            let screen = camera.world_to_viewport(cam_tf, sys.center()).ok()?;
            Some((screen, f.color))
        });
        match shown {
            Some((screen, color)) => {
                node.left = Val::Px(screen.x - ICON_SIZE / 2.0);
                node.top = Val::Px(screen.y - ICON_SIZE - 14.0);
                border.0 = color;
                *vis = Visibility::Visible;
            }
            None => *vis = Visibility::Hidden,
        }
    }
}

fn update_prompt(
    target: Res<CameraTarget>,
    star_q: Query<&StarId, With<StarRoot>>,
    npcs: Res<NpcTerritories>,
    dialog: Res<NpcDialog>,
    mut prompt: Query<(&mut Text, &mut Visibility), With<NpcPrompt>>,
) {
    let Ok((mut t, mut vis)) = prompt.get_single_mut() else { return };
    match targeted_faction(&target, &star_q, &npcs).filter(|_| !dialog.open) {
        Some(fi) => {
            **t = format!("[E] Parler au PNJ - {}", npcs.factions[fi].name);
            *vis = Visibility::Visible;
        }
        None => *vis = Visibility::Hidden,
    }
}

fn open_close_dialog(
    keys: Res<ButtonInput<KeyCode>>,
    panel: Res<NetPanel>,
    menu: Res<MenuState>,
    target: Res<CameraTarget>,
    star_q: Query<&StarId, With<StarRoot>>,
    npcs: Res<NpcTerritories>,
    settings: Res<GameSettings>,
    time: Res<Time>,
    mut eco: ResMut<Economy>,
    mut net: ResMut<Net>,
    mut dialog: ResMut<NpcDialog>,
) {
    if dialog.open && keys.just_pressed(KeyCode::Escape) {
        dialog.open = false;
        dialog.dirty = true;
        return;
    }
    if dialog.open || !keys.just_pressed(KeyCode::KeyE) || panel.focus.is_some() || menu.open {
        return;
    }
    let Some(fi) = targeted_faction(&target, &star_q, &npcs) else { return };
    let sys = target_system(&target.0, &star_q).flatten();
    *dialog = NpcDialog { open: true, faction: fi, sys, dirty: true, ..default() };
    // Colis à livrer à cette faction : remis dès l'arrivée
    while let Some(i) = eco.missions.iter().position(|m| m.kind == (MissionKind::Deliver { to: fi })) {
        let m = eco.complete(i);
        let msg = format!("Colis livre ! +{} cr", m.reward);
        net.notify(&msg, time.elapsed_secs_f64());
        dialog.feedback = Some((format!("{} ({})", msg, m.describe(&npcs, &settings)), true));
    }
}

fn handle_actions(
    q: Query<(&Interaction, &NpcAction), Changed<Interaction>>,
    mut dialog: ResMut<NpcDialog>,
    mut eco: ResMut<Economy>,
    mut npcs: ResMut<NpcTerritories>,
    mut settings: ResMut<GameSettings>,
) {
    for (inter, action) in &q {
        if *inter != Interaction::Pressed {
            continue;
        }
        let fi = dialog.faction;
        let mut result: Option<(String, bool)> = None;
        match *action {
            NpcAction::Go(s) => {
                dialog.screen = s;
                dialog.shop_group = None;
                dialog.feedback = None;
            }
            NpcAction::Group(g) => dialog.shop_group = if dialog.shop_group == Some(g) { None } else { Some(g) },
            NpcAction::NextLine => dialog.line += 1,
            NpcAction::CycleQty => dialog.qty = (dialog.qty + 1) % QTYS.len(),
            NpcAction::Haggle if dialog.haggled => result = Some(("Tu as deja marchande pendant cette visite.".into(), false)),
            NpcAction::Haggle => {
                dialog.haggled = true;
                // Plus la faction t'apprécie, plus elle cède
                let chance = 35 + eco.rep(fi).clamp(-30, 60) as usize / 2;
                if economy::pick(fi, dialog.line + eco.credits as usize, 55, 100) < chance {
                    dialog.haggle = 0.05 + economy::pick(fi, dialog.line, 56, 6) as f64 / 100.0;
                    result = Some((format!("\"Bon... je peux faire un geste.\" Remise de {:.0} % sur les achats.", dialog.haggle * 100.0), true));
                } else {
                    eco.add_rep(fi, -1);
                    result = Some(("\"Mes prix sont deja justes !\" Le PNJ s'agace (-1 reputation).".into(), false));
                }
            }
            NpcAction::Buy(g) => {
                let qty = QTYS[dialog.qty];
                result = Some(match eco.buy(fi, g, qty, dialog.haggle) {
                    Ok(cost) => {
                        eco.add_rep(fi, (cost / 2_000) as i32);
                        (format!("Achete : {qty} x {} pour {cost} cr.", GOODS[g].0), true)
                    }
                    Err(e) => (e.to_string(), false),
                });
            }
            NpcAction::Sell(g, all) => {
                let qty = if all { eco.count(g) } else { QTYS[dialog.qty].min(eco.count(g)) };
                result = Some(match eco.sell(fi, g, qty) {
                    Ok(gain) => {
                        eco.add_rep(fi, (gain / 2_000) as i32);
                        (format!("Vendu : {qty} x {} pour {gain} cr.", GOODS[g].0), true)
                    }
                    Err(e) => (e.to_string(), false),
                });
            }
            NpcAction::ModifyTrade => dialog.trade_seed += 1,
            NpcAction::AcceptTrade => {
                result = Some(match trade_offer(fi, dialog.trade_seed, &eco) {
                    Some(o) if eco.count(o.take.0) >= o.take.1 => {
                        eco.take(o.take.0, o.take.1);
                        eco.give(o.give.0, o.give.1);
                        eco.add_rep(fi, 1);
                        dialog.trade_seed += 1;
                        (format!("Echange conclu : {} x {} contre {} x {}.", o.take.1, GOODS[o.take.0].0, o.give.1, GOODS[o.give.0].0), true)
                    }
                    _ => ("Tu n'as plus de quoi faire cet echange.".to_string(), false),
                });
            }
            NpcAction::TakeMission(slot) => {
                let edition = eco.editions.get(&fi).copied().unwrap_or(0);
                result = Some(match economy::offered_missions(fi, edition, &npcs, &settings).get(slot).cloned() {
                    Some(m) => {
                        let text = m.describe(&npcs, &settings);
                        match eco.accept_mission(m) {
                            Ok(()) => (format!("Mission acceptee : {text}"), true),
                            Err(e) => (e.to_string(), false),
                        }
                    }
                    None => ("Cette mission n'est plus disponible.".to_string(), false),
                });
            }
            NpcAction::BuyStar => result = Some(buy_star(&mut dialog, &mut eco, &mut npcs, &mut settings)),
            NpcAction::Leave => dialog.open = false,
        }
        if result.is_some() {
            dialog.feedback = result;
        }
        dialog.dirty = true;
    }
}

/// Achat de l'étoile ciblée à la faction : elle quitte son territoire et devient une revendication du joueur.
fn buy_star(dialog: &mut NpcDialog, eco: &mut Economy, npcs: &mut NpcTerritories, settings: &mut GameSettings) -> (String, bool) {
    let fi = dialog.faction;
    let Some(sys) = dialog.sys.filter(|&s| npcs.faction_index_of(s) == Some(fi)) else {
        return ("Ciblez une etoile de ce territoire avant d'ouvrir la discussion.".into(), false);
    };
    let Some(info) = settings.systems.get(sys) else { return ("Etoile inconnue.".into(), false) };
    let (name, planets) = (info.name.clone(), info.planets.len());
    if npcs.factions[fi].stars.len() <= economy::MIN_FACTION_STARS {
        return ("\"Je ne vendrai pas une etoile de plus : c'est tout ce qui nous reste.\"".into(), false);
    }
    if settings.claims.len() >= crate::net::MAX_CLAIMS {
        return (format!("Maximum {} etoiles : abandonnez-en une (touche C) avant d'acheter.", crate::net::MAX_CLAIMS), false);
    }
    let price = economy::star_price(eco, fi, planets);
    if eco.credits < price {
        return (format!("Credits insuffisants : {name} coute {price} cr."), false);
    }
    eco.credits -= price;
    eco.sold_stars.push(sys);
    eco.add_rep(fi, 5);
    npcs.remove_star(sys, settings);
    settings.claims.push(sys as u32);
    settings.save();
    dialog.sys = None;
    (format!("{name} ({planets} planetes) est a vous pour {price} cr : elle ne fait plus partie du territoire PNJ."), true)
}

/// Ligne de texte suivie de boutons d'action.
fn row(commands: &mut Commands, label: String, color: Color, buttons: &[Entity]) -> Entity {
    let r = commands
        .spawn(Node { align_items: AlignItems::Center, justify_content: JustifyContent::SpaceBetween, column_gap: Val::Px(8.0), ..default() })
        .id();
    let t = commands.spawn(text(label, 13.0, color)).id();
    commands.entity(r).add_child(t).add_children(buttons);
    r
}

fn rebuild_dialog(
    mut commands: Commands,
    mut dialog: ResMut<NpcDialog>,
    npcs: Res<NpcTerritories>,
    settings: Res<GameSettings>,
    eco: Res<Economy>,
    body_q: Query<Entity, With<DialogBody>>,
    mut root_q: Query<&mut Visibility, With<DialogRoot>>,
) {
    if !dialog.dirty {
        return;
    }
    dialog.dirty = false;
    if let Ok(mut vis) = root_q.get_single_mut() {
        *vis = if dialog.open { Visibility::Visible } else { Visibility::Hidden };
    }
    let Ok(body) = body_q.get_single() else { return };
    commands.entity(body).despawn_descendants();
    let Some(faction) = npcs.factions.get(dialog.faction).filter(|_| dialog.open) else { return };
    let fi = dialog.faction;
    let tier = eco.tier(fi);
    let qty = QTYS[dialog.qty];
    let mut kids: Vec<Entity> = Vec::new();

    let title = commands.spawn(text(format!("Emissaire - {}", faction.name), 20.0, ALIEN_GREEN)).id();
    kids.push(title);
    let sub = commands
        .spawn(text(format!("Credits : {} cr   |   Relation : {} ({})   |   Echap : partir", eco.credits, tier.name(), eco.rep(fi)), 11.0, TEXT_DIM))
        .id();
    kids.push(sub);

    let say = |commands: &mut Commands, s: String| {
        commands.spawn((text(s, 15.0, TEXT_COLOR), Node { margin: UiRect::vertical(Val::Px(6.0)), ..default() })).id()
    };
    let opt = |commands: &mut Commands, label: &str, a: NpcAction| button(commands, label, ACCENT, a);

    // Un ennemi ne commerce pas : retour au menu s'il était dans une boutique
    if !tier.trades() && !matches!(dialog.screen, Screen::Talk | Screen::Menu | Screen::Info) {
        dialog.screen = Screen::Menu;
    }

    match dialog.screen {
        Screen::Menu => {
            kids.push(say(&mut commands, greeting(&faction.name, fi, 0, tier)));
            let entries = if tier.trades() {
                vec![
                    ("Dialoguer", Screen::Talk),
                    ("Ouvrir le shop (acheter)", Screen::Shop),
                    ("Vendre de la soute", Screen::Sell),
                    ("Echanger", Screen::Trade),
                    ("Voir les missions", Screen::Missions),
                    ("Demander des informations", Screen::Info),
                ]
            } else {
                kids.push(commands.spawn(text("Il refuse de commercer avec toi.", 13.0, RED_SOFT)).id());
                vec![("Dialoguer", Screen::Talk), ("Demander des informations", Screen::Info)]
            };
            for (label, s) in entries {
                kids.push(opt(&mut commands, label, NpcAction::Go(s)));
            }
            kids.push(opt(&mut commands, "Partir", NpcAction::Leave));
        }
        Screen::Talk => {
            kids.push(say(&mut commands, format!("\"{}\"", greeting(&faction.name, fi, dialog.line + 1, tier))));
            kids.push(opt(&mut commands, "Continuer la discussion", NpcAction::NextLine));
            kids.push(opt(&mut commands, "Retour", NpcAction::Go(Screen::Menu)));
        }
        Screen::Shop => {
            kids.push(say(&mut commands, "\"Choisis un rayon, voyageur.\"".into()));
            let q = opt(&mut commands, &format!("Quantite : x{qty}"), NpcAction::CycleQty);
            let h = opt(&mut commands, "Negocier", NpcAction::Haggle);
            let bar = commands.spawn(Node { column_gap: Val::Px(8.0), ..default() }).id();
            commands.entity(bar).add_children(&[q, h]);
            kids.push(bar);
            if dialog.haggle > 0.0 {
                kids.push(commands.spawn(text(format!("Remise negociee : {:.0} %", dialog.haggle * 100.0), 12.0, OK_COLOR)).id());
            }
            for (g, name) in GROUPS.iter().enumerate() {
                let open = dialog.shop_group == Some(g);
                kids.push(opt(&mut commands, &format!("{} {}", if open { "v" } else { ">" }, name), NpcAction::Group(g)));
                if open {
                    for good in economy::goods_of_group(g) {
                        let price = eco.buy_price(fi, good, dialog.haggle);
                        let b = opt(&mut commands, &format!("Acheter x{qty} ({} cr)", price * qty as i64), NpcAction::Buy(good));
                        let owned = eco.count(good);
                        kids.push(row(&mut commands, format!("{}  -  {price} cr  (en soute : {owned})", GOODS[good].0), TEXT_COLOR, &[b]));
                    }
                }
            }
            // Territoire : l'étoile ciblée à l'ouverture, si elle est à cette faction
            kids.push(commands.spawn(text("Territoire", 12.0, TEXT_DIM)).id());
            match dialog.sys.filter(|&s| npcs.faction_index_of(s) == Some(fi)).and_then(|s| settings.systems.get(s)) {
                Some(info) => {
                    let price = economy::star_price(&eco, fi, info.planets.len());
                    let b = opt(&mut commands, &format!("Acheter ({price} cr)"), NpcAction::BuyStar);
                    kids.push(row(&mut commands, format!("{}  -  {} planetes", info.name, info.planets.len()), TEXT_COLOR, &[b]));
                }
                None => kids.push(commands.spawn(text("Ciblez une de leurs etoiles, puis E, pour l'acheter.", 11.0, TEXT_DIM)).id()),
            }
            kids.push(commands.spawn(text("Vaisseaux : bientot disponibles.", 11.0, TEXT_DIM)).id());
            kids.push(opt(&mut commands, "Retour", NpcAction::Go(Screen::Menu)));
        }
        Screen::Sell => {
            kids.push(say(&mut commands, "\"Montre-moi ce que tu as en soute.\"".into()));
            kids.push(opt(&mut commands, &format!("Quantite : x{qty}"), NpcAction::CycleQty));
            let stock: Vec<(GoodId, u32)> = eco.inventory.iter().map(|(&g, &n)| (g, n)).collect();
            if stock.is_empty() {
                kids.push(commands.spawn(text("Ta soute est vide.", 13.0, TEXT_DIM)).id());
            }
            for (good, n) in stock {
                let price = eco.sell_price(fi, good);
                let one = opt(&mut commands, &format!("Vendre x{}", qty.min(n)), NpcAction::Sell(good, false));
                let all = opt(&mut commands, "Tout", NpcAction::Sell(good, true));
                kids.push(row(&mut commands, format!("{} x{n}  -  rachat {price} cr", GOODS[good].0), TEXT_COLOR, &[one, all]));
            }
            kids.push(opt(&mut commands, "Retour", NpcAction::Go(Screen::Menu)));
        }
        Screen::Trade => match trade_offer(fi, dialog.trade_seed, &eco) {
            None => {
                kids.push(say(&mut commands, "\"Tu n'as rien a echanger, voyageur.\"".into()));
                kids.push(opt(&mut commands, "Retour", NpcAction::Go(Screen::Menu)));
            }
            Some(o) => {
                kids.push(say(&mut commands, format!("\"Je peux te donner {} x {} contre quelque chose d'interessant.\"", o.give.1, GOODS[o.give.0].0)));
                let mine = Economy::base_value(o.take.0, o.take.1);
                let theirs = Economy::base_value(o.give.0, o.give.1);
                kids.push(
                    commands
                        .spawn(text(
                            format!(
                                "Echange propose\nVous : {} x {}\nPNJ : {} x {}\nValeur de votre offre : {mine} cr\nValeur de l'offre du PNJ : {theirs} cr\nDifference : {:+} cr",
                                o.take.1, GOODS[o.take.0].0, o.give.1, GOODS[o.give.0].0, theirs - mine
                            ),
                            13.0,
                            OK_COLOR,
                        ))
                        .id(),
                );
                let a = opt(&mut commands, "Accepter", NpcAction::AcceptTrade);
                let m = opt(&mut commands, "Modifier l'offre", NpcAction::ModifyTrade);
                let bar = commands.spawn(Node { column_gap: Val::Px(8.0), ..default() }).id();
                commands.entity(bar).add_children(&[a, m]);
                kids.push(bar);
                kids.push(opt(&mut commands, "Annuler", NpcAction::Go(Screen::Menu)));
            }
        },
        Screen::Missions => {
            kids.push(say(&mut commands, "\"J'ai du travail pour un pilote de ton genre.\"".into()));
            let edition = eco.editions.get(&fi).copied().unwrap_or(0);
            for (slot, m) in economy::offered_missions(fi, edition, &npcs, &settings).iter().enumerate() {
                let b = opt(&mut commands, "Accepter", NpcAction::TakeMission(slot));
                kids.push(row(&mut commands, mission_text(m, &npcs, &settings), TEXT_COLOR, &[b]));
            }
            kids.push(commands.spawn(text(format!("En cours ({}/{}) :", eco.missions.len(), economy::MAX_ACTIVE_MISSIONS), 12.0, TEXT_DIM)).id());
            for m in &eco.missions {
                kids.push(commands.spawn(text(format!("- {}", m.describe(&npcs, &settings)), 12.0, ALIEN_GREEN)).id());
            }
            kids.push(opt(&mut commands, "Retour", NpcAction::Go(Screen::Menu)));
        }
        Screen::Info => {
            let n = faction.stars.len();
            kids.push(say(&mut commands, format!("\"{} controle {n} etoiles. Notre territoire est ferme aux revendications des etrangers.\"", faction.name)));
            let hint = match tier {
                Tier::Ally => "Tu es notre allie : remise maximale.",
                _ => "Commerce et missions font monter la reputation, donc les remises (jusqu'a 15 %).",
            };
            kids.push(commands.spawn(text(hint, 12.0, TEXT_DIM)).id());
            kids.push(opt(&mut commands, "Retour", NpcAction::Go(Screen::Menu)));
        }
    }
    if let Some((msg, ok)) = dialog.feedback.clone() {
        kids.push(commands.spawn(text(msg, 13.0, if ok { OK_COLOR } else { RED_SOFT })).id());
    }
    commands.entity(body).add_children(&kids);
}
