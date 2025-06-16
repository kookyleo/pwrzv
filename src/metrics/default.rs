//! Default implementation for unsupported platforms
//!
//! Provides a fallback implementation that returns errors for all metrics collection
//! operations on platforms that are not explicitly supported.

use super::MetricsCollector;
use crate::error::{PwrzvError, PwrzvResult};

/// Default metrics collector for unsupported platforms
///
/// This collector returns errors for all operations, indicating that
/// metrics collection is not supported on the current platform.
pub struct DefaultMetricsCollector;

impl MetricsCollector for DefaultMetricsCollector {
    fn collect_cpu_stats(&self) -> PwrzvResult<(f32, f32)> {
        Err(PwrzvError::calculation_error(
            "CPU stats collection not supported on this platform",
        ))
    }

    fn collect_memory_stats(&self) -> PwrzvResult<(f32, f32)> {
        Err(PwrzvError::calculation_error(
            "Memory stats collection not supported on this platform",
        ))
    }

    fn collect_disk_stats(&self) -> PwrzvResult<f32> {
        Err(PwrzvError::calculation_error(
            "Disk stats collection not supported on this platform",
        ))
    }

    fn collect_network_stats(&self) -> PwrzvResult<f32> {
        Err(PwrzvError::calculation_error(
            "Network stats collection not supported on this platform",
        ))
    }

    fn collect_fd_stats(&self) -> PwrzvResult<f32> {
        Err(PwrzvError::calculation_error(
            "FD stats collection not supported on this platform",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_collector_creation() {
        let collector = DefaultMetricsCollector;
        // Just verify we can create the collector
        assert_eq!(std::mem::size_of_val(&collector), 0); // Zero-sized type
    }

    #[test]
    fn test_all_methods_return_errors() {
        let collector = DefaultMetricsCollector;

        // All methods should return errors
        assert!(collector.collect_cpu_stats().is_err());
        assert!(collector.collect_memory_stats().is_err());
        assert!(collector.collect_disk_stats().is_err());
        assert!(collector.collect_network_stats().is_err());
        assert!(collector.collect_fd_stats().is_err());
    }

    #[test]
    fn test_error_messages() {
        let collector = DefaultMetricsCollector;

        // Test that error messages are informative
        if let Err(e) = collector.collect_cpu_stats() {
            assert!(e.to_string().contains("not supported"));
        }

        if let Err(e) = collector.collect_memory_stats() {
            assert!(e.to_string().contains("not supported"));
        }
    }
}
