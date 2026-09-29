use std::path::{Path, PathBuf};

// ─────────────────────────────────────────────────────────────────────────
//  Version et canal (stable / instable)
//
//  Ces valeurs sont injectées par la CI au moment de la compilation
//  (variables SPACESPORE_*). En local, la version vient de Cargo.toml
//  ([workspace.package] version) et le canal vaut "dev".
// ─────────────────────────────────────────────────────────────────────────

pub const VERSION: &str = match option_env!("SPACESPORE_VERSION") {
    Some(v) => v,
    None => env!("CARGO_PKG_VERSION"),
};
/// 0.6.4 -> 604 (major * 10000 + minor * 100 + patch).
pub const VERSION_CODE: u32 = version_code(VERSION);
/// Numéro de build (instable uniquement, 0 sinon).
pub const BUILD: u64 = match option_env!("SPACESPORE_BUILD") {
    Some(b) => parse_u64(b),
    None => 0,
};
pub const CHANNEL: Channel = match option_env!("SPACESPORE_CHANNEL") {
    Some(c) => Channel::from_str_const(c),
    None => Channel::Dev,
};
pub const UPDATER_VERSION: u32 = 1;

pub const REPO_OWNER: &str = "liolu";
pub const REPO_NAME: &str = "spacespore";
/// Version stable (publiée sur GitHub Pages depuis docs/version.json).
pub const VERSION_URL: &str = "https://liolu.github.io/spacespore/version.json";
/// Version instable : fichier joint à la pré-release "unstable", recréée à
/// chaque push sur main.
pub const UNSTABLE_URL: &str = "https://github.com/liolu/spacespore/releases/download/unstable/unstable.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    Stable,
    Unstable,
    /// Compilation locale (cargo build) : pas de mise à jour automatique.
    Dev,
}

impl Channel {
    const fn from_str_const(s: &str) -> Self {
        match s.as_bytes() {
            b"stable" => Channel::Stable,
            b"unstable" => Channel::Unstable,
            _ => Channel::Dev,
        }
    }

    pub fn from_name(s: &str) -> Self {
        Self::from_str_const(s)
    }

    pub fn name(self) -> &'static str {
        match self {
            Channel::Stable => "stable",
            Channel::Unstable => "unstable",
            Channel::Dev => "dev",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Channel::Stable => "Stable",
            Channel::Unstable => "Instable",
            Channel::Dev => "Developpement",
        }
    }

    fn url(self) -> Option<&'static str> {
        match self {
            Channel::Stable => Some(VERSION_URL),
            Channel::Unstable => Some(UNSTABLE_URL),
            Channel::Dev => None,
        }
    }
}

/// Texte affiché pour la version installée, ex. "v0.6.4" ou "v0.6.4 instable (build 12)".
pub fn installed_label() -> String {
    match CHANNEL {
        Channel::Stable => format!("v{}", VERSION),
        Channel::Unstable => format!("v{} instable (build {})", VERSION, BUILD),
        Channel::Dev => format!("v{} (compilation locale)", VERSION),
    }
}

const fn parse_u64(s: &str) -> u64 {
    let b = s.as_bytes();
    let mut i = 0;
    let mut n: u64 = 0;
    while i < b.len() && b[i] >= b'0' && b[i] <= b'9' {
        n = n * 10 + (b[i] - b'0') as u64;
        i += 1;
    }
    n
}

const fn version_code(v: &str) -> u32 {
    let b = v.as_bytes();
    let mut parts = [0u32; 3];
    let mut idx = 0;
    let mut i = 0;
    while i < b.len() && idx < 3 {
        let c = b[i];
        if c == b'.' {
            idx += 1;
        } else if c >= b'0' && c <= b'9' {
            parts[idx] = parts[idx] * 10 + (c - b'0') as u32;
        } else {
            break; // suffixe type "-instable.12"
        }
        i += 1;
    }
    parts[0] * 10000 + parts[1] * 100 + parts[2]
}

#[derive(serde::Deserialize, Debug, Clone)]
pub struct VersionInfo {
    pub version: String,
    pub version_code: u32,
    /// Numéro de build (canal instable).
    #[serde(default)]
    pub build: u64,
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

    /// Texte de la version distante, ex. "v0.6.5" ou "instable build 42".
    pub fn label(&self) -> String {
        if self.build > 0 {
            format!("v{} instable (build {})", self.version, self.build)
        } else {
            format!("v{}", self.version)
        }
    }
}

pub fn needs_update(remote_code: u32) -> bool {
    remote_code > VERSION_CODE
}

pub fn make_agent(timeout_secs: u64) -> ureq::Agent {
    let tls = ureq::tls::TlsConfig::builder()
        .provider(ureq::tls::TlsProvider::NativeTls)
        .build();
    let config = ureq::Agent::config_builder()
        .tls_config(tls)
        .timeout_global(Some(std::time::Duration::from_secs(timeout_secs)))
        .build();
    ureq::Agent::new_with_config(config)
}

/// Télécharge les infos de la dernière version d'un canal.
pub fn fetch_channel(channel: Channel, timeout_secs: u64) -> Result<VersionInfo, String> {
    let url = channel.url().ok_or("aucun canal de mise a jour")?;
    let body = make_agent(timeout_secs)
        .get(url)
        .call()
        .map_err(|e| e.to_string())?
        .body_mut()
        .read_to_string()
        .map_err(|e| e.to_string())?;
    serde_json::from_str(&body).map_err(|e| e.to_string())
}

/// Faut-il installer `remote` (dernière version du canal `wanted`) à la place
/// de la version actuellement installée ?
pub fn should_install(wanted: Channel, remote: &VersionInfo) -> bool {
    if remote.platform_download_url().is_none() {
        return false;
    }
    if CHANNEL != wanted {
        return true; // changement de canal
    }
    match wanted {
        Channel::Stable => needs_update(remote.version_code),
        Channel::Unstable => remote.build > BUILD,
        Channel::Dev => false,
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_codes() {
        assert_eq!(version_code("0.6.4"), 604);
        assert_eq!(version_code("1.12.3"), 11203);
        assert_eq!(version_code("0.6.4-instable.12"), 604);
        assert_eq!(parse_u64("42"), 42);
        assert_eq!(Channel::from_name("unstable"), Channel::Unstable);
        assert_eq!(Channel::from_name("n'importe quoi"), Channel::Dev);
    }
}
