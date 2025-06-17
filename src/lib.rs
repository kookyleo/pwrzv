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
trait PowerReserveProvider {
    /// Get current power reserve level
    async fn get_power_reserve_level(&self) -> PwrzvResult<u8>;
    /// Get current power reserve level and detailed information
    async fn get_power_reserve_level_with_details(&self)
    -> PwrzvResult<(u8, HashMap<String, f32>)>;
}

/// Platform-specific power reserve provider enum
pub enum Provider {
    #[cfg(target_os = "linux")]
    Linux(linux::LinuxProvider),
    #[cfg(target_os = "macos")]
    MacOS(macos::MacProvider),
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    Unsupported,
}

impl Provider {
    /// Get current power reserve level
    pub async fn get_power_reserve_level(&self) -> PwrzvResult<u8> {
        match self {
            #[cfg(target_os = "linux")]
            Provider::Linux(provider) => provider.get_power_reserve_level().await,
            #[cfg(target_os = "macos")]
            Provider::MacOS(provider) => provider.get_power_reserve_level().await,
            #[cfg(not(any(target_os = "linux", target_os = "macos")))]
            Provider::Unsupported => Err(PwrzvError::unsupported_platform(&format!(
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
            Provider::Linux(provider) => provider.get_power_reserve_level_with_details().await,
            #[cfg(target_os = "macos")]
            Provider::MacOS(provider) => provider.get_power_reserve_level_with_details().await,
            #[cfg(not(any(target_os = "linux", target_os = "macos")))]
            Provider::Unsupported => Err(PwrzvError::unsupported_platform(&format!(
                "Platform '{}' is not supported yet. Only Linux and macOS are supported for now.",
                std::env::consts::OS
            ))),
        }
    }
}

/// Get current platform power reserve provider
pub fn get_provider() -> Provider {
    #[cfg(target_os = "linux")]
    {
        Provider::Linux(linux::LinuxProvider)
    }
    #[cfg(target_os = "macos")]
    {
        Provider::MacOS(macos::MacProvider)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        Provider::Unsupported
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
    let provider = get_provider();
    provider.get_power_reserve_level().await
}

/// Get current system power reserve level and detailed information (convenience function)
pub async fn get_power_reserve_level_with_details() -> PwrzvResult<(u8, HashMap<String, f32>)> {
    check_platform()?;
    let provider = get_provider();
    provider.get_power_reserve_level_with_details().await
}
