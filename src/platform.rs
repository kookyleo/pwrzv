//! Platform compatibility checking module
//!
//! Provides platform detection and compatibility verification functions.

use crate::error::{PwrzvError, PwrzvResult};

/// Check if the current platform is supported
pub fn check_platform() -> PwrzvResult<()> {
    if is_supported_platform() {
        Ok(())
    } else {
        Err(PwrzvError::UnsupportedPlatform {
            platform: get_platform_name().to_string(),
        })
    }
}

/// Check if the current platform is supported
pub fn is_supported_platform() -> bool {
    cfg!(target_os = "linux") || cfg!(target_os = "macos")
}

/// Legacy function for backward compatibility (Linux-only check)
pub fn is_linux_like() -> bool {
    cfg!(target_os = "linux")
}

/// Get the current platform name
pub fn get_platform_name() -> &'static str {
    if cfg!(target_os = "linux") {
        "Linux"
    } else if cfg!(target_os = "macos") {
        "macOS"
    } else if cfg!(target_os = "windows") {
        "Windows"
    } else if cfg!(target_os = "freebsd") {
        "FreeBSD"
    } else {
        "Unknown"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_detection() {
        let platform_name = get_platform_name();
        assert!(!platform_name.is_empty());

        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            assert!(is_supported_platform());
            assert!(check_platform().is_ok());
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            assert!(!is_supported_platform());
            assert!(check_platform().is_err());
        }
    }

    #[test]
    fn test_legacy_compatibility() {
        // Test that the legacy function still works
        #[cfg(target_os = "linux")]
        assert!(is_linux_like());

        #[cfg(not(target_os = "linux"))]
        assert!(!is_linux_like());
    }
}
