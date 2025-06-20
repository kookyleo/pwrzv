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

    /// Invalid value error
    #[error("Invalid value: {detail}")]
    InvalidValue { detail: String },
}

impl PwrzvError {
    /// Create unsupported platform error
    #[allow(dead_code)]
    pub(crate) fn unsupported_platform(platform: &str) -> Self {
        PwrzvError::UnsupportedPlatform {
            platform: platform.to_string(),
        }
    }

    /// Create parsing error
    #[allow(dead_code)]
    pub(crate) fn parse_error(detail: &str) -> Self {
        PwrzvError::ParseError {
            detail: detail.to_string(),
        }
    }

    /// Create resource access error
    #[allow(dead_code)]
    pub(crate) fn resource_access_error(resource: &str) -> Self {
        PwrzvError::ResourceAccessError {
            resource: resource.to_string(),
        }
    }

    /// Create calculation error
    #[allow(dead_code)]
    pub(crate) fn calculation_error(detail: &str) -> Self {
        PwrzvError::CalculationError {
            detail: detail.to_string(),
        }
    }

    /// Create collection error (alias for resource access error)
    #[allow(dead_code)]
    pub(crate) fn collection_error(detail: &str) -> Self {
        PwrzvError::ResourceAccessError {
            resource: detail.to_string(),
        }
    }

    /// Create invalid value error
    #[allow(dead_code)]
    pub(crate) fn invalid_value(detail: &str) -> Self {
        PwrzvError::InvalidValue {
            detail: detail.to_string(),
        }
    }
}

/// Result type for the pwrzv library
pub type PwrzvResult<T> = Result<T, PwrzvError>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    fn test_unsupported_platform_error() {
        let error = PwrzvError::unsupported_platform("windows");
        assert!(matches!(error, PwrzvError::UnsupportedPlatform { .. }));

        let error_str = error.to_string();
        assert!(error_str.contains("windows"));
        assert!(error_str.contains("Unsupported platform"));
    }

    #[test]
    fn test_parse_error() {
        let error = PwrzvError::parse_error("invalid JSON format");
        assert!(matches!(error, PwrzvError::ParseError { .. }));

        let error_str = error.to_string();
        assert!(error_str.contains("invalid JSON format"));
        assert!(error_str.contains("Failed to parse"));
    }

    #[test]
    fn test_resource_access_error() {
        let error = PwrzvError::resource_access_error("/proc/meminfo");
        assert!(matches!(error, PwrzvError::ResourceAccessError { .. }));

        let error_str = error.to_string();
        assert!(error_str.contains("/proc/meminfo"));
        assert!(error_str.contains("Failed to access"));
    }

    #[test]
    fn test_calculation_error() {
        let error = PwrzvError::calculation_error("division by zero");
        assert!(matches!(error, PwrzvError::CalculationError { .. }));

        let error_str = error.to_string();
        assert!(error_str.contains("division by zero"));
        assert!(error_str.contains("Calculation error"));
    }

    #[test]
    fn test_collection_error() {
        let error = PwrzvError::collection_error("unable to read CPU stats");
        assert!(matches!(error, PwrzvError::ResourceAccessError { .. }));

        let error_str = error.to_string();
        assert!(error_str.contains("unable to read CPU stats"));
    }

    #[test]
    fn test_invalid_value_error() {
        let error = PwrzvError::invalid_value("negative memory usage");
        assert!(matches!(error, PwrzvError::InvalidValue { .. }));

        let error_str = error.to_string();
        assert!(error_str.contains("negative memory usage"));
        assert!(error_str.contains("Invalid value"));
    }

    #[test]
    fn test_io_error_conversion() {
        let io_error = io::Error::new(io::ErrorKind::PermissionDenied, "Access denied");
        let pwrzv_error: PwrzvError = io_error.into();

        assert!(matches!(pwrzv_error, PwrzvError::IoError(_)));

        let error_str = pwrzv_error.to_string();
        assert!(error_str.contains("I/O error"));
        assert!(error_str.contains("Access denied"));
    }

    #[test]
    fn test_error_debug_format() {
        let error = PwrzvError::parse_error("test error");
        let debug_str = format!("{error:?}");
        assert!(debug_str.contains("ParseError"));
    }

    #[test]
    fn test_error_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<PwrzvError>();
        assert_sync::<PwrzvError>();
    }

    #[test]
    fn test_result_type() {
        let success: PwrzvResult<i32> = Ok(42);
        assert!(success.is_ok());
        if let Ok(value) = success {
            assert_eq!(value, 42);
        }

        let failure: PwrzvResult<i32> = Err(PwrzvError::parse_error("test"));
        assert!(failure.is_err());
    }

    #[test]
    fn test_error_chain() {
        let io_error = io::Error::new(io::ErrorKind::NotFound, "File not found");
        let pwrzv_error: PwrzvError = io_error.into();

        // Test that the original error is preserved in the chain
        let error_str = pwrzv_error.to_string();
        assert!(error_str.contains("File not found"));
    }

    #[test]
    fn test_error_construction_consistency() {
        let detail = "test detail message";

        let parse_error = PwrzvError::parse_error(detail);
        let resource_error = PwrzvError::resource_access_error(detail);
        let calc_error = PwrzvError::calculation_error(detail);
        let invalid_error = PwrzvError::invalid_value(detail);

        // All should contain the detail message
        assert!(parse_error.to_string().contains(detail));
        assert!(resource_error.to_string().contains(detail));
        assert!(calc_error.to_string().contains(detail));
        assert!(invalid_error.to_string().contains(detail));
    }
}
