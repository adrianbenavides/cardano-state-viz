//! This module defines all error types used throughout the application.

use std::io;
use std::path::PathBuf;
use thiserror::Error;

/// Main error type for the application
#[derive(Error, Debug)]
pub enum Error {
    /// IO errors (file not found, permission denied, etc.)
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    /// CBOR decoding errors
    #[error("CBOR decoding error: {0}")]
    CborDecode(String),

    /// Data source errors
    #[error("Data source error: {0}")]
    DataSource(String),

    /// Blockfrost API errors
    #[error("Blockfrost API error: {0}")]
    Blockfrost(String),

    /// Cardano node errors
    #[error("Cardano node error: {0}")]
    Node(String),

    /// Parser errors
    #[error("Parser error: {0}")]
    Parser(String),

    /// Schema validation errors
    #[error("Schema validation error: {0}")]
    SchemaValidation(String),

    /// Schema parsing errors
    #[error("Schema parsing error in {file:?}: {message}")]
    SchemaParse { file: PathBuf, message: String },

    /// State machine construction errors
    #[error("State machine error: {0}")]
    StateMachine(String),

    /// TUI/visualization errors
    #[error("TUI error: {0}")]
    Tui(String),

    /// Configuration errors
    #[error("Configuration error: {0}")]
    Config(String),

    /// Invalid address format
    #[error("Invalid Cardano address: {0}")]
    InvalidAddress(String),

    /// Invalid transaction hash
    #[error("Invalid transaction hash: {0}")]
    InvalidTxHash(String),

    /// Missing configuration
    #[error("Missing configuration: {0}")]
    MissingConfig(String),

    /// API rate limit exceeded
    #[error("API rate limit exceeded. Try again later or use caching.")]
    RateLimitExceeded,

    /// User quit (Ctrl+C, 'q' in TUI)
    #[error("User quit")]
    UserQuit,

    /// Feature not yet implemented
    #[error("Not implemented: {0}")]
    NotImplemented(String),

    /// Generic error with custom message
    #[error("{0}")]
    Custom(String),

    /// Wrapped anyhow errors
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Result type alias using our Error type
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Create a custom error with a message
    pub fn custom(msg: impl Into<String>) -> Self {
        Self::Custom(msg.into())
    }

    /// Create a data source error
    pub fn data_source(msg: impl Into<String>) -> Self {
        Self::DataSource(msg.into())
    }

    /// Create a parser error
    pub fn parser(msg: impl Into<String>) -> Self {
        Self::Parser(msg.into())
    }

    /// Create a schema validation error
    pub fn schema_validation(msg: impl Into<String>) -> Self {
        Self::SchemaValidation(msg.into())
    }

    /// Create a state machine error
    pub fn state_machine(msg: impl Into<String>) -> Self {
        Self::StateMachine(msg.into())
    }

    /// Check if error is a user quit
    pub fn is_user_quit(&self) -> bool {
        matches!(self, Error::UserQuit)
    }
}

// Implement From traits for common external error types

impl From<toml::de::Error> for Error {
    fn from(err: toml::de::Error) -> Self {
        Error::SchemaParse {
            file: PathBuf::from("unknown"),
            message: err.to_string(),
        }
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::Parser(format!("JSON error: {}", err))
    }
}

// Helper macros for creating errors

/// Create a custom error with formatting
#[macro_export]
macro_rules! custom_error {
    ($($arg:tt)*) => {
        $crate::error::Error::Custom(format!($($arg)*))
    };
}

/// Bail with a custom error message
#[macro_export]
macro_rules! bail {
    ($($arg:tt)*) => {
        return Err($crate::custom_error!($($arg)*))
    };
}

/// Ensure a condition is true or return error
#[macro_export]
macro_rules! ensure {
    ($cond:expr, $($arg:tt)*) => {
        if !($cond) {
            $crate::bail!($($arg)*);
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let err = Error::custom("test error");
        assert_eq!(err.to_string(), "test error");

        let err = Error::data_source("connection failed");
        assert_eq!(err.to_string(), "Data source error: connection failed");
    }

    #[test]
    fn test_user_quit() {
        let err = Error::UserQuit;
        assert!(err.is_user_quit());

        let err = Error::custom("other");
        assert!(!err.is_user_quit());
    }
}
