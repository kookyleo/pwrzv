# pwrzv

[![CI](https://github.com/kookyleo/pwrzv/workflows/CI/badge.svg)](https://github.com/kookyleo/pwrzv/actions)
[![codecov](https://codecov.io/gh/kookyleo/pwrzv/graph/badge.svg?token=CqfBIyojDm)](https://codecov.io/gh/kookyleo/pwrzv)
[![Crates.io](https://img.shields.io/crates/v/pwrzv.svg)](https://crates.io/crates/pwrzv)
[![Documentation](https://docs.rs/pwrzv/badge.svg)](https://docs.rs/pwrzv)
[![License](https://img.shields.io/crates/l/pwrzv.svg)](https://github.com/kookyleo/pwrzv#license)

![pwrzv](./assets/Pwrzv-in-Rolls-Royce.jpg)

受劳斯莱斯汽车启发的 Linux 系统性能储备表。
简洁、优雅，专注于真正重要的事情：您的机器还剩多少性能可用。

## 🛠 什么是 pwrzv？

灵感来自劳斯莱斯汽车中的 Power Reserve 仪表——它显示还有多少发动机功率可用——pwrzv 将同样的理念带到 Linux 系统中。它不显示原始使用率，而是估算系统核心资源中还有多少余量。

它提供一个简单的 0-5 分评分，基于多项实时指标计算：

- **CPU 使用率和 I/O 等待**
- **内存可用性**
- **Swap 活动**
- **磁盘 I/O**
- **网络吞吐量**
- **文件描述符消耗**

所有输入都通过 sigmoid 函数加权和转换，以反映实际瓶颈，而不仅仅是原始数字。

## 🚦 示例输出

### 基本使用
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

### 详细分析
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

## 📦 安装

### 从源码安装
```bash
git clone https://github.com/kookyleo/pwrzv.git
cd pwrzv
cargo install --path .
```

### 使用 Cargo
```bash
cargo install pwrzv
```

## 🖥️ 平台支持

**pwrzv 仅支持 Linux 系统。** 其他平台将显示错误消息。

检查平台兼容性：
```bash
pwrzv --check-platform
```

## 🔧 使用方法

### 命令行界面

```bash
# 基本使用
pwrzv

# 详细组件分析
pwrzv --detailed

# JSON 输出
pwrzv --format json

# YAML 输出
pwrzv --format yaml

# 静默模式（抑制警告）
pwrzv --quiet

# 检查平台兼容性
pwrzv --check-platform
```

### 库使用

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

#### 详细分析

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

#### 自定义配置

```rust
use pwrzv::{PowerReserveCalculator, SigmoidConfig, PwrzvError};

fn main() -> Result<(), PwrzvError> {
    let mut config = SigmoidConfig::default();
    config.cpu_threshold = 0.8;  // 更敏感的 CPU 阈值
    
    let calculator = PowerReserveCalculator::with_config(config);
    let metrics = calculator.collect_metrics()?;
    let score = calculator.calculate_power_reserve(&metrics)?;
    println!("Power Reserve Score: {}", score);
    Ok(())
}
```

## 📊 评分系统

评分系统使用 sigmoid 函数将资源利用率映射到 0-5 分制：

- **5 (Excellent)**: 资源充足，系统运行流畅
- **4 (Good)**: 资源充沛，性能良好
- **3 (Moderate)**: 性能适中，资源充足
- **2 (Low)**: 资源受限，建议优化
- **0-1 (Critical)**: 系统高负载，需要立即关注

### 工作原理

1. **资源收集**: 从 `/proc` 文件系统收集指标
2. **标准化**: 将原始指标转换为 0-1 范围
3. **Sigmoid 变换**: 应用可配置的阈值和曲线
4. **瓶颈检测**: 取最小分数（最差资源）
5. **最终评分**: 映射到 0-5 范围并附带级别描述

## 🧪 设计理念

虽然大多数系统监控工具突出显示已使用的资源，但 pwrzv 告诉您还剩多少。这使其成为以下场景的有用工具：

- **精简仪表盘** - 单一指标概览
- **自动扩缩容决策** - 何时扩容/缩容
- **性能监控** - 主动资源管理
- **系统健康检查** - 快速状态评估

## 🔄 示例

运行包含的示例：

```bash
# 基本使用示例
cargo run --example basic_usage

# 带不同配置的详细分析
PWRZV_JSON_OUTPUT=1 cargo run --example detailed_analysis
```

## 🧪 测试

```bash
# 运行所有测试
cargo test

# 仅运行单元测试
cargo test --lib

# 运行文档测试
cargo test --doc

# 运行示例
cargo run --example basic_usage
```

## 📚 API 文档

生成并查看完整的 API 文档：

```bash
cargo doc --open
```

## 🤝 贡献

1. Fork 此仓库
2. 创建功能分支
3. 为新功能添加测试
4. 确保所有测试通过：`cargo test`
5. 提交 pull request

## 📄 许可证

此项目使用 MIT 许可证 - 详见 [LICENSE](LICENSE) 文件。

## 🙏 致谢

- 灵感来自劳斯莱斯汽车的 Power Reserve 仪表
- 使用 Rust 构建以确保性能和可靠性
- 感谢 Linux 内核提供全面的 `/proc` 指标