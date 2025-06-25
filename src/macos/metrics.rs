use crate::error::{PwrzvError, PwrzvResult};
use serde::{Deserialize, Serialize};
use std::str;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MacSystemMetrics {
    /// CPU total usage ratio: non-idle time percentage
    /// Range [0.0, 1.0]
    pub cpu_usage_ratio: Option<f32>,

    /// CPU load ratio: loadavg(1min) / core_count
    /// Approaching or exceeding 1.0 indicates increasing queued tasks
    pub cpu_load_ratio: Option<f32>,

    /// Memory usage ratio: (active + wired) / physical_mem
    /// Note: cached and compressed can also be included
    pub memory_usage_ratio: Option<f32>,

    /// Memory compressed ratio: compressed_pages / physical_pages
    /// Range [0.0, 1.0], approaching 1.0 means system heavily relies on memory compression
    pub memory_compressed_ratio: Option<f32>,

    /// Network dropped packets ratio: dropped_packets / total_packets
    /// Range [0.0, 1.0], approaching 1.0 means network is dropping many packets
    pub network_dropped_packets_ratio: Option<f32>,

    /// File descriptor usage ratio: open_fds / max_fds (approximation)
    /// Range [0.0, 1.0], approaching 1.0 means FD limit is reached
    pub fd_usage_ratio: Option<f32>,

    /// Process count ratio: current_processes / typical_max_processes
    /// Range [0.0, +∞], > 1.0 means process count exceeds typical limits
    pub process_count_ratio: Option<f32>,
}

impl MacSystemMetrics {
    /// Collect all system metrics using optimized consolidated calls
    pub async fn collect_system_metrics() -> PwrzvResult<Self> {
        // Execute all metrics collection in parallel using tokio::join!
        let (cpu_result, memory_result, network_result, system_resource_result) = tokio::join!(
            Self::get_cpu_metrics_consolidated(),
            Self::get_memory_metrics_consolidated(),
            Self::get_network_metrics_consolidated(),
            Self::get_system_resource_metrics_consolidated()
        );

        // Extract results, using None for any failed metrics
        let (cpu_usage_ratio, cpu_load_ratio) = cpu_result.unwrap_or((None, None));
        let (memory_usage_ratio, memory_compressed_ratio) = memory_result.unwrap_or((None, None));
        let network_dropped_packets_ratio = network_result.unwrap_or(None);
        let (fd_usage_ratio, process_count_ratio) = system_resource_result.unwrap_or((None, None));

        Ok(MacSystemMetrics {
            cpu_usage_ratio,
            cpu_load_ratio,
            memory_usage_ratio,
            memory_compressed_ratio,
            network_dropped_packets_ratio,
            fd_usage_ratio,
            process_count_ratio,
        })
    }

    /// Get CPU metrics with consolidated system calls
    ///
    /// Uses parallel execution of:
    /// - `top -l 1 -n 0`: CPU usage statistics
    /// - `sysctl vm.loadavg hw.ncpu`: Load average and CPU core count
    ///
    /// # Returns
    ///
    /// A tuple of `(cpu_usage_ratio, cpu_load_ratio)` where each may be `None`
    /// if the corresponding metric could not be parsed.
    ///
    /// # Performance
    ///
    /// This approach is optimized for macOS system limitations.
    pub(crate) async fn get_cpu_metrics_consolidated() -> PwrzvResult<(Option<f32>, Option<f32>)> {
        // Execute CPU usage and load/core count in parallel
        let (top_result, sysctl_result) = tokio::join!(
            tokio::process::Command::new("top")
                .args(["-l", "1", "-n", "0"])
                .output(),
            tokio::process::Command::new("sysctl")
                .args(["vm.loadavg", "hw.ncpu"])
                .output()
        );

        let mut cpu_usage: Option<f32> = None;
        let mut load_ratio: Option<f32> = None;

        // Get CPU usage from top command
        #[allow(clippy::collapsible_if)]
        if let Ok(top_output) = top_result {
            if let Ok(top_str) = str::from_utf8(&top_output.stdout) {
                cpu_usage = Self::parse_top_cpu_usage(top_str);
            }
        }

        // Get CPU load from sysctl
        #[allow(clippy::collapsible_if)]
        if let Ok(sysctl_output) = sysctl_result {
            if let Ok(sysctl_str) = str::from_utf8(&sysctl_output.stdout) {
                load_ratio = Self::parse_sysctl_load_and_cores(sysctl_str);
            }
        }

        Ok((cpu_usage, load_ratio))
    }

    /// Get memory metrics with consolidated calls (usage + compressed in single vm_stat call)
    pub(crate) async fn get_memory_metrics_consolidated() -> PwrzvResult<(Option<f32>, Option<f32>)>
    {
        let output = tokio::process::Command::new("vm_stat")
            .output()
            .await
            .map_err(|e| {
                PwrzvError::resource_access_error(&format!("Failed to execute vm_stat: {e}"))
            })?;

        let output_str = str::from_utf8(&output.stdout).map_err(|e| {
            PwrzvError::parse_error(&format!("Failed to parse vm_stat output: {e}"))
        })?;

        let mut pages_active = 0u64;
        let mut pages_inactive = 0u64;
        let mut pages_speculative = 0u64;
        let mut pages_throttled = 0u64;
        let mut pages_wired = 0u64;
        let mut pages_purgeable = 0u64;
        let mut pages_free = 0u64;
        let mut pages_compressed = 0u64;

        for line in output_str.lines() {
            if line.contains("Pages active:") {
                pages_active = Self::parse_vm_stat_number(line)?;
            } else if line.contains("Pages inactive:") {
                pages_inactive = Self::parse_vm_stat_number(line)?;
            } else if line.contains("Pages speculative:") {
                pages_speculative = Self::parse_vm_stat_number(line)?;
            } else if line.contains("Pages throttled:") {
                pages_throttled = Self::parse_vm_stat_number(line)?;
            } else if line.contains("Pages wired down:") {
                pages_wired = Self::parse_vm_stat_number(line)?;
            } else if line.contains("Pages purgeable:") {
                pages_purgeable = Self::parse_vm_stat_number(line)?;
            } else if line.contains("Pages free:") {
                pages_free = Self::parse_vm_stat_number(line)?;
            } else if line.contains("Pages stored in compressor:") {
                pages_compressed = Self::parse_vm_stat_number(line)?;
            }
        }

        let total_pages = pages_active
            + pages_inactive
            + pages_speculative
            + pages_throttled
            + pages_wired
            + pages_purgeable
            + pages_free;
        // In macOS, inactive and purgeable pages can be reclaimed immediately
        // Only count active and wired pages as truly "used"
        let used_pages = pages_active + pages_wired;

        if total_pages > 0 {
            let memory_usage = used_pages as f32 / total_pages as f32;
            let memory_compressed = if total_pages > 0 {
                Some((pages_compressed as f32 / total_pages as f32).min(1.0))
            } else {
                None
            };
            Ok((Some(memory_usage), memory_compressed))
        } else {
            Ok((None, None))
        }
    }

    /// Get network metrics with consolidated calls (drop ratio in single netstat call)
    pub(crate) async fn get_network_metrics_consolidated() -> PwrzvResult<Option<f32>> {
        let output = tokio::process::Command::new("netstat")
            .arg("-i")
            .arg("-b")
            .output()
            .await
            .map_err(|e| {
                PwrzvError::resource_access_error(&format!("Failed to execute netstat: {e}"))
            })?;

        let output_str = str::from_utf8(&output.stdout).map_err(|e| {
            PwrzvError::parse_error(&format!("Failed to parse netstat output: {e}"))
        })?;

        let mut total_packets = 0u64;
        let mut total_dropped = 0u64;

        for line in output_str.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();

            // Skip header, loopback, and inactive interfaces
            if parts.len() >= 10
                && !parts[0].starts_with("Name")
                && parts[0] != "lo0"
                && !parts[0].ends_with('*')
            {
                // Get packet counts for drop ratio calculation
                let packets_in = parts
                    .get(4)
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(0);
                let packets_out = parts
                    .get(7)
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(0);

                // Only count interfaces that have actual traffic
                if packets_in > 0 || packets_out > 0 {
                    total_packets += packets_in + packets_out;

                    // Get error counts (column 5 for input errors, column 8 for output errors)
                    let errors_in = parts
                        .get(5)
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(0);
                    let errors_out = parts
                        .get(8)
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(0);
                    total_dropped += errors_in + errors_out;
                }
            }
        }

        let dropped_ratio = if total_packets > 0 {
            Some((total_dropped as f32 / total_packets as f32).min(1.0))
        } else {
            None
        };

        Ok(dropped_ratio)
    }

    /// Get system resource metrics with consolidated calls (fd usage + process count in optimized calls)
    pub(crate) async fn get_system_resource_metrics_consolidated()
    -> PwrzvResult<(Option<f32>, Option<f32>)> {
        // Execute all three commands in parallel for maximum efficiency
        let (ps_result, sysctl_result, fd_limit_result) = tokio::join!(
            // Get process count from ps
            tokio::process::Command::new("ps").arg("ax").output(),
            // Get system process limit from sysctl
            tokio::process::Command::new("sysctl")
                .arg("kern.maxproc")
                .output(),
            // Get system-wide file descriptor limit from sysctl
            tokio::process::Command::new("sysctl")
                .arg("kern.maxfiles")
                .output()
        );

        // Parse process count and calculate ratios
        let mut fd_usage_ratio: Option<f32> = None;
        let mut process_count_ratio: Option<f32> = None;

        // Get process count and file descriptor metrics
        #[allow(clippy::collapsible_if)]
        if let Ok(ps_output) = ps_result {
            if let Ok(ps_str) = str::from_utf8(&ps_output.stdout) {
                let process_count = ps_str.lines().count().saturating_sub(1) as u32;

                // Get file descriptor limits and usage
                #[allow(clippy::collapsible_if)]
                if let Ok(fd_limit_output) = fd_limit_result {
                    if let Ok(fd_limit_str) = str::from_utf8(&fd_limit_output.stdout) {
                        // Parse sysctl output: "kern.maxfiles: 245760"
                        #[allow(clippy::collapsible_if)]
                        if let Some(colon_pos) = fd_limit_str.find(':') {
                            if let Ok(fd_limit) =
                                fd_limit_str[colon_pos + 1..].trim().parse::<u32>()
                            {
                                // Get actual open file descriptors using lsof
                                #[allow(clippy::collapsible_if)]
                                if let Ok(lsof_output) = tokio::process::Command::new("lsof")
                                    .arg("-n") // Don't resolve hostnames
                                    .arg("-P") // Don't resolve port names
                                    .output()
                                    .await
                                {
                                    if lsof_output.status.success() {
                                        // Use lossy conversion to handle non-UTF8 characters in file paths
                                        let lsof_str = String::from_utf8_lossy(&lsof_output.stdout);
                                        // Subtract 1 for header line
                                        let actual_fds = lsof_str.lines().count().saturating_sub(1);
                                        fd_usage_ratio =
                                            Some((actual_fds as f32 / fd_limit as f32).min(1.0));
                                    }
                                }
                            }
                        }
                    }
                }

                // Get process count ratio
                #[allow(clippy::collapsible_if)]
                if let Ok(sysctl_output) = sysctl_result {
                    if let Ok(sysctl_str) = str::from_utf8(&sysctl_output.stdout) {
                        #[allow(clippy::collapsible_if)]
                        if let Some(colon_pos) = sysctl_str.find(':') {
                            if let Ok(max_processes) =
                                sysctl_str[colon_pos + 1..].trim().parse::<u32>()
                            {
                                process_count_ratio =
                                    Some((process_count as f32 / max_processes as f32).min(2.0));
                            }
                        }
                    }
                }
            }
        }

        Ok((fd_usage_ratio, process_count_ratio))
    }

    // Private helper methods
    fn parse_vm_stat_number(line: &str) -> PwrzvResult<u64> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if let Some(last_part) = parts.last() {
            let number_str = last_part.trim_end_matches('.');
            number_str.parse::<u64>().map_err(|e| {
                PwrzvError::parse_error(&format!(
                    "Failed to parse number from vm_stat line '{line}': {e}"
                ))
            })
        } else {
            Err(PwrzvError::parse_error(&format!(
                "No parseable parts found in vm_stat line: '{line}'"
            )))
        }
    }

    /// Parse CPU usage from top command output
    ///
    /// Expected format: "CPU usage: 12.34% user, 5.67% sys, 81.99% idle"
    fn parse_top_cpu_usage(output: &str) -> Option<f32> {
        for line in output.lines() {
            if line.contains("CPU usage:") {
                // Parse line like: "CPU usage: 12.34% user, 5.67% sys, 81.99% idle"
                let parts: Vec<&str> = line.split(',').collect();

                let mut user_pct = 0.0f32;
                let mut sys_pct = 0.0f32;

                for part in parts {
                    let part = part.trim();
                    if part.contains("% user") {
                        #[allow(clippy::collapsible_if)]
                        if let Some(pct_str) = part.split('%').next() {
                            if let Some(num_str) = pct_str.split_whitespace().last() {
                                user_pct = num_str.parse::<f32>().unwrap_or(0.0);
                            }
                        }
                    } else if part.contains("% sys") {
                        #[allow(clippy::collapsible_if)]
                        if let Some(pct_str) = part.split('%').next() {
                            if let Some(num_str) = pct_str.split_whitespace().last() {
                                sys_pct = num_str.parse::<f32>().unwrap_or(0.0);
                            }
                        }
                    }
                }

                let total_usage = (user_pct + sys_pct) / 100.0;
                return Some(total_usage.clamp(0.0, 1.0));
            }
        }
        None
    }

    /// Parse load average and CPU cores from sysctl output
    ///
    /// Expected format:
    /// ```text
    /// vm.loadavg: { 1.23 2.34 3.45 }
    /// hw.ncpu: 8
    /// ```
    fn parse_sysctl_load_and_cores(output: &str) -> Option<f32> {
        let mut load_1min: Option<f32> = None;
        let mut core_count: Option<u32> = None;

        for line in output.lines() {
            let line = line.trim();

            if line.starts_with("vm.loadavg:") {
                load_1min = Self::parse_load_average(line);
            } else if line.starts_with("hw.ncpu:") {
                core_count = Self::parse_cpu_core_count(line);
            }
        }

        if let (Some(load), Some(cores)) = (load_1min, core_count) {
            // Convert temporary load calculation to actual ratio
            let actual_load = load * 4.0; // Reverse the temporary calculation
            Some((actual_load / cores as f32).min(2.0))
        } else {
            None
        }
    }

    /// Parse load average from vm.loadavg output
    ///
    /// Expected format: `vm.loadavg: { 1.23 1.45 1.67 }`
    fn parse_load_average(line: &str) -> Option<f32> {
        let brace_start = line.find('{')?;
        let brace_end = line.find('}')?;
        let load_data = &line[brace_start + 1..brace_end];
        let parts: Vec<&str> = load_data.split_whitespace().collect();

        if !parts.is_empty() {
            let load_1min = parts[0].parse::<f32>().ok()?;
            // Temporary calculation, will be updated with actual core count
            Some((load_1min / 4.0).min(2.0))
        } else {
            None
        }
    }

    /// Parse CPU core count from hw.ncpu output
    ///
    /// Expected format: `hw.ncpu: 8`
    fn parse_cpu_core_count(line: &str) -> Option<u32> {
        let colon_pos = line.find(':')?;
        line[colon_pos + 1..].trim().parse::<u32>().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_metrics_fetch() {
        println!("Testing real macOS system metrics with parallel execution...");

        // Execute all metrics collection in parallel using tokio::join!
        let (cpu_result, memory_result, network_result, system_resource_result) = tokio::join!(
            MacSystemMetrics::get_cpu_metrics_consolidated(),
            MacSystemMetrics::get_memory_metrics_consolidated(),
            MacSystemMetrics::get_network_metrics_consolidated(),
            MacSystemMetrics::get_system_resource_metrics_consolidated()
        );

        // Verify and print CPU metrics
        let cpu_metrics = cpu_result.unwrap();
        println!("CPU metrics: {cpu_metrics:?}");
        // CPU usage can be None if parsing fails, which is valid behavior
        if let Some(cpu_usage) = cpu_metrics.0 {
            assert!((0.0..=1.0).contains(&cpu_usage));
        }
        // Load ratio can be None if parsing fails, which is valid behavior
        if let Some(load_ratio) = cpu_metrics.1 {
            assert!(load_ratio >= 0.0); // Load ratio can exceed 1.0
        }

        // Verify and print memory metrics
        let memory_metrics = memory_result.unwrap();
        println!("Memory metrics: {memory_metrics:?}");
        // Memory metrics can be None if parsing fails, which is valid behavior
        if let Some(memory_usage) = memory_metrics.0 {
            assert!((0.0..=1.0).contains(&memory_usage));
        }
        if let Some(memory_compressed) = memory_metrics.1 {
            assert!((0.0..=1.0).contains(&memory_compressed));
        }

        // Verify and print network metrics
        let network_metrics = network_result.unwrap();
        println!("Network metrics: {network_metrics:?}");
        // Network metrics can be None if parsing fails, which is valid behavior
        if let Some(dropped_ratio) = network_metrics {
            assert!((0.0..=1.0).contains(&dropped_ratio));
        }

        // Verify and print system resource metrics
        let system_resource_metrics = system_resource_result.unwrap();
        println!("System resource metrics: {system_resource_metrics:?}");
        // System resource metrics can be None if parsing fails, which is valid behavior
        if let Some(fd_usage) = system_resource_metrics.0 {
            assert!((0.0..=1.0).contains(&fd_usage));
        }
        if let Some(process_count_ratio) = system_resource_metrics.1 {
            assert!(process_count_ratio >= 0.0); // Process count ratio can exceed 1.0
        }
    }

    #[tokio::test]
    async fn test_get_memory_metrics_consolidated() {
        let result = MacSystemMetrics::get_memory_metrics_consolidated().await;
        assert!(result.is_ok());

        let (memory_usage, memory_compressed) = result.unwrap();
        println!(
            "Memory usage: {}, compressed: {}",
            memory_usage.unwrap_or(0.0),
            memory_compressed.unwrap_or(0.0)
        );

        // Memory metrics can be None if parsing fails, which is valid behavior
        if let Some(usage) = memory_usage {
            assert!((0.0..=1.0).contains(&usage));
        }
        if let Some(compressed) = memory_compressed {
            assert!((0.0..=1.0).contains(&compressed));
        }
    }

    #[tokio::test]
    async fn test_get_cpu_metrics_consolidated() {
        let result = MacSystemMetrics::get_cpu_metrics_consolidated().await;
        assert!(result.is_ok());

        let (cpu_usage, load_ratio) = result.unwrap();
        println!(
            "CPU usage: {}, load ratio: {}",
            cpu_usage.unwrap_or(0.0),
            load_ratio.unwrap_or(0.0)
        );

        // CPU metrics can be None if parsing fails, which is valid behavior
        if let Some(usage) = cpu_usage {
            assert!((0.0..=1.0).contains(&usage));
        }
        if let Some(ratio) = load_ratio {
            assert!(ratio >= 0.0);
        }
    }

    #[tokio::test]
    async fn test_get_network_metrics_consolidated() {
        let result = MacSystemMetrics::get_network_metrics_consolidated().await;
        assert!(result.is_ok());

        let dropped_ratio = result.unwrap();
        println!("Dropped packets ratio: {}", dropped_ratio.unwrap_or(0.0));

        // Network metrics can be None if parsing fails, which is valid behavior
        if let Some(ratio) = dropped_ratio {
            assert!((0.0..=1.0).contains(&ratio));
        }
    }

    #[test]
    fn test_parse_vm_stat_number() {
        // Test valid vm_stat line formats
        assert_eq!(
            MacSystemMetrics::parse_vm_stat_number("Pages active:                      123456.")
                .unwrap(),
            123456
        );
        assert_eq!(
            MacSystemMetrics::parse_vm_stat_number("Pages free:                        789012.")
                .unwrap(),
            789012
        );
        assert_eq!(
            MacSystemMetrics::parse_vm_stat_number("Pages wired down:                  456789.")
                .unwrap(),
            456789
        );

        // Test number without trailing dot
        assert_eq!(
            MacSystemMetrics::parse_vm_stat_number("Pages speculative:                 987654")
                .unwrap(),
            987654
        );

        // Test with extra whitespace
        assert_eq!(
            MacSystemMetrics::parse_vm_stat_number("  Pages compressed:    12345.  ").unwrap(),
            12345
        );

        // Test error cases - these should return Err, not fallback values
        assert!(MacSystemMetrics::parse_vm_stat_number("").is_err());
        assert!(MacSystemMetrics::parse_vm_stat_number("   ").is_err());
        assert!(MacSystemMetrics::parse_vm_stat_number("Pages active:").is_err());
        assert!(MacSystemMetrics::parse_vm_stat_number("Pages active: invalid").is_err());
        assert!(MacSystemMetrics::parse_vm_stat_number("Pages active: -123").is_err()); // Negative numbers

        // Test edge cases
        assert_eq!(
            MacSystemMetrics::parse_vm_stat_number("Pages zero:                        0.")
                .unwrap(),
            0
        );
        assert_eq!(
            MacSystemMetrics::parse_vm_stat_number(
                "Pages large:                       18446744073709551615."
            )
            .unwrap(),
            18446744073709551615u64
        ); // Max u64
    }
}
