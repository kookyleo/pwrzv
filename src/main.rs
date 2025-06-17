//! pwrzv - A cross-platform system power reserve monitoring tool
//!
//! Inspired by the Power Reserve gauge from Rolls-Royce cars

use std::env;
use std::process;

use clap::{Arg, ArgMatches, Command};
use pwrzv::{
    PowerReserveLevel, PwrzvError, check_platform, get_platform_name, get_power_reserve_level,
    get_power_reserve_level_with_details,
};
use std::collections::HashMap;

/// Application version
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Main function
#[tokio::main]
async fn main() {
    // Parse command line arguments
    let matches = build_cli().get_matches();

    // Execute different actions based on parameters
    if let Err(e) = run(matches).await {
        eprintln!("Error: {e}");
        process::exit(1);
    }
}

/// Build command line interface
fn build_cli() -> Command {
    Command::new("pwrzv")
        .version(VERSION)
        .about("A cross-platform system power reserve monitoring tool inspired by Rolls-Royce cars")
        .long_about(
            "pwrzv monitors system resources and provides power reserve level assessment, \
             inspired by the Power Reserve gauge from Rolls-Royce cars.\
             \n\nSupported platforms: Linux, macOS",
        )
        .arg(
            Arg::new("detailed")
                .short('d')
                .long("detailed")
                .value_name("FORMAT")
                .help("Show detailed component scores with optional format (text, json, yaml)")
                .value_parser(["text", "json", "yaml"])
                .num_args(0..=1)
                .default_missing_value("text"),
        )
}

/// Run main logic
async fn run(matches: ArgMatches) -> Result<(), PwrzvError> {
    // Check platform compatibility
    if let Err(e) = check_platform() {
        eprintln!("❌ Platform check failed: {e}");
        eprintln!("💡 Currently only Linux and macOS are supported");
        process::exit(1);
    }

    // Choose output method based on whether detailed information is needed
    if let Some(format) = matches.get_one::<String>("detailed") {
        let (level, details) = get_power_reserve_level_with_details().await?;
        output_detailed_result(format, PowerReserveLevel::try_from(level)?, &details)?;
    } else {
        let level = get_power_reserve_level().await?;
        println!("{}", level as u8);
    }

    Ok(())
}

/// Output detailed result
fn output_detailed_result(
    format: &str,
    level: PowerReserveLevel,
    details: &HashMap<String, f32>,
) -> Result<(), PwrzvError> {
    match format {
        "json" => {
            let output = serde_json::json!({
                "power_reserve_level": level as u8,
                "platform": get_platform_name(),
                "detailed_metrics": details
            });
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        }
        "yaml" => {
            let output = serde_json::json!({
                "power_reserve_level": level as u8,
                "platform": get_platform_name(),
                "detailed_metrics": details
            });
            println!("{}", serde_yaml::to_string(&output).unwrap());
        }
        _ => {
            // 默认文本格式
            println!("=== System Power Reserve Detailed Analysis ===");
            println!("Platform: {}", get_platform_name());
            println!("Power Reserve Level: {}", level as u8);
            println!();

            println!("=== System Metrics ===");
            print_metrics_section(details, "_ratio");

            println!("=== Pressure Scores ===");
            print_metrics_section(details, "_score");

            println!();
            match level {
                PowerReserveLevel::Abundant => println!("✅ System resources are abundant"),
                PowerReserveLevel::High => println!("✅ System resources are sufficient"),
                PowerReserveLevel::Medium => println!("⚠️ System resources are moderate"),
                PowerReserveLevel::Low => println!("⚠️ System resources are limited"),
                PowerReserveLevel::Critical => println!("🚨 System load is high"),
            }
        }
    }

    Ok(())
}

/// Print metrics section
fn print_metrics_section(details: &HashMap<String, f32>, suffix: &str) {
    let mut metrics: Vec<(String, f32)> = details
        .iter()
        .filter(|(key, _)| key.ends_with(suffix))
        .map(|(key, value)| (key.clone(), *value))
        .collect();

    metrics.sort_by(|a, b| a.0.cmp(&b.0));

    for (key, value) in metrics {
        let display_name = key.replace('_', " ").replace(suffix, "");
        if suffix == "_ratio" {
            println!("{display_name}: {value:.3} ({:.1}%)", value * 100.0);
        } else {
            println!("{display_name}: {value:.3}");
        }
    }
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parsing() {
        let app = build_cli();

        // Test default parameters (no detailed)
        let matches = app.try_get_matches_from(vec!["pwrzv"]).unwrap();
        assert!(matches.get_one::<String>("detailed").is_none());
    }

    #[test]
    fn test_cli_with_args() {
        // Test with detailed parameter
        let app = build_cli();
        let matches = app
            .try_get_matches_from(vec!["pwrzv", "--detailed", "json"])
            .unwrap();

        assert_eq!(matches.get_one::<String>("detailed").unwrap(), "json");

        // Test detailed without format (should default to text)
        let app = build_cli();
        let matches = app
            .try_get_matches_from(vec!["pwrzv", "--detailed"])
            .unwrap();

        assert_eq!(matches.get_one::<String>("detailed").unwrap(), "text");
    }
}
