use std::{path::Path, sync::Arc, time::Duration};

use anyhow::{Context, Result};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rand::{self, seq::IndexedRandom};
use reqwest::Response;

use super::{DownloadReporter, ReporterFactory};
use crate::config::app::{OutputConfig, ProgressBarConfig};

#[derive(Clone)]
pub struct ConsoleReporterFactory {
    multi_progress: MultiProgress,
    progress_config: Arc<ProgressBarConfig>,
    output_config: Arc<OutputConfig>,
}

impl ReporterFactory for ConsoleReporterFactory {
    fn create(&self) -> Self::Reporter {
        let mut rng = rand::rng();

        ConsoleReporter::new(
            self.multi_progress.clone(),
            self.progress_config.max_displayed_filename,
            Self::choose_or_empty(&self.progress_config.progress_bar_templates, &mut rng),
            Self::choose_or_empty(&self.progress_config.progress_bar_chars, &mut rng),
            Self::choose_or_empty(&self.progress_config.spinner_templates, &mut rng),
            Self::choose_or_empty(&self.progress_config.spinner_chars, &mut rng),
            Self::choose_or_empty(&self.progress_config.request_spinner_templates, &mut rng),
            Self::choose_or_empty(&self.progress_config.request_spinner_chars, &mut rng),
            self.output_config.clone(),
        )
    }

    type Reporter = ConsoleReporter;
}

impl ConsoleReporterFactory {
    pub fn new(progress_config: &ProgressBarConfig, output_config: &OutputConfig) -> Self {
        Self {
            multi_progress: MultiProgress::new(),
            progress_config: Arc::new(progress_config.clone()),
            output_config: Arc::new(output_config.clone()),
        }
    }

    fn choose_or_empty<T: rand::Rng>(choices: &[String], rng: &mut T) -> Arc<str> {
        Arc::from(choices.choose(rng).unwrap_or(&"".to_string()).as_str())
    }
}

#[derive(Debug)]
pub struct ConsoleReporter {
    multi_progress: MultiProgress,
    progress_bar: Option<ProgressBar>,
    file_size: Option<u64>,
    max_displayed_filename: usize,
    output_config: Arc<OutputConfig>,

    // Templates and chars
    progress_bar_template: Arc<str>,
    progress_bar_chars: Arc<str>,
    spinner_template: Arc<str>,
    spinner_chars: Arc<str>,
    request_spinner_template: Arc<str>,
    request_spinner_chars: Arc<str>,
}

impl ConsoleReporter {
    fn new(
        multi_progress: MultiProgress,
        max_displayed_filename: usize,
        progress_bar_template: Arc<str>,
        progress_bar_chars: Arc<str>,
        spinner_template: Arc<str>,
        spinner_chars: Arc<str>,
        request_spinner_template: Arc<str>,
        request_spinner_chars: Arc<str>,
        output_config: Arc<OutputConfig>,
    ) -> Self {
        Self {
            multi_progress,
            max_displayed_filename,
            progress_bar: None,
            file_size: None,
            progress_bar_template,
            progress_bar_chars,
            spinner_template,
            spinner_chars,
            output_config,
            request_spinner_template,
            request_spinner_chars,
        }
    }

    fn println(message: &Option<String>) {
        if let Some(message) = message {
            println!("{}", message);
        }
    }

    fn shorten_filename(&self, file: &Path) -> String {
        let name = file.file_name().unwrap().to_string_lossy().to_string();

        if name.len() <= self.max_displayed_filename {
            name
        } else {
            let extension = file.extension().unwrap_or_default().to_string_lossy().to_string();
            format!(
                "{}…{}",
                &name[0..self.max_displayed_filename - extension.len() - 1],
                extension
            )
        }
    }
}

impl DownloadReporter for ConsoleReporter {
    /// Create progress bar for request
    fn on_request(&mut self, url: &str) {
        let pb = self.multi_progress.add(
            ProgressBar::new_spinner()
                .with_style(
                    // TODO: Save styles in struct ??
                    ProgressStyle::with_template(&self.request_spinner_template)
                        .unwrap()
                        .tick_chars(&self.request_spinner_chars),
                )
                .with_message(format!("Requesting information about {}", url)),
        );
        pb.enable_steady_tick(Duration::from_millis(100));
        self.progress_bar.replace(pb);
    }

    fn on_response(&mut self, response: &Response) {
        if let Some(pb) = &self.progress_bar {
            pb.finish_and_clear()
        }
        Self::println(&self.output_config.message_on_response);
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
        let pb = if let Some(size) = self.file_size {
            ProgressBar::new(size).with_style(
                ProgressStyle::with_template(&self.progress_bar_template)
                    .unwrap()
                    .progress_chars(&self.progress_bar_chars),
            )
        } else {
            ProgressBar::new_spinner().with_style(
                ProgressStyle::with_template(&self.spinner_template)
                    .unwrap()
                    .tick_chars(&self.spinner_chars),
            )
        }
        .with_message(self.shorten_filename(file));

        let pb = self.multi_progress.add(pb);
        self.progress_bar = Some(pb);
    }
}
