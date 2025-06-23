use std::path::Path;

pub use console_reporter::{ConsoleReporter, ConsoleReporterFactory};

mod console_reporter;
mod silent_reporter;

pub trait DownloadReporter: Send + Sync {
    fn on_request(&mut self, url: &str);
    fn on_response(&mut self, response: &reqwest::Response);
    fn on_file_exists(&mut self, path: &Path, overwrite: bool);
    fn on_file_create(&mut self, path: &Path);
    fn on_file_size_known(&mut self, size: Option<u64>);
    fn on_start_download(&mut self, url: &str, file: &Path);
    fn on_progress(&mut self, delta: u64);
    fn on_complete(&mut self, url: &str, path: &Path);
    fn on_error(&mut self, error: &anyhow::Error);
}

pub trait ReporterFactory {
    type Reporter: DownloadReporter;
    fn create(&self) -> Self::Reporter;
}
