//! Basic usage example
//!
//! Demonstrates how to use the pwrzv library for system monitoring

use pwrzv::{
    PowerReserveLevel, PwrzvError, check_platform, get_power_reserve_level_direct,
    get_power_reserve_level_with_details_direct,
};

#[tokio::main]
async fn main() -> Result<(), PwrzvError> {
    println!("=== pwrzv Basic Usage Example ===\n");

    // Check platform compatibility
    if let Err(e) = check_platform() {
        eprintln!("❌ Platform check failed: {e}");
        eprintln!("pwrzv currently supports Linux and macOS systems.");
        return Ok(());
    }

    println!("✅ Platform check passed!");

    // Example 1: Get simple power reserve level
    println!("\n🔋 Example 1: Simple Power Reserve Level");
    println!("{}", "-".repeat(40));

    let level_u8 = get_power_reserve_level_direct().await?;
    let level = PowerReserveLevel::try_from(level_u8)?;

    println!("Power Reserve Level: {} ({}/5)", level, level_u8);

    match level {
        PowerReserveLevel::Abundant => println!("🌟 Excellent! System has abundant resources."),
        PowerReserveLevel::High => println!("✅ Good! System resources are sufficient."),
        PowerReserveLevel::Medium => println!("⚠️  Moderate load. Monitor for bottlenecks."),
        PowerReserveLevel::Low => println!("🔶 High load. Consider optimization."),
        PowerReserveLevel::Critical => println!("🚨 Critical load! Immediate action needed."),
    }

    // Example 2: Get detailed analysis
    println!("\n📊 Example 2: Detailed System Analysis");
    println!("{}", "-".repeat(40));

    let (detailed_level, details) = get_power_reserve_level_with_details_direct().await?;

    println!("Power Reserve Level: {}/5", detailed_level);
    println!("Available Metrics: {}", details.len());
    println!();

    // Display key metrics in a user-friendly way
    if !details.is_empty() {
        println!("📈 System Metrics (5-point scale: 5=Abundant, 1=Critical):");

        let mut sorted_metrics: Vec<_> = details.iter().collect();
        sorted_metrics.sort_by_key(|(k, _)| *k);

        for (key, value) in sorted_metrics {
            let display_name = format_metric_name(key);
            let status = match *value {
                5 => "🌟 Abundant",
                4 => "✅ High",
                3 => "⚠️  Medium",
                2 => "🔶 Low",
                1 => "🚨 Critical",
                _ => "❓ Unknown",
            };

            println!("  {:<30}: {} ({})", display_name, value, status);
        }
    }

    // Example 3: Real-time monitoring demonstration
    println!("\n🔄 Example 3: Real-time Monitoring (3 samples)");
    println!("{}", "-".repeat(40));

    for i in 1..=3 {
        let level = get_power_reserve_level_direct().await?;
        println!("Sample {}: Power Reserve = {}/5", i, level);

        if i < 3 {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    }

    println!("\n💡 Tips:");
    println!("  • Use `get_power_reserve_level_direct()` for quick monitoring");
    println!("  • Use `get_power_reserve_level_with_details_direct()` for detailed analysis");
    println!("  • All functions are async and collect metrics in real-time");
    println!("  • No background processes or storage - everything is direct!");

    Ok(())
}

/// Format metric name for display
fn format_metric_name(key: &str) -> String {
    key.replace('_', " ")
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
