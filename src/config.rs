use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use directories::{BaseDirs, ProjectDirs};
use serde::Deserialize;

mod cli;
pub use cli::{CliConfig, IntoOverwrite};

use crate::builder::{DownloaderBuilder, build_client};

pub const MAX_PARALLELS_REQUESTS: usize = 5;
pub const RETRIES: usize = 3;

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct AppConfig {
    #[serde(default)]
    pub general: GeneralConfig,

    #[serde(default)]
    pub download: DownloadConfig,

    #[serde(default)]
    pub progress_bar: ProgressBarConfig,

    #[serde(default)]
    pub output: OutputConfig,
}

impl AppConfig {
    /// It attempts to load the configuration from the default locations.
    /// If it fails, it returns the default configuration.
    pub fn load() -> Result<Self> {
        let config: AppConfig = load_config()?;
        config.validate()
    }

    pub fn load_from_path<P>(config_path: P) -> Result<Self>
    where
        P: AsRef<Path> + std::fmt::Debug,
    {
        let config: AppConfig = load_config_from_path(config_path)?;
        config.validate()
    }

    fn validate(self) -> Result<Self> {
        Ok(self) // TODO
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct GeneralConfig {
    #[serde(default)]
    pub log_level: LogLevel,

    #[serde(default)]
    pub config_path: Option<PathBuf>,
}

// TODO: redirects, gzip, user_agent, http2, proxy, cookies
#[derive(Debug, Deserialize)]
pub struct DownloadConfig {
    #[serde(default = "DownloadConfig::default_timeout")]
    pub timeout_secs: u64,

    #[serde(default = "DownloadConfig::default_connect_timeout")]
    pub connect_timeout_secs: u64,

    #[serde(default = "DownloadConfig::default_retries")]
    pub retries: usize,

    #[serde(default = "DownloadConfig::default_parallel_requests")]
    pub parallel_requests: usize,

    #[serde(default)]
    pub download_dir: Option<PathBuf>,
}

impl DownloadConfig {
    #[rustfmt::skip]
    fn default_timeout() -> u64 { 30 }

    #[rustfmt::skip]
    fn default_retries() -> usize { RETRIES }

    #[rustfmt::skip]
    fn default_connect_timeout() -> u64 { 5 }

    #[rustfmt::skip]
    fn default_parallel_requests() -> usize { MAX_PARALLELS_REQUESTS }
}

impl Default for DownloadConfig {
    fn default() -> Self {
        Self {
            timeout_secs: Self::default_timeout(),
            retries: Self::default_retries(),
            download_dir: Default::default(),
            connect_timeout_secs: Self::default_connect_timeout(),
            parallel_requests: Self::default_parallel_requests(),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct ProgressBarConfig {
    #[serde(default = "default_true")]
    pub enable: bool,

    #[serde(default = "default_true")]
    pub random: bool,

    #[serde(default = "ProgressBarConfig::default_progress_bar_templates")]
    pub progress_bar_templates: Vec<String>,

    #[serde(default = "ProgressBarConfig::default_progress_bar_chars")]
    pub progress_bar_chars: Vec<String>,

    #[serde(default = "ProgressBarConfig::default_spinner_templates")]
    pub spinner_templates: Vec<String>,

    #[serde(default = "ProgressBarConfig::default_spinner_chars")]
    pub spinner_chars: Vec<String>,
}

impl ProgressBarConfig {
    pub fn default_progress_bar_templates() -> Vec<String> {
        vec!["[{elapsed_precise}] {msg:20} {bar:40.cyan/blue} {bytes}/{total_bytes} ({eta})".to_string()]
    }

    pub fn default_progress_bar_chars() -> Vec<String> {
        vec!["▓ ░".to_string()]
    }

    pub fn default_spinner_templates() -> Vec<String> {
        vec!["{spinner:.green} [{elapsed_precise}] {msg:20} {bytes} ({bytes_per_sec})".to_string()]
    }

    pub fn default_spinner_chars() -> Vec<String> {
        vec!["⠁⠂⠄⡀⢀⠠⠐⠈⠘⠰⠔⠑⠊ ".to_string()] // ▉▊▋▌▍▎▏▎▍▌▋▊▉ 🕐🕑🕒🕓🕔🕕🕖🕗🕘🕙🕚🕛
    }
}

impl Default for ProgressBarConfig {
    fn default() -> Self {
        Self {
            enable: default_true(),
            random: default_true(),
            progress_bar_templates: Self::default_progress_bar_templates(),
            progress_bar_chars: Self::default_progress_bar_chars(),
            spinner_templates: Self::default_spinner_templates(),
            spinner_chars: Self::default_spinner_chars(),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct OutputConfig {
    #[serde(default)]
    pub message_on_request: Option<String>,

    #[serde(default)]
    pub message_on_response: Option<String>,

    #[serde(default)]
    pub message_on_file_exists: Option<String>,

    #[serde(default)]
    pub message_on_file_create: Option<String>,

    #[serde(default)]
    pub message_on_file_size_known: Option<String>,

    #[serde(default)]
    pub message_on_start_download: Option<String>,

    #[serde(default)]
    pub message_on_progress: Option<String>,

    #[serde(default)]
    pub message_on_complete: Option<String>,

    #[serde(default)]
    pub message_on_error: Option<String>,
}

impl OutputConfig {
    pub fn default_message_before_request() -> Option<String> {
        Some("Requesting information about {}".to_string())
    }
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            message_on_request: Default::default(),
            message_on_response: Default::default(),
            message_on_file_exists: Default::default(),
            message_on_file_create: Default::default(),
            message_on_file_size_known: Default::default(),
            message_on_start_download: Default::default(),
            message_on_progress: Default::default(),
            message_on_complete: Default::default(),
            message_on_error: Default::default(),
        }
    }
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    All,
    ErrorsOnly,
    ProgressBarOnly,
    Silent,
}

impl LogLevel {
    pub fn show_summary(self) -> bool {
        self == LogLevel::All
    }

    pub fn show_success(self) -> bool {
        self == LogLevel::All
    }

    pub fn show_errors(self) -> bool {
        self == LogLevel::All || self == LogLevel::ErrorsOnly
    }
}

impl Default for LogLevel {
    fn default() -> Self {
        Self::All
    }
}

#[rustfmt::skip]
fn default_true() -> bool { true }

#[rustfmt::skip]
fn default_false() -> bool { false }

/// Searches the config in several standard locations
fn find_config() -> Result<Option<PathBuf>> {
    // Checking the existence of files
    for path in get_config_paths() {
        if path.exists() {
            return Ok(Some(path));
        }
    }

    Ok(None)
}

/// Returns the configuration paths in order of check priority and depending on the operating system
///
/// 1. Path specified in `DW_CONFIG_PATH` environment variable
/// 2. User configs `~/.config/dw.toml`, `~/.config/dw/config.toml`, `~/.dw.toml`
///
/// It is possible to have multiple configs in the ~/.config/dw/ folder
fn get_config_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let mut seen = HashSet::new();

    // Auxiliary function for adding a path with uniqueness check
    let mut add_path = |path: PathBuf| {
        if seen.insert(path.clone()) {
            paths.push(path);
        }
    };

    // Override via environment variable
    if let Ok(custom_path) = std::env::var("DW_CONFIG_PATH") {
        add_path(PathBuf::from(custom_path));
    }

    // User configs
    if let Some(base_dirs) = BaseDirs::new() {
        // Windows: %APPDATA%\dw.toml
        // macOS: ~/Library/Application Support/dw.toml
        // Linux: ~/.config/dw.toml
        add_path(base_dirs.config_dir().join("dw.toml"));

        // Windows: %APPDATA%\dw\config.toml
        // macOS: ~/Library/Application Support/dw/config.toml
        // Linux: ~/.config/dw/config.toml
        if let Some(proj_dirs) = ProjectDirs::from("", "", "dw") {
            add_path(proj_dirs.config_dir().join("config.toml"));
        }
    }

    // Unix-specific paths (Linux/macOS)
    #[cfg(unix)]
    {
        if let Ok(home) = std::env::var("HOME") {
            let home_path = Path::new(&home);

            // ~/.config/dw.toml
            add_path(home_path.join(".config/dw.toml"));

            // ~/.config/dw/config.toml
            add_path(home_path.join(".config/dw/config.toml"));

            // ~/.dw.toml (резервный вариант)
            add_path(home_path.join(".dw.toml"));
        }

        // Global config (/etc/dw.toml)
        add_path(PathBuf::from("/etc/dw.toml"));
    }

    // Windows-specific paths
    #[cfg(windows)]
    {
        if let Ok(appdata) = env::var("APPDATA") {
            // %APPDATA%\dw.toml
            add_path(Path::new(&appdata).join("dw.toml"));

            // %APPDATA%\dw\config.toml
            add_path(Path::new(&appdata).join("dw").join("config.toml"));
        }

        if let Ok(localappdata) = env::var("LOCALAPPDATA") {
            // %LOCALAPPDATA%\dw.toml
            add_path(Path::new(&localappdata).join("dw.toml"));
        }
    }

    paths
}

/// Loads config from the first location found.
/// Return the default config if the file is not found.
fn load_config<T>() -> Result<T>
where
    T: serde::de::DeserializeOwned + Default,
{
    if let Some(config_path) = find_config()? {
        load_config_from_path(config_path)
    } else {
        Ok(T::default())
    }
}

/// Loads the config from the specified location
fn load_config_from_path<T, P>(config_path: P) -> Result<T>
where
    T: serde::de::DeserializeOwned,
    P: AsRef<Path> + std::fmt::Debug,
{
    let config_str = std::fs::read_to_string(&config_path)
        .with_context(|| format!("Failed to read config at {:?}", config_path))?;

    match toml::from_str(&config_str) {
        Ok(config) => Ok(config),
        Err(err) => Err(anyhow::anyhow!("Failed to parse config: {}", err)),
    }
}

impl From<&AppConfig> for DownloaderBuilder {
    fn from(value: &AppConfig) -> Self {
        let client = build_client(&value).expect("Failed to build reqwest::Client from config");
        Self::new()
            .with_parallel_requests(value.download.parallel_requests)
            .with_retries(value.download.retries)
            .with_client(client)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_path() {
        println!("{:#?}", get_config_paths());
    }

    #[test]
    fn test_custom_config_values() {
        let config_str = r#"
        [general]
        log_level = "ErrorsOnly"
        config_path = "/custom/path"

        [download]
        timeout_secs = 60
        download_dir = "/custom/path"
    "#;

        let config: AppConfig = toml::from_str(config_str).unwrap();

        assert_eq!(config.general.config_path, Some(PathBuf::from("/custom/path")));
        assert_eq!(config.general.log_level, LogLevel::ErrorsOnly);
        assert_eq!(config.download.timeout_secs, 60);
        assert_eq!(config.download.download_dir, Some("/custom/path".into()));
    }

    #[test]
    fn test_invalid_config() {
        let config = "invalid_field = 42";
        let actual = toml::from_str::<AppConfig>(config);
        assert!(actual.is_err(), "{:#?}", actual);
    }

    #[test]
    fn test_default_config() {
        let config: AppConfig = toml::from_str("").unwrap();
        println!("Config: {:#?}", config);
    }
}
