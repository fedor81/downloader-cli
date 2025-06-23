use std::{io::BufRead, path::PathBuf, sync::Arc};

use anyhow::{Context, Result};
use clap::Parser;
use tokio::sync::Mutex;

use downloader_cli::{
    DownloadResult, Downloader, DownloaderBuilder,
    config::{AppConfig, CliConfig, IntoOverwrite, LogLevel},
    reporter::{ConsoleReporterFactory, DownloadReporter, ReporterFactory},
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = CliConfig::parse();
    let mut config = load_config(&args)?;
    args.into_overwrite(&mut config);

    // Initializing the reporter based on the config
    let reporter_factory = ConsoleReporterFactory::from_config(&config);

    let downloader = build_downloader(&args, &config, reporter_factory)?;

    // Performing the download
    let result = execute_download(&args, downloader).await;

    handle_result(result, &config)
}

async fn execute_download(args: &CliConfig, downloader: Downloader) -> DownloadResult {
    if args.resume {
        downloader.resume_download().await
    } else {
        downloader.download_all().await
    }
}

fn handle_result(result: DownloadResult, config: &AppConfig) -> anyhow::Result<()> {
    if !result.errors.is_empty() {
        print_errors("Download errors", &result.errors, config.general.log_level);

        if config.general.log_level.show_summary() {
            let success_count = result.total - result.errors.len();
            println!("\nSuccessfully downloaded {} files", success_count);
        }
        anyhow::bail!("Some downloads failed");
    }

    if config.general.log_level.show_success() {
        println!("\nAll files downloaded successfully!");
    }

    Ok(())
}

fn build_downloader<F>(args: &CliConfig, config: &AppConfig, factory: F) -> Result<Downloader>
where
    F: ReporterFactory + Send + Sync + 'static,
    F::Reporter: DownloadReporter + Send + Sync + 'static,
{
    let destination = args
        .target
        .clone()
        .or_else(|| config.download.download_dir.clone())
        .unwrap_or_else(|| ".".into());
    let destination = PathBuf::from(destination);

    let mut builder = Downloader::builder()
        .with_timeout(config.download.timeout_secs)
        .with_retries(config.download.retries);

    // Processing the source (URL or file)
    let builder = process_source(&args.source, builder, factory, &destination, args.force)?;

    // Building a downloader and handling validation errors
    let (downloader, validation_errors) = builder.build()?;
    if !validation_errors.is_empty() {
        print_errors("Validation errors", &validation_errors, config.general.log_level);
    }

    Ok(downloader)
}

fn load_config(args: &CliConfig) -> anyhow::Result<AppConfig> {
    let mut config = if let Some(config_path) = &args.config {
        AppConfig::load_from_path(config_path)?
    } else {
        AppConfig::load()?
    };

    args.into_overwrite(&mut config);
    Ok(config)
}

/// Processes the source (URL or file)
fn process_source<F>(
    source: &str,
    mut builder: DownloaderBuilder,
    reporter_factory: F,
    destination: &PathBuf,
    overwrite: bool,
) -> anyhow::Result<DownloaderBuilder>
where
    F: ReporterFactory + Send + Sync + 'static,
    F::Reporter: DownloadReporter + Send + Sync + 'static,
{
    if Downloader::is_valid_url(source) {
        builder.add_task(
            source,
            destination,
            overwrite,
            Arc::from(Mutex::new(reporter_factory.create())),
        );
    } else {
        // Reads a list of URLs from a file separated by newlines
        let file =
            std::fs::File::open(source).with_context(|| format!("Failed to open source file: {}", source))?;

        let reader = std::io::BufReader::new(file);
        for (line_num, line) in reader.lines().enumerate() {
            let url =
                line.with_context(|| format!("Failed to read line {} from source file", line_num + 1))?;

            if !url.trim().is_empty() {
                builder.add_task(
                    &url,
                    destination,
                    overwrite,
                    Arc::from(Mutex::new(reporter_factory.create())),
                );
            }
        }
    }
    Ok(builder)
}

/// Prints errors based on silent mode
fn print_errors(title: &str, errors: &[anyhow::Error], log_level: LogLevel) {
    if errors.is_empty() || !log_level.show_errors() {
        return;
    }

    eprintln!("{} ({}):", title, errors.len());
    if errors.len() <= 5 {
        for err in errors {
            eprintln!("  - {}", err);
        }
    } else {
        eprintln!("  ... showing first 5 of {} errors ...", errors.len());
        for err in errors.into_iter().take(5) {
            eprintln!("  - {}", err);
        }
    }
}
