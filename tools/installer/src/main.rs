use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("========================================");
    println!("     SpaceSpore Installer v{}", spacespore_common::VERSION);
    println!("========================================");
    println!();

    let default_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("C:\\"))
        .join("Games")
        .join("SpaceSpore");

    println!(
        "Install directory [{}]:",
        default_dir.display()
    );
    print!("> ");
    let _ = io::stdout().flush();

    let mut input = String::new();
    let _ = io::stdin().read_line(&mut input);
    let input = input.trim();

    let install_dir = if input.is_empty() {
        default_dir
    } else {
        PathBuf::from(input)
    };

    println!();
    println!("Fetching latest version info...");

    let version_info: spacespore_common::VersionInfo = match fetch_version_info() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Failed to fetch version info: {}", e);
            wait_exit();
            return;
        }
    };

    println!(
        "Latest version: {} ({})",
        version_info.version, version_info.release_notes
    );
    println!("Downloading from: {}", version_info.download_url);
    println!();

    let zip_path = std::env::temp_dir().join("spacespore-install.zip");

    if let Err(e) = download_file(&version_info.download_url, &zip_path) {
        eprintln!("Download failed: {}", e);
        wait_exit();
        return;
    }

    println!();
    println!("Extracting to {}...", install_dir.display());
    fs::create_dir_all(&install_dir).ok();

    if let Err(e) = extract_zip(&zip_path, &install_dir) {
        eprintln!("Extraction failed: {}", e);
        wait_exit();
        return;
    }

    let _ = fs::remove_file(&zip_path);

    println!();
    println!("Creating Start Menu shortcut...");
    create_shortcut(&install_dir);

    println!();
    println!("========================================");
    println!("  Installation complete!");
    println!("  Location: {}", install_dir.display());
    println!("========================================");
    println!();
    println!("Press Enter to launch SpaceSpore...");
    let _ = io::stdin().read(&mut [0u8]);

    let game_exe = install_dir.join("spacespore.exe");
    if game_exe.exists() {
        let _ = Command::new(&game_exe).current_dir(&install_dir).spawn();
    }
}

fn fetch_version_info() -> Result<spacespore_common::VersionInfo, Box<dyn std::error::Error>> {
    let body: String = ureq::get(spacespore_common::VERSION_URL).call()?.body_mut().read_to_string()?;
    let info: spacespore_common::VersionInfo = serde_json::from_str(&body)?;
    Ok(info)
}

fn download_file(url: &str, dest: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let response = ureq::get(url).call()?;

    let total_size = response
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);

    let mut body = response.into_body();
    let mut file = fs::File::create(dest)?;
    let mut downloaded: u64 = 0;
    let mut buf = [0u8; 8192];

    loop {
        let n = body.as_reader().read(&mut buf)?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])?;
        downloaded += n as u64;

        if total_size > 0 {
            let pct = (downloaded as f64 / total_size as f64 * 100.0) as u32;
            let mb_done = downloaded as f64 / 1_048_576.0;
            let mb_total = total_size as f64 / 1_048_576.0;
            print!(
                "\r  [{:>3}%] {:.1} MB / {:.1} MB",
                pct, mb_done, mb_total
            );
            let _ = io::stdout().flush();
        }
    }
    println!();
    Ok(())
}

fn extract_zip(zip_path: &Path, install_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let file = fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let raw_name = entry.name().to_string();

        let relative = raw_name
            .split('/')
            .skip(1)
            .collect::<Vec<_>>()
            .join("/");

        if relative.is_empty() {
            continue;
        }

        let out_path = install_dir.join(&relative);

        if entry.is_dir() {
            fs::create_dir_all(&out_path)?;
        } else {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut outfile = fs::File::create(&out_path)?;
            io::copy(&mut entry, &mut outfile)?;
            println!("  {}", relative);
        }
    }

    Ok(())
}

fn create_shortcut(install_dir: &Path) {
    let game_exe = install_dir.join("spacespore.exe");

    if let Some(start_menu) = dirs::data_dir() {
        let programs = start_menu
            .join("Microsoft")
            .join("Windows")
            .join("Start Menu")
            .join("Programs");

        let lnk_path = programs.join("SpaceSpore.lnk");

        match mslnk::ShellLink::new(game_exe.to_str().unwrap_or_default()) {
            Ok(sl) => {
                if let Err(e) = sl.create_lnk(lnk_path.to_str().unwrap_or_default()) {
                    eprintln!("  Warning: could not create shortcut: {}", e);
                } else {
                    println!("  Created: {}", lnk_path.display());
                }
            }
            Err(e) => {
                eprintln!("  Warning: could not create shortcut: {}", e);
            }
        }
    }
}

fn wait_exit() {
    println!("Press Enter to exit...");
    let _ = io::stdin().read(&mut [0u8]);
}
