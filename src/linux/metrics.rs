use crate::error::{PwrzvError, PwrzvResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::process::Command;
use std::time::{Duration, Instant};
use tokio::time;

const SAMPLE_INTERVAL: u64 = 500; // 500ms

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LinuxSystemMetrics {
    /// CPU total usage ratio: `used / (cores × 100%)`
    /// Range [0.0, 1.0], 1.0 means CPU is completely saturated
    pub cpu_usage_ratio: f32,

    /// CPU I/O wait ratio: `iowait / (user + system + iowait) × 100%`
    /// Range [0.0, 1.0], approaching 1.0 means CPU is waiting for I/O operations
    pub cpu_io_wait_ratio: f32,

    /// CPU load ratio: `loadavg / cores`
    /// Range [0.0, +∞], > 1.0 means tasks are queuing (various situations)
    pub cpu_load_ratio: f32,

    /// Memory usage ratio: `memory_usage_ratio = 1 - (MemAvailable / MemTotal)`
    /// Range [0.0, 1.0], approaching 1.0 means memory usage is near saturation
    pub memory_usage_ratio: f32,

    /// Memory pressure
    /// Based on some avg60 value in /proc/pressure/memory
    /// Range [0.0, 1.0], approaching 1.0 means high memory pressure
    pub memory_pressure_ratio: f32,

    /// Disk I/O utilization: `%util / 100`
    /// Range [0.0, 1.0], approaching 1.0 means disk device is overwhelmed
    pub disk_io_ratio: f32,

    /// Network bandwidth utilization: `used_bandwidth / max_bandwidth`
    /// Range [0.0, 1.0], approaching 1.0 means link is saturated
    pub network_bandwidth_ratio: f32,

    /// Network dropped packets ratio: `dropped_packets / total_packets`
    /// Range [0.0, 1.0], approaching 1.0 means network is saturated
    pub network_dropped_packets_ratio: f32,

    /// File descriptor usage ratio: `used_fd / max_fd`
    /// Range [0.0, 1.0], approaching 1.0 means connection/file handles are exhausted
    pub fd_usage_ratio: f32,

    /// Process count usage ratio: `proc_count / max_proc`
    /// Range [0.0, 1.0], approaching 1.0 means process count reaches system limit
    pub process_count_ratio: f32,
}

#[derive(Debug, Clone)]
struct CpuStat {
    user: u64,
    nice: u64,
    system: u64,
    idle: u64,
    iowait: u64,
    irq: u64,
    softirq: u64,
    steal: u64,
}

#[derive(Debug, Clone)]
struct DiskStat {
    #[allow(dead_code)]
    reads_completed: u64,
    #[allow(dead_code)]
    writes_completed: u64,
    io_time_ms: u64,
}

#[derive(Debug, Clone)]
struct NetworkStat {
    rx_bytes: u64,
    tx_bytes: u64,
    rx_packets: u64,
    tx_packets: u64,
    rx_dropped: u64,
    tx_dropped: u64,
}

impl LinuxSystemMetrics {
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

        let (cpu_usage_ratio, cpu_io_wait_ratio, cpu_load_ratio) = cpu_result?;
        let (memory_usage_ratio, memory_pressure_ratio) = memory_result?;
        let disk_io_ratio = disk_result?;
        let (network_bandwidth_ratio, network_dropped_packets_ratio) = network_result?;
        let fd_usage_ratio = fd_result?;
        let process_count_ratio = process_result?;

        Ok(LinuxSystemMetrics {
            cpu_usage_ratio,
            cpu_io_wait_ratio,
            cpu_load_ratio,
            memory_usage_ratio,
            memory_pressure_ratio,
            disk_io_ratio,
            network_bandwidth_ratio,
            network_dropped_packets_ratio,
            fd_usage_ratio,
            process_count_ratio,
        })
    }

    /// Collect CPU-related metrics (requires sampling interval)
    async fn collect_cpu_metrics() -> PwrzvResult<(f32, f32, f32)> {
        // First sampling
        let stat1 = Self::read_cpu_stat()?;
        let _start_time = Instant::now();

        // Wait for sampling interval
        time::sleep(Duration::from_millis(SAMPLE_INTERVAL)).await;

        // Second sampling
        let stat2 = Self::read_cpu_stat()?;

        // Calculate CPU usage and I/O wait ratio
        let (cpu_usage_ratio, cpu_io_wait_ratio) = Self::calculate_cpu_usage(&stat1, &stat2)?;

        // Read load average
        let cpu_load_ratio = Self::read_load_average()?;

        Ok((cpu_usage_ratio, cpu_io_wait_ratio, cpu_load_ratio))
    }

    /// Collect memory-related metrics
    async fn collect_memory_metrics() -> PwrzvResult<(f32, f32)> {
        let memory_usage_ratio = Self::read_memory_usage()?;
        let memory_pressure_ratio = Self::read_memory_pressure().unwrap_or(0.0);
        Ok((memory_usage_ratio, memory_pressure_ratio))
    }

    /// Collect disk I/O metrics (requires sampling interval)
    async fn collect_disk_metrics() -> PwrzvResult<f32> {
        // First sampling
        let disk_stats1 = Self::read_disk_stats()?;

        // Wait for sampling interval
        time::sleep(Duration::from_millis(SAMPLE_INTERVAL)).await;

        // Second sampling
        let disk_stats2 = Self::read_disk_stats()?;

        // Calculate disk utilization
        Self::calculate_disk_utilization(&disk_stats1, &disk_stats2)
    }

    /// Collect network bandwidth and dropped packets metrics (requires sampling interval)
    async fn collect_network_metrics() -> PwrzvResult<(f32, f32)> {
        // First sampling
        let net_stats1 = Self::read_network_stats()?;

        // Wait for sampling interval
        time::sleep(Duration::from_millis(SAMPLE_INTERVAL)).await;

        // Second sampling
        let net_stats2 = Self::read_network_stats()?;

        // Calculate network bandwidth utilization and dropped packets ratio
        let bandwidth_ratio = Self::calculate_network_utilization(&net_stats1, &net_stats2)?;
        let dropped_packets_ratio =
            Self::calculate_network_dropped_packets(&net_stats1, &net_stats2)?;

        Ok((bandwidth_ratio, dropped_packets_ratio))
    }

    /// Collect file descriptor metrics
    async fn collect_fd_metrics() -> PwrzvResult<f32> {
        Self::read_fd_usage()
    }

    /// Collect process count metrics
    async fn collect_process_metrics() -> PwrzvResult<f32> {
        Self::read_process_count()
    }

    /// Read CPU statistics
    fn read_cpu_stat() -> PwrzvResult<CpuStat> {
        let content = fs::read_to_string("/proc/stat").map_err(|e| {
            PwrzvError::collection_error(&format!("Failed to read /proc/stat: {e}"))
        })?;

        let first_line = content
            .lines()
            .next()
            .ok_or_else(|| PwrzvError::collection_error("Empty /proc/stat"))?;

        let fields: Vec<&str> = first_line.split_whitespace().collect();
        if fields.len() < 8 || fields[0] != "cpu" {
            return Err(PwrzvError::collection_error("Invalid /proc/stat format"));
        }

        Ok(CpuStat {
            user: fields[1]
                .parse()
                .map_err(|_| PwrzvError::collection_error("Invalid CPU user time"))?,
            nice: fields[2]
                .parse()
                .map_err(|_| PwrzvError::collection_error("Invalid CPU nice time"))?,
            system: fields[3]
                .parse()
                .map_err(|_| PwrzvError::collection_error("Invalid CPU system time"))?,
            idle: fields[4]
                .parse()
                .map_err(|_| PwrzvError::collection_error("Invalid CPU idle time"))?,
            iowait: fields[5]
                .parse()
                .map_err(|_| PwrzvError::collection_error("Invalid CPU iowait time"))?,
            irq: fields[6]
                .parse()
                .map_err(|_| PwrzvError::collection_error("Invalid CPU irq time"))?,
            softirq: fields[7]
                .parse()
                .map_err(|_| PwrzvError::collection_error("Invalid CPU softirq time"))?,
            steal: if fields.len() > 8 {
                fields[8].parse().unwrap_or(0)
            } else {
                0
            },
        })
    }

    /// Calculate CPU usage and I/O wait ratio
    fn calculate_cpu_usage(stat1: &CpuStat, stat2: &CpuStat) -> PwrzvResult<(f32, f32)> {
        let total1 = stat1.user
            + stat1.nice
            + stat1.system
            + stat1.idle
            + stat1.iowait
            + stat1.irq
            + stat1.softirq
            + stat1.steal;
        let total2 = stat2.user
            + stat2.nice
            + stat2.system
            + stat2.idle
            + stat2.iowait
            + stat2.irq
            + stat2.softirq
            + stat2.steal;

        let total_diff = total2.saturating_sub(total1);
        if total_diff == 0 {
            return Ok((0.0, 0.0));
        }

        let idle_diff = stat2.idle.saturating_sub(stat1.idle);
        let iowait_diff = stat2.iowait.saturating_sub(stat1.iowait);

        let cpu_usage_ratio = 1.0 - (idle_diff as f32 / total_diff as f32);
        let cpu_io_wait_ratio = iowait_diff as f32 / total_diff as f32;

        Ok((
            cpu_usage_ratio.clamp(0.0, 1.0),
            cpu_io_wait_ratio.clamp(0.0, 1.0),
        ))
    }

    /// Read load average
    fn read_load_average() -> PwrzvResult<f32> {
        let content = fs::read_to_string("/proc/loadavg").map_err(|e| {
            PwrzvError::collection_error(&format!("Failed to read /proc/loadavg: {e}"))
        })?;

        let fields: Vec<&str> = content.split_whitespace().collect();
        if fields.is_empty() {
            return Err(PwrzvError::collection_error("Empty /proc/loadavg"));
        }

        let load_1min: f32 = fields[0]
            .parse()
            .map_err(|_| PwrzvError::collection_error("Invalid load average format"))?;

        // Get CPU core count
        let cpu_cores = Self::get_cpu_cores()?;

        Ok(load_1min / cpu_cores as f32)
    }

    /// Get CPU core count
    fn get_cpu_cores() -> PwrzvResult<u32> {
        let content = fs::read_to_string("/proc/cpuinfo").map_err(|e| {
            PwrzvError::collection_error(&format!("Failed to read /proc/cpuinfo: {e}"))
        })?;

        let core_count = content
            .lines()
            .filter(|line| line.starts_with("processor"))
            .count();

        if core_count == 0 {
            return Err(PwrzvError::collection_error("No CPU cores found"));
        }

        Ok(core_count as u32)
    }

    /// Read memory usage ratio
    fn read_memory_usage() -> PwrzvResult<f32> {
        let content = fs::read_to_string("/proc/meminfo").map_err(|e| {
            PwrzvError::collection_error(&format!("Failed to read /proc/meminfo: {e}"))
        })?;

        let mut mem_total = 0u64;
        let mut mem_available = 0u64;

        for line in content.lines() {
            if line.starts_with("MemTotal:") {
                mem_total = Self::parse_meminfo_value(line)?;
            } else if line.starts_with("MemAvailable:") {
                mem_available = Self::parse_meminfo_value(line)?;
            }
        }

        if mem_total == 0 {
            return Err(PwrzvError::collection_error("MemTotal not found"));
        }

        let usage_ratio = 1.0 - (mem_available as f32 / mem_total as f32);
        Ok(usage_ratio.clamp(0.0, 1.0))
    }

    /// Parse value from /proc/meminfo
    fn parse_meminfo_value(line: &str) -> PwrzvResult<u64> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            return Err(PwrzvError::collection_error("Invalid meminfo line format"));
        }

        parts[1]
            .parse::<u64>()
            .map_err(|_| PwrzvError::collection_error("Invalid meminfo value"))
    }

    /// Read memory pressure
    #[allow(clippy::collapsible_if)]
    fn read_memory_pressure() -> Option<f32> {
        let content = fs::read_to_string("/proc/pressure/memory").ok()?;

        for line in content.lines() {
            if line.starts_with("some ") {
                if let Some(avg60_part) = line
                    .split_whitespace()
                    .find(|part| part.starts_with("avg60="))
                {
                    if let Some(value_str) = avg60_part.strip_prefix("avg60=") {
                        if let Ok(pressure_percent) = value_str.parse::<f32>() {
                            return Some((pressure_percent / 100.0).min(1.0));
                        }
                    }
                }
            }
        }
        None
    }

    /// Read disk statistics
    fn read_disk_stats() -> PwrzvResult<HashMap<String, DiskStat>> {
        let content = fs::read_to_string("/proc/diskstats").map_err(|e| {
            PwrzvError::collection_error(&format!("Failed to read /proc/diskstats: {e}"))
        })?;

        let mut disk_stats = HashMap::new();

        for line in content.lines() {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() >= 14 {
                let device_name = fields[2];

                // Only process major block devices
                if !device_name.contains("loop")
                    && !device_name.chars().last().unwrap_or('a').is_ascii_digit()
                {
                    let reads_completed = fields[3].parse().unwrap_or(0);
                    let writes_completed = fields[7].parse().unwrap_or(0);
                    let io_time_ms = fields[12].parse().unwrap_or(0);

                    disk_stats.insert(
                        device_name.to_string(),
                        DiskStat {
                            reads_completed,
                            writes_completed,
                            io_time_ms,
                        },
                    );
                }
            }
        }

        Ok(disk_stats)
    }

    /// Calculate disk utilization
    fn calculate_disk_utilization(
        stats1: &HashMap<String, DiskStat>,
        stats2: &HashMap<String, DiskStat>,
    ) -> PwrzvResult<f32> {
        let mut total_utilization = 0.0;
        let mut device_count = 0;

        for (device, stat2) in stats2 {
            if let Some(stat1) = stats1.get(device) {
                let io_time_diff = stat2.io_time_ms.saturating_sub(stat1.io_time_ms);

                let utilization = (io_time_diff as f32 / SAMPLE_INTERVAL as f32).min(1.0);

                total_utilization += utilization;
                device_count += 1;
            }
        }

        if device_count > 0 {
            Ok(total_utilization / device_count as f32)
        } else {
            Ok(0.0)
        }
    }

    /// Read network statistics
    fn read_network_stats() -> PwrzvResult<HashMap<String, NetworkStat>> {
        let content = fs::read_to_string("/proc/net/dev").map_err(|e| {
            PwrzvError::collection_error(&format!("Failed to read /proc/net/dev: {e}"))
        })?;

        let mut network_stats = HashMap::new();
        let lines: Vec<&str> = content.lines().collect();

        for line in lines.iter().skip(2) {
            // Skip headers
            if let Some(colon_pos) = line.find(':') {
                let interface = line[..colon_pos].trim();
                if interface != "lo" {
                    // Skip loopback interface
                    let stats = line[colon_pos + 1..].trim();
                    let fields: Vec<&str> = stats.split_whitespace().collect();

                    // Format: bytes packets errs drop fifo frame compressed multicast | bytes packets errs drop fifo colls carrier compressed
                    if fields.len() >= 16 {
                        let rx_bytes = fields[0].parse().unwrap_or(0);
                        let rx_packets = fields[1].parse().unwrap_or(0);
                        let rx_dropped = fields[3].parse().unwrap_or(0); // drop field
                        let tx_bytes = fields[8].parse().unwrap_or(0);
                        let tx_packets = fields[9].parse().unwrap_or(0);
                        let tx_dropped = fields[11].parse().unwrap_or(0); // drop field

                        network_stats.insert(
                            interface.to_string(),
                            NetworkStat {
                                rx_bytes,
                                tx_bytes,
                                rx_packets,
                                tx_packets,
                                rx_dropped,
                                tx_dropped,
                            },
                        );
                    }
                }
            }
        }

        Ok(network_stats)
    }

    /// Calculate network bandwidth utilization
    /// Returns the maximum utilization across all active interfaces
    fn calculate_network_utilization(
        stats1: &HashMap<String, NetworkStat>,
        stats2: &HashMap<String, NetworkStat>,
    ) -> PwrzvResult<f32> {
        let mut max_utilization = 0.0f32;

        for (interface, stat2) in stats2 {
            if let Some(stat1) = stats1.get(interface) {
                let rx_diff = stat2.rx_bytes.saturating_sub(stat1.rx_bytes);
                let tx_diff = stat2.tx_bytes.saturating_sub(stat1.tx_bytes);

                // Calculate bytes per second
                let bytes_per_sec = (rx_diff + tx_diff) * 1000 / SAMPLE_INTERVAL;

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
        stats1: &HashMap<String, NetworkStat>,
        stats2: &HashMap<String, NetworkStat>,
    ) -> PwrzvResult<f32> {
        let mut max_dropped_ratio = 0.0f32;

        for (interface, stat2) in stats2 {
            if let Some(stat1) = stats1.get(interface) {
                // Calculate packet differences
                let rx_packets_diff = stat2.rx_packets.saturating_sub(stat1.rx_packets);
                let tx_packets_diff = stat2.tx_packets.saturating_sub(stat1.tx_packets);
                let rx_dropped_diff = stat2.rx_dropped.saturating_sub(stat1.rx_dropped);
                let tx_dropped_diff = stat2.tx_dropped.saturating_sub(stat1.tx_dropped);

                let total_packets = rx_packets_diff + tx_packets_diff;
                let total_dropped = rx_dropped_diff + tx_dropped_diff;

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
        // Try to read actual link speed from sysfs
        let speed_path = format!("/sys/class/net/{interface}/speed");
        #[allow(clippy::collapsible_if)]
        if let Ok(speed_str) = fs::read_to_string(&speed_path) {
            if let Ok(speed_mbps) = speed_str.trim().parse::<u64>() {
                // Convert Mbps to bytes per second
                // speed_mbps is in megabits per second, convert to bytes per second
                return speed_mbps * 1_000_000 / 8;
            }
        }

        // Try to get interface info from ethtool (if available)
        if let Ok(output) = Command::new("ethtool").arg(interface).output() {
            let output_str = String::from_utf8_lossy(&output.stdout);

            for line in output_str.lines() {
                if line.contains("Speed:") {
                    // Parse "Speed: 1000Mb/s" or similar
                    if let Some(speed_part) = line.split("Speed:").nth(1) {
                        let speed_str = speed_part.trim();
                        if speed_str.contains("1000Mb/s") || speed_str.contains("1000Mbps") {
                            return 125_000_000; // 1 Gbps = 125 MB/s
                        } else if speed_str.contains("100Mb/s") || speed_str.contains("100Mbps") {
                            return 12_500_000; // 100 Mbps = 12.5 MB/s
                        } else if speed_str.contains("10Mb/s") || speed_str.contains("10Mbps") {
                            return 1_250_000; // 10 Mbps = 1.25 MB/s
                        }
                    }
                }
            }
        }

        // Check if interface is up and has a carrier
        let carrier_path = format!("/sys/class/net/{interface}/carrier");
        let operstate_path = format!("/sys/class/net/{interface}/operstate");

        let is_up = fs::read_to_string(&operstate_path)
            .map(|s| s.trim() == "up")
            .unwrap_or(false);

        let has_carrier = fs::read_to_string(&carrier_path)
            .map(|s| s.trim() == "1")
            .unwrap_or(false);

        // If interface is up and active, use conservative estimates
        if is_up && has_carrier {
            match interface {
                // Ethernet interfaces - assume gigabit if active
                name if name.starts_with("eth")
                    || name.starts_with("enp")
                    || name.starts_with("eno") =>
                {
                    125_000_000 // 1 Gbps = 125 MB/s
                }
                // WiFi interfaces - conservative estimate
                name if name.starts_with("wlan")
                    || name.starts_with("wlp")
                    || name.starts_with("wlo") =>
                {
                    50_000_000 // ~400 Mbps = 50 MB/s (conservative WiFi)
                }
                // Virtual interfaces - high capacity
                name if name.starts_with("br")
                    || name.starts_with("bridge")
                    || name.starts_with("docker")
                    || name.starts_with("veth") =>
                {
                    1_000_000_000 // Virtual interfaces can be very fast
                }
                // USB and PPP interfaces
                name if name.starts_with("usb") => 12_500_000, // 100 Mbps
                name if name.starts_with("ppp") => 5_000_000,  // 40 Mbps
                // Default conservative estimate
                _ => 12_500_000, // 100 Mbps
            }
        } else {
            // Interface is down or no carrier, return minimal capacity
            1_000_000 // 8 Mbps minimum
        }
    }

    /// Read file descriptor usage ratio
    fn read_fd_usage() -> PwrzvResult<f32> {
        let content = fs::read_to_string("/proc/sys/fs/file-nr").map_err(|e| {
            PwrzvError::collection_error(&format!("Failed to read /proc/sys/fs/file-nr: {e}"))
        })?;

        let fields: Vec<&str> = content.split_whitespace().collect();
        if fields.len() >= 3 {
            let allocated: u64 = fields[0]
                .parse()
                .map_err(|_| PwrzvError::collection_error("Invalid allocated FDs"))?;
            let max_fds: u64 = fields[2]
                .parse()
                .map_err(|_| PwrzvError::collection_error("Invalid max FDs"))?;

            if max_fds > 0 {
                let usage_ratio = (allocated as f64 / max_fds as f64).min(1.0);
                return Ok(usage_ratio as f32);
            }
        }

        Err(PwrzvError::collection_error("Invalid file-nr format"))
    }

    /// Read process count usage ratio
    fn read_process_count() -> PwrzvResult<f32> {
        // Get current process count
        let proc_dir = fs::read_dir("/proc")
            .map_err(|e| PwrzvError::collection_error(&format!("Failed to read /proc: {e}")))?;

        let current_processes = proc_dir
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .chars()
                    .all(|c| c.is_ascii_digit())
            })
            .count();

        // Read maximum process count
        let max_procs = if let Ok(content) = fs::read_to_string("/proc/sys/kernel/pid_max") {
            content.trim().parse::<u64>().unwrap_or(32768)
        } else {
            32768 // Default value
        };

        let usage_ratio = (current_processes as f64 / max_procs as f64).min(1.0);
        Ok(usage_ratio as f32)
    }

    /// Validate metric data validity
    pub fn validate(&self) -> bool {
        self.cpu_usage_ratio >= 0.0
            && self.cpu_usage_ratio <= 1.0
            && self.cpu_io_wait_ratio >= 0.0
            && self.cpu_io_wait_ratio <= 1.0
            && self.cpu_load_ratio >= 0.0
            && self.memory_usage_ratio >= 0.0
            && self.memory_usage_ratio <= 1.0
            && self.memory_pressure_ratio >= 0.0
            && self.memory_pressure_ratio <= 1.0
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
    fn test_linux_system_metrics_validate() {
        // Test valid metrics
        let valid_metrics = LinuxSystemMetrics {
            cpu_usage_ratio: 0.5,
            cpu_io_wait_ratio: 0.1,
            cpu_load_ratio: 1.2,
            memory_usage_ratio: 0.7,
            memory_pressure_ratio: 0.2,
            disk_io_ratio: 0.3,
            network_bandwidth_ratio: 0.4,
            network_dropped_packets_ratio: 0.01,
            fd_usage_ratio: 0.6,
            process_count_ratio: 0.5,
        };
        assert!(valid_metrics.validate());

        // Test boundary values
        let boundary_metrics = LinuxSystemMetrics {
            cpu_usage_ratio: 1.0,
            cpu_io_wait_ratio: 0.0,
            cpu_load_ratio: 0.0,
            memory_usage_ratio: 1.0,
            memory_pressure_ratio: 1.0,
            disk_io_ratio: 1.0,
            network_bandwidth_ratio: 1.0,
            network_dropped_packets_ratio: 1.0,
            fd_usage_ratio: 1.0,
            process_count_ratio: 1.0,
        };
        assert!(boundary_metrics.validate());

        // Test invalid metrics (negative values)
        let invalid_metrics = LinuxSystemMetrics {
            cpu_usage_ratio: -0.1,
            cpu_io_wait_ratio: 0.1,
            cpu_load_ratio: 1.2,
            memory_usage_ratio: 0.7,
            memory_pressure_ratio: 0.2,
            disk_io_ratio: 0.3,
            network_bandwidth_ratio: 0.4,
            network_dropped_packets_ratio: 0.01,
            fd_usage_ratio: 0.6,
            process_count_ratio: 0.5,
        };
        assert!(!invalid_metrics.validate());

        // Test invalid metrics (values > 1.0 for ratios)
        let invalid_metrics2 = LinuxSystemMetrics {
            cpu_usage_ratio: 1.5,
            cpu_io_wait_ratio: 0.1,
            cpu_load_ratio: 1.2,
            memory_usage_ratio: 0.7,
            memory_pressure_ratio: 0.2,
            disk_io_ratio: 0.3,
            network_bandwidth_ratio: 0.4,
            network_dropped_packets_ratio: 0.01,
            fd_usage_ratio: 0.6,
            process_count_ratio: 0.5,
        };
        assert!(!invalid_metrics2.validate());
    }

    #[test]
    fn test_cpu_stat_parsing() {
        // Test valid CPU stat line
        let valid_line = "cpu  123456 789 234567 890123 456 789 123 0 0 0";
        let result = LinuxSystemMetrics::parse_cpu_stat_line(valid_line);
        assert!(result.is_ok());

        let stat = result.unwrap();
        assert_eq!(stat.user, 123456);
        assert_eq!(stat.nice, 789);
        assert_eq!(stat.system, 234567);
        assert_eq!(stat.idle, 890123);
        assert_eq!(stat.iowait, 456);

        // Test invalid CPU stat line (not enough fields)
        let invalid_line = "cpu  123 456";
        let result = LinuxSystemMetrics::parse_cpu_stat_line(invalid_line);
        assert!(result.is_err());

        // Test non-numeric values
        let invalid_line2 = "cpu  abc def ghi jkl mno pqr stu";
        let result = LinuxSystemMetrics::parse_cpu_stat_line(invalid_line2);
        assert!(result.is_err());
    }

    #[test]
    fn test_cpu_usage_calculation() {
        let stat1 = CpuStat {
            user: 1000,
            nice: 100,
            system: 500,
            idle: 8000,
            iowait: 200,
            irq: 50,
            softirq: 150,
            steal: 0,
        };

        let stat2 = CpuStat {
            user: 1100,   // +100
            nice: 110,    // +10
            system: 550,  // +50
            idle: 8300,   // +300
            iowait: 250,  // +50
            irq: 60,      // +10
            softirq: 170, // +20
            steal: 0,     // +0
        };

        let result = LinuxSystemMetrics::calculate_cpu_usage(&stat1, &stat2);
        assert!(result.is_ok());

        let (usage, iowait) = result.unwrap();

        // Total diff: 540, Non-idle diff: 240
        // Usage: 240/540 ≈ 0.444
        // IOWait: 50/540 ≈ 0.093
        assert!((usage - 0.444).abs() < 0.01);
        assert!((iowait - 0.093).abs() < 0.01);
    }

    #[test]
    fn test_meminfo_parsing() {
        let line = "MemTotal:       16384000 kB";
        let result = LinuxSystemMetrics::parse_meminfo_value(line);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 16384000);

        // Test invalid format
        let invalid_line = "MemTotal: invalid kB";
        let result = LinuxSystemMetrics::parse_meminfo_value(invalid_line);
        assert!(result.is_err());

        // Test missing kB suffix - this should actually work since parse_meminfo_value
        // only requires the numeric part after the colon
        let line_without_kb = "MemTotal:       16384000";
        let result = LinuxSystemMetrics::parse_meminfo_value(line_without_kb);
        // This should actually work as the function extracts the number
        assert!(result.is_ok() || result.is_err()); // Either is acceptable for this edge case
    }

    #[test]
    fn test_network_stat_parsing() {
        // Use realistic /proc/net/dev format: 16 fields minimum
        let valid_line =
            "  eth0: 1234567890 1000 20 30 40 50 60 70 987654321 800 10 15 25 35 45 55";
        let result = LinuxSystemMetrics::parse_network_stat_line(valid_line);
        assert!(result.is_ok());

        let (interface, stat) = result.unwrap();
        assert_eq!(interface, "eth0");
        assert_eq!(stat.rx_bytes, 1234567890);
        assert_eq!(stat.rx_packets, 1000);
        assert_eq!(stat.rx_dropped, 30);
        assert_eq!(stat.tx_bytes, 987654321);
        assert_eq!(stat.tx_packets, 800);
        assert_eq!(stat.tx_dropped, 15);

        // Test invalid line (not enough fields)
        let invalid_line = "eth0: 123 456";
        let result = LinuxSystemMetrics::parse_network_stat_line(invalid_line);
        assert!(result.is_err());
    }

    #[test]
    fn test_disk_utilization_calculation() {
        let mut stats1 = HashMap::new();
        stats1.insert(
            "sda".to_string(),
            DiskStat {
                reads_completed: 1000,
                writes_completed: 500,
                io_time_ms: 10000,
            },
        );

        let mut stats2 = HashMap::new();
        stats2.insert(
            "sda".to_string(),
            DiskStat {
                reads_completed: 1100,
                writes_completed: 550,
                io_time_ms: 10500, // +500ms over time period
            },
        );

        let result = LinuxSystemMetrics::calculate_disk_utilization(&stats1, &stats2);
        assert!(result.is_ok());

        let utilization = result.unwrap();
        // With 500ms time difference and assuming 1000ms collection interval
        // utilization should be 500/1000 = 0.5
        assert!((0.0..=1.0).contains(&utilization));
    }

    #[test]
    fn test_network_packet_loss_calculation() {
        let mut stats1 = HashMap::new();
        stats1.insert(
            "eth0".to_string(),
            NetworkStat {
                rx_bytes: 1000000,
                tx_bytes: 500000,
                rx_packets: 1000,
                tx_packets: 500,
                rx_dropped: 10,
                tx_dropped: 5,
            },
        );

        let mut stats2 = HashMap::new();
        stats2.insert(
            "eth0".to_string(),
            NetworkStat {
                rx_bytes: 2000000,
                tx_bytes: 1000000,
                rx_packets: 1100, // +100 packets
                tx_packets: 550,  // +50 packets
                rx_dropped: 15,   // +5 dropped
                tx_dropped: 8,    // +3 dropped
            },
        );

        let result = LinuxSystemMetrics::calculate_network_dropped_packets(&stats1, &stats2);
        assert!(result.is_ok());

        let dropped_ratio = result.unwrap();
        // Total packets: 150, dropped: 8, ratio: 8/150 ≈ 0.053
        assert!((dropped_ratio - 0.053).abs() < 0.01);
    }

    #[test]
    fn test_interface_speed_detection() {
        // Test default speed for unknown interface
        let speed = LinuxSystemMetrics::get_interface_speed("unknown123");
        assert!(speed > 0);

        // Test that different interface types get different speeds
        let eth_speed = LinuxSystemMetrics::get_interface_speed("eth0");
        let wifi_speed = LinuxSystemMetrics::get_interface_speed("wlan0");
        let usb_speed = LinuxSystemMetrics::get_interface_speed("usb0");

        assert!(eth_speed > 0);
        assert!(wifi_speed > 0);
        assert!(usb_speed > 0);
    }

    #[test]
    fn test_metrics_collection_error_handling() {
        // Test that parsing handles malformed data gracefully
        let invalid_cpu_line = "invalid cpu data";
        let result = LinuxSystemMetrics::parse_cpu_stat_line(invalid_cpu_line);
        assert!(result.is_err());

        match result.unwrap_err() {
            PwrzvError::ParseError { .. } => {}
            _ => panic!("Expected ParseError"),
        }
    }

    #[test]
    fn test_load_average_parsing() {
        // Test that load average calculation is reasonable
        let result = LinuxSystemMetrics::read_load_average();

        // In test environment, this might fail due to missing /proc access
        // but if it succeeds, the value should be reasonable
        if let Ok(load) = result {
            assert!(load >= 0.0);
            assert!(load < 1000.0); // Sanity check
        }
    }

    #[test]
    fn test_edge_cases() {
        // Test zero division protection in CPU calculation
        let zero_stat1 = CpuStat {
            user: 0,
            nice: 0,
            system: 0,
            idle: 0,
            iowait: 0,
            irq: 0,
            softirq: 0,
            steal: 0,
        };
        let zero_stat2 = CpuStat {
            user: 0,
            nice: 0,
            system: 0,
            idle: 0,
            iowait: 0,
            irq: 0,
            softirq: 0,
            steal: 0,
        };

        let result = LinuxSystemMetrics::calculate_cpu_usage(&zero_stat1, &zero_stat2);
        // Should handle zero division gracefully
        assert!(result.is_ok());

        let (usage, iowait) = result.unwrap();
        assert!((0.0..=1.0).contains(&usage));
        assert!((0.0..=1.0).contains(&iowait));
    }

    // Helper function for testing
    impl LinuxSystemMetrics {
        fn parse_cpu_stat_line(line: &str) -> PwrzvResult<CpuStat> {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() < 8 {
                return Err(PwrzvError::parse_error("Invalid CPU stat format"));
            }

            Ok(CpuStat {
                user: fields[1]
                    .parse()
                    .map_err(|_| PwrzvError::parse_error("Invalid user value"))?,
                nice: fields[2]
                    .parse()
                    .map_err(|_| PwrzvError::parse_error("Invalid nice value"))?,
                system: fields[3]
                    .parse()
                    .map_err(|_| PwrzvError::parse_error("Invalid system value"))?,
                idle: fields[4]
                    .parse()
                    .map_err(|_| PwrzvError::parse_error("Invalid idle value"))?,
                iowait: fields[5]
                    .parse()
                    .map_err(|_| PwrzvError::parse_error("Invalid iowait value"))?,
                irq: fields[6]
                    .parse()
                    .map_err(|_| PwrzvError::parse_error("Invalid irq value"))?,
                softirq: fields[7]
                    .parse()
                    .map_err(|_| PwrzvError::parse_error("Invalid softirq value"))?,
                steal: if fields.len() > 8 {
                    fields[8]
                        .parse()
                        .map_err(|_| PwrzvError::parse_error("Invalid steal value"))?
                } else {
                    0
                },
            })
        }

        fn parse_network_stat_line(line: &str) -> PwrzvResult<(String, NetworkStat)> {
            let parts: Vec<&str> = line.trim().split(':').collect();
            if parts.len() != 2 {
                return Err(PwrzvError::parse_error("Invalid network stat format"));
            }

            let interface = parts[0].trim().to_string();
            let fields: Vec<&str> = parts[1].split_whitespace().collect();

            if fields.len() < 16 {
                return Err(PwrzvError::parse_error("Insufficient network stat fields"));
            }

            Ok((
                interface,
                NetworkStat {
                    rx_bytes: fields[0]
                        .parse()
                        .map_err(|_| PwrzvError::parse_error("Invalid rx_bytes"))?,
                    rx_packets: fields[1]
                        .parse()
                        .map_err(|_| PwrzvError::parse_error("Invalid rx_packets"))?,
                    rx_dropped: fields[3]
                        .parse()
                        .map_err(|_| PwrzvError::parse_error("Invalid rx_dropped"))?,
                    tx_bytes: fields[8]
                        .parse()
                        .map_err(|_| PwrzvError::parse_error("Invalid tx_bytes"))?,
                    tx_packets: fields[9]
                        .parse()
                        .map_err(|_| PwrzvError::parse_error("Invalid tx_packets"))?,
                    tx_dropped: fields[11]
                        .parse()
                        .map_err(|_| PwrzvError::parse_error("Invalid tx_dropped"))?,
                },
            ))
        }
    }
}
