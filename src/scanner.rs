// ─────────────────────────────────────────────────────────────────────────
//  Scanner de l'astre ciblé (phase 9 de `roadmaps/fait/ROADMAP-0.10.md`)
//
//  Panneau à droite de l'écran, en sections fixes (C5 de `roadmaps/fait/ROADMAP-0.12-correctifs.md`) :
//  « Ici et maintenant » (heure, températures, météo, mis à jour une fois par seconde), puis
//  identité, physique, rotation et orbite, atmosphère, eau, vie, ressources. Libellés et valeurs
//  en colonnes : rien ne change de place. Touche I : l'afficher ou le masquer. Il lit le profil de
//  l'astre (`planetgen::profile`), le même que l'export `/profil`.
// ─────────────────────────────────────────────────────────────────────────

use bevy::prelude::*;

use crate::chat_cmd::target_body;
use crate::net_ui::NetPanel;
use crate::planet::{StarId, StarRoot};
use crate::planetgen::cache::{profile_of, Profile, ProfileCache};
use crate::planetgen::profile::{OrbitSection, PlanetProfile, Realism, StarProfile};
use crate::settings::GameSettings;
use crate::ui::{CameraTarget, MenuState};
use crate::world_clock::{duration_text, hour_text};

pub const SCANNER_KEY: KeyCode = KeyCode::KeyI;

pub struct ScannerPlugin;

impl Plugin for ScannerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Scanner>().add_systems(Startup, setup_scanner).add_systems(Update, update_scanner);
    }
}

/// Une section du panneau : titre et lignes (libellé, valeur).
#[derive(Clone, Debug, Default, PartialEq)]
struct Section {
    title: String,
    rows: Vec<(String, String)>,
}

impl Section {
    fn new(title: &str) -> Self {
        Self { title: title.into(), rows: Vec::new() }
    }

    fn row(&mut self, label: &str, value: impl Into<String>) {
        self.rows.push((label.into(), value.into()));
    }

    /// Lignes « libellé : valeur » d'un texte (sinon toute la ligne en valeur).
    fn lines(&mut self, text: &str) {
        for l in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
            match l.split_once(" : ") {
                Some((a, b)) if a.len() <= 24 => self.row(a, b),
                _ => self.row("", l),
            }
        }
    }
}

#[derive(Resource)]
struct Scanner {
    visible: bool,
    /// Astre affiché (clé `BodyId`) : son titre, ses sections, son niveau de danger.
    shown: Option<String>,
    title: String,
    sections: Vec<Section>,
    danger: u8,
    /// « Ici et maintenant » : lignes affichées, et depuis quand (mises à jour à 1 Hz).
    live: Vec<(String, String)>,
    live_age: f32,
    /// Le panneau doit être reconstruit (nouvel astre, ou les lignes vivantes changent).
    dirty: bool,
    /// Historique de l'astre (dex, 0.13.1).
    history: Option<Section>,
}

impl Default for Scanner {
    fn default() -> Self {
        Self { visible: true, shown: None, title: String::new(), sections: Vec::new(), danger: 0, live: Vec::new(), live_age: 0.0, dirty: true, history: None }
    }
}

#[derive(Component)]
struct ScannerPanel;

/// Valeur vivante (rang dans « Ici et maintenant »).
#[derive(Component)]
struct LiveCell(usize);

const TEXT: Color = Color::srgb(0.85, 0.92, 1.0);
const DIM: Color = Color::srgb(0.55, 0.65, 0.78);
const TITLE: Color = Color::srgb(0.45, 0.75, 1.0);
const LABEL_W: f32 = 96.0;

fn setup_scanner(mut commands: Commands) {
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(12.0),
            top: Val::Px(110.0),
            width: Val::Px(430.0),
            max_height: Val::Percent(80.0),
            overflow: Overflow::clip_y(),
            padding: UiRect::all(Val::Px(10.0)),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(1.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.02, 0.03, 0.06, 0.72)),
        BorderRadius::all(Val::Px(6.0)),
        Visibility::Hidden,
        ScannerPanel,
    ));
}

fn pct(x: f64) -> String {
    format!("{:.0} %", x * 100.0)
}

fn realism_mark(r: Realism) -> &'static str {
    match r {
        Realism::Realistic => "",
        Realism::Speculative => " (speculatif)",
        Realism::Fictional => " (fictif)",
    }
}

fn star_sections(s: &StarProfile) -> (String, Vec<Section>, u8) {
    let mut id = Section::new("IDENTITE");
    id.row("Type", format!("{}  {}", s.class, s.spectral_type.clone().unwrap_or_default()));
    let mut ph = Section::new("PHYSIQUE");
    ph.row("Temperature", format!("{:.0} K", s.temperature_k));
    if let Some(lum) = s.luminosity_sun {
        ph.row("Luminosite", format!("{lum:.3} L sol"));
    }
    if let Some(m) = s.mass_sun {
        ph.row("Masse", format!("{m:.2} M sol"));
    }
    ph.row("Rayon", format!("{:.3} R sol", s.radius_sun));
    let mut act = Section::new("ACTIVITE");
    if let (Some(age), Some(a)) = (s.age_gyr, s.magnetic_activity) {
        act.row("Age", format!("{age:.2} Gyr"));
        act.row("Activite", format!("{a:.2}   eruptions {}", s.flares));
    }
    if let (Some(uv), Some(x)) = (s.uv_flux, s.xray_flux) {
        act.row("UV / X", format!("{uv:.2} / {x:.1}  (Soleil = 1)"));
    }
    let danger = if s.xray_flux.unwrap_or(0.0) > 20.0 { 2 } else { 0 };
    (s.name.clone(), [id, ph, act].into_iter().filter(|x| !x.rows.is_empty()).collect(), danger)
}

/// Type secondaire : ce qui distingue ce monde (océan, lave, glace, désert, jardin...).
fn secondary(p: &PlanetProfile) -> String {
    let h = &p.hydrology;
    let ocean = h.ocean_fraction.unwrap_or(0.0);
    let liquid = h.ocean_liquid.as_deref().unwrap_or("aucun");
    let pressure = p.atmosphere.surface_pressure_bar.unwrap_or(0.0);
    let t = p.climate.mean_temperature_c;
    if p.gameplay.rogue {
        return "planete errante : aucune etoile, nuit eternelle".into();
    }
    if !p.gameplay.walkable {
        return "pas de sol, nuages a perte de vue".into();
    }
    if liquid == "lave" {
        return "monde de lave".into();
    }
    if liquid == "methane" {
        return "lacs de methane (comme Titan)".into();
    }
    if p.gameplay.habitability.unwrap_or(0.0) > 0.6 {
        return "monde-jardin".into();
    }
    if ocean > 0.9 {
        return "monde-ocean".into();
    }
    if pressure > 30.0 {
        return "enfer de serre (comme Venus)".into();
    }
    if p.gameplay.subsurface_ocean {
        return "ocean cache sous la glace".into();
    }
    if t < -60.0 {
        return "monde glace".into();
    }
    if !p.atmosphere.present {
        return "monde nu, sans air".into();
    }
    match p.biology.biomes.first() {
        Some(b) => format!("surtout {}", b.name),
        None => "monde sec".into(),
    }
}

/// Durée réelle en heures : « 23,9 h » ou « 12,3 j ».
fn real_hours(h: f64) -> String {
    if h < 48.0 { format!("{h:.1} h") } else { format!("{:.1} j", h / 24.0) }
}

/// Durée réelle en jours : « 225 j » ou « 11,9 ans ».
fn real_days(d: f64) -> String {
    if d < 700.0 { format!("{d:.0} j") } else { format!("{:.1} ans", d / 365.25) }
}

/// Section « Rotation et orbite » (C5) : le jour et l'année, en vraie valeur et en temps de jeu.
fn rotation_section(o: &OrbitSection, moon: bool) -> Section {
    let mut s = Section::new("ROTATION ET ORBITE");
    let game = |x: Option<f64>| x.map(duration_text);
    match (o.tidally_locked, moon) {
        (_, true) => s.row("Jour", "synchrone : une face vers sa planete"),
        (Some(true), _) => s.row("Jour", "synchrone : une face vers l'etoile"),
        _ => match (o.rotation_period_h, game(o.day_game_s)) {
            (Some(h), Some(g)) => s.row("Jour", format!("{}  (jeu : {g})", real_hours(h))),
            (Some(h), None) => s.row("Jour", real_hours(h)),
            (None, Some(g)) => s.row("Jour", format!("jeu : {g}")),
            (None, None) => {}
        },
    }
    match (moon, o.period_days, game(o.year_game_s)) {
        (true, _, Some(g)) => s.row("Annee", format!("celle de sa planete  (saisons en jeu : {g})")),
        (false, Some(d), Some(g)) => s.row("Annee", format!("{}  (saisons en jeu : {g})", real_days(d))),
        (false, Some(d), None) => s.row("Annee", real_days(d)),
        _ => {}
    }
    if let Some(g) = game(o.orbit_game_s) {
        s.row("Orbite", format!("un tour {} en {g} de jeu", if moon { "de sa planete" } else { "de l'etoile" }));
    }
    if let Some(t) = o.axial_tilt_deg {
        s.row("Inclinaison", format!("{t:.0} deg"));
    }
    s
}

fn body_sections(p: &PlanetProfile) -> (String, Vec<Section>, u8) {
    let moon = matches!(p.kind, crate::planetgen::profile::BodyKind::Moon);
    let main = match (moon, p.physics.size_class.as_deref()) {
        (true, _) => "lune".to_string(),
        (_, Some(class)) => class.to_string(),
        _ => "planete".to_string(),
    };
    let mut id = Section::new("IDENTITE");
    id.row("Type", main);
    id.row("", secondary(p));
    let traits: Vec<String> = p.traits.iter().map(|t| format!("{}{}", t.name, realism_mark(t.realism))).collect();
    if !traits.is_empty() {
        id.row("Traits", traits.join(", "));
    }

    let mut ph = Section::new("PHYSIQUE");
    ph.row("Gravite", format!("{:.2} g", p.physics.surface_gravity_g));
    let pressure = match p.atmosphere.surface_pressure_bar {
        Some(b) if b >= 0.01 => format!("{b:.2} bar"),
        Some(b) if b > 0.0 => format!("{:.1} mbar", b * 1000.0),
        _ => "vide".into(),
    };
    ph.row("Pression", pressure);
    let mut t = format!("{:.0} C", p.climate.mean_temperature_c);
    if let (Some(e), Some(po)) = (p.climate.equator_c, p.climate.pole_c) {
        t += &format!("  (equateur {e:.0}, poles {po:.0})");
    }
    ph.row("Temperature", t);
    let mut extra = Vec::new();
    if p.gameplay.ring.is_some() {
        extra.push("anneaux".to_string());
    }
    if let Some(t) = p.gameplay.tidal_heating.filter(|t| *t > 0.05) {
        extra.push(format!("marees {t:.2}"));
    }
    if !extra.is_empty() {
        ph.row("Autour", extra.join(", "));
    }

    let rot = rotation_section(&p.orbit, moon);

    let mut air = Section::new("ATMOSPHERE");
    if p.atmosphere.gases.is_empty() {
        air.row("Gaz", "aucune");
    } else {
        let gases: Vec<String> = p.atmosphere.gases.iter().take(4).map(|g| format!("{} {}{}", g.formula, pct(g.fraction), realism_mark(g.realism))).collect();
        air.row("Gaz", gases.join(", "));
        air.row("Nuages", p.atmosphere.clouds.clone());
    }

    let mut water = Section::new("EAU");
    let h = &p.hydrology;
    let mut w = h.water_state.clone().unwrap_or_else(|| "?".into());
    if let (Some(o), Some(liq)) = (h.ocean_fraction, h.ocean_liquid.as_deref()) {
        if o > 0.0 {
            w += &format!(", mers {} ({liq})", pct(o));
        }
    }
    if p.gameplay.subsurface_ocean {
        w += ", ocean sous la glace";
    }
    water.row("Eau", w);

    let mut life = Section::new("VIE");
    if let Some(l) = &p.biology.life {
        let mut v = l.clone();
        if let Some(chem) = &p.biology.biochemistry {
            v += &format!("  -  {chem}");
        }
        life.row("Vie", v);
        if let Some(f) = &p.biology.fauna {
            life.row("Faune", format!("~{} especes, jusqu'a {:.0} m ({})", f.species, f.max_size_m, f.locomotion.join(", ")));
        }
    }
    if !p.biology.biomes.is_empty() {
        let list: Vec<String> = p.biology.biomes.iter().take(3).map(|x| format!("{} {}", x.name, pct(x.fraction))).collect();
        life.row("Biomes", list.join(", "));
    }
    if let Some(score) = p.gameplay.habitability {
        life.row("Habitabilite", format!("{score:.2}  {}", p.gameplay.habitability_label.clone().unwrap_or_default()));
    }
    if !p.gameplay.hazards.is_empty() {
        life.row("Dangers", p.gameplay.hazards.join(", "));
    }

    let mut res = Section::new("RESSOURCES");
    if !p.resources.ores.is_empty() {
        let list: Vec<String> = p
            .resources
            .ores
            .iter()
            .take(5)
            .map(|o| format!("{}{} ({})", o.ore, realism_mark(o.realism), crate::planetgen::profile::difficulty_label(o.difficulty)))
            .collect();
        res.row("Minerais", list.join(", "));
    }
    let sections = [id, ph, rot, air, water, life, res].into_iter().filter(|s| !s.rows.is_empty()).collect();
    (p.name.clone(), sections, p.gameplay.danger_level)
}

/// Sections au format du dex.
fn info_sections(sections: &[Section]) -> Vec<crate::dex::InfoSection> {
    sections.iter().map(|s| (s.title.clone(), s.rows.clone())).collect()
}

/// Texte d'un panneau (tests, et pour lire ce que montre le scanner).
#[cfg(test)]
fn sections_text(title: &str, sections: &[Section]) -> String {
    let mut out = vec![format!("SCANNER  -  {title}")];
    for s in sections {
        out.push(s.title.clone());
        out.extend(s.rows.iter().map(|(a, b)| format!("{a} : {b}")));
    }
    out.join("\n")
}

/// Lignes « Ici et maintenant » (heure, saison, températures, météo, lunes, grotte), arrondies et
/// à largeur fixe.
fn live_rows(weather: &crate::world_clock::LocalWeather, now: Option<&str>, phases: Option<&str>, cave: Option<&str>, landing: Option<&str>) -> Vec<(String, String)> {
    let mut s = Section::new("");
    if weather.body.is_some() {
        let h = hour_text(weather.hour);
        s.row("Heure", format!("{h:>7}, soleil a {:>3.0} deg", weather.sun_deg));
        s.row("Saison", format!("{} (lat. {:.0} deg)", weather.season, weather.lat_deg));
        s.row("Temperature", format!("{:>4.0} C", weather.temp));
        s.row("Aujourd'hui", format!("{:>4.0} a {:>4.0} C", weather.day.0, weather.day.1));
        s.row("Cette annee", format!("{:>4.0} a {:>4.0} C", weather.year.0, weather.year.1));
    }
    for t in [now, phases, cave, landing].into_iter().flatten() {
        s.lines(t);
    }
    s.rows
}

/// Reconstruit le panneau : titre, sections, « Ici et maintenant », pied.
fn rebuild(commands: &mut Commands, panel: Entity, scanner: &Scanner) {
    let danger_color = match scanner.danger {
        3 => Color::srgb(1.0, 0.6, 0.55),
        2 => Color::srgb(1.0, 0.85, 0.5),
        _ => TEXT,
    };
    commands.entity(panel).despawn_descendants().with_children(|p| {
        p.spawn((Text::new(format!("SCANNER  -  {}", scanner.title)), TextFont { font_size: 14.0, ..default() }, TextColor(danger_color)));
        let live = (!scanner.live.is_empty()).then(|| Section { title: "ICI ET MAINTENANT".into(), rows: scanner.live.clone() });
        // « Ici et maintenant » d'abord (on le lit sur place) : ses libellés ne changent pas ;
        // l'historique de l'astre (dex) à la fin
        for (k, s) in live.iter().chain(scanner.sections.iter()).chain(scanner.history.iter()).enumerate() {
            let is_live = k == 0 && live.is_some();
            p.spawn((Text::new(s.title.clone()), TextFont { font_size: 11.0, ..default() }, TextColor(TITLE), Node { margin: UiRect::top(Val::Px(5.0)), ..default() }));
            for (i, (label, value)) in s.rows.iter().enumerate() {
                p.spawn(Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(6.0), ..default() }).with_children(|r| {
                    r.spawn((Text::new(label.clone()), TextFont { font_size: 11.5, ..default() }, TextColor(DIM), Node { width: Val::Px(LABEL_W), flex_shrink: 0.0, ..default() }));
                    let color = if label == "Dangers" { danger_color } else { TEXT };
                    let mut v = r.spawn((Text::new(value.clone()), TextFont { font_size: 11.5, ..default() }, TextColor(color), Node { flex_grow: 1.0, flex_shrink: 1.0, ..default() }));
                    if is_live {
                        v.insert(LiveCell(i));
                    }
                });
            }
        }
        p.spawn((Text::new("(I : masquer)"), TextFont { font_size: 11.0, ..default() }, TextColor(DIM), Node { margin: UiRect::top(Val::Px(5.0)), ..default() }));
    });
}

#[allow(clippy::too_many_arguments)]
fn update_scanner(
    mut commands: Commands,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    panel_ui: Res<NetPanel>,
    menu: Res<MenuState>,
    settings: Res<GameSettings>,
    cache: Res<ProfileCache>,
    target: Res<CameraTarget>,
    weather: Res<crate::world_clock::LocalWeather>,
    (cave, phases, weather_now, approach): (Res<crate::surface::NearestCave>, Res<crate::sky::MoonPhases>, Res<crate::weather::WeatherNow>, Res<crate::approche_ui::Approach>),
    (field, mut last, dex, dex_ui): (Res<crate::asteroids::AsteroidField>, ResMut<crate::dex::LastScan>, Res<crate::dex::Dex>, Res<crate::dex::DexUi>),
    star_q: Query<&StarId, With<StarRoot>>,
    mut scanner: ResMut<Scanner>,
    mut panel: Query<(Entity, &mut Visibility), With<ScannerPanel>>,
    mut cells: Query<(&LiveCell, &mut Text)>,
) {
    if keys.just_pressed(SCANNER_KEY) && panel_ui.focus.is_none() && !menu.open {
        scanner.visible = !scanner.visible;
    }
    let loaded = matches!(target.0, crate::ui::TargetKind::Star(id) if star_q.iter().any(|s| s.0 == id));
    if let crate::ui::TargetKind::Asteroid(k) = target.0 {
        // Astéroïde (C1) : calculé une fois affiché
        let key = Some(format!("{k:?}"));
        if key != scanner.shown {
            if let Some(a) = field.get(&k) {
                let lum = settings.systems.get(k.sys as usize).and_then(|s| s.lighting()).map_or(1.0, |p| p.luminosity_sun);
                let text = crate::asteroids::scanner_text(a, field.sources(k.sys as usize), lum);
                let mut lines = text.lines();
                scanner.title = lines.next().and_then(|l| l.split_once("-  ")).map_or(String::new(), |(_, n)| n.trim().to_string());
                let mut s = Section::new("ASTRE");
                s.lines(&lines.filter(|l| !l.starts_with("(I")).collect::<Vec<_>>().join("\n"));
                scanner.sections = vec![s];
                scanner.danger = 0;
                scanner.shown = key.clone();
                scanner.dirty = true;
                // Au dex : comètes à part, le reste avec les astéroïdes
                let category = if matches!(k.source(), crate::asteroids::Source::Comet) { "Cometes" } else { "Asteroides" };
                let title = scanner.title.clone();
                last.set(key.unwrap_or_default(), category, &title, info_sections(&scanner.sections));
            }
        }
    } else {
        let id = target_body(&target.0, loaded);
        let key = id.map(|i| i.key());
        // Recalcul seulement quand la cible change, ou quand le système chargé change (profils en cache)
        if key != scanner.shown || cache.is_changed() {
            scanner.shown = key.clone();
            let (title, mut sections, danger) = match id.and_then(|i| profile_of(&settings, &cache, i)) {
                Some(Profile::Star(s)) => star_sections(&s),
                Some(Profile::Body(b)) => body_sections(&b),
                None => (String::new(), Vec::new(), 0),
            };
            // Ceintures d'astéroïdes et étoiles multiples du système de l'étoile
            if let (Some(crate::planetgen::live::BodyId::Star { system, .. }), false) = (id, sections.is_empty()) {
                let mut sys = Section::new("SYSTEME");
                if let Some(line) = settings.systems.get(system as usize).and_then(|s| crate::asteroids::belts_line(&crate::asteroids::Sources::of(s))) {
                    sys.lines(&line);
                }
                if let Some(st) = settings.systems.get(system as usize).and_then(|s| s.stellar()).filter(|st| !st.companions.is_empty()) {
                    sys.row("Etoiles", st.kind.name());
                    sys.row("Zone habitable", format!("pour {:.2} L sol", st.luminosity));
                }
                if !sys.rows.is_empty() {
                    sections.push(sys);
                }
            }
            // Au dex (seulement une vraie cible : un astre trouvé)
            if let (Some(k), Some(b), false) = (key.clone(), id, sections.is_empty()) {
                let category = match b {
                    crate::planetgen::live::BodyId::Star { .. } => "Etoiles",
                    crate::planetgen::live::BodyId::Planet { .. } => "Planetes",
                    crate::planetgen::live::BodyId::Moon { .. } => "Lunes",
                };
                last.set(k, category, &title, info_sections(&sections));
            }
            scanner.title = title;
            scanner.sections = sections;
            scanner.danger = danger;
            scanner.dirty = true;
        }
    }
    // Historique de l'astre (dex) : à jour à chaque changement du dex
    if dex.is_changed() || scanner.dirty {
        let h = scanner.shown.as_deref().and_then(|k| dex.history(k)).map(|(title, rows)| Section { title, rows });
        if h != scanner.history {
            scanner.history = h;
            scanner.dirty = true;
        }
    }
    // « Ici et maintenant » : sur l'astre visité ou survolé, une fois par seconde
    scanner.live_age += time.delta_secs();
    if scanner.live_age >= 1.0 || scanner.dirty {
        scanner.live_age = 0.0;
        let here = weather.body.is_some() && weather.body == Some(target.0);
        let now_text = (weather_now.body == Some(target.0) && !weather_now.text.is_empty()).then_some(weather_now.text.as_str());
        let phase_text = (phases.target == Some(target.0) && !phases.text.is_empty()).then_some(phases.text.as_str());
        let cave_text = (here && !cave.text.is_empty()).then_some(cave.text.as_str());
        let empty = crate::world_clock::LocalWeather::default();
        let landing_text = (here && !approach.landing_text.is_empty()).then_some(approach.landing_text.as_str());
        let rows = live_rows(if here { &weather } else { &empty }, now_text, phase_text, cave_text, landing_text);
        // Mêmes libellés : seules les valeurs changent (pas de reconstruction)
        let same = rows.len() == scanner.live.len() && rows.iter().zip(&scanner.live).all(|(a, b)| a.0 == b.0);
        if !same {
            scanner.dirty = true;
        }
        scanner.live = rows;
        if !scanner.dirty {
            for (c, mut t) in &mut cells {
                if let Some((_, v)) = scanner.live.get(c.0) {
                    if t.0 != *v {
                        t.0 = v.clone();
                    }
                }
            }
        }
    }
    let show = scanner.visible && !scanner.sections.is_empty() && !dex_ui.open;
    for (e, mut v) in &mut panel {
        let wanted = if show { Visibility::Inherited } else { Visibility::Hidden };
        if *v != wanted {
            *v = wanted;
        }
        if scanner.dirty {
            rebuild(&mut commands, e, &scanner);
        }
    }
    scanner.dirty = false;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planetgen::live::BodyId;

    #[test]
    fn the_scanner_describes_stars_planets_and_moons() {
        let settings = GameSettings::default();
        let si = (0..200).find(|&s| settings.systems[s].planets().iter().any(|p| !p.moons.is_empty())).unwrap();
        let cache = ProfileCache::build(&settings, si);
        let sys = &settings.systems[si];
        let pi = sys.planets().iter().position(|p| !p.moons.is_empty()).unwrap();
        for id in [
            BodyId::Star { system: si as u32, index: 0 },
            BodyId::Planet { system: si as u32, index: pi as u16 },
            BodyId::Moon { system: si as u32, planet: pi as u16, index: 0 },
        ] {
            let (text, danger) = match profile_of(&settings, &cache, id).unwrap() {
                Profile::Star(s) => {
                    let (t, sec, d) = star_sections(&s);
                    (sections_text(&t, &sec), d)
                }
                Profile::Body(b) => {
                    let (t, sec, d) = body_sections(&b);
                    let t = sections_text(&t, &sec);
                    for field in ["Type :", "Gravite", "Pression", "Temperature", "Gaz", "Eau :", "Habitabilite", "ROTATION ET ORBITE", "Annee", "Orbite"] {
                        assert!(t.contains(field), "{field} absent :\n{t}");
                    }
                    (t, d)
                }
            };
            assert!(text.starts_with("SCANNER"), "{text}");
            assert!(danger <= 3);
        }
    }

    /// C5 : le jour et l'année d'une planète, en vrai et en temps de jeu (1 h de la planète = 1 min).
    #[test]
    fn the_day_and_year_are_shown_real_and_in_game() {
        let mut o = OrbitSection { rotation_period_h: Some(24.0), tidally_locked: Some(false), period_days: Some(365.25), day_game_s: Some(1440.0), year_game_s: Some(4.0 * 3600.0), orbit_game_s: Some(7200.0), axial_tilt_deg: Some(23.0), ..Default::default() };
        let s = rotation_section(&o, false);
        let text = format!("{:?}", s.rows);
        assert!(text.contains("24.0 h") && text.contains("jeu : 24 min"), "{text}");
        assert!(text.contains("365 j") && text.contains("4 h"), "{text}");
        o.tidally_locked = Some(true);
        assert!(format!("{:?}", rotation_section(&o, false).rows).contains("synchrone"));
    }

    /// C5 : « Ici et maintenant » garde les mêmes libellés d'une seconde à l'autre (rien ne bouge).
    #[test]
    fn live_rows_keep_their_labels() {
        let mut w = crate::world_clock::LocalWeather { body: Some(crate::ui::TargetKind::Planet(0)), hour: 9.5, temp: 12.3, ..Default::default() };
        let a = live_rows(&w, Some("Ciel : clair"), None, None, None);
        w.hour = 10.25;
        w.temp = -3.0;
        let b = live_rows(&w, Some("Ciel : pluie"), None, None, None);
        assert_eq!(a.iter().map(|x| &x.0).collect::<Vec<_>>(), b.iter().map(|x| &x.0).collect::<Vec<_>>());
        assert_eq!(a[0].1.len(), b[0].1.len(), "{} / {}", a[0].1, b[0].1);
        assert_eq!(a[2].1.len(), b[2].1.len());
    }
}
