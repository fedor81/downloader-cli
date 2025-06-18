use anyhow::{Context, Result};
use futures::StreamExt;
use regex::Regex;
use reqwest::{self, Client, Response};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;

pub use builder::DownloaderBuilder;

use crate::reporter::DownloadReporter;
mod builder;
pub mod config;
pub mod reporter;

pub struct Downloader {
    tasks: Vec<DownloadTask>,
    client: Client,
}

#[derive(Clone)]
pub struct DownloadTask {
    pub url: String,
    pub output: PathBuf,
    pub overwrite: bool,
    pub reporter: ThreadReporter,
}

pub type ThreadReporter = Arc<tokio::sync::Mutex<dyn DownloadReporter + Send>>;

impl Downloader {
    /// Creates a new downloader
    pub fn new(client: Client) -> Self {
        Self {
            tasks: Vec::new(),
            client,
        }
    }

    pub fn builder() -> DownloaderBuilder {
        DownloaderBuilder::new()
    }

    /// Add a download task
    pub fn add_task(&mut self, task: DownloadTask) {
        self.tasks.push(task);
    }

    pub fn task_count(&self) -> usize {
        self.tasks.len()
    }

    /// Downloads files with resume support
    pub async fn resume_download(&self) -> Vec<anyhow::Error> {
        todo!()
    }

    /// Downloads files asynchronously
    pub async fn download_all(&self) -> Vec<anyhow::Error> {
        let mut handles = tokio::task::JoinSet::new();
        let mut errors = Vec::new();

        for task in &self.tasks {
            let client = self.client.clone();
            let task = task.clone();
            handles.spawn(async move { Self::download_file(&client, &task).await });
        }

        while let Some(res) = handles.join_next().await {
            match res {
                Ok(Ok(_)) => {}
                Ok(Err(e)) => errors.push(e),
                Err(join_err) => errors.push(anyhow::anyhow!("Task failed: {}", join_err)),
            }
        }

        errors
    }

    fn validate_urls(urls: Vec<String>) -> (Vec<String>, Vec<anyhow::Error>) {
        let mut valid_urls = vec![];
        let mut errors = vec![];

        for url_string in urls {
            if Self::is_valid_url(&url_string) {
                valid_urls.push(url_string);
            } else {
                errors.push(anyhow::anyhow!("Can't parse url: {}", url_string));
            }
        }

        (valid_urls, errors)
    }

    pub fn is_valid_url(url: &str) -> bool {
        reqwest::Url::parse(url).is_ok()
    }

    async fn download_file(client: &Client, task: &DownloadTask) -> Result<()> {
        // Preparation
        {
            let mut reporter = task.reporter.lock().await;
            reporter.on_request(&task.url);
        }

        // Sending a request
        let response = match client
            .get(&task.url)
            .send()
            .await
            .with_context(|| format!("Failed to GET: '{}'", &task.url))
        {
            Ok(response) => {
                task.reporter.lock().await.on_response(&response);
                response
            }
            Err(e) => {
                task.reporter.lock().await.on_error(&e);
                return Err(e);
            }
        };

        // Checking the response status
        if !response.status().is_success() {
            let err = anyhow::anyhow!("Request {} failed with status: {}", &task.url, response.status());
            task.reporter.lock().await.on_error(&err);
            return Err(err);
        }

        // Get file size from Content-Length header (if any)
        let total_size = response
            .headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|ct_len| ct_len.to_str().ok())
            .and_then(|ct_len| ct_len.parse::<u64>().ok());

        task.reporter.lock().await.on_file_size_known(total_size);

        if Self::handle_existing_file(task).await? {
            return Err(anyhow::anyhow!("File exists: {}", task.output.display())
                .context("Use --overwrite to replace existing files"));
        }

        // Download
        Self::download_stream(task, response).await?;
        task.reporter.lock().await.on_complete(&task.url, &task.output);
        Ok(())
    }

    /// Creates a new file and downloads the stream by calling callbacks
    async fn download_stream(task: &DownloadTask, response: Response) -> Result<()> {
        let file = tokio::fs::File::create(&task.output)
            .await
            .with_context(|| format!("Failed to create file: {}", &task.output.display()))?;
        let mut writer = tokio::io::BufWriter::new(file);
        task.reporter.lock().await.on_file_create(&task.output);

        // Get the data stream from the response
        let mut stream = response.bytes_stream();
        task.reporter.lock().await.on_start_download();

        // Read the stream and write it to a file
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.with_context(|| "Failed to read response chunk")?;
            writer.write_all(&chunk).await?;
            task.reporter.lock().await.on_progress(chunk.len() as u64);
        }

        writer.flush().await?;
        Ok(())
    }

    /// Checks the existence of a file and whether it can be written to.
    ///
    /// Returns `false` if the file exists and can be overwritten, and `true` otherwise.
    async fn handle_existing_file(task: &DownloadTask) -> Result<bool> {
        let output_display = task.output.display();
        Ok(
            if tokio::fs::try_exists(&task.output)
                .await
                .with_context(|| format!("Failed to check file existence: {}", task.output.display()))?
            {
                let mut reporter = task.reporter.lock().await;
                reporter.on_file_exists(&task.output, task.overwrite);

                if task.overwrite {
                    tokio::fs::remove_file(&task.output).await.with_context(|| {
                        format!("Failed to remove existing file: {}", task.output.display())
                    })?;
                    false
                } else {
                    true
                }
            } else {
                false
            },
        )
    }

    /// Try to get the filename from the URL
    fn sanitize_filename(url: &str) -> String {
        const MAX_FILENAME_LENGTH: usize = 100;

        // Remove query and anchor parameters
        let re_params = Regex::new(r"[?#].*$").unwrap();
        let clean_url = re_params.replace(url, "");

        // Extract the last component of the path
        let mut base = clean_url.split('/').last().unwrap_or("temp");
        let re_special: Regex;

        if base.is_empty() {
            // Handling URLs ending in /
            base = url.split("://").nth(1).unwrap_or("temp");
            re_special = Regex::new(r"[^a-zA-Z0-9_]+").unwrap();
        } else {
            re_special = Regex::new(r"[^a-zA-Z0-9\_.]+").unwrap();
        }

        re_special
            .replace_all(base, "_")
            .trim_matches('_')
            .chars()
            .take(MAX_FILENAME_LENGTH)
            .collect::<String>()
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{Arc, Mutex},
        time::Duration,
    };

    use bytes::Bytes;
    use rand::{Rng, SeedableRng, rngs::StdRng};
    use warp::Filter;

    use crate::reporter::ConsoleReporter;

    use super::*;

    #[test]
    fn test_get_filename() {
        assert_eq!(
            Downloader::sanitize_filename("https://example.com/file.txt"),
            "file.txt"
        );
        assert_eq!(
            Downloader::sanitize_filename("https://example.com/file.txt?param=value"),
            "file.txt"
        );
        assert_eq!(
            Downloader::sanitize_filename("https://example.com/file.txt#fragment"),
            "file.txt"
        );
        assert_eq!(
            Downloader::sanitize_filename("https://example.com/file.txt?param=value#fragment"),
            "file.txt"
        );
        assert_eq!(
            Downloader::sanitize_filename("https://example.com/"),
            "example_com"
        );
        assert_eq!(
            Downloader::sanitize_filename("https://example.com/page/1/"),
            "example_com_page_1"
        );
        assert_eq!(
            Downloader::sanitize_filename("https://example.com/page/1/?param=value#fragment"),
            "example_com_page_1_param_value_fragment"
        );
    }

    fn create_realistic_stream(
        content: &'static [u8],
        base_chunk_size: usize,
    ) -> impl futures::Stream<Item = Result<Bytes, std::io::Error>> + 'static {
        let rng = Arc::new(Mutex::new(StdRng::from_os_rng()));

        futures::stream::iter(content.chunks(base_chunk_size).map(move |chunk| {
            let rng = Arc::clone(&rng);

            async move {
                // Get random delay parameters
                let (delay_ms, extra_delay) = {
                    let mut rng = rng.lock().expect("Can't lock the Mutex(rng)");
                    (
                        rng.random_range(50..500),
                        if rng.random_ratio(1, 10) {
                            rng.random_range(500..1000)
                        } else {
                            0
                        },
                    )
                };

                tokio::time::sleep(Duration::from_millis(delay_ms + extra_delay)).await;
                Ok(Bytes::from(chunk))
            }
        }))
        .buffered(3) // Parallel processing of chunks
    }

    #[tokio::test]
    async fn test_download_with_content_length() {
        test_download(true).await;
    }

    #[tokio::test]
    async fn test_download_without_content_length() {
        test_download(false).await;
    }

    async fn test_download(use_content_length: bool) {
        let content = &[0u8; 1024];
        let filename = "file.txt";
        let chuck_size = 16;

        let routes = warp::path(filename).map(move || {
            let stream = create_realistic_stream(content, chuck_size);

            let mut reply = warp::reply::Response::new(warp::hyper::Body::wrap_stream(stream));

            if use_content_length {
                reply.headers_mut().insert(
                    warp::http::header::CONTENT_LENGTH,
                    content.len().to_string().parse().expect("Can't parse"),
                );
            }

            const RESPONSE_DELAY_MS: u64 = 3000;
            std::thread::sleep(Duration::from_millis(RESPONSE_DELAY_MS)); // Response delay
            reply
        });

        let (addr, server) = warp::serve(routes).bind_ephemeral(([127, 0, 0, 1], 0));

        tokio::spawn(server);
        tokio::time::sleep(Duration::from_millis(500)).await; // Waiting for the server to start up

        let client = Client::new();
        let url = format!("http://{}/{}", addr, filename);
        let output = PathBuf::from(filename);

        // Register the Ctrl+C handler for deleting the created file
        ctrlc::try_set_handler({
            let output = output.clone();
            move || {
                let _ = std::fs::remove_file(&output);
                std::process::exit(0);
            }
        })
        .ok();

        let result = Downloader::download_file(
            &client,
            &DownloadTask {
                url,
                output: output.clone(),
                overwrite: true,
                reporter: Arc::from(tokio::sync::Mutex::new(ConsoleReporter::new())),
            },
        )
        .await;

        assert!(result.is_ok(), "Download failed: {}", result.unwrap_err());
        std::fs::remove_file(&output).ok();
    }
}
