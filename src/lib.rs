use anyhow::{Context, Result};
use futures::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use regex::Regex;
use reqwest::{self, Client};
use std::path::PathBuf;
use std::time::Duration;
use tokio::io::AsyncWriteExt;

pub use builder::DownloaderBuilder;
mod builder;
pub mod config;

#[derive(Debug, Clone)]
pub struct Downloader {
    tasks: Vec<DownloadTask>,
    client: Client,
}

#[derive(Debug, Clone)]
pub struct DownloadTask {
    pub url: String,
    pub output: PathBuf,
    pub overwrite: bool,
}

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

    async fn download_file(client: &reqwest::Client, task: &DownloadTask) -> Result<()> {
        let url = &task.url;
        let output = &task.output;

        // Create progress bar for request
        let pb = ProgressBar::new_spinner()
            .with_message(format!("Requesting information about {}", url));
        pb.enable_steady_tick(Duration::from_millis(100));

        let response = client
            .get(url)
            .send()
            .await
            .with_context(|| format!("Failed to GET: '{}'", url))?;

        pb.finish_and_clear();

        // Checking the response status
        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Request {} failed with status: {}",
                url,
                response.status()
            ));
        }

        // Get file size from Content-Length header (if any)
        let total_size = response
            .headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|ct_len| ct_len.to_str().ok())
            .and_then(|ct_len| ct_len.parse::<u64>().ok());

        if let Some(total_size) = total_size {
            println!("Size: {}", indicatif::HumanBytes(total_size));
        }

        let total_size = total_size.unwrap_or(10);

        // Progress bar for download
        let pb = ProgressBar::new(total_size);
        pb.set_style(ProgressStyle::default_bar()
                        .template("{spinner:.green} [{elapsed_precise}] {bar:40.cyan/blue} {bytes}/{total_bytes} ({eta})")
                        .context("Failed to set progress bar style")?
                        .progress_chars("▓ ░"));

        // pb.set_style(
        //     ProgressStyle::default_bar()
        //         .template("{spinner:.green} [{elapsed_precise}] {bytes} ({bytes_per_sec})")
        //         .context("Failed to set progress bar style")?
        //         .tick_chars("_>_>_>_>_>_>"),
        // );

        let output_display = output.display();

        if std::fs::exists(output)
            .with_context(|| format!("Can't check existence of file: {}", &output_display))?
        {
            if task.overwrite {
                std::fs::remove_file(output)
                    .with_context(|| format!("Can't remove file: {}", output_display))?;
            } else {
                return Err(anyhow::anyhow!(format!(
                    "File exists: {}. See '--help' for solutions.",
                    output_display
                )));
            }
        }

        let file = tokio::fs::File::create(output)
            .await
            .with_context(|| format!("Failed to create file: {}", output.display()))?;
        let mut writer = tokio::io::BufWriter::new(file);

        println!(
            "Saving as: {}",
            output.file_name().unwrap().to_str().unwrap()
        );

        // Get the data stream from the response
        let mut downloaded: u64 = 0;
        let mut stream = response.bytes_stream();

        // Read the stream bit by bit and write it to a file
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("Failed to read chunk")?;
            writer.write_all(&chunk).await?;

            // Update progress bar
            downloaded += chunk.len() as u64;
            pb.set_position(downloaded % total_size);
        }

        writer.flush().await?;
        pb.finish_with_message(format!("Download complete: {}", url));
        Ok(())
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
    use std::sync::{Arc, Mutex};

    use bytes::Bytes;
    use rand::{Rng, SeedableRng, rngs::StdRng};
    use warp::Filter;

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
            },
        )
        .await;

        assert!(result.is_ok(), "Download failed: {}", result.unwrap_err());
        std::fs::remove_file(&output).ok();
    }
}
