//! Platform detection module
//!
//! Provides platform compatibility checks to ensure the library only runs on Linux-like systems.

use crate::error::{PwrzvError, PwrzvResult};

/// Check if the current platform is a Linux-like system
pub fn check_platform() -> PwrzvResult<()> {
    if cfg!(target_os = "linux") {
        Ok(())
    } else {
        Err(PwrzvError::unsupported_platform(get_platform_name()))
    }
}

/// Get the current platform name
pub fn get_platform_name() -> &'static str {
    if cfg!(target_os = "linux") {
        "Linux"
    } else if cfg!(target_os = "windows") {
        "Windows"
    } else if cfg!(target_os = "macos") {
        "macOS"
    } else if cfg!(target_os = "freebsd") {
        "FreeBSD"
    } else if cfg!(target_os = "openbsd") {
        "OpenBSD"
    } else if cfg!(target_os = "netbsd") {
        "NetBSD"
    } else {
        "Unknown"
    }
}

/// Check if the current platform is Linux-like
pub fn is_linux_like() -> bool {
    cfg!(target_os = "linux")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_detection() {
        // This test will have different results on different platforms
        let platform_name = get_platform_name();
        assert!(!platform_name.is_empty());
        
        if cfg!(target_os = "linux") {
            assert!(is_linux_like());
            assert!(check_platform().is_ok());
        } else {
            assert!(!is_linux_like());
            assert!(check_platform().is_err());
        }
    }
} 