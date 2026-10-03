//! Profils complets des astres : `StarProfile` et `PlanetProfile` (planètes et lunes).
//!
//! Un profil n'est jamais stocké dans la liste des systèmes : il est calculé à la demande depuis
//! la configuration du système (elle-même recalculée depuis la graine) et les deltas du monde,
//! puis gardé dans le cache du système chargé (`cache.rs`).
//!
//! Phase 0 : les sections existent ; elles reprennent ce que le jeu sait déjà (rayon, orbite,
//! atmosphère oui/non, température, relief…) et restent vides (`None`, listes vides) pour ce que
//! les phases suivantes calculeront. Les valeurs marquées « provisoire » seront remplacées.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::live::{delta_of, BodyId, Live, WorldDeltas};
use super::seeds::{layer_seed, Layer};
use super::{atmosphere, system, units};
use crate::settings::{MoonConfig, PlanetConfig, StarConfig, StarSystemConfig};

/// Rigueur d'une donnée (règle 5) : réaliste, spéculative ou fictive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Realism {
    Realistic,
    Speculative,
    Fictional,
}

/// Trait ou anomalie d'un astre (phase 9).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Trait {
    pub name: String,
    pub realism: Realism,
    /// Probabilité d'apparition (0,98 courant … 0,001 rarissime).
    pub rarity: f64,
}

/// `f32` → `f64` sans faux chiffres (0,02 et non 0,019999999552965164) : le profil s'exporte en JSON.
fn f(x: f32) -> f64 {
    x.to_string().parse().unwrap_or(x as f64)
}

/// Sous-graines de chaque couche, affichées dans l'export pour le débogage.
fn layer_seeds(body_seed: u64, layers: &[Layer]) -> BTreeMap<&'static str, String> {
    layers.iter().map(|&l| (l.name(), format!("{:016x}", layer_seed(body_seed, l)))).collect()
}

// ─────────────────────────────────────────────────────────────────────────
//  Étoile
// ─────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize)]
pub struct StarProfile {
    pub id: String,
    pub name: String,
    pub system: String,
    pub seed: u64,
    pub layer_seeds: BTreeMap<&'static str, String>,
    /// Type (« naine rouge (M) », « geante rouge »…).
    pub class: String,
    /// Type spectral complet (« G2 V », « M4 V », « K1 III », « DA3.1 »…).
    pub spectral_type: Option<String>,
    /// Rayon affiché (unités du jeu) : compressé pour les géantes.
    pub radius_game: f64,
    /// Rayon réel.
    pub radius_sun: f64,
    pub mass_sun: Option<f64>,
    pub luminosity_sun: Option<f64>,
    pub temperature_k: f64,
    /// Couleur du corps noir (sRGB).
    pub color: [f32; 3],
    /// Intensité lumineuse du rendu (unités du jeu).
    pub light_intensity: f32,
    pub age_gyr: Option<f64>,
    pub lifetime_gyr: Option<f64>,
    /// Activité magnétique, 0 à 1.
    pub magnetic_activity: Option<f64>,
    /// UV reçus dans la zone habitable (Soleil/Terre = 1).
    pub uv_flux: Option<f64>,
    /// Rayons X (Soleil = 1).
    pub xray_flux: Option<f64>,
    /// Vent stellaire, perte de masse (Soleil = 1).
    pub stellar_wind: Option<f64>,
    pub flares: u32,
    pub traits: Vec<Trait>,
}

impl StarProfile {
    pub fn build(sys_idx: usize, star_idx: usize, sys: &StarSystemConfig, star: &StarConfig) -> Self {
        let seed = sys.body_seed();
        let id = BodyId::Star { system: sys_idx as u32, index: star_idx as u16 };
        // Seule l'étoile principale d'un système généré a une physique complète
        let physics = sys.star_physics_of(star_idx);
        let p = physics.as_ref();
        Self {
            id: id.key(),
            name: if sys.stars.len() > 1 { format!("{} {}", sys.name, (b'A' + star_idx as u8) as char) } else { sys.name.clone() },
            system: sys.name.clone(),
            seed,
            layer_seeds: layer_seeds(seed, &[Layer::Star]),
            class: star.class.name().to_string(),
            spectral_type: p.map(|p| p.spectral_type.clone()),
            radius_game: f(star.radius),
            radius_sun: p.map_or_else(|| units::game_to_sun_radii(f(star.radius)), |p| p.radius_sun),
            mass_sun: p.map(|p| p.mass_sun),
            luminosity_sun: p.map(|p| p.luminosity_sun),
            temperature_k: p.map_or_else(|| f(star.temperature()), |p| p.temperature_k),
            color: [star.light_color_r, star.light_color_g, star.light_color_b],
            light_intensity: star.intensity,
            age_gyr: p.map(|p| p.age_gyr),
            lifetime_gyr: p.map(|p| p.lifetime_gyr),
            magnetic_activity: p.map(|p| p.activity),
            uv_flux: p.map(|p| p.uv_flux),
            xray_flux: p.map(|p| p.xray_flux),
            stellar_wind: p.map(|p| p.stellar_wind),
            flares: star.flare_count,
            traits: Vec::new(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  Planète ou lune
// ─────────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BodyKind {
    Planet,
    Moon,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct OrbitSection {
    /// Astre autour duquel il tourne (clé `BodyId`).
    pub parent: String,
    /// Distance affichée (unités du jeu).
    pub distance_game: f64,
    /// Distance physique à l'étoile (UA) : sert au climat. Provisoire, voir `units::orbit_game_to_au`.
    pub semi_major_axis_au: Live,
    pub eccentricity: f64,
    pub inclination: f64,
    pub ascending_node: f64,
    pub arg_periapsis: f64,
    pub mean_anomaly_0: f64,
    pub period_days: Option<f64>,
    pub axial_tilt_deg: Option<f64>,
    /// Période de rotation (stockée en phase 2, utilisée en 0.11).
    pub rotation_period_h: Option<f64>,
    pub tidally_locked: Option<bool>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct PhysicsSection {
    pub radius_game: f64,
    pub radius_earth: Live,
    /// Provisoire : masse volumique terrestre supposée (la phase 2 tire la masse).
    pub mass_earth: Live,
    pub density_g_cm3: f64,
    pub surface_gravity_g: f64,
    pub escape_velocity_km_s: f64,
    /// Classe de taille (« minuscule » … « géante ») : phase 2.
    pub size_class: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct CompositionSection {
    /// Fractions de masse par matière (fer, silicates, glaces, gaz…) : départ + deltas.
    pub bulk: BTreeMap<String, Live>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct AtmosphereSection {
    pub present: bool,
    /// Au sol ; pour une géante, au niveau des nuages.
    pub surface_pressure_bar: Option<f64>,
    /// Gaz par ordre d'importance, avec leur rigueur (réel ou fictif).
    pub gases: Vec<GasShare>,
    /// Type de nuages (eau, acide sulfurique, méthane…).
    pub clouds: String,
    pub cloud_density: f64,
    pub cloud_altitude_game: f64,
    pub cloud_speed: f64,
    pub sky_color: Option<[f32; 3]>,
    pub sunset_color: Option<[f32; 3]>,
    pub haze_color: Option<[f32; 3]>,
}

/// Part d'un gaz dans l'atmosphère.
#[derive(Clone, Debug, Serialize)]
pub struct GasShare {
    pub formula: String,
    pub name: String,
    pub fraction: f64,
    pub realism: Realism,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct ClimateSection {
    /// Température moyenne de la surface (°C), effet de serre compris.
    pub mean_temperature_c: f64,
    /// À l'équateur et aux pôles, au niveau de la mer (°C).
    pub equator_c: Option<f64>,
    pub pole_c: Option<f64>,
    /// Refroidissement du niveau de la mer au sommet du relief (K).
    pub summit_cooling_k: Option<f64>,
    /// Écart jour / nuit (K), effectif à partir de la 0.11.
    pub day_night_k: Option<f64>,
    pub equilibrium_temperature_k: Option<f64>,
    pub greenhouse_k: Option<f64>,
    pub albedo: Option<f64>,
    pub wind_m_s: Option<f64>,
    pub winds: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct HydrologySection {
    /// Niveau de la mer du terrain (0..1 de la hauteur du relief), fixé par la couverture océanique.
    pub sea_level: f64,
    /// État de l'eau en surface : liquide, glace, vapeur, supercritique, absente.
    pub water_state: Option<String>,
    /// Réserve d'eau (0 : sèche, 1 : monde-océan).
    pub water_inventory: Option<f64>,
    pub ocean_fraction: Option<f64>,
    /// Liquide des océans (eau, méthane, ammoniac, lave…) : phase 4.
    pub ocean_liquid: Option<String>,
    pub ice_caps: Option<f64>,
    pub groundwater: Option<f64>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct GeologySection {
    pub age_gyr: Option<f64>,
    pub surface_age_gyr: Option<f64>,
    /// Chaleur interne (0 à 1).
    pub activity: Option<f64>,
    pub tectonics: Option<String>,
    pub volcanism: Option<f64>,
    /// Séismes de magnitude 5 et plus, Terre = 1.
    pub quakes: Option<f64>,
    /// Champ magnétique de surface, Terre = 1.
    pub magnetic_field: Option<f64>,
    /// Érosion (0 à 1) : pluie, vent, glace, âge de la surface.
    pub erosion: Option<f64>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct ReliefSection {
    pub terrain_height_game: f64,
    pub noise_scale: f64,
    pub detail_scale: f64,
    pub features: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct BiologySection {
    /// Niveau de vie (microbienne, simple, complexe), indépendant de l'habitabilité.
    pub life: Option<String>,
    /// Chances de vie microbienne, simple, complexe (0 à 1).
    pub life_chances: BTreeMap<&'static str, f64>,
    /// Chimie du vivant (réaliste, spéculative ou fictive).
    pub biochemistry: Option<String>,
    /// Des plantes couvrent les sols.
    pub flora: bool,
    /// Faune : paramètres seulement (créatures visibles en 0.11).
    pub fauna: Option<super::life::Fauna>,
    /// Biomes par part de la surface émergée, avec leur rigueur.
    pub biomes: Vec<BiomeShare>,
    /// Sols dominants des régions sèches ou sans vie.
    pub soils: Vec<String>,
    pub alien_flora: Option<bool>,
    /// Radiation au sol (0 : abritée, 1 : grillée).
    pub radiation: Option<f64>,
    /// Couleur moyenne de la planète vue de l'espace (sRGB).
    pub global_color: Option<[f32; 3]>,
}

/// Part d'un biome.
#[derive(Clone, Debug, Serialize)]
pub struct BiomeShare {
    pub name: String,
    pub fraction: f64,
    pub realism: Realism,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct ResourcesSection {
    /// Résumé des gisements (« or : filons, 1 600 m, difficile »).
    pub deposits: Vec<String>,
    /// Gisements détaillés (phase 8) : la quantité est vivante (départ − extrait).
    pub ores: Vec<OreProfile>,
    /// Valeur indicative de tous les gisements (crédits).
    pub total_worth: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct OreProfile {
    pub ore: String,
    pub realism: Realism,
    pub abundance: f64,
    pub depth_m: f64,
    pub distribution: &'static str,
    pub rarity: f64,
    pub difficulty: f64,
    pub amount_t: Live,
    /// Bien de l'économie correspondant (« Ressources »).
    pub good: &'static str,
}

/// Mots de la difficulté d'extraction.
pub fn difficulty_label(d: f64) -> &'static str {
    match d {
        d if d < 0.25 => "facile",
        d if d < 0.45 => "moyenne",
        d if d < 0.65 => "difficile",
        _ => "extreme",
    }
}

fn resources_section(r: &super::resources::Resources, id: BodyId, deltas: &WorldDeltas) -> ResourcesSection {
    let delta = delta_of(deltas, id);
    let ores: Vec<OreProfile> = r
        .deposits
        .iter()
        .map(|d| OreProfile {
            ore: d.ore.name().to_string(),
            realism: d.ore.realism(),
            abundance: f(d.abundance),
            depth_m: f(d.depth_m),
            distribution: d.distribution.name(),
            rarity: f(d.rarity),
            difficulty: f(d.difficulty),
            amount_t: Live::new(d.amount_t, -delta.ores.get(d.ore.name()).copied().unwrap_or(0.0)),
            good: crate::economy::GOODS[d.ore.good()].0,
        })
        .collect();
    ResourcesSection {
        deposits: r
            .deposits
            .iter()
            .map(|d| format!("{} : {}, {:.0} m, {}", d.ore.name(), d.distribution.name(), d.depth_m, difficulty_label(d.difficulty as f64)))
            .collect(),
        total_worth: r.deposits.iter().map(|d| d.worth()).sum::<f64>().round(),
        ores,
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct GameplaySection {
    /// On peut s'y poser et y marcher.
    pub walkable: bool,
    /// Score d'habitabilité (0 à 1) et son verdict.
    pub habitability: Option<f64>,
    pub habitability_label: Option<String>,
    /// Facteurs du score : température, eau, pression, radiation, gravité, air (0 à 1).
    pub habitability_factors: BTreeMap<&'static str, f64>,
    /// Dangers (« radiation (niveau 2) ») et le pire niveau (0 à 3).
    pub hazards: Vec<String>,
    pub danger_level: u8,
    /// Anneaux, aurores, chauffage par les marées.
    pub ring: Option<String>,
    pub aurora: Option<String>,
    pub tidal_heating: Option<f64>,
    pub subsurface_ocean: bool,
    /// Planète errante (C2) : sans étoile.
    pub rogue: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct PlanetProfile {
    pub id: String,
    pub kind: BodyKind,
    pub name: String,
    pub seed: u64,
    pub layer_seeds: BTreeMap<&'static str, String>,
    pub orbit: OrbitSection,
    pub physics: PhysicsSection,
    pub composition: CompositionSection,
    pub atmosphere: AtmosphereSection,
    pub climate: ClimateSection,
    pub hydrology: HydrologySection,
    pub geology: GeologySection,
    pub relief: ReliefSection,
    pub biology: BiologySection,
    pub resources: ResourcesSection,
    pub gameplay: GameplaySection,
    pub traits: Vec<Trait>,
}

const PLANET_LAYERS: [Layer; 12] = [
    Layer::Orbit,
    Layer::Physics,
    Layer::Composition,
    Layer::Atmosphere,
    Layer::Climate,
    Layer::Hydrology,
    Layer::Geology,
    Layer::Relief,
    Layer::Biology,
    Layer::Resources,
    Layer::Gameplay,
    Layer::Traits,
];

/// Physique d'un corps de rayon `radius_game` : rayon et masse vivants, le reste en découle.
/// Physique d'un corps : rayon et masse vivants (départ de la génération + deltas), gravité,
/// libération et densité recalculées depuis les valeurs courantes. `radius_earth` / `mass_earth`
/// à 0 : corps fait à la main, déduits du rayon affiché (masse volumique terrestre).
fn physics(radius_game: f64, radius_earth: f64, mass_earth: f64, size: Option<&str>, id: BodyId, deltas: &WorldDeltas) -> PhysicsSection {
    let delta = delta_of(deltas, id);
    let base_radius = if radius_earth > 0.0 { radius_earth } else { units::game_to_earth_radii(radius_game) };
    let base_mass = if mass_earth > 0.0 { mass_earth } else { base_radius * base_radius * base_radius };
    let radius = Live::new(base_radius, delta.radius_earth);
    let mass = Live::new(base_mass, delta.mass_earth);
    // Gravité, libération et densité se recalculent toujours depuis les valeurs courantes
    let (m, r) = (mass.current().max(0.0), radius.current().max(0.0));
    PhysicsSection {
        radius_game,
        radius_earth: radius,
        mass_earth: mass,
        density_g_cm3: units::density(m, r),
        surface_gravity_g: units::surface_gravity(m, r),
        escape_velocity_km_s: units::escape_velocity(m, r),
        size_class: size.map(str::to_string),
    }
}

/// Valeur positive, sinon `None` (donnée absente d'un corps fait à la main).
fn some(x: f32) -> Option<f64> {
    (x > 0.0).then(|| f(x))
}

fn atmosphere_section(p: &PlanetConfig) -> AtmosphereSection {
    let air = &p.air;
    let known = !air.gases.is_empty();
    AtmosphereSection {
        present: p.atmosphere || p.gaseous(),
        surface_pressure_bar: known.then(|| f(air.pressure_bar)),
        gases: air
            .gases
            .iter()
            .map(|(formula, x)| {
                let g = atmosphere::gas_info(formula);
                GasShare {
                    formula: formula.clone(),
                    name: g.map_or(formula.as_str(), |g| g.name).to_string(),
                    fraction: f(*x),
                    realism: g.map_or(Realism::Realistic, |g| g.realism),
                }
            })
            .collect(),
        clouds: air.clouds.name().to_string(),
        cloud_density: if p.atmosphere { f(p.cloud_density) } else { 0.0 },
        cloud_altitude_game: f(p.cloud_altitude),
        cloud_speed: f(p.cloud_speed),
        sky_color: known.then_some(air.sky),
        sunset_color: known.then_some(air.sunset),
        haze_color: known.then_some(air.haze),
    }
}

fn climate_section(p: &PlanetConfig) -> ClimateSection {
    let c = p.climate();
    let (equator, pole) = c.range();
    let known = p.climate.is_some();
    let air = &p.air;
    ClimateSection {
        mean_temperature_c: f(c.mean_c),
        equator_c: known.then(|| f(equator)),
        pole_c: known.then(|| f(pole)),
        summit_cooling_k: known.then(|| f(c.lapse)),
        day_night_k: known.then(|| f(c.diurnal * 2.0)),
        equilibrium_temperature_k: (air.t_eq_k > 0.0).then(|| f(air.t_eq_k)),
        greenhouse_k: (air.t_eq_k > 0.0).then(|| f(air.greenhouse_k)),
        albedo: (air.t_eq_k > 0.0).then(|| f(air.albedo)),
        wind_m_s: (air.t_eq_k > 0.0).then(|| f(air.wind_ms)),
        winds: (!air.winds.is_empty()).then(|| air.winds.clone()),
    }
}

fn gameplay_section(p: &PlanetConfig) -> GameplaySection {
    let h = &p.habitability;
    let known = !h.label.is_empty();
    let mut hazards: Vec<String> = h.dangers.iter().map(|d| format!("{} (niveau {})", d.name, d.level)).collect();
    if p.gaseous() {
        hazards.insert(0, "pas de sol : la pression abime la coque du vaisseau".to_string());
    }
    GameplaySection {
        walkable: !p.gaseous(),
        habitability: known.then(|| f(h.score)),
        habitability_label: known.then(|| h.label.clone()),
        habitability_factors: if known {
            [("temperature", h.temperature), ("eau", h.water), ("pression", h.pressure), ("radiation", h.radiation), ("gravite", h.gravity), ("air", h.air)]
                .into_iter()
                .map(|(k, v)| (k, f(v)))
                .collect()
        } else {
            BTreeMap::new()
        },
        hazards,
        danger_level: h.danger_level,
        ring: p.ring.map(|r| format!("de {:.0} a {:.0} (rayon {:.0}), {:.0} % de glace", r.inner, r.outer, p.radius, r.ice * 100.0)),
        rogue: p.rogue,
        aurora: p.aurora.map(|a| format!("force {:.2} vers {:.0} degres de latitude", a.strength, a.latitude)),
        tidal_heating: None,
        subsurface_ocean: p.hydrology.subsurface_ocean,
    }
}

fn geology_section(p: &PlanetConfig) -> GeologySection {
    let g = &p.geology;
    if g.age_gyr <= 0.0 {
        return GeologySection::default();
    }
    GeologySection {
        age_gyr: Some(f(g.age_gyr)),
        surface_age_gyr: Some(f(g.surface_age_gyr)),
        activity: Some(f(g.activity)),
        tectonics: Some(g.tectonics.name().to_string()),
        volcanism: Some(f(g.volcanism)),
        quakes: Some(f(g.quakes)),
        magnetic_field: Some(f(g.magnetic_field)),
        erosion: Some(f(g.relief.erosion)),
    }
}

/// Biomes et couleur globale : on échantillonne le vrai terrain (relief, mers, climat).
fn biology_section(p: &PlanetConfig) -> BiologySection {
    let b = &p.biomes;
    if !b.defined {
        return BiologySection::default();
    }
    let terrain = crate::terrain::Terrain::new(crate::terrain::BodyParams::planet(p));
    let n = 800;
    let mut counts: BTreeMap<&'static str, (usize, Realism)> = BTreeMap::new();
    let mut land = 0;
    let mut color = [0.0f32; 3];
    for i in 0..n {
        let z = 1.0 - 2.0 * (i as f32 + 0.5) / n as f32;
        let a = i as f32 * 2.399_963;
        let r = (1.0 - z * z).sqrt();
        let dir = bevy::math::Vec3::new(r * a.cos(), z, r * a.sin());
        let c = terrain.column(dir, terrain.voxel()).color;
        color = [color[0] + c[0], color[1] + c[1], color[2] + c[2]];
        if let Some(biome) = terrain.biome_at(dir) {
            land += 1;
            counts.entry(biome.name()).or_insert((0, biome.realism())).0 += 1;
        }
    }
    let mut biomes: Vec<BiomeShare> = counts
        .into_iter()
        .map(|(name, (k, realism))| BiomeShare { name: name.to_string(), fraction: k as f64 / land.max(1) as f64, realism })
        .collect();
    biomes.sort_by(|x, y| y.fraction.total_cmp(&x.fraction));
    let mut soils = Vec::new();
    if b.volcanism > 0.3 {
        soils.push("volcanique (basalte)".to_string());
    }
    if b.metal {
        soils.push("metallique (rouille)".to_string());
    }
    if b.salt {
        soils.push("sel (mers evaporees)".to_string());
    }
    if b.regolith {
        soils.push("regolithe".to_string());
    }
    if b.wetness > 0.4 {
        soils.push("argile".to_string());
    }
    soils.push("sable".to_string());
    let life = &p.life;
    BiologySection {
        life: Some(life.level.name().to_string()),
        life_chances: [("microbienne", life.chance_microbial), ("simple", life.chance_simple), ("complexe", life.chance_complex)]
            .into_iter()
            .map(|(k, v)| (k, f(v)))
            .collect(),
        biochemistry: (!life.biochemistry.is_empty()).then(|| life.biochemistry.clone()),
        flora: life.flora,
        fauna: life.fauna.clone(),
        biomes,
        soils,
        alien_flora: Some(b.alien),
        radiation: Some(f(b.radiation)),
        global_color: Some(color.map(|x| x / n as f32)),
    }
}

fn composition(base: &BTreeMap<String, f32>, id: BodyId, deltas: &WorldDeltas) -> CompositionSection {
    let delta = delta_of(deltas, id);
    let mut bulk: BTreeMap<String, Live> = base.iter().map(|(k, v)| (k.clone(), Live::new(f(*v), 0.0))).collect();
    for (k, v) in &delta.composition {
        bulk.entry(k.clone()).or_insert(Live::new(0.0, 0.0)).delta = *v;
    }
    CompositionSection { bulk }
}

impl PlanetProfile {
    pub fn planet(sys_idx: usize, index: usize, sys: &StarSystemConfig, p: &PlanetConfig, deltas: &WorldDeltas) -> Self {
        let id = BodyId::Planet { system: sys_idx as u32, index: index as u16 };
        let star_radius = f(p.star_radius);
        let au = if p.semi_major_au > 0.0 { f(p.semi_major_au) } else { units::orbit_game_to_au(f(p.orbit_distance), star_radius) };
        let size = system::size_class(p.kind, f(p.radius_earth.max(0.0)), p.hot);
        let seed = p.seed as u64;
        Self {
            id: id.key(),
            kind: BodyKind::Planet,
            name: format!("{} {}", sys.name, index + 1),
            seed,
            layer_seeds: layer_seeds(seed, &PLANET_LAYERS),
            orbit: OrbitSection {
                parent: BodyId::Star { system: sys_idx as u32, index: 0 }.key(),
                distance_game: f(p.orbit_distance),
                semi_major_axis_au: Live::new(au, delta_of(deltas, id).orbit_au),
                eccentricity: f(p.eccentricity),
                inclination: f(p.inclination),
                ascending_node: f(p.ascending_node),
                arg_periapsis: f(p.arg_periapsis),
                mean_anomaly_0: f(p.mean_anomaly_0),
                period_days: some(p.period_days),
                axial_tilt_deg: (p.semi_major_au > 0.0).then(|| f(p.axial_tilt)),
                rotation_period_h: some(p.rotation_h),
                tidally_locked: (p.semi_major_au > 0.0).then_some(p.tidally_locked),
            },
            physics: physics(f(p.radius), f(p.radius_earth), f(p.mass_earth), (p.radius_earth > 0.0).then_some(size), id, deltas),
            composition: composition(&p.resources.bulk, id, deltas),
            atmosphere: atmosphere_section(p),
            climate: climate_section(p),
            hydrology: {
                let h = &p.hydrology;
                let known = p.climate.is_some();
                HydrologySection {
                    sea_level: f(p.sea_level),
                    water_state: known.then(|| h.water_state.name().to_string()),
                    ocean_fraction: known.then(|| f(h.ocean_fraction)),
                    ocean_liquid: known.then(|| h.hydro.liquid.name().to_string()),
                    ice_caps: known.then(|| f(h.ice_caps)),
                    water_inventory: known.then(|| f(h.inventory)),
                    groundwater: known.then(|| f(h.groundwater_m)),
                }
            },
            geology: geology_section(p),
            relief: ReliefSection {
                terrain_height_game: f(p.terrain_height),
                noise_scale: f(p.noise_scale),
                detail_scale: f(p.detail_scale),
                features: p.geology.features(),
            },
            biology: biology_section(p),
            resources: resources_section(&p.resources, id, deltas),
            gameplay: gameplay_section(p),
            traits: p.traits.clone(),
        }
    }

    pub fn moon(
        sys_idx: usize,
        planet_index: usize,
        index: usize,
        sys: &StarSystemConfig,
        planet: &PlanetConfig,
        m: &MoonConfig,
        deltas: &WorldDeltas,
    ) -> Self {
        let id = BodyId::Moon { system: sys_idx as u32, planet: planet_index as u16, index: index as u16 };
        let parent = BodyId::Planet { system: sys_idx as u32, index: planet_index as u16 };
        // Lune générée (phase 9) : toute la chaîne, comme une planète
        if m.generated() {
            let mut prof = Self::planet(sys_idx, planet_index, sys, &m.as_planet(planet), deltas);
            prof.id = id.key();
            prof.kind = BodyKind::Moon;
            prof.name = format!("{} {} {}", sys.name, planet_index + 1, (b'a' + index as u8) as char);
            prof.layer_seeds = layer_seeds(m.seed as u64, &PLANET_LAYERS);
            prof.orbit.parent = parent.key();
            prof.orbit.distance_game = f(m.orbit_distance);
            prof.orbit.period_days = None;
            prof.orbit.rotation_period_h = None;
            prof.orbit.axial_tilt_deg = None;
            prof.orbit.semi_major_axis_au = Live::new(prof.orbit.semi_major_axis_au.base, delta_of(deltas, id).orbit_au);
            prof.physics = physics(f(m.radius), f(m.radius_earth), f(m.mass_earth), Some("lune"), id, deltas);
            prof.composition = composition(&m.resources.bulk, id, deltas);
            prof.resources = resources_section(&m.resources, id, deltas);
            prof.gameplay.tidal_heating = Some(f(m.tidal_heat));
            return prof;
        }
        // Une lune est à la distance de sa planète de l'étoile : même climat (comme `BodyParams::moon`)
        let au = if planet.semi_major_au > 0.0 {
            f(planet.semi_major_au)
        } else {
            units::orbit_game_to_au(f(planet.orbit_distance), f(planet.star_radius))
        };
        let body = crate::terrain::BodyParams::moon(m, planet);
        let seed = m.seed as u64;
        Self {
            id: id.key(),
            kind: BodyKind::Moon,
            name: format!("{} {} {}", sys.name, planet_index + 1, (b'a' + index as u8) as char),
            seed,
            layer_seeds: layer_seeds(seed, &PLANET_LAYERS),
            orbit: OrbitSection {
                parent: parent.key(),
                distance_game: f(m.orbit_distance),
                semi_major_axis_au: Live::new(au, delta_of(deltas, id).orbit_au),
                eccentricity: f(m.eccentricity),
                inclination: f(m.inclination),
                ascending_node: f(m.ascending_node),
                arg_periapsis: f(m.arg_periapsis),
                mean_anomaly_0: f(m.mean_anomaly_0),
                ..Default::default()
            },
            physics: physics(f(m.radius), f(m.radius_earth), f(m.mass_earth), (m.radius_earth > 0.0).then_some("lune"), id, deltas),
            composition: composition(&BTreeMap::new(), id, deltas),
            atmosphere: AtmosphereSection::default(),
            climate: {
                let (equator, pole) = body.climate.range();
                ClimateSection {
                    mean_temperature_c: f(body.temperature),
                    equator_c: Some(f(equator)),
                    pole_c: Some(f(pole)),
                    day_night_k: Some(f(body.climate.diurnal * 2.0)),
                    winds: Some("aucun (pas d'atmosphere)".into()),
                    ..Default::default()
                }
            },
            hydrology: HydrologySection::default(),
            geology: GeologySection::default(),
            relief: ReliefSection {
                terrain_height_game: f(body.terrain_height),
                noise_scale: f(body.noise_scale),
                detail_scale: f(body.detail_scale),
                features: Vec::new(),
            },
            biology: BiologySection::default(),
            resources: ResourcesSection::default(),
            gameplay: GameplaySection { walkable: true, ..Default::default() },
            traits: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planetgen::live::BodyDelta;
    use crate::settings::GameSettings;

    #[test]
    fn profiles_reflect_the_generated_bodies() {
        let settings = GameSettings::default();
        let deltas = WorldDeltas::new();
        for (si, sys) in settings.systems.dense().iter().enumerate().take(200) {
            let star = StarProfile::build(si, 0, sys, &sys.stars[0]);
            // Physique réelle : de la naine blanche (0,01 R☉) à la géante (100 R☉)
            assert!((0.005..=101.0).contains(&star.radius_sun), "{}", star.radius_sun);
            assert!(star.spectral_type.is_some() && star.mass_sun.is_some());
            for (pi, p) in sys.planets().iter().enumerate() {
                let prof = PlanetProfile::planet(si, pi, sys, p, &deltas);
                assert_eq!(prof.id, format!("s{si}.p{pi}"));
                // Physique réelle de la génération (phase 2)
                let r = prof.physics.radius_earth.current();
                assert_eq!(r, f(p.radius_earth));
                assert_eq!(prof.orbit.semi_major_axis_au.current(), f(p.semi_major_au));
                assert!((prof.physics.surface_gravity_g / f(p.gravity_g) - 1.0).abs() < 1e-3);
                assert!(prof.physics.size_class.is_some() && prof.orbit.period_days.is_some());
                assert_eq!(prof.gameplay.walkable, !p.gaseous());
                // Habitabilité et dangers (phase 9)
                assert!(prof.gameplay.habitability.is_some() && prof.gameplay.danger_level <= 3);
                if p.gaseous() {
                    assert_eq!(prof.gameplay.danger_level, 3);
                }
                // Atmosphère et climat de la phase 3
                assert_eq!(prof.atmosphere.gases.len(), p.air.gases.len());
                if prof.atmosphere.present && !p.gaseous() {
                    assert!(prof.atmosphere.surface_pressure_bar.unwrap() >= 0.01);
                    assert!(prof.climate.greenhouse_k.unwrap() >= 0.0);
                }
                let (eq, pole) = (prof.climate.equator_c.unwrap(), prof.climate.pole_c.unwrap());
                assert!(eq >= pole && eq >= prof.climate.mean_temperature_c);
                assert_eq!(prof.climate.mean_temperature_c, f(p.temperature()));
                for (mi, m) in p.moons.iter().enumerate() {
                    let moon = PlanetProfile::moon(si, pi, mi, sys, p, m, &deltas);
                    assert_eq!(moon.orbit.parent, prof.id);
                    assert!(moon.physics.radius_earth.current() < r / 2.9);
                    assert_eq!(moon.id, format!("s{si}.p{pi}.m{mi}"));
                    // Une lune peut avoir de l'air (Titan) ; si oui, au moins 10 mbar
                    if moon.atmosphere.present {
                        assert!(moon.atmosphere.surface_pressure_bar.unwrap() >= 0.01);
                    }
                }
            }
        }
    }

    #[test]
    fn gravity_follows_the_current_mass() {
        let settings = GameSettings::default();
        let sys = &settings.systems[0];
        let p = &sys.planets()[0];
        let intact = PlanetProfile::planet(0, 0, sys, p, &WorldDeltas::new());
        let mut deltas = WorldDeltas::new();
        let half = intact.physics.mass_earth.base / 2.0;
        deltas.insert("s0.p0".into(), BodyDelta { mass_earth: -half, ..Default::default() });
        let mined = PlanetProfile::planet(0, 0, sys, p, &deltas);
        assert_eq!(mined.physics.mass_earth.base, intact.physics.mass_earth.base);
        assert!((mined.physics.surface_gravity_g - intact.physics.surface_gravity_g / 2.0).abs() < 1e-9);
        assert!(mined.physics.escape_velocity_km_s < intact.physics.escape_velocity_km_s);
    }

    #[test]
    fn profiles_export_to_json() {
        let settings = GameSettings::default();
        let sys = &settings.systems[0];
        let json = serde_json::to_string_pretty(&PlanetProfile::planet(0, 0, sys, &sys.planets()[0], &WorldDeltas::new())).unwrap();
        for section in ["orbit", "physics", "composition", "atmosphere", "climate", "hydrology", "geology", "relief",
                        "biology", "resources", "gameplay", "traits", "layer_seeds"] {
            assert!(json.contains(&format!("\"{section}\"")), "section {section} absente");
        }
        let star = serde_json::to_string(&StarProfile::build(0, 0, sys, &sys.stars[0])).unwrap();
        assert!(star.contains("\"radius_sun\""));
    }
}
