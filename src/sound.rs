//! Son (0.13 P9) : le premier système de son du jeu (la musique reste pour plus tard). Aucun
//! fichier : les sons sont synthétisés au lancement (bruits filtrés, sinus) et mis en WAV en
//! mémoire. Des boucles tournent en permanence, muettes ; chaque image leur donne un volume et une
//! hauteur d'après le vol (`approche::FlightInfo`) :
//!
//! - sifflement de l'air : monte avec la vitesse, proportionnel à la densité de l'air ;
//! - grondement de la rentrée : suit la chaleur ;
//! - moteurs : suivent la poussée, étouffés dans le vide ;
//! - **silence dans le vide** : sans air, plus rien ne passe.
//!
//! Sons ponctuels : bang supersonique (`Boom`), tonnerre (éclairs de `weather.rs`, retardé par la
//! distance), avec le retard du son pour les autres joueurs.
//!
//! 0.14 D5, les sons du monde (toujours synthétisés, pas de musique : décision Q8) : pluie, vagues près d'un
//! rivage, rivière près d'un lit, lave et geysers près d'un évent actif, bulles sous l'eau (le reste s'étouffe),
//! bourdon et gouttes des grottes (le dehors s'éteint avec la profondeur), insectes la nuit là où il y a des
//! plantes, cris des bêtes de la faune à portée. Trois catégories de volume (`GameSettings::sound_world`,
//! `sound_ship`, `sound_weather`, `/volume monde|vaisseau|meteo`). Les boucles glissent d'un volume à l'autre
//! (jamais de coupure) et tout passe par `air_gain` : silence dans le vide.

#![allow(dead_code)]

use std::sync::Arc;

use bevy::audio::{AudioSink, AudioSinkPlayback, AudioSource, PlaybackSettings, Volume};
use bevy::prelude::*;

use crate::approche_fx::Boom;
use crate::settings::GameSettings;
use crate::surface::Surface;

pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SoundBank>()
            .add_event::<RemoteBoom>()
            .init_resource::<Pending>()
            .add_systems(Startup, setup_sounds)
            .add_systems(Update, (update_loops, one_shots, remote_booms, creature_calls).chain().after(crate::surface::SurfaceControl));
    }
}

const RATE: u32 = 22_050;

/// Les sons synthétisés.
#[derive(Resource, Default)]
struct SoundBank {
    wind: Handle<AudioSource>,
    rumble: Handle<AudioSource>,
    engine: Handle<AudioSource>,
    boom: Handle<AudioSource>,
    thunder: Handle<AudioSource>,
    /// Sons du monde (0.14 D5).
    rain: Handle<AudioSource>,
    waves: Handle<AudioSource>,
    river: Handle<AudioSource>,
    lava: Handle<AudioSource>,
    geyser: Handle<AudioSource>,
    bubbles: Handle<AudioSource>,
    cave: Handle<AudioSource>,
    insects: Handle<AudioSource>,
    /// Cris des bêtes : hurlement, pépiement, coassement.
    calls: Vec<Handle<AudioSource>>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Loop {
    Wind,
    Rumble,
    Engine,
    Rain,
    Waves,
    River,
    Lava,
    Geyser,
    Bubbles,
    Cave,
    Insects,
}

impl Loop {
    const ALL: [Loop; 11] = [Loop::Wind, Loop::Rumble, Loop::Engine, Loop::Rain, Loop::Waves, Loop::River, Loop::Lava, Loop::Geyser, Loop::Bubbles, Loop::Cave, Loop::Insects];
}

#[derive(Component)]
struct SoundLoop(Loop);

/// Sons à jouer plus tard (retard du son) : instant, force, 0 = tonnerre / 1 = bang.
#[derive(Resource, Default)]
struct Pending(Vec<(f64, f32, u8)>);

// ─────────────────────────────────────────────────────────────────────────
//  Synthèse
// ─────────────────────────────────────────────────────────────────────────

struct Rng(u32);

impl Rng {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1664525).wrapping_add(1013904223);
        ((self.0 >> 8) as f32 / (1u32 << 24) as f32) * 2.0 - 1.0
    }
}

/// Octets d'un fichier WAV (mono, 16 bits) à partir d'échantillons -1..1.
fn wav(samples: &[f32]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut b = Vec::with_capacity(44 + data_len as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&RATE.to_le_bytes());
    b.extend_from_slice(&(RATE * 2).to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        b.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32000.0) as i16).to_le_bytes());
    }
    b
}

/// Rend une boucle sans coupure : la fin se fond dans le début.
fn make_loop(mut s: Vec<f32>, fade: usize) -> Vec<f32> {
    let n = s.len();
    for i in 0..fade {
        let k = i as f32 / fade as f32;
        s[i] = s[i] * k + s[n - fade + i] * (1.0 - k);
    }
    s.truncate(n - fade);
    s
}

fn lowpass(s: &mut [f32], cutoff_hz: f32) {
    let a = (-std::f32::consts::TAU * cutoff_hz / RATE as f32).exp();
    let mut y = 0.0;
    for x in s.iter_mut() {
        y = a * y + (1.0 - a) * *x;
        *x = y;
    }
}

fn normalize(s: &mut [f32], peak: f32) {
    let m = s.iter().fold(1e-6f32, |m, x| m.max(x.abs()));
    for x in s.iter_mut() {
        *x *= peak / m;
    }
}

/// Souffle : bruit passe-bas et passe-haut (de l'air qui siffle).
fn wind_sound() -> Vec<f32> {
    let n = RATE as usize * 5;
    let mut r = Rng(12345);
    let mut s: Vec<f32> = (0..n).map(|_| r.next()).collect();
    let mut low = s.clone();
    lowpass(&mut low, 2200.0);
    let mut lower = s.clone();
    lowpass(&mut lower, 300.0);
    for i in 0..n {
        // Bruit moyen moins le grave : un sifflement ; une lente variation donne les rafales
        let gust = 0.75 + 0.25 * (i as f32 / RATE as f32 * std::f32::consts::TAU * 0.35).sin();
        s[i] = (low[i] - lower[i] * 0.8) * gust;
    }
    normalize(&mut s, 0.8);
    make_loop(s, RATE as usize / 2)
}

/// Grondement grave de la rentrée (bruit brun, ondulant).
fn rumble_sound() -> Vec<f32> {
    let n = RATE as usize * 4;
    let mut r = Rng(777);
    let mut s: Vec<f32> = (0..n).map(|_| r.next()).collect();
    lowpass(&mut s, 160.0);
    for (i, x) in s.iter_mut().enumerate() {
        let t = i as f32 / RATE as f32;
        *x *= 0.75 + 0.25 * (t * std::f32::consts::TAU * 7.0).sin();
    }
    normalize(&mut s, 0.9);
    make_loop(s, RATE as usize / 4)
}

/// Moteurs : un bourdonnement grave (sons harmoniques) et un souffle.
fn engine_sound() -> Vec<f32> {
    let n = RATE as usize * 2;
    let mut r = Rng(4242);
    let mut noise: Vec<f32> = (0..n).map(|_| r.next()).collect();
    lowpass(&mut noise, 700.0);
    let s: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let tau = std::f32::consts::TAU;
            // Fréquences entières par seconde : la boucle se referme sans saut
            0.5 * (tau * 55.0 * t).sin() + 0.3 * (tau * 110.0 * t).sin() + 0.18 * (tau * 165.0 * t).sin() + 0.35 * noise[i]
        })
        .collect();
    let mut s = s;
    normalize(&mut s, 0.85);
    s
}

/// Bang supersonique : une onde grave qui retombe, avec un claquement.
fn boom_sound() -> Vec<f32> {
    let n = RATE as usize * 3;
    let mut r = Rng(99);
    let mut noise: Vec<f32> = (0..n).map(|_| r.next()).collect();
    lowpass(&mut noise, 900.0);
    let mut phase = 0.0f32;
    let mut s = vec![0.0f32; n];
    for i in 0..n {
        let t = i as f32 / RATE as f32;
        let freq = 28.0 + 70.0 * (-t * 5.0).exp();
        phase += std::f32::consts::TAU * freq / RATE as f32;
        let env = (-t * 1.6).exp();
        let crack = (-t * 40.0).exp();
        s[i] = (phase.sin() * 0.9 + noise[i] * 0.6 * crack * 3.0) * env.max(crack);
    }
    normalize(&mut s, 0.95);
    s
}

/// Tonnerre : un grondement qui roule, avec un claquement au début.
fn thunder_sound() -> Vec<f32> {
    let n = RATE as usize * 6;
    let mut r = Rng(31337);
    let mut s: Vec<f32> = (0..n).map(|_| r.next()).collect();
    lowpass(&mut s, 420.0);
    for (i, x) in s.iter_mut().enumerate() {
        let t = i as f32 / RATE as f32;
        let roll = 0.6 + 0.4 * (t * std::f32::consts::TAU * 2.3 + (t * 5.0).sin()).sin();
        let env = (-t * 0.55).exp() * (1.0 - (-t * 25.0).exp());
        *x *= env * roll;
    }
    normalize(&mut s, 0.95);
    s
}

fn noise(seed: u32, secs: f32) -> Vec<f32> {
    let n = (RATE as f32 * secs) as usize;
    let mut r = Rng(seed);
    (0..n).map(|_| r.next()).collect()
}

/// Pluie : un crépitement (bruit sans le grave) semé de gouttes plus nettes.
fn rain_sound() -> Vec<f32> {
    let mut s = noise(2024, 4.0);
    let mut low = s.clone();
    lowpass(&mut low, 1200.0);
    let mut r = Rng(77);
    let mut drop = 0.0f32;
    for i in 0..s.len() {
        if r.next() > 0.9985 {
            drop = 1.0;
        }
        drop *= 0.985;
        s[i] = (s[i] - low[i]) * 0.5 + drop * r.next() * 0.6;
    }
    normalize(&mut s, 0.8);
    make_loop(s, RATE as usize / 4)
}

/// Vagues : un souffle grave qui enfle et se retire toutes les six secondes (deux vagues décalées).
fn waves_sound() -> Vec<f32> {
    let mut s = noise(3141, 12.0);
    lowpass(&mut s, 800.0);
    let tau = std::f32::consts::TAU;
    for (i, x) in s.iter_mut().enumerate() {
        let t = i as f32 / RATE as f32;
        let a = (0.5 + 0.5 * (tau * t / 6.0).sin()).powi(3);
        let b = (0.5 + 0.5 * (tau * t / 6.0 + 2.4).sin()).powi(4) * 0.6;
        *x *= 0.12 + a + b;
    }
    normalize(&mut s, 0.85);
    make_loop(s, RATE as usize / 2)
}

/// Rivière : un clapotis moyen, modulé par des bouillons.
fn river_sound() -> Vec<f32> {
    let mut s = noise(2718, 3.0);
    let mut low = s.clone();
    lowpass(&mut low, 2500.0);
    let mut lower = s.clone();
    lowpass(&mut lower, 500.0);
    let tau = std::f32::consts::TAU;
    for i in 0..s.len() {
        let t = i as f32 / RATE as f32;
        let bubble = 0.7 + 0.3 * ((tau * 3.0 * t).sin() * (tau * 7.0 * t + 1.0).sin());
        s[i] = (low[i] - lower[i] * 0.6) * bubble;
    }
    normalize(&mut s, 0.75);
    make_loop(s, RATE as usize / 3)
}

/// Lave : un grondement très grave qui crépite.
fn lava_sound() -> Vec<f32> {
    let mut s = noise(1618, 5.0);
    lowpass(&mut s, 90.0);
    let mut r = Rng(161);
    let mut crack = 0.0f32;
    let tau = std::f32::consts::TAU;
    for (i, x) in s.iter_mut().enumerate() {
        let t = i as f32 / RATE as f32;
        if r.next() > 0.9992 {
            crack = 0.8;
        }
        crack *= 0.97;
        *x = *x * 6.0 * (0.8 + 0.2 * (tau * 0.4 * t).sin()) + crack * r.next() * 0.5;
    }
    normalize(&mut s, 0.9);
    make_loop(s, RATE as usize / 2)
}

/// Geyser : un sifflement de vapeur (bande moyenne) qui ondule.
fn geyser_sound() -> Vec<f32> {
    let mut s = noise(4321, 3.0);
    let mut hi = s.clone();
    lowpass(&mut hi, 3500.0);
    let mut lo = s.clone();
    lowpass(&mut lo, 700.0);
    let tau = std::f32::consts::TAU;
    for i in 0..s.len() {
        let t = i as f32 / RATE as f32;
        s[i] = (hi[i] - lo[i]) * (0.75 + 0.25 * (tau * 2.0 * t).sin());
    }
    normalize(&mut s, 0.8);
    make_loop(s, RATE as usize / 3)
}

/// Sous l'eau : un grave sourd et des bulles.
fn bubbles_sound() -> Vec<f32> {
    let mut s = noise(5555, 4.0);
    lowpass(&mut s, 350.0);
    let mut r = Rng(55);
    let mut blip = 0.0f32;
    let mut phase = 0.0f32;
    for x in s.iter_mut() {
        if r.next() > 0.9995 {
            blip = 1.0;
        }
        blip *= 0.992;
        phase += std::f32::consts::TAU * (300.0 + 400.0 * blip) / RATE as f32;
        *x = *x * 3.0 + blip * phase.sin() * 0.5;
    }
    normalize(&mut s, 0.8);
    make_loop(s, RATE as usize / 2)
}

/// Grotte : un bourdon très grave et des gouttes qui résonnent.
fn cave_sound() -> Vec<f32> {
    let n = RATE as usize * 8;
    let mut r = Rng(8080);
    let tau = std::f32::consts::TAU;
    let mut s = vec![0.0f32; n];
    let mut drip = 0.0f32;
    let mut phase = 0.0f32;
    for (i, x) in s.iter_mut().enumerate() {
        let t = i as f32 / RATE as f32;
        if r.next() > 0.99993 {
            drip = 1.0;
        }
        drip *= 0.9993;
        phase += tau * 1800.0 / RATE as f32;
        // (fréquences entières par 8 s : la boucle se referme)
        *x = 0.5 * (tau * 45.0 * t).sin() + 0.3 * (tau * 67.5 * t).sin() * (0.6 + 0.4 * (tau * 0.125 * t).sin()) + drip * phase.sin() * 0.4;
    }
    normalize(&mut s, 0.7);
    s
}

/// Insectes de nuit : des stridulations aiguës par rafales.
fn insects_sound() -> Vec<f32> {
    let n = RATE as usize * 4;
    let tau = std::f32::consts::TAU;
    let mut s = vec![0.0f32; n];
    for (i, x) in s.iter_mut().enumerate() {
        let t = i as f32 / RATE as f32;
        let a = (0.5 + 0.5 * (tau * 9.0 * t).sin()).powi(6) * (0.5 + 0.5 * (tau * 0.5 * t).sin()).powi(2);
        let b = (0.5 + 0.5 * (tau * 13.0 * t + 1.0).sin()).powi(8) * (0.5 + 0.5 * (tau * 0.25 * t + 2.0).sin()).powi(2);
        *x = a * (tau * 3800.0 * t).sin() * 0.6 + b * (tau * 5200.0 * t).sin() * 0.35;
    }
    normalize(&mut s, 0.6);
    s
}

/// Cris des bêtes : hurlement (sinus qui monte puis descend, vibrato), pépiement (trois notes aiguës),
/// coassement (bruit grave modulé).
fn call_sounds() -> Vec<Vec<f32>> {
    let tau = std::f32::consts::TAU;
    let howl: Vec<f32> = {
        let n = (RATE as f32 * 1.6) as usize;
        let mut phase = 0.0f32;
        (0..n)
            .map(|i| {
                let t = i as f32 / RATE as f32;
                let f = 280.0 + 220.0 * (tau * t / 3.2).sin() + 12.0 * (tau * 6.0 * t).sin();
                phase += tau * f / RATE as f32;
                let env = (tau * t / 3.2).sin().max(0.0).powf(0.5);
                (phase.sin() * 0.7 + (2.0 * phase).sin() * 0.25) * env
            })
            .collect()
    };
    let chirp: Vec<f32> = {
        let n = (RATE as f32 * 0.9) as usize;
        (0..n)
            .map(|i| {
                let t = i as f32 / RATE as f32;
                let note = (t * 5.0).floor();
                let f = 1800.0 + 400.0 * (note * 1.7).sin();
                let env = ((t * 5.0).fract() * tau).sin().max(0.0).powi(2) * if note < 3.0 { 1.0 } else { 0.0 };
                (tau * f * t).sin() * env
            })
            .collect()
    };
    let croak: Vec<f32> = {
        let mut s = noise(9090, 0.5);
        lowpass(&mut s, 260.0);
        for (i, x) in s.iter_mut().enumerate() {
            let t = i as f32 / RATE as f32;
            *x *= 5.0 * (0.5 + 0.5 * (tau * 90.0 * t).sin()) * (-t * 4.0).exp() * (1.0 - (-t * 60.0).exp());
        }
        s
    };
    let mut out = vec![howl, chirp, croak];
    for s in &mut out {
        normalize(s, 0.8);
    }
    out
}

fn source(samples: &[f32]) -> AudioSource {
    AudioSource { bytes: Arc::from(wav(samples).into_boxed_slice()) }
}

fn setup_sounds(mut commands: Commands, mut sources: ResMut<Assets<AudioSource>>, mut bank: ResMut<SoundBank>) {
    bank.wind = sources.add(source(&wind_sound()));
    bank.rumble = sources.add(source(&rumble_sound()));
    bank.engine = sources.add(source(&engine_sound()));
    bank.boom = sources.add(source(&boom_sound()));
    bank.thunder = sources.add(source(&thunder_sound()));
    bank.rain = sources.add(source(&rain_sound()));
    bank.waves = sources.add(source(&waves_sound()));
    bank.river = sources.add(source(&river_sound()));
    bank.lava = sources.add(source(&lava_sound()));
    bank.geyser = sources.add(source(&geyser_sound()));
    bank.bubbles = sources.add(source(&bubbles_sound()));
    bank.cave = sources.add(source(&cave_sound()));
    bank.insects = sources.add(source(&insects_sound()));
    bank.calls = call_sounds().iter().map(|c| sources.add(source(c))).collect();
    for kind in Loop::ALL {
        let handle = match kind {
            Loop::Wind => bank.wind.clone(),
            Loop::Rumble => bank.rumble.clone(),
            Loop::Engine => bank.engine.clone(),
            Loop::Rain => bank.rain.clone(),
            Loop::Waves => bank.waves.clone(),
            Loop::River => bank.river.clone(),
            Loop::Lava => bank.lava.clone(),
            Loop::Geyser => bank.geyser.clone(),
            Loop::Bubbles => bank.bubbles.clone(),
            Loop::Cave => bank.cave.clone(),
            Loop::Insects => bank.insects.clone(),
        };
        commands.spawn((AudioPlayer::new(handle), PlaybackSettings::LOOP.with_volume(Volume::new(0.0)), SoundLoop(kind)));
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  Pilotage des boucles
// ─────────────────────────────────────────────────────────────────────────

/// Part du son qui passe : 0 dans le vide, 1 dans un air dense.
pub fn air_gain(density: f32) -> f32 {
    (density / (density + 0.06)).clamp(0.0, 1.0)
}

/// Ce que le monde fait entendre autour du joueur (0..1 chacun), d'après le terrain, la météo, les évents, la
/// faune : calculé une fois par image, lissé ensuite.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WorldMix {
    pub rain: f32,
    pub waves: f32,
    pub river: f32,
    pub lava: f32,
    pub geyser: f32,
    pub bubbles: f32,
    pub cave: f32,
    pub insects: f32,
}

/// Les sons du monde au point du joueur (dans le repère fixe de l'astre), en vol bas ou à pied.
pub fn world_mix(t: &crate::terrain::Terrain, here: Vec3, alt_voxels: f32, sample: &crate::weather::Sample, under: bool, underground: f32, night: f32, now: f64) -> WorldMix {
    let p = &t.params;
    let v = t.voxel();
    let up = here.normalize_or(Vec3::Y);
    let low = (1.0 - alt_voxels / 150.0).clamp(0.0, 1.0);
    let outside = (1.0 - 0.9 * underground).max(0.0);
    let mut m = WorldMix::default();
    if under {
        m.bubbles = 1.0;
    }
    m.cave = underground;
    m.rain = if sample.kind == crate::weather::Precip::None { 0.0 } else { sample.precip.clamp(0.0, 1.0) * outside };
    if low <= 0.0 {
        return m;
    }
    // Vagues : la mer à moins de 200 voxels (anneaux de 8 directions)
    let east = Vec3::Y.cross(up).normalize_or(Vec3::X);
    let north = up.cross(east);
    let mut shore: f32 = if t.sea_surface(up).is_some() { 1.0 } else { 0.0 };
    for (k, dist) in [30.0f32, 80.0, 200.0].into_iter().enumerate() {
        let ang = dist * v / p.radius.max(1.0);
        for a in 0..8 {
            let az = (a as f32 + 0.5 * k as f32) * std::f32::consts::TAU / 8.0;
            let d = (up * ang.cos() + (east * az.cos() + north * az.sin()) * ang.sin()).normalize();
            if t.sea_surface(d).is_some_and(|(_, kind)| kind.is_liquid()) {
                shore = shore.max(1.0 - dist / 220.0);
            }
        }
    }
    // (une mer calme sans vent fait peu de vagues)
    m.waves = shore * (0.35 + 0.65 * (sample.wind_speed() / 15.0).min(1.0)) * low * outside;
    // Rivière : un lit à moins de 60 voxels, plus fort quand il est large
    if let Some(net) = &t.rivers {
        if let Some((dir, width)) = net.nearest(up, 0.0) {
            let d = (dir - up).length() * p.radius / v;
            let reach = 40.0 + (width / v).min(60.0);
            m.river = (1.0 - d / reach).clamp(0.0, 1.0) * (0.4 + 0.6 * (width / v / 30.0).min(1.0)) * low * outside;
        }
    }
    // Lave et geysers : les évents actifs à portée
    for vent in crate::geoactive::vents_near(p, up, 1, &|d| t.vent_ground(d)) {
        let d = (vent.dir - up).length() * p.radius / v;
        let s = crate::geoactive::strength_in(&vent, now, &p.geo);
        match vent.kind {
            crate::geoactive::VentKind::Lava => m.lava = m.lava.max((1.0 - d / 300.0).clamp(0.0, 1.0) * s.max(0.3)),
            crate::geoactive::VentKind::Geyser | crate::geoactive::VentKind::Cryovolcano => m.geyser = m.geyser.max((1.0 - d / 150.0).clamp(0.0, 1.0) * s),
            crate::geoactive::VentKind::Fumarole => m.geyser = m.geyser.max((1.0 - d / 60.0).clamp(0.0, 1.0) * s * 0.4),
        }
    }
    m.lava *= low.max(0.3) * outside;
    m.geyser *= low * outside;
    // Insectes : la nuit, là où poussent des plantes
    if p.biomes.flora && p.atmosphere {
        m.insects = night * low * outside * if m.rain > 0.3 { 0.3 } else { 1.0 };
    }
    if under {
        let k = 0.1;
        m.rain *= k;
        m.waves *= k;
        m.river *= k;
        m.insects *= k;
        m.cave *= k;
    }
    m
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn update_loops(
    time: Res<Time>,
    clock: Res<crate::world_clock::WorldClock>,
    settings: Res<GameSettings>,
    surface: Res<Surface>,
    weather: Res<crate::weather::WeatherNow>,
    under: Res<crate::water::Underwater>,
    mut loops: Query<(&SoundLoop, &mut AudioSink)>,
    mut smooth: Local<[f32; 11]>,
    mut mix: Local<(WorldMix, f64)>,
) {
    let dt = time.delta_secs().min(0.1);
    let master = settings.sound_volume.clamp(0.0, 1.0);
    let (vol_world, vol_ship, vol_weather) = (settings.sound_world.clamp(0.0, 1.0), settings.sound_ship.clamp(0.0, 1.0), settings.sound_weather.clamp(0.0, 1.0));
    let f = *surface.flight();
    let (mut wind, mut rumble, mut engine, mut pitch_wind) = (0.0f32, 0.0f32, 0.0f32, 1.0f32);
    let mut air = 0.0;
    if surface.active() && f.active {
        air = air_gain(f.density);
        // Sifflement : monte avec la vitesse (par rapport à celle du son), plus fort dans un air dense
        let rel = (f.speed_vox / f.sound.max(1.0)).clamp(0.0, 4.0);
        wind = air * (0.08 + 0.6 * (rel / 1.5).min(1.0)) * vol_ship + air * weather.sample.wind_speed().min(30.0) / 30.0 * 0.15 * vol_weather;
        pitch_wind = 0.6 + 0.35 * rel.min(3.0);
        rumble = (air * f.heat.sqrt() * 0.95 + air * (1.0 - (f.mach - 1.0).abs().min(1.0)) * 0.15 * f.density.min(1.0)) * vol_ship;
        // Moteurs : la poussée demandée ; étouffés dans le vide
        let thrust = surface.pilot.length().clamp(0.0, 1.0);
        engine = thrust * (0.12 + 0.55 * air) * if f.boost { 1.25 } else { 1.0 } * vol_ship;
    } else if surface.active() {
        // À pied ou en cours de pose : un peu de vent selon la météo
        air = air_gain(surface.params().map_or(0.0, |p| crate::approche::air_density(&p, 0.0)));
        wind = air * weather.sample.wind_speed().min(30.0) / 30.0 * 0.25 * vol_weather;
    }
    // Sons du monde (0.14 D5), recalculés dix fois par seconde
    let now = time.elapsed_secs_f64();
    if now - mix.1 > 0.1 {
        mix.1 = now;
        mix.0 = match (surface.terrain(), surface.local_point()) {
            (Some(t), Some(here)) if surface.active() && air > 0.0 => {
                let alt = if f.active { f.alt / t.voxel() } else { 0.0 };
                world_mix(t, here, alt, &weather.sample, under.active(), surface.underground(), 1.0 - surface.daylight(), clock.secs)
            }
            _ => WorldMix::default(),
        };
    }
    let m = mix.0;
    let muffle = if under.active() { 0.12 } else { 1.0 };
    wind *= muffle * (1.0 - 0.9 * surface.underground());
    let w = air * vol_world;
    let target = [
        wind,
        rumble,
        engine,
        m.rain * air * vol_weather * 0.7,
        m.waves * w * 0.6,
        m.river * w * 0.55,
        m.lava * w * 0.8,
        m.geyser * w * 0.6,
        m.bubbles * vol_world * 0.5,
        m.cave * w * 0.5,
        m.insects * w * 0.35,
    ];
    for i in 0..11 {
        smooth[i] += (target[i] - smooth[i]) * (1.0 - (-6.0 * dt).exp());
    }
    for (l, mut sink) in &mut loops {
        let i = Loop::ALL.iter().position(|k| *k == l.0).unwrap_or(0);
        sink.set_volume((smooth[i] * master).clamp(0.0, 1.5));
        if l.0 == Loop::Wind {
            sink.set_speed(pitch_wind.clamp(0.4, 2.0));
        }
        if l.0 == Loop::Engine {
            sink.set_speed(if f.boost { 1.25 } else { 1.0 } * (0.9 + 0.2 * surface.pilot.length().min(1.0)));
        }
    }
}

/// Cris des bêtes (0.14 D5) : toutes les quelques secondes, une bête à portée crie (hurlement pour un
/// quadrupède ou un reptile, pépiement pour un volant ou un insecte, coassement pour le reste), plus fort
/// quand elle est près, étouffé sous l'eau, rien dans le vide.
fn creature_calls(
    mut commands: Commands,
    time: Res<Time>,
    settings: Res<GameSettings>,
    surface: Res<Surface>,
    under: Res<crate::water::Underwater>,
    bank: Res<SoundBank>,
    fauna: Res<crate::fauna::FaunaState>,
    creatures: Query<(&crate::fauna::Creature, &Transform)>,
    mut next: Local<f64>,
) {
    let now = time.elapsed_secs_f64();
    if now < *next {
        return;
    }
    *next = now + 2.0;
    let (Some(t), Some(here)) = (surface.terrain(), surface.local_point()) else { return };
    if !surface.active() || fauna.species.is_empty() {
        return;
    }
    let v = t.voxel();
    let mut best: Option<(f32, usize)> = None;
    for (c, tf) in &creatures {
        let d = (tf.translation - here).length() / v;
        if d < 250.0 && best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, c.species));
        }
    }
    let Some((d, sp)) = best else { return };
    let Some(sp) = fauna.species.get(sp) else { return };
    // Pas à chaque fois : une chance sur deux, ou toujours de tout près
    let h = ((now * 7.3).sin() * 43758.5).fract().abs();
    if h > 0.5 && d > 40.0 {
        *next = now + 1.0 + 4.0 * h as f64;
        return;
    }
    let air = air_gain(surface.params().map_or(0.0, |p| crate::approche::air_density(&p, surface.flight().alt)));
    let kind = match sp.family {
        "quadrupede" | "reptilien" => 0,
        "harpie" | "aile_celeste" | "insectoide" => 1,
        _ => 2,
    };
    let strength = (1.0 - d / 250.0).clamp(0.0, 1.0) * if under.active() { 0.15 } else { 1.0 } * (0.6 + 0.4 * (sp.size_m / 10.0).min(1.0));
    if let Some(hdl) = bank.calls.get(kind) {
        let master = settings.sound_volume.clamp(0.0, 1.0) * settings.sound_world.clamp(0.0, 1.0);
        let mut player = commands.spawn((AudioPlayer::new(hdl.clone()), PlaybackSettings::DESPAWN.with_volume(Volume::new(strength * master * air * 0.8))));
        // Une grosse bête crie plus grave
        player.insert(PlaybackSettings::DESPAWN.with_volume(Volume::new(strength * master * air * 0.8)).with_speed((1.3 - (sp.size_m / 12.0).min(0.7)).clamp(0.5, 1.4)));
    }
    *next = now + 3.0 + 9.0 * h as f64;
}

// ─────────────────────────────────────────────────────────────────────────
//  Sons ponctuels : bang, tonnerre
// ─────────────────────────────────────────────────────────────────────────

fn play(commands: &mut Commands, handle: Handle<AudioSource>, volume: f32) {
    if volume > 0.002 {
        commands.spawn((AudioPlayer::new(handle), PlaybackSettings::DESPAWN.with_volume(Volume::new(volume))));
    }
}

#[allow(clippy::too_many_arguments)]
fn one_shots(
    mut commands: Commands,
    time: Res<Time>,
    settings: Res<GameSettings>,
    surface: Res<Surface>,
    weather: Res<crate::weather::WeatherNow>,
    bank: Res<SoundBank>,
    mut booms: EventReader<Boom>,
    mut pending: ResMut<Pending>,
    mut last_flash: Local<f64>,
) {
    let now = time.elapsed_secs_f64();
    let master = settings.sound_volume.clamp(0.0, 1.0);
    let f = *surface.flight();
    for b in booms.read() {
        // Entendu à bord : le vaisseau est dans l'onde, sans retard (le bang des autres arrive plus tard)
        play(&mut commands, bank.boom.clone(), b.strength * master * air_gain(f.density).max(0.35));
    }
    // Éclairs : le tonnerre arrive après la lumière (la distance de l'impact / vitesse du son)
    if let Some((dir, t)) = weather.flash {
        if t > *last_flash {
            *last_flash = t;
            let distance = surface.local_point().map_or(2000.0, |p| p.normalize_or(Vec3::Y).angle_between(dir) * surface.params().map_or(10_000.0, |p| p.radius));
            let voxel = surface.params().map_or(1.0, |_| surface.terrain().map_or(1.0, |t| t.voxel()));
            let delay = (distance / voxel / f.sound.max(340.0)).clamp(0.1, 6.0) as f64;
            let strength = (1.0 - (distance / voxel / 6000.0).min(0.85)).clamp(0.2, 1.0);
            pending.0.push((now + delay, strength, 0));
        }
    }
    pending.0.retain(|&(at, strength, kind)| {
        if now < at {
            return true;
        }
        let air = air_gain(surface.params().map_or(0.0, |p| crate::approche::air_density(&p, f.alt)));
        let h = if kind == 0 { bank.thunder.clone() } else { bank.boom.clone() };
        play(&mut commands, h, strength * master * air);
        false
    });
}

/// Le bang d'un autre joueur, avec le retard de la distance (vitesse du son).
#[derive(Event, Clone, Copy, Debug)]
pub struct RemoteBoom {
    /// Distance (voxels) du joueur qui franchit le mur du son.
    pub distance: f32,
    pub strength: f32,
}

fn remote_booms(time: Res<Time>, surface: Res<Surface>, mut events: EventReader<RemoteBoom>, mut pending: ResMut<Pending>) {
    let now = time.elapsed_secs_f64();
    let sound = surface.params().map_or(340.0, |p| crate::approche::sound_speed(&p));
    for e in events.read() {
        let delay = (e.distance / sound.max(100.0)).clamp(0.0, 30.0) as f64;
        let strength = (e.strength * (1.0 - (e.distance / 40_000.0).min(0.9))).clamp(0.05, 1.0);
        pending.0.push((now + delay, strength, 1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_files_are_well_formed() {
        let b = wav(&[0.0, 0.5, -0.5, 1.0]);
        assert_eq!(&b[0..4], b"RIFF");
        assert_eq!(&b[8..16], b"WAVEfmt ");
        assert_eq!(u32::from_le_bytes(b[40..44].try_into().unwrap()), 8);
        assert_eq!(b.len(), 44 + 8);
        assert_eq!(u32::from_le_bytes(b[4..8].try_into().unwrap()) as usize, b.len() - 8);
    }

    #[test]
    fn loops_close_without_a_jump() {
        for s in [wind_sound(), rumble_sound(), rain_sound(), waves_sound(), river_sound(), lava_sound(), geyser_sound(), bubbles_sound(), cave_sound(), insects_sound()] {
            let max_step = s.windows(2).map(|w| (w[1] - w[0]).abs()).fold(0.0f32, f32::max);
            let seam = (s[0] - s[s.len() - 1]).abs();
            assert!(seam <= max_step * 2.0 + 0.02, "saut a la couture : {seam} (pas max {max_step})");
        }
    }

    #[test]
    fn every_sound_has_a_signal_and_stays_in_range() {
        let calls = call_sounds();
        for (name, s) in [("vent", wind_sound()), ("grondement", rumble_sound()), ("moteurs", engine_sound()), ("bang", boom_sound()), ("tonnerre", thunder_sound()), ("pluie", rain_sound()), ("vagues", waves_sound()), ("riviere", river_sound()), ("lave", lava_sound()), ("geyser", geyser_sound()), ("bulles", bubbles_sound()), ("grotte", cave_sound()), ("insectes", insects_sound()), ("hurlement", calls[0].clone()), ("pepiement", calls[1].clone()), ("coassement", calls[2].clone())] {
            let peak = s.iter().fold(0.0f32, |m, x| m.max(x.abs()));
            assert!(peak > 0.3 && peak <= 1.0, "{name} : crete {peak}");
        }
    }

    /// 0.14 D5 : le mélange du monde suit le lieu : au bord de la mer des vagues, loin d'elle rien ; sous l'eau les
    /// bulles et le reste étouffé ; sous terre la grotte et le dehors éteint ; la pluie seulement s'il pleut.
    #[test]
    fn the_world_mix_follows_the_place() {
        let t = crate::terrain::Terrain::new(crate::terrain::tests::earth_like());
        let v = t.voxel();
        let dry = crate::weather::Sample::default();
        // Un point en mer et un point loin de toute mer
        let mut sea = None;
        let mut inland = None;
        for k in 0..400 {
            let a = k as f32 * 0.61;
            let dir = Vec3::new(a.cos() * 0.7, (k as f32 * 0.037).sin() * 0.5, a.sin() * 0.7).normalize();
            if t.sea_surface(dir).is_some() {
                sea.get_or_insert(dir);
            } else if sea.is_none() || inland.is_none() {
                let m = world_mix(&t, dir * t.params.radius, 0.0, &dry, false, 0.0, 0.0, 0.0);
                if m.waves == 0.0 {
                    inland.get_or_insert(dir);
                }
            }
            if sea.is_some() && inland.is_some() {
                break;
            }
        }
        let (sea, inland) = (sea.expect("un point en mer"), inland.expect("un point loin de la mer"));
        let shore = world_mix(&t, sea * t.params.radius, 0.0, &dry, false, 0.0, 0.0, 0.0);
        assert!(shore.waves > 0.3, "{shore:?}");
        assert_eq!(shore.rain, 0.0);
        assert_eq!(shore.bubbles, 0.0);
        let diving = world_mix(&t, sea * t.params.radius, 0.0, &dry, true, 0.0, 0.0, 0.0);
        assert!(diving.bubbles > 0.9 && diving.waves < shore.waves * 0.2);
        let deep = world_mix(&t, inland * t.params.radius, 0.0, &dry, false, 1.0, 0.5, 0.0);
        assert!(deep.cave > 0.9 && deep.insects < 0.1, "{deep:?}");
        let rainy = crate::weather::Sample { precip: 0.8, kind: crate::weather::Precip::Rain, ..dry };
        assert!(world_mix(&t, inland * t.params.radius, 0.0, &rainy, false, 0.0, 0.0, 0.0).rain > 0.7);
        // En altitude : plus de vagues ni de rivière, la pluie reste
        let high = world_mix(&t, sea * t.params.radius, 400.0 * v / v, &rainy, false, 0.0, 0.0, 0.0);
        assert_eq!(high.waves, 0.0);
        assert!(high.rain > 0.7);
    }

    /// Silence dans le vide : sans air, rien ne passe ; l'air dense laisse tout passer.
    #[test]
    fn there_is_no_sound_in_a_vacuum() {
        assert_eq!(air_gain(0.0), 0.0);
        assert!(air_gain(0.01) < 0.2);
        assert!(air_gain(1.0) > 0.9);
        assert!(air_gain(0.1) < air_gain(0.5));
    }
}
