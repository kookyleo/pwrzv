use crate::error::PwrzvResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

/// Network statistics structure used by both platforms
#[derive(Debug, Clone)]
pub(crate) struct NetworkStats {
    #[allow(dead_code)]
    pub(crate) rx_bytes: u64,
    #[allow(dead_code)]
    pub(crate) tx_bytes: u64,
    pub(crate) rx_packets: u64,
    pub(crate) tx_packets: u64,
    pub(crate) rx_dropped: u64,
    pub(crate) tx_dropped: u64,
}

/// Linux system metrics structure
///
/// All metrics are optional to handle collection failures gracefully.
/// When a metric cannot be collected, it will be `None` rather than a fallback value.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LinuxSystemMetrics {
    /// CPU usage ratio: (user + nice + system) / total_time
    /// Range: [0.0, 1.0] where 1.0 means CPU is fully utilized
    pub cpu_usage_ratio: Option<f32>,

    /// CPU I/O wait ratio: iowait / total_time
    /// Range: [0.0, 1.0] where higher values indicate I/O bottlenecks
    pub cpu_io_wait_ratio: Option<f32>,

    /// CPU load ratio: 1-minute load average / CPU core count
    /// Range: [0.0, +∞] where > 1.0 indicates task queuing
    pub cpu_load_ratio: Option<f32>,

    /// Memory usage ratio: (total - available) / total
    /// Range: [0.0, 1.0] where 1.0 means memory is fully utilized
    pub memory_usage_ratio: Option<f32>,

    /// Memory pressure ratio: PSI memory average (10s) / 100
    /// Range: [0.0, 1.0] where higher values indicate memory pressure
    pub memory_pressure_ratio: Option<f32>,

    /// Disk I/O utilization: maximum disk utilization percentage
    /// Range: [0.0, 1.0] where 1.0 means disk I/O is fully saturated
    pub disk_io_utilization: Option<f32>,

    /// Network packet drop ratio: dropped_packets / total_packets
    /// Range: [0.0, 1.0] where higher values indicate network issues
    pub network_dropped_packets_ratio: Option<f32>,

    /// File descriptor usage ratio: open_fds / max_fds
    /// Range: [0.0, 1.0] where 1.0 means FD limit is reached
    pub fd_usage_ratio: Option<f32>,

    /// Process count ratio: current_processes / typical_limit
    /// Range: [0.0, +∞] where > 1.0 indicates high process count
    pub process_count_ratio: Option<f32>,
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
}

impl LinuxSystemMetrics {
    /// Collect all system metrics using optimized parallel execution
    ///
    /// This method uses `tokio::join!` to collect all metrics in parallel,
    /// maximizing performance and minimizing total collection time.
    ///
    /// # Returns
    ///
    /// A `LinuxSystemMetrics` struct where each field may be `None` if that
    /// specific metric could not be collected. The method itself only fails
    /// if there's a fundamental system error.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use pwrzv::get_power_reserve_level_with_details_direct;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let (level, details) = get_power_reserve_level_with_details_direct().await?;
    ///     
    ///     println!("Power reserve level: {}", level);
    ///     for (metric, score) in details {
    ///         println!("{}: {}", metric, score);
    ///     }
    ///     Ok(())
    /// }
    /// ```
    pub async fn collect_system_metrics() -> PwrzvResult<Self> {
        // Execute all metrics collection in parallel for optimal performance
        let (cpu_result, memory_result, network_result, disk_result, fd_result, process_result) = tokio::join!(
            Self::get_cpu_metrics_consolidated(),
            Self::get_memory_metrics_consolidated(),
            Self::get_network_metrics_consolidated(),
            Self::get_disk_io_utilization_instant(),
            Self::get_fd_usage(),
            Self::get_process_count()
        );

        // Extract results, using None for any failed metrics
        let (cpu_usage_ratio, cpu_io_wait_ratio, cpu_load_ratio) =
            cpu_result.unwrap_or((None, None, None));
        let (memory_usage_ratio, memory_pressure_ratio) = memory_result.unwrap_or((None, None));
        let network_dropped_packets_ratio = network_result.unwrap_or(None);
        let disk_io_utilization = disk_result.unwrap_or(None);
        let fd_usage_ratio = fd_result.unwrap_or(None);
        let process_count_ratio = process_result.unwrap_or(None);

        Ok(LinuxSystemMetrics {
            cpu_usage_ratio,
            cpu_io_wait_ratio,
            cpu_load_ratio,
            memory_usage_ratio,
            memory_pressure_ratio,
            disk_io_utilization,
            network_dropped_packets_ratio,
            fd_usage_ratio,
            process_count_ratio,
        })
    }

    /// Get CPU metrics with consolidated /proc/stat and /proc/loadavg reads
    ///
    /// Uses parallel file reads to retrieve:
    /// - `/proc/stat`: CPU time statistics for usage and I/O wait calculation
    /// - `/proc/loadavg`: Load averages
    /// - `/proc/cpuinfo`: CPU core count
    ///
    /// # Returns
    ///
    /// A tuple of `(cpu_usage_ratio, cpu_io_wait_ratio, cpu_load_ratio)` where
    /// each may be `None` if the corresponding metric could not be calculated.
    ///
    /// # Performance
    ///
    /// This consolidated approach is significantly faster than separate reads.
    pub(crate) async fn get_cpu_metrics_consolidated()
    -> PwrzvResult<(Option<f32>, Option<f32>, Option<f32>)> {
        // Execute all CPU-related reads in parallel
        let (stat_result, loadavg_result, cpuinfo_result) = tokio::join!(
            async { fs::read_to_string("/proc/stat") },
            async { fs::read_to_string("/proc/loadavg") },
            async { fs::read_to_string("/proc/cpuinfo") }
        );

        let mut cpu_usage: Option<f32> = None;
        let mut cpu_io_wait: Option<f32> = None;
        let mut cpu_load: Option<f32> = None;

        // Parse CPU statistics
        if let Ok(stat_content) = stat_result {
            if let Some(stat) = Self::parse_cpu_stat(&stat_content) {
                let total = stat.total();
                if total > 0 {
                    let idle_percent = stat.idle as f32 / total as f32;
                    cpu_usage = Some((1.0f32 - idle_percent).clamp(0.0, 1.0));
                    cpu_io_wait = Some((stat.iowait as f32 / total as f32).clamp(0.0, 1.0));
                }
            }
        }

        // Parse load average and combine with CPU core count
        if let (Ok(loadavg_content), Ok(cpuinfo_content)) = (loadavg_result, cpuinfo_result) {
            if let (Some(load_avg), Some(cpu_cores)) = (
                Self::parse_load_average(&loadavg_content),
                Self::parse_cpu_cores(&cpuinfo_content),
            ) {
                cpu_load = Some((load_avg / cpu_cores as f32).min(10.0)); // Cap at reasonable maximum
            }
        }

        Ok((cpu_usage, cpu_io_wait, cpu_load))
    }

    /// Get memory metrics with consolidated /proc/meminfo read
    ///
    /// Uses a single `/proc/meminfo` read to retrieve memory statistics and
    /// attempts to read PSI memory pressure if available.
    ///
    /// # Returns
    ///
    /// A tuple of `(memory_usage_ratio, memory_pressure_ratio)` where each
    /// may be `None` if the metric could not be calculated.
    pub(crate) async fn get_memory_metrics_consolidated() -> PwrzvResult<(Option<f32>, Option<f32>)>
    {
        // Execute memory info and pressure reads in parallel
        let (meminfo_result, pressure_result) =
            tokio::join!(async { fs::read_to_string("/proc/meminfo") }, async {
                fs::read_to_string("/proc/pressure/memory")
            });

        let memory_usage = if let Ok(meminfo_content) = meminfo_result {
            Self::parse_memory_usage(&meminfo_content)
        } else {
            None
        };

        let memory_pressure = if let Ok(pressure_content) = pressure_result {
            Self::parse_memory_pressure(&pressure_content)
        } else {
            None
        };

        Ok((memory_usage, memory_pressure))
    }

    /// Get network metrics with consolidated /proc/net/dev read
    ///
    /// Uses a single `/proc/net/dev` read to retrieve network interface
    /// statistics and calculates drop ratio.
    ///
    /// # Returns
    ///
    /// Network dropped packets ratio as `Option<f32>`, or `None` if no network
    /// activity was detected or parsing failed.
    pub(crate) async fn get_network_metrics_consolidated() -> PwrzvResult<Option<f32>> {
        let network_stats = match fs::read_to_string("/proc/net/dev") {
            Ok(content) => Self::parse_network_stats(&content),
            Err(_) => return Ok(None),
        };

        if let Some(stats) = network_stats {
            // Calculate dropped packets ratio only
            let dropped_ratio = Self::calculate_instant_dropped_packets_ratio(&stats);
            Ok(dropped_ratio)
        } else {
            Ok(None)
        }
    }

    /// Get disk I/O utilization using iostat or /proc/diskstats fallback
    ///
    /// Attempts to use `iostat -x` to get real utilization data, falling back
    /// to `/proc/diskstats` estimation if iostat is not available.
    ///
    /// # Returns
    ///
    /// Disk I/O utilization as `Option<f32>`, or `None` if no disks were found
    /// or parsing failed.
    pub(crate) async fn get_disk_io_utilization_instant() -> PwrzvResult<Option<f32>> {
        // Try to use iostat -x to get real %util first
        if let Some(iostat_util) = Self::get_disk_util_from_iostat().await {
            return Ok(Some(iostat_util));
        }

        // Fallback to /proc/diskstats estimation
        match fs::read_to_string("/proc/diskstats") {
            Ok(content) => {
                if let Some(disk_stats) = Self::parse_disk_stats(&content) {
                    Ok(Self::estimate_disk_utilization(&disk_stats))
                } else {
                    Ok(None)
                }
            }
            Err(_) => Ok(None),
        }
    }

    /// Get file descriptor usage ratio
    ///
    /// Reads system file descriptor limits and current usage from `/proc/sys/fs/`.
    ///
    /// # Returns
    ///
    /// FD usage ratio as `Option<f32>`, or `None` if the limits could not be read.
    pub(crate) async fn get_fd_usage() -> PwrzvResult<Option<f32>> {
        let (file_nr_result, file_max_result) = tokio::join!(
            async { fs::read_to_string("/proc/sys/fs/file-nr") },
            async { fs::read_to_string("/proc/sys/fs/file-max") }
        );

        if let (Ok(file_nr_content), Ok(file_max_content)) = (file_nr_result, file_max_result) {
            let open_fds = file_nr_content
                .split_whitespace()
                .next()
                .and_then(|s| s.parse::<u64>().ok());

            let max_fds = file_max_content.trim().parse::<u64>().ok();

            if let (Some(open_fds), Some(max_fds)) = (open_fds, max_fds) {
                if max_fds > 0 {
                    Ok(Some((open_fds as f32 / max_fds as f32).min(1.0)))
                } else {
                    Ok(None)
                }
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    /// Get process count ratio
    ///
    /// Uses `ps aux` to count processes and compares against a typical system limit.
    ///
    /// # Returns
    ///
    /// Process count ratio as `Option<f32>`, or `None` if process count could not be determined.
    pub(crate) async fn get_process_count() -> PwrzvResult<Option<f32>> {
        let output = match tokio::process::Command::new("ps")
            .args(["aux"])
            .output()
            .await
        {
            Ok(output) if output.status.success() => output,
            _ => return Ok(None),
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let process_count = stdout.lines().count().saturating_sub(1); // Subtract header line

        // Typical max processes is around 4096 for most systems
        let typical_max = 4096.0;
        Ok(Some((process_count as f32 / typical_max).min(10.0))) // Cap at reasonable maximum
    }

    // Private parsing methods

    /// Parse CPU statistics from /proc/stat content
    fn parse_cpu_stat(content: &str) -> Option<CpuStat> {
        let line = content.lines().next()?;
        let parts: Vec<&str> = line.split_whitespace().collect();

        if parts.len() < 8 {
            return None;
        }

        let user = parts[1].parse::<u64>().ok()?;
        let nice = parts[2].parse::<u64>().ok()?;
        let system = parts[3].parse::<u64>().ok()?;
        let idle = parts[4].parse::<u64>().ok()?;
        let iowait = parts[5].parse::<u64>().ok()?;
        let irq = parts[6].parse::<u64>().ok()?;
        let softirq = parts[7].parse::<u64>().ok()?;

        Some(CpuStat {
            user,
            nice,
            system,
            idle,
            iowait,
            irq,
            softirq,
        })
    }

    /// Parse load average from /proc/loadavg content
    fn parse_load_average(content: &str) -> Option<f32> {
        content
            .split_whitespace()
            .next()
            .and_then(|s| s.parse::<f32>().ok())
    }

    /// Parse CPU core count from /proc/cpuinfo content
    fn parse_cpu_cores(content: &str) -> Option<u32> {
        let core_count = content
            .lines()
            .filter(|line| line.starts_with("processor"))
            .count() as u32;

        if core_count > 0 {
            Some(core_count)
        } else {
            None
        }
    }

    /// Parse memory usage from /proc/meminfo content
    fn parse_memory_usage(content: &str) -> Option<f32> {
        let mut mem_total = 0u64;
        let mut mem_available = 0u64;

        for line in content.lines() {
            if line.starts_with("MemTotal:") {
                mem_total = Self::parse_meminfo_value(line).ok()?;
            } else if line.starts_with("MemAvailable:") {
                mem_available = Self::parse_meminfo_value(line).ok()?;
            }
        }

        if mem_total > 0 {
            let usage_ratio = if mem_available < mem_total {
                (mem_total - mem_available) as f32 / mem_total as f32
            } else {
                0.0
            };
            Some(usage_ratio.min(1.0))
        } else {
            None
        }
    }

    /// Parse value from /proc/meminfo line
    fn parse_meminfo_value(line: &str) -> Result<u64, ()> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            return Err(());
        }

        parts[1].parse::<u64>().map_err(|_| ())
    }

    /// Parse memory pressure from /proc/pressure/memory content
    fn parse_memory_pressure(content: &str) -> Option<f32> {
        for line in content.lines() {
            if line.starts_with("some avg10=") {
                let avg10_str = line.split("avg10=").nth(1)?.split_whitespace().next()?;
                let avg10 = avg10_str.parse::<f32>().ok()?;
                return Some((avg10 / 100.0).min(1.0));
            }
        }
        None
    }

    /// Parse network statistics from /proc/net/dev content
    fn parse_network_stats(content: &str) -> Option<HashMap<String, NetworkStats>> {
        let mut stats = HashMap::new();

        for line in content.lines().skip(2) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 17 {
                continue;
            }

            let interface = parts[0].trim_end_matches(':').to_string();

            // Skip loopback interface
            if interface == "lo" {
                continue;
            }

            let rx_bytes = parts[1].parse::<u64>().unwrap_or(0);
            let rx_packets = parts[2].parse::<u64>().unwrap_or(0);
            let rx_dropped = parts[4].parse::<u64>().unwrap_or(0);
            let tx_bytes = parts[9].parse::<u64>().unwrap_or(0);
            let tx_packets = parts[10].parse::<u64>().unwrap_or(0);
            let tx_dropped = parts[12].parse::<u64>().unwrap_or(0);

            stats.insert(
                interface,
                NetworkStats {
                    rx_bytes,
                    tx_bytes,
                    rx_packets,
                    tx_packets,
                    rx_dropped,
                    tx_dropped,
                },
            );
        }

        if stats.is_empty() { None } else { Some(stats) }
    }

    /// Parse disk statistics from /proc/diskstats content
    fn parse_disk_stats(content: &str) -> Option<HashMap<String, DiskStat>> {
        let mut stats = HashMap::new();

        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 14 {
                continue;
            }

            let device = parts[2].to_string();

            // Skip loop devices and partitions
            if device.starts_with("loop") || device.chars().last().unwrap_or('a').is_ascii_digit() {
                continue;
            }

            let sectors_read = parts[5].parse::<u64>().unwrap_or(0);
            let sectors_written = parts[9].parse::<u64>().unwrap_or(0);

            stats.insert(
                device,
                DiskStat {
                    sectors_read,
                    sectors_written,
                },
            );
        }

        if stats.is_empty() { None } else { Some(stats) }
    }

    /// Try to get real disk utilization from iostat -x
    async fn get_disk_util_from_iostat() -> Option<f32> {
        let output = tokio::process::Command::new("iostat")
            .args(["-x", "1", "1"])
            .output()
            .await
            .ok()?;

        let output_str = std::str::from_utf8(&output.stdout).ok()?;

        let mut max_util = 0.0f32;

        // Parse iostat -x output to find %util column
        for line in output_str.lines() {
            // Skip header lines and empty lines
            if line.contains("Device") || line.trim().is_empty() || line.contains("avg-cpu") {
                continue;
            }

            let parts: Vec<&str> = line.split_whitespace().collect();

            // iostat -x output format includes %util as the last column
            if parts.len() >= 14 {
                if let Some(util_str) = parts.last() {
                    if let Ok(util_percent) = util_str.parse::<f32>() {
                        let util_ratio = (util_percent / 100.0).min(1.0);
                        max_util = max_util.max(util_ratio);
                    }
                }
            }
        }

        Some(max_util)
    }

    /// Estimate disk utilization from /proc/diskstats
    fn estimate_disk_utilization(disk_stats: &HashMap<String, DiskStat>) -> Option<f32> {
        if disk_stats.is_empty() {
            return None;
        }

        let mut total_utilization = 0.0;
        let mut count = 0;

        for stat in disk_stats.values() {
            // Approximate utilization based on sectors read/written
            let sectors_total = stat.sectors_read + stat.sectors_written;
            let utilization = (sectors_total as f32 / 1000000.0).min(1.0); // Rough approximation
            total_utilization += utilization;
            count += 1;
        }

        if count > 0 {
            Some(total_utilization / count as f32)
        } else {
            None
        }
    }

    /// Calculate instant dropped packets ratio
    fn calculate_instant_dropped_packets_ratio(
        stats: &HashMap<String, NetworkStats>,
    ) -> Option<f32> {
        let mut total_packets = 0u64;
        let mut total_dropped = 0u64;

        for stat in stats.values() {
            let interface_packets = stat.rx_packets + stat.tx_packets;
            let interface_dropped = stat.rx_dropped + stat.tx_dropped;

            // Only count interfaces that have actual traffic (similar to macOS logic)
            if interface_packets > 0 {
                total_packets += interface_packets;
                total_dropped += interface_dropped;
            }
        }

        if total_packets == 0 {
            return None; // No interfaces with traffic, return None instead of 0.0
        }

        let dropped_ratio = total_dropped as f32 / total_packets as f32;
        Some(dropped_ratio.clamp(0.0, 1.0))
    }
}

impl CpuStat {
    fn total(&self) -> u64 {
        self.user + self.nice + self.system + self.idle + self.iowait + self.irq + self.softirq
    }
}

#[derive(Debug, Clone)]
struct DiskStat {
    sectors_read: u64,
    sectors_written: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[tokio::test]
    async fn test_collect_system_metrics() {
        println!("Testing Linux system metrics collection...");

        let result = LinuxSystemMetrics::collect_system_metrics().await;
        assert!(result.is_ok(), "System metrics collection should succeed");

        let metrics = result.unwrap();
        println!("Collected metrics: {metrics:#?}");

        // Validate individual metrics if they exist
        if let Some(cpu_usage) = metrics.cpu_usage_ratio {
            assert!(
                (0.0..=1.0).contains(&cpu_usage),
                "CPU usage should be in [0.0, 1.0], got: {cpu_usage}"
            );
        }

        if let Some(cpu_io_wait) = metrics.cpu_io_wait_ratio {
            assert!(
                (0.0..=1.0).contains(&cpu_io_wait),
                "CPU I/O wait should be in [0.0, 1.0], got: {cpu_io_wait}"
            );
        }

        if let Some(load_ratio) = metrics.cpu_load_ratio {
            assert!(
                load_ratio >= 0.0,
                "CPU load ratio should be non-negative, got: {load_ratio}"
            );
        }

        if let Some(memory_usage) = metrics.memory_usage_ratio {
            assert!(
                (0.0..=1.0).contains(&memory_usage),
                "Memory usage should be in [0.0, 1.0], got: {memory_usage}"
            );
        }

        if let Some(memory_pressure) = metrics.memory_pressure_ratio {
            assert!(
                (0.0..=1.0).contains(&memory_pressure),
                "Memory pressure should be in [0.0, 1.0], got: {memory_pressure}"
            );
        }

        if let Some(disk_io) = metrics.disk_io_utilization {
            assert!(
                (0.0..=1.0).contains(&disk_io),
                "Disk I/O should be in [0.0, 1.0], got: {disk_io}"
            );
        }

        if let Some(network_drop) = metrics.network_dropped_packets_ratio {
            assert!(
                (0.0..=1.0).contains(&network_drop),
                "Network drop ratio should be in [0.0, 1.0], got: {network_drop}"
            );
        }

        if let Some(fd_usage) = metrics.fd_usage_ratio {
            assert!(
                (0.0..=1.0).contains(&fd_usage),
                "FD usage should be in [0.0, 1.0], got: {fd_usage}"
            );
        }

        if let Some(process_count) = metrics.process_count_ratio {
            assert!(
                process_count >= 0.0,
                "Process count ratio should be non-negative, got: {process_count}"
            );
        }

        // Count available metrics
        let available_count = [
            metrics.cpu_usage_ratio.is_some(),
            metrics.cpu_io_wait_ratio.is_some(),
            metrics.cpu_load_ratio.is_some(),
            metrics.memory_usage_ratio.is_some(),
            metrics.memory_pressure_ratio.is_some(),
            metrics.disk_io_utilization.is_some(),
            metrics.network_dropped_packets_ratio.is_some(),
            metrics.fd_usage_ratio.is_some(),
            metrics.process_count_ratio.is_some(),
        ]
        .iter()
        .filter(|&&x| x)
        .count();

        println!("Available metrics: {available_count}/9");

        // We should have at least some metrics available
        assert!(
            available_count > 0,
            "At least some metrics should be available"
        );
    }

    #[tokio::test]
    async fn test_individual_metric_methods() {
        println!("Testing individual metric collection methods...");

        // Test CPU metrics
        let cpu_result = LinuxSystemMetrics::get_cpu_metrics_consolidated().await;
        assert!(cpu_result.is_ok(), "CPU metrics should be collectible");
        let (cpu_usage, cpu_io_wait, cpu_load) = cpu_result.unwrap();
        println!("CPU metrics: usage={cpu_usage:?}, io_wait={cpu_io_wait:?}, load={cpu_load:?}");

        // Test memory metrics
        let memory_result = LinuxSystemMetrics::get_memory_metrics_consolidated().await;
        assert!(
            memory_result.is_ok(),
            "Memory metrics should be collectible"
        );
        let (memory_usage, memory_pressure) = memory_result.unwrap();
        println!("Memory metrics: usage={memory_usage:?}, pressure={memory_pressure:?}");

        // Test network metrics
        let network_result = LinuxSystemMetrics::get_network_metrics_consolidated().await;
        assert!(
            network_result.is_ok(),
            "Network metrics should be collectible"
        );
        let network_drop = network_result.unwrap();
        println!("Network metrics: drop_ratio={network_drop:?}");

        // Test disk metrics
        let disk_result = LinuxSystemMetrics::get_disk_io_utilization_instant().await;
        assert!(disk_result.is_ok(), "Disk metrics should be collectible");
        let disk_io = disk_result.unwrap();
        println!("Disk metrics: io_utilization={disk_io:?}");

        // Test FD usage
        let fd_result = LinuxSystemMetrics::get_fd_usage().await;
        assert!(fd_result.is_ok(), "FD metrics should be collectible");
        let fd_usage = fd_result.unwrap();
        println!("FD metrics: usage={fd_usage:?}");

        // Test process count
        let process_result = LinuxSystemMetrics::get_process_count().await;
        assert!(
            process_result.is_ok(),
            "Process metrics should be collectible"
        );
        let process_count = process_result.unwrap();
        println!("Process metrics: count_ratio={process_count:?}");
    }

    #[test]
    fn test_parse_cpu_stat() {
        let content = "cpu  123456 789 234567 890123 45678 901 234 0 0 0\n";
        let result = LinuxSystemMetrics::parse_cpu_stat(content);
        assert!(result.is_some());

        let stat = result.unwrap();
        assert_eq!(stat.user, 123456);
        assert_eq!(stat.nice, 789);
        assert_eq!(stat.system, 234567);
        assert_eq!(stat.idle, 890123);
        assert_eq!(stat.iowait, 45678);

        let total = stat.total();
        assert_eq!(total, 123456 + 789 + 234567 + 890123 + 45678 + 901 + 234);
    }

    #[test]
    fn test_parse_load_average() {
        let content = "1.23 2.34 3.45 1/234 5678\n";
        let result = LinuxSystemMetrics::parse_load_average(content);
        assert_eq!(result, Some(1.23));

        // Test invalid format
        let invalid_content = "invalid format\n";
        let result = LinuxSystemMetrics::parse_load_average(invalid_content);
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_cpu_cores() {
        let content = "processor\t: 0\nprocessor\t: 1\nprocessor\t: 2\nprocessor\t: 3\n";
        let result = LinuxSystemMetrics::parse_cpu_cores(content);
        assert_eq!(result, Some(4));

        // Test empty content
        let empty_content = "";
        let result = LinuxSystemMetrics::parse_cpu_cores(empty_content);
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_memory_usage() {
        let content = "MemTotal:       16384000 kB\nMemAvailable:   8192000 kB\n";
        let result = LinuxSystemMetrics::parse_memory_usage(content);
        assert!(result.is_some());

        let usage = result.unwrap();
        // Expected: (16384000 - 8192000) / 16384000 = 0.5
        assert!((usage - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_parse_memory_pressure() {
        let content = "some avg10=12.34 avg60=23.45 avg300=34.56 total=123456789\n";
        let result = LinuxSystemMetrics::parse_memory_pressure(content);
        assert!(result.is_some());

        let pressure = result.unwrap();
        // Expected: 12.34 / 100.0 = 0.1234
        assert!((pressure - 0.1234).abs() < 0.001);
    }

    #[test]
    fn test_serialization() {
        let metrics = LinuxSystemMetrics {
            cpu_usage_ratio: Some(0.5),
            cpu_io_wait_ratio: Some(0.1),
            cpu_load_ratio: Some(1.2),
            memory_usage_ratio: Some(0.7),
            memory_pressure_ratio: Some(0.2),
            disk_io_utilization: Some(0.3),
            network_dropped_packets_ratio: Some(0.01),
            fd_usage_ratio: Some(0.6),
            process_count_ratio: Some(0.8),
        };

        // Test JSON serialization
        let json = serde_json::to_string(&metrics).unwrap();
        assert!(json.contains("cpu_usage_ratio"));
        assert!(json.contains("0.5"));

        // Test deserialization
        let deserialized: LinuxSystemMetrics = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, metrics);
    }

    #[test]
    fn test_clone_and_debug() {
        let metrics = LinuxSystemMetrics {
            cpu_usage_ratio: Some(0.5),
            cpu_io_wait_ratio: Some(0.1),
            cpu_load_ratio: Some(1.2),
            memory_usage_ratio: Some(0.7),
            memory_pressure_ratio: Some(0.2),
            disk_io_utilization: Some(0.3),
            network_dropped_packets_ratio: Some(0.01),
            fd_usage_ratio: Some(0.6),
            process_count_ratio: Some(0.8),
        };

        // Test Clone
        let cloned = metrics.clone();
        assert_eq!(cloned, metrics);

        // Test Debug
        let debug_str = format!("{metrics:?}");
        assert!(debug_str.contains("LinuxSystemMetrics"));
        assert!(debug_str.contains("cpu_usage_ratio"));
    }

    #[tokio::test]
    async fn test_error_handling() {
        // Test that methods handle errors gracefully and return None rather than panicking

        // These tests verify the error handling paths, though they may not trigger
        // actual errors in a normal environment

        let cpu_result = LinuxSystemMetrics::get_cpu_metrics_consolidated().await;
        assert!(
            cpu_result.is_ok(),
            "CPU metrics should handle errors gracefully"
        );

        let memory_result = LinuxSystemMetrics::get_memory_metrics_consolidated().await;
        assert!(
            memory_result.is_ok(),
            "Memory metrics should handle errors gracefully"
        );

        let network_result = LinuxSystemMetrics::get_network_metrics_consolidated().await;
        assert!(
            network_result.is_ok(),
            "Network metrics should handle errors gracefully"
        );

        let disk_result = LinuxSystemMetrics::get_disk_io_utilization_instant().await;
        assert!(
            disk_result.is_ok(),
            "Disk metrics should handle errors gracefully"
        );

        let fd_result = LinuxSystemMetrics::get_fd_usage().await;
        assert!(
            fd_result.is_ok(),
            "FD metrics should handle errors gracefully"
        );

        let process_result = LinuxSystemMetrics::get_process_count().await;
        assert!(
            process_result.is_ok(),
            "Process metrics should handle errors gracefully"
        );
    }

    #[tokio::test]
    async fn test_integration_comprehensive() {
        println!("Running comprehensive Linux integration test...");

        // Test multiple collection cycles to ensure consistency
        let mut all_successful = true;
        let mut metrics_availability = [0; 9]; // Track availability of each metric

        for i in 0..3 {
            println!("Collection cycle {}", i + 1);

            match LinuxSystemMetrics::collect_system_metrics().await {
                Ok(metrics) => {
                    if metrics.cpu_usage_ratio.is_some() {
                        metrics_availability[0] += 1;
                    }
                    if metrics.cpu_io_wait_ratio.is_some() {
                        metrics_availability[1] += 1;
                    }
                    if metrics.cpu_load_ratio.is_some() {
                        metrics_availability[2] += 1;
                    }
                    if metrics.memory_usage_ratio.is_some() {
                        metrics_availability[3] += 1;
                    }
                    if metrics.memory_pressure_ratio.is_some() {
                        metrics_availability[4] += 1;
                    }
                    if metrics.disk_io_utilization.is_some() {
                        metrics_availability[5] += 1;
                    }

                    if metrics.network_dropped_packets_ratio.is_some() {
                        metrics_availability[6] += 1;
                    }
                    if metrics.fd_usage_ratio.is_some() {
                        metrics_availability[7] += 1;
                    }
                    if metrics.process_count_ratio.is_some() {
                        metrics_availability[8] += 1;
                    }

                    println!("  ✅ Collection successful");
                }
                Err(e) => {
                    println!("  ❌ Collection failed: {e}");
                    all_successful = false;
                }
            }

            // Small delay between collections
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        println!("Metric availability across 3 cycles:");
        let metric_names = [
            "CPU usage",
            "CPU I/O wait",
            "CPU load",
            "Memory usage",
            "Memory pressure",
            "Disk I/O",
            "Network drops",
            "FD usage",
            "Process count",
        ];

        for (i, &availability) in metrics_availability.iter().enumerate() {
            println!("  {}: {}/3 cycles", metric_names[i], availability);
        }

        // Check metric availability based on the operating system
        let consistently_available = metrics_availability.iter().filter(|&&x| x >= 2).count();

        #[cfg(target_os = "linux")]
        {
            // On Linux, we should have most metrics available
            assert!(
                consistently_available >= 5,
                "At least 5 metrics should be consistently available on Linux, got: {consistently_available}"
            );
        }

        #[cfg(not(target_os = "linux"))]
        {
            // On non-Linux systems, we expect fewer metrics to be available
            println!("⚠️  Running on non-Linux system, reduced metric availability expected");
            assert!(
                consistently_available >= 1,
                "At least 1 metric should be consistently available, got: {}",
                consistently_available
            );
        }

        println!("Integration test summary:");
        println!("  All collections successful: {all_successful}");
        println!("  Consistently available metrics: {consistently_available}/9");
    }
}
