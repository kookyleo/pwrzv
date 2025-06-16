//! Detailed analysis example
//!
//! Demonstrates how to use the pwrzv library for detailed system performance analysis and custom configuration

use pwrzv::{PowerReserveCalculator, PwrzvError, SigmoidConfig, platform};

fn main() -> Result<(), PwrzvError> {
    println!("=== pwrzv Detailed Analysis Example ===\n");

    // Check platform compatibility first
    if let Err(e) = platform::check_platform() {
        eprintln!("❌ Platform check failed: {e}");
        eprintln!("pwrzv currently only supports Linux systems.");
        eprintln!("This example will exit gracefully without running the analysis.");
        return Ok(()); // Return Ok to avoid non-zero exit code
    }

    println!("✅ Platform check passed!\n");

    // Use default configuration
    println!("🔧 Analyzing with default configuration...");
    if let Err(e) = analyze_with_config(SigmoidConfig::default(), "Default Configuration") {
        eprintln!("Analysis failed: {e}");
        return Ok(());
    }

    println!("\n{}\n", "=".repeat(50));

    // Use custom strict configuration
    println!("🔧 Analyzing with custom strict configuration...");
    let strict_config = SigmoidConfig {
        cpu_threshold: 0.7,    // Stricter CPU threshold (70%)
        cpu_steepness: 15.0,   // Steeper response curve
        iowait_threshold: 0.3, // Stricter I/O wait threshold
        iowait_steepness: 15.0,
        memory_threshold: 0.9, // Stricter memory threshold
        memory_steepness: 15.0,
        swap_threshold: 0.3, // Stricter Swap threshold
        swap_steepness: 15.0,
        disk_threshold: 0.9, // Stricter disk threshold
        disk_steepness: 15.0,
        network_threshold: 0.8, // Stricter network threshold
        network_steepness: 15.0,
        fd_threshold: 0.8, // Stricter file descriptor threshold
        fd_steepness: 15.0,
    };
    if let Err(e) = analyze_with_config(strict_config, "Strict Configuration") {
        eprintln!("Analysis failed: {e}");
        return Ok(());
    }

    println!("\n{}\n", "=".repeat(50));

    // Use lenient configuration
    println!("🔧 Analyzing with lenient configuration...");
    let lenient_config = SigmoidConfig {
        cpu_threshold: 0.95,   // Lenient CPU threshold (95%)
        cpu_steepness: 5.0,    // Gentle response curve
        iowait_threshold: 0.7, // Lenient I/O wait threshold
        iowait_steepness: 5.0,
        memory_threshold: 0.98, // Lenient memory threshold
        memory_steepness: 5.0,
        swap_threshold: 0.7, // Lenient Swap threshold
        swap_steepness: 5.0,
        disk_threshold: 0.98, // Lenient disk threshold
        disk_steepness: 5.0,
        network_threshold: 0.95, // Lenient network threshold
        network_steepness: 5.0,
        fd_threshold: 0.95, // Lenient file descriptor threshold
        fd_steepness: 5.0,
    };
    if let Err(e) = analyze_with_config(lenient_config, "Lenient Configuration") {
        eprintln!("Analysis failed: {e}");
        return Ok(());
    }

    Ok(())
}

fn analyze_with_config(config: SigmoidConfig, config_name: &str) -> Result<(), PwrzvError> {
    let calculator = PowerReserveCalculator::with_config(config);
    let metrics = calculator.collect_metrics()?;
    let detailed_score = calculator.calculate_detailed_score(&metrics)?;

    println!("📊 {config_name} Analysis Results:");
    println!(
        "  Final Score: {} ({})",
        detailed_score.final_score, detailed_score.level
    );
    println!("  Main Bottleneck: {}", detailed_score.bottleneck);
    println!("  Component Scores:");
    println!(
        "    CPU:              {} / 5",
        detailed_score.component_scores.cpu
    );
    println!(
        "    I/O Wait:         {} / 5",
        detailed_score.component_scores.iowait
    );
    println!(
        "    Memory:           {} / 5",
        detailed_score.component_scores.memory
    );
    println!(
        "    Swap:             {} / 5",
        detailed_score.component_scores.swap
    );
    println!(
        "    Disk I/O:         {} / 5",
        detailed_score.component_scores.disk
    );
    println!(
        "    Network I/O:      {} / 5",
        detailed_score.component_scores.network
    );
    println!(
        "    File Descriptors: {} / 5",
        detailed_score.component_scores.file_descriptor
    );
    println!();

    // Output detailed information in JSON format
    if std::env::var("PWRZV_JSON_OUTPUT").is_ok() {
        println!("\n🔍 JSON Format Output:");
        let json_output = serde_json::json!({
            "config_name": config_name,
            "score": detailed_score.final_score,
            "level": detailed_score.level,
            "component_scores": detailed_score.component_scores,
            "bottleneck": detailed_score.bottleneck,
            "raw_metrics": metrics,
            "config": calculator.get_config()
        });
        println!("{}", serde_json::to_string_pretty(&json_output).unwrap());
    }

    // Performance recommendations
    print_recommendations(&detailed_score);

    Ok(())
}

fn print_recommendations(detailed_score: &pwrzv::DetailedScore) {
    println!("\n💡 Performance Recommendations:");

    if detailed_score.final_score <= 2 {
        println!("   🚨 System is under high load, recommendations:");

        if detailed_score.component_scores.cpu <= 2 {
            println!("      • Check and optimize CPU-intensive processes");
            println!("      • Consider increasing CPU cores or upgrading the processor");
        }

        if detailed_score.component_scores.iowait <= 2 {
            println!("      • Check disk I/O bottlenecks, consider using SSD");
            println!("      • Optimize database queries and file access patterns");
        }

        if detailed_score.component_scores.memory <= 2 {
            println!("      • Free up unnecessary memory usage");
            println!("      • Consider increasing system memory");
        }

        if detailed_score.component_scores.swap <= 2 {
            println!("      • Reduce Swap usage, increase physical memory");
            println!("      • Adjust vm.swappiness parameter");
        }

        if detailed_score.component_scores.disk <= 2 {
            println!("      • Check disk I/O performance bottlenecks");
            println!("      • Consider using faster storage devices");
        }

        if detailed_score.component_scores.network <= 2 {
            println!("      • Check network bandwidth usage");
            println!("      • Optimize network configuration and connections");
        }

        if detailed_score.component_scores.file_descriptor <= 2 {
            println!("      • Check for file descriptor leaks");
            println!("      • Adjust system file descriptor limits");
        }
    } else if detailed_score.final_score >= 4 {
        println!("   ✅ System performance is good, continue to maintain:");
        println!("      • Regularly monitor system resource usage");
        println!("      • Establish performance baselines and alerting mechanisms");
    } else {
        println!("   ⚠️  System performance is moderate, consider:");
        println!("      • Monitor resource usage trends");
        println!("      • Plan ahead for system scaling");
    }
}
