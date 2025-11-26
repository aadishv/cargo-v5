#[cfg(feature = "clap")]
use std::ffi::OsStr;

#[cfg(feature = "clap")]
use clap_complete::engine::{CompletionCandidate, ValueCompleter};

use crate::errors::CliError;

async fn get_file_list() -> Result<Vec<String>, CliError> {
    let mut connection = crate::connection::open_connection().await.map_err(|e| CliError::SerialError(vex_v5_serial::connection::serial::SerialError::IoError(std::io::Error::new(std::io::ErrorKind::Other, e))))?;

    let entries = crate::commands::dir::get_file_entries(&mut connection).await?;
    let files = entries.into_iter().map(|entry| entry.path).collect();

    Ok(files)
}

#[cfg(feature = "clap")]
pub struct FileCompleter;

#[cfg(feature = "clap")]
impl ValueCompleter for FileCompleter {
    fn complete(&self, current: &OsStr) -> Vec<CompletionCandidate> {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let files = rt.block_on(get_file_list()).unwrap_or_default();

        let current_str = current.to_string_lossy();

        files
            .into_iter()
            .filter(|file| file.starts_with(&*current_str))
            .map(CompletionCandidate::new)
            .collect()
    }
}
