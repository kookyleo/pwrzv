//! pwrzv - A cross-platform system power reserve monitoring tool
//!
//! Inspired by the Power Reserve gauge from Rolls-Royce cars

use std::env;
use std::process;

use clap::{Arg, ArgMatches, Command};
use pwrzv::{
    PwrzvError, check_platform, get_platform_name, get_power_reserve_level_direct,
    get_power_reserve_level_with_details_direct,
};
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;

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
///
/// Creates the CLI structure using clap with support for detailed output
/// with optional format specification.
///
/// # Returns
///
/// A configured `Command` instance ready to parse command line arguments
///
/// # CLI Options
///
/// - `--detailed [FORMAT]`: Show detailed component scores
///   - `FORMAT` can be: `text` (default), `json`, or `yaml`
///   - If no format is specified, defaults to `text`
/// - `--interval/-t SECONDS`: Set output refresh interval (default: 3 seconds)
/// - `--once`: Show output once and exit
fn build_cli() -> Command {
    Command::new("pwrzv")
        .version(VERSION)
        .about("A cross-platform system power reserve monitoring tool inspired by Rolls-Royce cars")
        .long_about(
            "pwrzv monitors system resources and provides power reserve level assessment, \
             inspired by the Power Reserve gauge from Rolls-Royce cars.\
             \n\nSupported platforms: Linux, macOS\
             \n\nBy default, pwrzv runs continuously and updates output every 3 seconds.\
             \nUse --once for single-shot output.",
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
        .arg(
            Arg::new("interval")
                .short('t')
                .long("interval")
                .value_name("SECONDS")
                .help("Set output refresh interval in seconds (default: 3)")
                .value_parser(clap::value_parser!(u64)),
        )
        .arg(
            Arg::new("once")
                .long("once")
                .help("Show output once and exit")
                .action(clap::ArgAction::SetTrue),
        )
}

/// Run main logic
///
/// Handles the main application flow including platform detection,
/// argument parsing, and output formatting.
///
/// # Arguments
///
/// * `matches` - Parsed command line arguments from clap
///
/// # Returns
///
/// * `Ok(())` - If execution completes successfully
/// * `Err(PwrzvError)` - If any error occurs during execution
///
/// # Behavior
///
/// 1. Checks platform compatibility (Linux/macOS only)
/// 2. If `--once` flag: runs single-shot mode
/// 3. If continuous mode (default):
///    - Outputs results every specified interval
///    - Collects metrics in real-time for each output
///
/// # Platform Support
///
/// - **Linux**: Full support via `/proc` filesystem
/// - **macOS**: Full support via system commands
/// - **Other platforms**: Returns error with helpful message
async fn run(matches: ArgMatches) -> Result<(), PwrzvError> {
    // Check platform compatibility
    if let Err(e) = check_platform() {
        eprintln!("❌ Platform check failed: {e}");
        eprintln!("💡 Currently only Linux and macOS are supported");
        process::exit(1);
    }

    println!("✅ Platform check passed for: {}", get_platform_name());

    // Check if single-shot mode is requested
    if matches.get_flag("once") {
        // Choose output method based on whether detailed information is needed
        if let Some(format) = matches.get_one::<String>("detailed") {
            let (level, details) = get_power_reserve_level_with_details_direct().await?;
            output_detailed_result(format, level, &details)?;
        } else {
            let level = get_power_reserve_level_direct().await?;
            println!("{level:.2}");
        }
        return Ok(());
    }

    // Continuous monitoring mode
    let output_interval = matches.get_one::<u64>("interval").copied().unwrap_or(3); // Default 3 second

    eprintln!("🔄 Starting continuous monitoring (interval: {output_interval}s)");
    eprintln!("💡 Press Ctrl+C to stop");
    eprintln!();

    loop {
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");

        // Clear screen for better readability if detailed mode
        if matches.get_one::<String>("detailed").is_some() {
            print!("\x1b[2J\x1b[H"); // Clear screen and move cursor to top
            println!("{now}"); // Show current time
        }

        // Collect and output current status
        if let Some(format) = matches.get_one::<String>("detailed") {
            match get_power_reserve_level_with_details_direct().await {
                Ok((level, details)) => {
                    output_detailed_result(format, level, &details)?;
                }
                Err(e) => {
                    eprintln!("{now} ❌ Failed to collect metrics: {e}");
                }
            }
        } else {
            match get_power_reserve_level_direct().await {
                Ok(level) => {
                    println!("{now} Power Reserve: {level:.2}");
                }
                Err(e) => {
                    eprintln!("{now} ❌ Failed to collect metrics: {e}");
                }
            }
        }

        // Wait for next output interval
        sleep(Duration::from_secs(output_interval)).await;
    }
}

/// Output detailed result
///
/// Formats and outputs detailed system metrics in the specified format.
///
/// # Arguments
///
/// * `format` - Output format: "text", "json", or "yaml"
/// * `level` - Power reserve level as f32 (1.0-5.0)
/// * `details` - HashMap containing detailed metric names and values
///
/// # Returns
///
/// * `Ok(())` - If output is successful
/// * `Err(PwrzvError)` - If formatting or output fails
///
/// # Output Formats
///
/// ## Text Format
/// Human-readable format with sections for metrics and scores,
/// including interpretation and recommendations.
///
/// ## JSON Format
/// Machine-readable JSON with platform info, level, and all metrics.
///
/// ## YAML Format
/// YAML format with structured data for configuration management.
fn output_detailed_result(
    format: &str,
    level: f32,
    details: &HashMap<String, f32>,
) -> Result<(), PwrzvError> {
    match format {
        "json" => {
            let json_output = serde_json::json!({
                "platform": get_platform_name(),
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "power_reserve_level": level,
                "level_description": format_level_description(level),
                "metrics": details,
                "total_metrics": details.len()
            });
            println!("{}", serde_json::to_string_pretty(&json_output).unwrap());
        }
        "yaml" => {
            let yaml_data = serde_json::json!({
                "platform": get_platform_name(),
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "power_reserve_level": level,
                "level_description": format_level_description(level),
                "metrics": details,
                "total_metrics": details.len()
            });
            println!("{}", serde_yaml::to_string(&yaml_data).unwrap());
        }
        // Default to text format for any other cases
        _ => {
            println!("═══════════════════════════════════════════════════════════");
            println!("🔋 Power Reserve Analysis");
            println!("═══════════════════════════════════════════════════════════");
            println!();

            // Overall level with visual indicator
            println!(
                "📊 Overall Power Reserve: {:.2} {}",
                level,
                format_level_emoji(level)
            );
            println!("   Status: {}", format_level_description(level));
            println!();

            if !details.is_empty() {
                print_metrics_section(details);
            }

            println!("───────────────────────────────────────────────────────────");
            println!("💡 Interpretation:");
            println!("   • Scores range from 1.0 (Critical) to 5.0 (Abundant)");
            println!("   • Overall level is determined by the lowest component score");
            println!("   • Higher precision allows for more accurate assessment");

            if level < 2.0 {
                println!("   ⚠️  Consider optimizing system resources");
            }
        }
    }
    Ok(())
}

/// Print metrics section for text format
fn print_metrics_section(details: &HashMap<String, f32>) {
    println!("📈 Component Metrics:");

    let mut sorted_metrics: Vec<_> = details.iter().collect();
    sorted_metrics.sort_by(|a, b| a.1.partial_cmp(b.1).unwrap());

    for (key, value) in sorted_metrics {
        let status_emoji = format_level_emoji(*value);
        println!("   {key:<50}: {value:.3} {status_emoji}");
    }
    println!();
}

/// Format level description based on numeric value
fn format_level_description(level: f32) -> &'static str {
    if level >= 4.0 {
        "Abundant - Excellent performance"
    } else if level >= 3.0 {
        "High - Good performance"
    } else if level >= 2.0 {
        "Medium - Normal performance"
    } else if level >= 1.0 {
        "Low - Degraded performance"
    } else {
        "Critical - Poor performance"
    }
}

/// Get emoji representation of level
fn format_level_emoji(level: f32) -> &'static str {
    if level >= 4.0 {
        "🌟"
    } else if level >= 3.0 {
        "👌"
    } else if level >= 2.0 {
        "⚠️"
    } else if level >= 1.0 {
        "🔶"
    } else {
        "🚨"
    }
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

    #[test]
    fn test_cli_interval_parsing() {
        let app = build_cli();
        let matches = app
            .try_get_matches_from(vec!["pwrzv", "--interval", "10"])
            .unwrap();

        assert_eq!(matches.get_one::<u64>("interval").unwrap(), &10);
    }

    #[test]
    fn test_cli_once_flag() {
        let app = build_cli();
        let matches = app.try_get_matches_from(vec!["pwrzv", "--once"]).unwrap();

        assert!(matches.get_flag("once"));
    }
}
