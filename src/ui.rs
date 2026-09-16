use bevy::input::mouse::MouseWheel;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowRef};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::planet::RegeneratePlanet;
use crate::settings::{AsteroidBeltConfig, GameSettings, PlanetConfig, StarConfig};
use crate::astre::planete::gas_planet::{GasPlanetRes, RegenerateGasPlanet};

#[allow(non_snake_case)]
use crate::astre::Remnant_stellaire::black_hole::{
    BlackHoleRes, RegenerateBlackHole,
};

#[allow(non_snake_case)]
use crate::astre::Remnant_stellaire::nebula::{
    NebulaRes, RegenerateNebula,
};
#[allow(non_snake_case)]
use crate::astre::Remnant_stellaire::pulsar::{PulsarRes, RegeneratePulsar};
#[allow(non_snake_case)]
use crate::astre::Remnant_stellaire::magnetar::{MagnetarRes, RegenerateMagnetar};
#[allow(non_snake_case)]
use crate::astre::Remnant_stellaire::neutron_star::{NeutronStarRes, RegenerateNeutronStar};
#[allow(non_snake_case)]
use crate::astre::Remnant_stellaire::supernova::{SupernovaRes, RegenerateSupernova};
use crate::astre::planete::comet::{CometRes, RegenerateComet};
use crate::astre::planete::meteoroid::{MeteoroidRes, RegenerateMeteoroid};
use crate::astre::etoile::star::{StarRes as VoxelStarRes, RegenerateStar as RegenerateVoxelStar};
use crate::astre::etoile::protostar::{ProtostarRes, RegenerateProtostar};
use crate::astre::etoile::dwarf_star::{DwarfStarRes, RegenerateDwarfStar};
use crate::astre::etoile::main_sequence_star::{MainSequenceRes, RegenerateMainSequence};
use crate::astre::etoile::giant_star::{GiantStarRes, RegenerateGiantStar};
use crate::astre::etoile::supergiant_star::{SupergiantRes, RegenerateSupergiant};
use crate::astre::etoile::hypergiant_star::{HypergiantRes, RegenerateHypergiant};
#[derive(Component)]
struct AstresWindowCam;

#[derive(Component)]
struct AstresUiRoot;

#[derive(Resource, Clone, Copy, PartialEq, Eq)]
enum AstresTab {
    Planets,
    Stars,
    Remnants,
}

#[derive(Component, Clone, Copy)]
struct AstresTabButton(AstresTab);

#[derive(Resource)]
struct AstresTabState {
    active: AstresTab,
}

#[derive(Component, Clone, Copy)]
enum RegenAstreAction {
    // Planètes
    GasPlanet,
    Comet,
    Meteoroid,
    // Étoiles
    VoxelStar,
    Protostar,
    DwarfStar,
    MainSequence,
    GiantStar,
    Supergiant,
    Hypergiant,
    // Rémanents
    Nebula,
    BlackHole,
    Pulsar,
    Magnetar,
    NeutronStar,
    Supernova,
}

#[derive(Clone, Copy)]
enum AstreFamily {
    GasPlanet,
    Comet,
    Meteoroid,
    VoxelStar,
    Protostar,
    DwarfStar,
    MainSequence,
    GiantStar,
    Supergiant,
    Hypergiant,
    Nebula,
    BlackHole,
    Pulsar,
    Magnetar,
    NeutronStar,
    Supernova,
}

#[derive(Component, Clone, Copy)]
enum AstreEditAction {
    Add(AstreFamily),
    Remove(TargetKind),
    AdjustOrbit(TargetKind, f32),
    AdjustSize(TargetKind, f32),
    AdjustComa(TargetKind, f32),
    AdjustDustTail(TargetKind, f32),
}

#[derive(Serialize, Deserialize, Clone, Copy, Default)]
struct SavedAstre {
    orbit: f32,
    size: f32,
}

#[derive(Serialize, Deserialize, Default)]
struct SavedAstres {
    gas_planets: Vec<SavedAstre>,
    comets: Vec<SavedAstre>,
    meteoroids: Vec<SavedAstre>,
    voxel_stars: Vec<SavedAstre>,
    protostars: Vec<SavedAstre>,
    dwarf_stars: Vec<SavedAstre>,
    main_sequence: Vec<SavedAstre>,
    giants: Vec<SavedAstre>,
    supergiants: Vec<SavedAstre>,
    hypergiants: Vec<SavedAstre>,
    black_holes: Vec<SavedAstre>,
    pulsars: Vec<SavedAstre>,
    magnetars: Vec<SavedAstre>,
    neutron_stars: Vec<SavedAstre>,
    supernovae: Vec<SavedAstre>,
    #[serde(default = "default_nebula_enabled")]
    nebula_enabled: bool,
    nebula: SavedAstre,
}

fn default_nebula_enabled() -> bool { true }
pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app
            .insert_resource(MenuState { open: false })
            .insert_resource(CameraTarget(TargetKind::Planet(0)))
            .insert_resource(OptionsTabState { active: OptionsTab::General })
            .insert_resource(AstresTabState { active: AstresTab::Planets })
            .add_event::<RebuildUi>()
            .add_systems(PreStartup, load_saved_astres)
            .add_systems(
                Startup,
                (setup_game_ui, setup_options_window, setup_astres_window),
            )
            .add_systems(
                Update,
                (
                    toggle_menu,
                    handle_options_button,
                    handle_options_tabs,
                    handle_astres_tabs,
                    update_menu_visibility,
                    handle_slider_interactions,
                    handle_toggle_button,
                    handle_toggle_show_light,
                    handle_toggle_show_orbits,
                    handle_toggle_atmosphere,
                    handle_apply_button,
                    handle_center_buttons,
                    update_slider_visuals,
                    handle_body_actions,
                    rebuild_options_ui,
                    rebuild_astres_ui,
                    scroll_options_panel,
                    handle_astre_regen_buttons,
                    handle_astre_edit_actions,
                ),
            );
    }
}

#[derive(SystemParam)]
pub struct AstresResources<'w> {
    pub gas_res:      Res<'w, GasPlanetRes>,
    pub comet_res:    Res<'w, CometRes>,
    pub meteor_res:   Res<'w, MeteoroidRes>,
    pub vstar_res:    Res<'w, VoxelStarRes>,
    pub proto_res:    Res<'w, ProtostarRes>,
    pub dwarf_res:    Res<'w, DwarfStarRes>,
    pub ms_res:       Res<'w, MainSequenceRes>,
    pub giant_res:    Res<'w, GiantStarRes>,
    pub sg_res:       Res<'w, SupergiantRes>,
    pub hg_res:       Res<'w, HypergiantRes>,
    pub nebula_res:   Res<'w, NebulaRes>,
    pub black_res:    Res<'w, BlackHoleRes>,
    pub pulsar_res:   Res<'w, PulsarRes>,
    pub magnetar_res: Res<'w, MagnetarRes>,
    pub neutron_res:  Res<'w, NeutronStarRes>,
    pub sn_res:       Res<'w, SupernovaRes>,
}

#[derive(SystemParam)]
pub struct AstresMutableResources<'w> {
    pub gas_res:      ResMut<'w, GasPlanetRes>,
    pub comet_res:    ResMut<'w, CometRes>,
    pub meteor_res:   ResMut<'w, MeteoroidRes>,
    pub vstar_res:    ResMut<'w, VoxelStarRes>,
    pub proto_res:    ResMut<'w, ProtostarRes>,
    pub dwarf_res:    ResMut<'w, DwarfStarRes>,
    pub ms_res:       ResMut<'w, MainSequenceRes>,
    pub giant_res:    ResMut<'w, GiantStarRes>,
    pub sg_res:       ResMut<'w, SupergiantRes>,
    pub hg_res:       ResMut<'w, HypergiantRes>,
    pub nebula_res:   ResMut<'w, NebulaRes>,
    pub black_res:    ResMut<'w, BlackHoleRes>,
    pub pulsar_res:   ResMut<'w, PulsarRes>,
    pub magnetar_res: ResMut<'w, MagnetarRes>,
    pub neutron_res:  ResMut<'w, NeutronStarRes>,
    pub sn_res:       ResMut<'w, SupernovaRes>,
}

#[derive(SystemParam)]
pub struct RegenEvents<'w> {
    pub gas_events:      EventWriter<'w, RegenerateGasPlanet>,
    pub comet_events:    EventWriter<'w, RegenerateComet>,
    pub meteor_events:   EventWriter<'w, RegenerateMeteoroid>,
    pub vstar_events:    EventWriter<'w, RegenerateVoxelStar>,
    pub proto_events:    EventWriter<'w, RegenerateProtostar>,
    pub dwarf_events:    EventWriter<'w, RegenerateDwarfStar>,
    pub ms_events:       EventWriter<'w, RegenerateMainSequence>,
    pub giant_events:    EventWriter<'w, RegenerateGiantStar>,
    pub sg_events:       EventWriter<'w, RegenerateSupergiant>,
    pub hg_events:       EventWriter<'w, RegenerateHypergiant>,
    pub nebula_events:   EventWriter<'w, RegenerateNebula>,
    pub black_events:    EventWriter<'w, RegenerateBlackHole>,
    pub pulsar_events:   EventWriter<'w, RegeneratePulsar>,
    pub mag_events:      EventWriter<'w, RegenerateMagnetar>,
    pub neutron_events:  EventWriter<'w, RegenerateNeutronStar>,
    pub sn_events:       EventWriter<'w, RegenerateSupernova>,
}

#[derive(Resource)]
pub struct MenuState {
    pub open: bool,
}

#[derive(Resource, Clone, Copy, PartialEq, Eq)]
enum OptionsTab {
    General,
    Planets,
    Stars,
    Belts,
}

#[derive(Component, Clone, Copy)]
struct OptionsTabButton(OptionsTab);

#[derive(Resource)]
struct OptionsTabState {
    active: OptionsTab,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TargetKind {
    // ── Existants ──────────────────────────────────────────────────────────
    Planet(usize),
    Moon(usize, usize),
    Star(usize),
    GasPlanet(usize),
    Nebula,
    BlackHole(usize),
    // ── Planètes & corps ───────────────────────────────────────────────────
    Comet(usize),
    Meteoroid(usize),
    // ── Étoiles ────────────────────────────────────────────────────────────
    VoxelStar(usize),
    Protostar(usize),
    DwarfStar(usize),
    MainSequence(usize),
    GiantStar(usize),
    Supergiant(usize),
    Hypergiant(usize),
    // ── Rémanents stellaires ───────────────────────────────────────────────
    Pulsar(usize),
    Magnetar(usize),
    NeutronStar(usize),
    Supernova(usize),
}

#[derive(Resource)]
pub struct CameraTarget(pub TargetKind);

#[derive(Event)]
struct RebuildUi;

#[derive(Component)]
struct CenterButton(TargetKind);

#[derive(Component)]
struct CenterButtonBar;

#[derive(Component)]
struct OptionsButton;

#[derive(Component)]
struct MenuRoot;

#[derive(Component)]
struct OptionsWindowCam;

#[derive(Component)]
struct OptionsUiRoot;

#[derive(Component)]
struct SliderBar {
    setting: SettingKey,
    min: f32,
    max: f32,
}

#[derive(Component)]
struct SliderFill(SettingKey);

#[derive(Component)]
struct SliderLabel(SettingKey);

#[derive(Component)]
struct ToggleInvertY;

#[derive(Component)]
struct ToggleShowLight;

#[derive(Component)]
struct ToggleShowOrbits;

#[derive(Component)]
struct ToggleAtmosphere(usize);

#[derive(Component)]
struct ApplyPlanetButton;

#[derive(Component, Clone, Copy)]
enum BodyAction {
    AddPlanet,
    RemovePlanet(usize),
    AddStar,
    RemoveStar(usize),
    AddBelt,
    RemoveBelt(usize),
    AddMoon(usize),
    RemoveMoon(usize, usize),
}

#[derive(Clone, Copy, PartialEq)]
enum SettingKey {
    MouseSensitivity,
    ScrollSpeed,
    KeyboardSpeed,
    PlanetOrbitDistance(usize),
    PlanetRadius(usize),
    PlanetSeaLevel(usize),
    PlanetTerrainHeight(usize),
    PlanetSeed(usize),
    PlanetNoiseScale(usize),
    PlanetDetailScale(usize),
    StarOrbitDistance(usize),
    StarRadius(usize),
    StarIntensity(usize),
    StarLightRange(usize),
    StarColorR(usize),
    StarColorG(usize),
    StarColorB(usize),
    StarFlareCount(usize),
    StarFlareHeight(usize),
    StarFlareSpeed(usize),
    StarFlareSize(usize),
    StarFlareDistance(usize),
    BeltDistance(usize),
    BeltWidth(usize),
    BeltMinSize(usize),
    BeltMaxSize(usize),
    BeltCount(usize),
    PlanetCloudDensity(usize),
    PlanetCloudAltitude(usize),
    PlanetCloudSpeed(usize),
    MoonOrbitDistance(usize, usize),
    MoonRadius(usize, usize),
    MoonSeed(usize, usize),
}

impl SettingKey {
    fn get(self, s: &GameSettings) -> f32 {
        match self {
            Self::MouseSensitivity => s.mouse_sensitivity,
            Self::ScrollSpeed => s.scroll_speed,
            Self::KeyboardSpeed => s.keyboard_speed,
            Self::PlanetOrbitDistance(i) => s.planets.get(i).map(|p| p.orbit_distance).unwrap_or(0.0),
            Self::PlanetRadius(i) => s.planets.get(i).map(|p| p.radius).unwrap_or(50.0),
            Self::PlanetSeaLevel(i) => s.planets.get(i).map(|p| p.sea_level).unwrap_or(0.4),
            Self::PlanetTerrainHeight(i) => s.planets.get(i).map(|p| p.terrain_height).unwrap_or(22.0),
            Self::PlanetSeed(i) => s.planets.get(i).map(|p| p.seed as f32).unwrap_or(42.0),
            Self::PlanetNoiseScale(i) => s.planets.get(i).map(|p| p.noise_scale).unwrap_or(2.0),
            Self::PlanetDetailScale(i) => s.planets.get(i).map(|p| p.detail_scale).unwrap_or(4.0),
            Self::PlanetCloudDensity(i) => s.planets.get(i).map(|p| p.cloud_density).unwrap_or(0.5),
            Self::PlanetCloudAltitude(i) => s.planets.get(i).map(|p| p.cloud_altitude).unwrap_or(8.0),
            Self::PlanetCloudSpeed(i) => s.planets.get(i).map(|p| p.cloud_speed).unwrap_or(0.02),
            Self::StarOrbitDistance(i) => s.stars.get(i).map(|st| st.orbit_distance).unwrap_or(0.0),
            Self::StarRadius(i) => s.stars.get(i).map(|st| st.radius).unwrap_or(200.0),
            Self::StarIntensity(i) => s.stars.get(i).map(|st| st.intensity).unwrap_or(20.0),
            Self::StarLightRange(i) => s.stars.get(i).map(|st| st.light_range).unwrap_or(10000.0),
            Self::StarColorR(i) => s.stars.get(i).map(|st| st.light_color_r).unwrap_or(1.0),
            Self::StarColorG(i) => s.stars.get(i).map(|st| st.light_color_g).unwrap_or(0.92),
            Self::StarColorB(i) => s.stars.get(i).map(|st| st.light_color_b).unwrap_or(0.65),
            Self::StarFlareCount(i) => s.stars.get(i).map(|st| st.flare_count as f32).unwrap_or(5.0),
            Self::StarFlareHeight(i) => s.stars.get(i).map(|st| st.flare_height).unwrap_or(60.0),
            Self::StarFlareSpeed(i) => s.stars.get(i).map(|st| st.flare_speed).unwrap_or(1.0),
            Self::StarFlareSize(i) => s.stars.get(i).map(|st| st.flare_size).unwrap_or(6.0),
            Self::StarFlareDistance(i) => s.stars.get(i).map(|st| st.flare_distance).unwrap_or(0.0),
            Self::BeltDistance(i) => s.asteroid_belts.get(i).map(|b| b.distance).unwrap_or(300.0),
            Self::BeltWidth(i) => s.asteroid_belts.get(i).map(|b| b.width).unwrap_or(80.0),
            Self::BeltMinSize(i) => s.asteroid_belts.get(i).map(|b| b.min_size).unwrap_or(1.0),
            Self::BeltMaxSize(i) => s.asteroid_belts.get(i).map(|b| b.max_size).unwrap_or(5.0),
            Self::BeltCount(i) => s.asteroid_belts.get(i).map(|b| b.count as f32).unwrap_or(100.0),
            Self::MoonOrbitDistance(pi, mi) => s.planets.get(pi).and_then(|p| p.moons.get(mi)).map(|m| m.orbit_distance).unwrap_or(80.0),
            Self::MoonRadius(pi, mi) => s.planets.get(pi).and_then(|p| p.moons.get(mi)).map(|m| m.radius).unwrap_or(12.0),
            Self::MoonSeed(pi, mi) => s.planets.get(pi).and_then(|p| p.moons.get(mi)).map(|m| m.seed as f32).unwrap_or(77.0),
        }
    }

    fn set(self, s: &mut GameSettings, val: f32) {
        match self {
            Self::MouseSensitivity => s.mouse_sensitivity = val,
            Self::ScrollSpeed => s.scroll_speed = val,
            Self::KeyboardSpeed => s.keyboard_speed = val,
            Self::PlanetOrbitDistance(i) => {
                if let Some(p) = s.planets.get_mut(i) { p.orbit_distance = val; }
            }
            Self::PlanetRadius(i) => {
                if let Some(p) = s.planets.get_mut(i) { p.radius = val; }
            }
            Self::PlanetSeaLevel(i) => {
                if let Some(p) = s.planets.get_mut(i) { p.sea_level = val; }
            }
            Self::PlanetTerrainHeight(i) => {
                if let Some(p) = s.planets.get_mut(i) { p.terrain_height = val; }
            }
            Self::PlanetSeed(i) => {
                if let Some(p) = s.planets.get_mut(i) { p.seed = val as u32; }
            }
            Self::PlanetNoiseScale(i) => {
                if let Some(p) = s.planets.get_mut(i) { p.noise_scale = val; }
            }
            Self::PlanetDetailScale(i) => {
                if let Some(p) = s.planets.get_mut(i) { p.detail_scale = val; }
            }
            Self::PlanetCloudDensity(i) => {
                if let Some(p) = s.planets.get_mut(i) { p.cloud_density = val; }
            }
            Self::PlanetCloudAltitude(i) => {
                if let Some(p) = s.planets.get_mut(i) { p.cloud_altitude = val; }
            }
            Self::PlanetCloudSpeed(i) => {
                if let Some(p) = s.planets.get_mut(i) { p.cloud_speed = val; }
            }
            Self::StarOrbitDistance(i) => {
                if let Some(st) = s.stars.get_mut(i) { st.orbit_distance = val; }
            }
            Self::StarRadius(i) => {
                if let Some(st) = s.stars.get_mut(i) { st.radius = val; }
            }
            Self::StarIntensity(i) => {
                if let Some(st) = s.stars.get_mut(i) { st.intensity = val; }
            }
            Self::StarLightRange(i) => {
                if let Some(st) = s.stars.get_mut(i) { st.light_range = val; }
            }
            Self::StarColorR(i) => {
                if let Some(st) = s.stars.get_mut(i) { st.light_color_r = val; }
            }
            Self::StarColorG(i) => {
                if let Some(st) = s.stars.get_mut(i) { st.light_color_g = val; }
            }
            Self::StarColorB(i) => {
                if let Some(st) = s.stars.get_mut(i) { st.light_color_b = val; }
            }
            Self::StarFlareCount(i) => {
                if let Some(st) = s.stars.get_mut(i) { st.flare_count = val as u32; }
            }
            Self::StarFlareHeight(i) => {
                if let Some(st) = s.stars.get_mut(i) { st.flare_height = val; }
            }
            Self::StarFlareSpeed(i) => {
                if let Some(st) = s.stars.get_mut(i) { st.flare_speed = val; }
            }
            Self::StarFlareSize(i) => {
                if let Some(st) = s.stars.get_mut(i) { st.flare_size = val; }
            }
            Self::StarFlareDistance(i) => {
                if let Some(st) = s.stars.get_mut(i) { st.flare_distance = val; }
            }
            Self::BeltDistance(i) => {
                if let Some(b) = s.asteroid_belts.get_mut(i) { b.distance = val; }
            }
            Self::BeltWidth(i) => {
                if let Some(b) = s.asteroid_belts.get_mut(i) { b.width = val; }
            }
            Self::BeltMinSize(i) => {
                if let Some(b) = s.asteroid_belts.get_mut(i) { b.min_size = val; }
            }
            Self::BeltMaxSize(i) => {
                if let Some(b) = s.asteroid_belts.get_mut(i) { b.max_size = val; }
            }
            Self::BeltCount(i) => {
                if let Some(b) = s.asteroid_belts.get_mut(i) { b.count = val as u32; }
            }
            Self::MoonOrbitDistance(pi, mi) => {
                if let Some(m) = s.planets.get_mut(pi).and_then(|p| p.moons.get_mut(mi)) { m.orbit_distance = val; }
            }
            Self::MoonRadius(pi, mi) => {
                if let Some(m) = s.planets.get_mut(pi).and_then(|p| p.moons.get_mut(mi)) { m.radius = val; }
            }
            Self::MoonSeed(pi, mi) => {
                if let Some(m) = s.planets.get_mut(pi).and_then(|p| p.moons.get_mut(mi)) { m.seed = val as u32; }
            }
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::MouseSensitivity => "Sensibilite souris",
            Self::ScrollSpeed => "Vitesse zoom",
            Self::KeyboardSpeed => "Vitesse clavier",
            Self::PlanetOrbitDistance(_) => "Distance orbite",
            Self::PlanetRadius(_) => "Rayon",
            Self::PlanetSeaLevel(_) => "Niveau mer",
            Self::PlanetTerrainHeight(_) => "Hauteur terrain",
            Self::PlanetSeed(_) => "Graine",
            Self::PlanetNoiseScale(_) => "Echelle bruit",
            Self::PlanetDetailScale(_) => "Echelle details",
            Self::PlanetCloudDensity(_) => "Densite nuages",
            Self::PlanetCloudAltitude(_) => "Altitude nuages",
            Self::PlanetCloudSpeed(_) => "Vitesse nuages",
            Self::StarOrbitDistance(_) => "Distance orbite",
            Self::StarRadius(_) => "Rayon",
            Self::StarIntensity(_) => "Intensite",
            Self::StarLightRange(_) => "Portee lumiere",
            Self::StarColorR(_) => "Couleur R",
            Self::StarColorG(_) => "Couleur G",
            Self::StarColorB(_) => "Couleur B",
            Self::StarFlareCount(_) => "Eruptions",
            Self::StarFlareHeight(_) => "Hauteur eruptions",
            Self::StarFlareSpeed(_) => "Vitesse eruptions",
            Self::StarFlareSize(_) => "Taille eruptions",
            Self::StarFlareDistance(_) => "Distance eruptions",
            Self::BeltDistance(_) => "Distance",
            Self::BeltWidth(_) => "Ecartement",
            Self::BeltMinSize(_) => "Taille min",
            Self::BeltMaxSize(_) => "Taille max",
            Self::BeltCount(_) => "Nombre",
            Self::MoonOrbitDistance(_, _) => "Distance orbite",
            Self::MoonRadius(_, _) => "Rayon",
            Self::MoonSeed(_, _) => "Graine",
        }
    }
}

// ── Colors ──

const BG_DARK: Color = Color::srgba(0.06, 0.06, 0.10, 1.0);
const BG_CARD: Color = Color::srgba(0.10, 0.10, 0.16, 1.0);
const BG_CARD_HEADER: Color = Color::srgba(0.13, 0.13, 0.20, 1.0);
const BG_PANEL: Color = Color::srgba(0.12, 0.12, 0.18, 1.0);
const BG_SLIDER: Color = Color::srgba(0.18, 0.18, 0.26, 1.0);
const ACCENT: Color = Color::srgb(0.3, 0.6, 1.0);
const ACCENT_HOVER: Color = Color::srgb(0.4, 0.7, 1.0);
const TEXT_COLOR: Color = Color::srgb(0.9, 0.9, 0.95);
const TEXT_DIM: Color = Color::srgb(0.55, 0.55, 0.62);
const PLANET_COLOR: Color = Color::srgb(0.35, 0.7, 1.0);
const STAR_COLOR: Color = Color::srgb(1.0, 0.75, 0.25);
const BELT_COLOR: Color = Color::srgb(0.7, 0.55, 0.95);
const REMNANT_COLOR: Color = Color::srgb(0.8, 0.5, 1.0);
const RED_SOFT: Color = Color::srgb(0.85, 0.35, 0.35);

// ══════════════════════════════════════════════════════════
// Game window UI
// ══════════════════════════════════════════════════════════

fn setup_game_ui(mut commands: Commands, settings: Res<GameSettings>) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(20.0),
                top: Val::Px(20.0),
                padding: UiRect::axes(Val::Px(16.0), Val::Px(8.0)),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(BG_DARK),
            BorderColor(ACCENT),
            BorderRadius::all(Val::Px(6.0)),
            Button,
            OptionsButton,
        ))
        .with_child((
            Text::new("Options"),
            TextFont { font_size: 18.0, ..default() },
            TextColor(TEXT_COLOR),
        ));

    let menu_root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(50.0),
                top: Val::Percent(50.0),
                width: Val::Px(400.0),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(2.0)),
                margin: UiRect {
                    left: Val::Px(-200.0),
                    top: Val::Px(-160.0),
                    ..default()
                },
                ..default()
            },
            BackgroundColor(BG_DARK),
            BorderColor(ACCENT),
            BorderRadius::all(Val::Px(10.0)),
            Visibility::Hidden,
            MenuRoot,
        ))
        .id();

    let title = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(14.0)),
                justify_content: JustifyContent::Center,
                border: UiRect::bottom(Val::Px(1.0)),
                ..default()
            },
            BorderColor(BG_SLIDER),
        ))
        .with_child((
            Text::new("CAMERA"),
            TextFont { font_size: 20.0, ..default() },
            TextColor(TEXT_COLOR),
        ))
        .id();

    let content = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(14.0)),
            row_gap: Val::Px(10.0),
            ..default()
        })
        .id();

    let s1 = spawn_slider(&mut commands, &settings, SettingKey::MouseSensitivity, 0.05, 2.0);
    let s2 = spawn_slider(&mut commands, &settings, SettingKey::ScrollSpeed, 1.0, 30.0);
    let s3 = spawn_slider(&mut commands, &settings, SettingKey::KeyboardSpeed, 0.5, 5.0);
    let invert = spawn_toggle(&mut commands, "Inverser Y", settings.invert_y, ToggleInvertY);
    let show_light = spawn_toggle(
        &mut commands,
        "Afficher eclairage",
        settings.show_light_indicator,
        ToggleShowLight,
    );
    let show_orbits = spawn_toggle(
        &mut commands,
        "Afficher orbites",
        settings.show_orbits,
        ToggleShowOrbits,
    );
    commands.entity(content).add_children(&[s1, s2, s3, invert, show_light, show_orbits]);
    commands.entity(menu_root).add_children(&[title, content]);

}

fn spawn_center_buttons(commands: &mut Commands, settings: &GameSettings) {
    let bar = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(20.0),
                left: Val::Percent(50.0),
                margin: UiRect { left: Val::Px(-200.0), ..default() },
                width: Val::Px(400.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(6.0),
                flex_wrap: FlexWrap::Wrap,
                row_gap: Val::Px(6.0),
                ..default()
            },
            CenterButtonBar,
        ))
        .id();

    for (i, _) in settings.stars.iter().enumerate() {
        let btn = spawn_center_btn(commands, &format!("E{}", i + 1), TargetKind::Star(i), STAR_COLOR);
        commands.entity(bar).add_child(btn);
    }
    for (i, _) in settings.planets.iter().enumerate() {
        let btn = spawn_center_btn(commands, &format!("P{}", i + 1), TargetKind::Planet(i), PLANET_COLOR);
        commands.entity(bar).add_child(btn);
    }
}

fn spawn_center_btn(commands: &mut Commands, label: &str, target: TargetKind, color: Color) -> Entity {
    commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(BG_DARK),
            BorderColor(color),
            BorderRadius::all(Val::Px(6.0)),
            Button,
            CenterButton(target),
        ))
        .with_child((
            Text::new(label.to_string()),
            TextFont { font_size: 13.0, ..default() },
            TextColor(TEXT_COLOR),
        ))
        .id()
}

// ══════════════════════════════════════════════════════════
// Options window
// ══════════════════════════════════════════════════════════

fn setup_options_window(
    mut commands: Commands,
    settings: Res<GameSettings>,
    tab: Res<OptionsTabState>,
) {
    let options_window = commands
        .spawn(Window {
            title: "SpaceSpore - Options".into(),
            resolution: (480.0_f32, 900.0_f32).into(),
            position: WindowPosition::Automatic,
            ..default()
        })
        .id();

    let ui_camera = commands
        .spawn((
            Camera2d,
            Camera {
                target: bevy::render::camera::RenderTarget::Window(WindowRef::Entity(options_window)),
                ..default()
            },
            OptionsWindowCam,
        ))
        .id();

    spawn_options_ui_root(&mut commands, &settings, ui_camera, tab.active);
}

fn spawn_options_ui_root(
    commands: &mut Commands,
    settings: &GameSettings,
    cam_entity: Entity,
    active_tab: OptionsTab,
) {
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                overflow: Overflow::scroll_y(),
                ..default()
            },
            BackgroundColor(BG_DARK),
            TargetCamera(cam_entity),
            OptionsUiRoot,
        ))
        .id();

    let title = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::axes(Val::Px(20.0), Val::Px(16.0)),
                justify_content: JustifyContent::Center,
                border: UiRect::bottom(Val::Px(2.0)),
                ..default()
            },
            BorderColor(BG_SLIDER),
        ))
        .with_child((
            Text::new("SYSTEME SOLAIRE"),
            TextFont { font_size: 22.0, ..default() },
            TextColor(TEXT_COLOR),
        ))
        .id();

    let content = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::axes(Val::Px(16.0), Val::Px(12.0)),
            row_gap: Val::Px(10.0),
            ..default()
        })
        .id();

    if active_tab == OptionsTab::General {
        let general_cat = spawn_category_header(commands, "REGLAGES GENERAUX", ACCENT);
        commands.entity(content).add_child(general_cat);
        for (key, min, max) in [
            (SettingKey::MouseSensitivity, 0.05, 2.0),
            (SettingKey::ScrollSpeed, 1.0, 30.0),
            (SettingKey::KeyboardSpeed, 0.5, 5.0),
        ] {
            let slider = spawn_slider(commands, settings, key, min, max);
            commands.entity(content).add_child(slider);
        }
        let invert = spawn_toggle(commands, "Inverser Y", settings.invert_y, ToggleInvertY);
        let light = spawn_toggle(commands, "Afficher eclairage", settings.show_light_indicator, ToggleShowLight);
        let orbits = spawn_toggle(commands, "Afficher orbites", settings.show_orbits, ToggleShowOrbits);
        commands.entity(content).add_children(&[invert, light, orbits]);
    }

    if active_tab == OptionsTab::Planets {
    // ════ PLANETES ════
    let planet_cat = spawn_category_header(commands, "PLANETES", PLANET_COLOR);
    commands.entity(content).add_child(planet_cat);

    let moon_color = Color::srgb(0.6, 0.6, 0.7);

    for (i, pcfg) in settings.planets.iter().enumerate() {
        let (card, body) = spawn_body_card(
            commands,
            settings,
            &format!("Planete {}", i + 1),
            PLANET_COLOR,
            BodyAction::RemovePlanet(i),
            &[
                (SettingKey::PlanetOrbitDistance(i), 100.0, 100000.0),
                (SettingKey::PlanetRadius(i), 20.0, 500.0),
                (SettingKey::PlanetSeaLevel(i), 0.1, 0.7),
                (SettingKey::PlanetTerrainHeight(i), 5.0, 200.0),
                (SettingKey::PlanetSeed(i), 1.0, 999.0),
                (SettingKey::PlanetNoiseScale(i), 0.5, 5.0),
                (SettingKey::PlanetDetailScale(i), 1.0, 10.0),
            ],
        );
        let atmo_toggle = spawn_toggle(commands, "Atmosphere", pcfg.atmosphere, ToggleAtmosphere(i));
        commands.entity(body).add_child(atmo_toggle);
        if pcfg.atmosphere {
            for (key, min, max) in [
                (SettingKey::PlanetCloudDensity(i), 0.1, 1.0),
                (SettingKey::PlanetCloudAltitude(i), 5.0, 200.0),
                (SettingKey::PlanetCloudSpeed(i), 0.005, 0.1),
            ] {
                let s = spawn_slider(commands, settings, key, min, max);
                commands.entity(body).add_child(s);
            }
        }
        commands.entity(content).add_child(card);

        for (mi, _) in pcfg.moons.iter().enumerate() {
            let (moon_card, _) = spawn_body_card(
                commands,
                settings,
                &format!("  Lune {} (P{})", mi + 1, i + 1),
                moon_color,
                BodyAction::RemoveMoon(i, mi),
                &[
                    (SettingKey::MoonOrbitDistance(i, mi), 30.0, 10000.0),
                    (SettingKey::MoonRadius(i, mi), 3.0, 150.0),
                    (SettingKey::MoonSeed(i, mi), 1.0, 999.0),
                ],
            );
            commands.entity(moon_card).insert((Button, CenterButton(TargetKind::Moon(i, mi))));
            commands.entity(content).add_child(moon_card);
        }

        let add_moon = spawn_add_button(commands, &format!("Ajouter lune (P{})", i + 1), BodyAction::AddMoon(i), moon_color);
        commands.entity(content).add_child(add_moon);
    }

    let add_planet = spawn_add_button(commands, "Ajouter planete", BodyAction::AddPlanet, PLANET_COLOR);
    commands.entity(content).add_child(add_planet);
    }

    if active_tab == OptionsTab::Stars {
    // ════ ETOILES ════
    let star_cat = spawn_category_header(commands, "ETOILES", STAR_COLOR);
    commands.entity(content).add_child(star_cat);

    for (i, _) in settings.stars.iter().enumerate() {
        let (card, _) = spawn_body_card(
            commands,
            settings,
            &format!("Etoile {}", i + 1),
            STAR_COLOR,
            BodyAction::RemoveStar(i),
            &[
                (SettingKey::StarOrbitDistance(i), 0.0, 5000.0),
                (SettingKey::StarRadius(i), 20.0, 2000.0),
                (SettingKey::StarIntensity(i), 415.0, 57000.0),
                (SettingKey::StarLightRange(i), 500.0, 100000.0),
                (SettingKey::StarColorR(i), 0.0, 1.0),
                (SettingKey::StarColorG(i), 0.0, 1.0),
                (SettingKey::StarColorB(i), 0.0, 1.0),
                (SettingKey::StarFlareCount(i), 0.0, 20.0),
                (SettingKey::StarFlareHeight(i), 10.0, 500.0),
                (SettingKey::StarFlareSpeed(i), 0.1, 5.0),
                (SettingKey::StarFlareSize(i), 2.0, 60.0),
                (SettingKey::StarFlareDistance(i), 0.0, 300.0),
            ],
        );
        commands.entity(content).add_child(card);
    }

    let add_star = spawn_add_button(commands, "Ajouter etoile", BodyAction::AddStar, STAR_COLOR);
    commands.entity(content).add_child(add_star);
    }

    if active_tab == OptionsTab::Belts {
    // ════ CEINTURES ════
    let belt_cat = spawn_category_header(commands, "CEINTURES", BELT_COLOR);
    commands.entity(content).add_child(belt_cat);

    for (i, _) in settings.asteroid_belts.iter().enumerate() {
        let (card, _) = spawn_body_card(
            commands,
            settings,
            &format!("Ceinture {}", i + 1),
            BELT_COLOR,
            BodyAction::RemoveBelt(i),
            &[
                (SettingKey::BeltDistance(i), 50.0, 100000.0),
                (SettingKey::BeltWidth(i), 15.0, 500.0),
                (SettingKey::BeltMinSize(i), 0.5, 30.0),
                (SettingKey::BeltMaxSize(i), 1.0, 60.0),
                (SettingKey::BeltCount(i), 10.0, 1000.0),
            ],
        );
        commands.entity(content).add_child(card);
    }

    let add_belt = spawn_add_button(commands, "Ajouter ceinture", BodyAction::AddBelt, BELT_COLOR);
    commands.entity(content).add_child(add_belt);
    }

    // ════ REGENERER ════
    let apply_btn = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::axes(Val::Px(0.0), Val::Px(14.0)),
                margin: UiRect::top(Val::Px(8.0)),
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(BG_PANEL),
            BorderColor(ACCENT),
            BorderRadius::all(Val::Px(8.0)),
            Button,
            ApplyPlanetButton,
        ))
        .with_child((
            Text::new("REGENERER"),
            TextFont { font_size: 17.0, ..default() },
            TextColor(TEXT_COLOR),
        ))
        .id();
    commands.entity(content).add_child(apply_btn);

    let tabs = spawn_options_tabs(commands, active_tab);
    commands.entity(root).add_children(&[title, tabs, content]);
}

// ══════════════════════════════════════════════════════════
// Layout helpers
// ══════════════════════════════════════════════════════════

fn spawn_options_tabs(commands: &mut Commands, active: OptionsTab) -> Entity {
    let bar = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            padding: UiRect::axes(Val::Px(10.0), Val::Px(8.0)),
            column_gap: Val::Px(6.0),
            ..default()
        })
        .id();

    for (tab, label, color) in [
        (OptionsTab::General, "General", ACCENT),
        (OptionsTab::Planets, "Planetes", PLANET_COLOR),
        (OptionsTab::Stars, "Etoiles", STAR_COLOR),
        (OptionsTab::Belts, "Ceintures", BELT_COLOR),
    ] {
        let selected = tab == active;
        let button = commands
            .spawn((
                Node {
                    flex_grow: 1.0,
                    justify_content: JustifyContent::Center,
                    padding: UiRect::axes(Val::Px(4.0), Val::Px(9.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(if selected { color.with_alpha(0.22) } else { BG_PANEL }),
                BorderColor(if selected { color } else { BG_SLIDER }),
                BorderRadius::all(Val::Px(4.0)),
                Button,
                OptionsTabButton(tab),
            ))
            .with_child((
                Text::new(label),
                TextFont { font_size: 12.0, ..default() },
                TextColor(if selected { color } else { TEXT_DIM }),
            ))
            .id();
        commands.entity(bar).add_child(button);
    }

    bar
}

fn spawn_category_header(commands: &mut Commands, label: &str, color: Color) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::new(Val::Px(14.0), Val::Px(0.0), Val::Px(10.0), Val::Px(6.0)),
                border: UiRect::left(Val::Px(4.0)),
                margin: UiRect::top(Val::Px(6.0)),
                ..default()
            },
            BorderColor(color),
        ))
        .with_child((
            Text::new(label.to_string()),
            TextFont { font_size: 16.0, ..default() },
            TextColor(color),
        ))
        .id()
}

fn spawn_body_card(
    commands: &mut Commands,
    settings: &GameSettings,
    title: &str,
    color: Color,
    remove_action: BodyAction,
    sliders: &[(SettingKey, f32, f32)],
) -> (Entity, Entity) {
    let card = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                border: UiRect::top(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(BG_CARD),
            BorderColor(color),
            BorderRadius::all(Val::Px(8.0)),
        ))
        .id();

    // Card header row
    let header = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(14.0), Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(BG_CARD_HEADER),
            BorderRadius::px(0.0, 0.0, 8.0, 8.0),
        ))
        .id();

    let title_text = commands
        .spawn((
            Text::new(title.to_string()),
            TextFont { font_size: 14.0, ..default() },
            TextColor(color),
        ))
        .id();

    let remove_btn = commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.4, 0.1, 0.1, 0.6)),
            BorderColor(RED_SOFT),
            BorderRadius::all(Val::Px(4.0)),
            Button,
            remove_action,
        ))
        .with_child((
            Text::new("Supprimer"),
            TextFont { font_size: 11.0, ..default() },
            TextColor(RED_SOFT),
        ))
        .id();

    commands.entity(header).add_children(&[title_text, remove_btn]);

    // Card body with sliders
    let body = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::new(Val::Px(14.0), Val::Px(14.0), Val::Px(6.0), Val::Px(12.0)),
            row_gap: Val::Px(6.0),
            ..default()
        })
        .id();

    for (key, min, max) in sliders {
        let s = spawn_slider(commands, settings, *key, *min, *max);
        commands.entity(body).add_child(s);
    }

    commands.entity(card).add_children(&[header, body]);
    (card, body)
}

fn spawn_add_button(commands: &mut Commands, label: &str, action: BodyAction, color: Color) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::axes(Val::Px(0.0), Val::Px(10.0)),
                margin: UiRect::vertical(Val::Px(2.0)),
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            BorderColor(color.with_alpha(0.4)),
            BorderRadius::all(Val::Px(8.0)),
            Button,
            action,
        ))
        .with_child((
            Text::new(format!("+ {}", label)),
            TextFont { font_size: 14.0, ..default() },
            TextColor(color.with_alpha(0.7)),
        ))
        .id()
}

fn spawn_slider(
    commands: &mut Commands,
    settings: &GameSettings,
    key: SettingKey,
    min: f32,
    max: f32,
) -> Entity {
    let val = key.get(settings);
    let frac = ((val - min) / (max - min)).clamp(0.0, 1.0);

    let row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(3.0),
            ..default()
        })
        .id();

    let label_row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            justify_content: JustifyContent::SpaceBetween,
            ..default()
        })
        .id();

    let name_label = commands
        .spawn((
            Text::new(key.label().to_string()),
            TextFont { font_size: 12.0, ..default() },
            TextColor(TEXT_DIM),
        ))
        .id();

    let val_text = if val == val.floor() && val.abs() < 10000.0 {
        format!("{:.0}", val)
    } else {
        format!("{:.2}", val)
    };

    let val_label = commands
        .spawn((
            Text::new(val_text),
            TextFont { font_size: 12.0, ..default() },
            TextColor(TEXT_COLOR),
            SliderLabel(key),
        ))
        .id();

    commands.entity(label_row).add_children(&[name_label, val_label]);

    let bar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(10.0),
                ..default()
            },
            BackgroundColor(BG_SLIDER),
            BorderRadius::all(Val::Px(5.0)),
            SliderBar { setting: key, min, max },
            Button,
        ))
        .id();

    let fill = commands
        .spawn((
            Node {
                width: Val::Percent(frac * 100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(ACCENT),
            BorderRadius::all(Val::Px(5.0)),
            SliderFill(key),
        ))
        .id();

    commands.entity(bar).add_child(fill);
    commands.entity(row).add_children(&[label_row, bar]);
    row
}

fn spawn_toggle(
    commands: &mut Commands,
    label: &str,
    value: bool,
    marker: impl Component,
) -> Entity {
    let row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            padding: UiRect::axes(Val::Px(0.0), Val::Px(4.0)),
            ..default()
        })
        .id();

    let name = commands
        .spawn((
            Text::new(label.to_string()),
            TextFont { font_size: 13.0, ..default() },
            TextColor(TEXT_DIM),
        ))
        .id();

    let btn = commands
        .spawn((
            Node {
                width: Val::Px(44.0),
                height: Val::Px(22.0),
                border: UiRect::all(Val::Px(2.0)),
                justify_content: if value { JustifyContent::End } else { JustifyContent::Start },
                padding: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(if value { ACCENT } else { BG_SLIDER }),
            BorderColor(if value { ACCENT } else { TEXT_DIM }),
            BorderRadius::all(Val::Px(11.0)),
            Button,
            marker,
        ))
        .with_child((
            Node {
                width: Val::Px(14.0),
                height: Val::Px(14.0),
                ..default()
            },
            BackgroundColor(Color::WHITE),
            BorderRadius::all(Val::Px(7.0)),
        ))
        .id();

    commands.entity(row).add_children(&[name, btn]);
    row
}

// ══════════════════════════════════════════════════════════
// Systems
// ══════════════════════════════════════════════════════════

fn toggle_menu(keys: Res<ButtonInput<KeyCode>>, mut state: ResMut<MenuState>) {
    if keys.just_pressed(KeyCode::Escape) {
        state.open = !state.open;
    }
}

fn handle_options_button(
    interactions: Query<&Interaction, (Changed<Interaction>, With<OptionsButton>)>,
    mut state: ResMut<MenuState>,
) {
    for interaction in &interactions {
        if *interaction == Interaction::Pressed {
            state.open = !state.open;
        }
    }
}

fn handle_options_tabs(
    interactions: Query<(&Interaction, &OptionsTabButton), Changed<Interaction>>,
    mut tab: ResMut<OptionsTabState>,
    mut rebuild: EventWriter<RebuildUi>,
) {
    for (interaction, button) in &interactions {
        if *interaction == Interaction::Pressed && tab.active != button.0 {
            tab.active = button.0;
            rebuild.send(RebuildUi);
        }
    }
}

fn handle_astres_tabs(
    interactions: Query<(&Interaction, &AstresTabButton), Changed<Interaction>>,
    mut tab: ResMut<AstresTabState>,
    mut rebuild: EventWriter<RebuildUi>,
) {
    for (interaction, button) in &interactions {
        if *interaction == Interaction::Pressed && tab.active != button.0 {
            tab.active = button.0;
            rebuild.send(RebuildUi);
        }
    }
}

fn update_menu_visibility(
    state: Res<MenuState>,
    mut menu_q: Query<&mut Visibility, With<MenuRoot>>,
) {
    if !state.is_changed() {
        return;
    }
    for mut vis in &mut menu_q {
        *vis = if state.open { Visibility::Visible } else { Visibility::Hidden };
    }
}

fn handle_slider_interactions(
    interactions: Query<(&Interaction, &SliderBar, &Node, &GlobalTransform), Changed<Interaction>>,
    primary_window: Query<&Window, With<PrimaryWindow>>,
    other_windows: Query<&Window, Without<PrimaryWindow>>,
    mut settings: ResMut<GameSettings>,
    mut regenerate_planet: EventWriter<RegeneratePlanet>,
) {
    for (interaction, slider, node, global_tf) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }

        let mut cursor_x = None;
        if let Ok(w) = primary_window.get_single() {
            if let Some(pos) = w.cursor_position() {
                cursor_x = Some(pos.x);
            }
        }
        if cursor_x.is_none() {
            for w in &other_windows {
                if let Some(pos) = w.cursor_position() {
                    cursor_x = Some(pos.x);
                }
            }
        }
        let Some(cx) = cursor_x else { continue };

        let bar_pos = global_tf.translation();
        let bar_width = node.width;
        if let Val::Percent(pct) = bar_width {
            let estimated_width = pct / 100.0 * 448.0;
            let bar_left = bar_pos.x - estimated_width * 0.5;
            let frac = ((cx - bar_left) / estimated_width).clamp(0.0, 1.0);
            let val = slider.min + frac * (slider.max - slider.min);
            slider.setting.set(&mut settings, val);
            settings.save();
            if matches!(slider.setting, SettingKey::StarIntensity(_) | SettingKey::StarLightRange(_)) {
                regenerate_planet.send(RegeneratePlanet);
            }
        }
    }
}

fn handle_toggle_button(
    interactions: Query<&Interaction, (Changed<Interaction>, With<ToggleInvertY>)>,
    mut settings: ResMut<GameSettings>,
    mut toggle_q: Query<(&mut BackgroundColor, &mut BorderColor, &mut Node), With<ToggleInvertY>>,
) {
    for interaction in &interactions {
        if *interaction == Interaction::Pressed {
            settings.invert_y = !settings.invert_y;
            settings.save();
            for (mut bg, mut border, mut node) in &mut toggle_q {
                *bg = BackgroundColor(if settings.invert_y { ACCENT } else { BG_SLIDER });
                *border = BorderColor(if settings.invert_y { ACCENT } else { TEXT_DIM });
                node.justify_content = if settings.invert_y {
                    JustifyContent::End
                } else {
                    JustifyContent::Start
                };
            }
        }
    }
}

fn handle_toggle_show_light(
    interactions: Query<&Interaction, (Changed<Interaction>, With<ToggleShowLight>)>,
    mut settings: ResMut<GameSettings>,
    mut toggle_q: Query<(&mut BackgroundColor, &mut BorderColor, &mut Node), With<ToggleShowLight>>,
) {
    for interaction in &interactions {
        if *interaction == Interaction::Pressed {
            settings.show_light_indicator = !settings.show_light_indicator;
            settings.save();
            for (mut bg, mut border, mut node) in &mut toggle_q {
                *bg = BackgroundColor(if settings.show_light_indicator { ACCENT } else { BG_SLIDER });
                *border = BorderColor(if settings.show_light_indicator { ACCENT } else { TEXT_DIM });
                node.justify_content = if settings.show_light_indicator {
                    JustifyContent::End
                } else {
                    JustifyContent::Start
                };
            }
        }
    }
}

fn handle_toggle_show_orbits(
    interactions: Query<&Interaction, (Changed<Interaction>, With<ToggleShowOrbits>)>,
    mut settings: ResMut<GameSettings>,
    mut toggle_q: Query<(&mut BackgroundColor, &mut BorderColor, &mut Node), With<ToggleShowOrbits>>,
) {
    for interaction in &interactions {
        if *interaction == Interaction::Pressed {
            settings.show_orbits = !settings.show_orbits;
            settings.save();
            for (mut bg, mut border, mut node) in &mut toggle_q {
                *bg = BackgroundColor(if settings.show_orbits { ACCENT } else { BG_SLIDER });
                *border = BorderColor(if settings.show_orbits { ACCENT } else { TEXT_DIM });
                node.justify_content = if settings.show_orbits {
                    JustifyContent::End
                } else {
                    JustifyContent::Start
                };
            }
        }
    }
}

fn handle_toggle_atmosphere(
    interactions: Query<(&Interaction, &ToggleAtmosphere), Changed<Interaction>>,
    mut settings: ResMut<GameSettings>,
    mut rebuild: EventWriter<RebuildUi>,
) {
    for (interaction, toggle) in &interactions {
        if *interaction == Interaction::Pressed {
            if let Some(p) = settings.planets.get_mut(toggle.0) {
                p.atmosphere = !p.atmosphere;
                settings.save();
                rebuild.send(RebuildUi);
            }
        }
    }
}

fn handle_apply_button(
    interactions: Query<&Interaction, (Changed<Interaction>, With<ApplyPlanetButton>)>,
    mut events: EventWriter<RegeneratePlanet>,
    mut btn_q: Query<&mut BackgroundColor, With<ApplyPlanetButton>>,
) {
    for interaction in &interactions {
        match interaction {
            Interaction::Pressed => {
                events.send(RegeneratePlanet);
            }
            Interaction::Hovered => {
                for mut bg in &mut btn_q {
                    *bg = BackgroundColor(ACCENT_HOVER);
                }
            }
            Interaction::None => {
                for mut bg in &mut btn_q {
                    *bg = BackgroundColor(BG_PANEL);
                }
            }
        }
    }
}

fn handle_center_buttons(
    interactions: Query<(&Interaction, &CenterButton), Changed<Interaction>>,
    mut target: ResMut<CameraTarget>,
) {
    for (interaction, btn) in &interactions {
        if *interaction == Interaction::Pressed {
            target.0 = btn.0;
        }
    }
}

fn update_slider_visuals(
    settings: Res<GameSettings>,
    sliders: Query<&SliderBar>,
    mut fills: Query<(&SliderFill, &mut Node), Without<SliderBar>>,
    mut labels: Query<(&SliderLabel, &mut Text)>,
) {
    if !settings.is_changed() {
        return;
    }

    for bar in &sliders {
        let val = bar.setting.get(&settings);
        let frac = ((val - bar.min) / (bar.max - bar.min)).clamp(0.0, 1.0);
        for (fill, mut node) in &mut fills {
            if fill.0 == bar.setting {
                node.width = Val::Percent(frac * 100.0);
            }
        }
    }

    for (label, mut text) in &mut labels {
        let val = label.0.get(&settings);
        let val_text = if val == val.floor() && val.abs() < 10000.0 {
            format!("{:.0}", val)
        } else {
            format!("{:.2}", val)
        };
        **text = val_text;
    }
}

fn handle_body_actions(
    interactions: Query<(&Interaction, &BodyAction), Changed<Interaction>>,
    mut settings: ResMut<GameSettings>,
    mut target: ResMut<CameraTarget>,
    mut regen: EventWriter<RegeneratePlanet>,
    mut rebuild: EventWriter<RebuildUi>,
) {
    for (interaction, action) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *action {
            BodyAction::AddPlanet => {
                let n = settings.planets.len();
                settings.planets.push(PlanetConfig {
                    orbit_distance: 450.0 + n as f32 * 200.0,
                    seed: 42 + n as u32 * 13,
                    ..PlanetConfig::default()
                });
            }
            BodyAction::RemovePlanet(i) => {
                if settings.planets.len() > 1 && i < settings.planets.len() {
                    settings.planets.remove(i);
                }
            }
            BodyAction::AddStar => {
                let n = settings.stars.len();
                settings.stars.push(StarConfig {
                    orbit_distance: 150.0 + n as f32 * 100.0,
                    radius: 80.0,
                    ..StarConfig::default()
                });
            }
            BodyAction::RemoveStar(i) => {
                if settings.stars.len() > 1 && i < settings.stars.len() {
                    settings.stars.remove(i);
                }
            }
            BodyAction::AddBelt => {
                settings.asteroid_belts.push(AsteroidBeltConfig {
                    distance: 300.0,
                    ..AsteroidBeltConfig::default()
                });
            }
            BodyAction::RemoveBelt(i) => {
                if i < settings.asteroid_belts.len() {
                    settings.asteroid_belts.remove(i);
                }
            }
            BodyAction::AddMoon(pi) => {
                if let Some(planet) = settings.planets.get_mut(pi) {
                    let n = planet.moons.len();
                    planet.moons.push(crate::settings::MoonConfig {
                        orbit_distance: 80.0 + n as f32 * 30.0,
                        radius: 12.0,
                        seed: 77 + n as u32 * 11,
                    });
                }
            }
            BodyAction::RemoveMoon(pi, mi) => {
                if let Some(planet) = settings.planets.get_mut(pi) {
                    if mi < planet.moons.len() {
                        planet.moons.remove(mi);
                    }
                }
            }
        }
        settings.save();
        target.0 = TargetKind::Planet(0);
        regen.send(RegeneratePlanet);
        rebuild.send(RebuildUi);
    }
}

fn rebuild_options_ui(
    mut commands: Commands,
    settings: Res<GameSettings>,
    tab: Res<OptionsTabState>,
    mut events: EventReader<RebuildUi>,
    ui_root_q: Query<Entity, With<OptionsUiRoot>>,
    camera_q: Query<Entity, With<OptionsWindowCam>>,
) {
    let mut should = false;
    for _ in events.read() {
        should = true;
    }
    if !should {
        return;
    }
    for entity in &ui_root_q {
        commands.entity(entity).despawn_recursive();
    }
    let Ok(cam) = camera_q.get_single() else { return };
    spawn_options_ui_root(&mut commands, &settings, cam, tab.active);
}

fn rebuild_astres_ui(
    mut commands: Commands,
    res: AstresResources,
    tab: Res<AstresTabState>,
    mut events: EventReader<RebuildUi>,
    ui_root_q: Query<Entity, With<AstresUiRoot>>,
    camera_q: Query<Entity, With<AstresWindowCam>>,
) {
    if events.read().next().is_none() {
        return;
    }
    for entity in &ui_root_q {
        commands.entity(entity).despawn_recursive();
    }
    let Ok(cam) = camera_q.get_single() else { return };
    spawn_astres_ui_root(
        &mut commands,
        &res.gas_res, &res.comet_res, &res.meteor_res,
        &res.vstar_res, &res.proto_res, &res.dwarf_res, &res.ms_res, &res.giant_res, &res.sg_res, &res.hg_res,
        &res.nebula_res, &res.black_res, &res.pulsar_res, &res.magnetar_res, &res.neutron_res, &res.sn_res,
        cam, tab.active,
    );
}

fn rebuild_center_buttons(
    mut commands: Commands,
    settings: Res<GameSettings>,
    mut events: EventReader<RebuildUi>,
    bar_q: Query<Entity, With<CenterButtonBar>>,
) {
    let mut should = false;
    for _ in events.read() {
        should = true;
    }
    if !should {
        return;
    }
    for entity in &bar_q {
        commands.entity(entity).despawn_recursive();
    }
    spawn_center_buttons(&mut commands, &settings);
}

fn scroll_options_panel(
    mut mouse_wheel: EventReader<MouseWheel>,
    primary_window: Query<&Window, With<PrimaryWindow>>,
    other_windows: Query<&Window, Without<PrimaryWindow>>,
    mut scroll_q: Query<&mut ScrollPosition, Or<(With<OptionsUiRoot>, With<AstresUiRoot>)>>,
) {
    let mut on_options_window = false;
    if let Ok(w) = primary_window.get_single() {
        if w.cursor_position().is_some() {
            on_options_window = false;
        }
    }
    for w in &other_windows {
        if w.cursor_position().is_some() {
            on_options_window = true;
        }
    }

    if !on_options_window {
        mouse_wheel.clear();
        return;
    }

    let mut dy = 0.0;
    for ev in mouse_wheel.read() {
        dy -= ev.y * 40.0;
    }
    if dy == 0.0 {
        return;
    }

    for mut scroll_pos in &mut scroll_q {
        scroll_pos.offset_y = (scroll_pos.offset_y + dy).max(0.0);
    }
}
// ══════════════════════════════════════════════════════════
// Fenêtre Astres
// ══════════════════════════════════════════════════════════

fn setup_astres_window(
    mut commands: Commands,
    res: AstresResources, // Remplace les 16 Res par cette structure
    tab: Res<AstresTabState>,
) {
    let astres_window = commands
        .spawn(Window {
            title: "SpaceSpore - Astres".into(),
            resolution: (480.0_f32, 860.0_f32).into(),
            position: WindowPosition::Automatic,
            ..default()
        })
        .id();
    let ui_camera = commands
        .spawn((
            Camera2d,
            Camera {
                target: bevy::render::camera::RenderTarget::Window(WindowRef::Entity(astres_window)),
                ..default()
            },
            AstresWindowCam,
        ))
        .id();
    spawn_astres_ui_root(
        &mut commands,
        &res.gas_res, &res.comet_res, &res.meteor_res,
        &res.vstar_res, &res.proto_res, &res.dwarf_res, &res.ms_res, &res.giant_res, &res.sg_res, &res.hg_res,
        &res.nebula_res, &res.black_res, &res.pulsar_res, &res.magnetar_res, &res.neutron_res, &res.sn_res,
        ui_camera, tab.active,
    );
}

fn spawn_astres_ui_root(
    commands:     &mut Commands,
    gas_res:      &GasPlanetRes,
    comet_res:    &CometRes,
    meteor_res:   &MeteoroidRes,
    vstar_res:    &VoxelStarRes,
    proto_res:    &ProtostarRes,
    dwarf_res:    &DwarfStarRes,
    ms_res:       &MainSequenceRes,
    giant_res:    &GiantStarRes,
    sg_res:       &SupergiantRes,
    hg_res:       &HypergiantRes,
    nebula_res:   &NebulaRes,
    black_res:    &BlackHoleRes,
    pulsar_res:   &PulsarRes,
    magnetar_res: &MagnetarRes,
    neutron_res:  &NeutronStarRes,
    sn_res:       &SupernovaRes,
    cam_entity:   Entity,
    active_tab:   AstresTab,
) {
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::axes(Val::Px(16.0), Val::Px(12.0)),
                row_gap: Val::Px(10.0),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            BackgroundColor(BG_DARK),
            TargetCamera(cam_entity),
            AstresUiRoot,
        ))
        .id();

    let title = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::axes(Val::Px(20.0), Val::Px(16.0)),
                justify_content: JustifyContent::Center,
                border: UiRect::bottom(Val::Px(2.0)),
                ..default()
            },
            BorderColor(BG_SLIDER),
        ))
        .with_child((
            Text::new("ASTRES"),
            TextFont {
                font_size: 22.0,
                ..default()
            },
            TextColor(TEXT_COLOR),
        ))
        .id();

    let content = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::axes(Val::Px(16.0), Val::Px(12.0)),
            row_gap: Val::Px(10.0),
            ..default()
        })
        .id();

    if active_tab == AstresTab::Planets {
    // ── Planètes gazeuses ──
    let gas_cat = spawn_category_header(commands, "PLANETES GAZEUSES", PLANET_COLOR);
    commands.entity(content).add_child(gas_cat);

    for (i, cfg) in gas_res.planets.iter().enumerate() {
        let info = format!(
            "Rayon {:.0}  -  Orbite {:.0}",
            cfg.radius, cfg.orbit_distance
        );

        let card = spawn_astre_card(
            commands,
            &format!("Planete gazeuse {}", i + 1),
            TargetKind::GasPlanet(i),
            PLANET_COLOR,
            &info,
        );

        commands.entity(content).add_child(card);
    }

    let gas_regen = spawn_astre_regen_button(
        commands,
        "Regenerer planetes gazeuses",
        RegenAstreAction::GasPlanet,
        PLANET_COLOR,
    );
    commands.entity(content).add_child(gas_regen);
    add_astre_add_button(commands, content, "une planete gazeuse", AstreFamily::GasPlanet, PLANET_COLOR);
    }

    if active_tab == AstresTab::Remnants {
    // ── Nébuleuse ──
    if nebula_res.enabled {
    let nebula_cat = spawn_category_header(commands, "NEBULEUSE", BELT_COLOR);
    commands.entity(content).add_child(nebula_cat);

    let nebula_info = format!(
        "Rayon {:.0}  -  Seed {}",
        nebula_res.config.radius, nebula_res.config.seed
    );

    let nebula_card = spawn_astre_card(
        commands,
        "Nebuleuse",
        TargetKind::Nebula,
        BELT_COLOR,
        &nebula_info,
    );
    commands.entity(content).add_child(nebula_card);

    let nebula_regen = spawn_astre_regen_button(
        commands,
        "Regenerer nebuleuse",
        RegenAstreAction::Nebula,
        BELT_COLOR,
    );
    commands.entity(content).add_child(nebula_regen);
    }
    add_astre_add_button(commands, content, "une nebuleuse", AstreFamily::Nebula, BELT_COLOR);

    // ── Trous noirs / remparts stellaires ──
    let black_cat = spawn_category_header(commands, "REMNANTS STELLAIRES", REMNANT_COLOR);
    commands.entity(content).add_child(black_cat);

    for (i, cfg) in black_res.holes.iter().enumerate() {
        let info = format!(
            "Horizon {:.0}  -  Influence {:.0}",
            cfg.event_horizon, cfg.influence_radius
        );

        let card = spawn_astre_card(
            commands,
            &format!("Trou noir {}", i + 1),
            TargetKind::BlackHole(i),
            REMNANT_COLOR,
            &info,
        );

        commands.entity(content).add_child(card);
    }

    let black_regen = spawn_astre_regen_button(
        commands,
        "Regenerer trous noirs",
        RegenAstreAction::BlackHole,
        REMNANT_COLOR,
    );
    commands.entity(content).add_child(black_regen);
    add_astre_add_button(commands, content, "un trou noir", AstreFamily::BlackHole, REMNANT_COLOR);

       // ── Pulsars ──
    let pulsar_cat = spawn_category_header(commands, "PULSARS", REMNANT_COLOR);
    commands.entity(content).add_child(pulsar_cat);
    for (i, cfg) in pulsar_res.pulsars.iter().enumerate() {
        let info = format!("Periode {:.3}s  -  Orbite {:.0}", cfg.period_initial, cfg.orbit_distance);
        let card = spawn_astre_card(commands, &format!("Pulsar {}", i+1), TargetKind::Pulsar(i), REMNANT_COLOR, &info);
        commands.entity(content).add_child(card);
    }
    let btn_pulsar = spawn_astre_regen_button(commands, "Regenerer pulsars", RegenAstreAction::Pulsar, REMNANT_COLOR);
    commands.entity(content).add_child(btn_pulsar);
    add_astre_add_button(commands, content, "un pulsar", AstreFamily::Pulsar, REMNANT_COLOR);

    // ── Magnétars ──
    let mag_cat = spawn_category_header(commands, "MAGNETARS", REMNANT_COLOR);
    commands.entity(content).add_child(mag_cat);
    for (i, cfg) in magnetar_res.magnetars.iter().enumerate() {
        let info = format!("Rayon {:.0}  -  Orbite {:.0}", cfg.radius, cfg.orbit_distance);
        let card = spawn_astre_card(commands, &format!("Magnetar {}", i+1), TargetKind::Magnetar(i), REMNANT_COLOR, &info);
        commands.entity(content).add_child(card);
    }
    let btn_magnetar = spawn_astre_regen_button(commands, "Regenerer magnetars", RegenAstreAction::Magnetar, REMNANT_COLOR);
    commands.entity(content).add_child(btn_magnetar);
    add_astre_add_button(commands, content, "un magnetar", AstreFamily::Magnetar, REMNANT_COLOR);

    // ── Étoiles à neutrons ──
    let ns_cat = spawn_category_header(commands, "ETOILES A NEUTRONS", REMNANT_COLOR);
    commands.entity(content).add_child(ns_cat);
    for (i, cfg) in neutron_res.stars.iter().enumerate() {
        let info = format!("Rayon {:.0}  -  Orbite {:.0}", cfg.radius, cfg.orbit_distance);
        let card = spawn_astre_card(commands, &format!("Etoile neutrons {}", i+1), TargetKind::NeutronStar(i), REMNANT_COLOR, &info);
        commands.entity(content).add_child(card);
    }
    let btn_neutron = spawn_astre_regen_button(commands, "Regenerer etoiles neutrons", RegenAstreAction::NeutronStar, REMNANT_COLOR);
    commands.entity(content).add_child(btn_neutron);
    add_astre_add_button(commands, content, "une etoile a neutrons", AstreFamily::NeutronStar, REMNANT_COLOR);

    // ── Supernovae ──
    let sn_cat = spawn_category_header(commands, "SUPERNOVAE", REMNANT_COLOR);
    commands.entity(content).add_child(sn_cat);
    for (i, cfg) in sn_res.supernovae.iter().enumerate() {
        let info = format!("Type {:?}  -  Orbite {:.0}", cfg.sn_type, cfg.orbit_distance);
        let card = spawn_astre_card(commands, &format!("Supernova {}", i+1), TargetKind::Supernova(i), REMNANT_COLOR, &info);
        commands.entity(content).add_child(card);
    }
    let btn_sn = spawn_astre_regen_button(commands, "Regenerer supernovae", RegenAstreAction::Supernova, REMNANT_COLOR);
    commands.entity(content).add_child(btn_sn);
    add_astre_add_button(commands, content, "une supernova", AstreFamily::Supernova, REMNANT_COLOR);
    }

    if active_tab == AstresTab::Planets {
    // ── Comètes ──
    let comet_cat = spawn_category_header(commands, "COMETES", PLANET_COLOR);
    commands.entity(content).add_child(comet_cat);
    for (i, cfg) in comet_res.comets.iter().enumerate() {
        let info = format!("Perihelie {:.0}  -  Aphelie {:.0}", cfg.perihelion, cfg.aphelion);
        let card = spawn_astre_card(commands, &format!("Comete {}", i+1), TargetKind::Comet(i), PLANET_COLOR, &info);
        commands.entity(content).add_child(card);
    }
    let btn_comet = spawn_astre_regen_button(commands, "Regenerer cometes", RegenAstreAction::Comet, PLANET_COLOR);
    commands.entity(content).add_child(btn_comet);
    add_astre_add_button(commands, content, "une comete", AstreFamily::Comet, PLANET_COLOR);

    // ── Météoroïdes ──
    let met_cat = spawn_category_header(commands, "METEOROÏDES", PLANET_COLOR);
    commands.entity(content).add_child(met_cat);
    for (i, cfg) in meteor_res.meteoroids.iter().enumerate() {
        let info = format!("Rayon {:.0}  -  {:?}", cfg.radius, cfg.composition);
        let card = spawn_astre_card(commands, &format!("Meteoroide {}", i+1), TargetKind::Meteoroid(i), PLANET_COLOR, &info);
        commands.entity(content).add_child(card);
    }
    let btn_meteor = spawn_astre_regen_button(commands, "Regenerer meteoroides", RegenAstreAction::Meteoroid, PLANET_COLOR);
    commands.entity(content).add_child(btn_meteor);
    add_astre_add_button(commands, content, "un meteoroide", AstreFamily::Meteoroid, PLANET_COLOR);
    }

    if active_tab == AstresTab::Stars {
    // ── Étoiles voxel ──
    let vstar_cat = spawn_category_header(commands, "ETOILES VOXEL", STAR_COLOR);
    commands.entity(content).add_child(vstar_cat);
    for (i, cfg) in vstar_res.stars.iter().enumerate() {
        let info = format!("Rayon {:.0}  -  Orbite {:.0}", cfg.radius, cfg.orbit_distance);
        let card = spawn_astre_card(commands, &format!("Etoile {}", i+1), TargetKind::VoxelStar(i), STAR_COLOR, &info);
        commands.entity(content).add_child(card);
    }
    let btn_vstar = spawn_astre_regen_button(commands, "Regenerer etoiles", RegenAstreAction::VoxelStar, STAR_COLOR);
    commands.entity(content).add_child(btn_vstar);
    add_astre_add_button(commands, content, "une etoile voxel", AstreFamily::VoxelStar, STAR_COLOR);

    // ── Protoétoiles ──
    let proto_cat = spawn_category_header(commands, "PROTOETOILES", STAR_COLOR);
    commands.entity(content).add_child(proto_cat);
    for (i, cfg) in proto_res.stars.iter().enumerate() {
        let info = format!("Rayon {:.0}  -  Orbite {:.0}", cfg.radius, cfg.orbit_distance);
        let card = spawn_astre_card(commands, &format!("Protoetoile {}", i+1), TargetKind::Protostar(i), STAR_COLOR, &info);
        commands.entity(content).add_child(card);
    }
    let btn_proto = spawn_astre_regen_button(commands, "Regenerer protoetoiles", RegenAstreAction::Protostar, STAR_COLOR);
    commands.entity(content).add_child(btn_proto);
    add_astre_add_button(commands, content, "une protoetoile", AstreFamily::Protostar, STAR_COLOR);

    // ── Naines ──
    let dwarf_cat = spawn_category_header(commands, "NAINES", STAR_COLOR);
    commands.entity(content).add_child(dwarf_cat);
    for (i, cfg) in dwarf_res.stars.iter().enumerate() {
        let info = format!("Rayon {:.0}  -  {:?}", cfg.radius, cfg.dwarf_type);
        let card = spawn_astre_card(commands, &format!("Naine {}", i+1), TargetKind::DwarfStar(i), STAR_COLOR, &info);
        commands.entity(content).add_child(card);
    }
    let btn_dwarf = spawn_astre_regen_button(commands, "Regenerer naines", RegenAstreAction::DwarfStar, STAR_COLOR);
    commands.entity(content).add_child(btn_dwarf);
    add_astre_add_button(commands, content, "une naine", AstreFamily::DwarfStar, STAR_COLOR);

    // ── Séquence principale ──
    let ms_cat = spawn_category_header(commands, "SEQUENCE PRINCIPALE", STAR_COLOR);
    commands.entity(content).add_child(ms_cat);
    for (i, cfg) in ms_res.stars.iter().enumerate() {
        let info = format!("Classe {:?}  -  Orbite {:.0}", cfg.spectral_class, cfg.orbit_distance);
        let card = spawn_astre_card(commands, &format!("Seq. princ. {}", i+1), TargetKind::MainSequence(i), STAR_COLOR, &info);
        commands.entity(content).add_child(card);
    }
    let btn_ms = spawn_astre_regen_button(commands, "Regenerer seq. principale", RegenAstreAction::MainSequence, STAR_COLOR);
    commands.entity(content).add_child(btn_ms);
    add_astre_add_button(commands, content, "une etoile principale", AstreFamily::MainSequence, STAR_COLOR);

    // ── Géantes ──
    let giant_cat = spawn_category_header(commands, "GEANTES", STAR_COLOR);
    commands.entity(content).add_child(giant_cat);
    for (i, cfg) in giant_res.stars.iter().enumerate() {
        let info = format!("Rayon {:.0}  -  {:?}", cfg.radius, cfg.giant_type);
        let card = spawn_astre_card(commands, &format!("Geante {}", i+1), TargetKind::GiantStar(i), STAR_COLOR, &info);
        commands.entity(content).add_child(card);
    }
    let btn_giant = spawn_astre_regen_button(commands, "Regenerer geantes", RegenAstreAction::GiantStar, STAR_COLOR);
    commands.entity(content).add_child(btn_giant);
    add_astre_add_button(commands, content, "une geante", AstreFamily::GiantStar, STAR_COLOR);

    // ── Supergéantes ──
    let sg_cat = spawn_category_header(commands, "SUPERGEANTES", STAR_COLOR);
    commands.entity(content).add_child(sg_cat);
    for (i, cfg) in sg_res.stars.iter().enumerate() {
        let info = format!("Rayon {:.0}  -  {:?}", cfg.radius, cfg.class);
        let card = spawn_astre_card(commands, &format!("Supergeante {}", i+1), TargetKind::Supergiant(i), STAR_COLOR, &info);
        commands.entity(content).add_child(card);
    }
    let btn_sg = spawn_astre_regen_button(commands, "Regenerer supergeantes", RegenAstreAction::Supergiant, STAR_COLOR);
    commands.entity(content).add_child(btn_sg);
    add_astre_add_button(commands, content, "une supergeante", AstreFamily::Supergiant, STAR_COLOR);

    // ── Hypergéantes ──
    let hg_cat = spawn_category_header(commands, "HYPERGEANTES", STAR_COLOR);
    commands.entity(content).add_child(hg_cat);
    for (i, cfg) in hg_res.stars.iter().enumerate() {
        let info = format!("Rayon {:.0}  -  {:?}", cfg.radius, cfg.hg_type);
        let card = spawn_astre_card(commands, &format!("Hypergeante {}", i+1), TargetKind::Hypergiant(i), STAR_COLOR, &info);
        commands.entity(content).add_child(card);
    }
    let btn_hg = spawn_astre_regen_button(commands, "Regenerer hypergeantes", RegenAstreAction::Hypergiant, STAR_COLOR);
    commands.entity(content).add_child(btn_hg);
    add_astre_add_button(commands, content, "une hypergeante", AstreFamily::Hypergiant, STAR_COLOR);
    }

    let tabs = spawn_astres_tabs(commands, active_tab);
    commands.entity(root).add_children(&[title, tabs, content]);
}

fn spawn_astres_tabs(commands: &mut Commands, active: AstresTab) -> Entity {
    let bar = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            column_gap: Val::Px(6.0),
            ..default()
        })
        .id();

    for (tab, label, color) in [
        (AstresTab::Planets, "Planetes", PLANET_COLOR),
        (AstresTab::Stars, "Etoiles", STAR_COLOR),
        (AstresTab::Remnants, "Remanents", REMNANT_COLOR),
    ] {
        let selected = tab == active;
        let button = commands
            .spawn((
                Node {
                    flex_grow: 1.0,
                    justify_content: JustifyContent::Center,
                    padding: UiRect::axes(Val::Px(4.0), Val::Px(9.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(if selected { color.with_alpha(0.22) } else { BG_PANEL }),
                BorderColor(if selected { color } else { BG_SLIDER }),
                BorderRadius::all(Val::Px(4.0)),
                Button,
                AstresTabButton(tab),
            ))
            .with_child((
                Text::new(label),
                TextFont { font_size: 12.0, ..default() },
                TextColor(if selected { color } else { TEXT_DIM }),
            ))
            .id();
        commands.entity(bar).add_child(button);
    }

    bar
}

fn spawn_astre_card(
    commands: &mut Commands,
    title: &str,
    target: TargetKind,
    color: Color,
    info: &str,
) -> Entity {
    let card = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(BG_CARD),
            BorderColor(color),
            BorderRadius::all(Val::Px(8.0)),
        ))
        .id();

    let header = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
            ..default()
        })
        .id();

    let left = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                padding: UiRect::axes(Val::Px(4.0), Val::Px(4.0)),
                ..default()
            },
            Button,
            CenterButton(target),
        ))
        .id();

    let title_text = commands
        .spawn((
            Text::new(title.to_string()),
            TextFont {
                font_size: 14.0,
                ..default()
            },
            TextColor(color),
        ))
        .id();

    let info_text = commands
        .spawn((
            Text::new(info.to_string()),
            TextFont {
                font_size: 11.0,
                ..default()
            },
            TextColor(TEXT_DIM),
        ))
        .id();

    commands.entity(left).add_children(&[title_text, info_text]);

    let remove_btn = spawn_astre_edit_button(
        commands,
        "Supprimer",
        AstreEditAction::Remove(target),
        RED_SOFT,
    );
    let actions = commands
        .spawn(Node {
            align_items: AlignItems::Center,
            column_gap: Val::Px(5.0),
            ..default()
        })
        .id();
    commands.entity(actions).add_child(remove_btn);
    commands.entity(header).add_children(&[left, actions]);
    commands.entity(card).add_child(header);

    let controls = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            justify_content: JustifyContent::SpaceEvenly,
            padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
            column_gap: Val::Px(5.0),
            ..default()
        })
        .id();
    for (label, action) in [
        ("Orbite -", AstreEditAction::AdjustOrbit(target, -50.0)),
        ("Orbite +", AstreEditAction::AdjustOrbit(target, 50.0)),
        ("Taille -", AstreEditAction::AdjustSize(target, -5.0)),
        ("Taille +", AstreEditAction::AdjustSize(target, 5.0)),
    ] {
        let button = spawn_astre_edit_button(commands, label, action, color);
        commands.entity(controls).add_child(button);
    }
    
    // Additional controls for comets
    if let TargetKind::Comet(_) = target {
        let comet_controls = commands
            .spawn(Node {
                width: Val::Percent(100.0),
                justify_content: JustifyContent::SpaceEvenly,
                padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
                column_gap: Val::Px(5.0),
                ..default()
            })
            .id();
        
        for (label, action) in [
            ("Coma -", AstreEditAction::AdjustComa(target, -10.0)),
            ("Coma +", AstreEditAction::AdjustComa(target, 10.0)),
            ("Queue -", AstreEditAction::AdjustDustTail(target, -50.0)),
            ("Queue +", AstreEditAction::AdjustDustTail(target, 50.0)),
        ] {
            let button = spawn_astre_edit_button(commands, label, action, color);
            commands.entity(comet_controls).add_child(button);
        }
        commands.entity(card).add_child(comet_controls);
    }
    
    // Additional controls for comets
    if let TargetKind::Comet(_) = target {
        let comet_controls = commands
            .spawn(Node {
                width: Val::Percent(100.0),
                justify_content: JustifyContent::SpaceEvenly,
                padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
                column_gap: Val::Px(5.0),
                ..default()
            })
            .id();
        
        for (label, action) in [
            ("Coma -", AstreEditAction::AdjustComa(target, -10.0)),
            ("Coma +", AstreEditAction::AdjustComa(target, 10.0)),
            ("Queue -", AstreEditAction::AdjustDustTail(target, -50.0)),
            ("Queue +", AstreEditAction::AdjustDustTail(target, 50.0)),
        ] {
            let button = spawn_astre_edit_button(commands, label, action, color);
            commands.entity(comet_controls).add_child(button);
        }
        commands.entity(card).add_child(comet_controls);
    }
    commands.entity(card).add_child(controls);

    card
}

fn spawn_astre_edit_button(
    commands: &mut Commands,
    label: &str,
    action: AstreEditAction,
    color: Color,
) -> Entity {
    commands
        .spawn((
            Node {
                flex_grow: 1.0,
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(Val::Px(5.0), Val::Px(5.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(BG_DARK),
            BorderColor(color.with_alpha(0.7)),
            BorderRadius::all(Val::Px(4.0)),
            Button,
            action,
        ))
        .with_child((
            Text::new(label.to_string()),
            TextFont { font_size: 10.0, ..default() },
            TextColor(color),
        ))
        .id()
}

fn add_astre_add_button(
    commands: &mut Commands,
    parent: Entity,
    label: &str,
    family: AstreFamily,
    color: Color,
) {
    let button = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::axes(Val::Px(0.0), Val::Px(9.0)),
                margin: UiRect::vertical(Val::Px(4.0)),
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            BorderColor(color.with_alpha(0.7)),
            BorderRadius::all(Val::Px(6.0)),
            Button,
            AstreEditAction::Add(family),
        ))
        .with_child((
            Text::new(format!("+ {}", label)),
            TextFont { font_size: 12.0, ..default() },
            TextColor(color),
        ))
        .id();
    commands.entity(parent).add_child(button);
}

fn astres_save_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("spacespore");
    fs::create_dir_all(&path).ok();
    path.push("astres.json");
    path
}

fn restore_saved_astres<T, D, A>(saved: &[SavedAstre], configs: &mut Vec<T>, mut default_config: D, mut apply: A)
where
    D: FnMut() -> T,
    A: FnMut(&mut T, SavedAstre),
{
    while configs.len() < saved.len() {
        configs.push(default_config());
    }
    configs.truncate(saved.len());
    for (config, saved) in configs.iter_mut().zip(saved.iter().copied()) {
        apply(config, saved);
    }
}

fn save_astres<T, F>(configs: &[T], mut values: F) -> Vec<SavedAstre>
where
    F: FnMut(&T) -> SavedAstre,
{
    configs.iter().map(&mut values).collect()
}

fn load_saved_astres(mut res: AstresMutableResources) {
    let Ok(contents) = fs::read_to_string(astres_save_path()) else { return; };
    let Ok(saved) = serde_json::from_str::<SavedAstres>(&contents) else { return; };

    restore_saved_astres(&saved.gas_planets, &mut res.gas_res.planets, Default::default, |c, s| { c.orbit_distance = s.orbit; c.radius = s.size; });
    restore_saved_astres(&saved.comets, &mut res.comet_res.comets, Default::default, |c, s| { c.perihelion = s.orbit; c.aphelion = (c.aphelion - c.perihelion + s.orbit).max(c.perihelion + 1.0); c.nucleus_radius = s.size; });
    restore_saved_astres(&saved.meteoroids, &mut res.meteor_res.meteoroids, Default::default, |c, s| { c.position.x = s.orbit; c.radius = s.size; });
    restore_saved_astres(&saved.voxel_stars, &mut res.vstar_res.stars, Default::default, |c, s| { c.orbit_distance = s.orbit; c.radius = s.size; });
    restore_saved_astres(&saved.protostars, &mut res.proto_res.stars, Default::default, |c, s| { c.orbit_distance = s.orbit; c.radius = s.size; });
    restore_saved_astres(&saved.dwarf_stars, &mut res.dwarf_res.stars, || crate::astre::etoile::dwarf_star::DwarfStarConfig::red(42, Vec3::ZERO), |c, s| { c.orbit_distance = s.orbit; c.radius = s.size; });
    restore_saved_astres(&saved.main_sequence, &mut res.ms_res.stars, Default::default, |c, s| { c.orbit_distance = s.orbit; c.base_radius = s.size; });
    restore_saved_astres(&saved.giants, &mut res.giant_res.stars, Default::default, |c, s| { c.orbit_distance = s.orbit; c.radius = s.size; });
    restore_saved_astres(&saved.supergiants, &mut res.sg_res.stars, Default::default, |c, s| { c.orbit_distance = s.orbit; c.radius = s.size; });
    restore_saved_astres(&saved.hypergiants, &mut res.hg_res.stars, Default::default, |c, s| { c.orbit_distance = s.orbit; c.radius = s.size; });
    restore_saved_astres(&saved.black_holes, &mut res.black_res.holes, Default::default, |c, s| { c.orbit_distance = s.orbit; c.event_horizon = s.size; });
    restore_saved_astres(&saved.pulsars, &mut res.pulsar_res.pulsars, Default::default, |c, s| { c.orbit_distance = s.orbit; c.radius = s.size; });
    restore_saved_astres(&saved.magnetars, &mut res.magnetar_res.magnetars, Default::default, |c, s| { c.orbit_distance = s.orbit; c.radius = s.size; });
    restore_saved_astres(&saved.neutron_stars, &mut res.neutron_res.stars, Default::default, |c, s| { c.orbit_distance = s.orbit; c.radius = s.size; });
    restore_saved_astres(&saved.supernovae, &mut res.sn_res.supernovae, || crate::astre::Remnant_stellaire::supernova::SupernovaConfig::type_ii(42, Vec3::ZERO), |c, s| { c.orbit_distance = s.orbit; c.progenitor_radius = s.size; });
    res.nebula_res.enabled = saved.nebula_enabled;
    res.nebula_res.config.position.x = saved.nebula.orbit;
    res.nebula_res.config.radius = saved.nebula.size;
}

fn save_current_astres(res: &AstresMutableResources) {
    let saved = SavedAstres {
        gas_planets: save_astres(&res.gas_res.planets, |c| SavedAstre { orbit: c.orbit_distance, size: c.radius }),
        comets: save_astres(&res.comet_res.comets, |c| SavedAstre { orbit: c.perihelion, size: c.nucleus_radius }),
        meteoroids: save_astres(&res.meteor_res.meteoroids, |c| SavedAstre { orbit: c.position.x, size: c.radius }),
        voxel_stars: save_astres(&res.vstar_res.stars, |c| SavedAstre { orbit: c.orbit_distance, size: c.radius }),
        protostars: save_astres(&res.proto_res.stars, |c| SavedAstre { orbit: c.orbit_distance, size: c.radius }),
        dwarf_stars: save_astres(&res.dwarf_res.stars, |c| SavedAstre { orbit: c.orbit_distance, size: c.radius }),
        main_sequence: save_astres(&res.ms_res.stars, |c| SavedAstre { orbit: c.orbit_distance, size: c.base_radius }),
        giants: save_astres(&res.giant_res.stars, |c| SavedAstre { orbit: c.orbit_distance, size: c.radius }),
        supergiants: save_astres(&res.sg_res.stars, |c| SavedAstre { orbit: c.orbit_distance, size: c.radius }),
        hypergiants: save_astres(&res.hg_res.stars, |c| SavedAstre { orbit: c.orbit_distance, size: c.radius }),
        black_holes: save_astres(&res.black_res.holes, |c| SavedAstre { orbit: c.orbit_distance, size: c.event_horizon }),
        pulsars: save_astres(&res.pulsar_res.pulsars, |c| SavedAstre { orbit: c.orbit_distance, size: c.radius }),
        magnetars: save_astres(&res.magnetar_res.magnetars, |c| SavedAstre { orbit: c.orbit_distance, size: c.radius }),
        neutron_stars: save_astres(&res.neutron_res.stars, |c| SavedAstre { orbit: c.orbit_distance, size: c.radius }),
        supernovae: save_astres(&res.sn_res.supernovae, |c| SavedAstre { orbit: c.orbit_distance, size: c.progenitor_radius }),
        nebula_enabled: res.nebula_res.enabled,
        nebula: SavedAstre { orbit: res.nebula_res.config.position.x, size: res.nebula_res.config.radius },
    };
    if let Ok(json) = serde_json::to_string_pretty(&saved) {
        fs::write(astres_save_path(), json).ok();
    }
}

fn spawn_astre_regen_button(
    commands: &mut Commands,
    label: &str,
    action: RegenAstreAction,
    color: Color,
) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::axes(Val::Px(0.0), Val::Px(10.0)),
                margin: UiRect::vertical(Val::Px(4.0)),
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(BG_PANEL),
            BorderColor(color),
            BorderRadius::all(Val::Px(8.0)),
            Button,
            action,
        ))
        .with_child((
            Text::new(label.to_string()),
            TextFont {
                font_size: 14.0,
                ..default()
            },
            TextColor(TEXT_COLOR),
        ))
        .id()
}

fn handle_astre_edit_actions(
    interactions: Query<(&Interaction, &AstreEditAction), Changed<Interaction>>,
    mut res: AstresMutableResources,
    mut events: RegenEvents,
    mut rebuild: EventWriter<RebuildUi>,
) {
    for (interaction, action) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }

        match *action {
            AstreEditAction::Add(family) => match family {
                AstreFamily::GasPlanet => { res.gas_res.planets.push(Default::default()); events.gas_events.send(RegenerateGasPlanet); }
                AstreFamily::Comet => { res.comet_res.comets.push(Default::default()); events.comet_events.send(RegenerateComet); }
                AstreFamily::Meteoroid => { res.meteor_res.meteoroids.push(Default::default()); events.meteor_events.send(RegenerateMeteoroid); }
                AstreFamily::VoxelStar => { res.vstar_res.stars.push(Default::default()); events.vstar_events.send(RegenerateVoxelStar); }
                AstreFamily::Protostar => { res.proto_res.stars.push(Default::default()); events.proto_events.send(RegenerateProtostar); }
                AstreFamily::DwarfStar => { res.dwarf_res.stars.push(crate::astre::etoile::dwarf_star::DwarfStarConfig::red(42, Vec3::ZERO)); events.dwarf_events.send(RegenerateDwarfStar); }
                AstreFamily::MainSequence => { res.ms_res.stars.push(Default::default()); events.ms_events.send(RegenerateMainSequence); }
                AstreFamily::GiantStar => { res.giant_res.stars.push(Default::default()); events.giant_events.send(RegenerateGiantStar); }
                AstreFamily::Supergiant => { res.sg_res.stars.push(Default::default()); events.sg_events.send(RegenerateSupergiant); }
                AstreFamily::Hypergiant => { res.hg_res.stars.push(Default::default()); events.hg_events.send(RegenerateHypergiant); }
                AstreFamily::Nebula => { res.nebula_res.enabled = true; events.nebula_events.send(RegenerateNebula); }
                AstreFamily::BlackHole => { res.black_res.holes.push(Default::default()); events.black_events.send(RegenerateBlackHole); }
                AstreFamily::Pulsar => { res.pulsar_res.pulsars.push(Default::default()); events.pulsar_events.send(RegeneratePulsar); }
                AstreFamily::Magnetar => { res.magnetar_res.magnetars.push(Default::default()); events.mag_events.send(RegenerateMagnetar); }
                AstreFamily::NeutronStar => { res.neutron_res.stars.push(Default::default()); events.neutron_events.send(RegenerateNeutronStar); }
                AstreFamily::Supernova => { res.sn_res.supernovae.push(crate::astre::Remnant_stellaire::supernova::SupernovaConfig::type_ii(42, Vec3::ZERO)); events.sn_events.send(RegenerateSupernova); }
            },
            AstreEditAction::Remove(target) => {
                match target {
                    TargetKind::GasPlanet(i) => { if i < res.gas_res.planets.len() { res.gas_res.planets.remove(i); } events.gas_events.send(RegenerateGasPlanet); }
                    TargetKind::Comet(i) => { if i < res.comet_res.comets.len() { res.comet_res.comets.remove(i); } events.comet_events.send(RegenerateComet); }
                    TargetKind::Meteoroid(i) => { if i < res.meteor_res.meteoroids.len() { res.meteor_res.meteoroids.remove(i); } events.meteor_events.send(RegenerateMeteoroid); }
                    TargetKind::VoxelStar(i) => { if i < res.vstar_res.stars.len() { res.vstar_res.stars.remove(i); } events.vstar_events.send(RegenerateVoxelStar); }
                    TargetKind::Protostar(i) => { if i < res.proto_res.stars.len() { res.proto_res.stars.remove(i); } events.proto_events.send(RegenerateProtostar); }
                    TargetKind::DwarfStar(i) => { if i < res.dwarf_res.stars.len() { res.dwarf_res.stars.remove(i); } events.dwarf_events.send(RegenerateDwarfStar); }
                    TargetKind::MainSequence(i) => { if i < res.ms_res.stars.len() { res.ms_res.stars.remove(i); } events.ms_events.send(RegenerateMainSequence); }
                    TargetKind::GiantStar(i) => { if i < res.giant_res.stars.len() { res.giant_res.stars.remove(i); } events.giant_events.send(RegenerateGiantStar); }
                    TargetKind::Supergiant(i) => { if i < res.sg_res.stars.len() { res.sg_res.stars.remove(i); } events.sg_events.send(RegenerateSupergiant); }
                    TargetKind::Hypergiant(i) => { if i < res.hg_res.stars.len() { res.hg_res.stars.remove(i); } events.hg_events.send(RegenerateHypergiant); }
                    TargetKind::BlackHole(i) => { if i < res.black_res.holes.len() { res.black_res.holes.remove(i); } events.black_events.send(RegenerateBlackHole); }
                    TargetKind::Pulsar(i) => { if i < res.pulsar_res.pulsars.len() { res.pulsar_res.pulsars.remove(i); } events.pulsar_events.send(RegeneratePulsar); }
                    TargetKind::Magnetar(i) => { if i < res.magnetar_res.magnetars.len() { res.magnetar_res.magnetars.remove(i); } events.mag_events.send(RegenerateMagnetar); }
                    TargetKind::NeutronStar(i) => { if i < res.neutron_res.stars.len() { res.neutron_res.stars.remove(i); } events.neutron_events.send(RegenerateNeutronStar); }
                    TargetKind::Supernova(i) => { if i < res.sn_res.supernovae.len() { res.sn_res.supernovae.remove(i); } events.sn_events.send(RegenerateSupernova); }
                    TargetKind::Nebula => { res.nebula_res.enabled = false; events.nebula_events.send(RegenerateNebula); }
                    _ => {}
                }
            }
            AstreEditAction::AdjustOrbit(target, delta) => {
                match target {
                    TargetKind::GasPlanet(i) => if let Some(c) = res.gas_res.planets.get_mut(i) { c.orbit_distance = (c.orbit_distance + delta).max(0.0); events.gas_events.send(RegenerateGasPlanet); },
                    TargetKind::Comet(i) => if let Some(c) = res.comet_res.comets.get_mut(i) { c.perihelion = (c.perihelion + delta).max(1.0); c.aphelion = (c.aphelion + delta).max(c.perihelion + 1.0); events.comet_events.send(RegenerateComet); },
                    TargetKind::Meteoroid(i) => if let Some(c) = res.meteor_res.meteoroids.get_mut(i) { c.position.x = (c.position.x + delta).max(0.0); events.meteor_events.send(RegenerateMeteoroid); },
                    TargetKind::VoxelStar(i) => if let Some(c) = res.vstar_res.stars.get_mut(i) { c.orbit_distance = (c.orbit_distance + delta).max(0.0); events.vstar_events.send(RegenerateVoxelStar); },
                    TargetKind::Protostar(i) => if let Some(c) = res.proto_res.stars.get_mut(i) { c.orbit_distance = (c.orbit_distance + delta).max(0.0); events.proto_events.send(RegenerateProtostar); },
                    TargetKind::DwarfStar(i) => if let Some(c) = res.dwarf_res.stars.get_mut(i) { c.orbit_distance = (c.orbit_distance + delta).max(0.0); events.dwarf_events.send(RegenerateDwarfStar); },
                    TargetKind::MainSequence(i) => if let Some(c) = res.ms_res.stars.get_mut(i) { c.orbit_distance = (c.orbit_distance + delta).max(0.0); events.ms_events.send(RegenerateMainSequence); },
                    TargetKind::GiantStar(i) => if let Some(c) = res.giant_res.stars.get_mut(i) { c.orbit_distance = (c.orbit_distance + delta).max(0.0); events.giant_events.send(RegenerateGiantStar); },
                    TargetKind::Supergiant(i) => if let Some(c) = res.sg_res.stars.get_mut(i) { c.orbit_distance = (c.orbit_distance + delta).max(0.0); events.sg_events.send(RegenerateSupergiant); },
                    TargetKind::Hypergiant(i) => if let Some(c) = res.hg_res.stars.get_mut(i) { c.orbit_distance = (c.orbit_distance + delta).max(0.0); events.hg_events.send(RegenerateHypergiant); },
                    TargetKind::BlackHole(i) => if let Some(c) = res.black_res.holes.get_mut(i) { c.orbit_distance = (c.orbit_distance + delta).max(0.0); events.black_events.send(RegenerateBlackHole); },
                    TargetKind::Pulsar(i) => if let Some(c) = res.pulsar_res.pulsars.get_mut(i) { c.orbit_distance = (c.orbit_distance + delta).max(0.0); events.pulsar_events.send(RegeneratePulsar); },
                    TargetKind::Magnetar(i) => if let Some(c) = res.magnetar_res.magnetars.get_mut(i) { c.orbit_distance = (c.orbit_distance + delta).max(0.0); events.mag_events.send(RegenerateMagnetar); },
                    TargetKind::NeutronStar(i) => if let Some(c) = res.neutron_res.stars.get_mut(i) { c.orbit_distance = (c.orbit_distance + delta).max(0.0); events.neutron_events.send(RegenerateNeutronStar); },
                    TargetKind::Supernova(i) => if let Some(c) = res.sn_res.supernovae.get_mut(i) { c.orbit_distance = (c.orbit_distance + delta).max(0.0); events.sn_events.send(RegenerateSupernova); },
                    TargetKind::Nebula => { res.nebula_res.config.position.x = (res.nebula_res.config.position.x + delta).max(0.0); events.nebula_events.send(RegenerateNebula); },
                    _ => {}
                }
            }
            AstreEditAction::AdjustSize(target, delta) => {
                match target {
                    TargetKind::GasPlanet(i) => if let Some(c) = res.gas_res.planets.get_mut(i) { c.radius = (c.radius + delta).max(1.0); events.gas_events.send(RegenerateGasPlanet); },
                    TargetKind::Comet(i) => if let Some(c) = res.comet_res.comets.get_mut(i) { c.nucleus_radius = (c.nucleus_radius + delta).max(1.0); events.comet_events.send(RegenerateComet); },
                    TargetKind::Meteoroid(i) => if let Some(c) = res.meteor_res.meteoroids.get_mut(i) { c.radius = (c.radius + delta).max(1.0); events.meteor_events.send(RegenerateMeteoroid); },
                    TargetKind::VoxelStar(i) => if let Some(c) = res.vstar_res.stars.get_mut(i) { c.radius = (c.radius + delta).max(1.0); events.vstar_events.send(RegenerateVoxelStar); },
                    TargetKind::Protostar(i) => if let Some(c) = res.proto_res.stars.get_mut(i) { c.radius = (c.radius + delta).max(1.0); events.proto_events.send(RegenerateProtostar); },
                    TargetKind::DwarfStar(i) => if let Some(c) = res.dwarf_res.stars.get_mut(i) { c.radius = (c.radius + delta).max(1.0); events.dwarf_events.send(RegenerateDwarfStar); },
                    TargetKind::MainSequence(i) => if let Some(c) = res.ms_res.stars.get_mut(i) { c.base_radius = (c.base_radius + delta).max(1.0); events.ms_events.send(RegenerateMainSequence); },
                    TargetKind::GiantStar(i) => if let Some(c) = res.giant_res.stars.get_mut(i) { c.radius = (c.radius + delta).max(1.0); events.giant_events.send(RegenerateGiantStar); },
                    TargetKind::Supergiant(i) => if let Some(c) = res.sg_res.stars.get_mut(i) { c.radius = (c.radius + delta).max(1.0); events.sg_events.send(RegenerateSupergiant); },
                    TargetKind::Hypergiant(i) => if let Some(c) = res.hg_res.stars.get_mut(i) { c.radius = (c.radius + delta).max(1.0); events.hg_events.send(RegenerateHypergiant); },
                    TargetKind::BlackHole(i) => if let Some(c) = res.black_res.holes.get_mut(i) { c.event_horizon = (c.event_horizon + delta).max(1.0); events.black_events.send(RegenerateBlackHole); },
                    TargetKind::Pulsar(i) => if let Some(c) = res.pulsar_res.pulsars.get_mut(i) { c.radius = (c.radius + delta).max(1.0); events.pulsar_events.send(RegeneratePulsar); },
                    TargetKind::Magnetar(i) => if let Some(c) = res.magnetar_res.magnetars.get_mut(i) { c.radius = (c.radius + delta).max(1.0); events.mag_events.send(RegenerateMagnetar); },
                    TargetKind::NeutronStar(i) => if let Some(c) = res.neutron_res.stars.get_mut(i) { c.radius = (c.radius + delta).max(1.0); events.neutron_events.send(RegenerateNeutronStar); },
                    TargetKind::Supernova(i) => if let Some(c) = res.sn_res.supernovae.get_mut(i) { c.progenitor_radius = (c.progenitor_radius + delta).max(1.0); events.sn_events.send(RegenerateSupernova); },
                    TargetKind::Nebula => { res.nebula_res.config.radius = (res.nebula_res.config.radius + delta).max(1.0); events.nebula_events.send(RegenerateNebula); },
                    _ => {}
                }
            }
            AstreEditAction::AdjustComa(target, delta) => {
                match target {
                    TargetKind::Comet(i) => if let Some(c) = res.comet_res.comets.get_mut(i) { c.coma_radius = (c.coma_radius + delta).max(1.0); events.comet_events.send(RegenerateComet); },
                    TargetKind::Meteoroid(i) => if let Some(c) = res.meteor_res.meteoroids.get_mut(i) { c.radius = (c.radius + delta).max(1.0); events.meteor_events.send(RegenerateMeteoroid); },
                    _ => {}
                }
            }
            AstreEditAction::AdjustDustTail(target, delta) => {
                match target {
                    TargetKind::Comet(i) => if let Some(c) = res.comet_res.comets.get_mut(i) { c.dust_tail_length = (c.dust_tail_length + delta).max(0.0); events.comet_events.send(RegenerateComet); },
                    TargetKind::Meteoroid(i) => if let Some(c) = res.meteor_res.meteoroids.get_mut(i) { c.radius = (c.radius + delta).max(1.0); events.meteor_events.send(RegenerateMeteoroid); },
                    _ => {}
                }
            }
        }
        save_current_astres(&res);
        rebuild.send(RebuildUi);
    }
}

fn handle_astre_regen_buttons(
    interactions: Query<(&Interaction, &RegenAstreAction), Changed<Interaction>>,
    mut events: RegenEvents, // Remplace les 16 EventWriter par cette structure
) {
    for (interaction, action) in &interactions {
        if *interaction != Interaction::Pressed { continue; }
        match action {
            RegenAstreAction::GasPlanet   => { events.gas_events.send(RegenerateGasPlanet); }
            RegenAstreAction::Comet       => { events.comet_events.send(RegenerateComet); }
            RegenAstreAction::Meteoroid   => { events.meteor_events.send(RegenerateMeteoroid); }
            RegenAstreAction::VoxelStar   => { events.vstar_events.send(RegenerateVoxelStar); }
            RegenAstreAction::Protostar   => { events.proto_events.send(RegenerateProtostar); }
            RegenAstreAction::DwarfStar   => { events.dwarf_events.send(RegenerateDwarfStar); }
            RegenAstreAction::MainSequence=> { events.ms_events.send(RegenerateMainSequence); }
            RegenAstreAction::GiantStar   => { events.giant_events.send(RegenerateGiantStar); }
            RegenAstreAction::Supergiant  => { events.sg_events.send(RegenerateSupergiant); }
            RegenAstreAction::Hypergiant  => { events.hg_events.send(RegenerateHypergiant); }
            RegenAstreAction::Nebula      => { events.nebula_events.send(RegenerateNebula); }
            RegenAstreAction::BlackHole   => { events.black_events.send(RegenerateBlackHole); }
            RegenAstreAction::Pulsar      => { events.pulsar_events.send(RegeneratePulsar); }
            RegenAstreAction::Magnetar    => { events.mag_events.send(RegenerateMagnetar); }
            RegenAstreAction::NeutronStar => { events.neutron_events.send(RegenerateNeutronStar); }
            RegenAstreAction::Supernova   => { events.sn_events.send(RegenerateSupernova); }
        }
    }
}