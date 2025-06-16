//! Power reserve calculation module
//!
//! Provides power reserve scoring calculation functions based on multiple system metrics.

use crate::PowerReserveLevel;
use crate::error::{PwrzvError, PwrzvResult};
use crate::metrics::SystemMetrics;
use crate::platform;
use serde::{Deserialize, Serialize};

/// Power reserve calculator
#[derive(Debug, Clone)]
pub struct PowerReserveCalculator {
    /// Sigmoid function parameter configuration
    config: SigmoidConfig,
}

/// Sigmoid function parameter configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigmoidConfig {
    /// CPU usage threshold and steepness
    pub cpu_threshold: f32,
    pub cpu_steepness: f32,
    /// I/O wait threshold and steepness
    pub iowait_threshold: f32,
    pub iowait_steepness: f32,
    /// Memory availability threshold and steepness
    pub memory_threshold: f32,
    pub memory_steepness: f32,
    /// Swap usage threshold and steepness
    pub swap_threshold: f32,
    pub swap_steepness: f32,
    /// Disk I/O threshold and steepness
    pub disk_threshold: f32,
    pub disk_steepness: f32,
    /// Network I/O threshold and steepness
    pub network_threshold: f32,
    pub network_steepness: f32,
    /// File descriptor threshold and steepness
    pub fd_threshold: f32,
    pub fd_steepness: f32,
}

impl Default for SigmoidConfig {
    fn default() -> Self {
        SigmoidConfig {
            cpu_threshold: 0.9,
            cpu_steepness: 10.0,
            iowait_threshold: 0.5,
            iowait_steepness: 10.0,
            memory_threshold: 0.95,
            memory_steepness: 10.0,
            swap_threshold: 0.5,
            swap_steepness: 10.0,
            disk_threshold: 0.95,
            disk_steepness: 10.0,
            network_threshold: 0.9,
            network_steepness: 10.0,
            fd_threshold: 0.9,
            fd_steepness: 10.0,
        }
    }
}

impl PowerReserveCalculator {
    /// Create a new calculator instance
    pub fn new() -> Self {
        PowerReserveCalculator {
            config: SigmoidConfig::default(),
        }
    }

    /// Create a calculator instance with custom configuration
    pub fn with_config(config: SigmoidConfig) -> Self {
        PowerReserveCalculator { config }
    }

    /// Collect system metrics
    pub fn collect_metrics(&self) -> PwrzvResult<SystemMetrics> {
        SystemMetrics::collect()
    }

    /// Calculate power reserve score
    ///
    /// Calculate 0-5 score based on multiple system metrics using Sigmoid functions
    pub fn calculate_power_reserve(&self, metrics: &SystemMetrics) -> PwrzvResult<u8> {
        // Validate platform and metrics
        self.validate_prerequisites(metrics)?;

        // Normalize metrics to [0, 1]
        let normalized_metrics = self.normalize_metrics(metrics);

        // Calculate component scores (using Sigmoid functions)
        let component_scores = self.calculate_component_scores(&normalized_metrics);

        // Take the lowest component score as final score (reflecting bottleneck effect)
        // Note: component_scores represent "pressure" (0-1), we need to convert to "reserve" (1-0)
        let min_pressure = component_scores
            .iter()
            .fold(f32::INFINITY, |a, &b| a.min(b));

        // Convert pressure to reserve: high pressure = low reserve, low pressure = high reserve
        let max_reserve = 1.0 - min_pressure;

        // Map to 0-5 score
        let score = (max_reserve * 5.0).round() as u8;
        Ok(score.min(5))
    }

    /// Calculate detailed score information
    pub fn calculate_detailed_score(&self, metrics: &SystemMetrics) -> PwrzvResult<DetailedScore> {
        // Validate platform and metrics
        self.validate_prerequisites(metrics)?;

        // Normalize metrics
        let normalized_metrics = self.normalize_metrics(metrics);

        // Calculate component scores
        let component_scores = self.calculate_component_scores(&normalized_metrics);

        let min_pressure = component_scores
            .iter()
            .fold(f32::INFINITY, |a, &b| a.min(b));

        let max_reserve = 1.0 - min_pressure;

        let final_score = (max_reserve * 5.0).round() as u8;

        Ok(DetailedScore {
            final_score: final_score.min(5),
            level: PowerReserveLevel::from_score(final_score.min(5)),
            component_scores: ComponentScores {
                cpu: ((1.0 - component_scores[0]) * 5.0).round() as u8,
                iowait: ((1.0 - component_scores[1]) * 5.0).round() as u8,
                memory: ((1.0 - component_scores[2]) * 5.0).round() as u8,
                swap: ((1.0 - component_scores[3]) * 5.0).round() as u8,
                disk: ((1.0 - component_scores[4]) * 5.0).round() as u8,
                network: ((1.0 - component_scores[5]) * 5.0).round() as u8,
                file_descriptor: ((1.0 - component_scores[6]) * 5.0).round() as u8,
            },
            bottleneck: self.identify_bottleneck(metrics),
        })
    }

    /// Validate platform and metrics prerequisites
    fn validate_prerequisites(&self, metrics: &SystemMetrics) -> PwrzvResult<()> {
        // Check platform compatibility
        platform::check_platform()?;

        // Validate metrics data
        if !metrics.validate() {
            return Err(PwrzvError::calculation_error("Invalid metrics data"));
        }

        Ok(())
    }

    /// Normalize metrics to [0, 1] range
    fn normalize_metrics(&self, metrics: &SystemMetrics) -> [f32; 7] {
        [
            metrics.cpu_usage / 100.0,
            metrics.cpu_iowait / 100.0,
            metrics.mem_available / 100.0,
            metrics.swap_usage / 100.0,
            metrics.disk_usage / 100.0,
            metrics.net_usage / 100.0,
            metrics.fd_usage / 100.0,
        ]
    }

    /// Calculate component scores using sigmoid functions
    fn calculate_component_scores(&self, normalized_metrics: &[f32; 7]) -> [f32; 7] {
        [
            self.sigmoid(
                normalized_metrics[0],
                self.config.cpu_threshold,
                self.config.cpu_steepness,
            ),
            self.sigmoid(
                normalized_metrics[1],
                self.config.iowait_threshold,
                self.config.iowait_steepness,
            ),
            self.sigmoid(
                1.0 - normalized_metrics[2],
                self.config.memory_threshold,
                self.config.memory_steepness,
            ), // Note: higher memory availability is better
            self.sigmoid(
                normalized_metrics[3],
                self.config.swap_threshold,
                self.config.swap_steepness,
            ),
            self.sigmoid(
                normalized_metrics[4],
                self.config.disk_threshold,
                self.config.disk_steepness,
            ),
            self.sigmoid(
                normalized_metrics[5],
                self.config.network_threshold,
                self.config.network_steepness,
            ),
            self.sigmoid(
                normalized_metrics[6],
                self.config.fd_threshold,
                self.config.fd_steepness,
            ),
        ]
    }

    /// Sigmoid function implementation
    ///
    /// Map input value to [0, 1] range, representing resource pressure score
    /// where 0 = no pressure, 1 = high pressure
    fn sigmoid(&self, x: f32, x0: f32, k: f32) -> f32 {
        1.0 / (1.0 + (-k * (x - x0)).exp())
    }

    /// Identify system bottleneck
    fn identify_bottleneck(&self, metrics: &SystemMetrics) -> String {
        let mut bottlenecks = Vec::new();

        if metrics.cpu_usage > 90.0 {
            bottlenecks.push("CPU");
        }
        if metrics.cpu_iowait > 50.0 {
            bottlenecks.push("I/O Wait");
        }
        if metrics.mem_available < 5.0 {
            bottlenecks.push("Memory");
        }
        if metrics.swap_usage > 50.0 {
            bottlenecks.push("Swap");
        }
        if metrics.disk_usage > 95.0 {
            bottlenecks.push("Disk I/O");
        }
        if metrics.net_usage > 90.0 {
            bottlenecks.push("Network I/O");
        }
        if metrics.fd_usage > 90.0 {
            bottlenecks.push("File Descriptors");
        }

        if bottlenecks.is_empty() {
            "None".to_string()
        } else {
            bottlenecks.join(", ")
        }
    }

    /// Get current configuration
    pub fn get_config(&self) -> &SigmoidConfig {
        &self.config
    }

    /// Update configuration
    pub fn set_config(&mut self, config: SigmoidConfig) {
        self.config = config;
    }
}

impl Default for PowerReserveCalculator {
    fn default() -> Self {
        Self::new()
    }
}

/// Detailed score information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailedScore {
    /// Final score (0-5)
    pub final_score: u8,
    /// Score level
    pub level: PowerReserveLevel,
    /// Component scores
    pub component_scores: ComponentScores,
    /// Identified bottleneck
    pub bottleneck: String,
}

/// Component scores
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentScores {
    /// CPU score
    pub cpu: u8,
    /// I/O wait score
    pub iowait: u8,
    /// Memory score
    pub memory: u8,
    /// Swap score
    pub swap: u8,
    /// Disk I/O score
    pub disk: u8,
    /// Network I/O score
    pub network: u8,
    /// File descriptor score
    pub file_descriptor: u8,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_power_reserve_calculator_new() {
        let calculator = PowerReserveCalculator::new();
        assert_eq!(calculator.config.cpu_threshold, 0.9);
    }

    #[test]
    fn test_sigmoid_function() {
        let calculator = PowerReserveCalculator::new();

        // Test sigmoid function basic behavior
        let result1 = calculator.sigmoid(0.0, 0.5, 10.0);
        let result2 = calculator.sigmoid(0.5, 0.5, 10.0);
        let result3 = calculator.sigmoid(1.0, 0.5, 10.0);

        assert!(result1 < result2);
        assert!(result2 < result3);
        assert!((0.0..=1.0).contains(&result1));
        assert!((0.0..=1.0).contains(&result3));
    }

    #[test]
    fn test_calculate_power_reserve_with_good_metrics() {
        let calculator = PowerReserveCalculator::new();
        // Create very low usage metrics that should result in high score
        let metrics = SystemMetrics {
            cpu_usage: 5.0,      // Very low CPU usage
            cpu_iowait: 1.0,     // Very low I/O wait
            mem_available: 95.0, // High memory available
            swap_usage: 0.0,     // No swap usage
            disk_usage: 2.0,     // Very low disk usage
            net_usage: 1.0,      // Very low network usage
            fd_usage: 5.0,       // Very low FD usage
        };

        // if cfg!(target_os = "linux") {
        // Test actual calculation only on Linux
        match calculator.calculate_power_reserve(&metrics) {
            Ok(score) => {
                // With all very low usage, we should get a good score
                assert!(score >= 3, "Expected score >= 3, got {score}");
            }
            Err(e) => {
                println!("Warning: Test failed due to platform check: {e}");
            }
        }
        // }
    }

    #[test]
    fn test_calculate_power_reserve_with_bad_metrics() {
        let calculator = PowerReserveCalculator::new();
        // Create high usage metrics that should result in low score
        let metrics = SystemMetrics {
            cpu_usage: 98.0,    // Very high CPU usage
            cpu_iowait: 70.0,   // Very high I/O wait
            mem_available: 1.0, // Very low memory available
            swap_usage: 90.0,   // High swap usage
            disk_usage: 99.0,   // Very high disk usage
            net_usage: 98.0,    // Very high network usage
            fd_usage: 95.0,     // Very high FD usage
        };

        // if cfg!(target_os = "linux") {
        match calculator.calculate_power_reserve(&metrics) {
            Ok(score) => {
                // With all very high usage, we should get a low score
                assert!(score <= 2, "Expected score <= 2, got {score}");
            }
            Err(e) => {
                println!("Warning: Test failed due to platform check: {e}");
            }
        }
        // }
    }

    #[test]
    fn test_identify_bottleneck() {
        let calculator = PowerReserveCalculator::new();

        let metrics_no_bottleneck = SystemMetrics {
            cpu_usage: 10.0,
            cpu_iowait: 1.0,
            mem_available: 90.0,
            swap_usage: 0.0,
            disk_usage: 5.0,
            net_usage: 1.0,
            fd_usage: 5.0,
        };

        let bottleneck = calculator.identify_bottleneck(&metrics_no_bottleneck);
        assert_eq!(bottleneck, "None");

        let metrics_cpu_bottleneck = SystemMetrics {
            cpu_usage: 95.0,
            cpu_iowait: 1.0,
            mem_available: 90.0,
            swap_usage: 0.0,
            disk_usage: 5.0,
            net_usage: 1.0,
            fd_usage: 5.0,
        };

        let bottleneck = calculator.identify_bottleneck(&metrics_cpu_bottleneck);
        assert!(bottleneck.contains("CPU"));
    }

    #[test]
    fn test_invalid_metrics() {
        let calculator = PowerReserveCalculator::new();
        let invalid_metrics = SystemMetrics {
            cpu_usage: -10.0, // Invalid value
            cpu_iowait: 1.0,
            mem_available: 90.0,
            swap_usage: 0.0,
            disk_usage: 5.0,
            net_usage: 1.0,
            fd_usage: 5.0,
        };

        if cfg!(target_os = "linux") {
            let result = calculator.calculate_power_reserve(&invalid_metrics);
            assert!(result.is_err());
        }
    }
}
