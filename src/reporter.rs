use std::{path::Path, sync::Arc, time::Duration};

use anyhow::{Context, Result};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use reqwest::Response;

pub trait DownloadReporter: Send + Sync {
    fn on_request(&mut self, url: &str);
    fn on_response(&mut self, response: &Response);
    fn on_file_exists(&mut self, path: &Path, overwrite: bool);
    fn on_file_create(&mut self, path: &Path);
    fn on_file_size_known(&mut self, size: Option<u64>);
    fn on_start_download(&mut self, url: &str, file: &Path);
    fn on_progress(&mut self, delta: u64);
    fn on_complete(&mut self, url: &str, path: &Path);
    fn on_error(&mut self, error: &anyhow::Error);
}

#[derive(Debug, Clone)]
pub struct ConsoleReporterFactory {
    multi_progress: MultiProgress,
}

impl ConsoleReporterFactory {
    pub fn new() -> Self {
        Self {
            multi_progress: MultiProgress::new(),
        }
    }

    pub fn create(&self) -> ConsoleReporter {
        ConsoleReporter::new(self.multi_progress.clone())
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
        pb.set_message(file.file_name().unwrap().to_string_lossy().to_string());

        if let Some(size) = self.file_size {
            pb.set_length(size);
            pb.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} {msg} [{elapsed_precise}] {bar:40.cyan/blue} {bytes}/{total_bytes} ({eta})",
                ).unwrap()
                .progress_chars("▓ ░"),
        );
        } else {
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("{spinner:.green} {msg} [{elapsed_precise}] {bytes} ({bytes_per_sec})")
                    .unwrap(),
            );
        }
        self.progress_bar = Some(pb);
    }
}
