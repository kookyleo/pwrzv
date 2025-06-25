//! Detailed metrics example
//!
//! Demonstrates advanced usage of the pwrzv library with detailed metrics

use pwrzv::{PwrzvError, get_power_reserve_level_with_details_direct};
use std::collections::HashMap;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), PwrzvError> {
    println!("=== pwrzv Detailed Metrics Analysis ===\n");

    // Example 1: Comprehensive system analysis
    println!("🔍 Example 1: Current System Analysis");
    println!("{}", "=".repeat(50));

    let (level, details) = get_power_reserve_level_with_details_direct().await?;

    println!("Power Reserve Level: {level:.3}/5.0");

    let assessment = categorize_level(level);
    println!("   {assessment}");

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
        println!("⏱️  Collecting sample {i} of 3...");

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
            .unwrap_or(3.0);
        let memory_score = details
            .iter()
            .find(|(k, _)| k.contains("Memory"))
            .map(|(_, v)| *v)
            .unwrap_or(3.0);

        println!(
            "   {sample_num:6} | {level:5.2} | CPU: {cpu_score:.2}, Memory: {memory_score:.2}"
        );
    }

    // Show if there's a trend
    let levels: Vec<f32> = samples.iter().map(|(_, level, _)| *level).collect();
    if levels.len() >= 2 {
        let trend = if levels.last() > levels.first() {
            "📈 Improving"
        } else if levels.last() < levels.first() {
            "📉 Degrading"
        } else {
            "➡️ Stable"
        };
        println!("\n🔄 Trend: {trend}");
    }

    // Example 3: Metric explanation
    println!("\n📚 Example 3: Understanding Metrics");
    println!("{}", "=".repeat(40));

    let (_, final_details) = get_power_reserve_level_with_details_direct().await?;

    println!("💡 Metric Explanation:");
    println!("   • Scores range from 1.0 (Critical) to 5.0 (Abundant)");
    println!("   • Higher values = better performance");
    println!("   • Lower values = more system stress");
    println!("   • Decimal precision allows for nuanced assessment");
    println!();

    explain_top_metrics(&final_details);

    println!("\n🚀 Usage Tips:");
    println!("   • Call `get_power_reserve_level_with_details_direct()` for analysis");
    println!("   • All data is collected in real-time - no background processes");
    println!("   • Metrics are platform-specific (Linux vs macOS)");
    println!("   • Use this for detailed diagnostics and monitoring");
    println!("   • Focus on consistently low-scoring metrics for optimization");

    Ok(())
}

/// Categorize power reserve level
fn categorize_level(level: f32) -> String {
    let (status, emoji) = match level {
        l if l >= 4.5 => ("Excellent - System running at optimal performance", "🌟"),
        l if l >= 3.5 => ("Good - System has sufficient resources available", "✅"),
        l if l >= 2.5 => ("Moderate - Monitor for potential bottlenecks", "⚠️"),
        l if l >= 1.5 => ("High Load - Resource optimization recommended", "🔶"),
        _ => ("Critical - Immediate attention required", "🚨"),
    };
    format!("{emoji} {status}")
}

/// Display metrics organized by category
fn display_metrics_by_category(details: &HashMap<String, f32>) {
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
            println!("\n🔧 {category_name}:");
            let mut sorted_metrics = category_metrics;
            sorted_metrics.sort_by_key(|(k, _)| *k);

            for (key, value) in sorted_metrics {
                let status = get_score_status(*value);
                println!("   {key:<35}: {value:.3} ({status})");
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
            println!("   {key:<35}: {value:.3} ({status})");
        }
    }
}

/// Explain the metrics with lowest scores (highest stress)
fn explain_top_metrics(details: &HashMap<String, f32>) {
    let mut sorted_metrics: Vec<_> = details.iter().collect();
    sorted_metrics.sort_by(|a, b| a.1.partial_cmp(b.1).unwrap()); // Sort by score, lowest first

    let stressed_metrics: Vec<_> = sorted_metrics.into_iter().take(3).collect();

    if !stressed_metrics.is_empty() {
        println!("🔍 Most Stressed Resources:");
        for (rank, (key, value)) in stressed_metrics.iter().enumerate() {
            let explanation = get_metric_explanation(key);
            println!("   {}. {}: {:.3} - {}", rank + 1, key, value, explanation);
        }
    }
}

/// Get score status description
fn get_score_status(value: f32) -> &'static str {
    match value {
        v if v >= 4.5 => "🌟 Abundant",
        v if v >= 3.5 => "✅ High",
        v if v >= 2.5 => "⚠️  Medium",
        v if v >= 1.5 => "🔶 Low",
        _ => "🚨 Critical",
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
        "RAM usage is elevated"
    } else if key.contains("Memory Compressed") {
        "Memory compression is active"
    } else if key.contains("Memory Pressure") {
        "System memory pressure detected"
    } else if key.contains("Disk IO") {
        "Disk I/O utilization is high"
    } else if key.contains("Network") {
        "Network packet dropping detected"
    } else if key.contains("File Descriptors") {
        "File descriptor usage is high"
    } else if key.contains("Process Count") {
        "Many processes are running"
    } else {
        "Resource utilization metric"
    }
}
