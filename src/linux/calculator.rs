use super::metrics::LinuxSystemMetrics;
use crate::error::PwrzvResult;
use crate::sigmoid::{SigmoidFn, get_sigmoid_config};
use crate::{PowerReserveLevel, PowerReserveMeterProvider};
use std::collections::HashMap;

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

/// Get network dropped packets configuration (env: PWRZV_LINUX_NETWORK_DROPPED_MIDPOINT, PWRZV_LINUX_NETWORK_DROPPED_STEEPNESS)
fn get_network_dropped_config() -> SigmoidFn {
    get_sigmoid_config("PWRZV_LINUX_NETWORK_DROPPED", 0.02, 100.0)
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
#[derive(Debug, Clone)]
pub(crate) struct LinuxProvider;

impl PowerReserveMeterProvider for LinuxProvider {
    async fn get_power_reserve_level(&self) -> PwrzvResult<u8> {
        let metrics = LinuxSystemMetrics::collect_system_metrics().await?;

        let (level, _) = Self::calculate(&metrics)?;
        Ok(level as u8)
    }

    async fn get_power_reserve_level_with_details(&self) -> PwrzvResult<(u8, HashMap<String, u8>)> {
        let metrics = LinuxSystemMetrics::collect_system_metrics().await?;

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
    ) -> PwrzvResult<(PowerReserveLevel, HashMap<String, u8>)> {
        let mut details = HashMap::new();
        let mut available_scores = Vec::new();

        // Process each metric if available
        if let Some(value) = metrics.cpu_usage_ratio {
            let score = get_cpu_usage_config().evaluate(value);
            let n = Self::five_point_scale(score);
            details.insert(format!("CPU Usage: {value}, (Score: {n})"), n);
            available_scores.push(n);
        }

        if let Some(value) = metrics.cpu_io_wait_ratio {
            let score = get_cpu_iowait_config().evaluate(value);
            let n = Self::five_point_scale(score);
            details.insert(format!("CPU IO Wait: {value}, (Score: {n})"), n);
            available_scores.push(n);
        }

        if let Some(value) = metrics.cpu_load_ratio {
            let score = get_cpu_load_config().evaluate(value);
            let n = Self::five_point_scale(score);
            details.insert(format!("CPU Load: {value}, (Score: {n})"), n);
            available_scores.push(n);
        }

        if let Some(value) = metrics.memory_usage_ratio {
            let score = get_memory_usage_config().evaluate(value);
            let n = Self::five_point_scale(score);
            details.insert(format!("Memory Usage: {value}, (Score: {n})"), n);
            available_scores.push(n);
        }

        if let Some(value) = metrics.memory_pressure_ratio {
            let score = get_memory_pressure_config().evaluate(value);
            let n = Self::five_point_scale(score);
            details.insert(format!("Memory Pressure: {value}, (Score: {n})"), n);
            available_scores.push(n);
        }

        if let Some(value) = metrics.disk_io_utilization {
            let score = get_disk_io_config().evaluate(value);
            let n = Self::five_point_scale(score);
            details.insert(format!("Disk IO Utilization: {value}, (Score: {n})"), n);
            available_scores.push(n);
        }

        if let Some(value) = metrics.network_dropped_packets_ratio {
            let score = get_network_dropped_config().evaluate(value);
            let n = Self::five_point_scale(score);
            details.insert(format!("Network Dropped Packets: {value}, (Score: {n})"), n);
            available_scores.push(n);
        }

        if let Some(value) = metrics.fd_usage_ratio {
            let score = get_fd_config().evaluate(value);
            let n = Self::five_point_scale(score);
            details.insert(format!("File Descriptors: {value}, (Score: {n})"), n);
            available_scores.push(n);
        }

        if let Some(value) = metrics.process_count_ratio {
            let score = get_process_config().evaluate(value);
            let n = Self::five_point_scale(score);
            details.insert(format!("Process Count: {value}, (Score: {n})"), n);
            available_scores.push(n);
        }

        // Find the minimum score from available metrics (bottleneck determines power reserve)
        if let Some(&final_score) = available_scores.iter().min() {
            let level =
                PowerReserveLevel::try_from(final_score).unwrap_or(PowerReserveLevel::Abundant);
            return Ok((level, details));
        }

        // If no metrics are available, return a default level
        Ok((PowerReserveLevel::Medium, details))
    }

    // 5-point scale (reverse) with better resolution
    // [0, 1] -> [5, 1]
    fn five_point_scale(score: f32) -> u8 {
        // Use more precise thresholds for better score distribution
        if score >= 0.8 {
            1 // Critical: sigmoid >= 0.8
        } else if score >= 0.6 {
            2 // Low: sigmoid >= 0.6
        } else if score >= 0.4 {
            3 // Medium: sigmoid >= 0.4
        } else if score >= 0.2 {
            4 // High: sigmoid >= 0.2
        } else {
            5 // Abundant: sigmoid < 0.2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_five_point_scale() {
        // Test the five_point_scale function with new threshold-based logic
        // [0, 1] -> [5, 1] using precise thresholds
        assert_eq!(LinuxProvider::five_point_scale(0.0), 5); // < 0.2 -> Abundant
        assert_eq!(LinuxProvider::five_point_scale(0.1), 5); // < 0.2 -> Abundant
        assert_eq!(LinuxProvider::five_point_scale(0.19), 5); // < 0.2 -> Abundant
        assert_eq!(LinuxProvider::five_point_scale(0.2), 4); // >= 0.2 -> High
        assert_eq!(LinuxProvider::five_point_scale(0.3), 4); // >= 0.2 -> High
        assert_eq!(LinuxProvider::five_point_scale(0.39), 4); // >= 0.2 -> High
        assert_eq!(LinuxProvider::five_point_scale(0.4), 3); // >= 0.4 -> Medium
        assert_eq!(LinuxProvider::five_point_scale(0.5), 3); // >= 0.4 -> Medium
        assert_eq!(LinuxProvider::five_point_scale(0.59), 3); // >= 0.4 -> Medium
        assert_eq!(LinuxProvider::five_point_scale(0.6), 2); // >= 0.6 -> Low
        assert_eq!(LinuxProvider::five_point_scale(0.7), 2); // >= 0.6 -> Low
        assert_eq!(LinuxProvider::five_point_scale(0.79), 2); // >= 0.6 -> Low
        assert_eq!(LinuxProvider::five_point_scale(0.8), 1); // >= 0.8 -> Critical
        assert_eq!(LinuxProvider::five_point_scale(0.9), 1); // >= 0.8 -> Critical
        assert_eq!(LinuxProvider::five_point_scale(1.0), 1); // >= 0.8 -> Critical
    }

    #[test]
    fn test_calculate_with_full_metrics() {
        let metrics = LinuxSystemMetrics {
            cpu_usage_ratio: Some(0.5),
            cpu_io_wait_ratio: Some(0.1),
            cpu_load_ratio: Some(0.8),
            memory_usage_ratio: Some(0.7),
            memory_pressure_ratio: Some(0.3),
            disk_io_utilization: Some(0.4),
            network_dropped_packets_ratio: Some(0.001),
            fd_usage_ratio: Some(0.5),
            process_count_ratio: Some(0.6),
        };

        let result = LinuxProvider::calculate(&metrics);
        assert!(
            result.is_ok(),
            "Calculation should succeed with full metrics"
        );

        let (level, details) = result.unwrap();

        // Should have details for all provided metrics
        assert_eq!(details.len(), 9, "Should have 9 metric details");

        // All scores should be in valid range [1, 5]
        for score in details.values() {
            assert!(
                *score >= 1 && *score <= 5,
                "Score {} should be in range [1, 5]",
                score
            );
        }

        // Level should be valid
        assert!(matches!(
            level,
            PowerReserveLevel::Critical
                | PowerReserveLevel::Low
                | PowerReserveLevel::Medium
                | PowerReserveLevel::High
                | PowerReserveLevel::Abundant
        ));
    }

    #[test]
    fn test_calculate_with_no_metrics() {
        let metrics = LinuxSystemMetrics {
            cpu_usage_ratio: None,
            cpu_io_wait_ratio: None,
            cpu_load_ratio: None,
            memory_usage_ratio: None,
            memory_pressure_ratio: None,
            disk_io_utilization: None,
            network_dropped_packets_ratio: None,
            fd_usage_ratio: None,
            process_count_ratio: None,
        };

        let result = LinuxProvider::calculate(&metrics);
        assert!(
            result.is_ok(),
            "Calculation should succeed even with no metrics"
        );

        let (level, details) = result.unwrap();
        assert_eq!(
            level,
            PowerReserveLevel::Medium,
            "Should default to medium level"
        );
        assert!(details.is_empty(), "Should have no detailed scores");
    }

    #[test]
    #[allow(unused_variables)]
    fn test_calculate_with_partial_metrics() {
        let metrics = LinuxSystemMetrics {
            cpu_usage_ratio: Some(0.3),
            cpu_io_wait_ratio: None,
            cpu_load_ratio: Some(0.6),
            memory_usage_ratio: Some(0.5),
            memory_pressure_ratio: None,
            disk_io_utilization: None,
            network_dropped_packets_ratio: None,
            fd_usage_ratio: None,
            process_count_ratio: None,
        };

        let result = LinuxProvider::calculate(&metrics);
        assert!(
            result.is_ok(),
            "Calculation should succeed with partial metrics"
        );

        let (level, details) = result.unwrap();

        // Should have details only for provided metrics
        assert_eq!(details.len(), 3, "Should have 3 metric details");

        // Check that we have the expected metrics
        let has_cpu_usage = details.keys().any(|k| k.contains("CPU Usage"));
        let has_cpu_load = details.keys().any(|k| k.contains("CPU Load"));
        let has_memory_usage = details.keys().any(|k| k.contains("Memory Usage"));
        let has_cpu_iowait = details.keys().any(|k| k.contains("CPU IO Wait"));

        assert!(has_cpu_usage, "Should have CPU usage metric");
        assert!(has_cpu_load, "Should have CPU load metric");
        assert!(has_memory_usage, "Should have memory usage metric");
        assert!(!has_cpu_iowait, "Should not have CPU IO Wait metric");
    }

    #[test]
    fn test_calculate_extreme_values() {
        // Test with extreme high values (should result in low scores)
        let high_load_metrics = LinuxSystemMetrics {
            cpu_usage_ratio: Some(0.95),
            cpu_io_wait_ratio: Some(0.5),
            cpu_load_ratio: Some(2.0),
            memory_usage_ratio: Some(0.95),
            memory_pressure_ratio: Some(0.8),
            disk_io_utilization: Some(0.9),
            network_dropped_packets_ratio: Some(0.05),
            fd_usage_ratio: Some(0.9),
            process_count_ratio: Some(0.85),
        };

        let result = LinuxProvider::calculate(&high_load_metrics);
        assert!(result.is_ok(), "Should handle extreme high values");

        let (level, details) = result.unwrap();

        // Should result in low overall level due to high system stress
        // Since we take MIN score (worst metric), high load should definitely result in Critical/Low
        assert!(
            matches!(level, PowerReserveLevel::Critical | PowerReserveLevel::Low),
            "High load should result in low power reserve (taking minimum score)"
        );

        // Most scores should be low (1 or 2)
        let low_scores = details.values().filter(|&&score| score <= 2).count();
        assert!(
            low_scores > 0,
            "Should have some low scores with high system load"
        );
    }

    #[test]
    fn test_calculate_low_values() {
        // Test with low system load (should result in high scores)
        let low_load_metrics = LinuxSystemMetrics {
            cpu_usage_ratio: Some(0.1),
            cpu_io_wait_ratio: Some(0.02),
            cpu_load_ratio: Some(0.3),
            memory_usage_ratio: Some(0.4),
            memory_pressure_ratio: Some(0.1),
            disk_io_utilization: Some(0.2),
            network_dropped_packets_ratio: Some(0.0),
            fd_usage_ratio: Some(0.3),
            process_count_ratio: Some(0.4),
        };

        let result = LinuxProvider::calculate(&low_load_metrics);
        assert!(result.is_ok(), "Should handle low system load");

        let (level, details) = result.unwrap();

        // Should result in high overall level due to low system stress
        assert!(
            matches!(level, PowerReserveLevel::High | PowerReserveLevel::Abundant),
            "Low load should result in high power reserve"
        );

        // Most scores should be high (4 or 5)
        let high_scores = details.values().filter(|&&score| score >= 4).count();
        assert!(
            high_scores > 0,
            "Should have some high scores with low system load"
        );
    }
}
