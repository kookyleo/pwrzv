//! System metrics module
//!
//! Provides system resource monitoring metrics definitions and data collection functions.

use crate::error::{PwrzvError, PwrzvResult};
use crate::platform;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufRead, BufReader};

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

impl SystemMetrics {
    /// Create a new system metrics instance
    pub fn new() -> Self {
        Self::default()
    }

    /// Collect all system metrics
    pub fn collect() -> PwrzvResult<Self> {
        // First check platform compatibility
        platform::check_platform()?;

        let mut metrics = SystemMetrics::new();

        // Collect various metrics, use default values and log warnings if any fails
        match read_cpu_stats() {
            Ok((cpu_usage, cpu_iowait)) => {
                metrics.cpu_usage = cpu_usage;
                metrics.cpu_iowait = cpu_iowait;
            }
            Err(e) => eprintln!("Warning: Failed to read CPU stats: {e}"),
        }

        match read_mem_stats() {
            Ok((mem_available, swap_usage)) => {
                metrics.mem_available = mem_available;
                metrics.swap_usage = swap_usage;
            }
            Err(e) => eprintln!("Warning: Failed to read memory stats: {e}"),
        }

        match read_disk_stats() {
            Ok(disk_usage) => metrics.disk_usage = disk_usage,
            Err(e) => eprintln!("Warning: Failed to read disk stats: {e}"),
        }

        match read_net_stats() {
            Ok(net_usage) => metrics.net_usage = net_usage,
            Err(e) => eprintln!("Warning: Failed to read network stats: {e}"),
        }

        match read_fd_stats() {
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

/// Read CPU statistics from /proc/stat
fn read_cpu_stats() -> PwrzvResult<(f32, f32)> {
    let file =
        File::open("/proc/stat").map_err(|_| PwrzvError::resource_access_error("/proc/stat"))?;
    let reader = BufReader::new(file);

    let line = reader
        .lines()
        .next()
        .ok_or_else(|| PwrzvError::parse_error("No CPU stats line found"))?
        .map_err(PwrzvError::from)?;

    if !line.starts_with("cpu ") {
        return Err(PwrzvError::parse_error("Invalid CPU stats format"));
    }

    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 6 {
        return Err(PwrzvError::parse_error("Incomplete CPU stats"));
    }

    let parse_cpu_field = |index: usize| -> PwrzvResult<u64> {
        parts[index]
            .parse::<u64>()
            .map_err(|_| PwrzvError::parse_error(&format!("Invalid CPU field at index {index}")))
    };

    let user = parse_cpu_field(1)?;
    let nice = parse_cpu_field(2)?;
    let system = parse_cpu_field(3)?;
    let idle = parse_cpu_field(4)?;
    let iowait = parse_cpu_field(5)?;

    let total = user + nice + system + idle + iowait;

    if total == 0 {
        return Ok((0.0, 0.0));
    }

    let cpu_usage = ((total - idle) as f32 / total as f32 * 100.0).min(100.0);
    let cpu_iowait = (iowait as f32 / total as f32 * 100.0).min(100.0);

    Ok((cpu_usage, cpu_iowait))
}

/// Read memory information from /proc/meminfo
fn read_mem_stats() -> PwrzvResult<(f32, f32)> {
    let file = File::open("/proc/meminfo")
        .map_err(|_| PwrzvError::resource_access_error("/proc/meminfo"))?;
    let reader = BufReader::new(file);

    let mut mem_total = 0.0;
    let mut mem_available = 0.0;
    let mut swap_total = 0.0;
    let mut swap_free = 0.0;

    for line in reader.lines() {
        let line = line.map_err(PwrzvError::from)?;
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }

        let value = parts[1].parse::<f32>().unwrap_or(0.0);
        match parts[0] {
            "MemTotal:" => mem_total = value,
            "MemAvailable:" => mem_available = value,
            "SwapTotal:" => swap_total = value,
            "SwapFree:" => swap_free = value,
            _ => {}
        }
    }

    let mem_available_pct = if mem_total > 0.0 {
        (mem_available / mem_total * 100.0).min(100.0)
    } else {
        100.0
    };

    let swap_usage_pct = if swap_total > 0.0 {
        ((swap_total - swap_free) / swap_total * 100.0).min(100.0)
    } else {
        0.0
    };

    Ok((mem_available_pct, swap_usage_pct))
}

/// Read disk I/O statistics from /proc/diskstats
fn read_disk_stats() -> PwrzvResult<f32> {
    let file = File::open("/proc/diskstats")
        .map_err(|_| PwrzvError::resource_access_error("/proc/diskstats"))?;
    let reader = BufReader::new(file);

    let mut total_io_time = 0u64;
    let mut device_count = 0;

    for line in reader.lines() {
        let line = line.map_err(PwrzvError::from)?;
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 14 {
            continue;
        }

        // Only count real disk devices (exclude partitions and virtual devices)
        let device_name = parts[2];
        if device_name.starts_with("sd")
            || device_name.starts_with("nvme")
            || device_name.starts_with("hd")
        {
            #[allow(clippy::collapsible_if)]
            if let Ok(io_time) = parts[13].parse::<u64>() {
                total_io_time += io_time;
                device_count += 1;
            }
        }
    }

    if device_count == 0 {
        return Ok(0.0);
    }

    // Convert average I/O time to utilization percentage
    let avg_io_time = total_io_time as f32 / device_count as f32;
    let disk_usage = (avg_io_time / 1000.0 * 100.0).min(100.0);

    Ok(disk_usage)
}

/// Read network I/O statistics from /proc/net/dev
fn read_net_stats() -> PwrzvResult<f32> {
    let file = File::open("/proc/net/dev")
        .map_err(|_| PwrzvError::resource_access_error("/proc/net/dev"))?;
    let reader = BufReader::new(file);

    let mut total_bytes = 0u64;

    for line in reader.lines() {
        let line = line.map_err(PwrzvError::from)?;

        // Skip lines containing lo (loopback) in interface name
        if line.contains("lo:") {
            continue;
        }

        // Find real network interfaces
        if line.contains("eth")
            || line.contains("enp")
            || line.contains("ens")
            || line.contains("wlan")
        {
            let parts: Vec<&str> = line.split_whitespace().collect();
            #[allow(clippy::collapsible_if)]
            if parts.len() >= 10 {
                if let (Ok(rx_bytes), Ok(tx_bytes)) =
                    (parts[1].parse::<u64>(), parts[9].parse::<u64>())
                {
                    total_bytes += rx_bytes + tx_bytes;
                }
            }
        }
    }

    // Assume 1Gbps network card, maximum bandwidth approximately 125MB/s
    let max_bandwidth = 125_000_000.0;
    let net_usage = (total_bytes as f32 / max_bandwidth).min(100.0);

    Ok(net_usage)
}

/// Read file descriptor usage from /proc/sys/fs/file-nr
fn read_fd_stats() -> PwrzvResult<f32> {
    let file = File::open("/proc/sys/fs/file-nr")
        .map_err(|_| PwrzvError::resource_access_error("/proc/sys/fs/file-nr"))?;
    let reader = BufReader::new(file);

    let line = reader
        .lines()
        .next()
        .ok_or_else(|| PwrzvError::parse_error("No file descriptor stats found"))?
        .map_err(PwrzvError::from)?;

    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 3 {
        return Err(PwrzvError::parse_error(
            "Invalid file descriptor stats format",
        ));
    }

    let used = parts[0]
        .parse::<f32>()
        .map_err(|_| PwrzvError::parse_error("Invalid file descriptor used count"))?;
    let max = parts[2]
        .parse::<f32>()
        .map_err(|_| PwrzvError::parse_error("Invalid file descriptor max count"))?;

    if max <= 0.0 {
        return Ok(0.0);
    }

    let fd_usage = (used / max * 100.0).min(100.0);
    Ok(fd_usage)
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

    #[cfg(target_os = "linux")]
    #[test]
    fn test_collect_metrics_on_linux() {
        // This test only runs on Linux
        match SystemMetrics::collect() {
            Ok(metrics) => {
                assert!(metrics.validate());
            }
            Err(e) => {
                // May not be able to access /proc filesystem in some test environments
                println!(
                    "Warning: Failed to collect metrics in test environment: {}",
                    e
                );
            }
        }
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn test_collect_metrics_on_non_linux() {
        // This test runs on non-Linux platforms
        let result = SystemMetrics::collect();
        assert!(result.is_err());

        if let Err(PwrzvError::UnsupportedPlatform { .. }) = result {
            // Expected error type
        } else {
            panic!("Expected UnsupportedPlatform error");
        }
    }
}
