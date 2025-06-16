//! Basic usage example
//!
//! Demonstrates how to use the pwrzv library for system monitoring

use pwrzv::{PowerReserveCalculator, PowerReserveLevel, PwrzvError, platform};

fn main() -> Result<(), PwrzvError> {
    println!("=== pwrzv Basic Usage Example ===\n");

    // Check platform compatibility first
    if let Err(e) = platform::check_platform() {
        eprintln!("❌ Platform check failed: {e}");
        eprintln!("pwrzv currently only supports Linux systems.");
        eprintln!("This example will exit gracefully without running the analysis.");
        return Ok(()); // Return Ok to avoid non-zero exit code
    }

    println!("✅ Platform check passed!");

    // Create calculator with default configuration
    let calculator = PowerReserveCalculator::new();

    // Collect system metrics
    let metrics = match calculator.collect_metrics() {
        Ok(metrics) => metrics,
        Err(e) => {
            eprintln!("Failed to collect system metrics: {e}");
            eprintln!("This example will exit gracefully.");
            return Ok(()); // Return Ok to avoid non-zero exit code
        }
    };

    // Calculate power reserve score
    let score = match calculator.calculate_power_reserve(&metrics) {
        Ok(score) => score,
        Err(e) => {
            eprintln!("Failed to calculate power reserve: {e}");
            eprintln!("This example will exit gracefully.");
            return Ok(()); // Return Ok to avoid non-zero exit code
        }
    };

    // Convert score to level for display
    let level = PowerReserveLevel::from_score(score);

    // Display results
    println!();
    println!("=== System Power Reserve Analysis ===");
    println!(
        "CPU Usage: {:.1}% (iowait: {:.1}%)",
        metrics.cpu_usage, metrics.cpu_iowait
    );
    println!("Memory Available: {:.1}%", metrics.mem_available);
    println!("Swap Usage: {:.1}%", metrics.swap_usage);
    println!("Disk I/O Usage: {:.1}%", metrics.disk_usage);
    println!("Network I/O Usage: {:.1}%", metrics.net_usage);
    println!("File Descriptor Usage: {:.1}%", metrics.fd_usage);
    println!();
    println!("Score: {score} / 5");
    println!("Level: {level}");

    // Provide recommendations
    match level {
        PowerReserveLevel::Critical => {
            println!(
                "\n🚨 System resources are severely constrained! Immediate optimization recommended."
            );
        }
        PowerReserveLevel::Low => {
            println!(
                "\n⚠️  System resources are constrained, monitoring and optimization recommended."
            );
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
