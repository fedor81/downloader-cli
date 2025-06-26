use super::{DownloadReporter, ReporterFactory};

pub struct SilentReporterFactory;

impl ReporterFactory for SilentReporterFactory {
    fn create(&self) -> Self::Reporter {
        Self::Reporter {}
    }
    type Reporter = SilentReporter;
}

pub struct SilentReporter;

impl DownloadReporter for SilentReporter {
    fn on_request(&mut self, url: &str) {}

    fn on_response(&mut self, response: &reqwest::Response) {}

    fn on_file_exists(&mut self, path: &std::path::Path, overwrite: bool) {}

    fn on_file_create(&mut self, path: &std::path::Path) {}

    fn on_file_size_known(&mut self, size: Option<u64>) {}

    fn on_start_download(&mut self, url: &str, file: &std::path::Path) {}

    fn on_progress(&mut self, delta: u64) {}

    fn on_complete(&mut self, url: &str, path: &std::path::Path) {}

    fn on_error(&mut self, error: &anyhow::Error) {}
}
