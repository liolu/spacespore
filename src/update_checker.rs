use bevy::prelude::*;
use spacespore_common::{VersionInfo, VERSION, VERSION_CODE, UPDATER_VERSION, VERSION_URL};
use std::sync::{Arc, Mutex};
use std::io::Read as IoRead;

#[derive(Resource)]
pub struct UpdateState {
    pub check_result: Arc<Mutex<Option<CheckResult>>>,
    pub status: UpdateStatus,
    pub download_progress: Arc<std::sync::atomic::AtomicU64>,
    pub download_total: Arc<std::sync::atomic::AtomicU64>,
}

#[derive(Clone, Debug)]
pub enum CheckResult {
    Available(VersionInfo),
    UpToDate,
    Error(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum UpdateStatus {
    Checking,
    NoUpdate,
    Available(String),
    Downloading,
    ReadyToApply(String),
    Applying,
    Failed(String),
}

pub fn spawn_update_check() -> UpdateState {
    let result = Arc::new(Mutex::new(None));
    let result_clone = result.clone();

    std::thread::spawn(move || {
        match check_version() {
            Ok(info) => {
                if spacespore_common::needs_update(info.version_code) {
                    *result_clone.lock().unwrap() = Some(CheckResult::Available(info));
                } else {
                    *result_clone.lock().unwrap() = Some(CheckResult::UpToDate);
                }
            }
            Err(e) => {
                *result_clone.lock().unwrap() = Some(CheckResult::Error(e));
            }
        }
    });

    UpdateState {
        check_result: result,
        status: UpdateStatus::Checking,
        download_progress: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        download_total: Arc::new(std::sync::atomic::AtomicU64::new(0)),
    }
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

fn check_version() -> Result<VersionInfo, String> {
    let agent = make_agent(3);

    let body: String = agent
        .get(VERSION_URL)
        .call()
        .map_err(|e| format!("Network error: {}", e))?
        .body_mut()
        .read_to_string()
        .map_err(|e| format!("Read error: {}", e))?;

    serde_json::from_str(&body).map_err(|e| format!("Parse error: {}", e))
}

pub fn poll_update_check(mut state: ResMut<UpdateState>) {
    if state.status != UpdateStatus::Checking {
        return;
    }

    let result = state.check_result.lock().unwrap().clone();
    match result {
        Some(CheckResult::Available(ref info)) => {
            info!("Update available: v{} (current: v{})", info.version, VERSION);
            state.status = UpdateStatus::Available(info.version.clone());
        }
        Some(CheckResult::UpToDate) => {
            info!("SpaceSpore v{} is up to date", VERSION);
            state.status = UpdateStatus::NoUpdate;
        }
        Some(CheckResult::Error(ref e)) => {
            warn!("Update check failed: {}", e);
            state.status = UpdateStatus::NoUpdate;
        }
        None => {}
    }
}

pub fn start_download(state: &mut UpdateState) {
    let result = state.check_result.lock().unwrap().clone();
    let info = match result {
        Some(CheckResult::Available(info)) => info,
        _ => return,
    };

    state.status = UpdateStatus::Downloading;
    let progress = state.download_progress.clone();
    let total = state.download_total.clone();
    let check_result = state.check_result.clone();

    std::thread::spawn(move || {
        match download_update(&info, &progress, &total) {
            Ok(zip_path) => {
                *check_result.lock().unwrap() =
                    Some(CheckResult::Available(VersionInfo {
                        version: info.version.clone(),
                        version_code: info.version_code,
                        download_url: zip_path,
                        release_notes: info.release_notes.clone(),
                        min_updater_version: info.min_updater_version,
                    }));
            }
            Err(e) => {
                *check_result.lock().unwrap() = Some(CheckResult::Error(e));
            }
        }
    });
}

fn download_update(
    info: &VersionInfo,
    progress: &std::sync::atomic::AtomicU64,
    total_size: &std::sync::atomic::AtomicU64,
) -> Result<String, String> {
    let temp_dir = std::env::temp_dir().join("spacespore-update");
    std::fs::create_dir_all(&temp_dir).map_err(|e| format!("Cannot create temp dir: {}", e))?;

    let zip_path = temp_dir.join("update.zip");

    let agent = make_agent(120);
    let response = agent.get(&info.download_url)
        .call()
        .map_err(|e| format!("Download error: {}", e))?;

    let content_length = response
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);

    total_size.store(content_length, std::sync::atomic::Ordering::Relaxed);

    let mut body = response.into_body();
    let mut file =
        std::fs::File::create(&zip_path).map_err(|e| format!("Cannot create file: {}", e))?;
    let mut downloaded: u64 = 0;
    let mut buf = [0u8; 8192];

    loop {
        let n = body
            .as_reader()
            .read(&mut buf)
            .map_err(|e| format!("Read error: {}", e))?;
        if n == 0 {
            break;
        }
        std::io::Write::write_all(&mut file, &buf[..n])
            .map_err(|e| format!("Write error: {}", e))?;
        downloaded += n as u64;
        progress.store(downloaded, std::sync::atomic::Ordering::Relaxed);
    }

    if content_length > 0 && downloaded != content_length {
        return Err(format!(
            "Incomplete download: {} / {} bytes",
            downloaded, content_length
        ));
    }

    Ok(zip_path.to_string_lossy().into_owned())
}

pub fn poll_download(mut state: ResMut<UpdateState>) {
    if state.status != UpdateStatus::Downloading {
        return;
    }

    let result = state.check_result.lock().unwrap().clone();
    match result {
        Some(CheckResult::Available(ref info)) => {
            let progress = state
                .download_progress
                .load(std::sync::atomic::Ordering::Relaxed);
            let total = state
                .download_total
                .load(std::sync::atomic::Ordering::Relaxed);

            if total > 0 && progress >= total {
                state.status = UpdateStatus::ReadyToApply(info.download_url.clone());
            }
        }
        Some(CheckResult::Error(ref e)) => {
            state.status = UpdateStatus::Failed(e.clone());
        }
        _ => {}
    }
}

pub fn apply_update(zip_path: &str) {
    let exe_path = std::env::current_exe().unwrap_or_default();
    let install_dir = exe_path
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .to_string_lossy()
        .into_owned();

    let temp_dir = std::env::temp_dir().join("spacespore-update");
    let config_path = temp_dir.join("update.json");

    let config = serde_json::json!({
        "zip_path": zip_path,
        "install_dir": install_dir,
        "game_exe": "spacespore.exe",
        "game_pid": std::process::id(),
    });

    if let Err(e) = std::fs::write(&config_path, config.to_string()) {
        warn!("Failed to write update config: {}", e);
        return;
    }

    let updater_path = std::path::Path::new(&install_dir).join("spacespore-updater.exe");
    if !updater_path.exists() {
        warn!("Updater not found at {}", updater_path.display());
        return;
    }

    match std::process::Command::new(&updater_path)
        .arg("--apply")
        .arg(config_path.to_string_lossy().as_ref())
        .spawn()
    {
        Ok(_) => {
            info!("Updater launched, exiting game...");
            std::process::exit(0);
        }
        Err(e) => {
            warn!("Failed to launch updater: {}", e);
        }
    }
}

pub struct UpdateCheckerPlugin;

impl Plugin for UpdateCheckerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (poll_update_check, poll_download));
    }
}
