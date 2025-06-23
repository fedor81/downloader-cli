use std::{path::Path, time::Duration};

use anyhow::{Context, Result};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use reqwest::Response;

use super::{DownloadReporter, ReporterFactory};
use crate::config::AppConfig;

#[derive(Debug, Clone)]
pub struct ConsoleReporterFactory {
    multi_progress: MultiProgress,
}

impl ReporterFactory for ConsoleReporterFactory {
    fn create(&self) -> Self::Reporter {
        ConsoleReporter::new(self.multi_progress.clone())
    }

    type Reporter = ConsoleReporter;
}

impl ConsoleReporterFactory {
    pub fn new() -> Self {
        Self {
            multi_progress: MultiProgress::new(),
        }
    }

    pub fn from_config(config: &AppConfig) -> Self {
        Self {
            multi_progress: MultiProgress::new(),
        }
    }
}

#[derive(Debug)]
pub struct ConsoleReporter {
    multi_progress: MultiProgress,
    progress_bar: Option<ProgressBar>,
    file_size: Option<u64>,
}

impl ConsoleReporter {
    fn new(multi_progress: MultiProgress) -> Self {
        Self {
            multi_progress,
            progress_bar: None,
            file_size: None,
        }
    }

    fn shorten_filename(file: &Path) -> String {
        let max_length = 20; // TODO: Move to config
        let name = file.file_name().unwrap().to_string_lossy().to_string();

        if name.len() <= max_length {
            name
        } else {
            let extension = file.extension().unwrap_or_default().to_string_lossy().to_string();
            format!("{}…{}", &name[0..max_length - extension.len() - 1], extension)
        }
    }
}

impl DownloadReporter for ConsoleReporter {
    /// Create progress bar for request
    fn on_request(&mut self, url: &str) {
        let pb = self
            .multi_progress
            .add(ProgressBar::new_spinner().with_message(format!("Requesting information about {}", url)));
        pb.enable_steady_tick(Duration::from_millis(100));
        self.progress_bar.replace(pb);
    }

    fn on_response(&mut self, response: &Response) {
        if let Some(pb) = &self.progress_bar {
            pb.finish_and_clear()
        }
    }

    fn on_file_exists(&mut self, path: &Path, overwrite: bool) {
        if !overwrite {
            println!("File exists: {}. See '--help' for solutions.", path.display());
        }
    }

    fn on_complete(&mut self, url: &str, path: &Path) {
        if let Some(pb) = &self.progress_bar {
            pb.finish();
            self.progress_bar = None
        }
    }

    fn on_error(&mut self, error: &anyhow::Error) {
        println!("{}", error);
    }

    fn on_file_size_known(&mut self, size: Option<u64>) {
        self.file_size = size;
        if let Some(size) = size {
            // println!("Size: {}", indicatif::HumanBytes(size));
        }
    }

    fn on_file_create(&mut self, path: &Path) {
        // println!("Saving as: {}", path.display());
    }

    /// Update progress bar
    fn on_progress(&mut self, delta: u64) {
        if let Some(pb) = &self.progress_bar {
            pb.inc(delta);
        }
    }

    /// Setup progress bar for download
    fn on_start_download(&mut self, url: &str, file: &Path) {
        let pb = self.multi_progress.add(ProgressBar::no_length());
        pb.set_message(Self::shorten_filename(file));

        if let Some(size) = self.file_size {
            pb.set_length(size);
            pb.set_style(
                ProgressStyle::default_bar()
                    // .template("{msg} [{elapsed_precise}] {bar:40.cyan/blue} {bytes}/{total_bytes} ({eta})")
                    .template(
                        "{msg:20} [{elapsed_precise}] {bar:40.cyan/blue} {bytes:>8}/{total_bytes:8} ({eta})",
                    )
                    .unwrap()
                    .progress_chars("▓ ░"),
            );
        } else {
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("{spinner:.green} {msg} [{elapsed_precise}] {bytes} ({bytes_per_sec})")
                    .unwrap()
                    .progress_chars("⠁⠂⠄⡀⢀⠠⠐⠈⠘⠰⠔⠑⠊ "),
            );
        }
        self.progress_bar = Some(pb);
    }
}
