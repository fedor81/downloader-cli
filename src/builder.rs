use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use anyhow::Result;
use reqwest::{Client, ClientBuilder};
use tokio::sync::{Mutex, Semaphore};

use crate::{
    config::{AppConfig, MAX_PARALLELS_REQUESTS, RETRIES},
    reporter::DownloadReporter,
};

use super::{DownloadTask, Downloader};

/// A builder for convenient construction
pub struct DownloaderBuilder {
    client: Option<Client>,
    tasks: Vec<DownloadTask>,
    retries: usize, // TODO
    parallel_requests: usize,
}

impl DownloaderBuilder {
    pub fn new() -> Self {
        Self {
            client: None,
            tasks: Vec::new(),
            retries: RETRIES,
            parallel_requests: MAX_PARALLELS_REQUESTS,
        }
    }

    /// Uses a custom http client
    pub fn with_client(mut self, client: Client) -> Self {
        self.client = Some(client);
        self
    }

    pub fn with_retries(mut self, retries: usize) -> Self {
        self.retries = retries;
        self
    }

    pub fn with_parallel_requests(mut self, count: usize) -> Self {
        self.parallel_requests = count;
        self
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

    /// Adds multiple tasks from the iterator
    pub fn add_tasks<I>(&mut self, tasks: I) -> &mut Self
    where
        I: IntoIterator<Item = (String, PathBuf, bool, Arc<Mutex<dyn DownloadReporter>>)>,
    {
        for (url, output, overwrite, reporter) in tasks {
            self.add_task(&url, output, overwrite, reporter);
        }
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
            parallel_requests: Arc::new(Semaphore::new(self.parallel_requests)),
        };

        Ok((downloader, errors))
    }
}

pub fn build_client(config: &AppConfig) -> Result<Client> {
    let builder = ClientBuilder::new();
    Ok(builder
        .timeout(Duration::from_secs(config.download.timeout_secs))
        .connect_timeout(Duration::from_secs(config.download.connect_timeout_secs))
        .build()?)
}
