use super::metrics::MacSystemMetrics;
use crate::error::PwrzvResult;
use crate::sigmoid::SigmoidFn;
use crate::{PowerReserveLevel, PowerReserveProvider, PwrzvError};
use std::collections::HashMap;

// ================================
// The core parameters of the macOS power reserve calculator
// ================================

const CPU_USAGE_CONFIG: SigmoidFn = SigmoidFn {
    midpoint: 0.75,
    steepness: 12.0,
};

const CPU_LOAD_CONFIG: SigmoidFn = SigmoidFn {
    midpoint: 1.0,
    steepness: 6.0,
};

const MEMORY_USAGE_CONFIG: SigmoidFn = SigmoidFn {
    midpoint: 0.8,
    steepness: 10.0,
};

const MEMORY_COMPRESSED_CONFIG: SigmoidFn = SigmoidFn {
    midpoint: 0.3,
    steepness: 15.0,
};

const DISK_IO_CONFIG: SigmoidFn = SigmoidFn {
    midpoint: 0.6,
    steepness: 8.0,
};

const NETWORK_CONFIG: SigmoidFn = SigmoidFn {
    midpoint: 0.7,
    steepness: 6.0,
};

const FD_CONFIG: SigmoidFn = SigmoidFn {
    midpoint: 0.85,
    steepness: 18.0,
};

const PROCESS_CONFIG: SigmoidFn = SigmoidFn {
    midpoint: 0.75,
    steepness: 12.0,
};

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
        let cpu_usage_score = CPU_USAGE_CONFIG.evaluate(metrics.cpu_usage_ratio);
        let cpu_load_score = CPU_LOAD_CONFIG.evaluate(metrics.cpu_load_ratio);
        let memory_usage_score = MEMORY_USAGE_CONFIG.evaluate(metrics.memory_usage_ratio);
        let memory_compressed_score =
            MEMORY_COMPRESSED_CONFIG.evaluate(metrics.memory_compressed_ratio);
        let disk_io_score = DISK_IO_CONFIG.evaluate(metrics.disk_io_ratio);
        let network_score = NETWORK_CONFIG.evaluate(metrics.network_bandwidth_ratio);
        let fd_score = FD_CONFIG.evaluate(metrics.fd_usage_ratio);
        let process_score = PROCESS_CONFIG.evaluate(metrics.process_count_ratio);

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
        // Test high load situation (should be High using max algorithm)
        let high_load_metrics = MacSystemMetrics {
            cpu_usage_ratio: 0.9,
            cpu_load_ratio: 1.8,
            memory_usage_ratio: 0.85,
            memory_compressed_ratio: 0.6,
            disk_io_ratio: 0.7,
            network_bandwidth_ratio: 0.6,
            fd_usage_ratio: 0.5,
            process_count_ratio: 0.6,
        };

        let (level, _) = MacProvider::calculate(&high_load_metrics).unwrap();
        // New algorithm uses max value * 5, high load usually results in High
        assert!(matches!(
            level,
            PowerReserveLevel::High | PowerReserveLevel::Medium
        ));

        // Test low load situation
        let low_load_metrics = MacSystemMetrics {
            cpu_usage_ratio: 0.15,
            cpu_load_ratio: 0.2,
            memory_usage_ratio: 0.4,
            memory_compressed_ratio: 0.1,
            disk_io_ratio: 0.2,
            network_bandwidth_ratio: 0.1,
            fd_usage_ratio: 0.1,
            process_count_ratio: 0.2,
        };

        let (level, _) = MacProvider::calculate(&low_load_metrics).unwrap();
        assert_eq!(level, PowerReserveLevel::Abundant);
    }
}
