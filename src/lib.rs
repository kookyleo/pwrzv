//! # pwrzv - Power Reserve Monitor
//!
//! A Rolls-Royce–inspired performance reserve meter for Linux and macOS systems.
//!
//! This library provides a simple way to monitor system performance by calculating
//! a "power reserve" score (0-5) that indicates how much computational headroom
//! your system has available.
//!
//! ## Quick Start
//!
//! ### Basic Usage
//!
//! ```rust
//! use pwrzv::get_power_reserve_level_direct;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let level = get_power_reserve_level_direct().await?;
//!     println!("Power Reserve Level: {}", level);
//!     Ok(())
//! }
//! ```
//!
//! ### Detailed Analysis
//!
//! ```rust
//! use pwrzv::{get_power_reserve_level_with_details_direct, PowerReserveLevel};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let (level, details) = get_power_reserve_level_with_details_direct().await?;
//!     let power_level = PowerReserveLevel::try_from(level)?;
//!     
//!     println!("Power Reserve: {} ({})", level, power_level);
//!     println!("Detailed metrics:");
//!     for (metric, value) in details {
//!         println!("  {}: {:.3}", metric, value);
//!     }
//!     Ok(())
//! }
//! ```
//!
//! ## Architecture
//!
//! The library uses a streamlined processing pipeline:
//!
//! 1. **Platform Detection**: Automatically detects Linux or macOS
//! 2. **Direct Metrics Collection**: Platform-specific calculator collects system metrics
//! 3. **Real-time Processing**: Applies sigmoid transformations and calculates scores instantly
//! 4. **Power Reserve Calculation**: Returns final 1-5 power reserve level
//!
//! All metrics are collected and processed in real-time without any intermediate storage,
//! making the library fast and lightweight.
//!
//! ## Environment Variable Configuration
//!
//! All sigmoid function parameters can be customized via environment variables:
//!
//! ```bash
//! export PWRZV_LINUX_CPU_USAGE_MIDPOINT=0.70
//! export PWRZV_LINUX_CPU_USAGE_STEEPNESS=10.0
//! ```
//!
//! ## Supported Platforms
//!
//! - **Linux**: Uses `/proc` filesystem for direct system access
//! - **macOS**: Uses system commands (`sysctl`, `vm_stat`, etc.)
//!
//! ## Error Handling
//!
//! All functions return `PwrzvResult<T>` which can be easily handled:
//!
//! ```rust
//! use pwrzv::{get_power_reserve_level_direct, PwrzvError};
//!
//! #[tokio::main]
//! async fn main() {
//!     match get_power_reserve_level_direct().await {
//!         Ok(level) => println!("Power Reserve: {}", level),
//!         Err(PwrzvError::UnsupportedPlatform { platform }) => {
//!             eprintln!("Platform {} not supported", platform);
//!         }
//!         Err(e) => eprintln!("Error: {}", e),
//!     }
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt};

#[cfg(target_os = "linux")]
use crate::linux::calculator::LinuxProvider;
#[cfg(target_os = "macos")]
use crate::macos::calculator::MacProvider;

pub mod error;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
mod sigmoid;

pub use error::{PwrzvError, PwrzvResult};

/// Power Reserve Level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum PowerReserveLevel {
    /// Abundant: System resources are abundant, suitable for high-performance tasks
    Abundant = 5,
    /// High: System resources are in good condition
    High = 4,
    /// Medium: System resources are in normal condition
    Medium = 3,
    /// Low: System resources are under pressure, need to optimize resource usage
    Low = 2,
    /// Critical: System resources are severely constrained, immediate action required
    Critical = 1,
}

impl fmt::Display for PowerReserveLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let level_str = match self {
            PowerReserveLevel::Abundant => "Abundant",
            PowerReserveLevel::High => "High",
            PowerReserveLevel::Medium => "Medium",
            PowerReserveLevel::Low => "Low",
            PowerReserveLevel::Critical => "Critical",
        };
        write!(f, "{level_str}")
    }
}

impl TryFrom<u8> for PowerReserveLevel {
    type Error = PwrzvError;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            5 => Ok(PowerReserveLevel::Abundant),
            4 => Ok(PowerReserveLevel::High),
            3 => Ok(PowerReserveLevel::Medium),
            2 => Ok(PowerReserveLevel::Low),
            1 => Ok(PowerReserveLevel::Critical),
            _ => Err(PwrzvError::InvalidValue {
                detail: format!("Invalid power reserve level: {value}"),
            }),
        }
    }
}

trait PowerReserveMeterProvider {
    async fn get_power_reserve_level(&self) -> PwrzvResult<u8>;
    async fn get_power_reserve_level_with_details(&self) -> PwrzvResult<(u8, HashMap<String, u8>)>;
}

// ================================
// Platform-specific power reserve calculator
// ================================

/// Platform-specific power reserve calculator enum
#[derive(Clone, Debug)]
enum Calculator {
    #[cfg(target_os = "linux")]
    Linux(LinuxProvider),
    #[cfg(target_os = "macos")]
    MacOS(MacProvider),
}

impl Calculator {
    /// Create calculator for the current platform
    fn new() -> PwrzvResult<Self> {
        #[cfg(target_os = "linux")]
        {
            Ok(Calculator::Linux(LinuxProvider))
        }
        #[cfg(target_os = "macos")]
        {
            Ok(Calculator::MacOS(MacProvider))
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            Err(PwrzvError::unsupported_platform(&format!(
                "Platform '{}' is not supported yet. Only Linux and macOS are supported for now.",
                std::env::consts::OS
            )))
        }
    }

    /// Get current power reserve level
    async fn get_power_reserve_level(&self) -> PwrzvResult<u8> {
        #[cfg(target_os = "linux")]
        {
            let Calculator::Linux(calc) = self;
            return calc.get_power_reserve_level().await;
        }
        #[cfg(target_os = "macos")]
        {
            let Calculator::MacOS(calc) = self;
            return calc.get_power_reserve_level().await;
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        unreachable!("Calculator should only be created on supported platforms")
    }

    /// Get current power reserve level with detailed information
    async fn get_power_reserve_level_with_details(&self) -> PwrzvResult<(u8, HashMap<String, u8>)> {
        #[cfg(target_os = "linux")]
        {
            let Calculator::Linux(calc) = self;
            return calc.get_power_reserve_level_with_details().await;
        }
        #[cfg(target_os = "macos")]
        {
            let Calculator::MacOS(calc) = self;
            return calc.get_power_reserve_level_with_details().await;
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        unreachable!("Calculator should only be created on supported platforms")
    }
}

/// Get the current platform name
///
/// # Returns
///
/// A string representing the current platform ("linux", "macos", etc.)
pub fn get_platform_name() -> &'static str {
    std::env::consts::OS
}

/// Get power reserve level directly without any intermediate storage
///
/// This function collects system metrics in real-time and calculates
/// the power reserve level immediately.
///
/// # Returns
///
/// Power reserve level as u8 (1-5) where:
/// - 5: Abundant resources
/// - 4: High resources  
/// - 3: Medium resources
/// - 2: Low resources
/// - 1: Critical resources
///
/// # Example
///
/// ```rust
/// use pwrzv::get_power_reserve_level_direct;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let level = get_power_reserve_level_direct().await?;
///     println!("Power Reserve Level: {}", level);
///     Ok(())
/// }
/// ```
pub async fn get_power_reserve_level_direct() -> PwrzvResult<u8> {
    let calculator = Calculator::new()?;
    calculator.get_power_reserve_level().await
}

/// Get power reserve level with detailed metrics directly
///
/// This function collects system metrics in real-time and calculates both
/// the overall power reserve level and detailed pressure scores for each metric.
///
/// # Returns
///
/// A tuple of (level, details) where:
/// - level: Power reserve level as u8 (1-5)
/// - details: HashMap containing pressure scores for each available metric
///
/// # Example
///
/// ```rust
/// use pwrzv::get_power_reserve_level_with_details_direct;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let (level, details) = get_power_reserve_level_with_details_direct().await?;
///     
///     println!("Power Reserve Level: {}", level);
///     println!("Detailed metrics:");
///     for (metric, score) in details {
///         println!("  {}: {:.3}", metric, score);
///     }
///     Ok(())
/// }
/// ```
pub async fn get_power_reserve_level_with_details_direct() -> PwrzvResult<(u8, HashMap<String, u8>)>
{
    let calculator = Calculator::new()?;
    calculator.get_power_reserve_level_with_details().await
}

// Legacy API compatibility functions (deprecated, but kept for backward compatibility)

/// Get power reserve level (legacy function, same as get_power_reserve_level_direct)
///
/// # Deprecated
///
/// Use `get_power_reserve_level_direct()` instead. This function is kept for
/// backward compatibility but may be removed in future versions.
pub async fn get_power_reserve_level() -> PwrzvResult<u8> {
    get_power_reserve_level_direct().await
}

/// Get power reserve level with details (legacy function, same as get_power_reserve_level_with_details_direct)
///
/// # Deprecated
///
/// Use `get_power_reserve_level_with_details_direct()` instead. This function is kept for
/// backward compatibility but may be removed in future versions.
pub async fn get_power_reserve_level_with_details() -> PwrzvResult<(u8, HashMap<String, u8>)> {
    get_power_reserve_level_with_details_direct().await
}

/// Check if the current platform is supported
///
/// # Returns
///
/// `Ok(())` if the platform is supported, `Err(PwrzvError)` otherwise.
///
/// # Example
///
/// ```rust
/// use pwrzv::check_platform;
///
/// match check_platform() {
///     Ok(()) => println!("Platform is supported"),
///     Err(e) => eprintln!("Platform not supported: {}", e),
/// }
/// ```
pub fn check_platform() -> PwrzvResult<()> {
    Calculator::new().map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_power_reserve_level_enum() {
        // Test enum values
        assert_eq!(PowerReserveLevel::Abundant as u8, 5);
        assert_eq!(PowerReserveLevel::High as u8, 4);
        assert_eq!(PowerReserveLevel::Medium as u8, 3);
        assert_eq!(PowerReserveLevel::Low as u8, 2);
        assert_eq!(PowerReserveLevel::Critical as u8, 1);
    }

    #[test]
    fn test_power_reserve_level_try_from() {
        // Test valid conversions
        assert_eq!(
            PowerReserveLevel::try_from(5).unwrap(),
            PowerReserveLevel::Abundant
        );
        assert_eq!(
            PowerReserveLevel::try_from(4).unwrap(),
            PowerReserveLevel::High
        );
        assert_eq!(
            PowerReserveLevel::try_from(3).unwrap(),
            PowerReserveLevel::Medium
        );
        assert_eq!(
            PowerReserveLevel::try_from(2).unwrap(),
            PowerReserveLevel::Low
        );
        assert_eq!(
            PowerReserveLevel::try_from(1).unwrap(),
            PowerReserveLevel::Critical
        );

        // Test invalid conversions
        assert!(PowerReserveLevel::try_from(0).is_err());
        assert!(PowerReserveLevel::try_from(6).is_err());
        assert!(PowerReserveLevel::try_from(255).is_err());
    }

    #[test]
    fn test_power_reserve_level_display() {
        assert_eq!(format!("{}", PowerReserveLevel::Abundant), "Abundant");
        assert_eq!(format!("{}", PowerReserveLevel::High), "High");
        assert_eq!(format!("{}", PowerReserveLevel::Medium), "Medium");
        assert_eq!(format!("{}", PowerReserveLevel::Low), "Low");
        assert_eq!(format!("{}", PowerReserveLevel::Critical), "Critical");
    }

    #[test]
    fn test_calculator_creation() {
        // Calculator should be created successfully on supported platforms
        let calculator = Calculator::new();
        assert!(
            calculator.is_ok(),
            "Calculator creation should succeed on supported platforms"
        );
    }

    #[test]
    fn test_get_platform_name() {
        let platform = get_platform_name();
        assert!(!platform.is_empty(), "Platform name should not be empty");

        // Should be one of the supported platforms
        assert!(
            platform == "linux" || platform == "macos",
            "Platform should be Linux or macos, got: {platform}"
        );
    }

    #[test]
    fn test_check_platform() {
        // Should succeed on supported platforms
        let result = check_platform();
        assert!(
            result.is_ok(),
            "Platform check should succeed on supported platforms"
        );
    }

    #[tokio::test]
    async fn test_get_power_reserve_level_direct() {
        // Should return a valid level
        let result = get_power_reserve_level_direct().await;
        assert!(
            result.is_ok(),
            "get_power_reserve_level_direct should succeed"
        );

        let level = result.unwrap();
        assert!(
            (1..=5).contains(&level),
            "Level should be in range [1, 5], got: {level}"
        );
    }

    #[tokio::test]
    async fn test_get_power_reserve_level_with_details_direct() {
        // Should return valid level and details
        let result = get_power_reserve_level_with_details_direct().await;
        assert!(
            result.is_ok(),
            "get_power_reserve_level_with_details_direct should succeed"
        );

        let (level, details) = result.unwrap();
        assert!(
            (1..=5).contains(&level),
            "Level should be in range [1, 5], got: {level}"
        );

        // All detail scores should be in valid range
        for (key, score) in &details {
            assert!(
                *score >= 1 && *score <= 5,
                "Score for '{key}' should be in range [1, 5], got: {score}"
            );
        }
    }

    #[tokio::test]
    async fn test_legacy_compatibility_functions() {
        // Test legacy functions still work
        let result1 = get_power_reserve_level().await;
        assert!(
            result1.is_ok(),
            "Legacy get_power_reserve_level should work"
        );

        let result2 = get_power_reserve_level_with_details().await;
        assert!(
            result2.is_ok(),
            "Legacy get_power_reserve_level_with_details should work"
        );

        // Both functions should return valid results
        let legacy_level = result1.unwrap();
        let (legacy_detail_level, legacy_details) = result2.unwrap();

        assert!(
            (1..=5).contains(&legacy_level),
            "Legacy level should be in range [1, 5], got: {legacy_level}"
        );
        assert!(
            (1..=5).contains(&legacy_detail_level),
            "Legacy detail level should be in range [1, 5], got: {legacy_detail_level}"
        );

        // All detail scores should be in valid range
        for (key, score) in &legacy_details {
            assert!(
                *score >= 1 && *score <= 5,
                "Score for '{key}' should be in range [1, 5], got: {score}"
            );
        }
    }

    #[test]
    fn test_power_reserve_level_ordering() {
        // Test that levels have correct numeric values for comparison
        assert!(PowerReserveLevel::Abundant as u8 > PowerReserveLevel::High as u8);
        assert!(PowerReserveLevel::High as u8 > PowerReserveLevel::Medium as u8);
        assert!(PowerReserveLevel::Medium as u8 > PowerReserveLevel::Low as u8);
        assert!(PowerReserveLevel::Low as u8 > PowerReserveLevel::Critical as u8);
    }
}
