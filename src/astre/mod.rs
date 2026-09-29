pub mod planete;

#[allow(non_snake_case)]
pub mod Remnant_stellaire;

pub mod etoile;

use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::PathBuf;

#[derive(Component)]
pub struct AstreLodRoot {
    pub cull_dist: f32,
    pub radius: f32,
    pub streamable: bool,
    pub label: &'static str,
}

#[derive(Component)]
pub struct AstreUnloaded;

#[derive(Component)]
pub struct AstrePendingUnload(pub f32);

#[derive(Component)]
pub struct AstrePendingReload;

#[derive(Event)]
pub struct ReloadAstre(pub Entity);

fn log_dir() -> PathBuf {
    crate::settings::data_dir().join("logs")
}
const MAX_RELOADS_PER_FRAME: usize = 2;
const SNAPSHOT_INTERVAL: f32 = 1.0;

#[derive(Resource)]
pub struct ProfilingLog {
    pub active: bool,
    pub counter: u32,
    pub hidden_total: u32,
    pub reload_total: u32,
    pub frame_count: u64,
    pub fps_min: f64,
    pub fps_max: f64,
    pub fps_sum: f64,
    pub fps_samples: u32,
    snapshot_timer: f32,
    writer: Option<BufWriter<File>>,
    file_path: Option<PathBuf>,
    start_time: Option<std::time::Instant>,
}

impl Default for ProfilingLog {
    fn default() -> Self {
        Self {
            active: false,
            counter: 0,
            hidden_total: 0,
            reload_total: 0,
            frame_count: 0,
            fps_min: f64::MAX,
            fps_max: 0.0,
            fps_sum: 0.0,
            fps_samples: 0,
            snapshot_timer: 0.0,
            writer: None,
            file_path: None,
            start_time: None,
        }
    }
}

impl ProfilingLog {
    fn open_file(&mut self) {
        let dir = log_dir();
        fs::create_dir_all(&dir).ok();
        let now = chrono::Local::now();
        let path = dir.join(format!("{}.log", now.format("%Y-%m-%d_%H-%M-%S")));
        match File::create(&path) {
            Ok(f) => {
                let mut w = BufWriter::new(f);
                let _ = writeln!(w, "=== PROFILING START === {}", now.format("%Y-%m-%d %H:%M:%S"));
                self.writer = Some(w);
                self.file_path = Some(path);
                self.start_time = Some(std::time::Instant::now());
            }
            Err(e) => {
                warn!("Cannot create profiling log: {e}");
            }
        }
    }

    fn close_file(&mut self) {
        let elapsed = self.start_time.map(|t| t.elapsed().as_secs_f32()).unwrap_or(0.0);
        let fps_avg = if self.fps_samples > 0 { self.fps_sum / self.fps_samples as f64 } else { 0.0 };
        if let Some(ref mut w) = self.writer {
            let _ = writeln!(w);
            let _ = writeln!(w, "========== SUMMARY ==========");
            let _ = writeln!(w, "duration:    {:.1}s", elapsed);
            let _ = writeln!(w, "frames:      {}", self.frame_count);
            let _ = writeln!(w, "events:      {} total", self.counter);
            let _ = writeln!(w, "  hidden:    {}", self.hidden_total);
            let _ = writeln!(w, "  reloaded:  {}", self.reload_total);
            let _ = writeln!(w, "FPS avg:     {:.1}", fps_avg);
            let _ = writeln!(w, "FPS min:     {:.1}", if self.fps_min == f64::MAX { 0.0 } else { self.fps_min });
            let _ = writeln!(w, "FPS max:     {:.1}", self.fps_max);
            let _ = writeln!(w, "=== PROFILING STOP ===");
            let _ = w.flush();
        }
        if let Some(ref p) = self.file_path {
            info!("Profiling log saved: {}", p.display());
        }
        self.writer = None;
        self.file_path = None;
        self.start_time = None;
    }

    fn elapsed_secs(&self) -> f32 {
        self.start_time.map(|t| t.elapsed().as_secs_f32()).unwrap_or(0.0)
    }

    fn log(&mut self, msg: &str) {
        if let Some(ref mut w) = self.writer {
            let _ = writeln!(w, "{}", msg);
            let _ = w.flush();
        }
    }
}

pub fn toggle_profiling(
    keys: Res<ButtonInput<KeyCode>>,
    mut profiling: ResMut<ProfilingLog>,
    unloaded_q: Query<(&AstreLodRoot,), With<AstreUnloaded>>,
    all_q: Query<&AstreLodRoot>,
) {
    if keys.just_pressed(KeyCode::F12) {
        profiling.active = !profiling.active;
        if profiling.active {
            profiling.counter = 0;
            profiling.hidden_total = 0;
            profiling.reload_total = 0;
            profiling.frame_count = 0;
            profiling.fps_min = f64::MAX;
            profiling.fps_max = 0.0;
            profiling.fps_sum = 0.0;
            profiling.fps_samples = 0;
            profiling.snapshot_timer = 0.0;
            profiling.open_file();

            let total = all_q.iter().count();
            let streamable = all_q.iter().filter(|l| l.streamable).count();
            let hidden = unloaded_q.iter().count();
            let mut type_counts: std::collections::HashMap<&str, u32> = std::collections::HashMap::new();
            for lod in &all_q {
                *type_counts.entry(lod.label).or_insert(0) += 1;
            }
            let mut types_str = String::new();
            let mut entries: Vec<_> = type_counts.into_iter().collect();
            entries.sort_by_key(|(k, _)| *k);
            for (label, count) in &entries {
                if !types_str.is_empty() { types_str.push_str(", "); }
                types_str.push_str(&format!("{}:{}", label, count));
            }

            profiling.log(&format!("astres: {} total ({} streamable) | {} hidden", total, streamable, hidden));
            profiling.log(&format!("types:  {}", types_str));
            profiling.log("");
            info!("=== PROFILING START === ({} astres, {} streamable)", total, streamable);
        } else {
            profiling.close_file();
            info!("=== PROFILING STOP === ({} events: {} hidden, {} reloaded)", profiling.counter, profiling.hidden_total, profiling.reload_total);
        }
    }
}

pub fn profiling_snapshot(
    mut profiling: ResMut<ProfilingLog>,
    time: Res<Time>,
    diagnostics: Res<DiagnosticsStore>,
    cam_q: Query<&Transform, With<Camera3d>>,
    all_q: Query<&AstreLodRoot>,
    unloaded_q: Query<Entity, With<AstreUnloaded>>,
) {
    if !profiling.active { return; }

    let fps = diagnostics.get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|d| d.smoothed())
        .unwrap_or(0.0);
    let frame_time = diagnostics.get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
        .and_then(|d| d.smoothed())
        .unwrap_or(0.0);

    profiling.frame_count += 1;
    if fps > 0.0 {
        if fps < profiling.fps_min { profiling.fps_min = fps; }
        if fps > profiling.fps_max { profiling.fps_max = fps; }
        profiling.fps_sum += fps;
        profiling.fps_samples += 1;
    }

    profiling.snapshot_timer += time.delta_secs();
    if profiling.snapshot_timer < SNAPSHOT_INTERVAL { return; }
    profiling.snapshot_timer = 0.0;

    let elapsed = profiling.elapsed_secs();
    let total = all_q.iter().count();
    let hidden = unloaded_q.iter().count();
    let visible = total - hidden;

    let cam_str = if let Ok(cam_tf) = cam_q.get_single() {
        let p = cam_tf.translation;
        let fwd = cam_tf.forward().as_vec3();
        format!("cam=({:.0},{:.0},{:.0}) fwd=({:.2},{:.2},{:.2})", p.x, p.y, p.z, fwd.x, fwd.y, fwd.z)
    } else {
        "cam=?".to_string()
    };

    let msg = format!(
        "[{:.1}s] FPS:{:.0} ft:{:.1}ms | visible:{}/{} hidden:{} | {}",
        elapsed, fps, frame_time, visible, total, hidden, cam_str
    );
    profiling.log(&msg);
}

const UNLOAD_DELAY: f32 = 1.0;

pub fn process_pending_reloads(
    mut commands: Commands,
    pending: Query<(Entity, &AstreLodRoot), With<AstrePendingReload>>,
    mut reload_events: EventWriter<ReloadAstre>,
    mut profiling: ResMut<ProfilingLog>,
) {
    for (i, (entity, lod)) in pending.iter().enumerate() {
        if i >= MAX_RELOADS_PER_FRAME { break; }
        commands.entity(entity).remove::<AstrePendingReload>();
        commands.entity(entity).remove::<AstreUnloaded>();
        reload_events.send(ReloadAstre(entity));
        if profiling.active {
            profiling.counter += 1;
            profiling.reload_total += 1;
            let t = profiling.elapsed_secs();
            let c = profiling.counter;
            let l = lod.label;
            profiling.log(&format!("  log {} | {:.2}s | RELOAD (queued) {}", c, t, l));
        }
    }
}

pub fn astre_lod_cull(
    mut commands: Commands,
    cam_q: Query<&Transform, With<Camera3d>>,
    mut root_q: Query<(Entity, &GlobalTransform, &AstreLodRoot, &mut Visibility, Option<&AstreUnloaded>, Option<&AstrePendingUnload>, Option<&AstrePendingReload>)>,
    sys_q: Query<&crate::planet::SystemIdx>,
    mut profiling: ResMut<ProfilingLog>,
    diagnostics: Res<DiagnosticsStore>,
    settings: Res<crate::settings::GameSettings>,
    spawned: Res<crate::planet::SpawnedSystems>,
    spatial: Res<crate::settings::SystemSpatialIndex>,
    time: Res<Time>,
) {
    let Ok(cam_tf) = cam_q.get_single() else { return };
    let cp = cam_tf.translation;
    let cam_fwd = cam_tf.forward().as_vec3();
    let dt = time.delta_secs();

    // Système du joueur : celui qui est chargé, sinon le plus proche via l'index spatial
    // (évite de parcourir les ~12 500 systèmes à chaque image).
    let player_sys = spawned.0.iter().next().copied()
        .or_else(|| spatial.nearest(cp, &settings));

    let (fps, frame_time) = if profiling.active {
        let f = diagnostics.get(&FrameTimeDiagnosticsPlugin::FPS)
            .and_then(|d| d.smoothed()).unwrap_or(0.0);
        let ft = diagnostics.get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
            .and_then(|d| d.smoothed()).unwrap_or(0.0);
        (f, ft)
    } else {
        (0.0, 0.0)
    };

    for (entity, gt, lod, mut vis, unloaded, pending, pending_reload) in &mut root_q {
        // skip entities already queued for reload
        if pending_reload.is_some() { continue; }

        let pos = gt.translation();
        let d = cp.distance(pos);

        // Astres d'un système : ceux du système du joueur sont toujours affichés,
        // ceux des autres systèmes sont masqués.
        if let Ok(si) = sys_q.get(entity) {
            if let Some(ps) = player_sys {
                if si.0 != ps {
                    if let Some(mut ec) = commands.get_entity(entity) {
                        ec.remove::<AstrePendingUnload>();
                        if unloaded.is_none() {
                            ec.try_insert(AstreUnloaded);
                            if profiling.active {
                                profiling.counter += 1;
                                profiling.hidden_total += 1;
                                let t = profiling.elapsed_secs();
                                let msg = format!(
                                    "  log {} | {:.2}s | FPS:{:.0} ft:{:.1}ms | HIDDEN {} | sys={} (player={}) pos=({:.0},{:.0},{:.0})",
                                    profiling.counter, t, fps, frame_time, lod.label, si.0, ps, pos.x, pos.y, pos.z
                                );
                                profiling.log(&msg);
                            }
                        }
                    }
                    if *vis != Visibility::Hidden { *vis = Visibility::Hidden; }
                    continue;
                }
                if let Some(mut ec) = commands.get_entity(entity) {
                    if pending.is_some() { ec.remove::<AstrePendingUnload>(); }
                    if unloaded.is_some() { ec.try_insert(AstrePendingReload); }
                }
                if *vis != Visibility::Inherited { *vis = Visibility::Inherited; }
                continue;
            }
        }

        if d < lod.radius * 5.0 {
            if let Some(mut ec) = commands.get_entity(entity) {
                if pending.is_some() { ec.remove::<AstrePendingUnload>(); }
                if unloaded.is_some() { ec.try_insert(AstrePendingReload); }
            }
            if *vis != Visibility::Inherited { *vis = Visibility::Inherited; }
            continue;
        }

        let to_body = (pos - cp).normalize_or_zero();
        let dot = cam_fwd.dot(to_body);

        let angular_margin = (lod.radius / d).min(0.5);
        let hide_dot = (0.3 - angular_margin).max(-0.1);
        let reload_dot = (hide_dot + 0.2).min(0.7);

        if dot < hide_dot {
            if unloaded.is_none() {
                if let Some(p) = pending {
                    let elapsed = p.0 + dt;
                    if elapsed >= UNLOAD_DELAY {
                        if let Some(mut ec) = commands.get_entity(entity) {
                            ec.remove::<AstrePendingUnload>();
                            ec.try_insert(AstreUnloaded);
                        }
                        if *vis != Visibility::Hidden { *vis = Visibility::Hidden; }
                        if profiling.active {
                            profiling.counter += 1;
                            profiling.hidden_total += 1;
                            let t = profiling.elapsed_secs();
                            let msg = format!(
                                "  log {} | {:.2}s | FPS:{:.0} ft:{:.1}ms | HIDDEN {} | dot={:.2} dist={:.0} pos=({:.0},{:.0},{:.0}) cam=({:.0},{:.0},{:.0})",
                                profiling.counter, t, fps, frame_time, lod.label, dot, d, pos.x, pos.y, pos.z, cp.x, cp.y, cp.z
                            );
                            profiling.log(&msg);
                        }
                    } else if let Some(mut ec) = commands.get_entity(entity) {
                        ec.try_insert(AstrePendingUnload(elapsed));
                    }
                } else if let Some(mut ec) = commands.get_entity(entity) {
                    ec.try_insert(AstrePendingUnload(0.0));
                }
            }
        } else if dot > reload_dot {
            if let Some(mut ec) = commands.get_entity(entity) {
                if pending.is_some() { ec.remove::<AstrePendingUnload>(); }
                if unloaded.is_some() { ec.try_insert(AstrePendingReload); }
            }
            if *vis != Visibility::Inherited { *vis = Visibility::Inherited; }
        } else {
            if pending.is_some() {
                if let Some(mut ec) = commands.get_entity(entity) {
                    ec.remove::<AstrePendingUnload>();
                }
            }
        }
    }
}
