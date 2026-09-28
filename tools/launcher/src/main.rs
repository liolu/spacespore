use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;
use std::process::Command;

fn main() {
    println!("========================================");
    println!("  SpaceSpore Launcher v{}", spacespore_common::VERSION);
    println!("========================================");
    println!();

    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    let game_exe = exe_dir.join("spacespore.exe");

    println!("Verification des mises a jour...");
    match check_for_update() {
        UpdateResult::Available(info) => {
            println!();
            println!("  Nouvelle version disponible: v{}", info.version);
            println!("  {}", info.release_notes);
            println!();
            print!("  Mettre a jour maintenant? (O/n) > ");
            let _ = io::stdout().flush();

            let mut input = String::new();
            let _ = io::stdin().read_line(&mut input);
            let input = input.trim().to_lowercase();

            if input.is_empty() || input == "o" || input == "oui" || input == "y" {
                println!();
                if let Err(e) = download_and_apply(&info, &exe_dir) {
                    eprintln!("Erreur de mise a jour: {}", e);
                    eprintln!("Lancement de la version actuelle...");
                }
            } else {
                println!("  Mise a jour ignoree.");
            }
        }
        UpdateResult::UpToDate => {
            println!("  Le jeu est a jour (v{})", spacespore_common::VERSION);
        }
        UpdateResult::Error(e) => {
            println!("  Impossible de verifier: {}", e);
            println!("  Lancement en mode hors-ligne...");
        }
    }

    println!();
    if game_exe.exists() {
        println!("Lancement de SpaceSpore...");
        let _ = Command::new(&game_exe)
            .current_dir(&exe_dir)
            .spawn();
    } else {
        eprintln!("spacespore.exe introuvable dans {}", exe_dir.display());
        eprintln!("Appuyez sur Entree pour quitter...");
        let _ = io::stdin().read(&mut [0u8]);
    }
}

enum UpdateResult {
    Available(spacespore_common::VersionInfo),
    UpToDate,
    Error(String),
}

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

fn check_for_update() -> UpdateResult {
    let agent = make_agent(5);

    let body: String = match agent.get(spacespore_common::VERSION_URL).call() {
        Ok(mut resp) => match resp.body_mut().read_to_string() {
            Ok(s) => s,
            Err(e) => return UpdateResult::Error(format!("{}", e)),
        },
        Err(e) => return UpdateResult::Error(format!("{}", e)),
    };

    let info: spacespore_common::VersionInfo = match serde_json::from_str(&body) {
        Ok(i) => i,
        Err(e) => return UpdateResult::Error(format!("{}", e)),
    };

    if spacespore_common::needs_update(info.version_code) {
        UpdateResult::Available(info)
    } else {
        UpdateResult::UpToDate
    }
}

fn download_and_apply(
    info: &spacespore_common::VersionInfo,
    install_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = std::env::temp_dir().join("spacespore-update");
    fs::create_dir_all(&temp_dir)?;
    let zip_path = temp_dir.join("update.zip");

    println!("  Telechargement...");
    let agent = make_agent(120);
    let response = agent.get(&info.download_url).call()?;

    let total_size = response
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);

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

        if total_size > 0 {
            let pct = (downloaded as f64 / total_size as f64 * 100.0) as u32;
            let mb_done = downloaded as f64 / 1_048_576.0;
            let mb_total = total_size as f64 / 1_048_576.0;
            print!("\r  [{:>3}%] {:.1} MB / {:.1} MB", pct, mb_done, mb_total);
            let _ = io::stdout().flush();
        }
    }
    println!();

    if total_size > 0 && downloaded != total_size {
        return Err(format!(
            "Telechargement incomplet: {} / {} octets",
            downloaded, total_size
        )
        .into());
    }

    println!("  Extraction...");
    let zip_file = fs::File::open(&zip_path)?;
    let mut archive = zip::ZipArchive::new(zip_file)?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let raw_name = entry.name().to_string();

        let relative = raw_name.split('/').skip(1).collect::<Vec<_>>().join("/");
        if relative.is_empty() {
            continue;
        }

        // Ne pas écraser le launcher lui-même pendant qu'il tourne
        if relative == "spacespore-launcher.exe" {
            let old_path = install_dir.join("spacespore-launcher.exe.old");
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
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut outfile = fs::File::create(&out_path)?;
            io::copy(&mut entry, &mut outfile)?;
            println!("    {}", relative);
        }
    }

    let _ = fs::remove_file(&zip_path);
    let _ = fs::remove_dir_all(&temp_dir);

    println!("  Mise a jour terminee!");
    Ok(())
}
