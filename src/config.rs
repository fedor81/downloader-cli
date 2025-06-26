use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use directories::{BaseDirs, ProjectDirs};

pub mod app;
mod cli;

pub use app::LogLevel;
pub use cli::{CliConfig, IntoOverwrite};

use crate::config::app::{AppConfig, TomlConfig};

pub trait Config
where
    Self: Sized,
{
    fn load() -> Result<Self>;
    fn load_from_path<P: AsRef<Path>>(config_path: P) -> Result<Self>;
}

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

/// Loads config from the path in arguments or first location found.
/// Return the default config if the file is not found.
pub fn load_config(args: &CliConfig) -> anyhow::Result<AppConfig> {
    let mut toml_config = if let Some(config_path) = &args.config {
        TomlConfig::load_from_path(config_path)?
    } else {
        TomlConfig::load()?
    };

    if let Some(another_config) = toml_config.general.config_path {
        toml_config = TomlConfig::load_from_path(another_config)?;
    }

    args.into_overwrite(&mut toml_config);
    Ok(AppConfig::from(toml_config))
}

/// Loads config from the first location found.
/// Return the default config if the file is not found.
fn load_config_internal<T>() -> Result<T>
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
    P: AsRef<Path>,
{
    let config_str = std::fs::read_to_string(&config_path).context("Failed to read config")?;

    match toml::from_str(&config_str) {
        Ok(config) => Ok(config),
        Err(err) => Err(anyhow::anyhow!("Failed to parse config: {}", err)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_path() {
        println!("{:#?}", get_config_paths());
    }
}
