use std::{path::Path, sync::Arc};

use anyhow::Result;
use reqwest::Client;
use tokio::sync::Mutex;

use crate::reporter::DownloadReporter;

use super::{DownloadTask, Downloader};

/// A builder for convenient construction
pub struct DownloaderBuilder {
    client: Option<Client>,
    tasks: Vec<DownloadTask>,
}

impl DownloaderBuilder {
    pub fn new() -> Self {
        Self {
            client: None,
            tasks: Vec::new(),
        }
    }

    /// Uses a custom http client
    pub fn with_client(&mut self, client: Client) -> &mut Self {
        self.client = Some(client);
        self
    }

    pub fn with_timeout(self, timeout: u64) -> Self {
        todo!()
    }

    pub fn with_retries(self, retries: u8) -> Self {
        todo!()
    }

    /// Adds a download task
    pub fn add_task(
        &mut self,
        url: &str,
        output: impl AsRef<Path>,
        overwrite: bool,
        reporter: Arc<Mutex<dyn DownloadReporter>>,
    ) -> &mut Self {
        self.tasks.push(DownloadTask {
            url: url.to_string(),
            output: output.as_ref().to_path_buf(),
            overwrite,
            reporter,
        });
        self
    }

    /// Creates a downloader with URLs validation
    pub fn build(self) -> Result<(Downloader, Vec<anyhow::Error>)> {
        let mut errors = Vec::new();
        let mut valid_tasks = Vec::new();

        for task in self.tasks {
            if Downloader::is_valid_url(&task.url) {
                valid_tasks.push(task);
            } else {
                errors.push(anyhow::anyhow!("Invalid URL: {}", task.url));
            }
        }

        if valid_tasks.is_empty() && errors.is_empty() {
            return Err(anyhow::anyhow!("No download tasks provided"));
        }

        let client = self.client.unwrap_or_else(Client::new);
        let downloader = Downloader {
            tasks: valid_tasks,
            client,
        };

        Ok((downloader, errors))
    }
}
