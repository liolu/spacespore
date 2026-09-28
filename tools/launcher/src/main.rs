#![windows_subsystem = "windows"]

use eframe::egui;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use spacespore_common::{exe_name, VersionInfo, GAME_BIN, LAUNCHER_BIN, VERSION};

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("SpaceSpore Launcher")
            .with_inner_size([440.0, 240.0])
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
//  État partagé entre la fenêtre et le thread de mise à jour
// ─────────────────────────────────────────────────────────────────────────

#[derive(Clone)]
enum Stage {
    Checking,
    Available(VersionInfo),
    /// Rien à faire : on lance directement le jeu. Le message est affiché
    /// brièvement (ex. "hors-ligne").
    ReadyToPlay(String),
    Downloading,
    Extracting,
    Failed(String),
}

struct Shared {
    stage: Mutex<Stage>,
    downloaded: AtomicU64,
    total: AtomicU64,
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
        });

        {
            let shared = shared.clone();
            let ctx = ctx.clone();
            std::thread::spawn(move || {
                let stage = match check_for_update() {
                    Ok(Some(info)) => Stage::Available(info),
                    Ok(None) => Stage::ReadyToPlay("Le jeu est a jour".into()),
                    Err(e) => Stage::ReadyToPlay(format!("Hors-ligne ({})", e)),
                };
                shared.set(stage);
                ctx.request_repaint();
            });
        }

        Self { shared, ctx, install_dir, game_exe, launch_error: None }
    }

    fn start_update(&self, info: VersionInfo) {
        let shared = self.shared.clone();
        let ctx = self.ctx.clone();
        let dir = self.install_dir.clone();
        shared.set(Stage::Downloading);
        std::thread::spawn(move || {
            let result = download_and_apply(&info, &dir, &shared, &ctx);
            shared.set(match result {
                Ok(()) => Stage::ReadyToPlay(format!("Mise a jour v{} installee", info.version)),
                Err(e) => Stage::Failed(e.to_string()),
            });
            ctx.request_repaint();
        });
    }

    /// Lance le jeu et ferme le launcher.
    fn launch_game(&mut self, ctx: &egui::Context) {
        if !self.game_exe.exists() {
            self.launch_error = Some(format!("{} introuvable", self.game_exe.display()));
            return;
        }
        match Command::new(&self.game_exe).current_dir(&self.install_dir).spawn() {
            Ok(_) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            Err(e) => self.launch_error = Some(format!("Impossible de lancer le jeu: {}", e)),
        }
    }
}

impl eframe::App for LauncherApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let ctx = ctx.clone();
        let stage = self.shared.stage.lock().unwrap().clone();

        egui::CentralPanel::default().show(&ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(8.0);
                ui.heading("SpaceSpore");
                ui.label(format!("Version installee : v{}", VERSION));
                ui.add_space(16.0);

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
                        ui.label(
                            egui::RichText::new(format!("Nouvelle version disponible : v{}", info.version))
                                .strong(),
                        );
                        ui.label(&info.release_notes);
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            if ui.button("Mettre a jour").clicked() {
                                self.start_update(info.clone());
                            }
                            if ui.button("Jouer sans mettre a jour").clicked() {
                                self.launch_game(&ctx);
                            }
                        });
                    }
                    Stage::Downloading => {
                        let done = self.shared.downloaded.load(Ordering::Relaxed);
                        let total = self.shared.total.load(Ordering::Relaxed);
                        ui.label("Telechargement de la mise a jour...");
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
                            ui.label("Installation de la mise a jour...");
                        });
                    }
                    Stage::ReadyToPlay(msg) => {
                        ui.label(msg);
                        ui.label("Lancement de SpaceSpore...");
                        self.launch_game(&ctx);
                    }
                    Stage::Failed(err) => {
                        ui.colored_label(egui::Color32::LIGHT_RED, format!("Erreur de mise a jour : {}", err));
                        ui.add_space(12.0);
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
//  Réseau / mise à jour
// ─────────────────────────────────────────────────────────────────────────

fn make_agent(timeout_secs: u64) -> ureq::Agent {
    let tls = ureq::tls::TlsConfig::builder()
        .provider(ureq::tls::TlsProvider::NativeTls)
        .build();
    let config = ureq::Agent::config_builder()
        .tls_config(tls)
        .timeout_global(Some(std::time::Duration::from_secs(timeout_secs)))
        .build();
    ureq::Agent::new_with_config(config)
}

/// `Ok(Some(info))` si une mise à jour existe pour cette plateforme.
fn check_for_update() -> Result<Option<VersionInfo>, String> {
    let agent = make_agent(5);
    let body = agent
        .get(spacespore_common::VERSION_URL)
        .call()
        .map_err(|e| e.to_string())?
        .body_mut()
        .read_to_string()
        .map_err(|e| e.to_string())?;
    let info: VersionInfo = serde_json::from_str(&body).map_err(|e| e.to_string())?;

    if spacespore_common::needs_update(info.version_code) && info.platform_download_url().is_some() {
        Ok(Some(info))
    } else {
        Ok(None)
    }
}

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

    let agent = make_agent(120);
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
