use crate::error::{PwrzvError, PwrzvResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command;
use std::time::Duration;
use tokio::time;

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
        let network_bandwidth_ratio = network_result?;
        let fd_usage_ratio = fd_result?;
        let process_count_ratio = process_result?;

        Ok(MacSystemMetrics {
            cpu_usage_ratio,
            cpu_load_ratio,
            memory_usage_ratio,
            memory_compressed_ratio,
            disk_io_ratio,
            network_bandwidth_ratio,
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

    /// Collect network bandwidth metrics
    async fn collect_network_metrics() -> PwrzvResult<f32> {
        Self::get_network_utilization().await
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
        // Use iostat to get disk throughput
        let output = Command::new("iostat")
            .args(["-d", "-c", "2", "-w", "1"])
            .output()
            .map_err(|e| PwrzvError::collection_error(&format!("Failed to run iostat: {e}")))?;

        let output_str = String::from_utf8_lossy(&output.stdout);

        // Parse the last measurement (second line of data)
        let lines: Vec<&str> = output_str.lines().collect();
        let mut max_throughput = 0.0f32;
        let mut found_data = false;

        // Find disk data lines (skip header and first measurement)
        for line in lines.iter().skip(2) {
            let parts: Vec<&str> = line.split_whitespace().collect();

            // Expected format: KB/t  tps  MB/s  KB/t  tps  MB/s (for multiple disks)
            // We want the MB/s values (every 3rd column starting from index 2)
            let mut i = 2;
            while i < parts.len() {
                if let Ok(mb_per_sec) = parts[i].parse::<f32>() {
                    max_throughput = max_throughput.max(mb_per_sec);
                    found_data = true;
                }
                i += 3; // Skip to next MB/s column
            }
        }

        if found_data {
            // Estimate utilization based on throughput
            // Assume typical SSD can handle ~500 MB/s sustained
            let estimated_max_throughput = 500.0; // MB/s
            let utilization = (max_throughput / estimated_max_throughput).min(1.0);
            Ok(utilization)
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
        time::sleep(Duration::from_millis(500)).await;

        // Second sampling
        let stats2 = Self::get_network_stats()?;

        // Calculate bandwidth utilization
        Self::calculate_network_utilization(&stats1, &stats2)
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

                if let (Ok(bytes_in), Ok(bytes_out), Ok(_packets_in), Ok(_packets_out)) = (
                    fields[6].parse::<u64>(),
                    fields[9].parse::<u64>(),
                    fields[4].parse::<u64>(),
                    fields[7].parse::<u64>(),
                ) {
                    stats.insert(
                        interface.to_string(),
                        NetworkInterfaceStats {
                            bytes_in,
                            bytes_out,
                        },
                    );
                }
            }
        }

        Ok(stats)
    }

    /// Calculate network bandwidth utilization
    fn calculate_network_utilization(
        stats1: &HashMap<String, NetworkInterfaceStats>,
        stats2: &HashMap<String, NetworkInterfaceStats>,
    ) -> PwrzvResult<f32> {
        let mut total_bytes_per_sec = 0u64;
        let mut active_interfaces = 0;

        for (interface, stats2) in stats2 {
            if let Some(stats1) = stats1.get(interface) {
                let bytes_in_diff = stats2.bytes_in.saturating_sub(stats1.bytes_in);
                let bytes_out_diff = stats2.bytes_out.saturating_sub(stats1.bytes_out);

                // Calculate bytes per second (sampling interval 500ms)
                let bytes_per_sec = (bytes_in_diff + bytes_out_diff) * 2;
                total_bytes_per_sec += bytes_per_sec;
                active_interfaces += 1;
            }
        }

        if active_interfaces == 0 {
            return Ok(0.0);
        }

        // Assume gigabit network (1 Gbps = 125 MB/s)
        let gigabit_bytes_per_sec = 125_000_000u64;
        let utilization = (total_bytes_per_sec as f64 / gigabit_bytes_per_sec as f64).min(1.0);

        Ok(utilization as f32)
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
            && self.fd_usage_ratio >= 0.0
            && self.fd_usage_ratio <= 1.0
            && self.process_count_ratio >= 0.0
            && self.process_count_ratio <= 1.0
    }
}
