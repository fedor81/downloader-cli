use anyhow::{Context, Result, anyhow};
use regex::Regex;
use reqwest::header::{CONTENT_LENGTH, HeaderValue, RANGE};
use reqwest::{self, IntoUrl};
use reqwest::{StatusCode, Url};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use tokio::{fs::File, io::AsyncWriteExt, task::JoinSet};

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
        let response = client
            .get(url)
            .send()
            .await
            .with_context(|| format!("Failed to GET '{}'", url))?;

        let mut file = File::create(output)
            .await
            .with_context(|| format!("Failed to create file '{}'", output.display()))?;

        let content = response
            .bytes()
            .await
            .with_context(|| format!("Failed to get bytes from '{}'", url))?;

        file.write_all(&content)
            .await
            .with_context(|| format!("Failed to write to '{}'", output.display()))?;

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

//     const CHUNK_SIZE: u64 = 8192;

//     // Getting information about the file
//     let response = client
//         .head(url)
//         .send()
//         .await
//         .context("Failed to send HEAD request")?;

//     // Check support for partial requests
//     if !response.headers().contains_key("accept-ranges") {
//         return Err(anyhow::anyhow!("Server does not support partial content"));
//     }

//     // Get the length of the content
//     let length = response
//         .headers()
//         .get(CONTENT_LENGTH)
//         .context("Response doesn't include content length")?
//         .to_str()
//         .context("Invalid Content-Length header")?;
//     let length = u64::from_str(length).context("Content-Length is not a valid number")?;

//     // Create a file for writing
//     let mut output_file = File::create(output)
//         .await
//         .context("Failed to create output file")?;

//     println!("Starting download of {} bytes...", length);

//     // Downloading the file in parts
//     for range in PartialRangeIter::new(0, length, CHUNK_SIZE)? {

//         let response = client
//             .get(url)
//             .header(RANGE, range)
//             .send()
//             .await
//             .context("Failed to send GET request")?;

//         match response.status() {
//             StatusCode::OK | StatusCode::PARTIAL_CONTENT => {
//                 let bytes = response
//                     .bytes()
//                     .await
//                     .context("Failed to read response bytes")?;
//                 output_file
//                     .write_all(&bytes)
//                     .context("Failed to write to file")?;
//             }
//             status => {
//                 return Err(anyhow::anyhow!("Unexpected server response: {}", status));
//             }
//         }
//     }

//     Ok(())
// }

#[cfg(test)]
mod tests {
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
}
