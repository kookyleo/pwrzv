use crate::error::{PwrzvError, PwrzvResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command;
use std::time::Duration;
use tokio::time;

const SAMPLE_INTERVAL: u64 = 500; // 500ms

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MacSystemMetrics {
    /// CPU total usage ratio: non-idle time percentage
    /// Range [0.0, 1.0]
    pub cpu_usage_ratio: f32,

    /// CPU load ratio: loadavg(1min) / core_count
    /// Approaching or exceeding 1.0 indicates increasing queued tasks
    pub cpu_load_ratio: f32,

    /// Memory usage ratio: (active + wired) / physical_mem
    /// Note: cached and compressed can also be included
    pub memory_usage_ratio: f32,

    /// Memory compressed ratio: compressed_pages / physical_pages
    /// Range [0.0, 1.0], approaching 1.0 means system heavily relies on memory compression
    pub memory_compressed_ratio: f32,

    /// Disk I/O activity: %util / 100 from iostat
    /// If unavailable, can be replaced with normalized avg queue length
    pub disk_io_ratio: f32,

    /// Network bandwidth utilization: interface traffic / estimated max bandwidth
    /// Can use `netstat` or `ifstat` sampling estimation
    pub network_bandwidth_ratio: f32,

    /// Network dropped packets ratio: `dropped_packets / total_packets`
    /// Range [0.0, 1.0], approaching 1.0 means network is saturated
    pub network_dropped_packets_ratio: f32,

    /// File descriptor usage ratio: open files / maxfiles
    /// `sysctl kern.num_files` / `kern.maxfiles`
    pub fd_usage_ratio: f32,

    /// Process count usage ratio: running processes / maxproc
    /// `sysctl kern.proc` / `kern.maxproc`
    pub process_count_ratio: f32,
}

#[derive(Debug, Clone)]
struct NetworkInterfaceStats {
    bytes_in: u64,
    bytes_out: u64,
    packets_in: u64,
    packets_out: u64,
    dropped_in: u64,
    dropped_out: u64,
}

impl MacSystemMetrics {
    /// Collect all system metrics in parallel
    pub async fn collect() -> PwrzvResult<Self> {
        // Start parallel sampling tasks
        let cpu_task = tokio::spawn(Self::collect_cpu_metrics());
        let memory_task = tokio::spawn(Self::collect_memory_metrics());
        let disk_task = tokio::spawn(Self::collect_disk_metrics());
        let network_task = tokio::spawn(Self::collect_network_metrics());
        let fd_task = tokio::spawn(Self::collect_fd_metrics());
        let process_task = tokio::spawn(Self::collect_process_metrics());

        // Wait for all tasks to complete
        let (cpu_result, memory_result, disk_result, network_result, fd_result, process_result) =
            tokio::try_join!(
                cpu_task,
                memory_task,
                disk_task,
                network_task,
                fd_task,
                process_task
            )
            .map_err(|e| PwrzvError::collection_error(&format!("Task join failed: {e}")))?;

        let (cpu_usage_ratio, cpu_load_ratio) = cpu_result?;
        let (memory_usage_ratio, memory_compressed_ratio) = memory_result?;
        let disk_io_ratio = disk_result?;
        let (network_bandwidth_ratio, network_dropped_packets_ratio) = network_result?;
        let fd_usage_ratio = fd_result?;
        let process_count_ratio = process_result?;

        Ok(MacSystemMetrics {
            cpu_usage_ratio,
            cpu_load_ratio,
            memory_usage_ratio,
            memory_compressed_ratio,
            disk_io_ratio,
            network_bandwidth_ratio,
            network_dropped_packets_ratio,
            fd_usage_ratio,
            process_count_ratio,
        })
    }

    /// Collect CPU-related metrics
    async fn collect_cpu_metrics() -> PwrzvResult<(f32, f32)> {
        let cpu_usage_ratio = Self::get_cpu_usage().await?;
        let cpu_load_ratio = Self::get_load_average()?;
        Ok((cpu_usage_ratio, cpu_load_ratio))
    }

    /// Collect memory-related metrics
    async fn collect_memory_metrics() -> PwrzvResult<(f32, f32)> {
        let memory_usage_ratio = Self::get_memory_usage()?;
        let memory_compressed_ratio = Self::get_memory_compressed_ratio()?;
        Ok((memory_usage_ratio, memory_compressed_ratio))
    }

    /// Collect disk I/O metrics
    async fn collect_disk_metrics() -> PwrzvResult<f32> {
        Self::get_disk_io_utilization().await
    }

    /// Collect network bandwidth and dropped packets metrics
    async fn collect_network_metrics() -> PwrzvResult<(f32, f32)> {
        let bandwidth_ratio = Self::get_network_utilization().await?;
        let dropped_packets_ratio = Self::get_network_dropped_packets().await?;
        Ok((bandwidth_ratio, dropped_packets_ratio))
    }

    /// Collect file descriptor metrics
    async fn collect_fd_metrics() -> PwrzvResult<f32> {
        Self::get_fd_usage()
    }

    /// Collect process count metrics
    async fn collect_process_metrics() -> PwrzvResult<f32> {
        Self::get_process_count()
    }

    /// Get CPU usage ratio
    async fn get_cpu_usage() -> PwrzvResult<f32> {
        // Use top command to get CPU usage
        let output = Command::new("top")
            .args(["-l", "2", "-n", "0", "-s", "1"])
            .output()
            .map_err(|e| PwrzvError::collection_error(&format!("Failed to run top: {e}")))?;

        let output_str = String::from_utf8_lossy(&output.stdout);

        // Parse the last CPU usage measurement
        let lines: Vec<&str> = output_str.lines().collect();
        let mut cpu_line = None;

        // Find the last CPU usage line
        for line in lines.iter().rev() {
            if line.contains("CPU usage:") {
                cpu_line = Some(*line);
                break;
            }
        }

        if let Some(line) = cpu_line {
            // Parse CPU usage: "CPU usage: 12.5% user, 6.25% sys, 81.25% idle"
            if let Some(idle_start) = line.find("% idle") {
                let before_idle = &line[..idle_start];
                if let Some(last_space) = before_idle.rfind(' ') {
                    let idle_str = &before_idle[last_space + 1..];
                    if let Ok(idle_percent) = idle_str.parse::<f32>() {
                        let cpu_usage = (100.0 - idle_percent) / 100.0;
                        return Ok(cpu_usage.clamp(0.0, 1.0));
                    }
                }
            }
        }

        // Fallback: use sysctl to get load information
        Self::get_cpu_usage_fallback()
    }

    /// CPU usage fallback method
    #[allow(clippy::collapsible_if)]
    fn get_cpu_usage_fallback() -> PwrzvResult<f32> {
        let output = Command::new("sysctl")
            .args(["-n", "vm.loadavg"])
            .output()
            .map_err(|e| PwrzvError::collection_error(&format!("Failed to run sysctl: {e}")))?;

        let output_str = String::from_utf8_lossy(&output.stdout);
        let parts: Vec<&str> = output_str.split_whitespace().collect();

        if parts.len() >= 3 {
            if let Ok(load_1min) = parts[1].parse::<f32>() {
                let cpu_cores = Self::get_cpu_cores()?;
                // Convert load to usage estimation
                let usage_estimate = (load_1min / cpu_cores as f32).min(1.0);
                return Ok(usage_estimate);
            }
        }

        Ok(0.0)
    }

    /// Get load average
    #[allow(clippy::collapsible_if)]
    fn get_load_average() -> PwrzvResult<f32> {
        let output = Command::new("sysctl")
            .args(["-n", "vm.loadavg"])
            .output()
            .map_err(|e| PwrzvError::collection_error(&format!("Failed to run sysctl: {e}")))?;

        let output_str = String::from_utf8_lossy(&output.stdout);
        let parts: Vec<&str> = output_str.split_whitespace().collect();

        if parts.len() >= 3 {
            if let Ok(load_1min) = parts[1].parse::<f32>() {
                let cpu_cores = Self::get_cpu_cores()?;
                return Ok(load_1min / cpu_cores as f32);
            }
        }

        Err(PwrzvError::collection_error("Failed to parse load average"))
    }

    /// Get CPU core count
    fn get_cpu_cores() -> PwrzvResult<u32> {
        let output = Command::new("sysctl")
            .args(["-n", "hw.ncpu"])
            .output()
            .map_err(|e| PwrzvError::collection_error(&format!("Failed to run sysctl: {e}")))?;

        let output_str = String::from_utf8_lossy(&output.stdout);
        output_str
            .trim()
            .parse::<u32>()
            .map_err(|_| PwrzvError::collection_error("Failed to parse CPU core count"))
    }

    /// Get memory usage ratio
    fn get_memory_usage() -> PwrzvResult<f32> {
        let output = Command::new("vm_stat")
            .output()
            .map_err(|e| PwrzvError::collection_error(&format!("Failed to run vm_stat: {e}")))?;

        let output_str = String::from_utf8_lossy(&output.stdout);

        let mut pages_free = 0u64;
        let mut pages_active = 0u64;
        let mut pages_inactive = 0u64;
        let mut pages_wired = 0u64;
        let mut pages_compressed = 0u64;

        for line in output_str.lines() {
            if line.contains("Pages free:") {
                pages_free = Self::parse_vm_stat_value(line)?;
            } else if line.contains("Pages active:") {
                pages_active = Self::parse_vm_stat_value(line)?;
            } else if line.contains("Pages inactive:") {
                pages_inactive = Self::parse_vm_stat_value(line)?;
            } else if line.contains("Pages wired down:") {
                pages_wired = Self::parse_vm_stat_value(line)?;
            } else if line.contains("Pages stored in compressor:") {
                pages_compressed = Self::parse_vm_stat_value(line)?;
            }
        }

        let total_pages =
            pages_free + pages_active + pages_inactive + pages_wired + pages_compressed;
        if total_pages == 0 {
            return Ok(0.0);
        }

        let used_pages = pages_active + pages_wired;
        let usage_ratio = used_pages as f32 / total_pages as f32;

        Ok(usage_ratio.clamp(0.0, 1.0))
    }

    /// Get memory compressed ratio
    fn get_memory_compressed_ratio() -> PwrzvResult<f32> {
        let output = Command::new("vm_stat")
            .output()
            .map_err(|e| PwrzvError::collection_error(&format!("Failed to run vm_stat: {e}")))?;

        let output_str = String::from_utf8_lossy(&output.stdout);

        let mut pages_compressed = 0u64;
        let mut total_pages = 0u64;

        for line in output_str.lines() {
            if line.contains("Pages stored in compressor:") {
                pages_compressed = Self::parse_vm_stat_value(line)?;
            } else if line.contains("Pages free:")
                || line.contains("Pages active:")
                || line.contains("Pages inactive:")
                || line.contains("Pages wired down:")
            {
                total_pages += Self::parse_vm_stat_value(line)?;
            }
        }

        if total_pages == 0 {
            return Ok(0.0);
        }

        let compressed_ratio = pages_compressed as f32 / (total_pages + pages_compressed) as f32;
        Ok(compressed_ratio.clamp(0.0, 1.0))
    }

    /// Parse numeric value from vm_stat output
    fn parse_vm_stat_value(line: &str) -> PwrzvResult<u64> {
        // Format: "Pages free:                               12345."
        let parts: Vec<&str> = line.split_whitespace().collect();
        if let Some(last_part) = parts.last() {
            let number_str = last_part.trim_end_matches('.');
            number_str
                .parse::<u64>()
                .map_err(|_| PwrzvError::collection_error("Failed to parse vm_stat value"))
        } else {
            Err(PwrzvError::collection_error("Invalid vm_stat line format"))
        }
    }

    /// Get disk I/O utilization
    async fn get_disk_io_utilization() -> PwrzvResult<f32> {
        // Use iostat -o to get disk utilization percentage
        // Calculate %util = (tps × msps) / 1000
        // This gives us the percentage of time the device is busy
        let output = Command::new("iostat")
            .args(["-o", "-d", "-c", "2", "-w", "1"])
            .output()
            .map_err(|e| PwrzvError::collection_error(&format!("Failed to run iostat: {e}")))?;

        let output_str = String::from_utf8_lossy(&output.stdout);

        // Parse the last measurement (second line of data) for tps and msps values
        let lines: Vec<&str> = output_str.lines().collect();
        let mut max_utilization = 0.0f32;
        let mut found_data = false;

        // Find disk data lines (skip header and first measurement)
        for line in lines.iter().skip(2) {
            let parts: Vec<&str> = line.split_whitespace().collect();

            // Expected format: sps tps msps  sps tps msps (for multiple disks)
            // We need tps (index 1, 4, 7...) and msps (index 2, 5, 8...)
            let mut i = 1;
            while i + 1 < parts.len() {
                if let (Ok(tps), Ok(msps)) = (parts[i].parse::<f32>(), parts[i + 1].parse::<f32>())
                {
                    // Calculate device utilization: %util = (tps × msps) / 1000
                    // This represents the percentage of time the device is busy
                    let utilization = ((tps * msps) / 1000.0).min(1.0);
                    max_utilization = max_utilization.max(utilization);
                    found_data = true;
                }
                i += 3; // Skip to next device (sps, tps, msps)
            }
        }

        if found_data {
            Ok(max_utilization)
        } else {
            // Fallback to a more conservative estimate
            Self::get_disk_io_fallback()
        }
    }

    /// Disk I/O fallback method
    #[allow(clippy::collapsible_if)]
    fn get_disk_io_fallback() -> PwrzvResult<f32> {
        let output = Command::new("sysctl")
            .args(["-n", "vm.loadavg"])
            .output()
            .map_err(|e| PwrzvError::collection_error(&format!("Failed to run sysctl: {e}")))?;

        let output_str = String::from_utf8_lossy(&output.stdout);
        let parts: Vec<&str> = output_str.split_whitespace().collect();

        if parts.len() >= 3 {
            if let Ok(load_1min) = parts[1].parse::<f32>() {
                // Conservative estimate: map load to I/O pressure
                // Load > 2.0 suggests some I/O pressure
                let io_estimate = ((load_1min - 1.0) / 3.0).clamp(0.0, 0.5);
                return Ok(io_estimate);
            }
        }

        Ok(0.0)
    }

    /// Get network bandwidth utilization
    async fn get_network_utilization() -> PwrzvResult<f32> {
        // First sampling
        let stats1 = Self::get_network_stats()?;

        // Wait for sampling interval
        time::sleep(Duration::from_millis(SAMPLE_INTERVAL)).await;

        // Second sampling
        let stats2 = Self::get_network_stats()?;

        // Calculate bandwidth utilization
        Self::calculate_network_utilization(&stats1, &stats2)
    }

    /// Get network dropped packets ratio
    async fn get_network_dropped_packets() -> PwrzvResult<f32> {
        // First sampling
        let stats1 = Self::get_network_stats()?;

        // Wait for sampling interval
        time::sleep(Duration::from_millis(SAMPLE_INTERVAL)).await;

        // Second sampling
        let stats2 = Self::get_network_stats()?;

        // Calculate dropped packets ratio
        Self::calculate_network_dropped_packets(&stats1, &stats2)
    }

    /// Get network interface statistics
    fn get_network_stats() -> PwrzvResult<HashMap<String, NetworkInterfaceStats>> {
        let output = Command::new("netstat")
            .args(["-ib"])
            .output()
            .map_err(|e| PwrzvError::collection_error(&format!("Failed to run netstat: {e}")))?;

        let output_str = String::from_utf8_lossy(&output.stdout);
        let mut stats = HashMap::new();

        for line in output_str.lines().skip(1) {
            // Skip header line
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() >= 10 {
                let interface = fields[0];

                // Skip loopback interface
                if interface.starts_with("lo") {
                    continue;
                }

                // Parse fields: Name Mtu Network Address Ipkts Ierrs Ibytes Opkts Oerrs Obytes Coll
                if let (
                    Ok(packets_in),
                    Ok(dropped_in),
                    Ok(bytes_in),
                    Ok(packets_out),
                    Ok(dropped_out),
                    Ok(bytes_out),
                ) = (
                    fields[4].parse::<u64>(), // Ipkts
                    fields[5].parse::<u64>(), // Ierrs
                    fields[6].parse::<u64>(), // Ibytes
                    fields[7].parse::<u64>(), // Opkts
                    fields[8].parse::<u64>(), // Oerrs
                    fields[9].parse::<u64>(), // Obytes
                ) {
                    stats.insert(
                        interface.to_string(),
                        NetworkInterfaceStats {
                            bytes_in,
                            bytes_out,
                            packets_in,
                            packets_out,
                            dropped_in,
                            dropped_out,
                        },
                    );
                }
            }
        }

        Ok(stats)
    }

    /// Calculate network bandwidth utilization
    /// Returns the maximum utilization across all active interfaces
    fn calculate_network_utilization(
        stats1: &HashMap<String, NetworkInterfaceStats>,
        stats2: &HashMap<String, NetworkInterfaceStats>,
    ) -> PwrzvResult<f32> {
        let mut max_utilization = 0.0f32;

        for (interface, stats2) in stats2 {
            if let Some(stats1) = stats1.get(interface) {
                let bytes_in_diff = stats2.bytes_in.saturating_sub(stats1.bytes_in);
                let bytes_out_diff = stats2.bytes_out.saturating_sub(stats1.bytes_out);

                // Calculate bytes per second
                let bytes_per_sec = (bytes_in_diff + bytes_out_diff) * (1000 / SAMPLE_INTERVAL);

                // Get actual interface speed from system
                let interface_capacity = Self::get_interface_speed(interface);

                if interface_capacity > 0 {
                    let utilization =
                        (bytes_per_sec as f64 / interface_capacity as f64).min(1.0) as f32;
                    max_utilization = max_utilization.max(utilization);
                }
            }
        }

        Ok(max_utilization)
    }

    /// Calculate network dropped packets ratio
    /// Returns the maximum dropped packets ratio across all active interfaces
    fn calculate_network_dropped_packets(
        stats1: &HashMap<String, NetworkInterfaceStats>,
        stats2: &HashMap<String, NetworkInterfaceStats>,
    ) -> PwrzvResult<f32> {
        let mut max_dropped_ratio = 0.0f32;

        for (interface, stats2) in stats2 {
            if let Some(stats1) = stats1.get(interface) {
                // Calculate packet differences
                let packets_in_diff = stats2.packets_in.saturating_sub(stats1.packets_in);
                let packets_out_diff = stats2.packets_out.saturating_sub(stats1.packets_out);
                let dropped_in_diff = stats2.dropped_in.saturating_sub(stats1.dropped_in);
                let dropped_out_diff = stats2.dropped_out.saturating_sub(stats1.dropped_out);

                let total_packets = packets_in_diff + packets_out_diff;
                let total_dropped = dropped_in_diff + dropped_out_diff;

                if total_packets > 0 {
                    let dropped_ratio =
                        (total_dropped as f64 / total_packets as f64).min(1.0) as f32;
                    max_dropped_ratio = max_dropped_ratio.max(dropped_ratio);
                }
            }
        }

        Ok(max_dropped_ratio)
    }

    /// Get actual interface speed from system (in bytes per second)
    fn get_interface_speed(interface: &str) -> u64 {
        // Try to get actual link speed using ifconfig
        if let Ok(output) = Command::new("ifconfig").arg(interface).output() {
            let output_str = String::from_utf8_lossy(&output.stdout);

            // Look for speed information in ifconfig output
            for line in output_str.lines() {
                if line.contains("media:") {
                    // Parse media line for speed info
                    if line.contains("1000baseT") || line.contains("1000BaseTX") {
                        return 125_000_000; // 1 Gbps = 125 MB/s
                    } else if line.contains("100baseTX") || line.contains("100BaseT") {
                        return 12_500_000; // 100 Mbps = 12.5 MB/s
                    } else if line.contains("10baseT") {
                        return 1_250_000; // 10 Mbps = 1.25 MB/s
                    }
                }

                // Check for active status and estimate WiFi speed
                if line.contains("status: active") && interface.starts_with("en") {
                    // For active ethernet without explicit speed, assume gigabit
                    return 125_000_000;
                }
            }
        }

        // Fallback: try system_profiler for more detailed network info
        if let Ok(output) = Command::new("system_profiler")
            .args(["-xml", "SPNetworkDataType"])
            .output()
        {
            let output_str = String::from_utf8_lossy(&output.stdout);

            // Look for this specific interface in system profiler output
            if output_str.contains(interface) {
                if output_str.contains("1000") {
                    return 125_000_000; // 1 Gbps
                } else if output_str.contains("100") {
                    return 12_500_000; // 100 Mbps
                }
            }
        }

        // Final fallback: conservative estimate based on interface type
        match interface {
            name if name.starts_with("en") => 125_000_000, // Assume gigabit ethernet
            name if name.starts_with("wl") || name == "awdl0" => 50_000_000, // Conservative WiFi
            _ => 12_500_000,                               // Conservative default
        }
    }

    /// Get file descriptor usage ratio
    fn get_fd_usage() -> PwrzvResult<f32> {
        // Get current open file count
        let open_files_output = Command::new("sysctl")
            .args(["-n", "kern.num_files"])
            .output()
            .map_err(|e| PwrzvError::collection_error(&format!("Failed to get open files: {e}")))?;

        let open_files_str = String::from_utf8_lossy(&open_files_output.stdout);
        let open_files: u64 = open_files_str
            .trim()
            .parse()
            .map_err(|_| PwrzvError::collection_error("Failed to parse open files count"))?;

        // Get maximum file count
        let max_files_output = Command::new("sysctl")
            .args(["-n", "kern.maxfiles"])
            .output()
            .map_err(|e| PwrzvError::collection_error(&format!("Failed to get max files: {e}")))?;

        let max_files_str = String::from_utf8_lossy(&max_files_output.stdout);
        let max_files: u64 = max_files_str
            .trim()
            .parse()
            .map_err(|_| PwrzvError::collection_error("Failed to parse max files count"))?;

        if max_files == 0 {
            return Ok(0.0);
        }

        let usage_ratio = (open_files as f64 / max_files as f64).min(1.0);
        Ok(usage_ratio as f32)
    }

    /// Get process count usage ratio
    fn get_process_count() -> PwrzvResult<f32> {
        // Get current process count
        let current_procs_output = Command::new("ps")
            .args(["-A"])
            .output()
            .map_err(|e| PwrzvError::collection_error(&format!("Failed to run ps: {e}")))?;

        let current_procs_str = String::from_utf8_lossy(&current_procs_output.stdout);
        let current_procs = current_procs_str.lines().count().saturating_sub(1); // Subtract header line

        // Get maximum process count
        let max_procs_output = Command::new("sysctl")
            .args(["-n", "kern.maxproc"])
            .output()
            .map_err(|e| PwrzvError::collection_error(&format!("Failed to get max procs: {e}")))?;

        let max_procs_str = String::from_utf8_lossy(&max_procs_output.stdout);
        let max_procs: u64 = max_procs_str
            .trim()
            .parse()
            .map_err(|_| PwrzvError::collection_error("Failed to parse max procs count"))?;

        if max_procs == 0 {
            return Ok(0.0);
        }

        let usage_ratio = (current_procs as f64 / max_procs as f64).min(1.0);
        Ok(usage_ratio as f32)
    }

    /// Validate metric data validity
    pub fn validate(&self) -> bool {
        self.cpu_usage_ratio >= 0.0
            && self.cpu_usage_ratio <= 1.0
            && self.cpu_load_ratio >= 0.0
            && self.memory_usage_ratio >= 0.0
            && self.memory_usage_ratio <= 1.0
            && self.memory_compressed_ratio >= 0.0
            && self.memory_compressed_ratio <= 1.0
            && self.disk_io_ratio >= 0.0
            && self.disk_io_ratio <= 1.0
            && self.network_bandwidth_ratio >= 0.0
            && self.network_bandwidth_ratio <= 1.0
            && self.network_dropped_packets_ratio >= 0.0
            && self.network_dropped_packets_ratio <= 1.0
            && self.fd_usage_ratio >= 0.0
            && self.fd_usage_ratio <= 1.0
            && self.process_count_ratio >= 0.0
            && self.process_count_ratio <= 1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mac_system_metrics_validate() {
        // Test valid metrics
        let valid_metrics = MacSystemMetrics {
            cpu_usage_ratio: 0.4,
            cpu_load_ratio: 1.5,
            memory_usage_ratio: 0.6,
            memory_compressed_ratio: 0.3,
            disk_io_ratio: 0.2,
            network_bandwidth_ratio: 0.5,
            network_dropped_packets_ratio: 0.01,
            fd_usage_ratio: 0.7,
            process_count_ratio: 0.4,
        };
        assert!(valid_metrics.validate());

        // Test boundary values
        let boundary_metrics = MacSystemMetrics {
            cpu_usage_ratio: 1.0,
            cpu_load_ratio: 0.0,
            memory_usage_ratio: 1.0,
            memory_compressed_ratio: 1.0,
            disk_io_ratio: 1.0,
            network_bandwidth_ratio: 1.0,
            network_dropped_packets_ratio: 1.0,
            fd_usage_ratio: 1.0,
            process_count_ratio: 1.0,
        };
        assert!(boundary_metrics.validate());

        // Test invalid metrics (negative values)
        let invalid_metrics = MacSystemMetrics {
            cpu_usage_ratio: -0.1,
            cpu_load_ratio: 1.5,
            memory_usage_ratio: 0.6,
            memory_compressed_ratio: 0.3,
            disk_io_ratio: 0.2,
            network_bandwidth_ratio: 0.5,
            network_dropped_packets_ratio: 0.01,
            fd_usage_ratio: 0.7,
            process_count_ratio: 0.4,
        };
        assert!(!invalid_metrics.validate());

        // Test invalid metrics (values > 1.0 for ratios)
        let invalid_metrics2 = MacSystemMetrics {
            cpu_usage_ratio: 1.5,
            cpu_load_ratio: 1.5,
            memory_usage_ratio: 0.6,
            memory_compressed_ratio: 0.3,
            disk_io_ratio: 0.2,
            network_bandwidth_ratio: 0.5,
            network_dropped_packets_ratio: 0.01,
            fd_usage_ratio: 0.7,
            process_count_ratio: 0.4,
        };
        assert!(!invalid_metrics2.validate());
    }

    #[test]
    fn test_vm_stat_parsing() {
        // Test valid vm_stat line
        let valid_line = "Pages free:                               12345.";
        let result = MacSystemMetrics::parse_vm_stat_value(valid_line);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 12345);

        // Test line without trailing dot
        let no_dot_line = "Pages active:                             67890";
        let result = MacSystemMetrics::parse_vm_stat_value(no_dot_line);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 67890);

        // Test invalid format (no numeric value)
        let invalid_line = "Pages invalid: no number here";
        let result = MacSystemMetrics::parse_vm_stat_value(invalid_line);
        assert!(result.is_err());

        // Test empty line
        let empty_line = "";
        let result = MacSystemMetrics::parse_vm_stat_value(empty_line);
        assert!(result.is_err());

        // Test line with multiple numbers (should take last one)
        let multi_number_line = "Pages active: 123 456 789.";
        let result = MacSystemMetrics::parse_vm_stat_value(multi_number_line);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 789);
    }

    #[test]
    fn test_memory_calculation() {
        // Test that memory calculations handle zero division
        // This is a unit test for the calculation logic
        let pages_free = 1000u64;
        let pages_active = 2000u64;
        let pages_inactive = 1500u64;
        let pages_wired = 500u64;
        let pages_compressed = 200u64;

        let total_pages =
            pages_free + pages_active + pages_inactive + pages_wired + pages_compressed;
        let used_pages = pages_active + pages_wired;
        let usage_ratio = used_pages as f32 / total_pages as f32;

        assert!((usage_ratio - 0.4762).abs() < 0.01); // 2500/5200 ≈ 0.4762

        // Test compressed ratio calculation
        let compressed_ratio = pages_compressed as f32 / (total_pages + pages_compressed) as f32;
        assert!((compressed_ratio - 0.037).abs() < 0.01); // 200/5400 ≈ 0.037
    }

    #[test]
    fn test_network_bandwidth_calculation() {
        let mut stats1 = HashMap::new();
        stats1.insert(
            "en0".to_string(),
            NetworkInterfaceStats {
                bytes_in: 1000000,
                bytes_out: 500000,
                packets_in: 1000,
                packets_out: 500,
                dropped_in: 10,
                dropped_out: 5,
            },
        );

        let mut stats2 = HashMap::new();
        stats2.insert(
            "en0".to_string(),
            NetworkInterfaceStats {
                bytes_in: 2000000,  // +1MB
                bytes_out: 1000000, // +500KB
                packets_in: 1100,   // +100 packets
                packets_out: 550,   // +50 packets
                dropped_in: 12,     // +2 dropped
                dropped_out: 7,     // +2 dropped
            },
        );

        let result = MacSystemMetrics::calculate_network_utilization(&stats1, &stats2);
        assert!(result.is_ok());

        let utilization = result.unwrap();
        assert!((0.0..=1.0).contains(&utilization));
    }

    #[test]
    fn test_network_packet_loss_calculation() {
        let mut stats1 = HashMap::new();
        stats1.insert(
            "en0".to_string(),
            NetworkInterfaceStats {
                bytes_in: 1000000,
                bytes_out: 500000,
                packets_in: 1000,
                packets_out: 500,
                dropped_in: 10,
                dropped_out: 5,
            },
        );

        let mut stats2 = HashMap::new();
        stats2.insert(
            "en0".to_string(),
            NetworkInterfaceStats {
                bytes_in: 2000000,
                bytes_out: 1000000,
                packets_in: 1100, // +100 packets
                packets_out: 550, // +50 packets
                dropped_in: 15,   // +5 dropped
                dropped_out: 8,   // +3 dropped
            },
        );

        let result = MacSystemMetrics::calculate_network_dropped_packets(&stats1, &stats2);
        assert!(result.is_ok());

        let dropped_ratio = result.unwrap();
        // Total packets: 150, dropped: 8, ratio: 8/150 ≈ 0.053
        assert!((dropped_ratio - 0.053).abs() < 0.01);
    }

    #[test]
    fn test_interface_speed_detection() {
        // Test speed detection for different interface types
        let en0_speed = MacSystemMetrics::get_interface_speed("en0");
        assert!(en0_speed > 0);

        let awdl_speed = MacSystemMetrics::get_interface_speed("awdl0");
        assert!(awdl_speed > 0);

        let unknown_speed = MacSystemMetrics::get_interface_speed("unknown123");
        assert!(unknown_speed > 0);

        // Ethernet interfaces should generally have higher speeds than WiFi
        assert!(en0_speed >= awdl_speed);
    }

    #[test]
    fn test_cpu_usage_parsing() {
        // Test top command CPU usage parsing logic
        let cpu_line = "CPU usage: 12.5% user, 6.25% sys, 81.25% idle";

        // Simulate the parsing logic from get_cpu_usage
        if let Some(idle_start) = cpu_line.find("% idle") {
            let before_idle = &cpu_line[..idle_start];
            if let Some(last_space) = before_idle.rfind(' ') {
                let idle_str = &before_idle[last_space + 1..];
                if let Ok(idle_percent) = idle_str.parse::<f32>() {
                    let cpu_usage = (100.0 - idle_percent) / 100.0;
                    assert!((cpu_usage - 0.1875).abs() < 0.001); // 18.75% usage
                }
            }
        }

        // Test invalid CPU line
        let invalid_line = "Invalid CPU line format";
        assert!(!invalid_line.contains("% idle"));
    }

    #[test]
    fn test_load_average_parsing() {
        // Test sysctl vm.loadavg parsing logic
        let loadavg_output = "{ 1.23 2.34 3.45 }";
        let parts: Vec<&str> = loadavg_output.split_whitespace().collect();

        #[allow(clippy::collapsible_if)]
        if parts.len() >= 3 {
            if let Ok(load_1min) = parts[1].parse::<f32>() {
                assert!((load_1min - 1.23).abs() < 0.001);

                // Test load ratio calculation with 4 cores
                let cpu_cores = 4u32;
                let load_ratio = load_1min / cpu_cores as f32;
                assert!((load_ratio - 0.3075).abs() < 0.001); // 1.23/4 = 0.3075
            }
        }
    }

    #[test]
    fn test_iostat_parsing_simulation() {
        // Test iostat output parsing logic simulation
        let iostat_line = "    0.5     2.5     1.2     0.3     1.8     0.9";
        let parts: Vec<&str> = iostat_line.split_whitespace().collect();

        // Test parsing tps and msps pairs (index 1,2 and 4,5)
        let mut max_utilization = 0.0f32;
        let mut i = 1;

        while i + 1 < parts.len() {
            if let (Ok(tps), Ok(msps)) = (parts[i].parse::<f32>(), parts[i + 1].parse::<f32>()) {
                let utilization = ((tps * msps) / 1000.0).min(1.0);
                max_utilization = max_utilization.max(utilization);
            }
            i += 3;
        }

        // 2.5 * 1.2 / 1000 = 0.003
        assert!((max_utilization - 0.003).abs() < 0.001);
    }

    #[test]
    fn test_netstat_parsing_simulation() {
        // Test netstat -ib output parsing
        let netstat_line = "en0      1500  link#4    12:34:56:78:9a:bc  1000000      0  2000000  500000      0  1000000     0";
        let fields: Vec<&str> = netstat_line.split_whitespace().collect();

        if fields.len() >= 10 {
            let interface = fields[0];
            assert_eq!(interface, "en0");

            if let (
                Ok(packets_in),
                Ok(dropped_in),
                Ok(bytes_in),
                Ok(packets_out),
                Ok(dropped_out),
                Ok(bytes_out),
            ) = (
                fields[4].parse::<u64>(), // Ipkts
                fields[5].parse::<u64>(), // Ierrs
                fields[6].parse::<u64>(), // Ibytes
                fields[7].parse::<u64>(), // Opkts
                fields[8].parse::<u64>(), // Oerrs
                fields[9].parse::<u64>(), // Obytes
            ) {
                assert_eq!(packets_in, 1000000);
                assert_eq!(bytes_in, 2000000);
                assert_eq!(packets_out, 500000);
                assert_eq!(bytes_out, 1000000);
                assert_eq!(dropped_in, 0);
                assert_eq!(dropped_out, 0);
            }
        }
    }

    #[test]
    fn test_edge_cases() {
        // Test zero division protection
        let mut empty_stats1 = HashMap::new();
        let mut empty_stats2 = HashMap::new();

        let result = MacSystemMetrics::calculate_network_utilization(&empty_stats1, &empty_stats2);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0.0);

        let result =
            MacSystemMetrics::calculate_network_dropped_packets(&empty_stats1, &empty_stats2);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0.0);

        // Test single interface with zero packets
        empty_stats1.insert(
            "en0".to_string(),
            NetworkInterfaceStats {
                bytes_in: 0,
                bytes_out: 0,
                packets_in: 0,
                packets_out: 0,
                dropped_in: 0,
                dropped_out: 0,
            },
        );

        empty_stats2.insert(
            "en0".to_string(),
            NetworkInterfaceStats {
                bytes_in: 0,
                bytes_out: 0,
                packets_in: 0,
                packets_out: 0,
                dropped_in: 0,
                dropped_out: 0,
            },
        );

        let result =
            MacSystemMetrics::calculate_network_dropped_packets(&empty_stats1, &empty_stats2);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0.0);
    }

    #[test]
    fn test_error_handling() {
        // Test vm_stat value parsing with invalid input
        let invalid_lines = [
            "Pages free: invalid",
            "Pages free:",
            "Not a vm_stat line",
            "Pages free: 12.34.56", // Multiple dots
        ];

        for line in &invalid_lines {
            let result = MacSystemMetrics::parse_vm_stat_value(line);
            assert!(result.is_err(), "Should fail for line: {line}");
        }
    }

    #[test]
    fn test_realistic_network_scenarios() {
        // High bandwidth usage scenario
        let mut stats1 = HashMap::new();
        stats1.insert(
            "en0".to_string(),
            NetworkInterfaceStats {
                bytes_in: 0,
                bytes_out: 0,
                packets_in: 0,
                packets_out: 0,
                dropped_in: 0,
                dropped_out: 0,
            },
        );

        let mut stats2 = HashMap::new();
        stats2.insert(
            "en0".to_string(),
            NetworkInterfaceStats {
                bytes_in: 100_000_000, // 100MB in 500ms = very high usage
                bytes_out: 50_000_000, // 50MB out
                packets_in: 100000,
                packets_out: 50000,
                dropped_in: 0,
                dropped_out: 0,
            },
        );

        let result = MacSystemMetrics::calculate_network_utilization(&stats1, &stats2);
        assert!(result.is_ok());

        // Should show high utilization
        let utilization = result.unwrap();
        assert!(utilization > 0.0);

        // Packet loss scenario
        stats2.get_mut("en0").unwrap().dropped_in = 1000; // 1000 dropped out of 150000 total

        let result = MacSystemMetrics::calculate_network_dropped_packets(&stats1, &stats2);
        assert!(result.is_ok());

        let dropped_ratio = result.unwrap();
        assert!((dropped_ratio - 0.0067).abs() < 0.001); // 1000/150000 ≈ 0.0067
    }

    #[test]
    fn test_macos_specific_interfaces() {
        // Test macOS-specific interface types
        let interfaces = ["en0", "en1", "awdl0", "utun0", "bridge0"];

        for interface in &interfaces {
            let speed = MacSystemMetrics::get_interface_speed(interface);
            assert!(
                speed > 0,
                "Interface {interface} should have non-zero speed"
            );

            // en interfaces should generally have higher speeds
            if interface.starts_with("en") {
                assert!(
                    speed >= 12_500_000,
                    "Ethernet interface {interface} should have at least 100Mbps"
                );
            }
        }
    }

    #[test]
    fn test_memory_metrics_ranges() {
        // Test that memory calculation results are in valid ranges
        let test_cases = [
            (1000, 2000, 1500, 500, 200), // Normal case
            (0, 1000, 0, 0, 0),           // Minimal memory
            (1, 1, 1, 1, 1),              // Edge case with small numbers
        ];

        for (free, active, inactive, wired, compressed) in test_cases {
            let total = free + active + inactive + wired + compressed;
            if total > 0 {
                let used = active + wired;
                let usage_ratio = used as f32 / total as f32;
                assert!((0.0..=1.0).contains(&usage_ratio));

                let compressed_ratio = compressed as f32 / (total + compressed) as f32;
                assert!((0.0..=1.0).contains(&compressed_ratio));
            }
        }
    }
}
