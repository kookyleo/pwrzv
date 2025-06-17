//! Detailed metrics example
//!
//! Shows comprehensive system metrics analysis

use pwrzv::{PowerReserveLevel, PwrzvError, check_platform, get_provider};

#[tokio::main]
async fn main() -> Result<(), PwrzvError> {
    println!("=== pwrzv Detailed Metrics Example ===\n");

    // Check platform and get provider
    check_platform()?;
    let provider = get_provider();

    // Get detailed system metrics
    let (level_u8, details) = provider.get_power_reserve_level_with_details().await?;
    let level = PowerReserveLevel::try_from(level_u8)?;

    println!("📊 System Metrics Analysis:");
    println!("{}", "=".repeat(40));

    // Display raw metrics
    println!("\n📈 System Pressure Metrics:");
    for (key, value) in &details {
        if key.ends_with("_ratio") && !key.contains("score") {
            let name = format_metric_name(key);
            println!("  {:<18}: {:.3} ({:.1}%)", name, *value, value * 100.0);
        }
    }

    // Display calculated scores
    println!("\n🎯 Component Scores:");
    for (key, value) in &details {
        if key.ends_with("_score") {
            let name = format_metric_name(&key.replace("_score", ""));
            let score = (1.0 - value) * 5.0; // Convert pressure to reserve score
            println!("  {name:<18}: {score:.2} / 5.0");
        }
    }

    println!("\n🏆 Overall Result:");
    println!("  Reserve Level: {level} ({level_u8}/5)");

    // System assessment
    let assessment = match level_u8 {
        5 => "🌟 Excellent - Abundant resources",
        4 => "✅ Good - Sufficient resources",
        3 => "⚠️ Moderate - Watch for bottlenecks",
        2 => "🔶 High Load - Optimization needed",
        1 => "🚨 Critical - Immediate action required",
        _ => "❓ Unknown state",
    };

    println!("  Assessment: {assessment}");

    Ok(())
}

fn format_metric_name(key: &str) -> String {
    key.replace("_ratio", "")
        .replace('_', " ")
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
