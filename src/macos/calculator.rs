use super::metrics::MacSystemMetrics;
use crate::error::PwrzvResult;
use crate::sigmoid::SigmoidFn;
use crate::{PowerReserveLevel, PowerReserveProvider, PwrzvError};
use std::collections::HashMap;
use std::env;

// ================================
// Environment variable helper for SigmoidFn configuration
// ================================

/// Create SigmoidFn from environment variables with fallback to defaults
fn get_sigmoid_config(
    env_prefix: &str,
    default_midpoint: f32,
    default_steepness: f32,
) -> SigmoidFn {
    let midpoint_env = format!("{env_prefix}_MIDPOINT");
    let steepness_env = format!("{env_prefix}_STEEPNESS");

    let midpoint = env::var(&midpoint_env)
        .ok()
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(default_midpoint);

    let steepness = env::var(&steepness_env)
        .ok()
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(default_steepness);

    SigmoidFn {
        midpoint,
        steepness,
    }
}

// ================================
// The core parameters of the macOS power reserve calculator
// ================================

/// Get CPU usage configuration (env: PWRZV_MACOS_CPU_USAGE_MIDPOINT, PWRZV_MACOS_CPU_USAGE_STEEPNESS)
fn get_cpu_usage_config() -> SigmoidFn {
    get_sigmoid_config("PWRZV_MACOS_CPU_USAGE", 0.60, 8.0)
}

/// Get CPU load configuration (env: PWRZV_MACOS_CPU_LOAD_MIDPOINT, PWRZV_MACOS_CPU_LOAD_STEEPNESS)
fn get_cpu_load_config() -> SigmoidFn {
    get_sigmoid_config("PWRZV_MACOS_CPU_LOAD", 1.2, 5.0)
}

/// Get memory usage configuration (env: PWRZV_MACOS_MEMORY_USAGE_MIDPOINT, PWRZV_MACOS_MEMORY_USAGE_STEEPNESS)
fn get_memory_usage_config() -> SigmoidFn {
    get_sigmoid_config("PWRZV_MACOS_MEMORY_USAGE", 0.85, 20.0)
}

/// Get memory compressed configuration (env: PWRZV_MACOS_MEMORY_COMPRESSED_MIDPOINT, PWRZV_MACOS_MEMORY_COMPRESSED_STEEPNESS)
fn get_memory_compressed_config() -> SigmoidFn {
    get_sigmoid_config("PWRZV_MACOS_MEMORY_COMPRESSED", 0.60, 15.0)
}

/// Get disk I/O configuration (env: PWRZV_MACOS_DISK_IO_MIDPOINT, PWRZV_MACOS_DISK_IO_STEEPNESS)
fn get_disk_io_config() -> SigmoidFn {
    get_sigmoid_config("PWRZV_MACOS_DISK_IO", 0.70, 10.0)
}

/// Get network bandwidth configuration (env: PWRZV_MACOS_NETWORK_MIDPOINT, PWRZV_MACOS_NETWORK_STEEPNESS)
fn get_network_config() -> SigmoidFn {
    get_sigmoid_config("PWRZV_MACOS_NETWORK", 0.80, 6.0)
}

/// Get network dropped packets configuration (env: PWRZV_MACOS_NETWORK_DROPPED_MIDPOINT, PWRZV_MACOS_NETWORK_DROPPED_STEEPNESS)
fn get_network_dropped_config() -> SigmoidFn {
    get_sigmoid_config("PWRZV_MACOS_NETWORK_DROPPED", 0.01, 50.0)
}

/// Get file descriptor configuration (env: PWRZV_MACOS_FD_MIDPOINT, PWRZV_MACOS_FD_STEEPNESS)
fn get_fd_config() -> SigmoidFn {
    get_sigmoid_config("PWRZV_MACOS_FD", 0.90, 30.0)
}

/// Get process count configuration (env: PWRZV_MACOS_PROCESS_MIDPOINT, PWRZV_MACOS_PROCESS_STEEPNESS)
fn get_process_config() -> SigmoidFn {
    get_sigmoid_config("PWRZV_MACOS_PROCESS", 0.80, 12.0)
}

// ================================

/// macOS power reserve provider
pub struct MacProvider;

impl PowerReserveProvider for MacProvider {
    async fn get_power_reserve_level(&self) -> PwrzvResult<u8> {
        let metrics = MacSystemMetrics::collect().await?;

        // Validate metrics data
        if !metrics.validate() {
            return Err(crate::error::PwrzvError::collection_error(
                "Invalid metrics data",
            ));
        }

        let (level, _) = Self::calculate(&metrics)?;
        Ok(level as u8)
    }

    async fn get_power_reserve_level_with_details(
        &self,
    ) -> PwrzvResult<(u8, HashMap<String, f32>)> {
        let metrics = MacSystemMetrics::collect().await?;

        // Validate metrics data
        if !metrics.validate() {
            return Err(crate::error::PwrzvError::collection_error(
                "Invalid metrics data",
            ));
        }

        let (level, details) = Self::calculate(&metrics)?;
        Ok((level as u8, details))
    }
}

impl MacProvider {
    /// Calculate the power reserve level and details
    ///
    /// # Arguments
    ///
    /// * `metrics` - The system metrics
    ///
    /// # Returns
    ///
    /// * `level` - The power reserve level
    /// * `details` - The details of the power reserve level
    fn calculate(
        metrics: &MacSystemMetrics,
    ) -> PwrzvResult<(PowerReserveLevel, HashMap<String, f32>)> {
        // calculate the score of each metric
        let cpu_usage_score = get_cpu_usage_config().evaluate(metrics.cpu_usage_ratio);
        let cpu_load_score = get_cpu_load_config().evaluate(metrics.cpu_load_ratio);
        let memory_usage_score = get_memory_usage_config().evaluate(metrics.memory_usage_ratio);
        let memory_compressed_score =
            get_memory_compressed_config().evaluate(metrics.memory_compressed_ratio);
        let disk_io_score = get_disk_io_config().evaluate(metrics.disk_io_ratio);
        let network_score = get_network_config().evaluate(metrics.network_bandwidth_ratio);
        let network_dropped_score =
            get_network_dropped_config().evaluate(metrics.network_dropped_packets_ratio);
        let fd_score = get_fd_config().evaluate(metrics.fd_usage_ratio);
        let process_score = get_process_config().evaluate(metrics.process_count_ratio);

        // build the details
        let mut details = HashMap::new();

        // Add raw metrics
        details.insert("cpu_usage_ratio".to_string(), metrics.cpu_usage_ratio);
        details.insert("cpu_load_ratio".to_string(), metrics.cpu_load_ratio);
        details.insert("memory_usage_ratio".to_string(), metrics.memory_usage_ratio);
        details.insert(
            "memory_compressed_ratio".to_string(),
            metrics.memory_compressed_ratio,
        );
        details.insert("disk_io_ratio".to_string(), metrics.disk_io_ratio);
        details.insert(
            "network_bandwidth_ratio".to_string(),
            metrics.network_bandwidth_ratio,
        );
        details.insert(
            "network_dropped_packets_ratio".to_string(),
            metrics.network_dropped_packets_ratio,
        );
        details.insert("fd_usage_ratio".to_string(), metrics.fd_usage_ratio);
        details.insert(
            "process_count_ratio".to_string(),
            metrics.process_count_ratio,
        );

        // Add calculated scores
        details.insert("cpu_usage_score".to_string(), cpu_usage_score);
        details.insert("cpu_load_score".to_string(), cpu_load_score);
        details.insert("memory_usage_score".to_string(), memory_usage_score);
        details.insert(
            "memory_compressed_score".to_string(),
            memory_compressed_score,
        );
        details.insert("disk_io_score".to_string(), disk_io_score);
        details.insert("network_score".to_string(), network_score);
        details.insert("network_dropped_score".to_string(), network_dropped_score);
        details.insert("fd_score".to_string(), fd_score);
        details.insert("process_score".to_string(), process_score);

        // final score = MAX(all scores) * 5
        if let Some(final_score) = [
            cpu_usage_score,
            cpu_load_score,
            memory_usage_score,
            memory_compressed_score,
            disk_io_score,
            network_score,
            network_dropped_score,
            fd_score,
            process_score,
        ]
        .iter()
        .max_by(|a, b| a.total_cmp(b))
        {
            let n = 5 - (final_score * 5.0) as u8; // n ∈ [0, 5]
            let level = PowerReserveLevel::try_from(n).unwrap_or(PowerReserveLevel::Abundant);
            return Ok((level, details));
        }
        // else
        Err(PwrzvError::collection_error("Something went wrong"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_power_level_calculation() {
        // Test high load situation (should trigger Critical with our sensitive parameters)
        let high_load_metrics = MacSystemMetrics {
            cpu_usage_ratio: 0.9,               // Very high CPU
            cpu_load_ratio: 1.8,                // High load ratio
            memory_usage_ratio: 0.85,           // At memory threshold
            memory_compressed_ratio: 0.6,       // Very high compression
            disk_io_ratio: 0.7,                 // High disk I/O
            network_bandwidth_ratio: 0.6,       // Moderate network
            network_dropped_packets_ratio: 0.0, // No packet loss
            fd_usage_ratio: 0.5,                // Normal FD usage
            process_count_ratio: 0.6,           // Normal process count
        };

        let (level, _) = MacProvider::calculate(&high_load_metrics).unwrap();
        // With our sensitive parameters, this should result in Critical or Low levels
        assert!(matches!(
            level,
            PowerReserveLevel::Critical | PowerReserveLevel::Low
        ));

        // Test normal load situation
        let normal_metrics = MacSystemMetrics {
            cpu_usage_ratio: 0.3,               // Normal CPU
            cpu_load_ratio: 0.8,                // Reasonable load
            memory_usage_ratio: 0.6,            // Normal memory
            memory_compressed_ratio: 0.2,       // Low compression
            disk_io_ratio: 0.2,                 // Low disk I/O
            network_bandwidth_ratio: 0.3,       // Low network
            network_dropped_packets_ratio: 0.0, // No packet loss
            fd_usage_ratio: 0.3,                // Low FD usage
            process_count_ratio: 0.4,           // Normal process count
        };

        let (level, _) = MacProvider::calculate(&normal_metrics).unwrap();
        // Should result in higher reserve levels
        assert!(matches!(
            level,
            PowerReserveLevel::Abundant | PowerReserveLevel::High | PowerReserveLevel::Medium
        ));
    }

    #[test]
    fn test_packet_loss_sensitivity() {
        // Test that packet loss triggers appropriate response
        let packet_loss_metrics = MacSystemMetrics {
            cpu_usage_ratio: 0.2,                // Low CPU
            cpu_load_ratio: 0.5,                 // Low load
            memory_usage_ratio: 0.4,             // Low memory
            memory_compressed_ratio: 0.1,        // Minimal compression
            disk_io_ratio: 0.1,                  // Minimal I/O
            network_bandwidth_ratio: 0.2,        // Low bandwidth usage
            network_dropped_packets_ratio: 0.05, // 5% packet loss!
            fd_usage_ratio: 0.2,                 // Low FD usage
            process_count_ratio: 0.3,            // Low process count
        };

        let (level, details) = MacProvider::calculate(&packet_loss_metrics).unwrap();

        // Even with low other metrics, packet loss should drive down the reserve level
        let network_dropped_score = details.get("network_dropped_score").unwrap();
        assert!(*network_dropped_score > 0.8); // Should be very high score (indicating problems)

        assert!(matches!(
            level,
            PowerReserveLevel::Critical | PowerReserveLevel::Low | PowerReserveLevel::Medium
        ));
    }

    #[test]
    fn test_sigmoid_parameter_tuning() {
        // Test memory compression sensitivity
        let metrics = MacSystemMetrics {
            cpu_usage_ratio: 0.1,
            cpu_load_ratio: 0.3,
            memory_usage_ratio: 0.4,
            memory_compressed_ratio: 0.65, // Above our 60% threshold
            disk_io_ratio: 0.1,
            network_bandwidth_ratio: 0.1,
            network_dropped_packets_ratio: 0.0,
            fd_usage_ratio: 0.1,
            process_count_ratio: 0.2,
        };

        let (_, details) = MacProvider::calculate(&metrics).unwrap();
        let memory_compressed_score = details.get("memory_compressed_score").unwrap();

        // Should show significant pressure due to compression
        assert!(*memory_compressed_score > 0.6);
    }

    #[test]
    fn test_environment_variable_configuration() {
        unsafe {
            // Set environment variables to test custom configuration
            env::set_var("PWRZV_MACOS_CPU_USAGE_MIDPOINT", "0.50");
            env::set_var("PWRZV_MACOS_CPU_USAGE_STEEPNESS", "15.0");

            let config = get_cpu_usage_config();
            assert_eq!(config.midpoint, 0.50);
            assert_eq!(config.steepness, 15.0);

            // Test that non-existent env vars use defaults
            env::remove_var("PWRZV_MACOS_MEMORY_USAGE_MIDPOINT");
            env::remove_var("PWRZV_MACOS_MEMORY_USAGE_STEEPNESS");

            let default_config = get_memory_usage_config();
            assert_eq!(default_config.midpoint, 0.85); // Default value
            assert_eq!(default_config.steepness, 20.0); // Default value

            // Test invalid env vars fall back to defaults
            env::set_var("PWRZV_MACOS_DISK_IO_MIDPOINT", "invalid_float");
            env::set_var("PWRZV_MACOS_DISK_IO_STEEPNESS", "not_a_number");

            let fallback_config = get_disk_io_config();
            assert_eq!(fallback_config.midpoint, 0.70); // Default value
            assert_eq!(fallback_config.steepness, 10.0); // Default value

            // Clean up
            env::remove_var("PWRZV_MACOS_CPU_USAGE_MIDPOINT");
            env::remove_var("PWRZV_MACOS_CPU_USAGE_STEEPNESS");
            env::remove_var("PWRZV_MACOS_DISK_IO_MIDPOINT");
            env::remove_var("PWRZV_MACOS_DISK_IO_STEEPNESS");
        }
    }

    #[test]
    fn test_realistic_scenarios() {
        // I/O bottleneck scenario
        let io_bottleneck = MacSystemMetrics {
            cpu_usage_ratio: 0.4,
            cpu_load_ratio: 2.0, // High load due to I/O wait
            memory_usage_ratio: 0.7,
            memory_compressed_ratio: 0.3,
            disk_io_ratio: 0.95, // Nearly saturated I/O
            network_bandwidth_ratio: 0.2,
            network_dropped_packets_ratio: 0.0,
            fd_usage_ratio: 0.4,
            process_count_ratio: 0.5,
        };

        let (level, _) = MacProvider::calculate(&io_bottleneck).unwrap();
        assert!(matches!(
            level,
            PowerReserveLevel::Critical | PowerReserveLevel::Low
        ));

        // Memory pressure scenario
        let memory_pressure = MacSystemMetrics {
            cpu_usage_ratio: 0.3,
            cpu_load_ratio: 0.8,
            memory_usage_ratio: 0.92,     // Very high memory usage
            memory_compressed_ratio: 0.8, // Heavy compression
            disk_io_ratio: 0.3,
            network_bandwidth_ratio: 0.2,
            network_dropped_packets_ratio: 0.0,
            fd_usage_ratio: 0.3,
            process_count_ratio: 0.4,
        };

        let (level, _) = MacProvider::calculate(&memory_pressure).unwrap();
        assert!(matches!(
            level,
            PowerReserveLevel::Critical | PowerReserveLevel::Low
        ));

        // Balanced but high load
        let balanced_high = MacSystemMetrics {
            cpu_usage_ratio: 0.7,
            cpu_load_ratio: 1.1,
            memory_usage_ratio: 0.8,
            memory_compressed_ratio: 0.5,
            disk_io_ratio: 0.6,
            network_bandwidth_ratio: 0.7,
            network_dropped_packets_ratio: 0.001, // Very minimal packet loss
            fd_usage_ratio: 0.6,
            process_count_ratio: 0.7,
        };

        let (level, _) = MacProvider::calculate(&balanced_high).unwrap();
        assert!(matches!(
            level,
            PowerReserveLevel::Low | PowerReserveLevel::Medium | PowerReserveLevel::High
        ));
    }
}
