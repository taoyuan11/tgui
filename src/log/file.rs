use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::PathBuf;

use super::config::LogFileConfig;

#[derive(Debug)]
pub(super) struct LogFileSink {
    config: LogFileConfig,
    file: File,
}

impl LogFileSink {
    pub(super) fn new(config: LogFileConfig) -> io::Result<Self> {
        fs::create_dir_all(&config.log_dir)?;
        let file = open_current_file(&config)?;
        Ok(Self { config, file })
    }

    pub(super) fn write_line(&mut self, line: &str) -> io::Result<()> {
        self.rotate_if_needed(line.len() as u64 + 1)?;
        writeln!(self.file, "{line}")?;
        self.file.flush()
    }

    fn rotate_if_needed(&mut self, incoming_bytes: u64) -> io::Result<()> {
        let current_size = self
            .file
            .metadata()
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        if current_size == 0
            || current_size.saturating_add(incoming_bytes) <= self.config.max_file_size_bytes
        {
            return Ok(());
        }

        self.file.flush()?;
        rotate_files(&self.config)?;
        self.file = open_current_file(&self.config)?;
        Ok(())
    }
}

fn open_current_file(config: &LogFileConfig) -> io::Result<File> {
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(current_path(config))
}

fn rotate_files(config: &LogFileConfig) -> io::Result<()> {
    let current = current_path(config);
    if config.max_files <= 1 {
        let _ = fs::remove_file(current);
        return Ok(());
    }

    let oldest = rotated_path(config, config.max_files - 1);
    let _ = fs::remove_file(oldest);

    for index in (1..config.max_files - 1).rev() {
        let from = rotated_path(config, index);
        if from.exists() {
            fs::rename(&from, rotated_path(config, index + 1))?;
        }
    }

    if current.exists() {
        fs::rename(current, rotated_path(config, 1))?;
    }

    Ok(())
}

fn current_path(config: &LogFileConfig) -> PathBuf {
    config.log_dir.join(&config.file_name)
}

fn rotated_path(config: &LogFileConfig, index: usize) -> PathBuf {
    config
        .log_dir
        .join(format!("{}.{}", config.file_name, index))
}

#[cfg(test)]
pub(super) fn write_lines_for_test(config: LogFileConfig, lines: &[&str]) -> io::Result<()> {
    let mut sink = LogFileSink::new(config)?;
    for line in lines {
        sink.write_line(line)?;
    }
    Ok(())
}
