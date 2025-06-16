use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use directories::{BaseDirs, ProjectDirs};
use serde::Deserialize;

// It is possible to have multiple configs in the ~/.config/dw/ folder

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub general: GeneralConfig,

    #[serde(default)]
    pub download: DownloadConfig,

    #[serde(default)]
    pub progress_bar: ProgressBarConfig,

    #[serde(default)]
    pub output: OutputConfig,
}

#[derive(Debug, Deserialize, Default)]
pub struct GeneralConfig {
    #[serde(default)]
    pub silent: bool,

    #[serde(default)]
    pub log_level: LogLevel,

    #[serde(default)]
    pub config_path: Option<PathBuf>, // TODO

    #[serde(default)]
    pub force: bool,
}

#[derive(Debug, Deserialize)]
pub struct DownloadConfig {
    #[serde(default = "DownloadConfig::default_timeout")]
    pub timeout_secs: u64,

    #[serde(default = "DownloadConfig::default_retries")]
    pub retries: u8,

    #[serde(default)]
    pub default_dir: Option<PathBuf>,

    #[serde(default = "default_true")]
    pub resume: bool,
}

impl DownloadConfig {
    #[rustfmt::skip]
    fn default_timeout() -> u64 { 30 }

    #[rustfmt::skip]
    fn default_retries() -> u8 { 3 }
}

impl Default for DownloadConfig {
    fn default() -> Self {
        Self {
            timeout_secs: Self::default_timeout(),
            retries: Self::default_retries(),
            default_dir: Default::default(),
            resume: default_true(),
        }
    }
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    All,
    ErrorsOnly,
    ProgressBarOnly,
    // Any other levels
}

impl Default for LogLevel {
    fn default() -> Self {
        Self::All
    }
}

#[derive(Debug, Deserialize)]
pub struct ProgressBarConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,

    #[serde(default)]
    pub progress_bar_symbols: Option<Vec<String>>,

    #[serde(default)]
    pub progress_bar_template: Option<String>,

    #[serde(default)]
    pub spinner_symbols: Option<Vec<String>>,
}

impl Default for ProgressBarConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            progress_bar_template: Default::default(),
            spinner_symbols: Default::default(),
            progress_bar_symbols: Default::default(),
        }
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct OutputConfig {
    #[serde(default)]
    pub message_before_start: Option<String>,

    #[serde(default)]
    pub message_after_success: Option<String>,

    #[serde(default)]
    pub message_after_error: Option<String>,

    #[serde(default)]
    pub message_after_completion: Option<String>,
}

#[rustfmt::skip]
fn default_true() -> bool { true }

/// Searches the config in several standard locations
pub fn find_config() -> Result<Option<PathBuf>> {
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

/// Loads config from the first location found
pub fn load_config<T>() -> Result<T>
where
    T: serde::de::DeserializeOwned + Default,
{
    if let Some(config_path) = find_config()? {
        load_config_from_path(config_path)
    } else {
        Ok(T::default()) // Return the default config if the file is not found
    }
}

/// Loads the config from the specified location
pub fn load_config_from_path<T, P>(config_path: P) -> Result<T>
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_custom_config_values() {
        let config_str = r#"
        [general]
        silent = true
        log_level = "ErrorsOnly"

        [download]
        timeout_secs = 60
        default_dir = "/custom/path"
    "#;

        let config: Config = toml::from_str(config_str).unwrap();

        assert!(config.general.silent);
        assert_eq!(config.general.log_level, LogLevel::ErrorsOnly);
        assert_eq!(config.download.timeout_secs, 60);
        assert_eq!(
            config.download.default_dir,
            Some(PathBuf::from("/custom/path"))
        );
        assert_eq!(config.download.retries, 3);
    }

    #[test]
    fn test_invalid_config() {
        let config = "invalid_field = 42";
        let actual = toml::from_str::<Config>(config);
        assert!(actual.is_err(), "{:#?}", actual);
    }

    #[test]
    fn test_default_config() {
        let config: Config = toml::from_str("").unwrap();
        println!("Config: {:#?}", config);
    }

    #[test]
    fn test_default_path() {
        println!("{:#?}", get_config_paths());
    }
}
