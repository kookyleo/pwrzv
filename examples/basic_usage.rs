//! Basic usage example
//!
//! Demonstrates how to use the pwrzv library for system monitoring

use pwrzv::{PowerReserveCalculator, PowerReserveLevel, PwrzvError};

fn main() -> Result<(), PwrzvError> {
    println!("=== pwrzv Basic Usage Example ===\n");

    // Check platform compatibility
    match pwrzv::check_platform() {
        Ok(()) => println!("✅ Platform check passed: {}", pwrzv::get_platform_name()),
        Err(e) => {
            eprintln!("❌ Platform check failed: {}", e);
            return Err(e);
        }
    }

    // Create calculator
    let calculator = PowerReserveCalculator::new();
    println!("📊 Power reserve calculator created");

    // Collect system metrics
    println!("🔍 Collecting system metrics...");
    let metrics = calculator.collect_metrics()?;
    
    // Validate metrics data
    if !metrics.validate() {
        eprintln!("⚠️  Warning: System metrics data is abnormal");
    }

    // Calculate simple score
    let score = calculator.calculate_power_reserve(&metrics)?;
    let level = PowerReserveLevel::from_score(score);

    println!("\n=== System Metrics ===");
    println!("CPU Usage:            {:.2}%", metrics.cpu_usage);
    println!("I/O Wait:             {:.2}%", metrics.cpu_iowait);
    println!("Memory Available:     {:.2}%", metrics.mem_available);
    println!("Swap Usage:           {:.2}%", metrics.swap_usage);
    println!("Disk I/O:             {:.2}%", metrics.disk_usage);
    println!("Network I/O:          {:.2}%", metrics.net_usage);
    println!("File Descriptor Usage: {:.2}%", metrics.fd_usage);

    println!("\n=== Power Reserve Assessment ===");
    println!("Score: {} / 5", score);
    println!("Level: {}", level);

    // Provide recommendations
    match level {
        PowerReserveLevel::Critical => {
            println!("\n🚨 System resources are severely constrained! Immediate optimization recommended.");
        }
        PowerReserveLevel::Low => {
            println!("\n⚠️  System resources are constrained, monitoring and optimization recommended.");
        }
        PowerReserveLevel::Moderate => {
            println!("\n✅ System running normally, moderate resource usage.");
        }
        PowerReserveLevel::Good => {
            println!("\n😊 System performance is good, with ample resource reserves.");
        }
        PowerReserveLevel::Excellent => {
            println!("\n🌟 System performance is excellent, with abundant resource reserves!");
        }
    }

    Ok(())
} 