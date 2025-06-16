//! pwrzv - Linux System Power Reserve Meter
//!
//! A Linux system performance monitoring tool inspired by Rolls-Royce Power Reserve gauge design

use clap::{Arg, Command, ArgMatches};
use pwrzv::{PowerReserveCalculator, SystemMetrics, PowerReserveLevel, PwrzvError};
use serde_json;
use serde_yaml;
use std::process;

/// Application version
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Main function
fn main() {
    // Parse command line arguments
    let matches = build_cli().get_matches();
    
    // Execute different operations based on arguments
    if let Err(e) = run(matches) {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}

/// Build command line interface
fn build_cli() -> Command {
    Command::new("pwrzv")
        .version(VERSION)
        .about("A Rolls-Royce–inspired performance reserve meter for Linux systems")
        .long_about(
            "pwrzv monitors Linux system resources and provides a 0-5 score \
             representing available performance headroom, inspired by the \
             Power Reserve gauge in Rolls-Royce cars."
        )
        .arg(
            Arg::new("format")
                .short('f')
                .long("format")
                .value_name("FORMAT")
                .help("Output format")
                .value_parser(["text", "json", "yaml"])
                .default_value("text")
        )
        .arg(
            Arg::new("detailed")
                .short('d')
                .long("detailed")
                .help("Show detailed component scores")
                .action(clap::ArgAction::SetTrue)
        )
        .arg(
            Arg::new("quiet")
                .short('q')
                .long("quiet")
                .help("Suppress warnings")
                .action(clap::ArgAction::SetTrue)
        )
        .arg(
            Arg::new("check-platform")
                .long("check-platform")
                .help("Check platform compatibility and exit")
                .action(clap::ArgAction::SetTrue)
        )
}

/// Run main logic
fn run(matches: ArgMatches) -> Result<(), PwrzvError> {
    // Check platform compatibility
    if matches.get_flag("check-platform") {
        return check_platform_command();
    }
    
    // Create calculator
    let calculator = PowerReserveCalculator::new();
    
    // Collect system metrics
    let metrics = match calculator.collect_metrics() {
        Ok(metrics) => metrics,
        Err(e) => {
            if matches.get_flag("quiet") {
                // In quiet mode, use default metrics instead of exiting
                SystemMetrics::default()
            } else {
                return Err(e);
            }
        }
    };
    
    // Choose output method based on whether detailed information is needed
    if matches.get_flag("detailed") {
        let detailed_score = calculator.calculate_detailed_score(&metrics)?;
        output_detailed_result(&matches, &metrics, &detailed_score)?;
    } else {
        let score = calculator.calculate_power_reserve(&metrics)?;
        output_simple_result(&matches, &metrics, score)?;
    }
    
    Ok(())
}

/// Check platform compatibility command
fn check_platform_command() -> Result<(), PwrzvError> {
    match pwrzv::platform::check_platform() {
        Ok(()) => {
            println!("Platform check: OK ({})", pwrzv::platform::get_platform_name());
            println!("This system is supported by pwrzv.");
            Ok(())
        }
        Err(e) => {
            eprintln!("Platform check: FAILED");
            eprintln!("Current platform: {}", pwrzv::platform::get_platform_name());
            Err(e)
        }
    }
}

/// Output simple result
fn output_simple_result(
    matches: &ArgMatches,
    metrics: &SystemMetrics,
    score: u8,
) -> Result<(), PwrzvError> {
    let format = matches.get_one::<String>("format").unwrap();
    let level = PowerReserveLevel::from_score(score);
    
    match format.as_str() {
        "json" => {
            let output = serde_json::json!({
                "power_reserve_score": score,
                "level": level,
                "metrics": metrics
            });
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        }
        "yaml" => {
            let output = serde_json::json!({
                "power_reserve_score": score,
                "level": level,
                "metrics": metrics
            });
            println!("{}", serde_yaml::to_string(&output).unwrap());
        }
        _ => {
            // Default text format
            println!("System Metrics:");
            println!("  CPU Usage: {:.2}% (iowait: {:.2}%)", metrics.cpu_usage, metrics.cpu_iowait);
            println!("  Memory Available: {:.2}%", metrics.mem_available);
            println!("  Swap Usage: {:.2}%", metrics.swap_usage);
            println!("  Disk I/O Usage: {:.2}%", metrics.disk_usage);
            println!("  Network I/O Usage: {:.2}%", metrics.net_usage);
            println!("  File Descriptor Usage: {:.2}%", metrics.fd_usage);
            println!("Power Reserve Score: {} ({})", score, level);
        }
    }
    
    Ok(())
}

/// Output detailed result
fn output_detailed_result(
    matches: &ArgMatches,
    metrics: &SystemMetrics,
    detailed_score: &pwrzv::calculator::DetailedScore,
) -> Result<(), PwrzvError> {
    let format = matches.get_one::<String>("format").unwrap();
    
    match format.as_str() {
        "json" => {
            let output = serde_json::json!({
                "power_reserve_score": detailed_score.final_score,
                "level": detailed_score.level,
                "component_scores": detailed_score.component_scores,
                "bottleneck": detailed_score.bottleneck,
                "metrics": metrics
            });
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        }
        "yaml" => {
            let output = serde_json::json!({
                "power_reserve_score": detailed_score.final_score,
                "level": detailed_score.level,
                "component_scores": detailed_score.component_scores,
                "bottleneck": detailed_score.bottleneck,
                "metrics": metrics
            });
            println!("{}", serde_yaml::to_string(&output).unwrap());
        }
        _ => {
            // Default text format
            println!("=== System Power Reserve Analysis ===");
            println!();
            println!("System Metrics:");
            println!("  CPU Usage: {:.2}% (iowait: {:.2}%)", metrics.cpu_usage, metrics.cpu_iowait);
            println!("  Memory Available: {:.2}%", metrics.mem_available);
            println!("  Swap Usage: {:.2}%", metrics.swap_usage);
            println!("  Disk I/O Usage: {:.2}%", metrics.disk_usage);
            println!("  Network I/O Usage: {:.2}%", metrics.net_usage);
            println!("  File Descriptor Usage: {:.2}%", metrics.fd_usage);
            println!();
            println!("Component Scores (0-5):");
            println!("  CPU:              {}", detailed_score.component_scores.cpu);
            println!("  I/O Wait:         {}", detailed_score.component_scores.iowait);
            println!("  Memory:           {}", detailed_score.component_scores.memory);
            println!("  Swap:             {}", detailed_score.component_scores.swap);
            println!("  Disk I/O:         {}", detailed_score.component_scores.disk);
            println!("  Network I/O:      {}", detailed_score.component_scores.network);
            println!("  File Descriptors: {}", detailed_score.component_scores.file_descriptor);
            println!();
            println!("Overall Assessment:");
            println!("  Power Reserve Score: {} ({})", detailed_score.final_score, detailed_score.level);
            println!("  Bottlenecks: {}", detailed_score.bottleneck);
            println!();
            
            // Add recommendations
            if detailed_score.final_score <= 2 {
                println!("⚠️  Recommendations:");
                if detailed_score.bottleneck.contains("CPU") {
                    println!("   • Consider reducing CPU-intensive processes");
                }
                if detailed_score.bottleneck.contains("Memory") {
                    println!("   • Free up memory or add more RAM");
                }
                if detailed_score.bottleneck.contains("Disk") {
                    println!("   • Check for heavy disk I/O operations");
                }
                if detailed_score.bottleneck.contains("Network") {
                    println!("   • Monitor network bandwidth usage");
                }
                if detailed_score.bottleneck.contains("File Descriptors") {
                    println!("   • Check for file descriptor leaks");
                }
            } else if detailed_score.final_score >= 4 {
                println!("✅ System has ample performance headroom.");
            }
        }
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parsing() {
        let app = build_cli();
        
        // Test default parameters
        let matches = app.try_get_matches_from(vec!["pwrzv"]).unwrap();
        assert_eq!(matches.get_one::<String>("format").unwrap(), "text");
        assert!(!matches.get_flag("detailed"));
        assert!(!matches.get_flag("quiet"));
    }

    #[test]
    fn test_cli_with_args() {
        let app = build_cli();
        
        // Test with parameters
        let matches = app.try_get_matches_from(vec![
            "pwrzv", 
            "--format", "json", 
            "--detailed", 
            "--quiet"
        ]).unwrap();
        
        assert_eq!(matches.get_one::<String>("format").unwrap(), "json");
        assert!(matches.get_flag("detailed"));
        assert!(matches.get_flag("quiet"));
    }
}