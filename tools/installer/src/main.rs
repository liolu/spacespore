#![windows_subsystem = "windows"]

extern crate native_windows_gui as nwg;
extern crate native_windows_derive as nwd;

use nwd::NwgUi;
use nwg::NativeUi;
use std::cell::RefCell;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, atomic::{AtomicU64, AtomicBool, Ordering}};
use std::thread;

#[derive(Default, NwgUi)]
pub struct InstallerApp {
    #[nwg_control(
        size: (500, 340),
        position: (300, 200),
        title: "SpaceSpore Installer",
        flags: "WINDOW|VISIBLE",
        center: true
    )]
    #[nwg_events(OnWindowClose: [InstallerApp::on_close])]
    window: nwg::Window,

    #[nwg_control(
        text: "SpaceSpore Installer",
        size: (460, 30),
        position: (20, 15)
    )]
    title_label: nwg::Label,

    #[nwg_control(
        text: "Dossier d'installation :",
        size: (460, 20),
        position: (20, 60)
    )]
    path_label: nwg::Label,

    #[nwg_control(
        text: "",
        size: (370, 25),
        position: (20, 85)
    )]
    path_input: nwg::TextInput,

    #[nwg_control(
        text: "Parcourir",
        size: (80, 25),
        position: (400, 85)
    )]
    #[nwg_events(OnButtonClick: [InstallerApp::on_browse])]
    browse_btn: nwg::Button,

    #[nwg_control(
        text: "Creer un raccourci Bureau",
        size: (250, 25),
        position: (20, 125)
    )]
    desktop_shortcut: nwg::CheckBox,

    #[nwg_control(
        text: "Creer un raccourci Menu Demarrer",
        size: (260, 25),
        position: (20, 155)
    )]
    startmenu_shortcut: nwg::CheckBox,

    #[nwg_control(
        text: "Installer",
        size: (150, 35),
        position: (175, 200)
    )]
    #[nwg_events(OnButtonClick: [InstallerApp::on_install])]
    install_btn: nwg::Button,

    #[nwg_control(
        size: (460, 20),
        position: (20, 250),
        range: 0..100
    )]
    progress: nwg::ProgressBar,

    #[nwg_control(
        text: "Pret a installer",
        size: (460, 20),
        position: (20, 280)
    )]
    status_label: nwg::Label,

    #[nwg_control]
    #[nwg_events(OnNotice: [InstallerApp::on_notice])]
    notice: nwg::Notice,

    progress_val: RefCell<Arc<AtomicU64>>,
    total_val: RefCell<Arc<AtomicU64>>,
    done_flag: RefCell<Arc<AtomicBool>>,
    error_msg: RefCell<Arc<std::sync::Mutex<Option<String>>>>,
    install_dir: RefCell<PathBuf>,
}

impl InstallerApp {
    fn on_close(&self) {
        nwg::stop_thread_dispatch();
    }

    fn on_browse(&self) {
        let mut dialog = Default::default();
        let _ = nwg::FileDialog::builder()
            .title("Choisir le dossier d'installation")
            .action(nwg::FileDialogAction::OpenDirectory)
            .build(&mut dialog);

        if dialog.run(Some(&self.window)) {
            if let Ok(path) = dialog.get_selected_item() {
                let path = PathBuf::from(path).join("SpaceSpore");
                self.path_input.set_text(&path.to_string_lossy());
            }
        }
    }

    fn on_install(&self) {
        let install_path = self.path_input.text();
        if install_path.is_empty() {
            nwg::modal_info_message(&self.window, "Erreur", "Veuillez choisir un dossier d'installation.");
            return;
        }

        let install_dir = PathBuf::from(&install_path);
        *self.install_dir.borrow_mut() = install_dir.clone();

        self.install_btn.set_enabled(false);
        self.browse_btn.set_enabled(false);
        self.status_label.set_text("Recuperation des informations...");

        let progress = Arc::new(AtomicU64::new(0));
        let total = Arc::new(AtomicU64::new(0));
        let done = Arc::new(AtomicBool::new(false));
        let error = Arc::new(std::sync::Mutex::new(None::<String>));

        *self.progress_val.borrow_mut() = progress.clone();
        *self.total_val.borrow_mut() = total.clone();
        *self.done_flag.borrow_mut() = done.clone();
        *self.error_msg.borrow_mut() = error.clone();

        let create_desktop = self.desktop_shortcut.check_state() == nwg::CheckBoxState::Checked;
        let create_startmenu = self.startmenu_shortcut.check_state() == nwg::CheckBoxState::Checked;
        let sender = self.notice.sender();

        thread::spawn(move || {
            if let Err(e) = do_install(&install_dir, &progress, &total, create_desktop, create_startmenu, &sender) {
                *error.lock().unwrap() = Some(e);
            }
            done.store(true, Ordering::Relaxed);
            sender.notice();
        });
    }

    fn on_notice(&self) {
        let progress = self.progress_val.borrow().load(Ordering::Relaxed);
        let total = self.total_val.borrow().load(Ordering::Relaxed);

        if total > 0 {
            let pct = (progress as f64 / total as f64 * 100.0) as u32;
            self.progress.set_pos(pct);
            let mb_done = progress as f64 / 1_048_576.0;
            let mb_total = total as f64 / 1_048_576.0;
            self.status_label.set_text(&format!(
                "Telechargement... {:.1} MB / {:.1} MB ({}%)",
                mb_done, mb_total, pct
            ));
        }

        if self.done_flag.borrow().load(Ordering::Relaxed) {
            self.progress.set_pos(100);

            let err = self.error_msg.borrow().lock().unwrap().clone();
            if let Some(e) = err {
                self.status_label.set_text("Erreur lors de l'installation");
                nwg::modal_info_message(&self.window, "Erreur", &format!("L'installation a echoue:\n{}", e));
                self.install_btn.set_enabled(true);
                self.browse_btn.set_enabled(true);
            } else {
                self.status_label.set_text("Installation terminee !");
                let install_dir = self.install_dir.borrow().clone();
                let msg = format!(
                    "SpaceSpore a ete installe avec succes dans:\n{}\n\nLancer le jeu maintenant ?",
                    install_dir.display()
                );
                let params = nwg::MessageParams {
                    title: "Installation terminee",
                    content: &msg,
                    buttons: nwg::MessageButtons::YesNo,
                    icons: nwg::MessageIcons::Info,
                };
                if nwg::message(&params) == nwg::MessageChoice::Yes {
                    let launcher = install_dir.join("spacespore-launcher.exe");
                    let game = install_dir.join("spacespore.exe");
                    let exe = if launcher.exists() { launcher } else { game };
                    let _ = std::process::Command::new(&exe)
                        .current_dir(&install_dir)
                        .spawn();
                }
                nwg::stop_thread_dispatch();
            }
        }
    }
}

fn do_install(
    install_dir: &Path,
    progress: &AtomicU64,
    total: &AtomicU64,
    create_desktop: bool,
    create_startmenu: bool,
    sender: &nwg::NoticeSender,
) -> Result<(), String> {
    let info = fetch_version_info()?;
    sender.notice();

    fs::create_dir_all(install_dir).map_err(|e| format!("Impossible de creer le dossier: {}", e))?;

    let temp_zip = std::env::temp_dir().join("spacespore-install.zip");
    download_file(&info.download_url, &temp_zip, progress, total, sender)?;

    extract_zip(&temp_zip, install_dir)?;
    let _ = fs::remove_file(&temp_zip);

    let game_exe = install_dir.join("spacespore.exe");
    let launcher_exe = install_dir.join("spacespore-launcher.exe");
    let target_exe = if launcher_exe.exists() { &launcher_exe } else { &game_exe };

    if create_startmenu {
        if let Some(data_dir) = dirs::data_dir() {
            let programs = data_dir
                .join("Microsoft")
                .join("Windows")
                .join("Start Menu")
                .join("Programs");
            let lnk = programs.join("SpaceSpore.lnk");
            if let Ok(sl) = mslnk::ShellLink::new(target_exe.to_str().unwrap_or_default()) {
                let _ = sl.create_lnk(lnk.to_str().unwrap_or_default());
            }
        }
    }

    if create_desktop {
        if let Some(desktop) = dirs::desktop_dir() {
            let lnk = desktop.join("SpaceSpore.lnk");
            if let Ok(sl) = mslnk::ShellLink::new(target_exe.to_str().unwrap_or_default()) {
                let _ = sl.create_lnk(lnk.to_str().unwrap_or_default());
            }
        }
    }

    Ok(())
}

fn fetch_version_info() -> Result<spacespore_common::VersionInfo, String> {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(10)))
        .build();
    let agent = ureq::Agent::new_with_config(config);

    let body: String = agent
        .get(spacespore_common::VERSION_URL)
        .call()
        .map_err(|e| format!("Erreur reseau: {}\n\nVerifiez que GitHub Pages est active sur le depot.", e))?
        .body_mut()
        .read_to_string()
        .map_err(|e| format!("Erreur lecture: {}", e))?;

    serde_json::from_str(&body).map_err(|e| format!("Erreur JSON: {}", e))
}

fn download_file(
    url: &str,
    dest: &Path,
    progress: &AtomicU64,
    total_size: &AtomicU64,
    sender: &nwg::NoticeSender,
) -> Result<(), String> {
    let response = ureq::get(url)
        .call()
        .map_err(|e| format!("Erreur telechargement: {}", e))?;

    let content_length = response
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);

    total_size.store(content_length, Ordering::Relaxed);

    let mut body = response.into_body();
    let mut file = fs::File::create(dest).map_err(|e| format!("Erreur fichier: {}", e))?;
    let mut downloaded: u64 = 0;
    let mut buf = [0u8; 8192];
    let mut last_notify = std::time::Instant::now();

    loop {
        let n = body.as_reader().read(&mut buf).map_err(|e| format!("Erreur lecture: {}", e))?;
        if n == 0 { break; }
        file.write_all(&buf[..n]).map_err(|e| format!("Erreur ecriture: {}", e))?;
        downloaded += n as u64;
        progress.store(downloaded, Ordering::Relaxed);

        if last_notify.elapsed() >= std::time::Duration::from_millis(100) {
            sender.notice();
            last_notify = std::time::Instant::now();
        }
    }

    if content_length > 0 && downloaded != content_length {
        return Err(format!("Telechargement incomplet: {} / {}", downloaded, content_length));
    }

    Ok(())
}

fn extract_zip(zip_path: &Path, install_dir: &Path) -> Result<(), String> {
    let file = fs::File::open(zip_path).map_err(|e| format!("Erreur ouverture zip: {}", e))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("Erreur zip: {}", e))?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| format!("Erreur zip entry: {}", e))?;
        let raw_name = entry.name().to_string();

        let relative = raw_name.split('/').skip(1).collect::<Vec<_>>().join("/");
        if relative.is_empty() { continue; }

        let out_path = install_dir.join(&relative);

        if entry.is_dir() {
            let _ = fs::create_dir_all(&out_path);
        } else {
            if let Some(parent) = out_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let mut outfile = fs::File::create(&out_path)
                .map_err(|e| format!("Erreur extraction {}: {}", relative, e))?;
            std::io::copy(&mut entry, &mut outfile)
                .map_err(|e| format!("Erreur copie {}: {}", relative, e))?;
        }
    }

    Ok(())
}

fn main() {
    nwg::init().expect("Failed to init Native Windows GUI");

    let mut font = Default::default();
    let _ = nwg::Font::builder()
        .family("Segoe UI")
        .size(16)
        .build(&mut font);
    nwg::Font::set_global_default(Some(font));

    let app = InstallerApp::build_ui(Default::default()).expect("Failed to build UI");

    let default_path = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("C:\\"))
        .join("Games")
        .join("SpaceSpore");
    app.path_input.set_text(&default_path.to_string_lossy());
    app.desktop_shortcut.set_check_state(nwg::CheckBoxState::Checked);
    app.startmenu_shortcut.set_check_state(nwg::CheckBoxState::Checked);

    nwg::dispatch_thread_events();
}
