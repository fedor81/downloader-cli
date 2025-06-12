use anyhow::{Context, Result, anyhow};
use futures::StreamExt;
use indicatif::{HumanBytes, ProgressBar, ProgressStyle};
use regex::Regex;
use reqwest::Url;
use reqwest::header::CONTENT_LENGTH;
use reqwest::{self};
use std::fmt::format;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::AsyncWriteExt;

pub struct Downloader {
    urls: Vec<String>,
    destination: PathBuf,
    overwrite: bool,
}

impl Downloader {
    /// Returns: Self and a vector of unvalidated URLs
    pub fn build(urls: Vec<String>, destination: PathBuf) -> Result<(Self, Vec<anyhow::Error>)> {
        let (urls, errors) = Downloader::validate_urls(urls);

        if urls.is_empty() {
            return Err(anyhow::anyhow!("No valid URLs provided"));
        }

        Ok((
            Self {
                urls,
                destination,
                overwrite: false,
            },
            errors,
        ))
    }

    /// Reads a list of URLs from a file separated by newlines
    ///
    /// Returns: Self and a vector of unvalidated URLs
    pub fn from_file(path: &Path, destination: PathBuf) -> Result<(Self, Vec<anyhow::Error>)> {
        let reader = BufReader::new(std::fs::File::open(path).unwrap());
        Downloader::build(
            reader.lines().map(|line| line.unwrap()).collect(),
            destination,
        )
    }

    pub async fn download_async(&self) -> Result<()> {
        let client = reqwest::Client::new();
        let mut tasks = tokio::task::JoinSet::new();
        let mut errors = vec![];

        for url in self.urls.iter() {
            let output = PathBuf::from(Downloader::get_filename(url));
            let client = client.clone();
            let url = url.clone();
            tasks.spawn(
                async move { Downloader::download_file_async(&client, &url, &output).await },
            );
        }

        while let Some(result) = tasks.join_next().await {
            match result {
                Ok(Ok(_)) => {} // Success download
                Ok(Err(e)) => errors.push(format!("Download error: {}", e)),
                Err(join_err) => errors.push(format!("Task failed: {}", join_err)),
            }
        }

        if !errors.is_empty() {
            return Err(anyhow::anyhow!("Errors occurred:\n{}", errors.join("\n")));
        }
        Ok(())
    }

    fn validate_urls(urls: Vec<String>) -> (Vec<String>, Vec<anyhow::Error>) {
        let mut valid_urls = vec![];
        let mut errors = vec![];

        for url_string in urls {
            match reqwest::Url::parse(&url_string) {
                Ok(_) => valid_urls.push(url_string),
                Err(e) => errors.push(e.into()),
            }
        }

        (valid_urls, errors)
    }

    async fn download_file_async(
        client: &reqwest::Client,
        url: &str,
        output: &PathBuf,
    ) -> Result<()> {
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

        let mut file = tokio::fs::File::create(output)
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
    fn get_filename(url: &str) -> String {
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
    use reqwest::Client;
    use warp::Filter;

    use super::*;

    #[test]
    fn test_get_filename() {
        assert_eq!(
            Downloader::get_filename("https://example.com/file.txt"),
            "file.txt"
        );
        assert_eq!(
            Downloader::get_filename("https://example.com/file.txt?param=value"),
            "file.txt"
        );
        assert_eq!(
            Downloader::get_filename("https://example.com/file.txt#fragment"),
            "file.txt"
        );
        assert_eq!(
            Downloader::get_filename("https://example.com/file.txt?param=value#fragment"),
            "file.txt"
        );
        assert_eq!(
            Downloader::get_filename("https://example.com/"),
            "example_com"
        );
        assert_eq!(
            Downloader::get_filename("https://example.com/page/1/"),
            "example_com_page_1"
        );
        assert_eq!(
            Downloader::get_filename("https://example.com/page/1/?param=value#fragment"),
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
                    let mut rng = rng.lock().unwrap();
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
                    content.len().to_string().parse().unwrap(),
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

        let result = Downloader::download_file_async(&client, &url, &output).await;

        assert!(result.is_ok(), "Download failed: {}", result.unwrap_err());
        std::fs::remove_file(&output).ok();
    }
}
