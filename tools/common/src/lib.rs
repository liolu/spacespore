pub const VERSION: &str = "0.5.0";
pub const VERSION_CODE: u32 = 500;
pub const UPDATER_VERSION: u32 = 1;

pub const REPO_OWNER: &str = "liolu";
pub const REPO_NAME: &str = "spacespore";
pub const VERSION_URL: &str = "https://liolu.github.io/spacespore/version.json";

#[derive(serde::Deserialize, Debug, Clone)]
pub struct VersionInfo {
    pub version: String,
    pub version_code: u32,
    pub download_url: String,
    pub release_notes: String,
    pub min_updater_version: u32,
}

pub fn needs_update(remote_code: u32) -> bool {
    remote_code > VERSION_CODE
}
