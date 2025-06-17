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
//! use pwrzv::get_power_reserve_level;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let level = get_power_reserve_level().await?;
//!     println!("Power Reserve Level: {}", level);
//!     Ok(())
//! }
//! ```
//!
//! ### Detailed Analysis
//!
//! ```rust
//! use pwrzv::{get_power_reserve_level_with_details, PowerReserveLevel};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let (level, details) = get_power_reserve_level_with_details().await?;
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
//! The library uses a multi-stage processing pipeline:
//!
//! 1. **Platform Detection**: Automatically detects Linux or macOS
//! 2. **Metric Collection**: Gathers system metrics using platform-specific methods
//! 3. **Normalization**: Converts raw values to 0-1 scale
//! 4. **Sigmoid Transformation**: Applies configurable curves to each metric
//! 5. **Scoring**: Calculates final 0-5 power reserve score
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
//! use pwrzv::{get_power_reserve_level, PwrzvError};
//!
//! #[tokio::main]
//! async fn main() {
//!     match get_power_reserve_level().await {
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

pub mod error;
pub mod linux;
pub mod macos;
pub mod sigmoid;
pub use error::{PwrzvError, PwrzvResult};

/// Power Reserve Level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum PowerReserveLevel {
    /// Abundant: System resources are abundant, suitable for high-performance tasks
    Abundant = 5,
    /// High: System resources are sufficient, suitable for moderate-performance tasks
    High = 4,
    /// Medium: System resources are moderate, recommend light tasks
    Medium = 3,
    /// Low: System resources are limited, recommend reducing task execution
    Low = 2,
    /// Critical: System resources are extremely limited, recommend pausing non-essential tasks
    Critical = 1,
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

impl fmt::Display for PowerReserveLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PowerReserveLevel::Abundant => write!(f, "Abundant - System resources are abundant"),
            PowerReserveLevel::High => write!(f, "High - System resources are sufficient"),
            PowerReserveLevel::Medium => write!(f, "Medium - System resources are moderate"),
            PowerReserveLevel::Low => write!(f, "Low - System resources are limited"),
            PowerReserveLevel::Critical => write!(f, "Critical - System under heavy load"),
        }
    }
}

/// Power Reserve Provider trait
trait PowerReserveMeterProvider {
    /// Get current power reserve level
    async fn get_power_reserve_level(&self) -> PwrzvResult<u8>;
    /// Get current power reserve level and detailed information
    async fn get_power_reserve_level_with_details(&self)
    -> PwrzvResult<(u8, HashMap<String, f32>)>;
}

/// Platform-specific power reserve provider enum
pub enum MeterProvider {
    #[cfg(target_os = "linux")]
    Linux(linux::LinuxProvider),
    #[cfg(target_os = "macos")]
    MacOS(macos::MacProvider),
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    Unsupported,
}

impl MeterProvider {
    /// Get current power reserve level
    pub async fn get_power_reserve_level(&self) -> PwrzvResult<u8> {
        match self {
            #[cfg(target_os = "linux")]
            MeterProvider::Linux(provider) => provider.get_power_reserve_level().await,
            #[cfg(target_os = "macos")]
            MeterProvider::MacOS(provider) => provider.get_power_reserve_level().await,
            #[cfg(not(any(target_os = "linux", target_os = "macos")))]
            MeterProvider::Unsupported => Err(PwrzvError::unsupported_platform(&format!(
                "Platform '{}' is not supported yet. Only Linux and macOS are supported for now.",
                std::env::consts::OS
            ))),
        }
    }

    /// Get current power reserve level and detailed information
    pub async fn get_power_reserve_level_with_details(
        &self,
    ) -> PwrzvResult<(u8, HashMap<String, f32>)> {
        match self {
            #[cfg(target_os = "linux")]
            MeterProvider::Linux(provider) => provider.get_power_reserve_level_with_details().await,
            #[cfg(target_os = "macos")]
            MeterProvider::MacOS(provider) => provider.get_power_reserve_level_with_details().await,
            #[cfg(not(any(target_os = "linux", target_os = "macos")))]
            MeterProvider::Unsupported => Err(PwrzvError::unsupported_platform(&format!(
                "Platform '{}' is not supported yet. Only Linux and macOS are supported for now.",
                std::env::consts::OS
            ))),
        }
    }
}

/// Get current platform power reserve provider
pub fn get_meter_provider() -> MeterProvider {
    #[cfg(target_os = "linux")]
    {
        MeterProvider::Linux(linux::LinuxProvider)
    }
    #[cfg(target_os = "macos")]
    {
        MeterProvider::MacOS(macos::MacProvider)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        MeterProvider::Unsupported
    }
}

/// Check if current platform is supported
pub fn check_platform() -> PwrzvResult<()> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        Ok(())
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        Err(PwrzvError::unsupported_platform(&format!(
            "Platform '{}' is not supported yet. Only Linux and macOS are supported for now.",
            std::env::consts::OS
        )))
    }
}

/// Get current platform name
pub fn get_platform_name() -> &'static str {
    std::env::consts::OS
}

/// Get current system power reserve level (convenience function)
pub async fn get_power_reserve_level() -> PwrzvResult<u8> {
    check_platform()?;
    let provider = get_meter_provider();
    provider.get_power_reserve_level().await
}

/// Get current system power reserve level and detailed information (convenience function)
pub async fn get_power_reserve_level_with_details() -> PwrzvResult<(u8, HashMap<String, f32>)> {
    check_platform()?;
    let provider = get_meter_provider();
    provider.get_power_reserve_level_with_details().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_power_reserve_level_enum() {
        // Test all valid conversions
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
        assert_eq!(
            PowerReserveLevel::Abundant.to_string(),
            "Abundant - System resources are abundant"
        );
        assert_eq!(
            PowerReserveLevel::High.to_string(),
            "High - System resources are sufficient"
        );
        assert_eq!(
            PowerReserveLevel::Medium.to_string(),
            "Medium - System resources are moderate"
        );
        assert_eq!(
            PowerReserveLevel::Low.to_string(),
            "Low - System resources are limited"
        );
        assert_eq!(
            PowerReserveLevel::Critical.to_string(),
            "Critical - System under heavy load"
        );
    }

    #[test]
    fn test_power_reserve_level_serialization() {
        let level = PowerReserveLevel::High;
        let json = serde_json::to_string(&level).unwrap();
        assert_eq!(json, "\"High\"");

        let deserialized: PowerReserveLevel = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, PowerReserveLevel::High);
    }

    #[test]
    fn test_get_platform_name() {
        let platform = get_platform_name();
        assert!(!platform.is_empty());

        // Should be one of the supported platforms in CI/test environments
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            assert!(platform == "linux" || platform == "macos");
        }
    }

    #[test]
    fn test_check_platform() {
        let result = check_platform();

        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            assert!(result.is_ok());
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            assert!(result.is_err());
            match result.unwrap_err() {
                PwrzvError::UnsupportedPlatform { platform } => {
                    assert!(!platform.is_empty());
                }
                _ => panic!("Expected UnsupportedPlatform error"),
            }
        }
    }

    #[test]
    fn test_get_meter_provider() {
        let provider = get_meter_provider();

        #[cfg(target_os = "linux")]
        {
            assert!(matches!(provider, MeterProvider::Linux(_)));
        }

        #[cfg(target_os = "macos")]
        {
            assert!(matches!(provider, MeterProvider::MacOS(_)));
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            assert!(matches!(provider, MeterProvider::Unsupported));
        }
    }

    #[tokio::test]
    async fn test_get_power_reserve_level_api() {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            let result = get_power_reserve_level().await;
            match result {
                Ok(level) => {
                    assert!((1..=5).contains(&level));
                }
                Err(e) => {
                    // Allow collection errors in test environments where system access might be limited
                    match e {
                        PwrzvError::ResourceAccessError { .. } => {}
                        PwrzvError::IoError(_) => {}
                        PwrzvError::CalculationError { .. } => {}
                        _ => panic!("Unexpected error: {e}"),
                    }
                }
            }
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            let result = get_power_reserve_level().await;
            assert!(result.is_err());
            assert!(matches!(
                result.unwrap_err(),
                PwrzvError::UnsupportedPlatform { .. }
            ));
        }
    }

    #[tokio::test]
    async fn test_get_power_reserve_level_with_details_api() {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            let result = get_power_reserve_level_with_details().await;
            match result {
                Ok((level, details)) => {
                    assert!((1..=5).contains(&level));
                    assert!(!details.is_empty());

                    // Verify that all detail values are in reasonable ranges
                    for (key, value) in details {
                        assert!(value.is_finite(), "Metric {key} has invalid value: {value}");

                        // Ratio values should be between 0 and 1 (allowing some margin for edge cases)
                        if key.ends_with("_ratio") {
                            assert!(
                                (0.0..=1.5).contains(&value),
                                "Ratio metric {key} has invalid value: {value} (should be 0.0-1.0)"
                            );
                        }

                        // Score values should be between 0 and 1
                        if key.ends_with("_score") {
                            assert!(
                                (0.0..=1.0).contains(&value),
                                "Score metric {key} has invalid value: {value} (should be 0.0-1.0)"
                            );
                        }
                    }
                }
                Err(e) => {
                    // Allow collection errors in test environments
                    match e {
                        PwrzvError::ResourceAccessError { .. } => {}
                        PwrzvError::IoError(_) => {}
                        PwrzvError::CalculationError { .. } => {}
                        _ => panic!("Unexpected error: {e}"),
                    }
                }
            }
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            let result = get_power_reserve_level_with_details().await;
            assert!(result.is_err());
            assert!(matches!(
                result.unwrap_err(),
                PwrzvError::UnsupportedPlatform { .. }
            ));
        }
    }

    #[tokio::test]
    async fn test_meter_provider_consistency() {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            let provider = get_meter_provider();

            // Test that the provider methods are consistent
            let level_result = provider.get_power_reserve_level().await;
            let details_result = provider.get_power_reserve_level_with_details().await;

            match (level_result, details_result) {
                (Ok(level1), Ok((level2, _))) => {
                    // Allow some difference due to system state changes between calls
                    // but they should be close (within 1 level)
                    let diff = (level1 as i8 - level2 as i8).abs();
                    assert!(
                        diff <= 1,
                        "Provider methods return levels too far apart: {level1} vs {level2}"
                    );
                }
                (Err(_), Err(_)) => {
                    // Both failed - acceptable in test environments
                }
                _ => {
                    // One succeeded, one failed - potential issue but might be environmental
                }
            }
        }
    }
}
