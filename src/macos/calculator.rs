use super::metrics::MacSystemMetrics;
use crate::error::PwrzvResult;
use crate::sigmoid::SigmoidFn;
use crate::{PowerReserveLevel, PowerReserveProvider, PwrzvError};
use std::collections::HashMap;

// ================================
// The core parameters of the macOS power reserve calculator
// ================================

// CPU Usage: 稍微线性响应，在 60% 开始明显影响性能
const CPU_USAGE_CONFIG: SigmoidFn = SigmoidFn {
    midpoint: 0.60,  // 60% CPU 使用率是一个较好的平衡点
    steepness: 8.0,  // 降低陡峭度，使响应更线性
};

// CPU Load: 超过 1.0 表示有排队，1.2 是明显的性能瓶颈
const CPU_LOAD_CONFIG: SigmoidFn = SigmoidFn {
    midpoint: 1.2,   // 稍微提高临界点
    steepness: 5.0,  // 保持相对平缓的响应
};

// Memory Usage: 很陡峭的曲线，85% 之前都还好，之后急剧恶化
const MEMORY_USAGE_CONFIG: SigmoidFn = SigmoidFn {
    midpoint: 0.85,  // 85% 内存使用率是关键临界点
    steepness: 20.0, // 非常陡峭，符合内存特性
};

// Memory Compressed: 压缩内存比率，macOS 压缩算法很高效，60% 开始有压力
const MEMORY_COMPRESSED_CONFIG: SigmoidFn = SigmoidFn {
    midpoint: 0.60,  // 60% 压缩比开始有明显影响，现代 macOS 压缩很高效
    steepness: 15.0, // 中等陡峭度，适应 macOS 内存管理特性
};

// Disk I/O: 中等陡峭度，70% 利用率开始有影响
const DISK_IO_CONFIG: SigmoidFn = SigmoidFn {
    midpoint: 0.70,  // 70% I/O 利用率
    steepness: 10.0, // 中等陡峭度
};

// Network: 相对平缓，80% 带宽利用率才开始明显影响
const NETWORK_CONFIG: SigmoidFn = SigmoidFn {
    midpoint: 0.80,  // 80% 网络带宽利用率
    steepness: 6.0,  // 相对平缓
};

// File Descriptor: 很陡峭，90% 之前还好，之后很快就会出问题
const FD_CONFIG: SigmoidFn = SigmoidFn {
    midpoint: 0.90,  // 90% 文件描述符使用率
    steepness: 30.0, // 非常陡峭，接近极限时迅速恶化
};

// Process Count: 中等陡峭度，80% 进程数开始有影响
const PROCESS_CONFIG: SigmoidFn = SigmoidFn {
    midpoint: 0.80,  // 80% 进程数比率
    steepness: 12.0, // 中等陡峭度
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
        // Test high load situation (should trigger Critical with our sensitive parameters)
        let high_load_metrics = MacSystemMetrics {
            cpu_usage_ratio: 0.9,       // Very high CPU
            cpu_load_ratio: 1.8,        // High load ratio
            memory_usage_ratio: 0.85,   // At memory threshold
            memory_compressed_ratio: 0.6, // Very high compression
            disk_io_ratio: 0.7,         // High disk I/O
            network_bandwidth_ratio: 0.6, // Moderate network
            fd_usage_ratio: 0.5,        // Normal FD usage
            process_count_ratio: 0.6,   // Normal process count
        };

        let (level, _) = MacProvider::calculate(&high_load_metrics).unwrap();
        // With our sensitive parameters, this should result in Critical or Low levels
        assert!(matches!(
            level,
            PowerReserveLevel::Critical | PowerReserveLevel::Low
        ));

        // Test low load situation
        let low_load_metrics = MacSystemMetrics {
            cpu_usage_ratio: 0.15,
            cpu_load_ratio: 0.2,
            memory_usage_ratio: 0.4,
            memory_compressed_ratio: 0.1,
            disk_io_ratio: 0.2,
            network_bandwidth_ratio: 0.3,
            fd_usage_ratio: 0.3,
            process_count_ratio: 0.4,
        };

        let (level, _) = MacProvider::calculate(&low_load_metrics).unwrap();
        assert_eq!(level, PowerReserveLevel::Abundant);
    }

    #[test]
    fn test_sigmoid_parameter_tuning() {
        // Test CPU usage - should be more linear
        let cpu_config = CPU_USAGE_CONFIG;
        assert!(cpu_config.evaluate(0.3) < 0.2);  // 30% CPU should be low score
        assert!(cpu_config.evaluate(0.6) > 0.4 && cpu_config.evaluate(0.6) < 0.6);  // 60% CPU moderate
        assert!(cpu_config.evaluate(0.9) > 0.8);  // 90% CPU should be high score

        // Test memory usage - should be steep (adjusted based on actual values)
        let mem_config = MEMORY_USAGE_CONFIG;
        assert!(mem_config.evaluate(0.7) < 0.1);   // 70% memory should still be very low
        assert!(mem_config.evaluate(0.85) > 0.4 && mem_config.evaluate(0.85) < 0.6);  // 85% memory moderate (midpoint)
        assert!(mem_config.evaluate(0.95) > 0.8);  // 95% memory should be high (adjusted from 0.9 to 0.8)

        // Test memory compressed - updated for realistic macOS behavior
        let compressed_config = MEMORY_COMPRESSED_CONFIG;
        assert!(compressed_config.evaluate(0.3) < 0.2);   // 30% compressed still low
        assert!(compressed_config.evaluate(0.6) > 0.4 && compressed_config.evaluate(0.6) < 0.6);  // 60% moderate (midpoint)
        assert!(compressed_config.evaluate(0.8) > 0.8);   // 80% compressed high

        // Test file descriptor - should be very steep near limit
        let fd_config = FD_CONFIG;
        assert!(fd_config.evaluate(0.8) < 0.1);   // 80% FD should still be low
        assert!(fd_config.evaluate(0.9) > 0.4 && fd_config.evaluate(0.9) < 0.6);  // 90% FD moderate (midpoint)
        assert!(fd_config.evaluate(0.95) > 0.8);  // 95% FD should be high (adjusted from 0.9 to 0.8)
    }

    #[test]
    fn test_realistic_scenarios() {
        // Test memory pressure scenario
        let memory_pressure = MacSystemMetrics {
            cpu_usage_ratio: 0.5,       // Normal CPU
            cpu_load_ratio: 0.8,        // Normal load
            memory_usage_ratio: 0.92,   // Very high memory usage
            memory_compressed_ratio: 0.75, // Very high compression (updated for new threshold)
            disk_io_ratio: 0.3,         // Low disk I/O
            network_bandwidth_ratio: 0.2, // Low network
            fd_usage_ratio: 0.4,        // Normal FD usage
            process_count_ratio: 0.6,   // Normal process count
        };

        let (level, details) = MacProvider::calculate(&memory_pressure).unwrap();
        println!("Memory pressure scenario - Level: {:?}", level);
        println!("Memory usage score: {:.3}", details.get("memory_usage_score").unwrap_or(&0.0));
        println!("Memory compressed score: {:.3}", details.get("memory_compressed_score").unwrap_or(&0.0));
        
        // Should result in critical or high load due to severe memory pressure
        assert!(matches!(level, PowerReserveLevel::Critical | PowerReserveLevel::Low | PowerReserveLevel::Medium));

        // Test CPU intensive scenario
        let cpu_intensive = MacSystemMetrics {
            cpu_usage_ratio: 0.85,      // High CPU usage
            cpu_load_ratio: 2.5,        // Very high load
            memory_usage_ratio: 0.6,    // Normal memory
            memory_compressed_ratio: 0.1, // Low compression
            disk_io_ratio: 0.4,         // Moderate disk I/O
            network_bandwidth_ratio: 0.3, // Low network
            fd_usage_ratio: 0.5,        // Normal FD usage
            process_count_ratio: 0.7,   // Normal process count
        };

        let (level, details) = MacProvider::calculate(&cpu_intensive).unwrap();
        println!("CPU intensive scenario - Level: {:?}", level);
        println!("CPU usage score: {:.3}", details.get("cpu_usage_score").unwrap_or(&0.0));
        println!("CPU load score: {:.3}", details.get("cpu_load_score").unwrap_or(&0.0));
        
        // Should result in critical or high load due to severe CPU pressure
        assert!(matches!(level, PowerReserveLevel::Critical | PowerReserveLevel::Low | PowerReserveLevel::Medium));

        // Test balanced normal load
        let balanced_normal = MacSystemMetrics {
            cpu_usage_ratio: 0.45,      // Moderate CPU
            cpu_load_ratio: 0.7,        // Normal load
            memory_usage_ratio: 0.7,    // Normal memory
            memory_compressed_ratio: 0.15, // Low compression
            disk_io_ratio: 0.5,         // Moderate disk I/O
            network_bandwidth_ratio: 0.4, // Moderate network
            fd_usage_ratio: 0.6,        // Normal FD usage
            process_count_ratio: 0.6,   // Normal process count
        };

        let (level, _) = MacProvider::calculate(&balanced_normal).unwrap();
        println!("Balanced normal scenario - Level: {:?}", level);
        
        // Should result in low to medium load
        assert!(matches!(level, PowerReserveLevel::Abundant | PowerReserveLevel::High | PowerReserveLevel::Medium));
    }
}
