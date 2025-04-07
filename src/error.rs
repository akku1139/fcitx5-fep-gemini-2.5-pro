// Defines custom error types for the application.

use std::{fmt, io};

#[derive(Debug)]
pub enum FepError {
    Io(io::Error),
    TerminalSetup(String),
    FcitxConnection(String),
    // Add other specific error types as needed
}

impl fmt::Display for FepError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FepError::Io(err) => write!(f, "IO Error: {}", err),
            FepError::TerminalSetup(msg) => write!(f, "Terminal Setup Error: {}", msg),
            FepError::FcitxConnection(msg) => write!(f, "Fcitx Connection Error: {}", msg),
        }
    }
}

impl std::error::Error for FepError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            FepError::Io(err) => Some(err),
            _ => None,
        }
    }
}

// Allow converting io::Error into FepError
impl From<io::Error> for FepError {
    fn from(err: io::Error) -> Self {
        FepError::Io(err)
    }
}
