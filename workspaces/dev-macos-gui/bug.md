# dev-macos-gui 缺陷记录（bug）

> 说明：本文件记录与本工作区相关的缺陷或环境限制。

## 2026-01-27 · Rust 工具链版本限制导致 Tauri 宿主无法在当前环境通过 `cargo check`

- 标题：Rust 工具链版本过低，Tauri 依赖链中的 `time` crate 无法编译
- 现象与复现步骤：
  - 在仓库根目录下执行：
    - `cd workspaces/dev-macos-gui`
    - `cargo check --manifest-path src-tauri/Cargo.toml`
  - 输出提示：
    - `time@0.3.46` / `time-core@0.1.8` 需要 `rustc 1.88.0`，而当前环境为 `rustc 1.86.0`。
- 期望行为 vs 实际行为：
  - 期望：在本地开发环境中能够成功执行 `cargo check`/`cargo build`，完成 `src-tauri` 宿主应用的基本编译检查。
  - 实际：由于全局 Rust 工具链版本低于依赖要求，`cargo check` 失败。
- 初步分析与修复方案：
  - 该问题属于环境/工具链限制，而非本工作区代码逻辑缺陷。
  - 潜在解决方案：
    - 在本机或 CI 上升级 Rust 工具链至 `1.88.0` 或更高版本；或
    - 在具备更高版本 Rust 的环境中执行构建与打包；或
    - 根据 Tauri 官方支持矩阵，选择与当前 `rustc` 兼容的旧版 Tauri 及其依赖（需进一步验证）。
- 状态：待环境升级 / 兼容性确认
