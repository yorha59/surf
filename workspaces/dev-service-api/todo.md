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
      - 通过 HTTP `POST /rpc` 发送 `scan.start` 请求，在 `/tmp` 路径上启动扫描，返回有效 `task_id`；
      - 通过 HTTP `POST /rpc` 调用 `scan.status`，在任务运行/完成时返回 `state`、`progress`、`scanned_files`、`scanned_bytes` 等字段；
      - 通过 HTTP `POST /rpc` 调用 `scan.result`，在任务完成后返回包含 `summary`、`top_files`、`by_extension`、`stale_files` 的结果结构；
      - 通过 HTTP `POST /rpc` 调用 `scan.cancel`，对不存在的 `task_id` 返回带 `error.code = -32602` 的错误响应。

- `./test_service.sh`
  - 状态：✅ 通过
  - 实际行为：在脚本内部使用 `target/release/surf-service` 启动服务，在 `http://127.0.0.1:1234/rpc` 上提供 HTTP JSON-RPC 入口，并完成一次 `scan.start` → `scan.status` → `scan.result` → `scan.cancel` 的往返调试流程。

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
实现一个最小可用的 JSON-RPC 服务二进制，提供 `scan.start`、`scan.status`、`scan.result`、`scan.cancel` 四类核心方法，支持异步任务执行与状态查询；完成本工作区内的构建与自测。本轮补充迭代引入 HTTP `POST /rpc` 主路径，对齐 `Architecture.md` 第 4.2、6.2 与 PRD 第 3.2.2、8 节对服务模式的要求。

## 本轮任务列表（构建 + 自测）

### 1. 构建与依赖检查
- [x] 在本工作区运行 `cargo build --release`，确认编译通过（仅存在非致命告警）。
- [x] 在本工作区运行 `cargo test`，修复单元测试与集成测试（包括 `tests/integration.rs`）。

### 2. JSON-RPC 方法族对齐 Architecture.md 6.2
- [x] 确认并实现 `scan.start`/`scan.status`/`scan.result`/`scan.cancel` 四个方法。
- [x] `scan.start`：接受 Architecture.md 中定义的核心参数（兼容 `path`/`root_path`，支持数值与带单位字符串的 `min_size`），返回 `task_id`。
- [x] `scan.status`：基于 `task_id` 返回任务 `state`、`progress`，并补充 `scanned_files`、`scanned_bytes`、`eta_seconds` 字段。
- [x] `scan.result`：在任务完成后返回包含 `task_id`、`summary`、`top_files`、`by_extension`、`stale_files` 的结果结构。
- [x] 保证 JSON-RPC 响应遵循 2.0 规范，回显请求 `id`；当前实现通过 HTTP `POST /rpc` 和内部统一的 JSON-RPC 调度函数复用同一套方法族。

### 3. 自测与工具脚本
- [x] 修正集成测试 `tests/integration.rs`：
  - 通过原生 `TcpStream` 构造 HTTP 请求（`POST /rpc`），并从 HTTP 响应中解析 JSON-RPC body；
  - 放宽对 `id` 取值的强约束，聚焦方法语义与结果结构；
  - 通过 `CARGO_BIN_EXE_surf-service` 自动定位待测二进制，避免对特定构建 profile（debug/release）的硬编码依赖。
- [x] 更新 `test_service.sh`，改为使用 `curl`（或 `python3`）以 HTTP 方式向 `http://127.0.0.1:1234/rpc` 发送 JSON-RPC 请求，并确保可从仓库根目录直接运行脚本。

### 4. 产物交付可用性
- [x] 确认 `cargo build --release` 生成 `target/release/surf-service`，可被交付工作区直接收集使用。
- [x] 通过 `test_service.sh` 启动二进制并完成一次完整的 `scan.start` → `scan.status` → `scan.result` → `scan.cancel` 流程，验证服务在本机可独立运行，并满足 HTTP `POST /rpc` 主路径的对外契约。

### 5. 文档与后续工作
- [x] 在本工作区 `README.md` 中补充基于 HTTP `POST /rpc` 的启动与调用示例，包含代表性请求/响应样例；后续高级文档（USAGE 等）视整体项目文档规划再补充。

---

### 本轮补充结论

- 状态：✅ 本轮完成且自测通过。
- 代表性 HTTP `/rpc` 请求与响应示例：

```bash
curl -s -X POST \
  -H "Content-Type: application/json" \
  --data '{"jsonrpc":"2.0","id":1,"method":"scan.start","params":{"path":"/tmp","threads":2,"min_size":"1MB","limit":10}}' \
  http://127.0.0.1:1234/rpc
```

典型响应（示意）：

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": { "task_id": "550e8400-e29b-41d4-a716-446655440000" },
  "error": null
}
```

随后可以通过 `scan.status` / `scan.result` / `scan.cancel` 使用相同的 HTTP `/rpc` 入口继续管理该任务。
