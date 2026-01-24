# 架构设计文档

## 1. 架构目标
- **架构要解决的问题**：为 Linux 和 macOS 系统提供一个极速、美观且功能强大的磁盘扫描与分析工具，帮助用户快速定位磁盘占用大户，支持多维度可视化分析，并提供清理建议。
- **设计原则**：
  - 高性能：利用并发技术最大化 IO 效率，针对 SSD 进行优化
  - 可扩展性：模块化设计，支持多种运行模式和输出格式
  - 跨平台兼容：完美支持 Linux (x86/ARM) 和 macOS (Intel/Apple Silicon)
  - 安全性：扫描过程中不修改任何文件数据，删除操作需二次确认
  - 无依赖分发：提供单文件静态编译二进制包
- **约束条件**：
  - 最小化内存占用，确保在处理数百万文件时系统不卡顿
  - 支持 JSON-RPC 2.0 协议
  - 提供 TUI 和 GUI 两种交互方式

## 2. 技术栈
- **后端语言与框架**：Rust (Edition 2021)，使用 rayon/tokio 实现并发，clap 处理命令行参数
- **前端技术**：React + Tailwind CSS + Vite，使用 Tauri 作为桌面应用框架
- **数据存储**：SQLite (通过 rusqlite 或 sqlx) 存储配置和历史记录
- **部署方式**：
  - 命令行工具：单文件静态编译二进制
  - GUI 应用：通过 Tauri 打包为 macOS 应用
- **第三方依赖**：
  - 终端进度条：indicatif
  - TUI：ratatui
  - JSON-RPC：基于 tokio 实现
  - 前端可视化：Recharts 或 D3.js
  - 状态管理：TanStack Query 或 Zustand
  - 命令行解析：clap
  - 序列化：serde, serde_json
  - CSV 导出：csv
  - HTML 转义：html-escape
  - 终端输入：termion

## 3. 整体架构概览
- **系统整体结构**：
  - 核心扫描引擎 (Scanner Engine)：负责并发扫描磁盘，收集文件元数据
  - 数据聚合层 (Data Aggregator)：处理和存储扫描结果
  - 服务层 (Service Layer)：提供 JSON-RPC 接口
  - 命令行界面 (CLI/TUI)：提供终端交互
  - 图形界面 (GUI)：基于 Tauri + React 的桌面应用
- **模块关系**：
  - 核心扫描引擎是基础，被服务层、CLI 和 GUI 共享
  - 服务层封装扫描引擎，提供网络接口
  - CLI 和 GUI 作为不同的前端，通过不同方式与核心功能交互

## 4. 模块拆分设计

### 4.1 核心扫描引擎 (Scanner Engine)
- **模块职责**：
  - 并发扫描磁盘文件系统
  - 收集文件元数据（路径、大小、修改时间等）
  - 处理权限不足的目录
  - 支持过滤规则
- **模块边界**：
  - 负责：文件系统扫描、元数据收集、过滤处理
  - 不负责：结果展示、持久化存储
- **对外提供的能力**：
  - `scan(path, min_size, threads)`：启动扫描并返回结果
  - `status()`：获取扫描进度和状态
- **依赖哪些模块**：
  - 标准库：`std::fs`, `std::thread`
  - 第三方：`rayon` 或 `tokio` 用于并发
- **推荐负责人角色**：后端工程师

### 4.2 数据聚合层 (Data Aggregator)
- **模块职责**：
  - 内存中维护目录树结构
  - 汇总和排序扫描结果
  - 提供数据查询接口
- **模块边界**：
  - 负责：数据结构管理、结果汇总、排序
  - 不负责：文件系统扫描、结果展示
- **对外提供的能力**：
  - `get_tree(path)`：获取指定路径的目录树
  - `get_top_files(limit)`：获取按大小排序的前 N 个文件
  - `get_file_type_distribution()`：获取文件类型分布
- **依赖哪些模块**：
  - 核心扫描引擎的扫描结果
- **推荐负责人角色**：后端工程师

### 4.3 服务层 (Service Layer)
- **模块职责**：
  - 实现 JSON-RPC 2.0 协议
  - 管理扫描任务的生命周期
  - 处理并发请求
- **模块边界**：
  - 负责：网络通信、任务管理、协议实现
  - 不负责：文件系统扫描、数据存储
- **对外提供的能力**：
  - JSON-RPC 接口：`Surf.Scan`, `Surf.Status`, `Surf.GetResults`, `Surf.Cancel`
- **依赖哪些模块**：
  - 核心扫描引擎
  - 数据聚合层
  - 第三方：`tokio` 用于网络通信
- **推荐负责人角色**：后端工程师

> 本小节为 `dev-service-api` 开发 Agent 的主要接口约定与行为规范，开发时应以此为准。

#### 4.3.1 JSON-RPC 通用约定

- **协议版本**：严格遵循 JSON-RPC 2.0，所有请求与响应必须包含：
  - `jsonrpc: "2.0"`
  - `id: string | number | null`
  - `method: string`
  - `params: object`（本项目不使用 positional params）
- **监听地址与端口**：
  - 默认 `host = 127.0.0.1`，`port = 1234`，与 CLI 参数 `--host` / `--port` 对应。
  - **安全默认值**：未显式指定时，仅监听本地回环地址，避免暴露到公网。
  - 若用户将 `host` 设置为 `0.0.0.0` 等非本地地址，视为显式放宽安全策略，服务层不会再额外限制，但会在日志中进行高风险提示。
- **传输层**：
  - 初期实现基于 TCP + 行分隔 JSON（一个请求/响应一行），后续可按需扩展为 HTTP Transport。
  - 统一采用 UTF-8 编码。
- **任务 ID 约定**：
  - 服务层为每次扫描生成唯一的 `task_id`（推荐 UUID v4 字符串，如 `"a1b2-..."`）。
  - `task_id` 在服务进程存活期间全局唯一。
  - 所有与特定扫描任务相关的方法均通过 `task_id` 进行路由。
- **任务状态机**：
  - 任务状态枚举：`queued` / `running` / `completed` / `failed` / `canceled`。
  - 状态迁移：
    - `queued -> running`：被调度器选中并实际调用 `surf-core` 启动扫描时。
    - `running -> completed`：扫描正常结束。
    - `running -> failed`：扫描过程中发生不可恢复错误（如路径不存在、IO 错误等）。
    - `queued | running -> canceled`：用户通过 `Surf.Cancel` 取消，或服务层主动回收长时间未访问任务。
  - 终止态：`completed` / `failed` / `canceled`，终止态任务不会再迁移到其他状态。
- **并发与资源限制**：
  - 服务层对**同时运行的扫描任务数**设置上限 `max_concurrent_scans`（默认 4，可通过配置或环境变量调整）。
  - 超出上限的新任务：
    - 接受请求并创建任务，初始状态为 `queued`；
    - 当有运行中的任务结束时，从队列中按 FIFO 或简单优先级策略调度。
  - PRD 中“同时 10 个扫描任务”的高并发验收，通过【已运行 + 队列中的任务总数 ≥ 10】满足，服务应保持可用且不崩溃。
- **任务生命周期与回收**：
  - 终止态任务在内存中保留至少 `task_ttl_seconds`（建议默认 600 秒）以便客户端查询结果。
  - 超过 TTL 的任务将被后台清理，`Surf.GetResults` / `Surf.Status` 再访问时返回“任务不存在”错误。

#### 4.3.2 JSON-RPC 错误模型

- 所有错误通过 JSON-RPC 标准 `error` 对象返回：

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32001,
    "message": "TASK_NOT_FOUND",
    "data": {
      "detail": "task_id not found: ..."
    }
  }
}
```

- **通用错误码约定**（`code` 字段）：
  - `-32600`：`INVALID_REQUEST`（标准码）—— JSON-RPC 请求结构本身非法。
  - `-32601`：`METHOD_NOT_FOUND`（标准码）。
  - `-32602`：`INVALID_PARAMS`（标准码）—— 参数缺失或类型错误。
  - `-32603`：`INTERNAL_ERROR`（标准码）—— 未归类的内部异常。
  - `-32001`：`TASK_NOT_FOUND`—— `task_id` 不存在或已被回收。
  - `-32002`：`TASK_NOT_IN_RUNNING_STATE`—— 仅允许对 `running`/`queued` 状态任务进行的操作（如取消），目标任务已处于终止态。
  - `-32003`：`CONCURRENCY_LIMIT_EXCEEDED`—— 服务端拒绝创建新任务且不入队（仅在未来可能的“硬拒绝策略”下使用，MVP 可以不启用）。
  - `-32010`：`PERMISSION_DENIED`—— 目标路径权限不足或被服务配置显式禁止。

- `error.data` 字段：
  - 类型：`object`，用于补充错误上下文（如 `path`、`task_id`、底层 IO 错误信息摘要）。
  - 对外不泄露敏感信息（如完整系统用户名、环境变量等），仅提供排查所需的最小必要信息。

#### 4.3.3 `Surf.Scan` 接口

> 用途：创建新的扫描任务，立即运行或进入队列，返回 `task_id` 及初始状态。

- **请求**：

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "Surf.Scan",
  "params": {
    "path": "/path/to/scan",
    "min_size": "100MB",
    "threads": 8,
    "limit": 100,
    "exclude_patterns": ["**/node_modules/**"],
    "tag": "optional-client-tag"
  }
}
```

- **参数说明 (`params`)**：
  - `path: string`（必填）
    - 起始扫描根目录，对应 CLI `--path`，允许相对路径与绝对路径。
  - `min_size: string`（可选）
    - 与 CLI 一致，支持 `B`/`KB`/`MB`/`GB` 单位字符串；缺省等价于 `0`。
  - `threads: number`（可选）
    - 并发扫描线程数，对应 CLI `--threads`；缺省时使用逻辑核心数。
  - `limit: number`（可选）
    - 结果 TopN 限制，对应 CLI `--limit`，影响 `Surf.GetResults` 默认输出规模。
  - `exclude_patterns: string[]`（可选，MVP 可以忽略实现细节，仅预留字段）
    - 路径排除规则，语义与未来 CLI 参数对齐。
  - `tag: string`（可选）
    - 供上层（如 GUI）打标识别此任务用途，例如 `"onboarding-initial-scan"`。

- **成功响应**：

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "task_id": "uuid-string",
    "state": "queued",  // 或 "running"
    "path": "/path/to/scan",
    "min_size_bytes": 104857600,
    "threads": 8,
    "limit": 100
  }
}
```

- **错误场景**：
  - `INVALID_PARAMS (-32602)`：参数缺失或无法解析，如非法 `min_size` 单位。
  - `PERMISSION_DENIED (-32010)`：对 `path` 无读取权限。
  - `INTERNAL_ERROR (-32603)`：内部 IO 错误或 `surf-core` 初始化失败。

#### 4.3.4 `Surf.Status` 接口

> 用途：查询一个或多个扫描任务的实时状态与进度，用于驱动进度条或任务列表。

- **请求（查询单任务）**：

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "Surf.Status",
  "params": {
    "task_id": "uuid-string"
  }
}
```

- **请求（查询所有活跃任务）**：

```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "Surf.Status",
  "params": {
    "task_id": null
  }
}
```

- **参数说明**：
  - `task_id: string | null`（可选）
    - 为字符串时：仅查询该任务状态。
    - 为 `null` 或未提供：返回当前所有非终止态任务的状态列表（可按实现添加分页限制，如最多返回 100 条）。

- **成功响应（单任务）**：

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "task_id": "uuid-string",
    "state": "running",            // queued/running/completed/failed/canceled
    "progress": 0.42,               // 0.0 ~ 1.0，估算值
    "scanned_files": 123456,
    "scanned_bytes": 9876543210,
    "total_bytes_estimate": 12345678901, // 若无法估算可为 null
    "started_at": 1710000000,       // Unix timestamp (seconds)
    "updated_at": 1710000100,
    "tag": "optional-client-tag"
  }
}
```

- **成功响应（多任务）**：
  - `result` 字段为上述对象的数组。

- **错误场景**：
  - `TASK_NOT_FOUND (-32001)`：查询单任务时，`task_id` 不存在或已被回收。

#### 4.3.5 `Surf.GetResults` 接口

> 用途：获取已完成扫描任务的结果摘要或 TopN 列表，用于表格/可视化展示。

- **请求**：

```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "method": "Surf.GetResults",
  "params": {
    "task_id": "uuid-string",
    "mode": "flat",
    "limit": 100
  }
}
```

- **参数说明**：
  - `task_id: string`（必填）
  - `mode: string`（可选）
    - `"flat"`：返回按大小降序排序的扁平列表（对应 CLI TopN 表格）。
    - `"summary"`：仅返回整体统计（总文件数、总大小、起始路径）。
    - 默认值：`"flat"`。
  - `limit: number`（可选）
    - 覆盖创建任务时的 `limit`，指定本次返回的最大条目数。

- **成功响应示例（flat 模式）**：

```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "result": {
    "task_id": "uuid-string",
    "state": "completed",          // 仅在 completed 状态下返回完整结果
    "path": "/path/to/scan",
    "total_files": 1234567,
    "total_bytes": 9876543210,
    "entries": [
      {
        "path": "/path/to/file1.log",
        "size": 123456789,
        "is_dir": false,
        "file_type": "log",
        "modified_at": 1709999999
      }
      // ... 按 size 降序，最多 limit 条
    ]
  }
}
```

- **成功响应示例（summary 模式）**：
  - 仅返回 `task_id`、`state`、`path`、`total_files`、`total_bytes` 等聚合字段，不包含 `entries`，以降低网络开销。

- **错误场景**：
  - `TASK_NOT_FOUND (-32001)`：任务不存在或已被回收。
  - `INVALID_PARAMS (-32602)`：当目标任务状态不是 `completed` 时（包括 `queued` / `running` / `failed` / `canceled`），服务端**不返回任何结果数据**，而是返回该错误。MVP / 当前阶段的统一约定是：仅在任务 `state = "completed"` 时才允许通过 `Surf.GetResults` 获取结果。
    - `error.data.detail` 示例：`"task is not in completed state (current: running)"`，实现时可带上当前状态值，便于客户端区分。
    - 未来如需支持进行中任务的部分结果（partial TopN），将在后续迭代中以**协议扩展**形式引入（如新增 mode 或参数），不影响本阶段既有行为；该能力不在当前 MVP 范围内。

- **与 `surf-core` / 数据聚合层的边界**：
  - 服务层仅缓存**聚合后的结果视图**（如 TopN 列表、总文件数与总大小等），不长期持有完整文件列表。
  - 扫描完成后，`surf-core` + Data Aggregator 产出一次性的结果结构体；服务层将其转换为可序列化的中间结构并保存在内存中，供 `Surf.GetResults` 多次读取。
  - 不涉及持久化存储；历史结果持久化交由 `dev-persistence` 后续扩展。

#### 4.3.6 `Surf.Cancel` 接口

> 用途：取消排队或正在运行的扫描任务。该操作应设计为**幂等**。

- **请求**：

```json
{
  "jsonrpc": "2.0",
  "id": 5,
  "method": "Surf.Cancel",
  "params": {
    "task_id": "uuid-string"
  }
}
```

- **参数说明**：
  - `task_id: string`（必填）

- **成功响应**：

```json
{
  "jsonrpc": "2.0",
  "id": 5,
  "result": {
    "task_id": "uuid-string",
    "previous_state": "running",   // 可能为 queued/running/completed/failed/canceled
    "current_state": "canceled"    // 若 previous_state 已是终止态，则等于 previous_state
  }
}
```

- **行为约定**：
  - 如果任务处于 `queued`：从队列中移除，直接标记为 `canceled`。
  - 如果任务处于 `running`：请求底层 `surf-core` 尽快中断扫描（例如通过取消标志）；中断完成后标记为 `canceled`。
  - 如果任务已处于终止态（`completed`/`failed`/`canceled`）：
    - 仍返回 200 级别的 JSON-RPC 成功响应，`current_state` 与 `previous_state` 相同，以保证幂等性。
  - 取消操作**不删除已存在的部分结果**，`Surf.GetResults` 在终止态下仍可访问（例如部分扫描结果或失败原因摘要）。

- **错误场景**：
  - `TASK_NOT_FOUND (-32001)`：任务不存在或已被回收。

#### 4.3.7 服务层资源与安全策略（面向 dev-service-api）

- **资源控制**：
  - `max_concurrent_scans`：限制同时运行的扫描任务数量，默认 4，可通过配置/环境变量调整。
  - `task_queue_limit`：可选的队列长度上限（例如 100）；超过后可以返回 `CONCURRENCY_LIMIT_EXCEEDED`，避免内存被大量排队任务占满。
  - 长时间运行任务：
    - 服务层应定期更新任务的 `updated_at` 时间戳；
    - 可选地为单个任务设置最大香蕉执行时间（如 24 小时），超时自动标记为 `failed` 并中断 `surf-core` 扫描。
- **安全策略**：
  - 默认只监听 `127.0.0.1`，需用户显式配置才允许远程访问。
  - 短期内不引入复杂认证机制；若监听非本地地址，建议结合系统防火墙或反向代理进行访问控制。
  - 对传入的 `path` 做最小合法性校验：禁止明显不合法的路径字符串，避免路径注入类问题。
  - 不提供删除/修改文件的 JSON-RPC 方法，保持服务层只读；删除能力仅在 CLI/TUI/GUI 中通过本地操作提供。
- **与 `surf-core` 的交互边界**：
  - 服务层对每个任务维护一个 `ScanHandle`（实现细节由 `dev-core-scanner` 提供）：
    - `start_scan(path, min_size, threads) -> ScanHandle`
    - `poll_status(handle) -> StatusSnapshot`
    - `collect_results(handle) -> AggregatedResult`
    - `cancel(handle)`
  - 服务层不直接操作文件系统，只通过上述 API 与扫描引擎交互。
  - 状态轮询与结果收集由服务层的后台任务（例如基于 `tokio::spawn`）完成，前端 JSON-RPC 请求只读取最新快照，避免阻塞网络线程。

### 4.4 命令行界面 (CLI/TUI)
- **模块职责**：
  - 解析命令行参数
  - 显示扫描进度
  - 展示扫描结果
  - 提供 TUI 交互
- **模块边界**：
  - 负责：用户输入处理、结果展示、终端交互
  - 不负责：文件系统扫描、数据存储
- **对外提供的能力**：
  - 命令行工具：`surf [options] [path]`
  - TUI 交互：键盘导航、文件操作
- **依赖哪些模块**：
  - 核心扫描引擎
  - 数据聚合层
  - 第三方：`clap` 用于参数解析，`indicatif` 用于进度条，`ratatui` 用于 TUI
- **推荐负责人角色**：后端工程师

> 单次运行模式下，CLI 采用 stdout/stderr 分流策略：默认表格模式中进度条与日志统一输出到 stderr，最终结果表格输出到 stdout；在 `--json` 模式下，stdout 仅在扫描成功时一次性输出完整 JSON，所有进度与错误均写入 stderr；用户通过 Ctrl+C 中断扫描时，两种模式下均不输出部分结果，仅清理进度条并在 stderr 提示“用户中断”，以 130 等非零退出码结束进程。

### 4.5 图形界面层 (GUI Layer)
- **模块职责**：
  - 提供 macOS 桌面应用界面
  - 管理用户配置
  - 展示扫描结果和可视化
  - 提供文件操作功能
- **模块边界**：
  - 负责：用户界面、交互逻辑、可视化展示
  - 不负责：文件系统扫描、网络通信
- **对外提供的能力**：
  - macOS 应用：Surf.app
  - 图形化扫描控制和结果展示
- **依赖哪些模块**：
  - 服务层（通过 JSON-RPC）
  - 第三方：Tauri, React, Tailwind CSS, Recharts/D3.js
- **推荐负责人角色**：前端工程师

### 4.6 持久化存储层 (Persistence Layer)
- **模块职责**：
  - 存储用户配置
  - 记录扫描历史
  - 管理应用状态
- **模块边界**：
  - 负责：数据持久化、配置管理
  - 不负责：文件系统扫描、结果展示
- **对外提供的能力**：
  - `save_config(config)`：保存用户配置
  - `load_config()`：加载用户配置
  - `save_scan_history(history)`：保存扫描历史
  - `load_scan_history()`：加载扫描历史
- **依赖哪些模块**：
  - 第三方：`rusqlite` 或 `sqlx` 用于 SQLite 操作
- **推荐负责人角色**：后端工程师

## 5. 核心数据流

### 5.1 单次运行模式数据流
1. 用户执行 `surf --path /path/to/scan --min-size 100MB`
2. CLI 模块解析参数，调用核心扫描引擎的 `scan()` 方法
3. 核心扫描引擎启动并发扫描，定期更新进度
4. 扫描完成后，数据聚合层汇总和排序结果
5. CLI 模块以表格形式展示结果

### 5.2 服务模式数据流
1. 用户执行 `surf --service --port 1234`（或独立的 `surf-service` 可执行文件）启动服务层，监听 `127.0.0.1:1234`。
2. 客户端（CLI/GUI 或其他应用）通过 JSON-RPC 调用 `Surf.Scan` 方法，创建新的扫描任务并获得 `task_id`。
3. 服务层根据当前并发情况将任务置为 `running` 或 `queued`，并通过 `surf-core` 启动或排队实际扫描逻辑。
4. 扫描进行中，客户端周期性调用 `Surf.Status`：
   - 传入 `task_id` 获取单一任务的进度；
   - 或传入 `null` 获取所有活跃任务的列表，用于任务面板展示。
5. 若用户在 GUI/CLI 中选择取消某个任务，客户端调用 `Surf.Cancel`，服务层请求 `surf-core` 中断扫描并将任务标记为 `canceled`。
6. 扫描完成或进入终止态后，客户端通过 `Surf.GetResults` 获取结果摘要或 TopN 列表，驱动表格/可视化展示。
7. 客户端在完成展示与必要的交互后，可以不再访问该 `task_id`；服务层在 TTL 到期后自动回收该任务的内存与内部句柄。

### 5.3 GUI 模式数据流
1. 用户打开 Surf.app，配置扫描参数
2. GUI 层通过 Tauri 调用后端桥接函数
3. 后端桥接函数启动服务模式（如果未启动）并发送 JSON-RPC 请求
4. 服务层处理请求，调用核心扫描引擎
5. 核心扫描引擎启动并发扫描
6. GUI 层通过 React Query 管理扫描状态和结果
7. 扫描完成后，GUI 层展示结果和可视化

## 6. 风险与待确认问题

### 6.1 技术风险
- **性能风险**：处理数百万文件时的内存占用和扫描速度
- **权限风险**：在 macOS 上获取全盘访问权限的稳定性
- **并发风险**：多线程扫描可能导致系统 IO 压力过大

### 6.2 待确认问题
// 以下问题结合最新 PRD 进行了部分确认与限定：
- **GUI 平台范围**：当前确认 **仅支持 macOS GUI**，Linux GUI 不在本阶段范围内，如未来需要再单独立项设计。
- **扫描快照功能**：PRD 中提到“查看上一次的扫描快照（若支持快照功能）”，本阶段将快照视为**增量能力**，不作为 MVP 的必做项；后续若实现，依托「持久化存储层」存储聚合后的扫描结果摘要，而非完整文件列表。
- **文件删除操作**：在 CLI/TUI/GUI 中统一采用“移入回收站/废纸篓 + 二次确认”的策略，不提供直接永久删除；具体调用各平台系统 API 的方式在实现阶段细化。
 - **服务模式 partial results**：对于进行中的扫描任务通过 JSON-RPC 返回部分结果（如 TopN）的能力，被视为**未来增强能力**，本阶段明确策略为“仅在任务 `state = "completed"` 时通过 `Surf.GetResults` 返回结果”；如未来需要支持 partial results，将在后续迭代中单独设计协议扩展与性能影响评估。

### 6.3 潜在风险
- **系统资源占用**：高并发扫描可能影响系统性能
- **网络安全**：服务模式下的 JSON-RPC 接口可能存在安全风险
- **依赖库兼容性**：第三方库的版本更新可能导致兼容性问题

## 7. 开发 Agent 拆分与工作区规划

> 本节从编排视角，将上述模块拆分映射到多个开发 Agent 及其各自工作区，便于在 Surf 流程中并行开发与统一交付。

### 7.1 开发 Agent 列表

- **dev-core-scanner**
  - 负责模块：核心扫描引擎 (4.1)、数据聚合层的基础结构 (4.2)
  - 主要目标：实现高性能文件系统扫描、过滤与目录树/TopN 等聚合能力，为 CLI / Service / GUI 提供统一数据源。

- **dev-service-api**
  - 负责模块：服务层 (4.3)
  - 主要目标：实现 JSON-RPC 2.0 接口（含 `Surf.Scan` / `Surf.Status` / `Surf.GetResults` / `Surf.Cancel` 等）、任务管理与并发请求处理。

- **dev-cli-tui**
  - 负责模块：命令行界面 (CLI/TUI, 4.4)
  - 主要目标：实现命令行参数解析、进度展示、结果表格输出以及基于 `ratatui` 的交互式 TUI，包括删除操作的二次确认。

- **dev-macos-gui**
  - 负责模块：图形界面层 (GUI, 4.5)
  - 主要目标：基于 Tauri + React 实现 macOS GUI，包括 Onboarding 流程、配置管理、Treemap/列表视图和文件操作集成。

- **dev-persistence**
  - 负责模块：持久化存储层 (4.6)
  - 主要目标：实现配置与扫描历史的持久化，预留支持“扫描快照摘要”的能力。

### 7.2 工作区与目录规划

- **dev-core-scanner 工作区**：`workspaces/dev-core-scanner/`
  - 预期产物：Rust 库 crate（如 `surf-core`），提供扫描与聚合 API；可编译单元测试与简单基准测试。

- **dev-service-api 工作区**：`workspaces/dev-service-api/`
  - 预期产物：可执行二进制（如 `surf-service`），对外监听本地 TCP 端口，提供 JSON-RPC 接口。

- **dev-cli-tui 工作区**：`workspaces/dev-cli-tui/`
  - 预期产物：可执行二进制（如 `surf`），包含 CLI + TUI 模式，依赖 `surf-core` 提供扫描能力。

- **dev-macos-gui 工作区**：`workspaces/dev-macos-gui/`
  - 预期产物：基于 Tauri 的 macOS 应用工程，构建后生成 `Surf.app`，通过 JSON-RPC 与 `surf-service` 通信。

- **dev-persistence 工作区**：`workspaces/dev-persistence/`
  - 预期产物：Rust 库 crate（如 `surf-storage`），封装 SQLite 操作，为 CLI/Service/GUI 提供统一的配置与历史记录访问接口。

### 7.3 与交付阶段的产物衔接

- 各开发 Agent 在其工作区内完成构建后，交付阶段的 `delivery-runner` 将按以下方式汇总：
  - 从 `workspaces/dev-core-scanner/` 和 `workspaces/dev-cli-tui/` 获取 CLI/TUI 相关可执行文件与运行脚本。
  - 从 `workspaces/dev-service-api/` 获取服务模式二进制与配置示例。
  - 从 `workspaces/dev-macos-gui/` 获取打包好的 `Surf.app` 及必要的启动脚本。
  - 从 `workspaces/dev-persistence/` 获取迁移脚本或初始化数据库逻辑说明。
- `delivery-runner` 在自己的交付工作区 `release/` 下，按平台与形态（CLI/TUI/Service/GUI）组织最终发布产物，并在 `test/` 目录下基于 PRD 8. 验收标准设计和执行独立测试。

- 为支持自动化交付，本节补充如下**构建命令与产物布局约定**，供交付节点与各开发 Agent 对齐。

- **各工作区本地构建约定（Rust 部分）**
  - `dev-cli-tui`（工作区根：`workspaces/dev-cli-tui/`）
    - 目标 crate：`surf-cli`，bin 名称：`surf`。
    - 推荐在 `workspaces/dev-cli-tui/surf-cli/` 下执行：
      - `cargo build --release`
    - 预期产物（相对 `workspaces/dev-cli-tui/surf-cli/`）：
      - 可执行文件：`target/release/surf`
    - 如在仓库根目录统一构建，可使用：
      - `cargo build -p surf-cli --release`（产物路径保持为根目录下 `target/release/surf`）。

  - `dev-service-api`（工作区根：`workspaces/dev-service-api/`）
    - 目标 crate：`surf-service`，bin 名称：`surf-service`。
    - 推荐在 `workspaces/dev-service-api/surf-service/` 下执行：
      - `cargo build --release`
    - 预期产物（相对 `workspaces/dev-service-api/surf-service/`）：
      - 可执行文件：`target/release/surf-service`
    - 如在仓库根目录统一构建，可使用：
      - `cargo build -p surf-service --release`（产物路径为根目录下 `target/release/surf-service`）。

  - `dev-core-scanner`（工作区根：`workspaces/dev-core-scanner/`）
    - 目标 crate：`surf-core`，类型：库 crate。
    - 推荐在 `workspaces/dev-core-scanner/surf-core/` 下执行：
      - `cargo build --release`
    - 预期产物：
      - 库文件：位于 `target/release/` 下的 `libsurf_core*` 相关文件（具体文件名由平台与 Rust 目标三元组决定）。
    - 交付语义：`surf-core` **不直接作为独立二进制在交付阶段暴露**，而是在构建 `surf` / `surf-service` 时作为依赖被一并编译和链接；交付节点只需关心最终 CLI / Service 二进制是否能在目标平台上独立运行。

- **交付工作区 `release/` 目录布局建议**（示例）

  > 交付节点自身的工作区位于仓库根目录下，以下路径均以仓库根为基准；平台命名遵循 `os-arch` 约定，可根据实际需要扩展（如添加 `linux-aarch64`、`macos-aarch64` 等）。

  ```text
  release/
    linux-x86_64/
      cli/
        surf              # 由 dev-cli-tui 构建产物复制/链接而来
      service/
        surf-service      # 由 dev-service-api 构建产物复制/链接而来
      gui/
        # TODO: 预留 Linux GUI 形态占位（当前 PRD 不要求实现）

    macos-x86_64/
      cli/
        surf
      service/
        surf-service
      gui/
        Surf.app          # TODO: dev-macos-gui 工作区构建完成后，由交付节点从其工作区复制

    macos-aarch64/
      cli/
        surf              # 如实际交付为通用二进制，可使用统一构建产物
      service/
        surf-service
      gui/
        Surf.app          # 同上，预留 Apple Silicon 形态
  ```

  - `delivery-runner` 在进入交付阶段时，应根据当前目标平台（或编译矩阵）决定需要构建的组合，并：
    - 调用对应工作区构建命令（例如：
      - `cd workspaces/dev-cli-tui/surf-cli && cargo build --release`
      - `cd workspaces/dev-service-api/surf-service && cargo build --release`
      ）；
    - 将 `target/release/surf` 与 `target/release/surf-service` 拷贝或通过符号链接方式布置到上表中的 `release/<platform>/cli/` 与 `release/<platform>/service/` 目录；
    - 对于未来的 `dev-macos-gui`、`dev-persistence`：
      - `dev-macos-gui` 产物 `Surf.app` 由其工作区的 Tauri 构建命令产出，交付阶段仅负责复制到对应 `release/<platform>/gui/`；
      - `dev-persistence` 相关迁移脚本或初始化 SQL 文件可统一放置在 `release/<platform>/service/migrations/` 等子目录下，具体命名在引入该工作区时补充（当前仅占位）。

- **`test/` 目录与 PRD 8. 验收标准的对应关系（结构约定）**

  > 交付工作区下的 `test/` 目录用于组织端到端验收测试资产，其结构与 PRD 8 各子章节（CLI / 服务模式 / TUI / macOS GUI / 非功能性）一一对应。仅定义结构与示例命令形式，具体测试内容由测试/交付节点在后续实现。

  ```text
  test/
    case.md              # 文档化测试用例清单，与 PRD 8 条目做双向映射
    scripts/
      cli_oneoff_basic.sh        # 覆盖“CLI / 单次运行模式”基础验收
      cli_json_mode.sh           # 覆盖 `--json` 行为与 stdout/stderr 语义
      service_jsonrpc_basic.sh   # 覆盖服务模式启动与基本 JSON-RPC 交互
      tui_basic_navigation.sh    # 覆盖 TUI 导航与删除确认流程
      macos_gui_onboarding.sh    # TODO：覆盖 macOS GUI Onboarding 与权限申请
      nonfunc_perf_smoke.sh      # TODO：覆盖大目录扫描下的性能与资源占用
  ```

  - 上述脚本应以 `release/` 中的实际交付产物为入口，例如：
    - `./release/linux-x86_64/cli/surf --path <dir> ...`
    - `./release/linux-x86_64/service/surf-service --host 127.0.0.1 --port 1234 --max-concurrent-scans 4 ...`
  - `test/case.md` 推荐最少包含以下信息列，以保证与 PRD 8 的可追溯性：
    - `id`：测试用例标识，例如 `AC-CLI-ONEOFF-001`；
    - `prd_ref`：对应的 PRD 条目（如 `8. CLI / 单次运行模式`、`CLI-ONEOFF-003` 等）；
    - `script`：关联的脚本文件名与参数示例；
    - `expected`：高层预期结果摘要（如“进程退出码为 0，输出列表按大小降序排序”）。
