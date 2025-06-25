//! Basic usage example
//!
//! Demonstrates how to use the pwrzv library for system monitoring

use pwrzv::{
    PwrzvError, check_platform, get_power_reserve_level_direct,
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

    let level = get_power_reserve_level_direct().await?;

    println!("Power Reserve Level: {level:.2}/5.0");

    // Provide interpretation based on level
    let (status, emoji, description) = interpret_level(level);
    println!("{emoji} {status}: {description}");

    // Example 2: Get detailed analysis
    println!("\n📊 Example 2: Detailed System Analysis");
    println!("{}", "-".repeat(40));

    let (detailed_level, details) = get_power_reserve_level_with_details_direct().await?;

    println!("Power Reserve Level: {detailed_level:.2}/5.0");
    println!("Available Metrics: {}", details.len());
    println!();

    // Display key metrics in a user-friendly way
    if !details.is_empty() {
        println!("📈 System Metrics (5.0-point scale with precision):");

        let mut sorted_metrics: Vec<_> = details.iter().collect();
        sorted_metrics.sort_by(|a, b| a.1.partial_cmp(b.1).unwrap());

        for (key, value) in sorted_metrics {
            let display_name = format_metric_name(key);
            let (status, emoji, _) = interpret_level(*value);

            println!("  {display_name:<30}: {value:.3} ({emoji} {status})");
        }
    }

    // Example 3: Real-time monitoring demonstration
    println!("\n🔄 Example 3: Real-time Monitoring (3 samples)");
    println!("{}", "-".repeat(40));

    let mut samples = Vec::new();
    for i in 1..=3 {
        let level = get_power_reserve_level_direct().await?;
        let (status, emoji, _) = interpret_level(level);
        println!("Sample {i}: Power Reserve = {level:.2}/5.0 ({emoji} {status})");
        samples.push(level);

        if i < 3 {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    }

    // Calculate some basic statistics
    let avg = samples.iter().sum::<f32>() / samples.len() as f32;
    let min = samples.iter().fold(f32::INFINITY, |a, &b| a.min(b));
    let max = samples.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));

    println!("\n📊 Sample Statistics:");
    println!("  Average: {avg:.3}");
    println!("  Range: {min:.3} - {max:.3}");
    println!(
        "  Stability: {}",
        if (max - min) < 0.5 {
            "Stable"
        } else {
            "Variable"
        }
    );

    println!("\n💡 Tips:");
    println!("  • Use `get_power_reserve_level_direct()` for quick monitoring");
    println!("  • Use `get_power_reserve_level_with_details_direct()` for detailed analysis");
    println!("  • All functions are async and collect metrics in real-time");
    println!("  • No background processes or storage - everything is direct!");
    println!("  • Values now have decimal precision for more accurate assessment");

    Ok(())
}

/// Interpret power reserve level
fn interpret_level(level: f32) -> (&'static str, &'static str, &'static str) {
    if level >= 4.5 {
        (
            "Abundant",
            "🌟",
            "Excellent! System has abundant resources.",
        )
    } else if level >= 3.5 {
        ("High", "✅", "Good! System resources are sufficient.")
    } else if level >= 2.5 {
        ("Medium", "⚠️", "Moderate load. Monitor for bottlenecks.")
    } else if level >= 1.5 {
        ("Low", "🔶", "High load. Consider optimization.")
    } else {
        ("Critical", "🚨", "Critical load! Immediate action needed.")
    }
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
