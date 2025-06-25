use super::metrics::LinuxSystemMetrics;
use crate::PowerReserveMeterProvider;
use crate::error::PwrzvResult;
use crate::sigmoid::{SigmoidFn, get_sigmoid_config};
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
    async fn get_power_reserve_level(&self) -> PwrzvResult<f32> {
        let metrics = LinuxSystemMetrics::collect_system_metrics().await?;

        let (level, _) = Self::calculate(&metrics)?;
        Ok(level)
    }

    async fn get_power_reserve_level_with_details(
        &self,
    ) -> PwrzvResult<(f32, HashMap<String, f32>)> {
        let metrics = LinuxSystemMetrics::collect_system_metrics().await?;

        let (level, details) = Self::calculate(&metrics)?;
        Ok((level, details))
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
    /// * `level` - The power reserve level as f32 (1.0-5.0)
    /// * `details` - The details of the power reserve level
    fn calculate(metrics: &LinuxSystemMetrics) -> PwrzvResult<(f32, HashMap<String, f32>)> {
        let mut details = HashMap::new();
        let mut available_scores = Vec::new();

        // Process each metric if available
        if let Some(value) = metrics.cpu_usage_ratio {
            let score = get_cpu_usage_config().evaluate(value);
            let n = Self::five_point_scale_with_decimal(score);
            details.insert(format!("CPU Usage: {value:.3} (Score: {n:.3})"), n);
            available_scores.push(n);
        }

        if let Some(value) = metrics.cpu_io_wait_ratio {
            let score = get_cpu_iowait_config().evaluate(value);
            let n = Self::five_point_scale_with_decimal(score);
            details.insert(format!("CPU IO Wait: {value:.3} (Score: {n:.3})"), n);
            available_scores.push(n);
        }

        if let Some(value) = metrics.cpu_load_ratio {
            let score = get_cpu_load_config().evaluate(value);
            let n = Self::five_point_scale_with_decimal(score);
            details.insert(format!("CPU Load: {value:.3} (Score: {n:.3})"), n);
            available_scores.push(n);
        }

        if let Some(value) = metrics.memory_usage_ratio {
            let score = get_memory_usage_config().evaluate(value);
            let n = Self::five_point_scale_with_decimal(score);
            details.insert(format!("Memory Usage: {value:.3} (Score: {n:.3})"), n);
            available_scores.push(n);
        }

        if let Some(value) = metrics.memory_pressure_ratio {
            let score = get_memory_pressure_config().evaluate(value);
            let n = Self::five_point_scale_with_decimal(score);
            details.insert(format!("Memory Pressure: {value:.3} (Score: {n:.3})"), n);
            available_scores.push(n);
        }

        if let Some(value) = metrics.disk_io_utilization {
            let score = get_disk_io_config().evaluate(value);
            let n = Self::five_point_scale_with_decimal(score);
            details.insert(
                format!("Disk IO Utilization: {value:.3} (Score: {n:.3})"),
                n,
            );
            available_scores.push(n);
        }

        if let Some(value) = metrics.network_dropped_packets_ratio {
            let score = get_network_dropped_config().evaluate(value);
            let n = Self::five_point_scale_with_decimal(score);
            details.insert(
                format!("Network Dropped Packets: {value:.3} (Score: {n:.3})"),
                n,
            );
            available_scores.push(n);
        }

        if let Some(value) = metrics.fd_usage_ratio {
            let score = get_fd_config().evaluate(value);
            let n = Self::five_point_scale_with_decimal(score);
            details.insert(format!("File Descriptors: {value:.3} (Score: {n:.3})"), n);
            available_scores.push(n);
        }

        if let Some(value) = metrics.process_count_ratio {
            let score = get_process_config().evaluate(value);
            let n = Self::five_point_scale_with_decimal(score);
            details.insert(format!("Process Count: {value:.3} (Score: {n:.3})"), n);
            available_scores.push(n);
        }

        // Find the minimum score from available metrics (bottleneck determines power reserve)
        if let Some(&final_score) = available_scores
            .iter()
            .min_by(|a, b| a.partial_cmp(b).unwrap())
        {
            return Ok((final_score, details));
        }

        // If no metrics are available, return a default level
        Ok((3.0, details))
    }

    /// Convert sigmoid score to 5-point scale with decimal precision
    /// [0, 1.0] -> [5.0, 0.0]
    fn five_point_scale_with_decimal(score: f32) -> f32 {
        let score = 5.0 * (1.0 - score);
        // Retain 4 decimal places for precision
        let factor = 10_000f32;
        (score * factor).round() / factor
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_five_point_scale_with_decimal() {
        // Test the five_point_scale_with_decimal function
        // [0, 1] -> [5.0, 1.0] linear mapping with 4 decimal precision
        assert_eq!(LinuxProvider::five_point_scale_with_decimal(0.0), 5.0000); // 5 * (1 - 0) = 5
        assert_eq!(LinuxProvider::five_point_scale_with_decimal(0.2), 4.0000); // 5 * (1 - 0.2) = 4 
        assert_eq!(LinuxProvider::five_point_scale_with_decimal(0.4), 3.0000); // 5 * (1 - 0.4) = 3
        assert_eq!(LinuxProvider::five_point_scale_with_decimal(0.6), 2.0000); // 5 * (1 - 0.6) = 2
        assert_eq!(LinuxProvider::five_point_scale_with_decimal(0.8), 1.0000); // 5 * (1 - 0.8) = 1
        assert_eq!(LinuxProvider::five_point_scale_with_decimal(1.0), 0.0000); // 5 * (1 - 1) = 0

        // Test decimal precision
        let score = LinuxProvider::five_point_scale_with_decimal(0.1234);
        assert!((score - 4.3830).abs() < 0.0001); // 5 * (1 - 0.1234) = 4.383
    }

    #[test]
    fn test_calculate_with_full_metrics() {
        let metrics = LinuxSystemMetrics {
            cpu_usage_ratio: Some(0.5),
            cpu_io_wait_ratio: Some(0.1),
            cpu_load_ratio: Some(0.8),
            memory_usage_ratio: Some(0.7),
            memory_pressure_ratio: Some(0.2),
            disk_io_utilization: Some(0.6),
            network_dropped_packets_ratio: Some(0.01),
            fd_usage_ratio: Some(0.3),
            process_count_ratio: Some(0.4),
        };

        let result = LinuxProvider::calculate(&metrics);
        assert!(result.is_ok());

        let (level, details) = result.unwrap();

        // Should have entries for all metrics
        assert!(!details.is_empty());

        // All scores should be in valid range [1.0, 5.0]
        for score in details.values() {
            assert!(
                *score >= 1.0 && *score <= 5.0,
                "Score {score} should be in range [1.0, 5.0]"
            );
        }

        // Level should be valid
        assert!(level >= 1.0 && level <= 5.0);
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
        assert!(result.is_ok());

        let (level, details) = result.unwrap();

        assert_eq!(
            level, 3.0,
            "Should default to 3.0 level when no metrics available"
        );
        assert!(details.is_empty(), "Should have no detailed scores");
    }

    #[test]
    fn test_calculate_with_partial_metrics() {
        let metrics = LinuxSystemMetrics {
            cpu_usage_ratio: Some(0.3),
            cpu_io_wait_ratio: None,
            cpu_load_ratio: Some(0.5),
            memory_usage_ratio: None,
            memory_pressure_ratio: Some(0.1),
            disk_io_utilization: None,
            network_dropped_packets_ratio: None,
            fd_usage_ratio: None,
            process_count_ratio: None,
        };

        let result = LinuxProvider::calculate(&metrics);
        assert!(result.is_ok());

        let (level, details) = result.unwrap();

        // Should have exactly 3 entries (for the 3 non-None metrics)
        assert_eq!(details.len(), 3);

        // All scores should be in valid range
        for score in details.values() {
            assert!(
                *score >= 1.0 && *score <= 5.0,
                "Score {score} should be in range [1.0, 5.0]"
            );
        }

        // Level should be the minimum of the calculated scores
        assert!(level >= 1.0 && level <= 5.0);
    }

    #[test]
    fn test_calculate_extreme_values() {
        // Test with high load (should result in low scores)
        let metrics = LinuxSystemMetrics {
            cpu_usage_ratio: Some(0.95),              // Very high CPU usage
            cpu_io_wait_ratio: Some(0.8),             // High I/O wait
            cpu_load_ratio: Some(3.0),                // Very high load
            memory_usage_ratio: Some(0.98),           // Very high memory usage
            memory_pressure_ratio: Some(0.9),         // High memory pressure
            disk_io_utilization: Some(0.99),          // Very high disk I/O
            network_dropped_packets_ratio: Some(0.1), // High dropped packets
            fd_usage_ratio: Some(0.95),               // High FD usage
            process_count_ratio: Some(0.9),           // High process count
        };

        let result = LinuxProvider::calculate(&metrics);
        assert!(result.is_ok());

        let (level, details) = result.unwrap();

        // Since we take MIN score (worst metric), high load should result in low power reserve
        assert!(
            level <= 2.0,
            "High load should result in low power reserve (taking minimum score)"
        );

        // Most scores should be low (<=2.0)
        let low_scores = details.values().filter(|&&score| score <= 2.0).count();
        assert!(
            low_scores > 0,
            "Should have some low scores with high system load"
        );
    }

    #[test]
    fn test_calculate_low_values() {
        // Test with low system stress (should result in high scores)
        let metrics = LinuxSystemMetrics {
            cpu_usage_ratio: Some(0.05),                // Very low CPU usage
            cpu_io_wait_ratio: Some(0.01),              // Very low I/O wait
            cpu_load_ratio: Some(0.1),                  // Very low load
            memory_usage_ratio: Some(0.1),              // Low memory usage
            memory_pressure_ratio: Some(0.01),          // Very low memory pressure
            disk_io_utilization: Some(0.05),            // Very low disk I/O
            network_dropped_packets_ratio: Some(0.001), // Very low dropped packets
            fd_usage_ratio: Some(0.1),                  // Low FD usage
            process_count_ratio: Some(0.1),             // Low process count
        };

        let result = LinuxProvider::calculate(&metrics);
        assert!(result.is_ok());

        let (level, details) = result.unwrap();

        // Should result in high overall level due to low system stress
        assert!(level >= 4.0, "Low load should result in high power reserve");

        // Most scores should be high (>=4.0)
        let high_scores = details.values().filter(|&&score| score >= 4.0).count();
        assert!(
            high_scores > 0,
            "Should have some high scores with low system load"
        );
    }
}
