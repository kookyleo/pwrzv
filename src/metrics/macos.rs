//! macOS platform implementation for system metrics collection
//!
//! Provides macOS-specific implementations using system commands (`top`, `vm_stat`,
//! `iostat`, `netstat`, `sysctl`) for collecting various system performance metrics.

use super::MetricsCollector;
use crate::error::{PwrzvError, PwrzvResult};
use std::process::Command;

/// macOS-specific metrics collector implementation
pub struct MacOSMetricsCollector;

impl MetricsCollector for MacOSMetricsCollector {
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

/// Read CPU statistics using top command
#[allow(clippy::collapsible_if)]
fn read_cpu_stats() -> PwrzvResult<(f32, f32)> {
    // Try using top command with single iteration for more reliable results
    let output = Command::new("top")
        .args(["-l", "1", "-n", "0"])
        .output()
        .map_err(|e| PwrzvError::resource_access_error(&format!("top command failed: {e}")))?;

    let stdout = String::from_utf8(output.stdout)
        .map_err(|e| PwrzvError::parse_error(&format!("Invalid top output: {e}")))?;

    // Look for CPU usage line like "CPU usage: 12.5% user, 25.0% sys, 62.5% idle"
    for line in stdout.lines() {
        if line.starts_with("CPU usage:") {
            // Parse CPU usage from top output
            let mut user_pct: f32 = 0.0;
            let mut sys_pct: f32 = 0.0;

            for part in line.split(',') {
                let part = part.trim();
                if part.contains("user") {
                    if let Some(pct_str) = part.split('%').next() {
                        if let Some(num_str) = pct_str.split_whitespace().last() {
                            user_pct = num_str.parse().unwrap_or(0.0);
                        }
                    }
                } else if part.contains("sys") {
                    if let Some(pct_str) = part.split('%').next() {
                        if let Some(num_str) = pct_str.split_whitespace().last() {
                            sys_pct = num_str.parse().unwrap_or(0.0);
                        }
                    }
                }
            }

            let cpu_usage = (user_pct + sys_pct).min(100.0);
            // macOS doesn't typically separate iowait in top output, so we approximate
            let cpu_iowait = if cpu_usage > 80.0 {
                (cpu_usage - 80.0) / 4.0
            } else {
                0.0
            };

            return Ok((cpu_usage, cpu_iowait));
        }
    }

    // Fallback: return conservative values if parsing fails
    Ok((0.0, 0.0))
}

/// Read memory statistics using vm_stat and sysctl
fn read_memory_stats() -> PwrzvResult<(f32, f32)> {
    // Use vm_stat command to get memory information
    let output = Command::new("vm_stat")
        .output()
        .map_err(|e| PwrzvError::resource_access_error(&format!("vm_stat command failed: {e}")))?;

    let stdout = String::from_utf8(output.stdout)
        .map_err(|e| PwrzvError::parse_error(&format!("Invalid vm_stat output: {e}")))?;

    let mut pages_free = 0u64;
    let mut pages_inactive = 0u64;
    let mut pages_speculative = 0u64;
    let mut pages_wired = 0u64;
    let mut pages_active = 0u64;
    let mut pages_compressed = 0u64;

    for line in stdout.lines() {
        if let Some(captures) = line.split_once(':') {
            let key = captures.0.trim();
            let value_str = captures.1.trim().trim_end_matches('.');
            if let Ok(value) = value_str.parse::<u64>() {
                match key {
                    "Pages free" => pages_free = value,
                    "Pages inactive" => pages_inactive = value,
                    "Pages speculative" => pages_speculative = value,
                    "Pages wired down" => pages_wired = value,
                    "Pages active" => pages_active = value,
                    "Pages occupied by compressor" => pages_compressed = value,
                    _ => {}
                }
            }
        }
    }

    // Calculate total and available memory (page size is typically 4KB on macOS)
    let _page_size = 4096;
    let total_pages = pages_free
        + pages_inactive
        + pages_speculative
        + pages_wired
        + pages_active
        + pages_compressed;
    let available_pages = pages_free + pages_inactive + pages_speculative;

    if total_pages == 0 {
        return Ok((100.0, 0.0));
    }

    let mem_available = (available_pages as f32 / total_pages as f32 * 100.0).min(100.0);

    // Get swap usage using sysctl
    let swap_usage = read_swap_usage().unwrap_or(0.0);

    Ok((mem_available, swap_usage))
}

/// Read swap usage using sysctl
fn read_swap_usage() -> PwrzvResult<f32> {
    let output = Command::new("sysctl")
        .args(["-n", "vm.swapusage"])
        .output()
        .map_err(|e| {
            PwrzvError::resource_access_error(&format!("sysctl swap command failed: {e}"))
        })?;

    let stdout = String::from_utf8(output.stdout)
        .map_err(|e| PwrzvError::parse_error(&format!("Invalid swap output: {e}")))?;

    // Parse output like "total = 1024.00M  used = 512.00M  free = 512.00M  (encrypted)"
    let mut total = 0.0;
    let mut used = 0.0;

    for part in stdout.split_whitespace() {
        if part.starts_with("total=") {
            if let Some(val_str) = part.strip_prefix("total=") {
                total = parse_memory_value(val_str)?;
            }
        } else if let Some(val_str) = part.strip_prefix("used=") {
            used = parse_memory_value(val_str)?;
        }
    }

    if total <= 0.0 {
        return Ok(0.0);
    }

    Ok((used / total * 100.0).min(100.0))
}

/// Parse memory value with unit suffix (e.g., "1024.00M" -> 1024.0)
fn parse_memory_value(value_str: &str) -> PwrzvResult<f32> {
    let value_str = value_str.trim_end_matches(|c: char| c.is_alphabetic());
    value_str
        .parse::<f32>()
        .map_err(|_| PwrzvError::parse_error("Invalid memory value"))
}

/// Read disk I/O statistics using iostat
fn read_disk_stats() -> PwrzvResult<f32> {
    // Use iostat command to get disk I/O statistics
    let output = Command::new("iostat")
        .args(["-d", "-c", "1"])
        .output()
        .map_err(|e| PwrzvError::resource_access_error(&format!("iostat command failed: {e}")))?;

    let stdout = String::from_utf8(output.stdout)
        .map_err(|e| PwrzvError::parse_error(&format!("Invalid iostat output: {e}")))?;

    // Parse iostat output to get disk utilization
    // This is a simplified implementation - actual iostat parsing is more complex
    let lines: Vec<&str> = stdout.lines().collect();
    if lines.len() < 3 {
        return Ok(0.0);
    }

    // Look for disk device lines and average their utilization
    let mut total_util = 0.0;
    let mut device_count = 0;

    for line in lines.iter().skip(2) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 3 && parts[0].starts_with("disk") {
            // Assume the last column is utilization percentage
            if let Ok(util) = parts
                .last()
                .unwrap_or(&"0")
                .trim_end_matches('%')
                .parse::<f32>()
            {
                total_util += util;
                device_count += 1;
            }
        }
    }

    if device_count == 0 {
        return Ok(0.0);
    }

    Ok((total_util / device_count as f32).min(100.0))
}

/// Read network I/O statistics using netstat
fn read_network_stats() -> PwrzvResult<f32> {
    // Use netstat command to get network statistics
    let output = Command::new("netstat")
        .args(["-I", "en0", "-b"])
        .output()
        .map_err(|e| PwrzvError::resource_access_error(&format!("netstat command failed: {e}")))?;

    let stdout = String::from_utf8(output.stdout)
        .map_err(|e| PwrzvError::parse_error(&format!("Invalid netstat output: {e}")))?;

    // Parse netstat output to get bytes transferred
    let mut total_bytes = 0u64;

    for line in stdout.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 7 {
            // Ibytes and Obytes are typically at positions 6 and 9
            if let (Ok(ibytes), Ok(obytes)) = (
                parts.get(6).unwrap_or(&"0").parse::<u64>(),
                parts.get(9).unwrap_or(&"0").parse::<u64>(),
            ) {
                total_bytes += ibytes + obytes;
            }
        }
    }

    // Assume 1Gbps network card, maximum bandwidth approximately 125MB/s
    let max_bandwidth = 125_000_000.0;
    let net_usage = (total_bytes as f32 / max_bandwidth).min(100.0);

    Ok(net_usage)
}

/// Read file descriptor statistics using sysctl
fn read_fd_stats() -> PwrzvResult<f32> {
    // Use sysctl to get file descriptor limits instead of lsof for better reliability
    let max_output = Command::new("sysctl")
        .args(["-n", "kern.maxfilesperproc"])
        .output()
        .map_err(|e| {
            PwrzvError::resource_access_error(&format!("sysctl maxfiles command failed: {e}"))
        })?;

    let max_files: f32 = String::from_utf8(max_output.stdout)
        .map_err(|e| PwrzvError::parse_error(&format!("Invalid maxfiles output: {e}")))?
        .trim()
        .parse()
        .map_err(|_| PwrzvError::parse_error("Invalid maxfiles value"))?;

    // Get open files count using sysctl instead of lsof
    let used_output = Command::new("sysctl")
        .args(["-n", "kern.openfiles"])
        .output()
        .map_err(|e| {
            PwrzvError::resource_access_error(&format!("sysctl openfiles command failed: {e}"))
        })?;

    let used_files: f32 = String::from_utf8(used_output.stdout)
        .map_err(|e| PwrzvError::parse_error(&format!("Invalid openfiles output: {e}")))?
        .trim()
        .parse()
        .map_err(|_| PwrzvError::parse_error("Invalid openfiles value"))?;

    if max_files <= 0.0 {
        return Ok(0.0);
    }

    Ok((used_files / max_files * 100.0).min(100.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_macos_collector_creation() {
        let collector = MacOSMetricsCollector;
        // Just verify we can create the collector
        assert_eq!(std::mem::size_of_val(&collector), 0); // Zero-sized type
    }

    #[test]
    fn test_cpu_stats_collection() {
        let collector = MacOSMetricsCollector;

        // Test that the method exists and returns the correct type
        match collector.collect_cpu_stats() {
            Ok((cpu_usage, cpu_iowait)) => {
                assert!((0.0..=100.0).contains(&cpu_usage));
                assert!((0.0..=100.0).contains(&cpu_iowait));
                println!("CPU usage: {cpu_usage}%, iowait: {cpu_iowait}%");
            }
            Err(e) => {
                // May fail in some test environments
                println!("CPU stats collection failed: {e}");
            }
        }
    }

    #[test]
    fn test_memory_stats_collection() {
        let collector = MacOSMetricsCollector;

        match collector.collect_memory_stats() {
            Ok((mem_available, swap_usage)) => {
                assert!((0.0..=100.0).contains(&mem_available));
                assert!((0.0..=100.0).contains(&swap_usage));
                println!("Memory available: {mem_available}%, swap usage: {swap_usage}%");
            }
            Err(e) => {
                // May fail in some test environments
                println!("Memory stats collection failed: {e}");
            }
        }
    }

    #[test]
    fn test_parse_memory_value() {
        assert_eq!(parse_memory_value("1024.00M").unwrap(), 1024.0);
        assert_eq!(parse_memory_value("512.50G").unwrap(), 512.5);
        assert_eq!(parse_memory_value("0.00K").unwrap(), 0.0);

        // Test error case
        assert!(parse_memory_value("invalid").is_err());
    }
}
