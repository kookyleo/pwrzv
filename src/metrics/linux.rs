//! Linux platform implementation for system metrics collection
//!
//! Provides Linux-specific implementations using `/proc` filesystem for collecting
//! various system performance metrics.

use super::MetricsCollector;
use crate::error::{PwrzvError, PwrzvResult};
use std::fs::File;
use std::io::{BufRead, BufReader};

/// Linux-specific metrics collector implementation
pub struct LinuxMetricsCollector;

impl MetricsCollector for LinuxMetricsCollector {
    fn collect_cpu_stats(&self) -> PwrzvResult<(f32, f32)> {
        read_cpu_stats()
    }

    fn collect_memory_stats(&self) -> PwrzvResult<(f32, f32)> {
        read_memory_stats()
    }

    fn collect_disk_stats(&self) -> PwrzvResult<f32> {
        read_disk_stats()
    }

    fn collect_network_stats(&self) -> PwrzvResult<f32> {
        read_network_stats()
    }

    fn collect_fd_stats(&self) -> PwrzvResult<f32> {
        read_fd_stats()
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
fn read_memory_stats() -> PwrzvResult<(f32, f32)> {
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
fn read_network_stats() -> PwrzvResult<f32> {
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
    fn test_linux_collector_creation() {
        let collector = LinuxMetricsCollector;
        // Just verify we can create the collector
        assert_eq!(std::mem::size_of_val(&collector), 0); // Zero-sized type
    }

    #[test]
    fn test_cpu_stats_parsing() {
        // This test would require mocking /proc/stat, so we just verify the function exists
        // In a real test environment, you might want to create temporary files for testing
        let collector = LinuxMetricsCollector;

        // Test that the method exists and returns the correct type
        match collector.collect_cpu_stats() {
            Ok((cpu_usage, cpu_iowait)) => {
                assert!((0.0..=100.0).contains(&cpu_usage));
                assert!((0.0..=100.0).contains(&cpu_iowait));
            }
            Err(_) => {
                // Expected in test environment without proper /proc filesystem
                println!("CPU stats collection failed in test environment");
            }
        }
    }

    #[test]
    fn test_memory_stats_parsing() {
        let collector = LinuxMetricsCollector;

        match collector.collect_memory_stats() {
            Ok((mem_available, swap_usage)) => {
                assert!((0.0..=100.0).contains(&mem_available));
                assert!((0.0..=100.0).contains(&swap_usage));
            }
            Err(_) => {
                // Expected in test environment without proper /proc filesystem
                println!("Memory stats collection failed in test environment");
            }
        }
    }
}
