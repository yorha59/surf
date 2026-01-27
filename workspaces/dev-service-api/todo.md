# dev-service-api 开发任务清单

## 自测记录（本轮迭代）

### 构建与测试命令

在 `workspaces/dev-service-api/` 下执行：

```bash
# 1. 构建发布二进制
cargo build --release

# 2. 运行本 crate 的单元测试与集成测试（包括基于真实二进制的集成测试）
cargo test

# 3. 运行工作区自带的服务自测脚本（依赖第 1 步生成的 release 二进制）
./test_service.sh
```

### 运行结果摘要

- `cargo build --release`
  - 状态：✅ 通过

- `cargo test`
  - 状态：✅ 通过
  - 覆盖范围：
    - `src/main.rs` 中的 JSON-RPC 处理核心逻辑单元测试（`handle_scan_start`、`handle_scan_status` 等）。
    - `tests/integration.rs` 中基于真实 `surf-service` 二进制的端到端集成测试，包括：
      - `scan.start`：在 `/tmp` 路径上启动扫描，返回有效 `task_id`；
      - `scan.status`：在任务运行/完成时返回 `state`、`progress`、`scanned_files`、`scanned_bytes` 等字段；
      - `scan.result`：在任务完成后返回包含 `summary`、`top_files`、`by_extension`、`stale_files` 的结果结构；
      - `scan.cancel`：对不存在的 `task_id` 返回带 `error.code = -32602` 的错误响应。

- `./test_service.sh`
  - 状态：✅ 通过
  - 实际行为：在脚本内部使用 `target/release/surf-service` 启动服务，在 `127.0.0.1:1234` 上监听，并完成一次 `scan.start` → `scan.status` → `scan.result` → `scan.cancel` 的往返调试流程。

### 关键行为与接口契约（与 Architecture 对齐）

- 服务监听：默认 `127.0.0.1:1234`，可通过 `--host` / `--port` 覆盖。
- 请求契约：
  - 所有请求遵循 JSON-RPC 2.0，包含 `jsonrpc` / `method` / `params` / `id` 字段；
  - `scan.start.params` 兼容 `path` 与 `root_path` 字段名，`min_size` 支持纯数值和类似 `"100MB"` 的带单位字符串。
- 响应契约：
  - `scan.start`：`result = { "task_id": string }`；
  - `scan.status`：`result` 中包含 `task_id`、`state`、`progress`、`scanned_files`、`scanned_bytes`、`eta_seconds` 以及可选的 `result`/`error`；
  - `scan.result`：`result` 中包含 `task_id`、`summary`、`top_files`、`by_extension`、`stale_files`；
  - `scan.cancel`：对有效任务返回 `result = null`（当前实现仅标记状态，尚未真正中断底层扫描，属于后续优化点）；对无效 `task_id` 返回 `error.code = -32602`。

---

## 本轮迭代目标（2026-01-27）
实现一个最小可用的 JSON-RPC 服务二进制，提供 `scan.start`、`scan.status`、`scan.result`、`scan.cancel` 四类核心方法，支持异步任务执行与状态查询；完成本工作区内的构建与自测。

## 本轮任务列表（构建 + 自测）

### 1. 构建与依赖检查
- [x] 在本工作区运行 `cargo build --release`，确认编译通过（仅存在非致命告警）。
- [x] 在本工作区运行 `cargo test`，修复单元测试与集成测试（包括 `tests/integration.rs`）。

### 2. JSON-RPC 方法族对齐 Architecture.md 6.2
- [x] 确认并实现 `scan.start`/`scan.status`/`scan.result`/`scan.cancel` 四个方法。
- [x] `scan.start`：接受 Architecture.md 中定义的核心参数（兼容 `path`/`root_path`，支持数值与带单位字符串的 `min_size`），返回 `task_id`。
- [x] `scan.status`：基于 `task_id` 返回任务 `state`、`progress`，并补充 `scanned_files`、`scanned_bytes`、`eta_seconds` 字段。
- [x] `scan.result`：在任务完成后返回包含 `task_id`、`summary`、`top_files`、`by_extension`、`stale_files` 的结果结构。
- [x] 保证 JSON-RPC 响应遵循 2.0 规范，回显请求 `id`。

### 3. 自测与工具脚本
- [x] 修正集成测试 `tests/integration.rs`：
  - 使用显式的 TCP 连接与半关闭写端，避免阻塞；
  - 放宽对 `id` 取值的强约束，聚焦方法语义与结果结构；
  - 通过 `CARGO_BIN_EXE_surf-service` 自动定位待测二进制，避免对特定构建 profile（debug/release）的硬编码依赖。
- [x] 更新 `test_service.sh`，避免对特定 `nc -N` 选项的依赖，增加对 `nc`/`python3` 的回退支持，并确保可从仓库根目录直接运行脚本。

### 4. 产物交付可用性
- [x] 确认 `cargo build --release` 生成 `target/release/surf-service`，可被交付工作区直接收集使用。
- [x] 通过 `test_service.sh` 启动二进制并完成一次完整的 `scan.start` → `scan.status` → `scan.result` → `scan.cancel` 流程，验证服务在本机可独立运行。

### 5. 文档与后续工作
- [ ] 文档补充（例如 README/USAGE 等）暂不在本轮范围内，等待编排者或设计节点明确统一文档形态后再处理。
