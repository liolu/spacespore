use bevy::input::mouse::MouseWheel;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowRef};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::planet::RegeneratePlanet;
use crate::settings::{AsteroidBeltConfig, GameSettings, PlanetConfig, StarConfig};
use crate::astre::{AstreLodRoot, AstreUnloaded};
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

#[derive(Component)]
struct RadarWindowCam;

#[derive(Component)]
struct RadarRoot;

#[derive(Component)]
struct RadarArea;

#[derive(Component)]
struct RadarDot(Entity);

#[derive(Component)]
struct RadarCamIndicator;

#[derive(Component)]
struct RadarCamDir;

#[derive(Component)]
struct RadarInfoText;

#[derive(Resource, Clone, Copy, PartialEq, Eq)]
enum AstresTab {
    Liste,
    Editer,
    Ajouter,
}

#[derive(Resource, Default)]
struct SelectedAstre(Option<TargetKind>);

#[derive(Component, Clone, Copy)]
struct SelectAstre(TargetKind);

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
            .insert_resource(AstresTabState { active: AstresTab::Liste })
            .insert_resource(SelectedAstre::default())
            .add_event::<RebuildUi>()
            .add_systems(
                Startup,
                (setup_game_ui, setup_astres_window, setup_radar_window),
            )
            .add_systems(
                Update,
                (
                    toggle_menu,
                    handle_options_button,
                    handle_astres_tabs,
                    update_menu_visibility,
                    handle_slider_interactions,
                    handle_astre_sliders,
                    handle_toggle_button,
                    handle_toggle_show_light,
                    handle_toggle_show_orbits,
                    handle_toggle_show_systems,
                    handle_toggle_atmosphere,
                    handle_apply_button,
                    handle_center_buttons,
                    handle_select_astre,
                ),
            )
            .add_systems(
                Update,
                (
                    update_slider_visuals,
                    update_astre_slider_visuals,
                    handle_body_actions,
                    rebuild_astres_ui,
                    scroll_options_panel,
                    handle_astre_regen_buttons,
                    handle_astre_edit_actions,
                    update_radar,
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
    // ── Centre galactique ────────────────────────────────────────────────
    GalacticCore,
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
struct ToggleShowSystems;

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
            Self::PlanetOrbitDistance(i) => s.systems.first().and_then(|sys| sys.planets.get(i)).map(|p| p.orbit_distance).unwrap_or(0.0),
            Self::PlanetRadius(i) => s.systems.first().and_then(|sys| sys.planets.get(i)).map(|p| p.radius).unwrap_or(50.0),
            Self::PlanetSeaLevel(i) => s.systems.first().and_then(|sys| sys.planets.get(i)).map(|p| p.sea_level).unwrap_or(0.4),
            Self::PlanetTerrainHeight(i) => s.systems.first().and_then(|sys| sys.planets.get(i)).map(|p| p.terrain_height).unwrap_or(22.0),
            Self::PlanetSeed(i) => s.systems.first().and_then(|sys| sys.planets.get(i)).map(|p| p.seed as f32).unwrap_or(42.0),
            Self::PlanetNoiseScale(i) => s.systems.first().and_then(|sys| sys.planets.get(i)).map(|p| p.noise_scale).unwrap_or(2.0),
            Self::PlanetDetailScale(i) => s.systems.first().and_then(|sys| sys.planets.get(i)).map(|p| p.detail_scale).unwrap_or(4.0),
            Self::PlanetCloudDensity(i) => s.systems.first().and_then(|sys| sys.planets.get(i)).map(|p| p.cloud_density).unwrap_or(0.5),
            Self::PlanetCloudAltitude(i) => s.systems.first().and_then(|sys| sys.planets.get(i)).map(|p| p.cloud_altitude).unwrap_or(8.0),
            Self::PlanetCloudSpeed(i) => s.systems.first().and_then(|sys| sys.planets.get(i)).map(|p| p.cloud_speed).unwrap_or(0.02),
            Self::StarOrbitDistance(i) => s.systems.first().and_then(|sys| sys.stars.get(i)).map(|st| st.orbit_distance).unwrap_or(0.0),
            Self::StarRadius(i) => s.systems.first().and_then(|sys| sys.stars.get(i)).map(|st| st.radius).unwrap_or(200.0),
            Self::StarIntensity(i) => s.systems.first().and_then(|sys| sys.stars.get(i)).map(|st| st.intensity).unwrap_or(20.0),
            Self::StarLightRange(i) => s.systems.first().and_then(|sys| sys.stars.get(i)).map(|st| st.light_range).unwrap_or(10000.0),
            Self::StarColorR(i) => s.systems.first().and_then(|sys| sys.stars.get(i)).map(|st| st.light_color_r).unwrap_or(1.0),
            Self::StarColorG(i) => s.systems.first().and_then(|sys| sys.stars.get(i)).map(|st| st.light_color_g).unwrap_or(0.92),
            Self::StarColorB(i) => s.systems.first().and_then(|sys| sys.stars.get(i)).map(|st| st.light_color_b).unwrap_or(0.65),
            Self::StarFlareCount(i) => s.systems.first().and_then(|sys| sys.stars.get(i)).map(|st| st.flare_count as f32).unwrap_or(5.0),
            Self::StarFlareHeight(i) => s.systems.first().and_then(|sys| sys.stars.get(i)).map(|st| st.flare_height).unwrap_or(60.0),
            Self::StarFlareSpeed(i) => s.systems.first().and_then(|sys| sys.stars.get(i)).map(|st| st.flare_speed).unwrap_or(1.0),
            Self::StarFlareSize(i) => s.systems.first().and_then(|sys| sys.stars.get(i)).map(|st| st.flare_size).unwrap_or(6.0),
            Self::StarFlareDistance(i) => s.systems.first().and_then(|sys| sys.stars.get(i)).map(|st| st.flare_distance).unwrap_or(0.0),
            Self::BeltDistance(i) => s.systems.first().and_then(|sys| sys.asteroid_belts.get(i)).map(|b| b.distance).unwrap_or(300.0),
            Self::BeltWidth(i) => s.systems.first().and_then(|sys| sys.asteroid_belts.get(i)).map(|b| b.width).unwrap_or(80.0),
            Self::BeltMinSize(i) => s.systems.first().and_then(|sys| sys.asteroid_belts.get(i)).map(|b| b.min_size).unwrap_or(1.0),
            Self::BeltMaxSize(i) => s.systems.first().and_then(|sys| sys.asteroid_belts.get(i)).map(|b| b.max_size).unwrap_or(5.0),
            Self::BeltCount(i) => s.systems.first().and_then(|sys| sys.asteroid_belts.get(i)).map(|b| b.count as f32).unwrap_or(100.0),
            Self::MoonOrbitDistance(pi, mi) => s.systems.first().and_then(|sys| sys.planets.get(pi)).and_then(|p| p.moons.get(mi)).map(|m| m.orbit_distance).unwrap_or(80.0),
            Self::MoonRadius(pi, mi) => s.systems.first().and_then(|sys| sys.planets.get(pi)).and_then(|p| p.moons.get(mi)).map(|m| m.radius).unwrap_or(12.0),
            Self::MoonSeed(pi, mi) => s.systems.first().and_then(|sys| sys.planets.get(pi)).and_then(|p| p.moons.get(mi)).map(|m| m.seed as f32).unwrap_or(77.0),
        }
    }

    fn set(self, s: &mut GameSettings, val: f32) {
        match self {
            Self::MouseSensitivity => s.mouse_sensitivity = val,
            Self::ScrollSpeed => s.scroll_speed = val,
            Self::KeyboardSpeed => s.keyboard_speed = val,
            Self::PlanetOrbitDistance(i) => {
                if let Some(p) = s.systems.first_mut().and_then(|sys| sys.planets.get_mut(i)) { p.orbit_distance = val; }
            }
            Self::PlanetRadius(i) => {
                if let Some(p) = s.systems.first_mut().and_then(|sys| sys.planets.get_mut(i)) { p.radius = val; }
            }
            Self::PlanetSeaLevel(i) => {
                if let Some(p) = s.systems.first_mut().and_then(|sys| sys.planets.get_mut(i)) { p.sea_level = val; }
            }
            Self::PlanetTerrainHeight(i) => {
                if let Some(p) = s.systems.first_mut().and_then(|sys| sys.planets.get_mut(i)) { p.terrain_height = val; }
            }
            Self::PlanetSeed(i) => {
                if let Some(p) = s.systems.first_mut().and_then(|sys| sys.planets.get_mut(i)) { p.seed = val as u32; }
            }
            Self::PlanetNoiseScale(i) => {
                if let Some(p) = s.systems.first_mut().and_then(|sys| sys.planets.get_mut(i)) { p.noise_scale = val; }
            }
            Self::PlanetDetailScale(i) => {
                if let Some(p) = s.systems.first_mut().and_then(|sys| sys.planets.get_mut(i)) { p.detail_scale = val; }
            }
            Self::PlanetCloudDensity(i) => {
                if let Some(p) = s.systems.first_mut().and_then(|sys| sys.planets.get_mut(i)) { p.cloud_density = val; }
            }
            Self::PlanetCloudAltitude(i) => {
                if let Some(p) = s.systems.first_mut().and_then(|sys| sys.planets.get_mut(i)) { p.cloud_altitude = val; }
            }
            Self::PlanetCloudSpeed(i) => {
                if let Some(p) = s.systems.first_mut().and_then(|sys| sys.planets.get_mut(i)) { p.cloud_speed = val; }
            }
            Self::StarOrbitDistance(i) => {
                if let Some(st) = s.systems.first_mut().and_then(|sys| sys.stars.get_mut(i)) { st.orbit_distance = val; }
            }
            Self::StarRadius(i) => {
                if let Some(st) = s.systems.first_mut().and_then(|sys| sys.stars.get_mut(i)) { st.radius = val; }
            }
            Self::StarIntensity(i) => {
                if let Some(st) = s.systems.first_mut().and_then(|sys| sys.stars.get_mut(i)) { st.intensity = val; }
            }
            Self::StarLightRange(i) => {
                if let Some(st) = s.systems.first_mut().and_then(|sys| sys.stars.get_mut(i)) { st.light_range = val; }
            }
            Self::StarColorR(i) => {
                if let Some(st) = s.systems.first_mut().and_then(|sys| sys.stars.get_mut(i)) { st.light_color_r = val; }
            }
            Self::StarColorG(i) => {
                if let Some(st) = s.systems.first_mut().and_then(|sys| sys.stars.get_mut(i)) { st.light_color_g = val; }
            }
            Self::StarColorB(i) => {
                if let Some(st) = s.systems.first_mut().and_then(|sys| sys.stars.get_mut(i)) { st.light_color_b = val; }
            }
            Self::StarFlareCount(i) => {
                if let Some(st) = s.systems.first_mut().and_then(|sys| sys.stars.get_mut(i)) { st.flare_count = val as u32; }
            }
            Self::StarFlareHeight(i) => {
                if let Some(st) = s.systems.first_mut().and_then(|sys| sys.stars.get_mut(i)) { st.flare_height = val; }
            }
            Self::StarFlareSpeed(i) => {
                if let Some(st) = s.systems.first_mut().and_then(|sys| sys.stars.get_mut(i)) { st.flare_speed = val; }
            }
            Self::StarFlareSize(i) => {
                if let Some(st) = s.systems.first_mut().and_then(|sys| sys.stars.get_mut(i)) { st.flare_size = val; }
            }
            Self::StarFlareDistance(i) => {
                if let Some(st) = s.systems.first_mut().and_then(|sys| sys.stars.get_mut(i)) { st.flare_distance = val; }
            }
            Self::BeltDistance(i) => {
                if let Some(b) = s.systems.first_mut().and_then(|sys| sys.asteroid_belts.get_mut(i)) { b.distance = val; }
            }
            Self::BeltWidth(i) => {
                if let Some(b) = s.systems.first_mut().and_then(|sys| sys.asteroid_belts.get_mut(i)) { b.width = val; }
            }
            Self::BeltMinSize(i) => {
                if let Some(b) = s.systems.first_mut().and_then(|sys| sys.asteroid_belts.get_mut(i)) { b.min_size = val; }
            }
            Self::BeltMaxSize(i) => {
                if let Some(b) = s.systems.first_mut().and_then(|sys| sys.asteroid_belts.get_mut(i)) { b.max_size = val; }
            }
            Self::BeltCount(i) => {
                if let Some(b) = s.systems.first_mut().and_then(|sys| sys.asteroid_belts.get_mut(i)) { b.count = val as u32; }
            }
            Self::MoonOrbitDistance(pi, mi) => {
                if let Some(m) = s.systems.first_mut().and_then(|sys| sys.planets.get_mut(pi)).and_then(|p| p.moons.get_mut(mi)) { m.orbit_distance = val; }
            }
            Self::MoonRadius(pi, mi) => {
                if let Some(m) = s.systems.first_mut().and_then(|sys| sys.planets.get_mut(pi)).and_then(|p| p.moons.get_mut(mi)) { m.radius = val; }
            }
            Self::MoonSeed(pi, mi) => {
                if let Some(m) = s.systems.first_mut().and_then(|sys| sys.planets.get_mut(pi)).and_then(|p| p.moons.get_mut(mi)) { m.seed = val as u32; }
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

// ── AstreField: sliders for resource-based astres ──

#[derive(Clone, Copy, PartialEq)]
enum AstreField {
    GasOrbit(usize), GasRadius(usize), GasSeed(usize), GasEcc(usize), GasInc(usize),
    GasBandCount(usize), GasBandEmissive(usize),
    CometPerihelion(usize), CometAphelion(usize), CometNucleus(usize),
    CometComa(usize), CometDustTail(usize), CometIonTail(usize), CometInclination(usize),
    MeteorRadius(usize),
    VStarOrbit(usize), VStarRadius(usize),
    ProtoOrbit(usize), ProtoRadius(usize), ProtoTemp(usize),
    DwarfOrbit(usize), DwarfRadius(usize),
    MSOrbit(usize), MSRadius(usize),
    GiantOrbit(usize), GiantRadius(usize),
    SGOrbit(usize), SGRadius(usize),
    HGOrbit(usize), HGRadius(usize),
    BHOrbit(usize), BHHorizon(usize), BHInfluence(usize),
    PulsarOrbit(usize), PulsarRadius(usize), PulsarPeriod(usize),
    MagOrbit(usize), MagRadius(usize),
    NSOrbit(usize), NSRadius(usize),
    SNOrbit(usize), SNRadius(usize),
    NebRadius, NebSeed,
}

impl AstreField {
    fn label(self) -> &'static str {
        match self {
            Self::GasOrbit(_) | Self::VStarOrbit(_) | Self::ProtoOrbit(_) | Self::DwarfOrbit(_) |
            Self::MSOrbit(_) | Self::GiantOrbit(_) | Self::SGOrbit(_) | Self::HGOrbit(_) |
            Self::BHOrbit(_) | Self::PulsarOrbit(_) | Self::MagOrbit(_) | Self::NSOrbit(_) |
            Self::SNOrbit(_) | Self::CometPerihelion(_) => "Distance orbite",
            Self::GasRadius(_) | Self::VStarRadius(_) | Self::ProtoRadius(_) | Self::DwarfRadius(_) |
            Self::MSRadius(_) | Self::GiantRadius(_) | Self::SGRadius(_) | Self::HGRadius(_) |
            Self::PulsarRadius(_) | Self::MagRadius(_) | Self::NSRadius(_) | Self::SNRadius(_) |
            Self::CometNucleus(_) | Self::MeteorRadius(_) | Self::NebRadius => "Rayon",
            Self::GasSeed(_) | Self::NebSeed => "Graine",
            Self::GasEcc(_) => "Excentricite",
            Self::GasInc(_) | Self::CometInclination(_) => "Inclinaison",
            Self::GasBandCount(_) => "Bandes",
            Self::GasBandEmissive(_) => "Luminosite bandes",
            Self::CometAphelion(_) => "Aphelie",
            Self::CometComa(_) => "Coma",
            Self::CometDustTail(_) => "Queue poussiere",
            Self::CometIonTail(_) => "Queue ion",
            Self::ProtoTemp(_) => "Temperature",
            Self::BHHorizon(_) => "Horizon",
            Self::BHInfluence(_) => "Influence",
            Self::PulsarPeriod(_) => "Periode",
        }
    }

    fn get(self, res: &AstresResources) -> f32 {
        match self {
            Self::GasOrbit(i) => res.gas_res.planets.get(i).map(|c| c.orbit_distance).unwrap_or(0.0),
            Self::GasRadius(i) => res.gas_res.planets.get(i).map(|c| c.radius).unwrap_or(100.0),
            Self::GasSeed(i) => res.gas_res.planets.get(i).map(|c| c.seed as f32).unwrap_or(42.0),
            Self::GasEcc(i) => res.gas_res.planets.get(i).map(|c| c.eccentricity).unwrap_or(0.0),
            Self::GasInc(i) => res.gas_res.planets.get(i).map(|c| c.inclination).unwrap_or(0.0),
            Self::GasBandCount(i) => res.gas_res.planets.get(i).map(|c| c.band_count).unwrap_or(6.0),
            Self::GasBandEmissive(i) => res.gas_res.planets.get(i).map(|c| c.band_emissive).unwrap_or(0.0),
            Self::CometPerihelion(i) => res.comet_res.comets.get(i).map(|c| c.perihelion).unwrap_or(500.0),
            Self::CometAphelion(i) => res.comet_res.comets.get(i).map(|c| c.aphelion).unwrap_or(5000.0),
            Self::CometNucleus(i) => res.comet_res.comets.get(i).map(|c| c.nucleus_radius).unwrap_or(15.0),
            Self::CometComa(i) => res.comet_res.comets.get(i).map(|c| c.coma_radius).unwrap_or(50.0),
            Self::CometDustTail(i) => res.comet_res.comets.get(i).map(|c| c.dust_tail_length).unwrap_or(200.0),
            Self::CometIonTail(i) => res.comet_res.comets.get(i).map(|c| c.ion_tail_length).unwrap_or(300.0),
            Self::CometInclination(i) => res.comet_res.comets.get(i).map(|c| c.inclination).unwrap_or(0.0),
            Self::MeteorRadius(i) => res.meteor_res.meteoroids.get(i).map(|c| c.radius).unwrap_or(5.0),
            Self::VStarOrbit(i) => res.vstar_res.stars.get(i).map(|c| c.orbit_distance).unwrap_or(0.0),
            Self::VStarRadius(i) => res.vstar_res.stars.get(i).map(|c| c.radius).unwrap_or(100.0),
            Self::ProtoOrbit(i) => res.proto_res.stars.get(i).map(|c| c.orbit_distance).unwrap_or(0.0),
            Self::ProtoRadius(i) => res.proto_res.stars.get(i).map(|c| c.radius).unwrap_or(100.0),
            Self::ProtoTemp(i) => res.proto_res.stars.get(i).map(|c| c.temperature).unwrap_or(0.5),
            Self::DwarfOrbit(i) => res.dwarf_res.stars.get(i).map(|c| c.orbit_distance).unwrap_or(0.0),
            Self::DwarfRadius(i) => res.dwarf_res.stars.get(i).map(|c| c.radius).unwrap_or(30.0),
            Self::MSOrbit(i) => res.ms_res.stars.get(i).map(|c| c.orbit_distance).unwrap_or(0.0),
            Self::MSRadius(i) => res.ms_res.stars.get(i).map(|c| c.base_radius).unwrap_or(100.0),
            Self::GiantOrbit(i) => res.giant_res.stars.get(i).map(|c| c.orbit_distance).unwrap_or(0.0),
            Self::GiantRadius(i) => res.giant_res.stars.get(i).map(|c| c.radius).unwrap_or(200.0),
            Self::SGOrbit(i) => res.sg_res.stars.get(i).map(|c| c.orbit_distance).unwrap_or(0.0),
            Self::SGRadius(i) => res.sg_res.stars.get(i).map(|c| c.radius).unwrap_or(400.0),
            Self::HGOrbit(i) => res.hg_res.stars.get(i).map(|c| c.orbit_distance).unwrap_or(0.0),
            Self::HGRadius(i) => res.hg_res.stars.get(i).map(|c| c.radius).unwrap_or(800.0),
            Self::BHOrbit(i) => res.black_res.holes.get(i).map(|c| c.orbit_distance).unwrap_or(0.0),
            Self::BHHorizon(i) => res.black_res.holes.get(i).map(|c| c.event_horizon).unwrap_or(30.0),
            Self::BHInfluence(i) => res.black_res.holes.get(i).map(|c| c.influence_radius).unwrap_or(500.0),
            Self::PulsarOrbit(i) => res.pulsar_res.pulsars.get(i).map(|c| c.orbit_distance).unwrap_or(0.0),
            Self::PulsarRadius(i) => res.pulsar_res.pulsars.get(i).map(|c| c.radius).unwrap_or(15.0),
            Self::PulsarPeriod(i) => res.pulsar_res.pulsars.get(i).map(|c| c.period_initial).unwrap_or(0.033),
            Self::MagOrbit(i) => res.magnetar_res.magnetars.get(i).map(|c| c.orbit_distance).unwrap_or(0.0),
            Self::MagRadius(i) => res.magnetar_res.magnetars.get(i).map(|c| c.radius).unwrap_or(15.0),
            Self::NSOrbit(i) => res.neutron_res.stars.get(i).map(|c| c.orbit_distance).unwrap_or(0.0),
            Self::NSRadius(i) => res.neutron_res.stars.get(i).map(|c| c.radius).unwrap_or(15.0),
            Self::SNOrbit(i) => res.sn_res.supernovae.get(i).map(|c| c.orbit_distance).unwrap_or(0.0),
            Self::SNRadius(i) => res.sn_res.supernovae.get(i).map(|c| c.progenitor_radius).unwrap_or(100.0),
            Self::NebRadius => res.nebula_res.config.radius,
            Self::NebSeed => res.nebula_res.config.seed as f32,
        }
    }

    fn set(self, res: &mut AstresMutableResources, val: f32) {
        match self {
            Self::GasOrbit(i) => if let Some(c) = res.gas_res.planets.get_mut(i) { c.orbit_distance = val; },
            Self::GasRadius(i) => if let Some(c) = res.gas_res.planets.get_mut(i) { c.radius = val; },
            Self::GasSeed(i) => if let Some(c) = res.gas_res.planets.get_mut(i) { c.seed = val as u32; },
            Self::GasEcc(i) => if let Some(c) = res.gas_res.planets.get_mut(i) { c.eccentricity = val; },
            Self::GasInc(i) => if let Some(c) = res.gas_res.planets.get_mut(i) { c.inclination = val; },
            Self::GasBandCount(i) => if let Some(c) = res.gas_res.planets.get_mut(i) { c.band_count = val; },
            Self::GasBandEmissive(i) => if let Some(c) = res.gas_res.planets.get_mut(i) { c.band_emissive = val; },
            Self::CometPerihelion(i) => if let Some(c) = res.comet_res.comets.get_mut(i) { c.perihelion = val; },
            Self::CometAphelion(i) => if let Some(c) = res.comet_res.comets.get_mut(i) { c.aphelion = val; },
            Self::CometNucleus(i) => if let Some(c) = res.comet_res.comets.get_mut(i) { c.nucleus_radius = val; },
            Self::CometComa(i) => if let Some(c) = res.comet_res.comets.get_mut(i) { c.coma_radius = val; },
            Self::CometDustTail(i) => if let Some(c) = res.comet_res.comets.get_mut(i) { c.dust_tail_length = val; },
            Self::CometIonTail(i) => if let Some(c) = res.comet_res.comets.get_mut(i) { c.ion_tail_length = val; },
            Self::CometInclination(i) => if let Some(c) = res.comet_res.comets.get_mut(i) { c.inclination = val; },
            Self::MeteorRadius(i) => if let Some(c) = res.meteor_res.meteoroids.get_mut(i) { c.radius = val; },
            Self::VStarOrbit(i) => if let Some(c) = res.vstar_res.stars.get_mut(i) { c.orbit_distance = val; },
            Self::VStarRadius(i) => if let Some(c) = res.vstar_res.stars.get_mut(i) { c.radius = val; },
            Self::ProtoOrbit(i) => if let Some(c) = res.proto_res.stars.get_mut(i) { c.orbit_distance = val; },
            Self::ProtoRadius(i) => if let Some(c) = res.proto_res.stars.get_mut(i) { c.radius = val; },
            Self::ProtoTemp(i) => if let Some(c) = res.proto_res.stars.get_mut(i) { c.temperature = val; },
            Self::DwarfOrbit(i) => if let Some(c) = res.dwarf_res.stars.get_mut(i) { c.orbit_distance = val; },
            Self::DwarfRadius(i) => if let Some(c) = res.dwarf_res.stars.get_mut(i) { c.radius = val; },
            Self::MSOrbit(i) => if let Some(c) = res.ms_res.stars.get_mut(i) { c.orbit_distance = val; },
            Self::MSRadius(i) => if let Some(c) = res.ms_res.stars.get_mut(i) { c.base_radius = val; },
            Self::GiantOrbit(i) => if let Some(c) = res.giant_res.stars.get_mut(i) { c.orbit_distance = val; },
            Self::GiantRadius(i) => if let Some(c) = res.giant_res.stars.get_mut(i) { c.radius = val; },
            Self::SGOrbit(i) => if let Some(c) = res.sg_res.stars.get_mut(i) { c.orbit_distance = val; },
            Self::SGRadius(i) => if let Some(c) = res.sg_res.stars.get_mut(i) { c.radius = val; },
            Self::HGOrbit(i) => if let Some(c) = res.hg_res.stars.get_mut(i) { c.orbit_distance = val; },
            Self::HGRadius(i) => if let Some(c) = res.hg_res.stars.get_mut(i) { c.radius = val; },
            Self::BHOrbit(i) => if let Some(c) = res.black_res.holes.get_mut(i) { c.orbit_distance = val; },
            Self::BHHorizon(i) => if let Some(c) = res.black_res.holes.get_mut(i) { c.event_horizon = val; },
            Self::BHInfluence(i) => if let Some(c) = res.black_res.holes.get_mut(i) { c.influence_radius = val; },
            Self::PulsarOrbit(i) => if let Some(c) = res.pulsar_res.pulsars.get_mut(i) { c.orbit_distance = val; },
            Self::PulsarRadius(i) => if let Some(c) = res.pulsar_res.pulsars.get_mut(i) { c.radius = val; },
            Self::PulsarPeriod(i) => if let Some(c) = res.pulsar_res.pulsars.get_mut(i) { c.period_initial = val; },
            Self::MagOrbit(i) => if let Some(c) = res.magnetar_res.magnetars.get_mut(i) { c.orbit_distance = val; },
            Self::MagRadius(i) => if let Some(c) = res.magnetar_res.magnetars.get_mut(i) { c.radius = val; },
            Self::NSOrbit(i) => if let Some(c) = res.neutron_res.stars.get_mut(i) { c.orbit_distance = val; },
            Self::NSRadius(i) => if let Some(c) = res.neutron_res.stars.get_mut(i) { c.radius = val; },
            Self::SNOrbit(i) => if let Some(c) = res.sn_res.supernovae.get_mut(i) { c.orbit_distance = val; },
            Self::SNRadius(i) => if let Some(c) = res.sn_res.supernovae.get_mut(i) { c.progenitor_radius = val; },
            Self::NebRadius => res.nebula_res.config.radius = val,
            Self::NebSeed => res.nebula_res.config.seed = val as u32,
        }
    }

    fn regen(self, events: &mut RegenEvents) {
        match self {
            Self::GasOrbit(_) | Self::GasRadius(_) | Self::GasSeed(_) | Self::GasEcc(_) |
            Self::GasInc(_) | Self::GasBandCount(_) | Self::GasBandEmissive(_) => { events.gas_events.send(RegenerateGasPlanet); }
            Self::CometPerihelion(_) | Self::CometAphelion(_) | Self::CometNucleus(_) |
            Self::CometComa(_) | Self::CometDustTail(_) | Self::CometIonTail(_) |
            Self::CometInclination(_) => { events.comet_events.send(RegenerateComet); }
            Self::MeteorRadius(_) => { events.meteor_events.send(RegenerateMeteoroid); }
            Self::VStarOrbit(_) | Self::VStarRadius(_) => { events.vstar_events.send(RegenerateVoxelStar); }
            Self::ProtoOrbit(_) | Self::ProtoRadius(_) | Self::ProtoTemp(_) => { events.proto_events.send(RegenerateProtostar); }
            Self::DwarfOrbit(_) | Self::DwarfRadius(_) => { events.dwarf_events.send(RegenerateDwarfStar); }
            Self::MSOrbit(_) | Self::MSRadius(_) => { events.ms_events.send(RegenerateMainSequence); }
            Self::GiantOrbit(_) | Self::GiantRadius(_) => { events.giant_events.send(RegenerateGiantStar); }
            Self::SGOrbit(_) | Self::SGRadius(_) => { events.sg_events.send(RegenerateSupergiant); }
            Self::HGOrbit(_) | Self::HGRadius(_) => { events.hg_events.send(RegenerateHypergiant); }
            Self::BHOrbit(_) | Self::BHHorizon(_) | Self::BHInfluence(_) => { events.black_events.send(RegenerateBlackHole); }
            Self::PulsarOrbit(_) | Self::PulsarRadius(_) | Self::PulsarPeriod(_) => { events.pulsar_events.send(RegeneratePulsar); }
            Self::MagOrbit(_) | Self::MagRadius(_) => { events.mag_events.send(RegenerateMagnetar); }
            Self::NSOrbit(_) | Self::NSRadius(_) => { events.neutron_events.send(RegenerateNeutronStar); }
            Self::SNOrbit(_) | Self::SNRadius(_) => { events.sn_events.send(RegenerateSupernova); }
            Self::NebRadius | Self::NebSeed => { events.nebula_events.send(RegenerateNebula); }
        }
    }
}

#[derive(Component)]
struct AstreSliderBar {
    field: AstreField,
    min: f32,
    max: f32,
}

#[derive(Component)]
struct AstreSliderFill(AstreField);

#[derive(Component)]
struct AstreSliderLabel(AstreField);

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
    let show_systems = spawn_toggle(
        &mut commands,
        "Afficher chunks",
        settings.show_systems,
        ToggleShowSystems,
    );
    commands.entity(content).add_children(&[s1, s2, s3, invert, show_light, show_orbits, show_systems]);
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
// Layout helpers
// ══════════════════════════════════════════════════════════

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

fn handle_toggle_show_systems(
    interactions: Query<&Interaction, (Changed<Interaction>, With<ToggleShowSystems>)>,
    mut settings: ResMut<GameSettings>,
    mut toggle_q: Query<(&mut BackgroundColor, &mut BorderColor, &mut Node), With<ToggleShowSystems>>,
) {
    for interaction in &interactions {
        if *interaction == Interaction::Pressed {
            settings.show_systems = !settings.show_systems;
            settings.save();
            for (mut bg, mut border, mut node) in &mut toggle_q {
                *bg = BackgroundColor(if settings.show_systems { ACCENT } else { BG_SLIDER });
                *border = BorderColor(if settings.show_systems { ACCENT } else { TEXT_DIM });
                node.justify_content = if settings.show_systems {
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
            if let Some(p) = settings.systems.first_mut().and_then(|sys| sys.planets.get_mut(toggle.0)) {
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
                if let Some(sys) = settings.systems.first_mut() {
                    let n = sys.planets.len();
                    sys.planets.push(PlanetConfig {
                        orbit_distance: 450.0 + n as f32 * 200.0,
                        seed: 42 + n as u32 * 13,
                        ..PlanetConfig::default()
                    });
                }
            }
            BodyAction::RemovePlanet(i) => {
                if let Some(sys) = settings.systems.first_mut() {
                    if sys.planets.len() > 1 && i < sys.planets.len() {
                        sys.planets.remove(i);
                    }
                }
            }
            BodyAction::AddStar => {
                if let Some(sys) = settings.systems.first_mut() {
                    let n = sys.stars.len();
                    sys.stars.push(StarConfig {
                        orbit_distance: 150.0 + n as f32 * 100.0,
                        radius: 80.0,
                        ..StarConfig::default()
                    });
                }
            }
            BodyAction::RemoveStar(i) => {
                if let Some(sys) = settings.systems.first_mut() {
                    if sys.stars.len() > 1 && i < sys.stars.len() {
                        sys.stars.remove(i);
                    }
                }
            }
            BodyAction::AddBelt => {
                if let Some(sys) = settings.systems.first_mut() {
                    sys.asteroid_belts.push(AsteroidBeltConfig {
                        distance: 300.0,
                        ..AsteroidBeltConfig::default()
                    });
                }
            }
            BodyAction::RemoveBelt(i) => {
                if let Some(sys) = settings.systems.first_mut() {
                    if i < sys.asteroid_belts.len() {
                        sys.asteroid_belts.remove(i);
                    }
                }
            }
            BodyAction::AddMoon(pi) => {
                if let Some(planet) = settings.systems.first_mut().and_then(|sys| sys.planets.get_mut(pi)) {
                    let n = planet.moons.len();
                    planet.moons.push(crate::settings::MoonConfig {
                        orbit_distance: 80.0 + n as f32 * 30.0,
                        radius: 12.0,
                        seed: 77 + n as u32 * 11,
                        ..Default::default()
                    });
                }
            }
            BodyAction::RemoveMoon(pi, mi) => {
                if let Some(planet) = settings.systems.first_mut().and_then(|sys| sys.planets.get_mut(pi)) {
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

fn rebuild_astres_ui(
    mut commands: Commands,
    settings: Res<GameSettings>,
    res: AstresResources,
    tab: Res<AstresTabState>,
    selected: Res<SelectedAstre>,
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
    spawn_astres_ui_root(&mut commands, &settings, &res, cam, tab.active, selected.0);
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
    mut scroll_q: Query<&mut ScrollPosition, With<AstresUiRoot>>,
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
        dy -= wheel_lines(ev) * 40.0;
    }
    if dy == 0.0 {
        return;
    }

    for mut scroll_pos in &mut scroll_q {
        scroll_pos.offset_y = (scroll_pos.offset_y + dy).max(0.0);
    }
}
// ══════════════════════════════════════════════════════════
// Fenêtre Astres (unique)
// ══════════════════════════════════════════════════════════

fn setup_astres_window(
    mut commands: Commands,
    settings: Res<GameSettings>,
    res: AstresResources,
    tab: Res<AstresTabState>,
    selected: Res<SelectedAstre>,
) {
    let mut astres_win = Window {
        title: "SpaceSpore - Editeur".into(),
        resolution: (500.0_f32, 900.0_f32).into(),
        position: WindowPosition::Automatic,
        ..default()
    };
    astres_win.set_minimized(true);
    let astres_window = commands.spawn(astres_win).id();
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
    spawn_astres_ui_root(&mut commands, &settings, &res, ui_camera, tab.active, selected.0);
}

fn spawn_astres_ui_root(
    commands:     &mut Commands,
    settings:     &GameSettings,
    res:          &AstresResources,
    cam_entity:   Entity,
    active_tab:   AstresTab,
    selected:     Option<TargetKind>,
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
            AstresUiRoot,
        ))
        .id();

    let title = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::axes(Val::Px(20.0), Val::Px(14.0)),
                justify_content: JustifyContent::Center,
                border: UiRect::bottom(Val::Px(2.0)),
                ..default()
            },
            BorderColor(BG_SLIDER),
        ))
        .with_child((
            Text::new("EDITEUR"),
            TextFont { font_size: 22.0, ..default() },
            TextColor(TEXT_COLOR),
        ))
        .id();

    let tabs = spawn_astres_tabs(commands, active_tab);

    let content = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
            row_gap: Val::Px(8.0),
            ..default()
        })
        .id();

    match active_tab {
        AstresTab::Liste => build_liste_tab(commands, settings, res, content, selected),
        AstresTab::Editer => build_editer_tab(commands, settings, res, content, selected),
        AstresTab::Ajouter => build_ajouter_tab(commands, content),
    }

    commands.entity(root).add_children(&[title, tabs, content]);
}

// ── Liste tab ──

fn spawn_list_item(
    commands: &mut Commands,
    label: &str,
    info: &str,
    target: TargetKind,
    color: Color,
    is_selected: bool,
) -> Entity {
    let bg = if is_selected { color.with_alpha(0.15) } else { BG_CARD };
    let border = if is_selected { color } else { color.with_alpha(0.3) };
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                border: UiRect::left(Val::Px(if is_selected { 4.0 } else { 2.0 })),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(bg),
            BorderColor(border),
            BorderRadius::all(Val::Px(4.0)),
            Button,
            SelectAstre(target),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(label.to_string()),
                TextFont { font_size: 13.0, ..default() },
                TextColor(color),
            ));
            parent.spawn((
                Text::new(info.to_string()),
                TextFont { font_size: 10.0, ..default() },
                TextColor(TEXT_DIM),
            ));
        })
        .id()
}

fn build_liste_tab(
    commands: &mut Commands,
    settings: &GameSettings,
    res: &AstresResources,
    content: Entity,
    selected: Option<TargetKind>,
) {
    let moon_color = Color::srgb(0.6, 0.6, 0.7);

    // Planetes rocheuses (settings)
    let sys = settings.systems.first();
    if let Some(sys) = sys {
        if !sys.planets.is_empty() {
            let cat = spawn_category_header(commands, "PLANETES", PLANET_COLOR);
            commands.entity(content).add_child(cat);
            for (i, p) in sys.planets.iter().enumerate() {
                let info = format!("R {:.0}  Orb {:.0}", p.radius, p.orbit_distance);
                let item = spawn_list_item(commands, &format!("Planete {}", i+1), &info, TargetKind::Planet(i), PLANET_COLOR, selected == Some(TargetKind::Planet(i)));
                commands.entity(content).add_child(item);
                for (mi, m) in p.moons.iter().enumerate() {
                    let minfo = format!("R {:.0}  Orb {:.0}", m.radius, m.orbit_distance);
                    let mitem = spawn_list_item(commands, &format!("  Lune {} (P{})", mi+1, i+1), &minfo, TargetKind::Moon(i, mi), moon_color, selected == Some(TargetKind::Moon(i, mi)));
                    commands.entity(content).add_child(mitem);
                }
            }
        }
        if !sys.stars.is_empty() {
            let cat = spawn_category_header(commands, "ETOILES (settings)", STAR_COLOR);
            commands.entity(content).add_child(cat);
            for (i, s) in sys.stars.iter().enumerate() {
                let info = format!("R {:.0}  Orb {:.0}", s.radius, s.orbit_distance);
                let item = spawn_list_item(commands, &format!("Etoile {}", i+1), &info, TargetKind::Star(i), STAR_COLOR, selected == Some(TargetKind::Star(i)));
                commands.entity(content).add_child(item);
            }
        }
        if !sys.asteroid_belts.is_empty() {
            let cat = spawn_category_header(commands, "CEINTURES", BELT_COLOR);
            commands.entity(content).add_child(cat);
            for (i, _) in sys.asteroid_belts.iter().enumerate() {
                let item = spawn_list_item(commands, &format!("Ceinture {}", i+1), "", TargetKind::Planet(1000+i), BELT_COLOR, false);
                commands.entity(content).add_child(item);
            }
        }
    }

    // Gas planets
    if !res.gas_res.planets.is_empty() {
        let cat = spawn_category_header(commands, "PLANETES GAZEUSES", PLANET_COLOR);
        commands.entity(content).add_child(cat);
        for (i, c) in res.gas_res.planets.iter().enumerate() {
            let info = format!("R {:.0}  Orb {:.0}", c.radius, c.orbit_distance);
            let item = spawn_list_item(commands, &format!("Gazeuse {}", i+1), &info, TargetKind::GasPlanet(i), PLANET_COLOR, selected == Some(TargetKind::GasPlanet(i)));
            commands.entity(content).add_child(item);
        }
    }

    // Comets
    if !res.comet_res.comets.is_empty() {
        let cat = spawn_category_header(commands, "COMETES", PLANET_COLOR);
        commands.entity(content).add_child(cat);
        for (i, c) in res.comet_res.comets.iter().enumerate() {
            let info = format!("Peri {:.0}  Aph {:.0}", c.perihelion, c.aphelion);
            let item = spawn_list_item(commands, &format!("Comete {}", i+1), &info, TargetKind::Comet(i), PLANET_COLOR, selected == Some(TargetKind::Comet(i)));
            commands.entity(content).add_child(item);
        }
    }

    // Meteoroids
    if !res.meteor_res.meteoroids.is_empty() {
        let cat = spawn_category_header(commands, "METEOROIDES", PLANET_COLOR);
        commands.entity(content).add_child(cat);
        for (i, c) in res.meteor_res.meteoroids.iter().enumerate() {
            let info = format!("R {:.0}", c.radius);
            let item = spawn_list_item(commands, &format!("Meteoroide {}", i+1), &info, TargetKind::Meteoroid(i), PLANET_COLOR, selected == Some(TargetKind::Meteoroid(i)));
            commands.entity(content).add_child(item);
        }
    }

    // Stars (resource-based)
    macro_rules! list_stars {
        ($label:expr, $list:expr, $kind:ident, $color:expr, $fmt:expr) => {
            if !$list.is_empty() {
                let cat = spawn_category_header(commands, $label, $color);
                commands.entity(content).add_child(cat);
                for (i, c) in $list.iter().enumerate() {
                    let info = $fmt(c);
                    let item = spawn_list_item(commands, &format!("{} {}", $label, i+1), &info, TargetKind::$kind(i), $color, selected == Some(TargetKind::$kind(i)));
                    commands.entity(content).add_child(item);
                }
            }
        }
    }
    list_stars!("Etoile voxel", res.vstar_res.stars, VoxelStar, STAR_COLOR, |c: &crate::astre::etoile::star::StarConfig| format!("R {:.0}  Orb {:.0}", c.radius, c.orbit_distance));
    list_stars!("Protoetoile", res.proto_res.stars, Protostar, STAR_COLOR, |c: &crate::astre::etoile::protostar::ProtostarConfig| format!("R {:.0}  Orb {:.0}", c.radius, c.orbit_distance));
    list_stars!("Naine", res.dwarf_res.stars, DwarfStar, STAR_COLOR, |c: &crate::astre::etoile::dwarf_star::DwarfStarConfig| format!("R {:.0}  {:?}", c.radius, c.dwarf_type));
    list_stars!("Seq. princ.", res.ms_res.stars, MainSequence, STAR_COLOR, |c: &crate::astre::etoile::main_sequence_star::MainSequenceConfig| format!("{:?}  Orb {:.0}", c.spectral_class, c.orbit_distance));
    list_stars!("Geante", res.giant_res.stars, GiantStar, STAR_COLOR, |c: &crate::astre::etoile::giant_star::GiantStarConfig| format!("R {:.0}  {:?}", c.radius, c.giant_type));
    list_stars!("Supergeante", res.sg_res.stars, Supergiant, STAR_COLOR, |c: &crate::astre::etoile::supergiant_star::SupergiantConfig| format!("R {:.0}  {:?}", c.radius, c.class));
    list_stars!("Hypergeante", res.hg_res.stars, Hypergiant, STAR_COLOR, |c: &crate::astre::etoile::hypergiant_star::HypergiantConfig| format!("R {:.0}  {:?}", c.radius, c.hg_type));

    // Remnants
    if res.nebula_res.enabled {
        let cat = spawn_category_header(commands, "NEBULEUSE", BELT_COLOR);
        commands.entity(content).add_child(cat);
        let info = format!("R {:.0}  Seed {}", res.nebula_res.config.radius, res.nebula_res.config.seed);
        let item = spawn_list_item(commands, "Nebuleuse", &info, TargetKind::Nebula, BELT_COLOR, selected == Some(TargetKind::Nebula));
        commands.entity(content).add_child(item);
    }
    if !res.black_res.holes.is_empty() {
        let cat = spawn_category_header(commands, "TROUS NOIRS", REMNANT_COLOR);
        commands.entity(content).add_child(cat);
        for (i, c) in res.black_res.holes.iter().enumerate() {
            let info = format!("H {:.0}  Inf {:.0}", c.event_horizon, c.influence_radius);
            let item = spawn_list_item(commands, &format!("Trou noir {}", i+1), &info, TargetKind::BlackHole(i), REMNANT_COLOR, selected == Some(TargetKind::BlackHole(i)));
            commands.entity(content).add_child(item);
        }
    }
    list_stars!("Pulsar", res.pulsar_res.pulsars, Pulsar, REMNANT_COLOR, |c: &crate::astre::Remnant_stellaire::pulsar::PulsarConfig| format!("P {:.3}s  Orb {:.0}", c.period_initial, c.orbit_distance));
    list_stars!("Magnetar", res.magnetar_res.magnetars, Magnetar, REMNANT_COLOR, |c: &crate::astre::Remnant_stellaire::magnetar::MagnetarConfig| format!("R {:.0}  Orb {:.0}", c.radius, c.orbit_distance));
    list_stars!("Etoile neutrons", res.neutron_res.stars, NeutronStar, REMNANT_COLOR, |c: &crate::astre::Remnant_stellaire::neutron_star::NeutronStarConfig| format!("R {:.0}  Orb {:.0}", c.radius, c.orbit_distance));
    list_stars!("Supernova", res.sn_res.supernovae, Supernova, REMNANT_COLOR, |c: &crate::astre::Remnant_stellaire::supernova::SupernovaConfig| format!("{:?}  Orb {:.0}", c.sn_type, c.orbit_distance));
}

// ── Editer tab ──

fn spawn_astre_slider(
    commands: &mut Commands,
    field: AstreField,
    value: f32,
    min: f32,
    max: f32,
) -> Entity {
    let frac = ((value - min) / (max - min)).clamp(0.0, 1.0);
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
            Text::new(field.label().to_string()),
            TextFont { font_size: 12.0, ..default() },
            TextColor(TEXT_DIM),
        ))
        .id();

    let val_text = if value == value.floor() && value.abs() < 10000.0 {
        format!("{:.0}", value)
    } else {
        format!("{:.2}", value)
    };

    let val_label = commands
        .spawn((
            Text::new(val_text),
            TextFont { font_size: 12.0, ..default() },
            TextColor(TEXT_COLOR),
            AstreSliderLabel(field),
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
            AstreSliderBar { field, min, max },
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
            AstreSliderFill(field),
        ))
        .id();

    commands.entity(bar).add_child(fill);
    commands.entity(row).add_children(&[label_row, bar]);
    row
}

fn build_editer_tab(
    commands: &mut Commands,
    settings: &GameSettings,
    res: &AstresResources,
    content: Entity,
    selected: Option<TargetKind>,
) {
    let Some(sel) = selected else {
        let msg = commands.spawn((
            Text::new("Selectionnez un astre dans l'onglet Liste"),
            TextFont { font_size: 14.0, ..default() },
            TextColor(TEXT_DIM),
        )).id();
        commands.entity(content).add_child(msg);
        return;
    };

    match sel {
        TargetKind::Planet(i) => {
            let cat = spawn_category_header(commands, &format!("PLANETE {}", i+1), PLANET_COLOR);
            commands.entity(content).add_child(cat);
            let (card, body) = spawn_body_card(commands, settings, &format!("Planete {}", i+1), PLANET_COLOR, BodyAction::RemovePlanet(i), &[
                (SettingKey::PlanetOrbitDistance(i), 100.0, 100000.0),
                (SettingKey::PlanetRadius(i), 20.0, 500.0),
                (SettingKey::PlanetSeaLevel(i), 0.0, 0.7),
                (SettingKey::PlanetTerrainHeight(i), 5.0, 200.0),
                (SettingKey::PlanetSeed(i), 1.0, 999.0),
                (SettingKey::PlanetNoiseScale(i), 0.5, 5.0),
                (SettingKey::PlanetDetailScale(i), 1.0, 10.0),
            ]);
            let has_atmo = settings.systems.first().and_then(|s| s.planets.get(i)).map(|p| p.atmosphere).unwrap_or(false);
            let atmo_toggle = spawn_toggle(commands, "Atmosphere", has_atmo, ToggleAtmosphere(i));
            commands.entity(body).add_child(atmo_toggle);
            if has_atmo {
                for (key, min, max) in [
                    (SettingKey::PlanetCloudDensity(i), 0.0, 1.0),
                    (SettingKey::PlanetCloudAltitude(i), 5.0, 200.0),
                    (SettingKey::PlanetCloudSpeed(i), 0.005, 0.1),
                ] {
                    let s = spawn_slider(commands, settings, key, min, max);
                    commands.entity(body).add_child(s);
                }
            }
            commands.entity(content).add_child(card);
            // Moons
            let moon_color = Color::srgb(0.6, 0.6, 0.7);
            if let Some(p) = settings.systems.first().and_then(|s| s.planets.get(i)) {
                for (mi, _) in p.moons.iter().enumerate() {
                    let (mc, _) = spawn_body_card(commands, settings, &format!("Lune {}", mi+1), moon_color, BodyAction::RemoveMoon(i, mi), &[
                        (SettingKey::MoonOrbitDistance(i, mi), 30.0, 10000.0),
                        (SettingKey::MoonRadius(i, mi), 3.0, 150.0),
                        (SettingKey::MoonSeed(i, mi), 1.0, 999.0),
                    ]);
                    commands.entity(content).add_child(mc);
                }
                let add_moon = spawn_add_button(commands, &format!("Ajouter lune (P{})", i+1), BodyAction::AddMoon(i), moon_color);
                commands.entity(content).add_child(add_moon);
            }
            let apply = spawn_regen_button(commands);
            commands.entity(content).add_child(apply);
        }
        TargetKind::Moon(pi, mi) => {
            let moon_color = Color::srgb(0.6, 0.6, 0.7);
            let cat = spawn_category_header(commands, &format!("LUNE {} (P{})", mi+1, pi+1), moon_color);
            commands.entity(content).add_child(cat);
            let (card, _) = spawn_body_card(commands, settings, &format!("Lune {}", mi+1), moon_color, BodyAction::RemoveMoon(pi, mi), &[
                (SettingKey::MoonOrbitDistance(pi, mi), 30.0, 10000.0),
                (SettingKey::MoonRadius(pi, mi), 3.0, 150.0),
                (SettingKey::MoonSeed(pi, mi), 1.0, 999.0),
            ]);
            commands.entity(content).add_child(card);
            let apply = spawn_regen_button(commands);
            commands.entity(content).add_child(apply);
        }
        TargetKind::Star(i) => {
            let cat = spawn_category_header(commands, &format!("ETOILE {}", i+1), STAR_COLOR);
            commands.entity(content).add_child(cat);
            let (card, _) = spawn_body_card(commands, settings, &format!("Etoile {}", i+1), STAR_COLOR, BodyAction::RemoveStar(i), &[
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
            ]);
            commands.entity(content).add_child(card);
            let apply = spawn_regen_button(commands);
            commands.entity(content).add_child(apply);
        }
        TargetKind::GasPlanet(i) => {
            build_astre_editor(commands, res, content, "PLANETE GAZEUSE", PLANET_COLOR, TargetKind::GasPlanet(i), &[
                (AstreField::GasOrbit(i), 100.0, 100000.0),
                (AstreField::GasRadius(i), 20.0, 800.0),
                (AstreField::GasSeed(i), 1.0, 999.0),
                (AstreField::GasEcc(i), 0.0, 0.3),
                (AstreField::GasInc(i), -0.5, 0.5),
                (AstreField::GasBandCount(i), 2.0, 16.0),
                (AstreField::GasBandEmissive(i), 0.0, 2.0),
            ], RegenAstreAction::GasPlanet);
        }
        TargetKind::Comet(i) => {
            build_astre_editor(commands, res, content, "COMETE", PLANET_COLOR, TargetKind::Comet(i), &[
                (AstreField::CometPerihelion(i), 100.0, 20000.0),
                (AstreField::CometAphelion(i), 500.0, 50000.0),
                (AstreField::CometNucleus(i), 1.0, 100.0),
                (AstreField::CometComa(i), 5.0, 200.0),
                (AstreField::CometDustTail(i), 0.0, 1000.0),
                (AstreField::CometIonTail(i), 0.0, 1500.0),
                (AstreField::CometInclination(i), -1.0, 1.0),
            ], RegenAstreAction::Comet);
        }
        TargetKind::Meteoroid(i) => {
            build_astre_editor(commands, res, content, "METEOROIDE", PLANET_COLOR, TargetKind::Meteoroid(i), &[
                (AstreField::MeteorRadius(i), 1.0, 50.0),
            ], RegenAstreAction::Meteoroid);
        }
        TargetKind::VoxelStar(i) => {
            build_astre_editor(commands, res, content, "ETOILE VOXEL", STAR_COLOR, TargetKind::VoxelStar(i), &[
                (AstreField::VStarOrbit(i), 0.0, 10000.0),
                (AstreField::VStarRadius(i), 20.0, 500.0),
            ], RegenAstreAction::VoxelStar);
        }
        TargetKind::Protostar(i) => {
            build_astre_editor(commands, res, content, "PROTOETOILE", STAR_COLOR, TargetKind::Protostar(i), &[
                (AstreField::ProtoOrbit(i), 0.0, 10000.0),
                (AstreField::ProtoRadius(i), 20.0, 500.0),
                (AstreField::ProtoTemp(i), 0.0, 1.0),
            ], RegenAstreAction::Protostar);
        }
        TargetKind::DwarfStar(i) => {
            build_astre_editor(commands, res, content, "NAINE", STAR_COLOR, TargetKind::DwarfStar(i), &[
                (AstreField::DwarfOrbit(i), 0.0, 10000.0),
                (AstreField::DwarfRadius(i), 5.0, 200.0),
            ], RegenAstreAction::DwarfStar);
        }
        TargetKind::MainSequence(i) => {
            build_astre_editor(commands, res, content, "SEQ. PRINCIPALE", STAR_COLOR, TargetKind::MainSequence(i), &[
                (AstreField::MSOrbit(i), 0.0, 10000.0),
                (AstreField::MSRadius(i), 20.0, 500.0),
            ], RegenAstreAction::MainSequence);
        }
        TargetKind::GiantStar(i) => {
            build_astre_editor(commands, res, content, "GEANTE", STAR_COLOR, TargetKind::GiantStar(i), &[
                (AstreField::GiantOrbit(i), 0.0, 10000.0),
                (AstreField::GiantRadius(i), 50.0, 1000.0),
            ], RegenAstreAction::GiantStar);
        }
        TargetKind::Supergiant(i) => {
            build_astre_editor(commands, res, content, "SUPERGEANTE", STAR_COLOR, TargetKind::Supergiant(i), &[
                (AstreField::SGOrbit(i), 0.0, 10000.0),
                (AstreField::SGRadius(i), 100.0, 2000.0),
            ], RegenAstreAction::Supergiant);
        }
        TargetKind::Hypergiant(i) => {
            build_astre_editor(commands, res, content, "HYPERGEANTE", STAR_COLOR, TargetKind::Hypergiant(i), &[
                (AstreField::HGOrbit(i), 0.0, 10000.0),
                (AstreField::HGRadius(i), 200.0, 3000.0),
            ], RegenAstreAction::Hypergiant);
        }
        TargetKind::BlackHole(i) => {
            build_astre_editor(commands, res, content, "TROU NOIR", REMNANT_COLOR, TargetKind::BlackHole(i), &[
                (AstreField::BHOrbit(i), 0.0, 10000.0),
                (AstreField::BHHorizon(i), 5.0, 200.0),
                (AstreField::BHInfluence(i), 50.0, 2000.0),
            ], RegenAstreAction::BlackHole);
        }
        TargetKind::Pulsar(i) => {
            build_astre_editor(commands, res, content, "PULSAR", REMNANT_COLOR, TargetKind::Pulsar(i), &[
                (AstreField::PulsarOrbit(i), 0.0, 10000.0),
                (AstreField::PulsarRadius(i), 5.0, 100.0),
                (AstreField::PulsarPeriod(i), 0.001, 2.0),
            ], RegenAstreAction::Pulsar);
        }
        TargetKind::Magnetar(i) => {
            build_astre_editor(commands, res, content, "MAGNETAR", REMNANT_COLOR, TargetKind::Magnetar(i), &[
                (AstreField::MagOrbit(i), 0.0, 10000.0),
                (AstreField::MagRadius(i), 5.0, 100.0),
            ], RegenAstreAction::Magnetar);
        }
        TargetKind::NeutronStar(i) => {
            build_astre_editor(commands, res, content, "ETOILE A NEUTRONS", REMNANT_COLOR, TargetKind::NeutronStar(i), &[
                (AstreField::NSOrbit(i), 0.0, 10000.0),
                (AstreField::NSRadius(i), 5.0, 100.0),
            ], RegenAstreAction::NeutronStar);
        }
        TargetKind::Supernova(i) => {
            build_astre_editor(commands, res, content, "SUPERNOVA", REMNANT_COLOR, TargetKind::Supernova(i), &[
                (AstreField::SNOrbit(i), 0.0, 10000.0),
                (AstreField::SNRadius(i), 20.0, 500.0),
            ], RegenAstreAction::Supernova);
        }
        TargetKind::Nebula => {
            build_astre_editor(commands, res, content, "NEBULEUSE", BELT_COLOR, TargetKind::Nebula, &[
                (AstreField::NebRadius, 50.0, 2000.0),
                (AstreField::NebSeed, 1.0, 999.0),
            ], RegenAstreAction::Nebula);
        }
        _ => {}
    }
}

fn build_astre_editor(
    commands: &mut Commands,
    res: &AstresResources,
    content: Entity,
    title: &str,
    color: Color,
    target: TargetKind,
    sliders: &[(AstreField, f32, f32)],
    regen_action: RegenAstreAction,
) {
    let cat = spawn_category_header(commands, title, color);
    commands.entity(content).add_child(cat);

    for &(field, min, max) in sliders {
        let val = field.get(res);
        let s = spawn_astre_slider(commands, field, val, min, max);
        commands.entity(content).add_child(s);
    }

    let remove_btn = spawn_astre_edit_button(commands, "Supprimer", AstreEditAction::Remove(target), RED_SOFT);
    commands.entity(content).add_child(remove_btn);

    let regen = spawn_astre_regen_button(commands, "Regenerer", regen_action, color);
    commands.entity(content).add_child(regen);
}

fn spawn_regen_button(commands: &mut Commands) -> Entity {
    commands
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
        .id()
}

// ── Ajouter tab ──

fn build_ajouter_tab(commands: &mut Commands, content: Entity) {
    let cat_planet = spawn_category_header(commands, "PLANETES & CORPS", PLANET_COLOR);
    commands.entity(content).add_child(cat_planet);
    for (label, family) in [
        ("Planete rocheuse", AstreFamily::GasPlanet), // will use BodyAction instead
        ("Planete gazeuse", AstreFamily::GasPlanet),
        ("Comete", AstreFamily::Comet),
        ("Meteoroide", AstreFamily::Meteoroid),
    ] {
        if label == "Planete rocheuse" {
            let btn = spawn_add_button(commands, "Ajouter planete rocheuse", BodyAction::AddPlanet, PLANET_COLOR);
            commands.entity(content).add_child(btn);
        } else {
            add_astre_add_button(commands, content, label, family, PLANET_COLOR);
        }
    }

    let cat_star = spawn_category_header(commands, "ETOILES", STAR_COLOR);
    commands.entity(content).add_child(cat_star);
    let btn_star = spawn_add_button(commands, "Ajouter etoile (settings)", BodyAction::AddStar, STAR_COLOR);
    commands.entity(content).add_child(btn_star);
    for (label, family) in [
        ("Etoile voxel", AstreFamily::VoxelStar),
        ("Protoetoile", AstreFamily::Protostar),
        ("Naine", AstreFamily::DwarfStar),
        ("Seq. principale", AstreFamily::MainSequence),
        ("Geante", AstreFamily::GiantStar),
        ("Supergeante", AstreFamily::Supergiant),
        ("Hypergeante", AstreFamily::Hypergiant),
    ] {
        add_astre_add_button(commands, content, label, family, STAR_COLOR);
    }

    let cat_belt = spawn_category_header(commands, "CEINTURES", BELT_COLOR);
    commands.entity(content).add_child(cat_belt);
    let btn_belt = spawn_add_button(commands, "Ajouter ceinture", BodyAction::AddBelt, BELT_COLOR);
    commands.entity(content).add_child(btn_belt);

    let cat_rem = spawn_category_header(commands, "REMANENTS", REMNANT_COLOR);
    commands.entity(content).add_child(cat_rem);
    for (label, family) in [
        ("Nebuleuse", AstreFamily::Nebula),
        ("Trou noir", AstreFamily::BlackHole),
        ("Pulsar", AstreFamily::Pulsar),
        ("Magnetar", AstreFamily::Magnetar),
        ("Etoile a neutrons", AstreFamily::NeutronStar),
        ("Supernova", AstreFamily::Supernova),
    ] {
        add_astre_add_button(commands, content, label, family, REMNANT_COLOR);
    }
}

fn spawn_astres_tabs(commands: &mut Commands, active: AstresTab) -> Entity {
    let bar = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            column_gap: Val::Px(6.0),
            padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
            ..default()
        })
        .id();

    for (tab, label, color) in [
        (AstresTab::Liste, "Liste", ACCENT),
        (AstresTab::Editer, "Editer", STAR_COLOR),
        (AstresTab::Ajouter, "Ajouter", PLANET_COLOR),
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

/// Convertit un événement molette en "crans" : les trackpads (macOS surtout,
/// et certains pilotes Linux) envoient des pixels au lieu de lignes, ce qui
/// rendait le zoom et le défilement beaucoup trop rapides.
pub(crate) fn wheel_lines(ev: &MouseWheel) -> f32 {
    use bevy::input::mouse::MouseScrollUnit;
    match ev.unit {
        MouseScrollUnit::Line => ev.y,
        MouseScrollUnit::Pixel => ev.y / 40.0,
    }
}

fn astres_save_path() -> PathBuf {
    crate::settings::data_dir().join("astres.json")
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

pub(crate) fn load_saved_astres(mut res: AstresMutableResources) {
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

fn handle_select_astre(
    interactions: Query<(&Interaction, &SelectAstre), Changed<Interaction>>,
    mut selected: ResMut<SelectedAstre>,
    mut tab: ResMut<AstresTabState>,
    mut rebuild: EventWriter<RebuildUi>,
) {
    for (interaction, sel) in &interactions {
        if *interaction == Interaction::Pressed {
            selected.0 = Some(sel.0);
            tab.active = AstresTab::Editer;
            rebuild.send(RebuildUi);
        }
    }
}

fn handle_astre_sliders(
    interactions: Query<(&Interaction, &AstreSliderBar, &Node, &GlobalTransform), Changed<Interaction>>,
    primary_window: Query<&Window, With<PrimaryWindow>>,
    other_windows: Query<&Window, Without<PrimaryWindow>>,
    mut res: AstresMutableResources,
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
            slider.field.set(&mut res, val);
        }
    }
}

fn update_astre_slider_visuals(
    res: AstresResources,
    sliders: Query<&AstreSliderBar>,
    mut fills: Query<(&AstreSliderFill, &mut Node), Without<AstreSliderBar>>,
    mut labels: Query<(&AstreSliderLabel, &mut Text)>,
) {
    for bar in &sliders {
        let val = bar.field.get(&res);
        let frac = ((val - bar.min) / (bar.max - bar.min)).clamp(0.0, 1.0);
        for (fill, mut node) in &mut fills {
            if fill.0 == bar.field {
                node.width = Val::Percent(frac * 100.0);
            }
        }
    }

    for (label, mut text) in &mut labels {
        let val = label.0.get(&res);
        let val_text = if val == val.floor() && val.abs() < 10000.0 {
            format!("{:.0}", val)
        } else {
            format!("{:.2}", val)
        };
        **text = val_text;
    }
}

const RADAR_SIZE: f32 = 500.0;
const RADAR_PAD: f32 = 20.0;
const RADAR_DOT_SIZE: f32 = 8.0;
const RADAR_CAM_SIZE: f32 = 10.0;

fn setup_radar_window(mut commands: Commands) {
    let mut radar_win = Window {
        title: "SpaceSpore - Radar".into(),
        resolution: (RADAR_SIZE, RADAR_SIZE + 60.0).into(),
        position: WindowPosition::Automatic,
        ..default()
    };
    radar_win.set_minimized(true);
    let window = commands.spawn(radar_win).id();

    let cam = commands
        .spawn((
            Camera2d,
            Camera {
                target: bevy::render::camera::RenderTarget::Window(WindowRef::Entity(window)),
                ..default()
            },
            RadarWindowCam,
        ))
        .id();

    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(Color::srgb(0.02, 0.02, 0.05)),
            TargetCamera(cam),
            RadarRoot,
        ))
        .id();

    let header = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(BG_DARK),
        ))
        .with_child((
            Text::new("RADAR - VUE XZ"),
            TextFont { font_size: 14.0, ..default() },
            TextColor(TEXT_COLOR),
        ))
        .id();

    let info = commands
        .spawn((
            Text::new(""),
            TextFont { font_size: 11.0, ..default() },
            TextColor(TEXT_DIM),
            RadarInfoText,
        ))
        .id();
    commands.entity(header).add_child(info);

    let area = commands
        .spawn((
            Node {
                width: Val::Px(RADAR_SIZE),
                height: Val::Px(RADAR_SIZE),
                position_type: PositionType::Relative,
                ..default()
            },
            BackgroundColor(Color::srgb(0.04, 0.04, 0.08)),
            RadarArea,
        ))
        .id();

    // camera indicator
    let cam_dot = commands
        .spawn((
            Node {
                width: Val::Px(RADAR_CAM_SIZE),
                height: Val::Px(RADAR_CAM_SIZE),
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                ..default()
            },
            BackgroundColor(Color::srgb(1.0, 1.0, 1.0)),
            BorderRadius::all(Val::Px(RADAR_CAM_SIZE / 2.0)),
            RadarCamIndicator,
        ))
        .id();

    // camera direction line
    let cam_dir = commands
        .spawn((
            Node {
                width: Val::Px(2.0),
                height: Val::Px(20.0),
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                ..default()
            },
            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.5)),
            RadarCamDir,
        ))
        .id();

    commands.entity(area).add_children(&[cam_dot, cam_dir]);
    commands.entity(root).add_children(&[header, area]);
}

fn update_radar(
    mut commands: Commands,
    body_q: Query<(Entity, &GlobalTransform, &AstreLodRoot, Option<&AstreUnloaded>)>,
    cam_q: Query<&Transform, With<Camera3d>>,
    area_q: Query<Entity, With<RadarArea>>,
    mut dots: Query<(Entity, &RadarDot, &mut Node, &mut BackgroundColor)>,
    mut cam_ind: Query<&mut Node, (With<RadarCamIndicator>, Without<RadarDot>, Without<RadarCamDir>)>,
    mut cam_dir_q: Query<&mut Node, (With<RadarCamDir>, Without<RadarCamIndicator>, Without<RadarDot>)>,
    mut info_q: Query<&mut Text, With<RadarInfoText>>,
) {
    let Ok(cam_tf) = cam_q.get_single() else { return };
    let Ok(area_e) = area_q.get_single() else { return };

    let cam_pos = cam_tf.translation;
    let cam_fwd = cam_tf.forward().as_vec3();

    let mut min_x = f32::MAX;
    let mut max_x = f32::MIN;
    let mut min_z = f32::MAX;
    let mut max_z = f32::MIN;

    let bodies: Vec<_> = body_q.iter().collect();

    // include camera in bounds
    min_x = min_x.min(cam_pos.x);
    max_x = max_x.max(cam_pos.x);
    min_z = min_z.min(cam_pos.z);
    max_z = max_z.max(cam_pos.z);

    for (_, gt, _, _) in &bodies {
        let p = gt.translation();
        min_x = min_x.min(p.x);
        max_x = max_x.max(p.x);
        min_z = min_z.min(p.z);
        max_z = max_z.max(p.z);
    }

    let margin = 500.0;
    min_x -= margin;
    max_x += margin;
    min_z -= margin;
    max_z += margin;

    let range_x = (max_x - min_x).max(1.0);
    let range_z = (max_z - min_z).max(1.0);
    let range = range_x.max(range_z);

    let center_x = (min_x + max_x) / 2.0;
    let center_z = (min_z + max_z) / 2.0;

    let usable = RADAR_SIZE - RADAR_PAD * 2.0;

    let to_px = |wx: f32, wz: f32| -> (f32, f32) {
        let nx = (wx - center_x) / range + 0.5;
        let nz = (wz - center_z) / range + 0.5;
        (RADAR_PAD + nx * usable, RADAR_PAD + nz * usable)
    };

    // update camera indicator
    let (cx, cz) = to_px(cam_pos.x, cam_pos.z);
    if let Ok(mut node) = cam_ind.get_single_mut() {
        node.left = Val::Px(cx - RADAR_CAM_SIZE / 2.0);
        node.top = Val::Px(cz - RADAR_CAM_SIZE / 2.0);
    }

    // update camera direction line (endpoint of a short segment from cam pos along fwd)
    if let Ok(mut node) = cam_dir_q.get_single_mut() {
        let dir_len = 25.0;
        let dx = cam_fwd.x;
        let dz = cam_fwd.z;
        let end_x = cx + dx * dir_len;
        let end_z = cz + dz * dir_len;
        node.left = Val::Px(end_x - 3.0);
        node.top = Val::Px(end_z - 3.0);
        node.width = Val::Px(6.0);
        node.height = Val::Px(6.0);
    }

    // track existing dots
    let mut existing: std::collections::HashMap<Entity, Entity> = std::collections::HashMap::new();
    for (dot_e, radar_dot, _, _) in &dots {
        existing.insert(radar_dot.0, dot_e);
    }

    let mut visible_count = 0u32;
    let mut hidden_count = 0u32;

    for &(body_e, ref gt, ref lod, ref unloaded) in &bodies {
        let p = gt.translation();
        let (px, pz) = to_px(p.x, p.z);
        let is_hidden = unloaded.is_some();

        if is_hidden { hidden_count += 1; } else { visible_count += 1; }

        let color = if is_hidden {
            Color::srgba(0.8, 0.15, 0.1, 0.5)
        } else {
            match lod.label {
                "Star" | "VoxelStar" | "Protostar" | "DwarfStar" | "MainSequence"
                | "GiantStar" | "Supergiant" | "Hypergiant" => Color::srgb(1.0, 0.9, 0.2),
                "Planet" | "GasPlanet" => Color::srgb(0.2, 0.6, 1.0),
                "Moon" => Color::srgb(0.6, 0.6, 0.65),
                "Comet" | "Meteoroid" => Color::srgb(0.5, 0.8, 0.9),
                "BlackHole" => Color::srgb(0.6, 0.0, 0.8),
                "Nebula" => Color::srgb(0.8, 0.3, 0.9),
                "Pulsar" | "Magnetar" | "NeutronStar" => Color::srgb(0.3, 1.0, 0.8),
                "Supernova" => Color::srgb(1.0, 0.5, 0.1),
                _ => Color::srgb(0.5, 0.5, 0.5),
            }
        };

        let dot_size = if is_hidden { RADAR_DOT_SIZE * 0.6 } else {
            (RADAR_DOT_SIZE * (lod.radius / 200.0).clamp(0.5, 3.0)).clamp(4.0, 16.0)
        };

        if let Some(&dot_e) = existing.get(&body_e) {
            if let Ok((_, _, mut node, mut bg)) = dots.get_mut(dot_e) {
                node.left = Val::Px(px - dot_size / 2.0);
                node.top = Val::Px(pz - dot_size / 2.0);
                node.width = Val::Px(dot_size);
                node.height = Val::Px(dot_size);
                *bg = BackgroundColor(color);
            }
            existing.remove(&body_e);
        } else {
            let dot = commands
                .spawn((
                    Node {
                        width: Val::Px(dot_size),
                        height: Val::Px(dot_size),
                        position_type: PositionType::Absolute,
                        left: Val::Px(px - dot_size / 2.0),
                        top: Val::Px(pz - dot_size / 2.0),
                        ..default()
                    },
                    BackgroundColor(color),
                    BorderRadius::all(Val::Px(dot_size / 2.0)),
                    RadarDot(body_e),
                ))
                .id();
            commands.entity(area_e).add_child(dot);
        }
    }

    // remove dots for bodies that no longer exist
    for (_, dot_e) in existing {
        commands.entity(dot_e).despawn_recursive();
    }

    // update info text
    if let Ok(mut text) = info_q.get_single_mut() {
        **text = format!("{} vis / {} hid", visible_count, hidden_count);
    }
}