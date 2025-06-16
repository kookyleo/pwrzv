# pwrzv

[![CI](https://github.com/kookyleo/pwrzv/workflows/CI/badge.svg)](https://github.com/kookyleo/pwrzv/actions)
[![codecov](https://codecov.io/gh/kookyleo/pwrzv/graph/badge.svg?token=CqfBIyojDm)](https://codecov.io/gh/kookyleo/pwrzv)
[![Crates.io](https://img.shields.io/crates/v/pwrzv.svg)](https://crates.io/crates/pwrzv)
[![Documentation](https://docs.rs/pwrzv/badge.svg)](https://docs.rs/pwrzv)
[![License](https://img.shields.io/crates/l/pwrzv.svg)](https://github.com/kookyleo/pwrzv#license)


![pwrzv](./assets/Pwrzv-in-Rolls-Royce.jpg)

A Rolls-Royce–inspired performance reserve meter for Linux systems.
Elegant, minimal, and focused on what really matters: how much performance your machine has left to give.

## 🛠 What is pwrzv?

Inspired by the Power Reserve gauge in Rolls-Royce cars — which shows how much engine power is still available — pwrzv brings the same philosophy to Linux systems. Instead of showing raw usage, it estimates how much headroom remains in your system's core resources.

It provides a simple 0–5 score, calculated from multiple real-time metrics:

- **CPU usage and I/O wait**
- **Memory availability**
- **Swap activity**
- **Disk I/O**
- **Network throughput**
- **File descriptor consumption**

All inputs are weighted and transformed via sigmoid functions to reflect practical bottlenecks, not just raw numbers.

## 🚦 Example Output

### Basic Usage
```bash
$ pwrzv
System Metrics:
  CPU Usage: 12.34% (iowait: 0.00%)
  Memory Available: 78.50%
  Swap Usage: 0.00%
  Disk I/O Usage: 5.10%
  Network I/O Usage: 0.75%
  File Descriptor Usage: 3.42%
Power Reserve Score: 5 (Excellent - Abundant resources)
```

### Detailed Analysis
```bash
$ pwrzv --detailed
=== System Power Reserve Analysis ===

System Metrics:
  CPU Usage: 12.34% (iowait: 0.00%)
  Memory Available: 78.50%
  Swap Usage: 0.00%
  Disk I/O Usage: 5.10%
  Network I/O Usage: 0.75%
  File Descriptor Usage: 3.42%

Component Scores (0-5):
  CPU:              5
  I/O Wait:         5
  Memory:           4
  Swap:             5
  Disk I/O:         5
  Network I/O:      5
  File Descriptors: 5

Overall Assessment:
  Power Reserve Score: 4 (Good - Ample resources)
  Bottlenecks: None

✅ System has ample performance headroom.
```

## 📦 Installation

### From Source
```bash
git clone https://github.com/kookyleo/pwrzv.git
cd pwrzv
cargo install --path .
```

### Using Cargo
```bash
cargo install pwrzv
```

## 🖥️ Platform Support

**pwrzv only supports Linux systems.** Other platforms will display an error message.

Check platform compatibility:
```bash
pwrzv --check-platform
```

## 🔧 Usage

### Command Line Interface

```bash
# Basic usage
pwrzv

# Detailed component analysis
pwrzv --detailed

# JSON output
pwrzv --format json

# YAML output
pwrzv --format yaml

# Quiet mode (suppress warnings)
pwrzv --quiet

# Check platform compatibility
pwrzv --check-platform
```

### Library Usage

```rust
use pwrzv::{PowerReserveCalculator, PwrzvError};

fn main() -> Result<(), PwrzvError> {
    let calculator = PowerReserveCalculator::new();
    let metrics = calculator.collect_metrics()?;
    let score = calculator.calculate_power_reserve(&metrics)?;
    println!("Power Reserve Score: {}", score);
    Ok(())
}
```

#### Detailed Analysis

```rust
use pwrzv::{PowerReserveCalculator, PwrzvError};

fn main() -> Result<(), PwrzvError> {
    let calculator = PowerReserveCalculator::new();
    let metrics = calculator.collect_metrics()?;
    let detailed = calculator.calculate_detailed_score(&metrics)?;
    
    println!("Overall Score: {} ({})", detailed.final_score, detailed.level);
    println!("Bottlenecks: {}", detailed.bottleneck);
    println!("CPU Score: {}", detailed.component_scores.cpu);
    Ok(())
}
```

#### Custom Configuration

```rust
use pwrzv::{PowerReserveCalculator, SigmoidConfig, PwrzvError};

fn main() -> Result<(), PwrzvError> {
    let mut config = SigmoidConfig::default();
    config.cpu_threshold = 0.8;  // More sensitive CPU threshold
    
    let calculator = PowerReserveCalculator::with_config(config);
    let metrics = calculator.collect_metrics()?;
    let score = calculator.calculate_power_reserve(&metrics)?;
    println!("Power Reserve Score: {}", score);
    Ok(())
}
```

## 📊 Scoring System

The scoring system uses sigmoid functions to map resource utilization to a 0-5 scale:

- **5 (Excellent)**: Abundant resources, system running smoothly
- **4 (Good)**: Ample resources available, good performance
- **3 (Moderate)**: Adequate performance, resources sufficient
- **2 (Low)**: Resource constrained, consider optimization
- **0-1 (Critical)**: System under heavy load, immediate attention needed

### How It Works

1. **Resource Collection**: Gathers metrics from `/proc` filesystem
2. **Normalization**: Converts raw metrics to 0-1 scale
3. **Sigmoid Transformation**: Applies configurable thresholds and curves
4. **Bottleneck Detection**: Takes the minimum score (worst resource)
5. **Final Scoring**: Maps to 0-5 range with level descriptions

## 🧪 Philosophy

While most system monitors highlight how much is used, pwrzv tells you how much is left. This makes it a useful tool for:

- **Minimal dashboards** - Single metric overview
- **Autoscaling decisions** - When to scale up/down
- **Performance monitoring** - Proactive resource management
- **System health checks** - Quick status assessment

## 🔄 Examples

Run the included examples:

```bash
# Basic usage example
cargo run --example basic_usage

# Detailed analysis with different configurations
PWRZV_JSON_OUTPUT=1 cargo run --example detailed_analysis
```

## 🧪 Testing

```bash
# Run all tests
cargo test

# Run only unit tests
cargo test --lib

# Run documentation tests
cargo test --doc

# Run examples
cargo run --example basic_usage
```

## 📚 API Documentation

Generate and view the full API documentation:

```bash
cargo doc --open
```

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch
3. Add tests for new functionality
4. Ensure all tests pass: `cargo test`
5. Submit a pull request

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- Inspired by the Power Reserve gauge in Rolls-Royce automobiles
- Built with Rust for performance and reliability
- Thanks to the Linux kernel for providing comprehensive `/proc` metrics