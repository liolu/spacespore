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
            .add_systems(Update, (update_loops, one_shots, remote_booms).chain().after(crate::surface::SurfaceControl));
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
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Loop {
    Wind,
    Rumble,
    Engine,
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

fn source(samples: &[f32]) -> AudioSource {
    AudioSource { bytes: Arc::from(wav(samples).into_boxed_slice()) }
}

fn setup_sounds(mut commands: Commands, mut sources: ResMut<Assets<AudioSource>>, mut bank: ResMut<SoundBank>) {
    bank.wind = sources.add(source(&wind_sound()));
    bank.rumble = sources.add(source(&rumble_sound()));
    bank.engine = sources.add(source(&engine_sound()));
    bank.boom = sources.add(source(&boom_sound()));
    bank.thunder = sources.add(source(&thunder_sound()));
    for (kind, handle) in [(Loop::Wind, bank.wind.clone()), (Loop::Rumble, bank.rumble.clone()), (Loop::Engine, bank.engine.clone())] {
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

fn update_loops(time: Res<Time>, settings: Res<GameSettings>, surface: Res<Surface>, weather: Res<crate::weather::WeatherNow>, mut loops: Query<(&SoundLoop, &mut AudioSink)>, mut smooth: Local<[f32; 3]>) {
    let dt = time.delta_secs().min(0.1);
    let master = settings.sound_volume.clamp(0.0, 1.0);
    let f = *surface.flight();
    let (mut wind, mut rumble, mut engine, mut pitch_wind) = (0.0f32, 0.0f32, 0.0f32, 1.0f32);
    if surface.active() && f.active {
        let air = air_gain(f.density);
        // Sifflement : monte avec la vitesse (par rapport à celle du son), plus fort dans un air dense
        let rel = (f.speed_vox / f.sound.max(1.0)).clamp(0.0, 4.0);
        wind = air * (0.08 + 0.6 * (rel / 1.5).min(1.0)) + air * weather.sample.wind_speed().min(30.0) / 30.0 * 0.15;
        pitch_wind = 0.6 + 0.35 * rel.min(3.0);
        rumble = air * f.heat.sqrt() * 0.95 + air * (1.0 - (f.mach - 1.0).abs().min(1.0)) * 0.15 * f.density.min(1.0);
        // Moteurs : la poussée demandée ; étouffés dans le vide
        let thrust = surface.pilot.length().clamp(0.0, 1.0);
        engine = thrust * (0.12 + 0.55 * air) * if f.boost { 1.25 } else { 1.0 };
    } else if surface.active() {
        // À pied ou en cours de pose : un peu de vent selon la météo
        let air = air_gain(surface.params().map_or(0.0, |p| crate::approche::air_density(&p, 0.0)));
        wind = air * weather.sample.wind_speed().min(30.0) / 30.0 * 0.25;
    }
    let target = [wind, rumble, engine];
    for i in 0..3 {
        smooth[i] += (target[i] - smooth[i]) * (1.0 - (-6.0 * dt).exp());
    }
    for (l, mut sink) in &mut loops {
        let v = match l.0 {
            Loop::Wind => smooth[0],
            Loop::Rumble => smooth[1],
            Loop::Engine => smooth[2],
        };
        sink.set_volume((v * master).clamp(0.0, 1.5));
        if l.0 == Loop::Wind {
            sink.set_speed(pitch_wind.clamp(0.4, 2.0));
        }
        if l.0 == Loop::Engine {
            sink.set_speed(if f.boost { 1.25 } else { 1.0 } * (0.9 + 0.2 * surface.pilot.length().min(1.0)));
        }
    }
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
        for s in [wind_sound(), rumble_sound()] {
            let max_step = s.windows(2).map(|w| (w[1] - w[0]).abs()).fold(0.0f32, f32::max);
            let seam = (s[0] - s[s.len() - 1]).abs();
            assert!(seam <= max_step * 2.0 + 0.02, "saut a la couture : {seam} (pas max {max_step})");
        }
    }

    #[test]
    fn every_sound_has_a_signal_and_stays_in_range() {
        for (name, s) in [("vent", wind_sound()), ("grondement", rumble_sound()), ("moteurs", engine_sound()), ("bang", boom_sound()), ("tonnerre", thunder_sound())] {
            let peak = s.iter().fold(0.0f32, |m, x| m.max(x.abs()));
            assert!(peak > 0.3 && peak <= 1.0, "{name} : crete {peak}");
        }
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
