use std::path::PathBuf;

use downloader_cli::Downloader;

#[tokio::main]
async fn main() {
    let urls = vec!["http://212.183.159.230/5MB.zip".to_string()];

    let (downloader, errors) = Downloader::build(urls, PathBuf::from("5MB.zip")).unwrap();

    if !errors.is_empty() {
        println!(
            "Errors occurred:\n{}",
            errors
                .into_iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    downloader.download_async().await.unwrap();
}
