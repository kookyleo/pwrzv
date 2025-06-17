use super::metrics::LinuxSystemMetrics;
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
// The core parameters of the Linux power reserve calculator
// ================================

/// Get CPU usage configuration (env: PWRZV_LINUX_CPU_USAGE_MIDPOINT, PWRZV_LINUX_CPU_USAGE_STEEPNESS)
fn get_cpu_usage_config() -> SigmoidFn {
    get_sigmoid_config("PWRZV_LINUX_CPU_USAGE", 0.65, 8.0)
}

/// Get CPU I/O wait configuration (env: PWRZV_LINUX_CPU_IOWAIT_MIDPOINT, PWRZV_LINUX_CPU_IOWAIT_STEEPNESS)
fn get_cpu_iowait_config() -> SigmoidFn {
    get_sigmoid_config("PWRZV_LINUX_CPU_IOWAIT", 0.20, 20.0)
}

/// Get CPU load configuration (env: PWRZV_LINUX_CPU_LOAD_MIDPOINT, PWRZV_LINUX_CPU_LOAD_STEEPNESS)
fn get_cpu_load_config() -> SigmoidFn {
    get_sigmoid_config("PWRZV_LINUX_CPU_LOAD", 1.2, 5.0)
}

/// Get memory usage configuration (env: PWRZV_LINUX_MEMORY_USAGE_MIDPOINT, PWRZV_LINUX_MEMORY_USAGE_STEEPNESS)
fn get_memory_usage_config() -> SigmoidFn {
    get_sigmoid_config("PWRZV_LINUX_MEMORY_USAGE", 0.85, 18.0)
}

/// Get memory pressure configuration (env: PWRZV_LINUX_MEMORY_PRESSURE_MIDPOINT, PWRZV_LINUX_MEMORY_PRESSURE_STEEPNESS)
fn get_memory_pressure_config() -> SigmoidFn {
    get_sigmoid_config("PWRZV_LINUX_MEMORY_PRESSURE", 0.30, 12.0)
}

/// Get disk I/O configuration (env: PWRZV_LINUX_DISK_IO_MIDPOINT, PWRZV_LINUX_DISK_IO_STEEPNESS)
fn get_disk_io_config() -> SigmoidFn {
    get_sigmoid_config("PWRZV_LINUX_DISK_IO", 0.70, 10.0)
}

/// Get network bandwidth configuration (env: PWRZV_LINUX_NETWORK_MIDPOINT, PWRZV_LINUX_NETWORK_STEEPNESS)
fn get_network_config() -> SigmoidFn {
    get_sigmoid_config("PWRZV_LINUX_NETWORK", 0.80, 6.0)
}

/// Get network dropped packets configuration (env: PWRZV_LINUX_NETWORK_DROPPED_MIDPOINT, PWRZV_LINUX_NETWORK_DROPPED_STEEPNESS)
fn get_network_dropped_config() -> SigmoidFn {
    get_sigmoid_config("PWRZV_LINUX_NETWORK_DROPPED", 0.01, 50.0)
}

/// Get file descriptor configuration (env: PWRZV_LINUX_FD_MIDPOINT, PWRZV_LINUX_FD_STEEPNESS)
fn get_fd_config() -> SigmoidFn {
    get_sigmoid_config("PWRZV_LINUX_FD", 0.90, 25.0)
}

/// Get process count configuration (env: PWRZV_LINUX_PROCESS_MIDPOINT, PWRZV_LINUX_PROCESS_STEEPNESS)
fn get_process_config() -> SigmoidFn {
    get_sigmoid_config("PWRZV_LINUX_PROCESS", 0.80, 12.0)
}

// ================================

/// Linux power reserve provider
pub struct LinuxProvider;

impl PowerReserveProvider for LinuxProvider {
    async fn get_power_reserve_level(&self) -> PwrzvResult<u8> {
        let metrics = LinuxSystemMetrics::collect().await?;

        // Validate metrics data
        if !metrics.validate() {
            return Err(PwrzvError::collection_error("Invalid metrics data"));
        }

        let (level, _) = Self::calculate(&metrics)?;
        Ok(level as u8)
    }

    async fn get_power_reserve_level_with_details(
        &self,
    ) -> PwrzvResult<(u8, HashMap<String, f32>)> {
        let metrics = LinuxSystemMetrics::collect().await?;

        // Validate metrics data
        if !metrics.validate() {
            return Err(PwrzvError::collection_error("Invalid metrics data"));
        }

        let (level, details) = Self::calculate(&metrics)?;
        Ok((level as u8, details))
    }
}

impl LinuxProvider {
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
        metrics: &LinuxSystemMetrics,
    ) -> PwrzvResult<(PowerReserveLevel, HashMap<String, f32>)> {
        // calculate the score of each metric
        let cpu_usage_score = get_cpu_usage_config().evaluate(metrics.cpu_usage_ratio);
        let cpu_iowait_score = get_cpu_iowait_config().evaluate(metrics.cpu_io_wait_ratio);
        let cpu_load_score = get_cpu_load_config().evaluate(metrics.cpu_load_ratio);
        let memory_usage_score = get_memory_usage_config().evaluate(metrics.memory_usage_ratio);
        let memory_pressure_score =
            get_memory_pressure_config().evaluate(metrics.memory_pressure_ratio);
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
        details.insert("cpu_io_wait_ratio".to_string(), metrics.cpu_io_wait_ratio);
        details.insert("cpu_load_ratio".to_string(), metrics.cpu_load_ratio);
        details.insert("memory_usage_ratio".to_string(), metrics.memory_usage_ratio);
        details.insert(
            "memory_pressure_ratio".to_string(),
            metrics.memory_pressure_ratio,
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
        details.insert("cpu_iowait_score".to_string(), cpu_iowait_score);
        details.insert("cpu_load_score".to_string(), cpu_load_score);
        details.insert("memory_usage_score".to_string(), memory_usage_score);
        details.insert("memory_pressure_score".to_string(), memory_pressure_score);
        details.insert("disk_io_score".to_string(), disk_io_score);
        details.insert("network_score".to_string(), network_score);
        details.insert("network_dropped_score".to_string(), network_dropped_score);
        details.insert("fd_score".to_string(), fd_score);
        details.insert("process_score".to_string(), process_score);

        // final score = MAX(all scores) * 5
        if let Some(final_score) = [
            cpu_usage_score,
            cpu_iowait_score,
            cpu_load_score,
            memory_usage_score,
            memory_pressure_score,
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
        // Test high load situation (should trigger Critical/Low with sensitive parameters)
        let high_load_metrics = LinuxSystemMetrics {
            cpu_usage_ratio: 0.95,              // Very high CPU
            cpu_io_wait_ratio: 0.5,             // Very high I/O wait
            cpu_load_ratio: 2.0,                // High load ratio
            memory_usage_ratio: 0.9,            // Very high memory usage
            memory_pressure_ratio: 0.8,         // High memory pressure
            disk_io_ratio: 0.8,                 // High disk I/O
            network_bandwidth_ratio: 0.7,       // High network
            network_dropped_packets_ratio: 0.0, // No packet loss
            fd_usage_ratio: 0.6,                // Normal FD usage
            process_count_ratio: 0.7,           // Normal process count
        };

        let (level, _) = LinuxProvider::calculate(&high_load_metrics).unwrap();
        // With these extreme parameters, should trigger Critical/Low
        assert!(matches!(
            level,
            PowerReserveLevel::Critical | PowerReserveLevel::Low
        ));

        // Test normal load situation
        let normal_metrics = LinuxSystemMetrics {
            cpu_usage_ratio: 0.4,               // Normal CPU
            cpu_io_wait_ratio: 0.05,            // Low I/O wait
            cpu_load_ratio: 0.8,                // Reasonable load
            memory_usage_ratio: 0.6,            // Normal memory
            memory_pressure_ratio: 0.1,         // Low memory pressure
            disk_io_ratio: 0.3,                 // Low disk I/O
            network_bandwidth_ratio: 0.2,       // Low network
            network_dropped_packets_ratio: 0.0, // No packet loss
            fd_usage_ratio: 0.4,                // Normal FD usage
            process_count_ratio: 0.5,           // Normal process count
        };

        let (level, _) = LinuxProvider::calculate(&normal_metrics).unwrap();
        // Should result in higher reserve levels
        assert!(matches!(
            level,
            PowerReserveLevel::Abundant | PowerReserveLevel::High | PowerReserveLevel::Medium
        ));
    }

    #[test]
    fn test_packet_loss_sensitivity() {
        // Test that packet loss triggers appropriate response
        let packet_loss_metrics = LinuxSystemMetrics {
            cpu_usage_ratio: 0.3,                // Low CPU
            cpu_io_wait_ratio: 0.02,             // Very low I/O wait
            cpu_load_ratio: 0.6,                 // Low load
            memory_usage_ratio: 0.5,             // Low memory
            memory_pressure_ratio: 0.05,         // Minimal memory pressure
            disk_io_ratio: 0.1,                  // Minimal I/O
            network_bandwidth_ratio: 0.3,        // Low bandwidth usage
            network_dropped_packets_ratio: 0.03, // 3% packet loss!
            fd_usage_ratio: 0.3,                 // Low FD usage
            process_count_ratio: 0.4,            // Low process count
        };

        let (level, details) = LinuxProvider::calculate(&packet_loss_metrics).unwrap();

        // Even with low other metrics, packet loss should drive down the reserve level
        let network_dropped_score = details.get("network_dropped_score").unwrap();
        assert!(*network_dropped_score > 0.6); // Should be significant score (indicating problems)

        assert!(matches!(
            level,
            PowerReserveLevel::Critical | PowerReserveLevel::Low
        ));
    }

    #[test]
    fn test_sigmoid_parameter_tuning() {
        // Test I/O wait sensitivity (Linux-specific)
        let io_wait_metrics = LinuxSystemMetrics {
            cpu_usage_ratio: 0.2,
            cpu_io_wait_ratio: 0.25, // Above our 20% threshold
            cpu_load_ratio: 0.5,
            memory_usage_ratio: 0.4,
            memory_pressure_ratio: 0.1,
            disk_io_ratio: 0.3,
            network_bandwidth_ratio: 0.2,
            network_dropped_packets_ratio: 0.0,
            fd_usage_ratio: 0.2,
            process_count_ratio: 0.3,
        };

        let (_, details) = LinuxProvider::calculate(&io_wait_metrics).unwrap();
        let cpu_iowait_score = details.get("cpu_iowait_score").unwrap();

        // Should show significant impact due to I/O wait
        assert!(*cpu_iowait_score > 0.7);

        // Test memory pressure sensitivity (Linux PSI)
        let memory_pressure_metrics = LinuxSystemMetrics {
            cpu_usage_ratio: 0.2,
            cpu_io_wait_ratio: 0.05,
            cpu_load_ratio: 0.5,
            memory_usage_ratio: 0.6,
            memory_pressure_ratio: 0.4, // Above our 30% threshold
            disk_io_ratio: 0.2,
            network_bandwidth_ratio: 0.1,
            network_dropped_packets_ratio: 0.0,
            fd_usage_ratio: 0.2,
            process_count_ratio: 0.3,
        };

        let (_, details) = LinuxProvider::calculate(&memory_pressure_metrics).unwrap();
        let memory_pressure_score = details.get("memory_pressure_score").unwrap();

        // Should show significant pressure
        assert!(*memory_pressure_score > 0.6);
    }

    #[test]
    fn test_environment_variable_configuration() {
        unsafe {
            // Set environment variables to test custom configuration
            env::set_var("PWRZV_LINUX_CPU_IOWAIT_MIDPOINT", "0.15");
            env::set_var("PWRZV_LINUX_CPU_IOWAIT_STEEPNESS", "25.0");

            let config = get_cpu_iowait_config();
            assert_eq!(config.midpoint, 0.15);
            assert_eq!(config.steepness, 25.0);

            // Test that non-existent env vars use defaults
            env::remove_var("PWRZV_LINUX_MEMORY_PRESSURE_MIDPOINT");
            env::remove_var("PWRZV_LINUX_MEMORY_PRESSURE_STEEPNESS");

            let default_config = get_memory_pressure_config();
            assert_eq!(default_config.midpoint, 0.30); // Default value
            assert_eq!(default_config.steepness, 12.0); // Default value

            // Test invalid env vars fall back to defaults
            env::set_var("PWRZV_LINUX_NETWORK_DROPPED_MIDPOINT", "invalid");
            env::set_var("PWRZV_LINUX_NETWORK_DROPPED_STEEPNESS", "not_a_float");

            let fallback_config = get_network_dropped_config();
            assert_eq!(fallback_config.midpoint, 0.01); // Default value
            assert_eq!(fallback_config.steepness, 50.0); // Default value

            // Clean up
            env::remove_var("PWRZV_LINUX_CPU_IOWAIT_MIDPOINT");
            env::remove_var("PWRZV_LINUX_CPU_IOWAIT_STEEPNESS");
            env::remove_var("PWRZV_LINUX_NETWORK_DROPPED_MIDPOINT");
            env::remove_var("PWRZV_LINUX_NETWORK_DROPPED_STEEPNESS");
        }
    }

    #[test]
    fn test_realistic_linux_scenarios() {
        // Database server under load
        let db_server_load = LinuxSystemMetrics {
            cpu_usage_ratio: 0.7,
            cpu_io_wait_ratio: 0.3, // High I/O wait due to DB queries
            cpu_load_ratio: 1.5,    // High load
            memory_usage_ratio: 0.8,
            memory_pressure_ratio: 0.4,
            disk_io_ratio: 0.85, // Very high disk usage
            network_bandwidth_ratio: 0.4,
            network_dropped_packets_ratio: 0.0,
            fd_usage_ratio: 0.7, // Many connections
            process_count_ratio: 0.6,
        };

        let (level, _) = LinuxProvider::calculate(&db_server_load).unwrap();
        assert!(matches!(
            level,
            PowerReserveLevel::Critical | PowerReserveLevel::Low
        ));

        // Network-intensive application with packet loss
        let network_app = LinuxSystemMetrics {
            cpu_usage_ratio: 0.5,
            cpu_io_wait_ratio: 0.05,
            cpu_load_ratio: 0.9,
            memory_usage_ratio: 0.6,
            memory_pressure_ratio: 0.2,
            disk_io_ratio: 0.3,
            network_bandwidth_ratio: 0.9,        // Very high bandwidth
            network_dropped_packets_ratio: 0.02, // 2% packet loss
            fd_usage_ratio: 0.8,                 // Many network connections
            process_count_ratio: 0.5,
        };

        let (level, _) = LinuxProvider::calculate(&network_app).unwrap();
        assert!(matches!(
            level,
            PowerReserveLevel::Critical | PowerReserveLevel::Low
        ));

        // Memory-constrained container
        let memory_constrained = LinuxSystemMetrics {
            cpu_usage_ratio: 0.4,
            cpu_io_wait_ratio: 0.1,
            cpu_load_ratio: 0.8,
            memory_usage_ratio: 0.95,   // Very high memory usage
            memory_pressure_ratio: 0.7, // High memory pressure
            disk_io_ratio: 0.4,
            network_bandwidth_ratio: 0.3,
            network_dropped_packets_ratio: 0.0,
            fd_usage_ratio: 0.4,
            process_count_ratio: 0.6,
        };

        let (level, _) = LinuxProvider::calculate(&memory_constrained).unwrap();
        assert!(matches!(
            level,
            PowerReserveLevel::Critical | PowerReserveLevel::Low
        ));

        // Well-balanced system
        let balanced_system = LinuxSystemMetrics {
            cpu_usage_ratio: 0.5,
            cpu_io_wait_ratio: 0.08,
            cpu_load_ratio: 0.9,
            memory_usage_ratio: 0.7,
            memory_pressure_ratio: 0.2,
            disk_io_ratio: 0.5,
            network_bandwidth_ratio: 0.6,
            network_dropped_packets_ratio: 0.0,
            fd_usage_ratio: 0.5,
            process_count_ratio: 0.6,
        };

        let (level, _) = LinuxProvider::calculate(&balanced_system).unwrap();
        assert!(matches!(
            level,
            PowerReserveLevel::Medium | PowerReserveLevel::High
        ));
    }
}
