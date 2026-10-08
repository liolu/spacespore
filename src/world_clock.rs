//! Horloge du monde et rotation des astres (0.11, phase A1).
//!
//! - `WorldClock` : secondes de jeu (f64) depuis la création du monde, enregistrées dans
//!   `world.json`, données par l'hôte en multijoueur (règle 9). Tout ce qui bouge est une fonction de
//!   `(graine, temps)` : orbites, rotation, saisons.
//! - Vitesse du temps (Q1) : 1 h de la planète = 1 min de jeu (jour terrestre = 24 min), un jour ne
//!   dépasse pas 3 h de jeu. Saisons (Q2) : proportionnelles à la vraie période orbitale, 1 h de
//!   jeu en moyenne, entre 10 min et 6 h.
//! - Rotation : autour de l'axe incliné (`axial_tilt`) ; rotation synchrone = la même face vers
//!   l'étoile (ou vers la planète, pour une lune). L'axe garde sa direction par rapport à l'étoile
//!   au fil de l'année des saisons (été du nord quand il penche vers elle), indépendamment des
//!   orbites affichées.
//! - Repère fixe de l'astre (règle 10) : tout ce qui est posé sur un astre est stocké dans ce repère ;
//!   la rotation n'est qu'une transformation au rendu (`world = centre + spin * local`).

use bevy::prelude::*;
use std::f64::consts::TAU;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::planetgen::climate::{Climate, Moment, Season, DAY_PEAK};
use crate::settings::{GameSettings, MoonConfig, PlanetConfig};

/// Secondes de jeu par heure de la planète (Q1 : 1 h = 1 min).
pub const SECS_PER_PLANET_HOUR: f64 = 60.0;
/// Un jour ne dure pas plus de 3 h de jeu (sauf rotation synchrone).
pub const MAX_DAY_SECS: f64 = 3.0 * 3600.0;
/// Saisons (Q2) : 1 h de jeu en moyenne, bornées entre 10 min et 6 h.
pub const MIN_SEASON_SECS: f64 = 600.0;
pub const MAX_SEASON_SECS: f64 = 6.0 * 3600.0;
/// Période orbitale (jours) dont la saison dure 1 h : réglée pour une moyenne de 1 h sur tout le
/// monde généré (test `seasons_last_one_hour_on_average`).
pub const SEASON_REF_DAYS: f64 = 300.0;
/// Accélération maximale du temps (`/temps`).
pub const MAX_SPEED: f64 = 10_000.0;
/// Écart toléré avec l'horloge de l'hôte avant de s'y recaler (secondes de jeu).
pub const CLOCK_TOLERANCE: f64 = 0.5;

/// Horloge du monde.
#[derive(Resource, Clone, Copy, Debug)]
pub struct WorldClock {
    /// Secondes de jeu depuis la création du monde.
    pub secs: f64,
    /// Facteur d'accélération (`/temps`), 1 en jeu normal.
    pub speed: f64,
}

impl Default for WorldClock {
    fn default() -> Self {
        Self { secs: 0.0, speed: 1.0 }
    }
}

impl WorldClock {
    pub fn advance(&mut self, dt: f64) {
        self.secs += dt * self.speed;
        SAVED.store(self.secs.to_bits(), Ordering::Relaxed);
    }

    /// Recale l'horloge sur celle de l'hôte (s'il y a un écart notable ou une autre vitesse).
    pub fn follow(&mut self, host_secs: f64, host_speed: f64) {
        if !host_secs.is_finite() || !host_speed.is_finite() {
            return;
        }
        self.speed = host_speed.clamp(0.0, MAX_SPEED);
        if (host_secs - self.secs).abs() > CLOCK_TOLERANCE * self.speed.max(1.0) {
            self.secs = host_secs.max(0.0);
        }
    }
}

/// Dernière valeur de l'horloge, lue par la sauvegarde du monde (`settings::WorldSave`).
static SAVED: AtomicU64 = AtomicU64::new(0);

pub fn saved_secs() -> f64 {
    f64::from_bits(SAVED.load(Ordering::Relaxed))
}

pub struct WorldClockPlugin;

impl Plugin for WorldClockPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WorldClock>()
            .add_event::<ClockCommand>()
            .add_systems(Startup, load_clock)
            .add_systems(First, tick_clock)
            .init_resource::<LocalWeather>()
            .add_systems(Update, (save_clock, update_local_weather, run_clock_commands).chain())
            .add_systems(Last, save_clock_on_exit);
    }
}

fn load_clock(settings: Res<GameSettings>, mut clock: ResMut<WorldClock>) {
    clock.secs = settings.world_clock.max(0.0);
    SAVED.store(clock.secs.to_bits(), Ordering::Relaxed);
}

fn tick_clock(time: Res<Time>, mut clock: ResMut<WorldClock>) {
    clock.advance(time.delta_secs_f64());
}

/// Toutes les 30 s (temps réel), l'heure est écrite dans `world.json`.
fn save_clock(time: Res<Time>, settings: Res<GameSettings>, mut last: Local<f64>) {
    let now = time.elapsed_secs_f64();
    if now - *last >= 30.0 {
        *last = now;
        settings.save_world();
    }
}

fn save_clock_on_exit(mut exit: EventReader<AppExit>, settings: Res<GameSettings>) {
    if exit.read().next().is_some() {
        settings.save_world();
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  Jours, années, rotation
// ─────────────────────────────────────────────────────────────────────────

/// Durée d'un jour (rotation) en secondes de jeu : `rotation_h` heures × 1 min, au plus 3 h.
pub fn day_secs(rotation_h: f32) -> f64 {
    let h = if rotation_h > 0.0 { rotation_h as f64 } else { 24.0 };
    (h * SECS_PER_PLANET_HOUR).min(MAX_DAY_SECS)
}

/// Durée d'une saison en secondes de jeu (Q2).
pub fn season_secs(period_days: f32) -> f64 {
    if period_days <= 0.0 {
        return 3600.0;
    }
    (3600.0 * period_days as f64 / SEASON_REF_DAYS).clamp(MIN_SEASON_SECS, MAX_SEASON_SECS)
}

/// Fraction (0..1) tirée d'une graine : décale le départ des jours et des années.
fn phase_of(seed: u32, k: u32) -> f64 {
    let mut x = (seed as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (k as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    x ^= x >> 31;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 29;
    (x >> 11) as f64 / (1u64 << 53) as f64
}

/// Rotation d'un astre (planète ou lune).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spin {
    /// Inclinaison de l'axe (radians).
    pub tilt: f64,
    pub day_s: f64,
    pub year_s: f64,
    pub day_phase: f64,
    pub year_phase: f64,
    /// Rotation synchrone : la face +X regarde toujours l'astre central (étoile ou planète).
    pub locked: bool,
    /// Excentricité de l'orbite (autour de l'étoile) et moment du périhélie dans l'année (0..1).
    pub ecc: f64,
    pub peri: f64,
}

impl Spin {
    pub fn planet(p: &PlanetConfig) -> Self {
        Self {
            tilt: if p.tidally_locked { 0.0 } else { (p.axial_tilt as f64).to_radians() },
            day_s: day_secs(p.rotation_h),
            year_s: 4.0 * season_secs(p.period_days),
            day_phase: phase_of(p.seed, 1),
            year_phase: phase_of(p.seed, 2),
            locked: p.tidally_locked,
            ecc: p.eccentricity as f64,
            peri: phase_of(p.seed, 3),
        }
    }

    /// Une lune tourne en synchrone autour de sa planète (comme la Lune) ; ses saisons sont
    /// celles de sa planète.
    pub fn moon(m: &MoonConfig, parent: &PlanetConfig) -> Self {
        let _ = m;
        Self {
            tilt: 0.0,
            day_s: 0.0,
            year_s: 4.0 * season_secs(parent.period_days),
            day_phase: 0.0,
            year_phase: phase_of(parent.seed, 2),
            locked: true,
            ecc: parent.eccentricity as f64,
            peri: phase_of(parent.seed, 3),
        }
    }

    /// Avancement de l'année (0..1) : 0 = été du nord (l'axe penche vers l'étoile).
    pub fn year_fraction(&self, t: f64) -> f64 {
        (t / self.year_s + self.year_phase).rem_euclid(1.0)
    }

    /// Saison à l'instant `t` pour un astre de température moyenne `mean_c` (°C) : déclinaison de
    /// l'étoile avec le retard des saisons (un mois sur douze, l'été le plus chaud vient après le
    /// solstice) et écart dû à l'excentricité (plus chaud au périhélie : T ∝ r^-1/2).
    pub fn season(&self, t: f64, mean_c: f32) -> Season {
        let y = self.year_fraction(t);
        let decl = if self.locked { 0.0 } else { (self.tilt.sin() * (TAU * (y - SEASON_LAG)).cos()).asin() as f32 };
        let offset = ((mean_c as f64 + 273.15).max(0.0) * 0.5 * self.ecc * (TAU * (y - self.peri - SEASON_LAG)).cos()) as f32;
        Season { decl, offset, sun_lon: None }
    }

    /// L'astre a des saisons (axe incliné de plus de 3°).
    pub fn has_seasons(&self) -> bool {
        !self.locked && self.tilt.sin().abs() > 3f64.to_radians().sin()
    }

    /// Orientation de l'astre à l'instant `t`. `to_center` : direction (monde) de l'astre vers son
    /// étoile, ou vers sa planète pour une lune.
    pub fn rotation(&self, t: f64, to_center: Vec3) -> Quat {
        let flat = Vec3::new(to_center.x, 0.0, to_center.z);
        let d = flat.try_normalize().unwrap_or(Vec3::X);
        if self.locked {
            // La face +X vers le centre : rot_y(θ) · X = (cos θ, 0, −sin θ)
            return Quat::from_rotation_y((-d.z).atan2(d.x));
        }
        // L'axe penche vers une direction qui fait le tour de l'étoile en une année
        let phi = (TAU * self.year_fraction(t)) as f32;
        let lean = Quat::from_rotation_y(phi) * d;
        let (s, c) = (self.tilt.sin() as f32, self.tilt.cos() as f32);
        let axis = (Vec3::Y * c + lean * s).normalize();
        let tilt = Quat::from_rotation_arc(Vec3::Y, axis);
        let angle = TAU * (t / self.day_s + self.day_phase).rem_euclid(1.0);
        (tilt * Quat::from_rotation_y(angle as f32)).normalize()
    }
}

/// Retard des saisons sur l'étoile (fraction d'année).
pub const SEASON_LAG: f64 = 1.0 / 12.0;

/// Longitude (repère fixe de l'astre) du point où l'étoile est au zénith.
pub fn sun_longitude(rotation: Quat, to_star: Vec3) -> f32 {
    let s = rotation.inverse() * to_star;
    s.x.atan2(s.z)
}

/// Heure locale (0..24) en un point `p` du repère fixe de l'astre, l'étoile étant dans la
/// direction `to_star` (monde). Midi : l'étoile au plus haut.
pub fn local_hour(rotation: Quat, to_star: Vec3, p: Vec3) -> f32 {
    let s = rotation.inverse() * to_star;
    let lp = p.x.atan2(p.z);
    let ls = s.x.atan2(s.z);
    (12.0 + 24.0 * (lp - ls) / std::f32::consts::TAU).rem_euclid(24.0)
}

/// Hauteur de l'étoile (radians) vue d'un point `p` du repère fixe de l'astre.
pub fn sun_elevation(rotation: Quat, to_star: Vec3, p: Vec3) -> f32 {
    let s = (rotation.inverse() * to_star).normalize_or(Vec3::Y);
    s.dot(p.normalize_or(Vec3::Y)).clamp(-1.0, 1.0).asin()
}

/// Saison à la latitude `lat` (radians) : été, automne, hiver, printemps (hémisphère sud décalé).
pub fn season_name(spin: &Spin, t: f64, lat: f32) -> &'static str {
    if !spin.has_seasons() {
        return "aucune saison";
    }
    let mut k = ((spin.year_fraction(t) * 4.0 + 0.5).floor() as i64).rem_euclid(4);
    // Axe à plus de 90° (rotation rétrograde) : le « nord » est en dessous
    let south = (lat < 0.0) != (spin.tilt.cos() < 0.0);
    if south {
        k = (k + 2) % 4;
    }
    ["ete", "automne", "hiver", "printemps"][k as usize]
}

/// Heure lisible : « 14 h 05 ».
pub fn hour_text(h: f32) -> String {
    let m = (h * 60.0).round() as i64 % (24 * 60);
    format!("{} h {:02}", m / 60, m % 60)
}

// ─────────────────────────────────────────────────────────────────────────
//  Météo locale : heure, saison, température (HUD, scanner, /heure)
// ─────────────────────────────────────────────────────────────────────────

/// Heure, saison et températures là où l'on est (posé, en vol bas) ou sous le vaisseau, sur
/// l'astre visité ou ciblé. Recalculé à chaque image.
#[derive(Resource, Default, Clone)]
pub struct LocalWeather {
    pub body: Option<crate::TargetKind>,
    pub name: String,
    /// Heure locale (0..24), hauteur de l'étoile (degrés), latitude (degrés).
    pub hour: f32,
    pub sun_deg: f32,
    pub lat_deg: f32,
    pub season: &'static str,
    pub locked: bool,
    pub day_s: f64,
    pub season_s: f64,
    /// Température maintenant, minimum et maximum du jour, puis de l'année (°C).
    pub temp: f32,
    pub day: (f32, f32),
    pub year: (f32, f32),
}

impl LocalWeather {
    /// Ligne courte du HUD : « 14 h 05  ete  12 C ».
    pub fn short(&self) -> String {
        if self.body.is_none() {
            return String::new();
        }
        format!("{}  {}  {:.0} C", hour_text(self.hour), self.season, self.temp)
    }

    /// Réponse de `/heure`.
    pub fn long(&self) -> String {
        let day = if self.locked { "rotation synchrone : jour ou nuit eternels".to_string() } else { format!("jour de {}", duration_text(self.day_s)) };
        format!(
            "{} : {}, soleil a {:.0} deg, {} (lat. {:.0} deg), {:.0} C (jour {:.0} a {:.0}, annee {:.0} a {:.0}) - {day}, saison de {}.",
            self.name,
            hour_text(self.hour),
            self.sun_deg,
            self.season,
            self.lat_deg,
            self.temp,
            self.day.0,
            self.day.1,
            self.year.0,
            self.year.1,
            duration_text(self.season_s),
        )
    }
}

/// Rotation et climat (sans saison) d'une planète ou d'une lune, et son nom.
pub fn body_spin(settings: &GameSettings, kind: &crate::TargetKind) -> Option<(Spin, Climate, String)> {
    use crate::TargetKind;
    let climate = crate::surface::body_params(settings, kind)?.climate;
    match *kind {
        TargetKind::Planet(id) => {
            let sys = settings.systems.get(id / 1000)?;
            let p = sys.planets().get(id % 1000)?;
            Some((Spin::planet(p), climate, format!("{} {}", sys.name, id % 1000 + 1)))
        }
        TargetKind::Moon(pid, mi) => {
            let sys = settings.systems.get(pid / 1000)?;
            let planets = sys.planets();
            let p = planets.get(pid % 1000)?;
            let m = p.moons.get(mi)?;
            Some((Spin::moon(m, p), climate, format!("{} {} {}", sys.name, pid % 1000 + 1, (b'a' + mi as u8) as char)))
        }
        _ => None,
    }
}

/// Températures (°C) au point `dir` du repère fixe de l'astre, à l'altitude relative `alt` :
/// maintenant, min / max du jour, min / max de l'année.
pub fn temperatures(spin: &Spin, climate: &Climate, t: f64, hour: f32, dir: Vec3, alt: f32) -> (f32, (f32, f32), (f32, f32)) {
    let lat = dir.y.clamp(-1.0, 1.0).asin();
    let today = climate.at(spin.season(t, climate.mean_c));
    let at = |c: &Climate, h: f32| c.temperature(lat, alt, Some(Moment { hour: h }));
    let now = at(&today, hour / 24.0);
    let (lo, hi) = (DAY_PEAK - 0.5, DAY_PEAK);
    let day = if spin.locked { (now, now) } else { (at(&today, lo), at(&today, hi)) };
    let mut year = (f32::MAX, f32::MIN);
    for k in 0..24 {
        let tk = t + spin.year_s * k as f64 / 24.0;
        let c = climate.at(spin.season(tk, climate.mean_c));
        let (a, b) = if spin.locked { (at(&c, hour / 24.0), at(&c, hour / 24.0)) } else { (at(&c, lo), at(&c, hi)) };
        year = (year.0.min(a), year.1.max(b));
    }
    (now, day, year)
}

#[allow(clippy::too_many_arguments)]
fn update_local_weather(
    clock: Res<WorldClock>,
    settings: Res<GameSettings>,
    surface: Res<crate::surface::Surface>,
    target: Res<crate::CameraTarget>,
    planets: Query<(&Transform, &crate::planet::PlanetId), With<crate::planet::PlanetRoot>>,
    moons: Query<(&Transform, &crate::planet::MoonId), With<crate::planet::MoonRoot>>,
    stars: Query<&Transform, With<crate::planet::StarRoot>>,
    mut weather: ResMut<LocalWeather>,
) {
    use crate::TargetKind;
    let kind = surface.body().unwrap_or(target.0);
    let tf = match kind {
        TargetKind::Planet(id) => planets.iter().find(|(_, p)| p.0 == id).map(|(t, _)| *t),
        TargetKind::Moon(pid, mi) => moons.iter().find(|(_, m)| m.planet_idx == pid && m.moon_idx == mi).map(|(t, _)| *t),
        _ => None,
    };
    let (Some(tf), Some((spin, climate, name))) = (tf, body_spin(&settings, &kind)) else {
        if weather.body.is_some() {
            *weather = LocalWeather::default();
        }
        return;
    };
    let Some(star) = stars.iter().map(|s| s.translation).min_by(|a, b| a.distance_squared(tf.translation).total_cmp(&b.distance_squared(tf.translation))) else { return };
    let to_star = (star - tf.translation).normalize_or(Vec3::X);
    // Le point : là où l'on est posé ou survole, sinon le point de stationnement du vaisseau
    // (fixe dans le repère de l'astre : il ne saute pas quand la caméra tourne ou zoome)
    let p = surface.local_point().unwrap_or_else(|| surface.hover_dir(&kind).unwrap_or(Vec3::Y));
    let dir = p.normalize_or(Vec3::Y);
    let alt = surface.ground_altitude().unwrap_or(0.0);
    let hour = local_hour(tf.rotation, to_star, dir);
    let (temp, day, year) = temperatures(&spin, &climate, clock.secs, hour, dir, alt);
    *weather = LocalWeather {
        body: Some(kind),
        name,
        hour,
        sun_deg: sun_elevation(tf.rotation, to_star, dir).to_degrees(),
        lat_deg: dir.y.clamp(-1.0, 1.0).asin().to_degrees(),
        season: season_name(&spin, clock.secs, dir.y),
        locked: spin.locked,
        day_s: spin.day_s,
        season_s: spin.year_s / 4.0,
        temp,
        day,
        year,
    };
}

// ─────────────────────────────────────────────────────────────────────────
//  Commandes /heure et /temps
// ─────────────────────────────────────────────────────────────────────────

#[derive(Event, Clone, Debug)]
pub enum ClockCommand {
    Hour,
    /// Tests : avance l'horloge jusqu'à cette heure locale (0..24) de l'astre ciblé.
    SetHour(f32),
    Speed(String),
}

fn run_clock_commands(
    time: Res<Time>,
    mut events: EventReader<ClockCommand>,
    mut clock: ResMut<WorldClock>,
    weather: Res<LocalWeather>,
    mut net: ResMut<crate::net::Net>,
) {
    let now = time.elapsed_secs_f64();
    for ev in events.read() {
        match ev {
            ClockCommand::Speed(arg) => {
                if !net.may_set_clock() {
                    net.notify("Seul l'hote de la partie peut changer la vitesse du temps.", now);
                    continue;
                }
                let arg = arg.trim().trim_start_matches(['x', 'X', '*']);
                match arg.replace(',', ".").parse::<f64>() {
                    Ok(f) if (0.0..=MAX_SPEED).contains(&f) => {
                        clock.speed = f;
                        net.notify(&format!("Vitesse du temps : x{f} (1 h de la planete = {:.1} s).", SECS_PER_PLANET_HOUR / f.max(1e-9)), now);
                    }
                    _ if arg.is_empty() => net.notify(&format!("Vitesse du temps : x{} (/temps <facteur>, 1 = normal).", clock.speed), now),
                    _ => net.notify(&format!("/temps <facteur> : de 0 a {MAX_SPEED} (1 = normal)."), now),
                }
            }
            ClockCommand::SetHour(h) => {
                if !net.may_set_clock() {
                    net.notify("Seul l'hote de la partie peut changer l'heure.", now);
                } else if weather.body.is_none() {
                    net.notify("Ciblez une planete ou une lune (chargee) pour regler son heure.", now);
                } else if weather.locked {
                    net.notify("Rotation synchrone : l'heure ne change jamais ici (une face toujours au jour). Choisissez une planete qui tourne.", now);
                } else {
                    let ahead = (h - weather.hour).rem_euclid(24.0) as f64 / 24.0 * weather.day_s;
                    clock.secs += ahead;
                    net.notify(&format!("Heure reglee a {} (horloge avancee de {}).", hour_text(*h), duration_text(ahead)), now);
                }
            }
            ClockCommand::Hour => {
                if weather.body.is_none() {
                    net.notify("Ciblez une planete ou une lune (chargee) pour connaitre son heure.", now);
                } else {
                    net.notify(&weather.long(), now);
                }
            }
        }
    }
}

/// « 24 min », « 1 h 30 ».
pub fn duration_text(secs: f64) -> String {
    let m = (secs / 60.0).round() as i64;
    if m < 60 {
        format!("{m} min")
    } else if m % 60 == 0 {
        format!("{} h", m / 60)
    } else {
        format!("{} h {:02}", m / 60, m % 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn earth() -> Spin {
        Spin { tilt: 23.4f64.to_radians(), day_s: day_secs(24.0), year_s: 4.0 * season_secs(365.25), day_phase: 0.0, year_phase: 0.0, locked: false, ecc: 0.017, peri: 0.53 }
    }

    #[test]
    fn earth_day_lasts_24_minutes_and_is_capped() {
        assert_eq!(day_secs(24.0), 24.0 * 60.0);
        assert_eq!(day_secs(10.0), 600.0);
        assert_eq!(day_secs(5000.0), MAX_DAY_SECS);
        // Terre : saisons de ~1 h 13 (moyenne du monde : 1 h)
        assert!((season_secs(SEASON_REF_DAYS as f32) - 3600.0).abs() < 1.0);
        assert!((season_secs(365.25) / 3600.0 - 1.2175).abs() < 0.01);
        assert_eq!(season_secs(1.0), MIN_SEASON_SECS);
        assert_eq!(season_secs(1e6), MAX_SEASON_SECS);
    }

    #[test]
    fn noon_comes_back_every_day() {
        let s = earth();
        let to_star = Vec3::X;
        let p = Vec3::new(0.3, 0.2, 0.9).normalize();
        let h0 = local_hour(s.rotation(1000.0, to_star), to_star, p);
        let h1 = local_hour(s.rotation(1000.0 + s.day_s * 0.25, to_star), to_star, p);
        let h2 = local_hour(s.rotation(1000.0 + s.day_s, to_star), to_star, p);
        // Six heures plus tard (à l'inclinaison près), le même point un jour plus tard
        assert!(((h1 - h0).rem_euclid(24.0) - 6.0).abs() < 0.6, "{h0} {h1}");
        // (à quelques minutes près : l'axe a un peu tourné avec l'année, comme le jour solaire)
        let diff = (h2 - h0 + 12.0).rem_euclid(24.0) - 12.0;
        assert!(diff.abs() < 0.15, "{h0} {h2}");
    }

    #[test]
    fn locked_bodies_keep_one_face_to_their_star() {
        let s = Spin { locked: true, tilt: 0.0, ..earth() };
        for (t, to_star) in [(0.0, Vec3::X), (5000.0, Vec3::new(-1.0, 0.0, 1.0).normalize()), (1e7, Vec3::NEG_Z)] {
            let r = s.rotation(t, to_star);
            // La face +X de l'astre regarde l'étoile : midi éternel
            assert!((r * Vec3::X).dot(to_star) > 0.999);
            assert!((local_hour(r, to_star, Vec3::X) - 12.0).abs() < 0.01);
        }
    }

    #[test]
    fn seasons_follow_the_tilted_axis() {
        let s = earth();
        let to_star = Vec3::X;
        let north = Vec3::new(0.0, 0.8, 0.6).normalize();
        // Été du nord (année = 0) : l'étoile monte haut au nord ; six mois plus tard, l'inverse
        let summer = s.rotation(0.0, to_star);
        let winter = s.rotation(s.year_s * 0.5, to_star);
        let decl = |r: Quat| (r.inverse() * to_star).y.asin().to_degrees();
        assert!((decl(summer) - 23.4).abs() < 0.5, "{}", decl(summer));
        assert!((decl(winter) + 23.4).abs() < 0.5, "{}", decl(winter));
        assert_eq!(season_name(&s, 0.0, north.y.asin()), "ete");
        assert_eq!(season_name(&s, 0.0, -0.5), "hiver");
        assert_eq!(season_name(&s, s.year_s * 0.25, 0.5), "automne");
        let flat = Spin { tilt: 0.01, ..s };
        assert_eq!(season_name(&flat, 0.0, 0.5), "aucune saison");
    }

    /// Règle 10 : un point posé dans le repère de l'astre suit sa rotation, même après des heures
    /// de jeu (précision f32 sur des temps f64).
    #[test]
    fn a_point_on_the_ground_stays_put() {
        let s = earth();
        let to_star = Vec3::X;
        let local = Vec3::new(0.2, 0.5, -0.8).normalize() * 12_000.0;
        for t in [0.0, 600.0, 3.6e6, 3.6e6 + 1.0 / 60.0] {
            let r = s.rotation(t, to_star);
            let world = r * local;
            let back = r.inverse() * world;
            assert!((back - local).length() < 0.02, "t = {t} : {}", (back - local).length());
            assert!((world.length() - local.length()).abs() < 0.02);
        }
        // Une image (1/60 s) plus tard, au bout de 1 000 h de jeu, le sol a bougé de quelques
        // unités au plus (pas de saut)
        let a = s.rotation(3.6e6, to_star) * local;
        let b = s.rotation(3.6e6 + 1.0 / 60.0, to_star) * local;
        assert!(a.distance(b) < 2.0, "{}", a.distance(b));
    }

    #[test]
    fn clients_follow_the_host_clock() {
        let mut c = WorldClock::default();
        c.advance(10.0);
        c.follow(10.2, 1.0);
        assert_eq!(c.secs, 10.0, "petit ecart : on garde son horloge");
        c.follow(500.0, 10.0);
        assert_eq!((c.secs, c.speed), (500.0, 10.0));
        c.advance(1.0);
        assert_eq!(c.secs, 510.0);
    }

    /// Q2 : une saison dure 1 h de jeu en moyenne sur tout le monde genere.
    #[test]
    fn seasons_last_one_hour_on_average() {
        use crate::settings::{default_galaxies, default_systems, DEFAULT_WORLD_SEED};
        let galaxies = default_galaxies(DEFAULT_WORLD_SEED);
        let systems = default_systems(&galaxies, DEFAULT_WORLD_SEED);
        // (sans les planètes errantes : pas d'étoile, pas de saisons)
        let v: Vec<f64> = systems.dense().iter().take(3000).flat_map(|s| s.planets_uncached().iter().filter(|p| !p.rogue).map(|p| season_secs(p.period_days)).collect::<Vec<_>>()).collect();
        let mean = v.iter().sum::<f64>() / v.len() as f64;
        println!("saison moyenne : {mean:.0} s");
        // (recalée en C3 : les planètes des étoiles doubles serrées ont des années plus longues ; en 0.13.6 la zone
        // d'influence des étoiles retire des planètes lointaines aux années longues : ~49 min)
        assert!((2800.0..4200.0).contains(&mean), "{mean}");
        assert!(v.iter().all(|s| (MIN_SEASON_SECS..=MAX_SEASON_SECS).contains(s)));
    }

    /// A3 : la Terre (≈ 10 °C entre le jour et la nuit), la Lune (≈ 250 °C), Vénus (presque rien).
    #[test]
    fn day_and_night_temperatures_of_earth_moon_and_venus() {
        use crate::planetgen::atmosphere::diurnal_amplitude;
        let swing = |t_k: f32, bar: f32, rotation_h: f32| {
            let c = Climate { mean_c: t_k - 273.15, diurnal: diurnal_amplitude(t_k, bar, rotation_h), ..Default::default() };
            let spin = Spin { tilt: 0.0, ..earth() };
            let (_, day, _) = temperatures(&spin, &c, 0.0, 12.0, Vec3::new(1.0, 0.0, 0.0), 0.0);
            day.1 - day.0
        };
        let earth = swing(288.0, 1.0, 24.0);
        let moon = swing(250.0, 0.0, 708.0);
        let venus = swing(737.0, 92.0, 2802.0);
        let mars = swing(210.0, 0.006, 24.6);
        assert!((7.0..15.0).contains(&earth), "terre {earth}");
        assert!((220.0..320.0).contains(&moon), "lune {moon}");
        assert!(venus < 5.0, "venus {venus}");
        assert!((50.0..110.0).contains(&mars), "mars {mars}");
    }

    /// A3 : l'été du nord est plus chaud que son hiver, et l'hémisphère sud est à l'inverse.
    #[test]
    fn summer_is_warmer_than_winter() {
        let s = earth();
        let c = Climate { mean_c: 15.0, span: 50.0, lapse: 50.0, diurnal: 5.0, tilt: 23.4, ..Default::default() };
        let north = Vec3::new(0.7, 0.7, 0.0).normalize();
        let south = Vec3::new(0.7, -0.7, 0.0).normalize();
        // Été du nord un peu après le solstice (retard des saisons)
        let summer = s.year_s * SEASON_LAG;
        let winter = summer + s.year_s * 0.5;
        let t = |time: f64, dir: Vec3| temperatures(&s, &c, time, 12.0, dir, 0.0).0;
        assert!(t(summer, north) > t(winter, north) + 12.0, "{} {}", t(summer, north), t(winter, north));
        assert!(t(summer, south) < t(winter, south) - 12.0);
        // Min / max de l'année encadrent ceux du jour
        let (_, day, year) = temperatures(&s, &c, summer, 12.0, north, 0.0);
        assert!(year.0 <= day.0 && year.1 >= day.1 - 0.5);
    }
}
