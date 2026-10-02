// ─────────────────────────────────────────────────────────────────────────
//  Statistiques d'une galaxie : combien d'étoiles, de planètes, de lunes de chaque sorte
//
//    /stats            : la galaxie où l'on est
//    /stats <n>        : la galaxie n (0 = la nôtre)
//    /stats tout       : toutes les galaxies (plus long)
//    F3                : masquer / réafficher le panneau
//
//  Le calcul tourne en arrière-plan (les planètes sont recalculées, sans remplir le cache) ;
//  le résultat s'affiche dans un panneau et s'écrit dans `saves/vX.Y.Z/stats/`.
// ─────────────────────────────────────────────────────────────────────────

use bevy::prelude::*;
use bevy::tasks::{block_on, futures_lite::future, AsyncComputeTaskPool, Task};
use std::collections::BTreeMap;

use crate::net::Net;
use crate::planet::SpawnedSystems;
use crate::planetgen::geology::Tectonics;
use crate::planetgen::hydrology::{Liquid, WaterState};
use crate::planetgen::life::LifeLevel;
use crate::planetgen::star::StarClass;
use crate::planetgen::system::{size_class, PlanetKind};
use crate::settings::{data_dir, GameSettings, StarSystemConfig};

pub const STATS_KEY: KeyCode = KeyCode::F3;

pub struct StatsPlugin;

impl Plugin for StatsPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<StatsCommand>()
            .init_resource::<StatsPanel>()
            .add_systems(Startup, setup_panel)
            .add_systems(Update, (run_stats_commands, finish_stats, show_panel).chain());
    }
}

/// `/stats ...` tapé dans le chat (l'argument).
#[derive(Event)]
pub struct StatsCommand(pub String);

#[derive(Resource, Default)]
struct StatsPanel {
    visible: bool,
    text: String,
    task: Option<(String, Task<String>)>,
}

#[derive(Component)]
struct StatsRoot;

#[derive(Component)]
struct StatsText;

/// Compteur par catégorie, dans l'ordre d'apparition des catégories.
#[derive(Default)]
struct Tally {
    total: usize,
    counts: BTreeMap<(usize, String), usize>,
    order: Vec<String>,
}

impl Tally {
    fn add(&mut self, key: &str) {
        let i = self.order.iter().position(|k| k == key).unwrap_or_else(|| {
            self.order.push(key.to_string());
            self.order.len() - 1
        });
        *self.counts.entry((i, key.to_string())).or_insert(0) += 1;
    }

    fn line(&self, title: &str, total: usize) -> String {
        let parts: Vec<String> = self.counts.iter().map(|((_, k), n)| format!("{k} {n} ({})", pct(*n, total))).collect();
        format!("{title} : {}", parts.join(", "))
    }
}

fn pct(n: usize, total: usize) -> String {
    if total == 0 {
        return "0 %".into();
    }
    let p = n as f64 * 100.0 / total as f64;
    if p >= 10.0 {
        format!("{p:.0} %")
    } else if p >= 0.1 {
        format!("{p:.1} %")
    } else {
        format!("{p:.2} %")
    }
}

/// Statistiques de tous les astres de ces systèmes.
pub fn report(title: &str, systems: &[StarSystemConfig]) -> String {
    let mut stars = Tally::default();
    let (mut kinds, mut sizes, mut liquids, mut life, mut tecto, mut hab, mut temps, mut traits) =
        (Tally::default(), Tally::default(), Tally::default(), Tally::default(), Tally::default(), Tally::default(), Tally::default(), Tally::default());
    let (mut moon_flags, mut planet_flags) = (Tally::default(), Tally::default());
    let (mut planets, mut rocky, mut moons, mut per_system) = (0usize, 0usize, 0usize, [0usize; 9]);
    // Ordre d'affichage logique (du plus chaud au plus froid, du plus petit au plus grand...)
    stars.order = StarClass::ALL.iter().map(|c| c.name().to_string()).collect();
    let set = |t: &mut Tally, keys: &[&str]| t.order = keys.iter().map(|k| k.to_string()).collect();
    set(&mut kinds, &["rocheuses", "mini-Neptunes", "geantes de glace", "geantes gazeuses", "Jupiter chauds"]);
    set(&mut sizes, &["minuscule", "petite", "terrestre", "super-Terre"]);
    set(&mut temps, &["brulantes (>100 C)", "chaudes (45-100 C)", "temperees (-10-45 C)", "froides (-80 a -10 C)", "glacees (<-80 C)"]);
    set(&mut liquids, &["eau liquide", "eau gelee", "methane", "ammoniac", "lave", "sans mers"]);
    set(&mut tecto, &["plaques", "une seule plaque", "inactive"]);
    set(&mut life, &[LifeLevel::Complex.name(), LifeLevel::Simple.name(), LifeLevel::Microbial.name(), LifeLevel::None.name()]);
    set(&mut hab, &["habitable", "vivable avec equipement", "hostile", "inhabitable"]);
    set(&mut traits, &["peu communs", "rares", "legendaires"]);
    for sys in systems {
        stars.total += 1;
        if let Some(s) = sys.stars.first() {
            stars.add(s.class.name());
        }
        let list = sys.planets_uncached();
        per_system[list.len().min(8)] += 1;
        for p in list.iter() {
            planets += 1;
            kinds.add(match p.kind {
                PlanetKind::Rocky => "rocheuses",
                PlanetKind::MiniNeptune => "mini-Neptunes",
                PlanetKind::IceGiant => "geantes de glace",
                PlanetKind::GasGiant if p.hot => "Jupiter chauds",
                PlanetKind::GasGiant => "geantes gazeuses",
            });
            for (flag, on) in [
                ("anneaux", p.ring.is_some()),
                ("aurores", p.aurora.is_some()),
                ("rotation synchrone", p.tidally_locked),
                ("avec atmosphere", p.atmosphere),
                ("avec oxygene", p.air.fraction("O2") > 0.05),
            ] {
                if on {
                    planet_flags.add(flag);
                }
            }
            for t in &p.traits {
                if t.rarity < 0.9 {
                    traits.add(match t.rarity {
                        r if r <= 0.001 => "legendaires",
                        r if r <= 0.004 => "rares",
                        _ => "peu communs",
                    });
                }
            }
            if p.gaseous() {
                continue;
            }
            rocky += 1;
            sizes.add(size_class(p.kind, p.radius_earth as f64, p.hot));
            let h = &p.hydrology;
            liquids.add(match (h.hydro.liquid, h.water_state) {
                (Liquid::Water, WaterState::Liquid) => "eau liquide",
                (Liquid::Water, _) => "eau gelee",
                (Liquid::Lava, _) => "lave",
                (Liquid::Methane, _) => "methane",
                (Liquid::Ammonia, _) => "ammoniac",
                (Liquid::None, _) => "sans mers",
            });
            let t = p.temperature();
            temps.add(match t {
                t if t > 100.0 => "brulantes (>100 C)",
                t if t > 45.0 => "chaudes (45-100 C)",
                t if t > -10.0 => "temperees (-10-45 C)",
                t if t > -80.0 => "froides (-80 a -10 C)",
                _ => "glacees (<-80 C)",
            });
            life.add(p.life.level.name());
            if p.life.flora {
                planet_flags.add("avec plantes");
            }
            tecto.add(match p.geology.tectonics {
                Tectonics::Plates => "plaques",
                Tectonics::StagnantLid => "une seule plaque",
                Tectonics::Inactive => "inactive",
            });
            hab.add(&p.habitability.label);
        }
        for p in list.iter() {
            for m in &p.moons {
                moons += 1;
                for (flag, on) in [
                    ("avec air", m.atmosphere),
                    ("volcaniques (marees)", m.tidal_heat > 0.2 && m.geology.volcanism > 0.3),
                    ("ocean sous la glace", m.hydrology.subsurface_ocean),
                    ("avec vie", m.life.level >= LifeLevel::Microbial),
                    ("avec plantes", m.life.flora),
                ] {
                    if on {
                        moon_flags.add(flag);
                    }
                }
            }
        }
    }
    let n = systems.len();
    let counts: Vec<String> = (1..=8).filter(|&k| per_system[k] > 0).map(|k| format!("{k}: {}", pct(per_system[k], n))).collect();
    let mut lines = vec![
        format!("STATISTIQUES  -  {title}   (F3 : masquer)"),
        format!("{n} systemes, {planets} planetes ({:.1} par systeme), {moons} lunes", planets as f64 / n.max(1) as f64),
        format!("Planetes par systeme : {}", counts.join(", ")),
        String::new(),
        stars.line("ETOILES", n),
        String::new(),
        kinds.line("PLANETES", planets),
        planet_flags.line("Particularites", planets),
        traits.line("Traits tires", planets),
        String::new(),
        format!("PLANETES ROCHEUSES ({rocky})"),
        sizes.line("Tailles", rocky),
        temps.line("Temperatures", rocky),
        liquids.line("Mers", rocky),
        tecto.line("Tectonique", rocky),
        life.line("Vie", rocky),
        hab.line("Habitabilite", rocky),
        String::new(),
        moon_flags.line(&format!("LUNES ({moons})"), moons),
    ];
    lines.retain(|l| !l.ends_with(" : "));
    lines.join("\n")
}

fn setup_panel(mut commands: Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(12.0),
                top: Val::Px(150.0),
                width: Val::Px(620.0),
                padding: UiRect::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.03, 0.06, 0.82)),
            BorderRadius::all(Val::Px(6.0)),
            Visibility::Hidden,
            StatsRoot,
        ))
        .with_children(|p| {
            p.spawn((Text::new(""), TextFont { font_size: 12.0, ..default() }, TextColor(Color::srgb(0.85, 0.92, 1.0)), StatsText));
        });
}

fn run_stats_commands(
    time: Res<Time>,
    mut events: EventReader<StatsCommand>,
    settings: Res<GameSettings>,
    spawned: Res<SpawnedSystems>,
    mut panel: ResMut<StatsPanel>,
    mut net: ResMut<Net>,
) {
    let now = time.elapsed_secs_f64();
    for StatsCommand(arg) in events.read() {
        let arg = arg.trim().to_lowercase();
        let here = spawned.0.iter().next().and_then(|&si| settings.systems.get(si)).map_or(0, |s| s.galaxy_id);
        let galaxies = settings.galaxies.len() as u32;
        let (title, filter): (String, Option<u32>) = if arg.is_empty() {
            (format!("galaxie {here} ({})", settings.galaxies.get(here as usize).map_or("?", |g| g.kind.name())), Some(here))
        } else if matches!(arg.as_str(), "tout" | "toutes" | "all") {
            ("toutes les galaxies".to_string(), None)
        } else if let Ok(g) = arg.parse::<u32>() {
            if g >= galaxies {
                net.notify(&format!("Il n'y a pas de galaxie {g} : choisissez de 0 a {}.", galaxies - 1), now);
                continue;
            }
            (format!("galaxie {g} ({})", settings.galaxies[g as usize].kind.name()), Some(g))
        } else {
            net.notify("/stats : galaxie actuelle ; /stats <n> : galaxie n ; /stats tout : tout l'univers. F3 : masquer.", now);
            continue;
        };
        let systems: Vec<StarSystemConfig> = settings.systems.iter().filter(|s| filter.map_or(true, |g| s.galaxy_id == g)).cloned().collect();
        net.notify(&format!("Statistiques de {title} : {} systemes, calcul en cours...", systems.len()), now);
        let t = title.clone();
        let task = AsyncComputeTaskPool::get().spawn(async move { report(&t, &systems) });
        panel.task = Some((title, task));
    }
}

fn finish_stats(time: Res<Time>, mut panel: ResMut<StatsPanel>, mut net: ResMut<Net>) {
    let Some((_, task)) = panel.task.as_mut() else { return };
    let Some(text) = block_on(future::poll_once(task)) else { return };
    let (title, _) = panel.task.take().unwrap();
    // Aussi dans un fichier, pour comparer
    let dir = data_dir().join("stats");
    let name: String = title.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    let path = dir.join(format!("{name}.txt"));
    let saved = std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&path, &text)).is_ok();
    panel.text = text;
    panel.visible = true;
    let now = time.elapsed_secs_f64();
    if saved {
        net.notify(&format!("Statistiques pretes (F3 : masquer), aussi dans {}.", path.display()), now);
    }
}

fn show_panel(
    keys: Res<ButtonInput<KeyCode>>,
    net_panel: Res<crate::net_ui::NetPanel>,
    mut panel: ResMut<StatsPanel>,
    mut root: Query<&mut Visibility, With<StatsRoot>>,
    mut text: Query<&mut Text, With<StatsText>>,
) {
    if keys.just_pressed(STATS_KEY) && net_panel.focus.is_none() && !panel.text.is_empty() {
        panel.visible = !panel.visible;
    }
    let wanted = if panel.visible && !panel.text.is_empty() { Visibility::Inherited } else { Visibility::Hidden };
    for mut v in &mut root {
        if *v != wanted {
            *v = wanted;
        }
    }
    if panel.is_changed() {
        for mut t in &mut text {
            if t.0 != panel.text {
                t.0 = panel.text.clone();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn galaxy_report_counts_everything() {
        let settings = GameSettings::default();
        let systems: Vec<StarSystemConfig> = settings.systems.iter().filter(|s| s.galaxy_id == 0).cloned().collect();
        let start = std::time::Instant::now();
        let text = report("galaxie 0", &systems);
        let secs = start.elapsed().as_secs_f64();
        for part in ["ETOILES", "naine rouge (M)", "PLANETES", "rocheuses", "Mers", "Vie", "Habitabilite", "LUNES", "%"] {
            assert!(text.contains(part), "{part} absent :\n{text}");
        }
        assert!(text.contains(&format!("{} systemes", systems.len())));
        // Les étoiles somment à 100 % (à l'arrondi près) : chaque système compte une étoile
        let star_line = text.lines().find(|l| l.starts_with("ETOILES")).unwrap();
        let total: usize = star_line.split(", ").filter_map(|p| p.split_whitespace().rev().nth(2)?.parse::<usize>().ok()).sum();
        assert_eq!(total, systems.len(), "{star_line}");
        println!("{text}\n({secs:.2} s)");
    }
}
