use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Result;
use serde::Deserialize;

use super::{Config, load_config_from_path, load_config_internal};

pub const MAX_PARALLELS_REQUESTS: usize = 5;
pub const RETRIES: usize = 3;

#[derive(Debug)]
pub struct AppConfig {
    pub general: Arc<GeneralConfig>,
    pub download: Arc<DownloadConfig>,
    pub progress_bar: Arc<ProgressBarConfig>,
    pub output: Arc<OutputConfig>,
}

impl Config for AppConfig {
    fn load() -> Result<Self> {
        Ok(Self::from(TomlConfig::load()?))
    }

    fn load_from_path<P: AsRef<Path>>(config_path: P) -> Result<Self> {
        Ok(Self::from(TomlConfig::load_from_path(config_path)?))
    }
}

impl From<TomlConfig> for AppConfig {
    fn from(value: TomlConfig) -> Self {
        Self {
            general: Arc::new(value.general),
            download: Arc::new(value.download),
            progress_bar: Arc::new(value.progress_bar),
            output: Arc::new(value.output),
        }
    }
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(super) struct TomlConfig {
    #[serde(default)]
    pub general: GeneralConfig,

    #[serde(default)]
    pub download: DownloadConfig,

    #[serde(default)]
    pub progress_bar: ProgressBarConfig,

    #[serde(default)]
    pub output: OutputConfig,
}

impl AppConfig {}

impl TomlConfig {
    /// It attempts to load the configuration from the default locations.
    /// If it fails, it returns the default configuration.
    pub fn load() -> Result<Self> {
        let config: Self = load_config_internal()?;
        config.validate()
    }

    pub fn load_from_path<P>(config_path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let config: Self = load_config_from_path(config_path)?;
        config.validate()
    }

    fn validate(self) -> Result<Self> {
        Ok(self) // TODO
    }
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct GeneralConfig {
    #[serde(default)]
    pub log_level: LogLevel,

    #[serde(default)]
    pub config_path: Option<PathBuf>,
}

// TODO: redirects, gzip, user_agent, http2, proxy, cookies
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
pub struct ProgressBarConfig {
    #[serde(default = "default_true")]
    pub enable: bool,

    #[serde(default = "ProgressBarConfig::default_max_displayed_filename")]
    pub max_displayed_filename: usize,

    #[serde(default = "ProgressBarConfig::default_progress_bar_templates")]
    pub progress_bar_templates: Vec<String>,

    #[serde(default = "ProgressBarConfig::default_progress_bar_chars")]
    pub progress_bar_chars: Vec<String>,

    #[serde(default = "ProgressBarConfig::default_spinner_templates")]
    pub spinner_templates: Vec<String>,

    #[serde(default = "ProgressBarConfig::default_spinner_chars")]
    pub spinner_chars: Vec<String>,

    #[serde(default = "ProgressBarConfig::default_request_spinner_templates")]
    pub request_spinner_templates: Vec<String>,

    #[serde(default = "ProgressBarConfig::default_spinner_chars")]
    pub request_spinner_chars: Vec<String>,
}

impl ProgressBarConfig {
    #[rustfmt::skip]
    pub fn default_max_displayed_filename() -> usize { 20 }

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

    pub fn default_request_spinner_templates() -> Vec<String> {
        vec!["{spinner} {msg}".to_string()]
    }
}

impl Default for ProgressBarConfig {
    fn default() -> Self {
        Self {
            enable: default_true(),
            progress_bar_templates: Self::default_progress_bar_templates(),
            progress_bar_chars: Self::default_progress_bar_chars(),
            spinner_templates: Self::default_spinner_templates(),
            spinner_chars: Self::default_spinner_chars(),
            request_spinner_templates: Self::default_request_spinner_templates(),
            request_spinner_chars: Self::default_spinner_chars(),
            max_displayed_filename: Self::default_max_displayed_filename(),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct OutputConfig {
    #[serde(default)]
    pub message_on_start: Option<String>,

    #[serde(default)]
    pub message_on_errors: Option<String>,

    #[serde(default = "OutputConfig::default_message_on_success")]
    pub message_on_success: Option<String>,

    #[serde(default = "OutputConfig::default_message_on_finish")]
    pub message_on_finish: Option<String>,

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
}

impl OutputConfig {
    pub fn default_message_before_request() -> Option<String> {
        Some("Requesting information about {}".to_string())
    }

    pub fn default_message_on_finish() -> Option<String> {
        Some("Finish!".to_owned())
    }

    fn default_message_on_success() -> Option<String> {
        Some("\nAll files downloaded successfully!".to_owned())
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
            message_on_success: Self::default_message_on_success(),
            message_on_start: Default::default(),
            message_on_errors: Default::default(),
            message_on_finish: Self::default_message_on_finish(),
        }
    }
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum ProgressBarType {
    Spinner,
    ProgressBar,
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

#[cfg(test)]
mod tests {
    use super::*;

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

        let config: TomlConfig = toml::from_str(config_str).unwrap();

        assert_eq!(config.general.config_path, Some(PathBuf::from("/custom/path")));
        assert_eq!(config.general.log_level, LogLevel::ErrorsOnly);
        assert_eq!(config.download.timeout_secs, 60);
        assert_eq!(config.download.download_dir, Some("/custom/path".into()));
    }

    #[test]
    fn test_invalid_config() {
        let config = "invalid_field = 42";
        let actual = toml::from_str::<TomlConfig>(config);
        assert!(actual.is_err(), "{:#?}", actual);
    }

    #[test]
    fn test_default_config() {
        let config: TomlConfig = toml::from_str("").unwrap();
        println!("Config: {:#?}", config);
    }
}
