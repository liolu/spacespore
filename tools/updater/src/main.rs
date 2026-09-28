use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::Path;
use std::process::Command;

#[derive(serde::Deserialize)]
struct UpdateConfig {
    zip_path: String,
    install_dir: String,
    game_exe: String,
    game_pid: u32,
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 3 || args[1] != "--apply" {
        eprintln!("Usage: spacespore-updater --apply <update.json>");
        std::process::exit(1);
    }

    let config_path = &args[2];
    println!("SpaceSpore Updater v{}", spacespore_common::VERSION);
    println!("Reading update config: {}", config_path);

    let config: UpdateConfig = match fs::read_to_string(config_path) {
        Ok(s) => match serde_json::from_str(&s) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Failed to parse update config: {}", e);
                std::process::exit(1);
            }
        },
        Err(e) => {
            eprintln!("Failed to read update config: {}", e);
            std::process::exit(1);
        }
    };

    println!("Waiting for game (PID {}) to exit...", config.game_pid);
    wait_for_process(config.game_pid);

    println!("Extracting update...");
    if let Err(e) = extract_update(&config) {
        eprintln!("Failed to extract update: {}", e);
        eprintln!("Press Enter to exit...");
        let _ = io::stdin().read(&mut [0u8]);
        std::process::exit(1);
    }

    if let Err(e) = fs::remove_file(config_path) {
        eprintln!("Warning: could not remove update config: {}", e);
    }
    if let Err(e) = fs::remove_file(&config.zip_path) {
        eprintln!("Warning: could not remove update zip: {}", e);
    }

    println!("Update complete! Launching SpaceSpore...");
    let game_path = Path::new(&config.install_dir).join(&config.game_exe);
    let _ = Command::new(&game_path).spawn();
}

fn wait_for_process(pid: u32) {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Threading::{OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE};
        use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};

        unsafe {
            let handle = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid);
            if !handle.is_null() && handle != INVALID_HANDLE_VALUE {
                WaitForSingleObject(handle, 10_000);
                CloseHandle(handle);
            }
        }
    }

    #[cfg(not(windows))]
    {
        let _ = pid;
        thread::sleep(Duration::from_secs(2));
    }
}

fn extract_update(config: &UpdateConfig) -> Result<(), Box<dyn std::error::Error>> {
    let zip_file = fs::File::open(&config.zip_path)?;
    let mut archive = zip::ZipArchive::new(zip_file)?;
    let install_dir = Path::new(&config.install_dir);

    let updater_exe = install_dir.join("spacespore-updater.exe");
    let updater_old = install_dir.join("spacespore-updater.exe.old");
    if updater_exe.exists() {
        let _ = fs::remove_file(&updater_old);
        fs::rename(&updater_exe, &updater_old)?;
    }

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
            println!("  extracted: {}", relative);
        }
    }

    Ok(())
}
