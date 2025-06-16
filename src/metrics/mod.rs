//! System metrics module
//!
//! Provides system resource monitoring metrics definitions and data collection functions.
//! Supports multiple platforms through trait abstraction.

use crate::error::PwrzvResult;
use crate::platform;
use serde::{Deserialize, Serialize};

// Platform-specific implementations
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
mod default;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;

// Re-export platform-specific collectors
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub use default::DefaultMetricsCollector;
#[cfg(target_os = "linux")]
pub use linux::LinuxMetricsCollector;
#[cfg(target_os = "macos")]
pub use macos::MacOSMetricsCollector;

/// System metrics data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    /// Total CPU utilization (%)
    pub cpu_usage: f32,
    /// CPU I/O wait time (%)
    pub cpu_iowait: f32,
    /// Available memory percentage (%)
    pub mem_available: f32,
    /// Swap usage percentage (%)
    pub swap_usage: f32,
    /// Disk I/O utilization (%)
    pub disk_usage: f32,
    /// Network I/O utilization (%)
    pub net_usage: f32,
    /// File descriptor usage percentage (%)
    pub fd_usage: f32,
}

impl Default for SystemMetrics {
    fn default() -> Self {
        SystemMetrics {
            cpu_usage: 0.0,
            cpu_iowait: 0.0,
            mem_available: 100.0,
            swap_usage: 0.0,
            disk_usage: 0.0,
            net_usage: 0.0,
            fd_usage: 0.0,
        }
    }
}

/// Trait for collecting system metrics on different platforms
pub trait MetricsCollector {
    /// Collect CPU statistics (usage%, iowait%)
    fn collect_cpu_stats(&self) -> PwrzvResult<(f32, f32)>;

    /// Collect memory statistics (available%, swap_usage%)
    fn collect_memory_stats(&self) -> PwrzvResult<(f32, f32)>;

    /// Collect disk I/O statistics (usage%)
    fn collect_disk_stats(&self) -> PwrzvResult<f32>;

    /// Collect network I/O statistics (usage%)
    fn collect_network_stats(&self) -> PwrzvResult<f32>;

    /// Collect file descriptor statistics (usage%)
    fn collect_fd_stats(&self) -> PwrzvResult<f32>;
}

/// Create a platform-specific metrics collector
pub fn create_metrics_collector() -> Box<dyn MetricsCollector> {
    #[cfg(target_os = "linux")]
    {
        Box::new(LinuxMetricsCollector)
    }
    #[cfg(target_os = "macos")]
    {
        Box::new(MacOSMetricsCollector)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        Box::new(DefaultMetricsCollector)
    }
}

impl SystemMetrics {
    /// Create a new system metrics instance
    pub fn new() -> Self {
        Self::default()
    }

    /// Collect all system metrics using platform-specific collector
    pub fn collect() -> PwrzvResult<Self> {
        // First check platform compatibility
        platform::check_platform()?;

        let collector = create_metrics_collector();
        let mut metrics = SystemMetrics::new();

        // Collect various metrics, use default values and log warnings if any fails
        match collector.collect_cpu_stats() {
            Ok((cpu_usage, cpu_iowait)) => {
                metrics.cpu_usage = cpu_usage;
                metrics.cpu_iowait = cpu_iowait;
            }
            Err(e) => eprintln!("Warning: Failed to read CPU stats: {e}"),
        }

        match collector.collect_memory_stats() {
            Ok((mem_available, swap_usage)) => {
                metrics.mem_available = mem_available;
                metrics.swap_usage = swap_usage;
            }
            Err(e) => eprintln!("Warning: Failed to read memory stats: {e}"),
        }

        match collector.collect_disk_stats() {
            Ok(disk_usage) => metrics.disk_usage = disk_usage,
            Err(e) => eprintln!("Warning: Failed to read disk stats: {e}"),
        }

        match collector.collect_network_stats() {
            Ok(net_usage) => metrics.net_usage = net_usage,
            Err(e) => eprintln!("Warning: Failed to read network stats: {e}"),
        }

        match collector.collect_fd_stats() {
            Ok(fd_usage) => metrics.fd_usage = fd_usage,
            Err(e) => eprintln!("Warning: Failed to read file descriptor stats: {e}"),
        }

        Ok(metrics)
    }

    /// Validate the validity of metrics data
    pub fn validate(&self) -> bool {
        let is_valid_percentage = |val: f32| (0.0..=100.0).contains(&val);

        is_valid_percentage(self.cpu_usage)
            && is_valid_percentage(self.cpu_iowait)
            && is_valid_percentage(self.mem_available)
            && is_valid_percentage(self.swap_usage)
            && is_valid_percentage(self.disk_usage)
            && is_valid_percentage(self.net_usage)
            && is_valid_percentage(self.fd_usage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_metrics_default() {
        let metrics = SystemMetrics::default();
        assert_eq!(metrics.cpu_usage, 0.0);
        assert_eq!(metrics.mem_available, 100.0);
        assert!(metrics.validate());
    }

    #[test]
    fn test_system_metrics_validation() {
        let mut metrics = SystemMetrics::default();
        assert!(metrics.validate());

        metrics.cpu_usage = -1.0;
        assert!(!metrics.validate());

        metrics.cpu_usage = 101.0;
        assert!(!metrics.validate());

        metrics.cpu_usage = 50.0;
        assert!(metrics.validate());
    }

    #[test]
    fn test_metrics_collector_creation() {
        let _collector = create_metrics_collector();

        // Just verify that we can create a collector without checking specific type names
        // since they can vary and this test is mainly about ensuring the factory function works
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            // We should be able to create a supported collector on supported platforms
            println!(
                "Created collector on {}",
                crate::platform::get_platform_name()
            );
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            // Should still create a default collector on unsupported platforms
            println!("Created default collector on unsupported platform");
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn test_collect_metrics_on_supported_platforms() {
        // This test runs on supported platforms (Linux and macOS)
        match SystemMetrics::collect() {
            Ok(metrics) => {
                assert!(metrics.validate());
                println!(
                    "Successfully collected metrics on {}",
                    crate::platform::get_platform_name()
                );
            }
            Err(e) => {
                // May not be able to access system resources in some test environments
                println!("Warning: Failed to collect metrics in test environment: {e}");
            }
        }
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    #[test]
    fn test_collect_metrics_on_unsupported_platforms() {
        // This test runs on unsupported platforms
        let result = SystemMetrics::collect();
        assert!(result.is_err());

        if let Err(PwrzvError::UnsupportedPlatform { .. }) = result {
            // Expected error type
        } else {
            panic!("Expected UnsupportedPlatform error");
        }
    }

    #[test]
    fn test_trait_methods_error_handling() {
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            let collector = DefaultMetricsCollector;

            assert!(collector.collect_cpu_stats().is_err());
            assert!(collector.collect_memory_stats().is_err());
            assert!(collector.collect_disk_stats().is_err());
            assert!(collector.collect_network_stats().is_err());
            assert!(collector.collect_fd_stats().is_err());
        }
    }

    #[test]
    fn test_platform_support_detection() {
        // Test that the platform detection logic works correctly
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            assert!(crate::platform::is_supported_platform());
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            assert!(!crate::platform::is_supported_platform());
        }
    }
}
