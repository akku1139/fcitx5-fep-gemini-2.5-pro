// src/error.rs
// Defines custom error types for the application.

use std::{fmt, io};
use zbus; // Add zbus for its error type

#[derive(Debug)]
pub enum FepError {
    Io(io::Error),
    TerminalSetup(String),
    FcitxConnection(String),
    Zbus(zbus::Error), // Include zbus::Error
    // Add other specific error types as needed
}

impl fmt::Display for FepError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FepError::Io(err) => write!(f, "IO Error: {}", err),
            FepError::TerminalSetup(msg) => write!(f, "Terminal Setup Error: {}", msg),
            FepError::FcitxConnection(msg) => write!(f, "Fcitx Connection Error: {}", msg),
            FepError::Zbus(err) => write!(f, "D-Bus Error: {}", err),
        }
    }
}

impl std::error::Error for FepError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            FepError::Io(err) => Some(err),
            FepError::Zbus(err) => Some(err),
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

// Allow converting zbus::Error into FepError
impl From<zbus::Error> for FepError {
    fn from(err: zbus::Error) -> Self {
        // Optionally map specific zbus errors to more specific FepErrors
        FepError::Zbus(err)
    }
}

// Allow converting FepError into Box<dyn Error> for main result type
impl From<FepError> for Box<dyn std::error::Error> {
    fn from(err: FepError) -> Self {
        Box::new(err)
    }
}
