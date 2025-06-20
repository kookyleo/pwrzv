# pwrzv

[![CI](https://github.com/kookyleo/pwrzv/workflows/CI/badge.svg)](https://github.com/kookyleo/pwrzv/actions)
[![codecov](https://codecov.io/gh/kookyleo/pwrzv/graph/badge.svg?token=CqfBIyojDm)](https://codecov.io/gh/kookyleo/pwrzv)
[![Crates.io](https://img.shields.io/crates/v/pwrzv.svg)](https://crates.io/crates/pwrzv)
[![Documentation](https://docs.rs/pwrzv/badge.svg)](https://docs.rs/pwrzv)
[![License](https://img.shields.io/crates/l/pwrzv.svg)](https://github.com/kookyleo/pwrzv#license)


![pwrzv](./assets/Pwrzv-in-Rolls-Royce.jpg)

A Rolls-Royce–inspired performance reserve meter for Linux and macOS systems.
Elegant, minimal, and focused on what really matters: how much performance your machine has left to give.

## ⚠️ Beta Stage Notice

**This library is currently in Beta stage and not yet fully mature.**

- Parameter tuning may not be precise enough and might need adjustment for specific systems
- API and behavior may change in future versions
- We welcome your feedback and contributions through [Issues](https://github.com/kookyleo/pwrzv/issues) and [Pull Requests](https://github.com/kookyleo/pwrzv/pulls)
- Please test thoroughly before using in production environments

Your feedback is crucial for improving this project!

## 🛠 What is pwrzv?

Inspired by the Power Reserve gauge in Rolls-Royce cars — which shows how much engine power is still available — pwrzv brings the same philosophy to Unix-like systems. Instead of showing raw usage, it estimates how much headroom remains in your system's core resources.

It provides a simple 0–5 score, calculated from multiple real-time metrics:

- **CPU usage and I/O wait**
- **Memory availability**
- **Swap activity**
- **Disk I/O**
- **Network throughput and packet loss**
- **File descriptor consumption**

All inputs are weighted and transformed via sigmoid functions to reflect practical bottlenecks, not just raw numbers.

## 🚦 Example Output

### Basic Usage
```bash
$ pwrzv
2
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

**pwrzv supports Linux and macOS systems** for now. Other platforms will display an error message.

### Platform-Specific Implementation

- **Linux**: Uses `/proc` filesystem for direct system metrics access
- **macOS**: Uses system commands (`sysctl`, `vm_stat`, `iostat`, etc.) for metrics collection

## 🔧 Usage

### Command Line Interface

```bash
# Basic usage (simplest numeric output)
pwrzv

# Detailed component analysis (default text format)
pwrzv --detailed

# Detailed analysis with JSON output
pwrzv --detailed json

# Detailed analysis with YAML output
pwrzv --detailed yaml
```

### Library Usage

```rust
use pwrzv::{get_power_reserve_level_direct, PwrzvError};

#[tokio::main]
async fn main() -> Result<(), PwrzvError> {
    let level = get_power_reserve_level_direct().await?;
    println!("Power Reserve Level: {}/5", level);
    Ok(())
}
```

#### Detailed Analysis

```rust
use pwrzv::{get_power_reserve_level_with_details_direct, PowerReserveLevel, PwrzvError};

#[tokio::main]
async fn main() -> Result<(), PwrzvError> {
    let (level, details) = get_power_reserve_level_with_details_direct().await?;
    let power_level = PowerReserveLevel::try_from(level)?;
    
    println!("Power Reserve: {} ({})", level, power_level);
    println!("Detailed metrics:");
    for (metric, value) in details {
        println!("  {}: {:.3}", metric, value);
    }
    Ok(())
}
```

#### Platform Support Check

```rust
use pwrzv::{check_platform, get_platform_name, PwrzvError};

fn main() -> Result<(), PwrzvError> {
    println!("Running on: {}", get_platform_name());
    
    match check_platform() {
        Ok(()) => println!("Platform is supported!"),
        Err(e) => eprintln!("Platform not supported: {}", e),
    }
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

1. **Resource Collection**: Gathers metrics from `/proc` filesystem or system commands
2. **Normalization**: Converts raw metrics to 0-1 scale
3. **Sigmoid Transformation**: Applies configurable thresholds and curves
4. **Bottleneck Detection**: Takes the minimum score (worst resource)
5. **Final Scoring**: Maps to 0-5 range with level descriptions

## 🧮 Numerical Calculation Methods

pwrzv employs sophisticated mathematical algorithms to convert raw system metrics into meaningful power reserve scores:

### Sigmoid Function Transformation

The core calculation uses the **sigmoid function** to transform linear resource utilization into a smooth 0-1 scale:

```
f(x) = 1 / (1 + e^(-k * (x - x₀)))
```

Where:
- **x**: Raw metric value (0-1 range after normalization)
- **x₀ (midpoint)**: The threshold where the metric begins significantly impacting the score
- **k (steepness)**: Controls the curve's steepness; higher values create more dramatic score changes

### Multi-Stage Processing Pipeline

1. **Raw Data Collection**: Platform-specific metric gathering (Linux: `/proc` filesystem, macOS: system commands)
2. **Normalization**: Convert raw values to 0-1 scale for consistent processing
3. **Sigmoid Transformation**: Apply individual sigmoid curves to each metric based on its characteristics
4. **Bottleneck Analysis**: Identify the worst-performing resource (minimum score)
5. **Final Mapping**: Transform the 0-1 result to the 0-5 Power Reserve scale

### Adaptive Thresholds

Each metric uses carefully tuned parameters:
- **CPU metrics**: Balanced sensitivity for both usage spikes and sustained load
- **Memory metrics**: Higher thresholds to account for normal OS caching behavior  
- **I/O metrics**: Moderate sensitivity to distinguish between light and heavy workloads
- **Network metrics**: Separate handling for bandwidth utilization vs. packet loss sensitivity

This mathematical approach ensures that pwrzv provides intuitive, actionable scores that reflect real system performance bottlenecks rather than raw utilization percentages.

## ⚙️ Environment Variable Configuration

pwrzv supports customizing sigmoid function parameters for each metric via environment variables to adapt to different system characteristics and use cases.

### macOS Platform Environment Variables

```bash
# CPU usage configuration (default: midpoint=0.60, steepness=8.0)
export PWRZV_MACOS_CPU_USAGE_MIDPOINT=0.60
export PWRZV_MACOS_CPU_USAGE_STEEPNESS=8.0

# CPU load configuration (default: midpoint=1.2, steepness=5.0)
export PWRZV_MACOS_CPU_LOAD_MIDPOINT=1.2
export PWRZV_MACOS_CPU_LOAD_STEEPNESS=5.0

# Memory usage configuration (default: midpoint=0.85, steepness=20.0)
export PWRZV_MACOS_MEMORY_USAGE_MIDPOINT=0.85
export PWRZV_MACOS_MEMORY_USAGE_STEEPNESS=20.0

# Memory compression configuration (default: midpoint=0.60, steepness=15.0)
export PWRZV_MACOS_MEMORY_COMPRESSED_MIDPOINT=0.60
export PWRZV_MACOS_MEMORY_COMPRESSED_STEEPNESS=15.0

# Disk I/O configuration (default: midpoint=0.70, steepness=10.0)
export PWRZV_MACOS_DISK_IO_MIDPOINT=0.70
export PWRZV_MACOS_DISK_IO_STEEPNESS=10.0

# Network bandwidth configuration (default: midpoint=0.80, steepness=6.0)
export PWRZV_MACOS_NETWORK_MIDPOINT=0.80
export PWRZV_MACOS_NETWORK_STEEPNESS=6.0

# Network packet loss configuration (default: midpoint=0.01, steepness=50.0)
export PWRZV_MACOS_NETWORK_DROPPED_MIDPOINT=0.01
export PWRZV_MACOS_NETWORK_DROPPED_STEEPNESS=50.0

# File descriptor configuration (default: midpoint=0.90, steepness=30.0)
export PWRZV_MACOS_FD_MIDPOINT=0.90
export PWRZV_MACOS_FD_STEEPNESS=30.0

# Process count configuration (default: midpoint=0.80, steepness=12.0)
export PWRZV_MACOS_PROCESS_MIDPOINT=0.80
export PWRZV_MACOS_PROCESS_STEEPNESS=12.0
```

### Linux Platform Environment Variables

```bash
# CPU usage configuration (default: midpoint=0.65, steepness=8.0)
export PWRZV_LINUX_CPU_USAGE_MIDPOINT=0.65
export PWRZV_LINUX_CPU_USAGE_STEEPNESS=8.0

# CPU I/O wait configuration (default: midpoint=0.20, steepness=20.0)
export PWRZV_LINUX_CPU_IOWAIT_MIDPOINT=0.20
export PWRZV_LINUX_CPU_IOWAIT_STEEPNESS=20.0

# CPU load configuration (default: midpoint=1.2, steepness=5.0)
export PWRZV_LINUX_CPU_LOAD_MIDPOINT=1.2
export PWRZV_LINUX_CPU_LOAD_STEEPNESS=5.0

# Memory usage configuration (default: midpoint=0.85, steepness=18.0)
export PWRZV_LINUX_MEMORY_USAGE_MIDPOINT=0.85
export PWRZV_LINUX_MEMORY_USAGE_STEEPNESS=18.0

# Memory pressure configuration (default: midpoint=0.30, steepness=12.0)
export PWRZV_LINUX_MEMORY_PRESSURE_MIDPOINT=0.30
export PWRZV_LINUX_MEMORY_PRESSURE_STEEPNESS=12.0

# Disk I/O configuration (default: midpoint=0.70, steepness=10.0)
export PWRZV_LINUX_DISK_IO_MIDPOINT=0.70
export PWRZV_LINUX_DISK_IO_STEEPNESS=10.0

# Network bandwidth configuration (default: midpoint=0.80, steepness=6.0)
export PWRZV_LINUX_NETWORK_MIDPOINT=0.80
export PWRZV_LINUX_NETWORK_STEEPNESS=6.0

# Network packet loss configuration (default: midpoint=0.01, steepness=50.0)
export PWRZV_LINUX_NETWORK_DROPPED_MIDPOINT=0.01
export PWRZV_LINUX_NETWORK_DROPPED_STEEPNESS=50.0

# File descriptor configuration (default: midpoint=0.90, steepness=25.0)
export PWRZV_LINUX_FD_MIDPOINT=0.90
export PWRZV_LINUX_FD_STEEPNESS=25.0

# Process count configuration (default: midpoint=0.80, steepness=12.0)
export PWRZV_LINUX_PROCESS_MIDPOINT=0.80
export PWRZV_LINUX_PROCESS_STEEPNESS=12.0
```

### Parameter Meanings

- **midpoint**: Sigmoid function midpoint value, representing the threshold where this metric starts significantly affecting the score
- **steepness**: Sigmoid function steepness, higher values make the curve steeper and score changes more dramatic

### Usage Example

```bash
# Adjust CPU threshold for high-performance server
export PWRZV_LINUX_CPU_USAGE_MIDPOINT=0.80
export PWRZV_LINUX_CPU_USAGE_STEEPNESS=15.0

# Run pwrzv
pwrzv --detailed
```

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

# Detailed metrics analysis example
cargo run --example detailed_metrics
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