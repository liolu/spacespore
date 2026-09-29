#![windows_subsystem = "windows"]

use eframe::egui;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use spacespore_common::{exe_name, Channel, VersionInfo, CHANNEL, GAME_BIN, LAUNCHER_BIN};

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("SpaceSpore Launcher")
            .with_inner_size([460.0, 290.0])
            .with_resizable(false),
        centered: true,
        ..Default::default()
    };
    eframe::run_native(
        "SpaceSpore Launcher",
        options,
        Box::new(|cc| Ok(Box::new(LauncherApp::new(cc.egui_ctx.clone())))),
    )
}

// ─────────────────────────────────────────────────────────────────────────
//  Canal choisi (stable / instable), mémorisé dans launcher.json
// ─────────────────────────────────────────────────────────────────────────

#[derive(serde::Serialize, serde::Deserialize)]
struct LauncherConfig {
    channel: String,
}

fn config_path() -> PathBuf {
    spacespore_common::exe_dir().join("launcher.json")
}

fn load_channel() -> Channel {
    let saved = fs::read_to_string(config_path())
        .ok()
        .and_then(|s| serde_json::from_str::<LauncherConfig>(&s).ok())
        .map(|c| Channel::from_name(&c.channel));
    match saved {
        Some(c @ (Channel::Stable | Channel::Unstable)) => c,
        // Par défaut : le canal de la version installée (stable si compilation locale)
        _ if CHANNEL == Channel::Unstable => Channel::Unstable,
        _ => Channel::Stable,
    }
}

fn save_channel(channel: Channel) {
    let cfg = LauncherConfig { channel: channel.name().into() };
    if let Ok(json) = serde_json::to_string_pretty(&cfg) {
        let _ = fs::write(config_path(), json);
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  État partagé entre la fenêtre et le thread de mise à jour
// ─────────────────────────────────────────────────────────────────────────

#[derive(Clone)]
enum Stage {
    Checking,
    /// Une autre version est disponible sur le canal choisi.
    Available(VersionInfo),
    /// Rien à installer : le joueur peut lancer le jeu.
    ReadyToPlay(String),
    Downloading,
    Extracting,
    /// Mise à jour installée : on lance le jeu.
    Installed(String),
    Failed(String),
}

struct Shared {
    stage: Mutex<Stage>,
    downloaded: AtomicU64,
    total: AtomicU64,
    /// Incrémenté à chaque vérification : une vérification périmée
    /// (changement de canal entre-temps) est ignorée.
    check_id: AtomicU64,
}

impl Shared {
    fn set(&self, stage: Stage) {
        *self.stage.lock().unwrap() = stage;
    }
}

struct LauncherApp {
    shared: Arc<Shared>,
    ctx: egui::Context,
    install_dir: PathBuf,
    game_exe: PathBuf,
    channel: Channel,
    launched: bool,
    launch_error: Option<String>,
}

impl LauncherApp {
    fn new(ctx: egui::Context) -> Self {
        let install_dir = spacespore_common::exe_dir();
        let game_exe = install_dir.join(exe_name(GAME_BIN));
        let shared = Arc::new(Shared {
            stage: Mutex::new(Stage::Checking),
            downloaded: AtomicU64::new(0),
            total: AtomicU64::new(0),
            check_id: AtomicU64::new(0),
        });
        let app = Self {
            shared,
            ctx,
            install_dir,
            game_exe,
            channel: load_channel(),
            launched: false,
            launch_error: None,
        };
        app.start_check();
        app
    }

    fn start_check(&self) {
        let shared = self.shared.clone();
        let ctx = self.ctx.clone();
        let channel = self.channel;
        let id = shared.check_id.fetch_add(1, Ordering::SeqCst) + 1;
        shared.set(Stage::Checking);
        std::thread::spawn(move || {
            let stage = match spacespore_common::fetch_channel(channel, 5) {
                Ok(info) if spacespore_common::should_install(channel, &info) => Stage::Available(info),
                Ok(_) => Stage::ReadyToPlay("Le jeu est a jour".into()),
                Err(e) => Stage::ReadyToPlay(format!("Hors-ligne ou aucune version disponible ({})", e)),
            };
            if shared.check_id.load(Ordering::SeqCst) == id {
                shared.set(stage);
            }
            ctx.request_repaint();
        });
    }

    fn start_update(&self, info: VersionInfo) {
        let shared = self.shared.clone();
        let ctx = self.ctx.clone();
        let dir = self.install_dir.clone();
        shared.downloaded.store(0, Ordering::Relaxed);
        shared.total.store(0, Ordering::Relaxed);
        shared.set(Stage::Downloading);
        std::thread::spawn(move || {
            let result = download_and_apply(&info, &dir, &shared, &ctx);
            shared.set(match result {
                Ok(()) => Stage::Installed(format!("{} installee", info.label())),
                Err(e) => Stage::Failed(e.to_string()),
            });
            ctx.request_repaint();
        });
    }

    /// Lance le jeu et ferme le launcher.
    fn launch_game(&mut self, ctx: &egui::Context) {
        if self.launched {
            return;
        }
        if !self.game_exe.exists() {
            self.launch_error = Some(format!("{} introuvable", self.game_exe.display()));
            return;
        }
        match Command::new(&self.game_exe).current_dir(&self.install_dir).spawn() {
            Ok(_) => {
                self.launched = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Err(e) => self.launch_error = Some(format!("Impossible de lancer le jeu: {}", e)),
        }
    }
}

impl eframe::App for LauncherApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let ctx = ctx.clone();
        let stage = self.shared.stage.lock().unwrap().clone();
        let busy = matches!(stage, Stage::Downloading | Stage::Extracting | Stage::Installed(_));

        egui::CentralPanel::default().show(&ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(8.0);
                ui.heading("SpaceSpore");
                ui.label(format!("Installe : {}", spacespore_common::installed_label()));
                ui.add_space(8.0);

                // ── Choix du canal ──
                ui.add_enabled_ui(!busy, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Version :");
                        let before = self.channel;
                        ui.radio_value(&mut self.channel, Channel::Stable, "Stable")
                            .on_hover_text("Versions publiees, testees");
                        ui.radio_value(&mut self.channel, Channel::Unstable, "Instable")
                            .on_hover_text("Derniere version en cours de developpement (peut contenir des bugs)");
                        if self.channel != before {
                            save_channel(self.channel);
                            self.start_check();
                        }
                    });
                });
                ui.separator();
                ui.add_space(4.0);

                if let Some(err) = self.launch_error.clone() {
                    ui.colored_label(egui::Color32::LIGHT_RED, err);
                    ui.add_space(8.0);
                    if ui.button("Quitter").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    return;
                }

                match stage {
                    Stage::Checking => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label("Verification des mises a jour...");
                        });
                    }
                    Stage::Available(info) => {
                        let switching = CHANNEL != self.channel;
                        let title = if switching {
                            format!("Disponible : {}", info.label())
                        } else {
                            format!("Nouvelle version disponible : {}", info.label())
                        };
                        ui.label(egui::RichText::new(title).strong());
                        ui.label(&info.release_notes);
                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            let action = if switching { "Installer" } else { "Mettre a jour" };
                            if ui.button(action).clicked() {
                                self.start_update(info.clone());
                            }
                            if ui.button("Jouer sans changer").clicked() {
                                self.launch_game(&ctx);
                            }
                        });
                    }
                    Stage::ReadyToPlay(msg) => {
                        ui.label(msg);
                        ui.add_space(10.0);
                        if ui.add_sized([140.0, 34.0], egui::Button::new("Jouer")).clicked() {
                            self.launch_game(&ctx);
                        }
                    }
                    Stage::Downloading => {
                        let done = self.shared.downloaded.load(Ordering::Relaxed);
                        let total = self.shared.total.load(Ordering::Relaxed);
                        ui.label("Telechargement...");
                        let (frac, text) = if total > 0 {
                            (
                                done as f32 / total as f32,
                                format!("{:.1} MB / {:.1} MB", done as f64 / 1_048_576.0, total as f64 / 1_048_576.0),
                            )
                        } else {
                            (0.0, format!("{:.1} MB", done as f64 / 1_048_576.0))
                        };
                        ui.add(egui::ProgressBar::new(frac).text(text).animate(true));
                    }
                    Stage::Extracting => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label("Installation...");
                        });
                    }
                    Stage::Installed(msg) => {
                        ui.label(msg);
                        ui.label("Lancement de SpaceSpore...");
                        self.launch_game(&ctx);
                    }
                    Stage::Failed(err) => {
                        ui.colored_label(egui::Color32::LIGHT_RED, format!("Erreur : {}", err));
                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            if ui.button("Jouer quand meme").clicked() {
                                self.launch_game(&ctx);
                            }
                            if ui.button("Quitter").clicked() {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                        });
                    }
                }
            });
        });
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  Téléchargement / installation
// ─────────────────────────────────────────────────────────────────────────

fn download_and_apply(
    info: &VersionInfo,
    install_dir: &Path,
    shared: &Shared,
    ctx: &egui::Context,
) -> Result<(), Box<dyn std::error::Error>> {
    let url = info
        .platform_download_url()
        .ok_or_else(|| format!("Aucune version disponible pour {}", spacespore_common::PLATFORM))?;

    let temp_dir = std::env::temp_dir().join("spacespore-update");
    fs::create_dir_all(&temp_dir)?;
    let zip_path = temp_dir.join("update.zip");

    let agent = spacespore_common::make_agent(120);
    let response = agent.get(url).call()?;

    let total_size = response
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);
    shared.total.store(total_size, Ordering::Relaxed);

    let mut body = response.into_body();
    let mut file = fs::File::create(&zip_path)?;
    let mut downloaded: u64 = 0;
    let mut buf = [0u8; 8192];

    loop {
        let n = body.as_reader().read(&mut buf)?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])?;
        downloaded += n as u64;
        shared.downloaded.store(downloaded, Ordering::Relaxed);
        ctx.request_repaint();
    }
    drop(file);

    if total_size > 0 && downloaded != total_size {
        return Err(format!("Telechargement incomplet: {} / {} octets", downloaded, total_size).into());
    }

    shared.set(Stage::Extracting);
    ctx.request_repaint();

    let zip_file = fs::File::open(&zip_path)?;
    let mut archive = zip::ZipArchive::new(zip_file)?;
    let launcher_name = exe_name(LAUNCHER_BIN);

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let raw_name = entry.name().to_string();

        let relative = raw_name.split('/').skip(1).collect::<Vec<_>>().join("/");
        if relative.is_empty() {
            continue;
        }

        // Ne pas écraser le launcher lui-même pendant qu'il tourne
        if relative == launcher_name {
            let old_path = install_dir.join(format!("{}.old", launcher_name));
            let _ = fs::remove_file(&old_path);
            let new_path = install_dir.join(&relative);
            if new_path.exists() {
                let _ = fs::rename(&new_path, &old_path);
            }
        }

        let out_path = install_dir.join(&relative);

        if entry.is_dir() {
            fs::create_dir_all(&out_path)?;
        } else {
            let mode = entry.unix_mode();
            spacespore_common::write_extracted_file(&out_path, &mut entry, mode)
                .map_err(|e| io::Error::new(e.kind(), format!("{}: {}", relative, e)))?;
        }
    }
    spacespore_common::clear_macos_quarantine(install_dir);

    let _ = fs::remove_file(&zip_path);
    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}
