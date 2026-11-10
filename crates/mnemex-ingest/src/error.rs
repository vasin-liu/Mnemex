//! Scan and chunk errors. Hand-written; no thiserror.

use std::error::Error;
use std::fmt;
use std::path::PathBuf;

/// Failure while reading a directory of text files.
#[derive(Debug)]
pub enum IngestError {
    /// The scan root is missing or is not a directory.
    RootUnavailable {
        /// The path that was requested.
        path: PathBuf,
        /// The IO error from the operating system.
        source: std::io::Error,
    },
    /// A file or directory inside the root could not be read.
    Unreadable {
        /// The path that could not be read.
        path: PathBuf,
        /// The IO error from the operating system.
        source: std::io::Error,
    },
}

impl fmt::Display for IngestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RootUnavailable { path, source } => {
                write!(f, "scan root {} is unavailable: {source}", path.display())
            }
            Self::Unreadable { path, source } => {
                write!(f, "cannot read {}: {source}", path.display())
            }
        }
    }
}

impl Error for IngestError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::RootUnavailable { source, .. } | Self::Unreadable { source, .. } => Some(source),
        }
    }
}
