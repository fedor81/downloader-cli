use std::{io::BufRead, path::PathBuf, sync::Arc};

use anyhow::{Context, Result};
use clap::Parser;

use downloader_cli::{
    Downloader, DownloaderBuilder,
    config::{Config, load_config, load_config_from_path},
    reporter::{ConsoleReporter, ConsoleReporterFactory},
};
use tokio::sync::Mutex;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    /// URL
    source: String,

    /// Target filepath (existing directories will be treated as the target location)
    target: Option<String>,

    /// Silent mode
    #[arg(short, long)]
    silent: bool,

    /// Resume failed or cancelled download (partial sanity check)
    #[arg(short, long)]
    resume: bool,

    /// Uses the config specified in the argument
    #[arg(long)]
    config: Option<String>,

    /// Overwrite if the file already exists
    #[arg(short, long)]
    force: bool,
    //
    // UI
    //
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Cli::parse();
    let destination = args.target.unwrap_or_else(|| ".".to_string());
    let destination = PathBuf::from(destination);

    let config: Config = if let Some(config_path) = args.config {
        load_config_from_path(&config_path)?
    } else {
        load_config()?
    };

    let mut builder = Downloader::builder();

    // Processing the source (URL or file)
    process_source(&args.source, &mut builder, &destination, args.force)?;

    // Building a downloader and handling validation errors
    let (downloader, validation_errors) = builder.build()?;
    if !validation_errors.is_empty() {
        print_errors("Validation errors", validation_errors, !args.silent);
    }

    // Performing the download
    let download_errors = if args.resume {
        downloader.resume_download().await
    } else {
        downloader.download_all().await
    };

    // Handling download errors
    if !download_errors.is_empty() {
        let errors_count = download_errors.len();
        print_errors("Download errors", download_errors, !args.silent);

        if !args.silent {
            let success_count = downloader.task_count() - errors_count;
            println!("\nSuccessfully downloaded {} files", success_count);
        }
        anyhow::bail!("Some downloads failed");
    }

    if !args.silent {
        println!("\nAll files downloaded successfully!");
    }

    Ok(())
}

/// Processes the source (URL or file)
fn process_source(
    source: &str,
    builder: &mut DownloaderBuilder,
    destination: &PathBuf,
    overwrite: bool,
) -> anyhow::Result<()> {
    let reporter_factory = ConsoleReporterFactory::new();

    if Downloader::is_valid_url(source) {
        builder.add_task(
            source,
            destination,
            overwrite,
            Arc::from(Mutex::new(reporter_factory.create())),
        );
        Ok(())
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
        Ok(())
    }
}

/// Prints errors based on silent mode
fn print_errors(title: &str, errors: Vec<anyhow::Error>, show_errors: bool) {
    if errors.is_empty() || !show_errors {
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
