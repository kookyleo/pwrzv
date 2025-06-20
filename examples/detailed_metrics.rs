//! Detailed metrics example
//!
//! Demonstrates advanced usage of the pwrzv library with detailed system analysis

use pwrzv::{PowerReserveLevel, PwrzvError, get_power_reserve_level_with_details_direct};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), PwrzvError> {
    println!("=== pwrzv Detailed Metrics Analysis ===\n");

    // Example 1: Comprehensive system analysis
    println!("🔍 Example 1: Current System Analysis");
    println!("{}", "=".repeat(50));

    let (level_u8, details) = get_power_reserve_level_with_details_direct().await?;
    let level = PowerReserveLevel::try_from(level_u8)?;

    println!("Power Reserve Level: {} ({}/5)", level, level_u8);

    let assessment = match level_u8 {
        5 => "🌟 Excellent - System running at optimal performance",
        4 => "✅ Good - System has sufficient resources available",
        3 => "⚠️  Moderate - Monitor for potential bottlenecks",
        2 => "🔶 High Load - Resource optimization recommended",
        1 => "🚨 Critical - Immediate attention required",
        _ => "❓ Unknown state",
    };
    println!("   {}", assessment);

    if !details.is_empty() {
        println!("\n📊 Detailed Metrics ({} available):", details.len());
        display_metrics_by_category(&details);
    } else {
        println!("\n⚠️  No detailed metrics available");
    }

    // Example 2: Comparative analysis over time
    println!("\n📈 Example 2: Comparative Analysis (3 samples over 5 seconds)");
    println!("{}", "=".repeat(65));

    let mut samples = Vec::new();

    for i in 1..=3 {
        println!("⏱️  Collecting sample {} of 3...", i);

        let (sample_level, sample_details) = get_power_reserve_level_with_details_direct().await?;
        samples.push((i, sample_level, sample_details));

        if i < 3 {
            tokio::time::sleep(Duration::from_millis(2500)).await;
        }
    }

    // Display trend analysis
    println!("\n📊 Trend Analysis:");
    println!("   Sample | Level | Key Metrics");
    println!("   {}", "-".repeat(45));

    for (sample_num, level, details) in &samples {
        // Extract a few key metrics for comparison
        let cpu_score = details
            .iter()
            .find(|(k, _)| k.contains("CPU Usage"))
            .map(|(_, v)| *v)
            .unwrap_or(3);
        let memory_score = details
            .iter()
            .find(|(k, _)| k.contains("Memory"))
            .map(|(_, v)| *v)
            .unwrap_or(3);

        println!(
            "   {:6} | {:5} | CPU: {}, Memory: {}",
            sample_num, level, cpu_score, memory_score
        );
    }

    // Show if there's a trend
    let levels: Vec<u8> = samples.iter().map(|(_, level, _)| *level).collect();
    if levels.len() >= 2 {
        let trend = if levels.last() > levels.first() {
            "📈 Improving"
        } else if levels.last() < levels.first() {
            "📉 Degrading"
        } else {
            "➡️ Stable"
        };
        println!("\n🔄 Trend: {}", trend);
    }

    // Example 3: Metric explanation
    println!("\n📚 Example 3: Understanding Metrics");
    println!("{}", "=".repeat(40));

    let (_, final_details) = get_power_reserve_level_with_details_direct().await?;

    println!("💡 Metric Explanation:");
    println!("   • Scores range from 1 (Critical) to 5 (Abundant)");
    println!("   • Higher values = better performance");
    println!("   • Lower values = more system stress");
    println!();

    explain_top_metrics(&final_details);

    println!("\n🚀 Usage Tips:");
    println!("   • Call `get_power_reserve_level_with_details_direct()` for analysis");
    println!("   • All data is collected in real-time - no background processes");
    println!("   • Metrics are platform-specific (Linux vs macOS)");
    println!("   • Use this for detailed diagnostics and monitoring");

    Ok(())
}

/// Display metrics organized by category
fn display_metrics_by_category(details: &std::collections::HashMap<String, u8>) {
    let categories = [
        ("CPU Metrics", vec!["CPU Usage", "CPU Load", "CPU IO Wait"]),
        (
            "Memory Metrics",
            vec!["Memory Usage", "Memory Compressed", "Memory Pressure"],
        ),
        ("Storage Metrics", vec!["Disk IO"]),
        ("Network Metrics", vec!["Network"]),
        ("System Metrics", vec!["File Descriptors", "Process Count"]),
    ];

    // Collect all known prefixes first
    let known_prefixes: Vec<&str> = categories
        .iter()
        .flat_map(|(_, prefixes)| prefixes.iter().copied())
        .collect();

    for (category_name, prefixes) in &categories {
        let category_metrics: Vec<_> = details
            .iter()
            .filter(|(key, _)| prefixes.iter().any(|prefix| key.contains(prefix)))
            .collect();

        if !category_metrics.is_empty() {
            println!("\n🔧 {}:", category_name);
            let mut sorted_metrics = category_metrics;
            sorted_metrics.sort_by_key(|(k, _)| *k);

            for (key, value) in sorted_metrics {
                let status = get_score_status(*value);
                println!("   {:<35}: {} ({})", key, value, status);
            }
        }
    }

    // Show any remaining metrics
    let other_metrics: Vec<_> = details
        .iter()
        .filter(|(key, _)| !known_prefixes.iter().any(|prefix| key.contains(prefix)))
        .collect();

    if !other_metrics.is_empty() {
        println!("\n🔍 Other Metrics:");
        let mut sorted_other = other_metrics;
        sorted_other.sort_by_key(|(k, _)| *k);

        for (key, value) in sorted_other {
            let status = get_score_status(*value);
            println!("   {:<35}: {} ({})", key, value, status);
        }
    }
}

/// Explain the metrics with lowest scores (highest stress)
fn explain_top_metrics(details: &std::collections::HashMap<String, u8>) {
    let mut sorted_metrics: Vec<_> = details.iter().collect();
    sorted_metrics.sort_by_key(|(_, v)| *v); // Sort by score, lowest first

    let stressed_metrics: Vec<_> = sorted_metrics.into_iter().take(3).collect();

    if !stressed_metrics.is_empty() {
        println!("🔍 Most Stressed Resources:");
        for (rank, (key, value)) in stressed_metrics.iter().enumerate() {
            let explanation = get_metric_explanation(key);
            println!("   {}. {}: {} - {}", rank + 1, key, value, explanation);
        }
    }
}

/// Get score status description
fn get_score_status(value: u8) -> &'static str {
    match value {
        5 => "🌟 Abundant",
        4 => "✅ High",
        3 => "⚠️  Medium",
        2 => "🔶 Low",
        1 => "🚨 Critical",
        _ => "❓ Unknown",
    }
}

/// Get explanation for a metric
fn get_metric_explanation(key: &str) -> &'static str {
    if key.contains("CPU Usage") {
        "CPU is busy processing tasks"
    } else if key.contains("CPU Load") {
        "System load average is high"
    } else if key.contains("CPU IO Wait") {
        "CPU waiting for I/O operations"
    } else if key.contains("Memory Usage") {
        "Physical memory utilization"
    } else if key.contains("Memory Compressed") {
        "Memory compression pressure"
    } else if key.contains("Memory Pressure") {
        "Overall memory pressure"
    } else if key.contains("Disk IO") {
        "Disk I/O activity level"
    } else if key.contains("Network") {
        "Network traffic or errors"
    } else if key.contains("File Descriptors") {
        "File descriptor usage"
    } else if key.contains("Process Count") {
        "Number of running processes"
    } else {
        "System resource pressure"
    }
}
