use std::path::{Path, PathBuf};

pub const VERSION: &str = "0.6.2";
pub const VERSION_CODE: u32 = 602;
pub const UPDATER_VERSION: u32 = 1;

pub const REPO_OWNER: &str = "liolu";
pub const REPO_NAME: &str = "spacespore";
pub const VERSION_URL: &str = "https://liolu.github.io/spacespore/version.json";

#[derive(serde::Deserialize, Debug, Clone)]
pub struct VersionInfo {
    pub version: String,
    pub version_code: u32,
    /// URL du zip Windows (conservé tel quel pour les anciens clients).
    pub download_url: String,
    #[serde(default)]
    pub download_url_linux: Option<String>,
    #[serde(default)]
    pub download_url_macos: Option<String>,
    pub release_notes: String,
    pub min_updater_version: u32,
}

impl VersionInfo {
    /// URL du zip correspondant à la plateforme courante, s'il en existe un.
    pub fn platform_download_url(&self) -> Option<&str> {
        match PLATFORM {
            "linux" => self.download_url_linux.as_deref(),
            "macos" => self.download_url_macos.as_deref(),
            _ => Some(self.download_url.as_str()),
        }
    }
}

pub fn needs_update(remote_code: u32) -> bool {
    remote_code > VERSION_CODE
}

// ─────────────────────────────────────────────────────────────────────────
//  Plateforme
// ─────────────────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
pub const PLATFORM: &str = "windows";
#[cfg(target_os = "macos")]
pub const PLATFORM: &str = "macos";
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub const PLATFORM: &str = "linux";

pub const GAME_BIN: &str = "spacespore";
pub const LAUNCHER_BIN: &str = "spacespore-launcher";
pub const UPDATER_BIN: &str = "spacespore-updater";

/// Nom de fichier d'un exécutable (`spacespore` → `spacespore.exe` sous Windows).
pub fn exe_name(bin: &str) -> String {
    format!("{}{}", bin, std::env::consts::EXE_SUFFIX)
}

/// Dossier contenant l'exécutable courant.
pub fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
}

/// Écrit un fichier extrait d'une mise à jour.
///
/// Le contenu passe par un fichier temporaire puis un renommage : sous
/// Linux/macOS, écraser directement un binaire en cours d'exécution échoue
/// ("Text file busy"), alors qu'un renommage remplace le fichier proprement.
/// Les droits d'exécution Unix sont restaurés depuis le zip (ou forcés pour
/// les binaires connus si le zip n'en contient pas).
pub fn write_extracted_file(
    out_path: &Path,
    reader: &mut dyn std::io::Read,
    unix_mode: Option<u32>,
) -> std::io::Result<()> {
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut tmp_name = out_path.as_os_str().to_owned();
    tmp_name.push(".tmp");
    let tmp_path = PathBuf::from(tmp_name);
    {
        let mut out = std::fs::File::create(&tmp_path)?;
        std::io::copy(reader, &mut out)?;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let is_known_bin = out_path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| [GAME_BIN, LAUNCHER_BIN, UPDATER_BIN].contains(&n) || n.ends_with(".sh") || n.ends_with(".command"))
            .unwrap_or(false);
        let mut mode = unix_mode.map(|m| m & 0o777).filter(|m| *m != 0).unwrap_or(0o644);
        if is_known_bin {
            mode |= 0o755;
        }
        std::fs::set_permissions(&tmp_path, std::fs::Permissions::from_mode(mode))?;
    }
    #[cfg(not(unix))]
    let _ = unix_mode;

    #[cfg(windows)]
    if out_path.exists() {
        // Sous Windows, rename n'écrase pas toujours un fichier existant.
        std::fs::remove_file(out_path)?;
    }
    std::fs::rename(&tmp_path, out_path)
}

/// Retire le quarantine flag macOS des fichiers téléchargés, sinon Gatekeeper
/// refuse de lancer les binaires non signés.
pub fn clear_macos_quarantine(_dir: &Path) {
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("xattr")
            .args(["-dr", "com.apple.quarantine"])
            .arg(_dir)
            .status();
    }
}
