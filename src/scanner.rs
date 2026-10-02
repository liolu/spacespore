// ─────────────────────────────────────────────────────────────────────────
//  Scanner de l'astre ciblé (phase 9 de `ROADMAP-0.10.md`)
//
//  Panneau à droite de l'écran : type principal et secondaire, gravité, pression, température,
//  atmosphère, eau, habitabilité, dangers, traits. Touche I : l'afficher ou le masquer. Il lit le
//  profil de l'astre (`planetgen::profile`), le même que l'export `/profil`.
// ─────────────────────────────────────────────────────────────────────────

use bevy::prelude::*;

use crate::chat_cmd::target_body;
use crate::net_ui::NetPanel;
use crate::planet::{StarId, StarRoot};
use crate::planetgen::cache::{profile_of, Profile, ProfileCache};
use crate::planetgen::profile::{PlanetProfile, Realism, StarProfile};
use crate::settings::GameSettings;
use crate::ui::{CameraTarget, MenuState};

pub const SCANNER_KEY: KeyCode = KeyCode::KeyI;

pub struct ScannerPlugin;

impl Plugin for ScannerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Scanner>().add_systems(Startup, setup_scanner).add_systems(Update, update_scanner);
    }
}

#[derive(Resource)]
struct Scanner {
    visible: bool,
    /// Astre affiché (clé `BodyId`) et texte calculé pour lui.
    shown: Option<String>,
    text: String,
    danger: u8,
}

impl Default for Scanner {
    fn default() -> Self {
        Self { visible: true, shown: None, text: String::new(), danger: 0 }
    }
}

#[derive(Component)]
struct ScannerPanel;

#[derive(Component)]
struct ScannerText;

fn setup_scanner(mut commands: Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(12.0),
                top: Val::Px(110.0),
                width: Val::Px(330.0),
                padding: UiRect::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.03, 0.06, 0.72)),
            BorderRadius::all(Val::Px(6.0)),
            Visibility::Hidden,
            ScannerPanel,
        ))
        .with_children(|p| {
            p.spawn((Text::new(""), TextFont { font_size: 13.0, ..default() }, TextColor(Color::srgb(0.85, 0.92, 1.0)), ScannerText));
        });
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

fn star_text(s: &StarProfile) -> (String, u8) {
    let mut lines = vec![format!("SCANNER  -  {}", s.name)];
    lines.push(format!("Type : {}  {}", s.class, s.spectral_type.clone().unwrap_or_default()));
    let mut l = format!("Temperature {:.0} K", s.temperature_k);
    if let Some(lum) = s.luminosity_sun {
        l += &format!("   Luminosite {:.3} L sol", lum);
    }
    lines.push(l);
    if let (Some(m), r) = (s.mass_sun, s.radius_sun) {
        lines.push(format!("Masse {:.2} M sol   Rayon {:.3} R sol", m, r));
    }
    if let (Some(age), Some(act)) = (s.age_gyr, s.magnetic_activity) {
        lines.push(format!("Age {:.2} Gyr   Activite {:.2}   Eruptions {}", age, act, s.flares));
    }
    if let (Some(uv), Some(x)) = (s.uv_flux, s.xray_flux) {
        lines.push(format!("UV {:.2}   Rayons X {:.1}  (Soleil = 1)", uv, x));
    }
    let danger = if s.xray_flux.unwrap_or(0.0) > 20.0 { 2 } else { 0 };
    (lines.join("\n"), danger)
}

/// Type secondaire : ce qui distingue ce monde (océan, lave, glace, désert, jardin...).
fn secondary(p: &PlanetProfile) -> String {
    let h = &p.hydrology;
    let ocean = h.ocean_fraction.unwrap_or(0.0);
    let liquid = h.ocean_liquid.as_deref().unwrap_or("aucun");
    let pressure = p.atmosphere.surface_pressure_bar.unwrap_or(0.0);
    let t = p.climate.mean_temperature_c;
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

fn body_text(p: &PlanetProfile) -> (String, u8) {
    let mut lines = vec![format!("SCANNER  -  {}", p.name)];
    let main = match (&p.kind, p.physics.size_class.as_deref()) {
        (crate::planetgen::profile::BodyKind::Moon, _) => "lune".to_string(),
        (_, Some(class)) => class.to_string(),
        _ => "planete".to_string(),
    };
    lines.push(format!("Type : {main}  -  {}", secondary(p)));
    let pressure = match p.atmosphere.surface_pressure_bar {
        Some(b) if b >= 0.01 => format!("{b:.2} bar"),
        Some(b) if b > 0.0 => format!("{:.1} mbar", b * 1000.0),
        _ => "vide".into(),
    };
    lines.push(format!("Gravite {:.2} g   Pression {pressure}", p.physics.surface_gravity_g));
    let mut t = format!("Temperature {:.0} C", p.climate.mean_temperature_c);
    if let (Some(e), Some(po)) = (p.climate.equator_c, p.climate.pole_c) {
        t += &format!("  (equateur {e:.0}, poles {po:.0})");
    }
    lines.push(t);
    if p.atmosphere.gases.is_empty() {
        lines.push("Atmosphere : aucune".into());
    } else {
        let gases: Vec<String> = p
            .atmosphere
            .gases
            .iter()
            .take(4)
            .map(|g| format!("{} {}{}", g.formula, pct(g.fraction), realism_mark(g.realism)))
            .collect();
        lines.push(format!("Atmosphere : {}", gases.join(", ")));
        lines.push(format!("Nuages : {}", p.atmosphere.clouds));
    }
    let h = &p.hydrology;
    let water = h.water_state.clone().unwrap_or_else(|| "?".into());
    let mut w = format!("Eau : {water}");
    if let (Some(o), Some(liq)) = (h.ocean_fraction, h.ocean_liquid.as_deref()) {
        if o > 0.0 {
            w += &format!(", mers {} ({liq})", pct(o));
        }
    }
    if p.gameplay.subsurface_ocean {
        w += ", ocean sous la glace";
    }
    lines.push(w);
    if let Some(life) = &p.biology.life {
        let mut l = format!("Vie : {life}");
        if let Some(chem) = &p.biology.biochemistry {
            l += &format!("  -  {chem}");
        }
        if let Some(f) = &p.biology.fauna {
            l += &format!("\nFaune : ~{} especes, jusqu'a {:.0} m ({})", f.species, f.max_size_m, f.locomotion.join(", "));
        }
        lines.push(l);
    }
    if let Some(score) = p.gameplay.habitability {
        lines.push(format!("Habitabilite : {:.2}  {}", score, p.gameplay.habitability_label.clone().unwrap_or_default()));
    }
    if !p.gameplay.hazards.is_empty() {
        lines.push(format!("Dangers : {}", p.gameplay.hazards.join(", ")));
    }
    let mut extra = Vec::new();
    if p.gameplay.ring.is_some() {
        extra.push("anneaux".to_string());
    }
    if let Some(t) = p.gameplay.tidal_heating.filter(|t| *t > 0.05) {
        extra.push(format!("marees {t:.2}"));
    }
    if !extra.is_empty() {
        lines.push(extra.join("   "));
    }
    if !p.resources.ores.is_empty() {
        let list: Vec<String> = p
            .resources
            .ores
            .iter()
            .take(5)
            .map(|o| format!("{}{} ({})", o.ore, realism_mark(o.realism), crate::planetgen::profile::difficulty_label(o.difficulty)))
            .collect();
        lines.push(format!("Ressources : {}", list.join(", ")));
    }
    let traits: Vec<String> = p.traits.iter().map(|t| format!("{}{}", t.name, realism_mark(t.realism))).collect();
    if !traits.is_empty() {
        lines.push(format!("Traits : {}", traits.join(", ")));
    }
    if let Some(b) = p.biology.biomes.first() {
        let list: Vec<String> = p.biology.biomes.iter().take(3).map(|x| format!("{} {}", x.name, pct(x.fraction))).collect();
        let _ = b;
        lines.push(format!("Biomes : {}", list.join(", ")));
    }
    lines.push("(I : masquer)".into());
    (lines.join("\n"), p.gameplay.danger_level)
}

#[allow(clippy::too_many_arguments)]
fn update_scanner(
    keys: Res<ButtonInput<KeyCode>>,
    panel_ui: Res<NetPanel>,
    menu: Res<MenuState>,
    settings: Res<GameSettings>,
    cache: Res<ProfileCache>,
    target: Res<CameraTarget>,
    weather: Res<crate::world_clock::LocalWeather>,
    star_q: Query<&StarId, With<StarRoot>>,
    mut scanner: ResMut<Scanner>,
    mut panel: Query<&mut Visibility, With<ScannerPanel>>,
    mut text: Query<(&mut Text, &mut TextColor), With<ScannerText>>,
) {
    if keys.just_pressed(SCANNER_KEY) && panel_ui.focus.is_none() && !menu.open {
        scanner.visible = !scanner.visible;
    }
    let loaded = matches!(target.0, crate::ui::TargetKind::Star(id) if star_q.iter().any(|s| s.0 == id));
    let id = target_body(&target.0, loaded);
    let key = id.map(|i| i.key());
    // Recalcul seulement quand la cible change, ou quand le système chargé change (profils en cache)
    if key != scanner.shown || cache.is_changed() {
        scanner.shown = key.clone();
        (scanner.text, scanner.danger) = match id.and_then(|i| profile_of(&settings, &cache, i)) {
            Some(Profile::Star(s)) => star_text(&s),
            Some(Profile::Body(b)) => body_text(&b),
            None => (String::new(), 0),
        };
    }
    // Heure, saison et températures du jour et de l'année, en direct (0.11)
    let live = if weather.body.is_some() && weather.body == Some(target.0) { weather.scanner_line() } else { String::new() };
    let full = match scanner.text.rsplit_once('\n') {
        Some((head, tail)) if !live.is_empty() => format!("{head}\n{live}\n{tail}"),
        _ => scanner.text.clone(),
    };
    let show = scanner.visible && !scanner.text.is_empty();
    for mut v in &mut panel {
        let wanted = if show { Visibility::Inherited } else { Visibility::Hidden };
        if *v != wanted {
            *v = wanted;
        }
    }
    let color = match scanner.danger {
        3 => Color::srgb(1.0, 0.6, 0.55),
        2 => Color::srgb(1.0, 0.85, 0.5),
        _ => Color::srgb(0.85, 0.92, 1.0),
    };
    for (mut t, mut c) in &mut text {
        if t.0 != full {
            t.0 = full.clone();
        }
        if c.0 != color {
            c.0 = color;
        }
    }
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
                Profile::Star(s) => star_text(&s),
                Profile::Body(b) => {
                    let (t, d) = body_text(&b);
                    for field in ["Type :", "Gravite", "Pression", "Temperature", "Atmosphere", "Eau :", "Habitabilite"] {
                        assert!(t.contains(field), "{field} absent :\n{t}");
                    }
                    (t, d)
                }
            };
            assert!(text.starts_with("SCANNER"), "{text}");
            assert!(danger <= 3);
        }
    }
}
