use std::sync::Arc;

use crate::config::app::{AppConfig, LogLevel, OutputConfig};

use super::ProgramFlowReporter;

pub struct ProgramReporter {
    log_level: LogLevel,
    config: Arc<OutputConfig>,
}

impl ProgramReporter {
    fn print_message(&self, message: &Option<String>) {
        if let Some(message) = message {
            println!("{}", message);
        }
    }
}

impl ProgramFlowReporter for ProgramReporter {
    fn on_start(&mut self) {
        self.print_message(&self.config.message_on_start);
    }

    fn on_finish(&mut self) {
        self.print_message(&self.config.message_on_finish);
    }

    fn on_errors(&mut self, errors: Vec<anyhow::Error>) {
        // TODO: Unimplemented
    }

    fn on_success(&mut self) {
        self.print_message(&self.config.message_on_success);
    }
}

impl From<&AppConfig> for ProgramReporter {
    fn from(value: &AppConfig) -> Self {
        Self {
            log_level: value.general.log_level,
            config: Arc::clone(&value.output),
        }
    }
}
