//! Error handling module
//!
//! Defines all error types and result types used in the pwrzv library.

use std::io;
use thiserror::Error;

/// Error types for the pwrzv library
#[derive(Error, Debug)]
pub enum PwrzvError {
    /// I/O error, typically occurs when reading system files
    #[error("I/O error: {0}")]
    IoError(#[from] io::Error),

    /// Unsupported platform error
    #[error(
        "Unsupported platform: {platform}. This library only supports Linux and macOS for now."
    )]
    UnsupportedPlatform { platform: String },

    /// Data parsing error
    #[error("Failed to parse system data: {detail}")]
    ParseError { detail: String },

    /// System resource access error
    #[error("Failed to access system resource: {resource}")]
    ResourceAccessError { resource: String },

    /// Calculation error
    #[error("Calculation error: {detail}")]
    CalculationError { detail: String },
}

impl PwrzvError {
    /// Create unsupported platform error
    pub fn unsupported_platform(platform: &str) -> Self {
        PwrzvError::UnsupportedPlatform {
            platform: platform.to_string(),
        }
    }

    /// Create parsing error
    pub fn parse_error(detail: &str) -> Self {
        PwrzvError::ParseError {
            detail: detail.to_string(),
        }
    }

    /// Create resource access error
    pub fn resource_access_error(resource: &str) -> Self {
        PwrzvError::ResourceAccessError {
            resource: resource.to_string(),
        }
    }

    /// Create calculation error
    pub fn calculation_error(detail: &str) -> Self {
        PwrzvError::CalculationError {
            detail: detail.to_string(),
        }
    }
}

/// Result type for the pwrzv library
pub type PwrzvResult<T> = Result<T, PwrzvError>;
