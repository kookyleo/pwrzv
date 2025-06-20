# pwrzv

[![CI](https://github.com/kookyleo/pwrzv/workflows/CI/badge.svg)](https://github.com/kookyleo/pwrzv/actions)
[![codecov](https://codecov.io/gh/kookyleo/pwrzv/graph/badge.svg?token=CqfBIyojDm)](https://codecov.io/gh/kookyleo/pwrzv)
[![Crates.io](https://img.shields.io/crates/v/pwrzv.svg)](https://crates.io/crates/pwrzv)
[![Documentation](https://docs.rs/pwrzv/badge.svg)](https://docs.rs/pwrzv)
[![License](https://img.shields.io/crates/l/pwrzv.svg)](https://github.com/kookyleo/pwrzv#license)

![pwrzv](./assets/Pwrzv-in-Rolls-Royce.jpg)

一个受劳斯莱斯汽车仪表盘启发的 Linux 和 macOS 系统性能余量监控工具。
优雅、简洁，专注于真正重要的事情：你的系统还有多少性能余量可供使用。

## ⚠️ Beta 阶段声明

**本库目前处于 Beta 阶段，尚未完全成熟。**

- 参数调校可能不够精确，可能需要根据具体系统调整
- API 和行为可能在后续版本中发生变化
- 欢迎通过 [Issues](https://github.com/kookyleo/pwrzv/issues) 和 [Pull Requests](https://github.com/kookyleo/pwrzv/pulls) 贡献你的反馈和改进
- 生产环境使用前请充分测试验证

你的反馈对完善这个项目非常重要！

## 🛠 什么是 pwrzv？

受劳斯莱斯汽车的动力储备表启发——它显示引擎还有多少动力可用——pwrzv 将这一理念带到了类 Unix 系统。它不显示原始使用率，而是估算系统核心资源的剩余空间。

它提供一个简单的 0-5 分评分，基于多项实时指标计算：

- **CPU 使用率和 I/O 等待**
- **内存可用性**
- **Swap 活动**
- **磁盘 I/O**
- **网络吞吐量和丢包率**
- **文件描述符消耗**

所有输入都通过 sigmoid 函数加权和转换，以反映实际瓶颈，而不仅仅是原始数字。

## 🚦 示例输出

### 基本使用
```bash
$ pwrzv
2
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

**pwrzv 暂只支持 Linux 和 macOS 系统。**其他平台将显示错误信息。

### 平台特定实现

- **Linux**: 使用 `/proc` 文件系统直接访问系统指标
- **macOS**: 使用系统命令（`sysctl`、`vm_stat`、`iostat` 等）收集指标

## 🔧 使用方法

### 命令行界面

```bash
# 基本使用（最简洁的数值输出）
pwrzv

# 详细组件分析（默认文本格式）
pwrzv --detailed

# 详细分析 JSON 输出
pwrzv --detailed json

# 详细分析 YAML 输出
pwrzv --detailed yaml
```

### 库使用

```rust
use pwrzv::{get_power_reserve_level_direct, PwrzvError};

#[tokio::main]
async fn main() -> Result<(), PwrzvError> {
    let level = get_power_reserve_level_direct().await?;
    println!("动力余量等级: {}/5", level);
    Ok(())
}
```

#### 详细分析

```rust
use pwrzv::{get_power_reserve_level_with_details_direct, PowerReserveLevel, PwrzvError};

#[tokio::main]
async fn main() -> Result<(), PwrzvError> {
    let (level, details) = get_power_reserve_level_with_details_direct().await?;
    let power_level = PowerReserveLevel::try_from(level)?;
    
    println!("动力余量: {} ({})", level, power_level);
    println!("详细指标:");
    for (metric, value) in details {
        println!("  {}: {:.3}", metric, value);
    }
    Ok(())
}
```

#### 平台支持检查

```rust
use pwrzv::{check_platform, get_platform_name, PwrzvError};

fn main() -> Result<(), PwrzvError> {
    println!("运行平台: {}", get_platform_name());
    
    match check_platform() {
        Ok(()) => println!("平台支持!"),
        Err(e) => eprintln!("平台不支持: {}", e),
    }
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

1. **资源收集**: 平台特定的指标收集（Linux: `/proc` 文件系统, macOS: 系统命令）
2. **标准化**: 将原始指标转换为 0-1 范围
3. **Sigmoid 变换**: 应用可配置的阈值和曲线
4. **瓶颈检测**: 取最小分数（最差资源）
5. **最终评分**: 映射到 0-5 范围并附带级别描述

## 🧮 数值计算方法

pwrzv 采用先进的数学算法将原始系统指标转换为有意义的动力余量评分：

### Sigmoid 函数变换

核心计算使用 **sigmoid 函数**将线性资源利用率转换为平滑的 0-1 标度：

```
f(x) = 1 / (1 + e^(-k * (x - x₀)))
```

其中：
- **x**: 原始指标值（标准化后的 0-1 范围）
- **x₀ (midpoint)**: 指标开始显著影响评分的阈值点
- **k (steepness)**: 控制曲线陡峭度；数值越高，评分变化越剧烈

### 多阶段处理流水线

1. **原始数据收集**: 平台特定的指标收集（Linux: `/proc` 文件系统, macOS: 系统命令）
2. **标准化处理**: 将原始值转换为 0-1 标度以确保处理一致性
3. **Sigmoid 变换**: 根据各指标特性应用独立的 sigmoid 曲线
4. **瓶颈分析**: 识别表现最差的资源（最低评分）
5. **最终映射**: 将 0-1 结果转换为 0-5 动力余量评分

### 自适应阈值

各项指标使用精心调校的参数：
- **CPU 指标**: 在使用率峰值和持续负载之间保持平衡敏感度
- **内存指标**: 更高阈值以适应操作系统正常缓存行为
- **I/O 指标**: 适中敏感度以区分轻负载和重负载工作
- **网络指标**: 分别处理带宽利用率和丢包率敏感度

这种数学方法确保 pwrzv 提供直观、可操作的评分，反映真实的系统性能瓶颈，而非原始利用率百分比。

## ⚙️ 环境变量配置

pwrzv 支持通过环境变量自定义各个指标的 sigmoid 函数参数，以适应不同的系统特性和使用场景。

### macOS 平台环境变量

```bash
# CPU 使用率配置（默认：midpoint=0.60, steepness=8.0）
export PWRZV_MACOS_CPU_USAGE_MIDPOINT=0.60
export PWRZV_MACOS_CPU_USAGE_STEEPNESS=8.0

# CPU 负载配置（默认：midpoint=1.2, steepness=5.0）
export PWRZV_MACOS_CPU_LOAD_MIDPOINT=1.2
export PWRZV_MACOS_CPU_LOAD_STEEPNESS=5.0

# 内存使用率配置（默认：midpoint=0.85, steepness=20.0）
export PWRZV_MACOS_MEMORY_USAGE_MIDPOINT=0.85
export PWRZV_MACOS_MEMORY_USAGE_STEEPNESS=20.0

# 内存压缩配置（默认：midpoint=0.60, steepness=15.0）
export PWRZV_MACOS_MEMORY_COMPRESSED_MIDPOINT=0.60
export PWRZV_MACOS_MEMORY_COMPRESSED_STEEPNESS=15.0

# 磁盘 I/O 配置（默认：midpoint=0.70, steepness=10.0）
export PWRZV_MACOS_DISK_IO_MIDPOINT=0.70
export PWRZV_MACOS_DISK_IO_STEEPNESS=10.0

# 网络带宽配置（默认：midpoint=0.80, steepness=6.0）
export PWRZV_MACOS_NETWORK_MIDPOINT=0.80
export PWRZV_MACOS_NETWORK_STEEPNESS=6.0

# 网络丢包配置（默认：midpoint=0.01, steepness=50.0）
export PWRZV_MACOS_NETWORK_DROPPED_MIDPOINT=0.01
export PWRZV_MACOS_NETWORK_DROPPED_STEEPNESS=50.0

# 文件描述符配置（默认：midpoint=0.90, steepness=30.0）
export PWRZV_MACOS_FD_MIDPOINT=0.90
export PWRZV_MACOS_FD_STEEPNESS=30.0

# 进程数量配置（默认：midpoint=0.80, steepness=12.0）
export PWRZV_MACOS_PROCESS_MIDPOINT=0.80
export PWRZV_MACOS_PROCESS_STEEPNESS=12.0
```

### Linux 平台环境变量

```bash
# CPU 使用率配置（默认：midpoint=0.65, steepness=8.0）
export PWRZV_LINUX_CPU_USAGE_MIDPOINT=0.65
export PWRZV_LINUX_CPU_USAGE_STEEPNESS=8.0

# CPU I/O 等待配置（默认：midpoint=0.20, steepness=20.0）
export PWRZV_LINUX_CPU_IOWAIT_MIDPOINT=0.20
export PWRZV_LINUX_CPU_IOWAIT_STEEPNESS=20.0

# CPU 负载配置（默认：midpoint=1.2, steepness=5.0）
export PWRZV_LINUX_CPU_LOAD_MIDPOINT=1.2
export PWRZV_LINUX_CPU_LOAD_STEEPNESS=5.0

# 内存使用率配置（默认：midpoint=0.85, steepness=18.0）
export PWRZV_LINUX_MEMORY_USAGE_MIDPOINT=0.85
export PWRZV_LINUX_MEMORY_USAGE_STEEPNESS=18.0

# 内存压力配置（默认：midpoint=0.30, steepness=12.0）
export PWRZV_LINUX_MEMORY_PRESSURE_MIDPOINT=0.30
export PWRZV_LINUX_MEMORY_PRESSURE_STEEPNESS=12.0

# 磁盘 I/O 配置（默认：midpoint=0.70, steepness=10.0）
export PWRZV_LINUX_DISK_IO_MIDPOINT=0.70
export PWRZV_LINUX_DISK_IO_STEEPNESS=10.0

# 网络带宽配置（默认：midpoint=0.80, steepness=6.0）
export PWRZV_LINUX_NETWORK_MIDPOINT=0.80
export PWRZV_LINUX_NETWORK_STEEPNESS=6.0

# 网络丢包配置（默认：midpoint=0.01, steepness=50.0）
export PWRZV_LINUX_NETWORK_DROPPED_MIDPOINT=0.01
export PWRZV_LINUX_NETWORK_DROPPED_STEEPNESS=50.0

# 文件描述符配置（默认：midpoint=0.90, steepness=25.0）
export PWRZV_LINUX_FD_MIDPOINT=0.90
export PWRZV_LINUX_FD_STEEPNESS=25.0

# 进程数量配置（默认：midpoint=0.80, steepness=12.0）
export PWRZV_LINUX_PROCESS_MIDPOINT=0.80
export PWRZV_LINUX_PROCESS_STEEPNESS=12.0
```

### 参数含义

- **midpoint**: sigmoid 函数的中点值，表示该指标开始显著影响评分的阈值
- **steepness**: sigmoid 函数的陡峭度，数值越大曲线越陡峭，评分变化越剧烈

### 使用示例

```bash
# 为高性能服务器调整 CPU 阈值
export PWRZV_LINUX_CPU_USAGE_MIDPOINT=0.80
export PWRZV_LINUX_CPU_USAGE_STEEPNESS=15.0

# 运行 pwrzv
pwrzv --detailed
```

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

# 详细指标分析示例
cargo run --example detailed_metrics
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