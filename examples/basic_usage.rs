//! Basic usage example
//!
//! Demonstrates how to use the pwrzv library for system monitoring

use pwrzv::{PowerReserveLevel, PwrzvError, check_platform, get_provider};

#[tokio::main]
async fn main() -> Result<(), PwrzvError> {
    println!("=== pwrzv Basic Usage Example ===\n");

    // Check platform compatibility
    if let Err(e) = check_platform() {
        eprintln!("❌ Platform check failed: {e}");
        eprintln!("pwrzv currently supports Linux and macOS systems.");
        eprintln!("This example will exit gracefully without running the analysis.");
        return Ok(());
    }

    println!("✅ Platform check passed!");

    // Get platform-specific provider
    let provider = get_provider();

    // Get power reserve level and details
    let (level_u8, details) = match provider.get_power_reserve_level_with_details().await {
        Ok((level, details)) => (level, details),
        Err(e) => {
            eprintln!("Failed to get system metrics: {e}");
            return Ok(());
        }
    };

    let level = PowerReserveLevel::try_from(level_u8)?;

    // Display results
    println!("\n=== System Power Reserve Analysis ===");
    println!("📊 Key Metrics:");

    // Display available metrics
    if let Some(cpu_usage) = details.get("cpu_usage_ratio") {
        println!(
            "  CPU Usage:     {:.1}% (pressure: {:.3})",
            cpu_usage * 100.0,
            cpu_usage
        );
    }
    if let Some(memory_usage) = details.get("memory_usage_ratio") {
        println!(
            "  Memory Usage:  {:.1}% (pressure: {:.3})",
            memory_usage * 100.0,
            memory_usage
        );
    }
    if let Some(disk_io) = details.get("disk_io_ratio") {
        println!(
            "  Disk I/O:      {:.1}% (pressure: {:.3})",
            disk_io * 100.0,
            disk_io
        );
    }
    if let Some(network) = details.get("network_bandwidth_ratio") {
        println!(
            "  Network:       {:.1}% (pressure: {:.3})",
            network * 100.0,
            network
        );
    }

    println!();
    println!("Power Reserve Level: {level} ({level_u8} / 5)");

    // Performance assessment
    match level_u8 {
        4..=5 => println!("\n🌟 Excellent! System has abundant resource reserves."),
        2..=3 => println!("\n⚠️  Moderate load detected. Monitor system performance."),
        _ => println!("\n🚨 Heavy load! Consider optimizing resource usage."),
    }

    Ok(())
}
