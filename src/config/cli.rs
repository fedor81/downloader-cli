use clap::Parser;

use crate::config::{AppConfig, LogLevel};

/// # Important
/// It is important to avoid adding the same boolean type fields to both
/// the Cli and App Configs, as this can cause problems when merging them.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct CliConfig {
    /// URL
    pub source: String,

    /// Target filepath (existing directories will be treated as the target location)
    pub target: Option<String>,

    /// Silent mode
    #[arg(short, long)]
    pub silent: bool,

    /// Resume failed or cancelled download (partial sanity check)
    #[arg(short, long)]
    pub resume: bool,

    /// Uses the config specified in the argument
    #[arg(long)]
    pub config: Option<String>,

    /// Overwrite if the file already exists
    #[arg(short, long)]
    pub force: bool,
    //
    //
    // UI
    //
}

pub trait IntoOverwrite<T> {
    /// Overwrites the `target` fields with values from `self` (where they are set).
    /// Returns `&mutT` for chained calls
    fn into_overwrite<'a, 'b>(&'a self, target: &'b mut T) -> &'b mut T;
}

impl IntoOverwrite<AppConfig> for CliConfig {
    fn into_overwrite<'a, 'b>(&'a self, target: &'b mut AppConfig) -> &'b mut AppConfig {
        if self.silent {
            target.general.log_level = LogLevel::Silent;
        }

        target
    }
}
