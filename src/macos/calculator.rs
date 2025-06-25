use super::metrics::MacSystemMetrics;
use crate::PowerReserveMeterProvider;
use crate::error::PwrzvResult;
use crate::sigmoid::{SigmoidFn, get_sigmoid_config};
use std::collections::HashMap;

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

/// Get network dropped packets configuration (env: PWRZV_MACOS_NETWORK_DROPPED_MIDPOINT, PWRZV_MACOS_NETWORK_DROPPED_STEEPNESS)
fn get_network_dropped_config() -> SigmoidFn {
    get_sigmoid_config("PWRZV_MACOS_NETWORK_DROPPED", 0.02, 100.0)
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

#[derive(Debug, Clone)]
pub(crate) struct MacProvider;

impl PowerReserveMeterProvider for MacProvider {
    async fn get_power_reserve_level(&self) -> PwrzvResult<f32> {
        let metrics = MacSystemMetrics::collect_system_metrics().await?;

        let (level, _) = Self::calculate(&metrics)?;
        Ok(level)
    }

    async fn get_power_reserve_level_with_details(
        &self,
    ) -> PwrzvResult<(f32, HashMap<String, f32>)> {
        let metrics = MacSystemMetrics::collect_system_metrics().await?;

        let (level, details) = Self::calculate(&metrics)?;
        Ok((level, details))
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
    fn calculate(metrics: &MacSystemMetrics) -> PwrzvResult<(f32, HashMap<String, f32>)> {
        let mut details = HashMap::new();
        let mut available_scores = Vec::new();

        // Process each metric if available
        if let Some(value) = metrics.cpu_usage_ratio {
            let score = get_cpu_usage_config().evaluate(value);
            let n = Self::five_point_scale_with_decimal(score);
            details.insert(format!("CPU Usage: {value}, (Score: {n})"), n);
            available_scores.push(n);
        }

        if let Some(value) = metrics.cpu_load_ratio {
            let score = get_cpu_load_config().evaluate(value);
            let n = Self::five_point_scale_with_decimal(score);
            details.insert(format!("CPU Load: {value}, (Score: {n})"), n);
            available_scores.push(n);
        }

        if let Some(value) = metrics.memory_usage_ratio {
            let score = get_memory_usage_config().evaluate(value);
            let n = Self::five_point_scale_with_decimal(score);
            details.insert(format!("Memory Usage: {value}, (Score: {n})"), n);
            available_scores.push(n);
        }

        if let Some(value) = metrics.memory_compressed_ratio {
            let score = get_memory_compressed_config().evaluate(value);
            let n = Self::five_point_scale_with_decimal(score);
            details.insert(format!("Memory Compressed: {value}, (Score: {n})"), n);
            available_scores.push(n);
        }

        if let Some(value) = metrics.network_dropped_packets_ratio {
            let score = get_network_dropped_config().evaluate(value);
            let n = Self::five_point_scale_with_decimal(score);
            details.insert(format!("Network Dropped: {value}, (Score: {n})"), n);
            available_scores.push(n);
        }

        if let Some(value) = metrics.fd_usage_ratio {
            let score = get_fd_config().evaluate(value);
            let n = Self::five_point_scale_with_decimal(score);
            details.insert(format!("File Descriptors: {value}, (Score: {n})"), n);
            available_scores.push(n);
        }

        if let Some(value) = metrics.process_count_ratio {
            let score = get_process_config().evaluate(value);
            let n = Self::five_point_scale_with_decimal(score);
            details.insert(format!("Process Count: {value}, (Score: {n})"), n);
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
    fn test_calculate_with_full_metrics() {
        let metrics = MacSystemMetrics {
            cpu_usage_ratio: Some(0.5),
            cpu_load_ratio: Some(0.8),
            memory_usage_ratio: Some(0.7),
            memory_compressed_ratio: Some(0.3),
            network_dropped_packets_ratio: Some(0.001),
            fd_usage_ratio: Some(0.5),
            process_count_ratio: Some(0.6),
        };

        let result = MacProvider::calculate(&metrics);
        assert!(
            result.is_ok(),
            "Calculation should succeed with full metrics"
        );

        let (level, details) = result.unwrap();

        // Should have details for all provided metrics
        assert_eq!(details.len(), 7, "Should have 7 metric details");

        // All scores should be in valid range [0, 5]
        for score in details.values() {
            assert!(
                *score >= 0.0 && *score <= 5.0,
                "Score {score} should be in range [0, 5]"
            );
        }

        // Level should be valid
        assert!((0.0..=5.0).contains(&level));
    }

    #[test]
    fn test_calculate_with_no_metrics() {
        let metrics = MacSystemMetrics {
            cpu_usage_ratio: None,
            cpu_load_ratio: None,
            memory_usage_ratio: None,
            memory_compressed_ratio: None,
            network_dropped_packets_ratio: None,
            fd_usage_ratio: None,
            process_count_ratio: None,
        };

        let result = MacProvider::calculate(&metrics);
        assert!(
            result.is_ok(),
            "Calculation should succeed even with no metrics"
        );

        let (level, details) = result.unwrap();
        assert_eq!(level, 3.0, "Should default to medium level");
        assert!(details.is_empty(), "Should have no detailed scores");
    }

    #[test]
    #[allow(unused_variables)]
    fn test_calculate_with_partial_metrics() {
        let metrics = MacSystemMetrics {
            cpu_usage_ratio: Some(0.3),
            cpu_load_ratio: None,
            memory_usage_ratio: Some(0.6),
            memory_compressed_ratio: Some(0.2),
            network_dropped_packets_ratio: None,
            fd_usage_ratio: None,
            process_count_ratio: None,
        };

        let result = MacProvider::calculate(&metrics);
        assert!(
            result.is_ok(),
            "Calculation should succeed with partial metrics"
        );

        let (level, details) = result.unwrap();

        // Should have details only for provided metrics
        assert_eq!(details.len(), 3, "Should have 3 metric details");

        // Check that we have the expected metrics
        let has_cpu = details.keys().any(|k| k.contains("CPU Usage"));
        let has_memory_usage = details.keys().any(|k| k.contains("Memory Usage"));
        let has_memory_compressed = details.keys().any(|k| k.contains("Memory Compressed"));
        let has_cpu_load = details.keys().any(|k| k.contains("CPU Load"));

        assert!(has_cpu, "Should have CPU usage metric");
        assert!(has_memory_usage, "Should have memory usage metric");
        assert!(
            has_memory_compressed,
            "Should have memory compressed metric"
        );
        assert!(!has_cpu_load, "Should not have CPU load metric");
    }

    #[test]
    fn test_calculate_extreme_values() {
        // Test with high load (should result in low scores)
        let metrics = MacSystemMetrics {
            cpu_usage_ratio: Some(0.95),              // Very high CPU usage
            cpu_load_ratio: Some(3.0),                // Very high load
            memory_usage_ratio: Some(0.98),           // Very high memory usage
            memory_compressed_ratio: Some(0.9),       // High memory compression
            network_dropped_packets_ratio: Some(0.1), // High dropped packets
            fd_usage_ratio: Some(0.95),               // High FD usage
            process_count_ratio: Some(0.9),           // High process count
        };

        let result = MacProvider::calculate(&metrics);
        assert!(result.is_ok());

        let (level, details) = result.unwrap();

        // Since we take MIN score (worst metric), high load should result in low power reserve
        assert!(
            level <= 2.5,
            "High load should result in low power reserve (taking minimum score), got: {level}"
        );

        // Most scores should be low (<=2.5)
        let low_scores = details.values().filter(|&&score| score <= 2.5).count();
        assert!(
            low_scores > 0,
            "Should have some low scores with high system load"
        );
    }

    #[test]
    fn test_calculate_low_values() {
        // Test with low system load (should result in high scores)
        let low_load_metrics = MacSystemMetrics {
            cpu_usage_ratio: Some(0.1),
            cpu_load_ratio: Some(0.3),
            memory_usage_ratio: Some(0.4),
            memory_compressed_ratio: Some(0.1),
            network_dropped_packets_ratio: Some(0.0),
            fd_usage_ratio: Some(0.2),
            process_count_ratio: Some(0.3),
        };

        let result = MacProvider::calculate(&low_load_metrics);
        assert!(result.is_ok(), "Should handle low system load");

        let (level, details) = result.unwrap();

        // Should result in high overall level due to low system stress
        assert!(
            (4.0..=5.0).contains(&level),
            "Low load should result in high power reserve"
        );

        // Most scores should be high (4 or 5)
        let high_scores = details.values().filter(|&&score| score >= 4.0).count();
        assert!(
            high_scores > 0,
            "Should have some high scores with low system load"
        );
    }
}
