# Metrics Module

This module provides system metrics collection functionality with cross-platform support through trait abstraction.

## Module Structure

```
src/metrics/
├── mod.rs      # Core module with SystemMetrics struct and MetricsCollector trait
├── linux.rs   # Linux platform implementation (uses /proc filesystem)
├── macos.rs   # macOS platform implementation (uses system commands)
└── default.rs # Default implementation for unsupported platforms
```

## Architecture

### Core Components

- **`SystemMetrics`**: Data structure containing all collected system metrics
- **`MetricsCollector`** trait: Defines the interface for platform-specific collectors
- **Factory function**: `create_metrics_collector()` returns appropriate collector based on platform

### Platform Implementations

#### Linux (`linux.rs`)
- Uses `/proc` filesystem for direct access to system information
- Metrics sources:
  - CPU: `/proc/stat`
  - Memory: `/proc/meminfo`
  - Disk I/O: `/proc/diskstats`
  - Network: `/proc/net/dev`
  - File descriptors: `/proc/sys/fs/file-nr`

#### macOS (`macos.rs`)
- Uses system commands for metrics collection
- Metrics sources:
  - CPU: `top` command
  - Memory: `vm_stat` and `sysctl`
  - Disk I/O: `iostat`
  - Network: `netstat`
  - File descriptors: `sysctl` (kern.maxfilesperproc, kern.openfiles)

#### Default (`default.rs`)
- Fallback implementation for unsupported platforms
- Returns errors for all metrics collection attempts

## Usage

```rust
use pwrzv::metrics::{SystemMetrics, create_metrics_collector};

// Collect all metrics (platform-independent)
let metrics = SystemMetrics::collect()?;

// Or use a specific collector
let collector = create_metrics_collector();
let (cpu_usage, cpu_iowait) = collector.collect_cpu_stats()?;
```

## Testing

Each platform implementation includes comprehensive tests:
- Unit tests for individual functions
- Integration tests for full collection workflow
- Error handling tests for edge cases

## Adding New Platforms

To add support for a new platform:

1. Create a new file `src/metrics/your_platform.rs`
2. Implement the `MetricsCollector` trait
3. Add platform detection in `mod.rs`
4. Update the factory function `create_metrics_collector()`
5. Add appropriate tests

## Design Principles

- **Platform Independence**: Upper layers use the same interface regardless of platform
- **Graceful Degradation**: Failed metrics collection logs warnings but doesn't crash
- **Zero-Sized Types**: Collectors are stateless for optimal performance
- **Error Transparency**: Platform-specific errors are properly propagated 