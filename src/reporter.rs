use std::path::Path;

pub mod console_reporter;
pub mod program_flow;
pub mod silent_reporter;

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

pub trait ProgramFlowReporter {
    fn on_start(&mut self);
    fn on_finish(&mut self);
    fn on_errors(&mut self, errors: Vec<anyhow::Error>);
    fn on_success(&mut self);
}
