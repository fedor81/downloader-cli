use std::{
    fmt::Display,
    io::BufRead,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result};
use clap::Parser;
use tokio::sync::Mutex;

use downloader_cli::{
    DownloadResult, DownloadTask, Downloader,
    builder::DownloaderBuilder,
    config::{CliConfig, LogLevel, load_config},
    reporter::{
        DownloadReporter, ProgramFlowReporter, ReporterFactory, console_reporter::ConsoleReporterFactory,
        program_flow::ProgramReporter,
    },
};

type AppConfig = downloader_cli::config::app::AppConfig;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = CliConfig::parse();
    let config = load_config(&args)?;
    run(args, config).await
}

async fn run(args: CliConfig, config: AppConfig) -> anyhow::Result<()> {
    // Initializing reporters based on the config
    let mut program_reporter = ProgramReporter::from(&config);
    let reporter_factory = ConsoleReporterFactory::new(&config.progress_bar, &config.output);
    let downloader = build_downloader(&args, &config, reporter_factory)?;

    program_reporter.on_start();

    // Performing the download
    let result = execute_download(downloader, args.resume).await;
    handle_result(result, &config, &mut program_reporter)
}

async fn execute_download(mut downloader: Downloader, resume: bool) -> DownloadResult {
    if resume {
        downloader.resume_download().await
    } else {
        downloader.download_all_consume().await
    }
}

fn handle_result<T: ProgramFlowReporter>(
    result: DownloadResult,
    config: &AppConfig,
    program_reporter: &mut T,
) -> anyhow::Result<()> {
    if !result.errors.is_empty() {
        print_errors("Download errors", &result.errors, config.general.log_level);

        if config.general.log_level.show_summary() {
            let success_count = result.total - result.errors.len();
            println!("\nSuccessfully downloaded {} files", success_count);
        }
        anyhow::bail!("Some downloads failed");
    }

    if config.general.log_level.show_success() {
        program_reporter.on_success();
    }

    program_reporter.on_finish();

    Ok(())
}

fn build_downloader<F>(args: &CliConfig, config: &AppConfig, factory: F) -> Result<Downloader>
where
    F: ReporterFactory + Send + Sync + 'static,
    F::Reporter: DownloadReporter + Send + Sync + 'static,
{
    let destination = args
        .target
        .as_ref()
        .or_else(|| config.download.download_dir.as_ref());

    let mut builder = DownloaderBuilder::from(config);

    // Processing the source (URL or file)
    if Downloader::is_valid_url(&args.source) {
        builder.add_task(
            &args.source,
            destination.unwrap_or(&PathBuf::from(DownloadTask::sanitize_filename(&args.source))),
            args.force,
            Arc::from(Mutex::new(factory.create())),
        );
    } else {
        add_tasks_from_file(
            &args.source,
            &mut builder,
            factory,
            destination.unwrap_or(&PathBuf::from(".")),
            args.force,
        )?;
    }

    // Building a downloader and handling validation errors
    let (downloader, validation_errors) = builder.build()?;
    if !validation_errors.is_empty() {
        print_errors("Validation errors", &validation_errors, config.general.log_level);
    }

    Ok(downloader)
}

/// Reads a list of URLs from a file separated by newlines
/// and adds them to the downloader as tasks.
///
/// `destination` is the directory where the files will be saved.
fn add_tasks_from_file<F>(
    file: impl AsRef<Path> + Display,
    builder: &mut DownloaderBuilder,
    reporter_factory: F,
    destination: &PathBuf,
    overwrite: bool,
) -> anyhow::Result<()>
where
    F: ReporterFactory + Send + Sync + 'static,
    F::Reporter: DownloadReporter + Send + Sync + 'static,
{
    if !destination.is_dir() {
        return Err(anyhow::anyhow!("Destination path is not a directory"));
    }

    let file = std::fs::File::open(&file).with_context(|| format!("Failed to open source file: {}", file))?;
    let reader = std::io::BufReader::new(file);

    for (line_num, line) in reader.lines().enumerate() {
        let url = line.with_context(|| format!("Failed to read line {} from source file", line_num + 1))?;

        if !url.trim().is_empty() {
            builder.add_task(
                &url,
                destination.join(DownloadTask::sanitize_filename(&url)),
                overwrite,
                Arc::from(Mutex::new(reporter_factory.create())),
            );
        }
    }
    Ok(())
}

/// Prints errors based on silent mode
fn print_errors(title: &str, errors: &[anyhow::Error], log_level: LogLevel) {
    if !errors.is_empty() && log_level.show_errors() {
        eprintln!("{} ({}):", title, errors.len());

        for err in errors {
            eprintln!("  - {}", err);
        }
    }
}
