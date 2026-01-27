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
  
- 环境阻塞：Tauri 后端需 `rustc >= 1.88.0`（Architecture.md 10.1），当前环境为 `rustc 1.86.0`，
  导致 `cargo check --manifest-path src-tauri/Cargo.toml` 及依赖 Tauri 宿主编译的命令（如 `cargo build`、
  `npm run tauri:dev`、`npm run tauri:build`）无法通过，本轮开发在 Tauri 宿主层面受阻。

- 状态：已解决（当前环境已升级至 `rustc 1.93.0`，`cargo check`/`cargo build` 与 `npm run tauri:build` 均能在本机通过，保留本条作为历史记录）

## 2026-01-27 · Tauri DMG 打包脚本 bundle_dmg.sh 失败（Surf.app 已生成）

- 标题：`npm run tauri:build` 在执行 `bundle_dmg.sh` 时失败，但 Surf.app 已成功生成
- 现象与复现步骤：
  - 在仓库根目录下执行：
    - `cd workspaces/dev-macos-gui`
    - `npm run tauri:build`
  - 终端输出关键片段：
    - `Bundling Surf.app (.../workspaces/dev-macos-gui/src-tauri/target/release/bundle/macos/Surf.app)`
    - `Bundling Surf_0.1.0_aarch64.dmg (.../workspaces/dev-macos-gui/src-tauri/target/release/bundle/dmg/Surf_0.1.0_aarch64.dmg)`
    - `Running bundle_dmg.sh`
    - `Error failed to bundle project: error running bundle_dmg.sh`
- 期望行为 vs 实际行为：
  - 期望：`npm run tauri:build` 在本地 macOS 环境中能够完成 Surf.app 构建，并成功生成可分发的 DMG 安装镜像；
  - 实际：Surf.app 已成功生成，DMG 在执行自定义脚本 `bundle_dmg.sh` 阶段失败，导致命令整体返回非零退出码。
- 初步分析与修复方案：
  - 该问题集中在 DMG 打包脚本层面，对本地开发与测试可直接运行的 Surf.app 影响有限；
  - 建议后续由交付/打包节点检查 `bundle_dmg.sh` 的具体实现，确认：
    - 脚本是否依赖额外的打包工具或权限（如 `hdiutil`、自定义签名命令等）；
    - 是否需要为 CI/本地环境单独提供简化版打包流程，或仅在正式发布环境执行完整 DMG 生成。
- 当前结论：
  - 对本轮“在 rustc>=1.88.0 前提下完成 Tauri 后端编译与 GUI 端到端自测”的目标不构成阻塞；
  - Surf.app 已在 `src-tauri/target/release/bundle/macos/Surf.app` 生成，可以用于后续交付节点的手工验证与签名实验。
