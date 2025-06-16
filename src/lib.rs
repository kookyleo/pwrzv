//! # pwrzv - Linux System Power Reserve Meter
//!
//! A Rolls-Royce–inspired performance reserve meter for Linux systems.
//!
//! This library provides real-time monitoring and evaluation of Linux system resources,
//! mimicking the Power Reserve gauge in Rolls-Royce cars to calculate system's remaining
//! performance headroom.
//!
//! ## Features
//!
//! - CPU usage and I/O wait monitoring
//! - Memory and swap usage tracking
//! - Disk and network I/O monitoring
//! - File descriptor usage statistics
//! - Intelligent scoring algorithm based on Sigmoid functions
//!
//! ## Platform Support
//!
//! This library only supports Linux-like systems. Other platforms will return errors.
//!
//! ## Usage Examples
//!
//! ### Basic Usage
//!
//! ```rust,no_run
//! use pwrzv::{PowerReserveCalculator, PwrzvError};
//!
//! fn main() -> Result<(), PwrzvError> {
//!     let calculator = PowerReserveCalculator::new();
//!     let metrics = calculator.collect_metrics()?;
//!     let score = calculator.calculate_power_reserve(&metrics)?;
//!     println!("Power Reserve Score: {}", score);
//!     Ok(())
//! }
//! ```
//!
//! ### Detailed Analysis
//!
//! ```rust,no_run
//! use pwrzv::{PowerReserveCalculator, PwrzvError};
//!
//! fn main() -> Result<(), PwrzvError> {
//!     let calculator = PowerReserveCalculator::new();
//!     let metrics = calculator.collect_metrics()?;
//!     let detailed = calculator.calculate_detailed_score(&metrics)?;
//!     
//!     println!("Overall Score: {} ({})", detailed.final_score, detailed.level);
//!     println!("Bottlenecks: {}", detailed.bottleneck);
//!     println!("CPU Score: {}", detailed.component_scores.cpu);
//!     Ok(())
//! }
//! ```
//!
//! ### Custom Configuration
//!
//! ```rust,no_run
//! use pwrzv::{PowerReserveCalculator, SigmoidConfig, PwrzvError};
//!
//! fn main() -> Result<(), PwrzvError> {
//!     let mut config = SigmoidConfig::default();
//!     config.cpu_threshold = 0.8;  // Adjust CPU threshold
//!     
//!     let calculator = PowerReserveCalculator::with_config(config);
//!     let metrics = calculator.collect_metrics()?;
//!     let score = calculator.calculate_power_reserve(&metrics)?;
//!     println!("Power Reserve Score: {}", score);
//!     Ok(())
//! }
//! ```

pub mod calculator;
pub mod error;
pub mod metrics;
pub mod platform;

pub use calculator::{ComponentScores, DetailedScore, PowerReserveCalculator, SigmoidConfig};
pub use error::{PwrzvError, PwrzvResult};
pub use metrics::SystemMetrics;
pub use platform::{check_platform, get_platform_name, is_linux_like};

use std::fmt;

/// Power reserve scoring levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PowerReserveLevel {
    /// 0-1: System under heavy load, severely limited performance
    Critical = 0,
    /// 2: Resource constrained, optimization recommended
    Low = 2,
    /// 3: Moderate resources, adequate performance
    Moderate = 3,
    /// 4: Ample resources, good performance
    Good = 4,
    /// 5: Abundant resources, excellent performance
    Excellent = 5,
}

impl PowerReserveLevel {
    /// Convert score to level
    pub fn from_score(score: u8) -> Self {
        match score {
            0..=1 => PowerReserveLevel::Critical,
            2 => PowerReserveLevel::Low,
            3 => PowerReserveLevel::Moderate,
            4 => PowerReserveLevel::Good,
            5 => PowerReserveLevel::Excellent,
            _ => PowerReserveLevel::Excellent,
        }
    }

    /// Get level description
    pub fn description(&self) -> &'static str {
        match self {
            PowerReserveLevel::Critical => "Critical - System under heavy load",
            PowerReserveLevel::Low => "Low - Resource constrained",
            PowerReserveLevel::Moderate => "Moderate - Adequate performance",
            PowerReserveLevel::Good => "Good - Ample resources",
            PowerReserveLevel::Excellent => "Excellent - Abundant resources",
        }
    }
}

impl fmt::Display for PowerReserveLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_power_reserve_level_from_score() {
        assert_eq!(
            PowerReserveLevel::from_score(0),
            PowerReserveLevel::Critical
        );
        assert_eq!(
            PowerReserveLevel::from_score(1),
            PowerReserveLevel::Critical
        );
        assert_eq!(PowerReserveLevel::from_score(2), PowerReserveLevel::Low);
        assert_eq!(
            PowerReserveLevel::from_score(3),
            PowerReserveLevel::Moderate
        );
        assert_eq!(PowerReserveLevel::from_score(4), PowerReserveLevel::Good);
        assert_eq!(
            PowerReserveLevel::from_score(5),
            PowerReserveLevel::Excellent
        );
        assert_eq!(
            PowerReserveLevel::from_score(10),
            PowerReserveLevel::Excellent
        ); // Out of range
    }

    #[test]
    fn test_power_reserve_level_display() {
        let level = PowerReserveLevel::Critical;
        assert!(level.to_string().contains("Critical"));
        assert!(level.to_string().contains("heavy load"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_full_integration() {
        // Full integration test (only runs on Linux)
        let calculator = PowerReserveCalculator::new();

        match calculator.collect_metrics() {
            Ok(metrics) => {
                assert!(metrics.validate());

                match calculator.calculate_power_reserve(&metrics) {
                    Ok(score) => {
                        assert!(score <= 5);
                        let level = PowerReserveLevel::from_score(score);
                        assert!(!level.description().is_empty());
                    }
                    Err(e) => {
                        println!("Warning: Calculate power reserve failed: {e}");
                    }
                }

                match calculator.calculate_detailed_score(&metrics) {
                    Ok(detailed) => {
                        assert!(detailed.final_score <= 5);
                        assert!(!detailed.bottleneck.is_empty());
                        assert!(detailed.component_scores.cpu <= 5);
                    }
                    Err(e) => {
                        println!("Warning: Calculate detailed score failed: {e}");
                    }
                }
            }
            Err(e) => {
                println!("Warning: Integration test failed to collect metrics: {e}");
            }
        }
    }
}
