//! Installeur en mode terminal pour Linux et macOS.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use spacespore_common::{exe_name, GAME_BIN, LAUNCHER_BIN};

pub fn run() {
    println!("========================================");
    println!("  SpaceSpore Installer ({})", spacespore_common::PLATFORM);
    println!("========================================");
    println!();

    let default_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Games")
        .join("SpaceSpore");

    let answer = ask(&format!("Dossier d'installation [{}] > ", default_dir.display()));
    let install_dir = if answer.is_empty() { default_dir } else { expand_tilde(&answer) };

    let create_shortcuts = ask_yes_no(shortcut_question());

    println!();
    println!("Installation dans {}", install_dir.display());

    let progress = AtomicU64::new(0);
    let total = AtomicU64::new(0);
    let notify = || {
        let done = progress.load(Ordering::Relaxed);
        let size = total.load(Ordering::Relaxed);
        if size > 0 {
            let pct = (done as f64 / size as f64 * 100.0) as u32;
            print!(
                "\r  [{:>3}%] {:.1} MB / {:.1} MB",
                pct,
                done as f64 / 1_048_576.0,
                size as f64 / 1_048_576.0
            );
            let _ = io::stdout().flush();
        }
    };

    if let Err(e) = crate::install::download_and_extract(&install_dir, &progress, &total, &notify) {
        println!();
        eprintln!("L'installation a echoue:\n{}", e);
        std::process::exit(1);
    }
    println!();

    let launcher = install_dir.join(exe_name(LAUNCHER_BIN));
    let game = install_dir.join(exe_name(GAME_BIN));
    let target = if launcher.exists() { launcher } else { game };

    if create_shortcuts {
        match create_platform_shortcuts(&install_dir, &target) {
            Ok(created) => {
                for p in created {
                    println!("  Raccourci cree: {}", p.display());
                }
            }
            Err(e) => eprintln!("  Impossible de creer les raccourcis: {}", e),
        }
    }

    println!();
    println!("Installation terminee !");
    println!("Pour lancer le jeu: {}", target.display());

    if ask_yes_no("Lancer le jeu maintenant ? (O/n) > ") {
        if let Err(e) = std::process::Command::new(&target)
            .current_dir(&install_dir)
            .status()
        {
            eprintln!("Impossible de lancer le jeu: {}", e);
        }
    }
}

fn ask(prompt: &str) -> String {
    print!("{}", prompt);
    let _ = io::stdout().flush();
    let mut input = String::new();
    let _ = io::stdin().read_line(&mut input);
    input.trim().to_string()
}

fn ask_yes_no(prompt: &str) -> bool {
    let a = ask(prompt).to_lowercase();
    a.is_empty() || a == "o" || a == "oui" || a == "y" || a == "yes"
}

fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(path)
}

#[cfg(target_os = "macos")]
fn shortcut_question() -> &'static str {
    "Creer un raccourci sur le Bureau et dans Applications ? (O/n) > "
}

#[cfg(not(target_os = "macos"))]
fn shortcut_question() -> &'static str {
    "Ajouter SpaceSpore au menu des applications et au Bureau ? (O/n) > "
}

/// macOS : fichiers `.command` (double-clic → ouvre le Terminal et lance le jeu).
#[cfg(target_os = "macos")]
fn create_platform_shortcuts(_install_dir: &Path, target: &Path) -> io::Result<Vec<PathBuf>> {
    let script = format!(
        "#!/bin/bash\ncd \"$(dirname \"{0}\")\"\nexec \"{0}\"\n",
        target.display()
    );
    let mut created = Vec::new();
    let mut dirs_to_use = Vec::new();
    if let Some(d) = dirs::desktop_dir() {
        dirs_to_use.push(d);
    }
    if let Some(h) = dirs::home_dir() {
        dirs_to_use.push(h.join("Applications"));
    }
    for dir in dirs_to_use {
        fs::create_dir_all(&dir)?;
        let path = dir.join("SpaceSpore.command");
        write_executable(&path, &script)?;
        created.push(path);
    }
    Ok(created)
}

/// Linux : entrée `.desktop` (menu des applications + Bureau).
#[cfg(not(target_os = "macos"))]
fn create_platform_shortcuts(install_dir: &Path, target: &Path) -> io::Result<Vec<PathBuf>> {
    let entry = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=SpaceSpore\n\
         Comment=Voxel Universe\n\
         Exec=\"{}\"\n\
         Path={}\n\
         Terminal=true\n\
         Categories=Game;Simulation;\n",
        target.display(),
        install_dir.display()
    );
    let mut created = Vec::new();
    if let Some(data) = dirs::data_dir() {
        let apps = data.join("applications");
        fs::create_dir_all(&apps)?;
        let path = apps.join("spacespore.desktop");
        write_executable(&path, &entry)?;
        created.push(path);
    }
    if let Some(desktop) = dirs::desktop_dir() {
        if desktop.is_dir() {
            let path = desktop.join("spacespore.desktop");
            write_executable(&path, &entry)?;
            created.push(path);
        }
    }
    Ok(created)
}

fn write_executable(path: &Path, content: &str) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::write(path, content)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))
}
