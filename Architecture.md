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
  - 并发扫描本地磁盘文件系统（当前已实现形态为**一次性同步扫描**）
  - 收集文件层面的元数据（至少包含：路径、大小），并按大小降序排序
  - 按最小文件大小阈值进行过滤，屏蔽小于 `min_size` 的文件
  - 处理不存在路径、权限不足等基础错误场景，并以 `std::io::Error` 形式反馈给上层
- **模块边界**：
  - 负责：文件系统遍历、文件元数据获取与过滤、结果排序
  - 不负责：结果展示、JSON/表格序列化、任务生命周期管理与持久化存储
  - 关于**进度上报**：核心层负责维护扫描过程中的计数器并提供**只读快照 API**，但不直接管理任务 ID 或 JSON-RPC 语义（这些由服务层/CLI 在上层完成，见 4.3.7 与 5.1/5.2）。
- **依赖哪些模块**：
  - 标准库：`std::fs`, `std::path`, `std::io`
  - 第三方：`walkdir` 用于递归遍历目录树，`rayon` 用于并行扫描
- **推荐负责人角色**：后端工程师（dev-core-scanner 工作区）

- **核心数据结构与 API 形态（对应 `workspaces/dev-core-scanner/surf-core`）**：
  - 结果条目类型：

    ```rust
    pub struct FileEntry {
        pub path: PathBuf,
        pub size: u64,
    }
    ```

    - 语义：
      - `path`：文件的绝对或相对路径，由底层扫描遍历得到；当前仅返回**文件**，不返回目录条目。
      - `size`：文件大小（单位：字节），直接来自底层 `metadata.len()`。
      - 该结构体实现 `Clone` 和 `serde::Serialize`，便于上层进行排序重用和直接序列化。

  - 核心扫描函数签名与行为（**当前已实现**）：

    ```rust
    pub fn scan(root: &Path, min_size: u64, threads: usize) -> std::io::Result<Vec<FileEntry>>
    ```

    - 参数含义：
      - `root`：起始扫描根目录（对应 CLI 的 `--path` 参数；服务层的 `Surf.Scan.params.path` 在落到核心层前需解析为 `Path`）。
      - `min_size`：最小文件大小阈值（单位：字节），对应 CLI 解析后的 `--min-size`，以及服务层解析后的 `min_size`；小于该值的文件会被过滤掉。
      - `threads`：工作线程数，对应 CLI 的 `--threads`，以及服务层中的并发度配置；
        - CLI 层保证 `threads >= 1`，核心层内部对传入的 0 做防御性修正（退化为 1）。
    - 行为约定：
      - 若 `root` 不存在，则立即返回 `ErrorKind::NotFound` 类型的 `std::io::Error`，错误消息中包含形如 `"does not exist"` 的提示，供上层直接展示。
      - 使用 `walkdir::WalkDir` 递归遍历 `root` 下所有条目，对每个条目读取元数据：
        - 仅保留 `metadata.is_file() == true` 的文件条目；目录、符号链接等在当前迭代中一律过滤掉。
        - 仅保留 `metadata.len() >= min_size` 的文件，确保与 CLI / 服务层的最小文件大小过滤语义一致。
      - 使用 `rayon` 在局部线程池中并发执行上述遍历与过滤逻辑，线程数由 `threads` 控制。
      - 返回值为 **完整的** `Vec<FileEntry>`，并保证按 `size` 字段降序排序：
        - 关于 TopN 截断（`limit`）：当前设计中 **不在核心层处理**，而是由 CLI / 服务层在消费该 `Vec<FileEntry>` 时自行截断；这一点与 PRD 中 `--limit` 的语义保持一致。

- **配置与上层参数映射（CLI / 服务层）**：
  - CLI 单次运行模式：
    - `--path` → 直接映射为 `Args.path: PathBuf`，随后传入 `scan(&args.path, ...)`。
    - `--min-size` → 在 CLI 内通过 `parse_size(min_size: &str) -> Result<u64, String>` 解析为字节数；
      - 解析失败时，CLI 在 stderr 输出 `"Error parsing --min-size: ..."` 并以非零状态码退出，**不会调用核心扫描函数**，确保 `--json` 模式下 stdout 保持空白（符合 PRD 9.1.3）。
    - `--threads` → 在 CLI 层通过 `parse_threads` 校验，禁止 0 或非法值；校验失败同样只在 stderr 输出错误并非零退出，不触发扫描。
    - `--limit` → 仅在 CLI 层使用，用于对 `Vec<FileEntry>` 进行 `take(limit)`；核心层对 `limit` 不感知。
  - 服务层（JSON-RPC `Surf.Scan`）的设计对齐：
    - `Surf.Scan.params.path` / `min_size` / `threads` / `limit` 与 CLI 参数在语义上保持一致：
      - 服务层负责将字符串形式的 `path`、`min_size` 转换/解析为 `Path` 与字节数，再调用核心扫描函数或其异步包装。
      - `limit` 仍然作为结果裁剪参数，仅在聚合/结果阶段生效，不应传入核心层。
    - 若未来在核心层引入 `ScanConfig` 结构体封装上述参数，可以在不改变 CLI / 服务层外部接口的前提下进行重构（**未来迭代可调整**）。

- **返回结果与序列化约定（与 CLI JSON / 服务层对齐）**：
  - 核心层对上游只暴露 `Vec<FileEntry>`，不关心具体输出格式；
  - CLI / 服务层在将结果序列化为 JSON/表格/CSV 时，**应以 `FileEntry` 或其聚合视图为基础**，避免各自拼装不兼容字段集合：
    - CLI `--json` 模式当前采用以下结构（位于 `workspaces/dev-cli-tui/surf-cli/src/main.rs`）：

      ```rust
      #[derive(Serialize)]
      struct JsonEntry {
          path: String,
          size: u64,
          is_dir: bool,
      }

      #[derive(Serialize)]
      struct JsonOutput {
          root: String,
          entries: Vec<JsonEntry>,
      }
      ```

      - 构造规则：
        - `root` 直接来自 CLI 参数 `Args.path` 的字符串表示，代表本次扫描的根路径；
        - `entries` 由 `Vec<FileEntry>` 映射而来，基于 `limit` 截断：
          - `path` ← `FileEntry.path.display().to_string()`；
          - `size` ← `FileEntry.size`；
          - `is_dir` ← 当前固定为 `false`，因为核心扫描器仅返回文件条目（目录条目暂不支持，**未来迭代可调整**）。
      - 该结构满足 PRD 9.1.3 中“根路径 + 条目数组（含完整路径、大小、目录标识）”的要求；任何改动需保持 JSON 结构与 `surf-core::FileEntry` 语义的一致性。
    - 服务层 `Surf.GetResults` 中的 `entries` 列表应在概念上与上述 `JsonEntry` 兼容：
      - 至少包含 `path: string`、`size: number`、`is_dir: boolean` 等字段；
      - 可在聚合层/服务层中额外扩展 `file_type`、`modified_at` 等字段，但这些字段应从更完整的核心结果或聚合结构中推导，而不是在各前端（CLI/GUI）中各自发明。

- **与上层模块的契约（CLI 单次运行 & 服务层任务管理）**：
  - CLI 单次运行模式（One-off）调用约定（区分“历史同步实现”与“当前进度感知实现”）：
    - **历史同步实现路径（iteration ≤ 14）**：
      - CLI 以同步方式调用 `scan(&path, min_size_bytes, threads)`，等待结果返回：
        - 成功时，根据 `--json` 决定走 JsonOutput 序列化或表格打印路径；
        - 失败时，进度指示器由 CLI 层负责清理，错误文案基于 `std::io::Error` 直接渲染到 stderr，并以非零状态码退出，不输出部分结果。
      - 进度指示器在该历史路径下仅为简单的 spinner（`"Scanning <path> ..."`），不展示已扫描文件数与已扫描总大小。
    - **当前进度感知实现（已在 `surf-core` 与 `surf-cli` 中部分落地，仍有待完善）**：
      - 在保持 `scan(...)` 作为向后兼容同步 API 的前提下，核心层提供一组额外的进度感知抽象，供 CLI 与服务层复用：

        ```rust
        pub struct ScanConfig {
            pub root: PathBuf,
            pub min_size: u64,
            pub threads: usize,
        }

        pub struct ScanProgress {
            pub scanned_files: u64,
            pub scanned_bytes: u64,
            pub total_bytes_estimate: Option<u64>,
        }

        pub struct StatusSnapshot {
            /// 仅反映底层扫描是否已经自然结束；
            /// 任务级状态（queued/running/completed/failed/canceled）仍由服务层维护。
            pub done: bool,
            pub progress: ScanProgress,
            /// 若底层扫描因 IO 等原因失败，这里给出摘要信息（例如 `ErrorKind` + 文本描述），
            /// 供服务层映射为 JSON-RPC 错误码；具体结构在实现时可细化。
            pub error: Option<String>,
        }

        pub struct ScanHandle { /* opaque, Send + Sync */ }

        pub fn start_scan(config: ScanConfig) -> std::io::Result<ScanHandle>;
        pub fn poll_status(handle: &ScanHandle) -> StatusSnapshot;
        pub fn collect_results(handle: ScanHandle) -> std::io::Result<Vec<FileEntry>>;
        pub fn cancel(handle: &ScanHandle);
        ```

      - CLI 单次运行模式在目标形态下的调用方式：
        - 将原来的 `scan(&path, ...)` 替换为：
          1. 构造 `ScanConfig { root: path.clone(), min_size, threads }`；
          2. 调用 `start_scan(config)` 获取 `ScanHandle`；
          3. 在前台循环调用 `poll_status(&handle)`（例如每 100–200ms 一次），使用 `StatusSnapshot.progress.scanned_files` / `scanned_bytes` / `total_bytes_estimate` 更新 `indicatif` 进度条文本；
          4. 当 `StatusSnapshot.done == true` 时退出循环，调用 `collect_results(handle)` 获取最终 `Vec<FileEntry>`，后续流程与当前实现保持一致（表格或 JSON 输出，`--limit` 截断等）。
        - 进度条与日志继续全部输出到 stderr，stdout 仅在扫描成功结束后输出一次性结果；在 `--json` 模式下同样复用该约定，避免干扰 JSON 消费方（与 PRD 9.3 中的设计决策保持一致）。
        - 对于 Ctrl+C 中断场景，CLI 通过调用 `cancel(&handle)` 请求核心层尽快停止扫描，然后清理进度条并以非零状态码退出，仍然不输出部分结果。

  - 服务层任务管理（`ScanHandle` / `StatusSnapshot` / `AggregatedResult`）的预期集成方式（与 4.3.7 对齐）：
    - 当前 `surf-service` 仅实现 JSON-RPC 协议骨架，尚未真正调用 `surf-core`；本节的 `ScanHandle` 等抽象为**服务层与核心层之间的统一契约**，供后续迭代落地。
    - 服务层在创建扫描任务时，通过 `start_scan(ScanConfig)` 获取 `ScanHandle` 并存入任务表；在后台调度协程中周期性调用 `poll_status(&handle)`，把得到的 `StatusSnapshot`：
      - `StatusSnapshot.progress.scanned_files` 映射为 `Surf.Status.result.scanned_files`；
      - `StatusSnapshot.progress.scanned_bytes` 映射为 `Surf.Status.result.scanned_bytes`；
      - `StatusSnapshot.progress.total_bytes_estimate` 映射为 `Surf.Status.result.total_bytes_estimate`（若为 `None`，JSON 字段为 `null`）；
      - `StatusSnapshot.done` 结合服务层自己的任务状态机，共同决定 `Surf.Status.result.state` 从 `running` 迁移到 `completed`/`failed`；
      - `StatusSnapshot.error` 由服务层翻译为 JSON-RPC 错误码（如 `INTERNAL_ERROR` 或 `PERMISSION_DENIED`），并记入 `error.data.detail`。
    - 在扫描结束后，服务层调用 `collect_results(handle)` 获取完整 `Vec<FileEntry>`，再委托数据聚合层构造 `AggregatedResult` 并挂载到任务上，供 `Surf.GetResults` 使用；`AggregatedResult.entries` 与 CLI `JsonEntry` 保持字段/语义对齐。
    - 对于取消场景，服务层调用 `cancel(&handle)` 尝试中断底层遍历，同时将任务状态迁移为 `canceled`；无论底层是否能立即终止，后续 `Surf.Status` / `Surf.GetResults` 均以服务层状态机为主，核心层仅提供尽力而为的取消信号。

> 以上进度感知 API（`ScanConfig` / `ScanProgress` / `StatusSnapshot` / `ScanHandle` 及其方法）最初作为架构级设计提出，现已在 `surf-core` 代码中落地，并被 CLI 单次运行模式实际使用（参见 `workspaces/dev-core-scanner/surf-core/src/lib.rs` 与 `workspaces/dev-cli-tui/surf-cli/src/main.rs`）。当前实现仍存在若干局限：`total_bytes_estimate` 始终为 `None`，尚未对总字节数做估算；取消语义依然是“最佳努力”，在极端大目录或 IO 压力较高场景下，从用户按下 Ctrl+C 到进程真正退出之间仍可能存在可感知的延迟。但与早期设计相比，核心扫描逻辑已经在遍历过程中主动检查内部 `cancelled` 标志，并在检测到取消请求后尽快终止后续遍历与结果收集。CLI 侧则基于 `poll_status` 展示 `scanned_files` 与 `scanned_bytes`，并在收到 Ctrl+C（SIGINT）时调用 `cancel(&handle)`、清理进度条并以约定的非零退出码退出，从端到端路径上对齐了 PRD 9.1.1 中关于“实时进度反馈 + 中断后不输出部分结果”的行为要求。后续迭代由 `dev-core-scanner`、`dev-service-api` 与 `dev-cli-tui` 协同继续完善总量预估与服务模式进度映射等行为，以完全满足 PRD 9.1.1 与 9.2 中关于 `progress` / `scanned_files` / `scanned_bytes` 的验收要求。

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
  
> 当前实现中，与 TopN 相关的基础聚合能力（按 `size` 降序排序、按 `min_size` 过滤）仍由 `surf-core` crate 中的 `scan` 函数内部完成；本层在后续迭代中可以/建议逐步承接更多聚合与派生指标（如目录级汇总、文件类型分布等），但本轮不对接口形式做强约束，未来迭代可调整。

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

> 实现进度注记（iteration 65 / dev-service-api）：当前 `Surf.GetResults` 已按本小节约定实现参数校验与任务状态机集成，并通过 `TaskManager::collect_results_if_needed` 接入 `surf-core::collect_results` 与内存聚合层：当任务处于 `completed` 状态时，服务端实际调用核心层收集完整 `Vec<FileEntry>`，计算 `total_files`/`total_bytes`，并构造与 CLI `JsonEntry` 兼容的 `entries` 列表（支持 `mode = "flat"/"summary"` 以及 per-call `limit` 覆盖）；收集失败会将任务状态迁移为 `failed` 并返回附带错误详情的 `INVALID_PARAMS`。在任务仍处于运行或非 completed 状态时，`Surf.GetResults` 继续按照本节约定返回 `INVALID_PARAMS` 或 `TASK_NOT_FOUND`，调用方应以 `Surf.Status` 的进度与状态为主，通过 `Surf.GetResults` 获取“已完成任务”的结果快照。

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
- **与 `surf-core` 的交互边界（进度感知集成）**：
  - 服务层对每个任务维护一个 `ScanHandle`（结构与方法见 4.1 中的进度感知 API 设计，由 `dev-core-scanner` 提供具体实现）：
    - `start_scan(config: ScanConfig) -> ScanHandle`
    - `poll_status(handle: &ScanHandle) -> StatusSnapshot`
    - `collect_results(handle: ScanHandle) -> Vec<FileEntry>`（或在内部先交由数据聚合层转换为 `AggregatedResult` 再缓存）。
    - `cancel(handle: &ScanHandle)`
  - 服务层不直接操作文件系统，只通过上述 API 与扫描引擎交互；任务 ID (`task_id`) 与任务状态机（`queued` / `running` / `completed` / `failed` / `canceled`）由服务层维护，并与底层 `ScanHandle` 的生命周期关联：
    - 创建任务时：
      - 解析 JSON-RPC `Surf.Scan` 参数构造 `ScanConfig`；
      - 调用 `start_scan(config)` 成功后生成 `task_id`，并将 `(task_id, ScanHandle, 任务元数据)` 存入任务表，任务状态置为 `running` 或 `queued`（视并发策略而定）。
    - 周期性进度刷新：
      - 后台协程基于 `tokio::spawn` 周期性遍历 `running` 状态任务，调用 `poll_status(&handle)`：
        - 将 `StatusSnapshot.progress.scanned_files` 映射为任务内部的 `scanned_files` 计数，并同步到 JSON-RPC `Surf.Status` 的 `result.scanned_files` 字段；
        - 将 `StatusSnapshot.progress.scanned_bytes` 映射为任务内部的 `scanned_bytes` 计数，并同步到 `Surf.Status.result.scanned_bytes`；
        - 将 `StatusSnapshot.progress.total_bytes_estimate`（若为 `Some`）同步到 `Surf.Status.result.total_bytes_estimate`，否则在 JSON 中以 `null` 表示；
        - 由服务层根据 `scanned_bytes` 与 `total_bytes_estimate` 计算 `progress` 浮点值：

          ```text
          progress =
            if total_bytes_estimate.is_some() && total_bytes_estimate > 0 {
                scanned_bytes as f64 / total_bytes_estimate as f64
            } else {
                null        # 无法估算时，JSON 中可用 null 表示
            }
          ```

        - 当 `StatusSnapshot.done == true` 且 `StatusSnapshot.error.is_none()` 时，任务状态从 `running` 迁移为 `completed`；若 `error.is_some()`，则迁移为 `failed`，并在任务元数据中记录错误摘要。
        - 无论成功或失败，`Surf.Status.result.state` 字段均以服务层任务状态机的值为准，核心层仅通过 `done` / `error` 提供底层信号。
    - 结果收集：
      - 在任务状态进入 `completed`（或 `failed` 但仍需保留部分结果）后，服务层调用 `collect_results(handle)` 获取最终 `Vec<FileEntry>`，并委托数据聚合层生成 `AggregatedResult`：
        - `AggregatedResult.entries` 字段中的每个元素在概念上应与 CLI `JsonEntry` 对齐，至少包含 `path` / `size` / `is_dir` 等字段；
        - 聚合层可追加 `file_type`、`modified_at` 等派生信息，但不得更改核心字段含义。
      - `Surf.GetResults` 直接基于 `AggregatedResult` 序列化响应。
    - 取消与回收：
      - 当接收到 `Surf.Cancel` 请求且任务处于 `queued` 或 `running` 时，服务层调用 `cancel(&handle)` 向核心层发出中断信号，并将任务状态设置为 `canceled`（或在安全点完成迁移）；
      - 任务进入终止态后（`completed` / `failed` / `canceled`），在 `task_ttl_seconds` 内保留其 `StatusSnapshot` 与聚合结果，供后续 `Surf.Status` / `Surf.GetResults` 查询；TTL 到期后，从任务表中删除对应 `ScanHandle` 与缓存结果。
  - JSON-RPC `Surf.Status` 字段与内部快照/状态的对应关系（总结）：
    - `task_id`：来自服务层任务表主键；
    - `state`：来自服务层任务状态机（结合 `StatusSnapshot.done` / `error` 更新）；
    - `progress`：由服务层基于 `scanned_bytes` / `total_bytes_estimate` 估算，无法估算时可设为 `null` 或省略；
    - `scanned_files`：来自最近一次 `StatusSnapshot.progress.scanned_files`；
    - `scanned_bytes`：来自最近一次 `StatusSnapshot.progress.scanned_bytes`；
    - `total_bytes_estimate`：来自最近一次 `StatusSnapshot.progress.total_bytes_estimate`；
    - `started_at` / `updated_at`：由服务层在任务创建与每次轮询时维护，与核心层解耦；
    - `tag`：来自 `Surf.Scan` 请求参数，存放于任务元数据，不由核心层关心。

> 本节对 `Surf.Status` 中 `progress` / `scanned_files` / `scanned_bytes` / `total_bytes_estimate` 的来源和计算方式给出了明确映射关系。后续由 `dev-service-api` 在实现 JSON-RPC 方法时严格遵循本约定，确保服务模式与 CLI/TUI 共同复用 `surf-core` 的进度快照能力，而不是各自重复统计逻辑。

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
1. 用户执行 `surf --path /path/to/scan --min-size 100MB`。
2. CLI 模块解析参数，根据历史实现与当前实现路径选择不同的数据流：
   - **历史实现（无进度快照，仅 spinner）**：
     1. 直接调用核心扫描引擎的同步函数 `scan(&path, min_size_bytes, threads)`；
     2. 在等待结果期间，仅在 stderr 上展示 `indicatif::ProgressBar::new_spinner` 形式的“`Scanning <path> ...`”提示；
     3. 扫描完成或出错后关闭 spinner，进入结果展示或错误处理流程。
   - **当前实现路径（复用核心层进度快照，已基本对齐 PRD 9.1.1 的 CLI 行为）**：
     1. CLI 构造 `ScanConfig { root, min_size, threads }` 并调用 `start_scan(config)` 获取 `ScanHandle`（见 4.1）；
     2. 在前台循环调用 `poll_status(&handle)`，根据返回的 `StatusSnapshot.progress.scanned_files` / `scanned_bytes` / `total_bytes_estimate` 更新 stderr 上的进度条文案，例如：

        ```text
        Scanned {scanned_files} files, {human_readable(scanned_bytes)} read...
        ```

     3. 当 `StatusSnapshot.done == true` 时退出轮询，调用 `collect_results(handle)` 获取最终 `Vec<FileEntry>` 供后续展示使用；
     4. 对于 Ctrl+C 中断场景，CLI 通过 `ctrlc` crate 安装 SIGINT 处理器，在检测到中断时调用 `cancel(&handle)` 请求核心层终止扫描，清理进度条并在 stderr 提示“用户中断”，以 130 等非零退出码结束进程；无论表格还是 JSON 模式均不输出部分结果。核心扫描器在遍历过程中会周期性检查内部取消标志并尽快结束后续工作，但在极端大目录场景下仍可能存在短暂的延迟。
3. 扫描完成后，数据聚合层汇总和排序结果（当前 MVP 中，大部分 TopN 与排序逻辑仍由 `surf-core::scan` 内部完成，数据聚合层后续迭代逐步承接更多责任）。
4. CLI 模块以表格形式展示结果或根据 `--json` 输出结构化 JSON，保持如下 stdout/stderr 语义：
   - 进度条与日志统一输出到 stderr；
   - 表格或 JSON 结果仅在扫描成功完成后一次性输出到 stdout；
   - 错误场景（参数错误、IO 错误、中断）只在 stderr 输出文案，stdout 保持空白。

### 5.2 服务模式数据流
1. 用户执行 `surf --service --port 1234`（或独立的 `surf-service` 可执行文件）启动服务层，监听 `127.0.0.1:1234`。
2. 客户端（CLI/GUI 或其他应用）通过 JSON-RPC 调用 `Surf.Scan` 方法，创建新的扫描任务并获得 `task_id`：
   - 服务层解析请求参数，构造 `ScanConfig`；
   - 调用 `start_scan(config)` 获取 `ScanHandle`；
   - 在内存任务表中登记 `(task_id, ScanHandle, 任务状态及元数据)`，状态初始为 `running` 或 `queued`（取决于 `max_concurrent_scans` 策略）。
3. 服务层根据当前并发情况调度任务：
   - 当任务真正启动或从排队转为运行时，对应的 `ScanHandle` 被传入后台 worker 协程；
   - worker 在生命周期内周期性调用 `poll_status(&handle)`，更新任务内部的 `scanned_files` / `scanned_bytes` / `total_bytes_estimate` 等字段，并驱动任务状态从 `running` 向 `completed` / `failed` 演进（见 4.3.7）。
4. 扫描进行中，客户端周期性调用 `Surf.Status`：
   - 传入 `task_id` 获取单一任务的进度；
   - 或传入 `null` 获取所有活跃任务的列表，用于任务面板展示；
   - 服务层在处理 `Surf.Status` 时，从任务表中读取最近一次 `StatusSnapshot` 的聚合结果并映射到 JSON 响应：
     - `scanned_files` / `scanned_bytes` / `total_bytes_estimate` 分别对应内部计数；
     - `progress` 由服务层基于 `scanned_bytes` 与 `total_bytes_estimate` 估算；
     - `state` 则完全来自服务层任务状态机（`queued` / `running` / `completed` / `failed` / `canceled`）。
5. 若用户在 GUI/CLI 中选择取消某个任务，客户端调用 `Surf.Cancel`，服务层查找任务并：
   - 调用 `cancel(&handle)` 请求 `surf-core` 尽快中断扫描；
   - 将任务状态迁移为 `canceled`（或在安全点迁移），后续 `Surf.Status` 反映终止态，`Surf.GetResults` 视实现选择是否返回部分结果或仅返回失败摘要（当前设计倾向于不返回 partial TopN，见 4.3.5 与 6.2）。
6. 扫描完成或进入终止态后，客户端通过 `Surf.GetResults` 获取结果摘要或 TopN 列表，驱动表格/可视化展示：
   - 服务层在任务结束时调用 `collect_results(handle)` 获取 `Vec<FileEntry>`，并通过数据聚合层构造 `AggregatedResult` 缓存于任务结构中；
   - `Surf.GetResults` 基于缓存的 `AggregatedResult` 序列化响应，而不再访问核心层。
7. 客户端在完成展示与必要的交互后，可以不再访问该 `task_id`；服务层在 TTL 到期后自动回收该任务的内存与内部 `ScanHandle`/结果缓存，后续再访问同一 `task_id` 会返回 `TASK_NOT_FOUND (-32001)` 错误。

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
    - 构建注意事项：在部分离线或使用镜像源的环境中，`cargo build -p surf-service --release` 可能因 `clap` / `clap_builder` 依赖解析冲突而失败（例如报错 `failed to select a version for clap_builder ... does not have these features`）；这类问题属于构建环境与上游依赖的版本/特性对齐问题，需要在具备网络或完整依赖缓存的本地/CI 环境中由人类开发者调整依赖版本或 Cargo 配置后重新构建。本仓库不在架构层面对具体镜像或依赖修复策略做出强约束，仅约定成功构建后应按上述路径产出 `surf-service` 二进制供交付阶段使用。
    - 现实状态注记（iteration 76 / delivery）：当前仓库根下的 `release/linux-x86_64/service/surf-service` 二进制仍是早期仅提供监听能力的占位实现，启动时会在 stdout 打印类似 “surf-service listening on 127.0.0.1:21523 ... JSON-RPC methods (Surf.Scan / Surf.Status / Surf.GetResults / Surf.Cancel) are not implemented yet; this binary ...” 的提示，并不会对 JSON-RPC 请求返回有效响应。受上述 `clap_builder` 依赖冲突限制，本运行环境暂无法从最新源码重新构建 `surf-service` 并同步到 `release/` 目录。因此，本轮交付阶段引入的 `test/scripts/service_jsonrpc_basic.sh` 与 `test/scripts/service_jsonrpc_invalid_params.sh` 在本环境中预期会失败，其错误日志会包含该占位文案；后续需在具备正常 crates.io/镜像配置的开发机或 CI 上修复依赖问题、重建 `target/release/surf-service` 并将其拷贝为 `release/linux-x86_64/service/surf-service` 后，再以这些脚本作为 JSON-RPC 基本/错误路径的交付验收基线。
    - 现实状态注记（iteration 77 / delivery）：本轮基于 `release/linux-x86_64/cli/surf` 运行了 `test/scripts/cli_oneoff_basic.sh` 与 `test/scripts/cli_json_mode.sh` 两个 CLI 冒烟脚本。两脚本均 PASS（退出码均为 0）：前者完成 `--help` 最小检查，后者验证 `--json` 输出结构包含 `root` 与 `entries` 字段，CLI 二进制存在且可用。后续建议继续在具备网络的开发机或 CI 上完善服务层二进制的构建并复查服务相关脚本，以保证交付端到端覆盖。
    - 现实状态注记（本次 Ralph 第 1 轮 / delivery）：本轮在仓库根目录运行 `bash test/scripts/service_jsonrpc_invalid_params.sh`，脚本在发送带非法 `min_size`（如 `10XYZ`）的 `Surf.Scan` 请求后同样只看到服务进程输出占位提示 `"surf-service listening on 127.0.0.1:21523 ... JSON-RPC methods (Surf.Scan / Surf.Status / Surf.GetResults / Surf.Cancel) are not implemented yet; this binary ..."`，客户端侧收到空响应并以 `EXIT_CODE:1` 标记 FAIL。该结果进一步佐证当前 `release/linux-x86_64/service/surf-service` 仍为不带 JSON-RPC 真实实现的占位二进制，无法用于验证服务模式下的参数校验与错误码行为；在具备正常 Rust 依赖环境的开发机或 CI 上重新构建并替换该二进制之前，`test/scripts/service_jsonrpc_basic.sh` 与 `test/scripts/service_jsonrpc_invalid_params.sh` 的失败应视为交付工件版本落后的已知问题，而非服务层源码逻辑本身的回归。

    - 现实状态注记（本次 Ralph 第 2 轮 / delivery）：
      - 脚本：`test/scripts/service_jsonrpc_basic.sh`
      - 二进制：`release/linux-x86_64/service/surf-service`
      - 退出码：`1`（FAIL）
      - 输出要点：服务进程 stdout 显示 `surf-service listening on 127.0.0.1:21523 ... JSON-RPC methods (Surf.Scan / Surf.Status / Surf.GetResults / Surf.Cancel) are not implemented yet ...`，客户端侧收到空响应。
      - 失败原因：占位二进制缺少 `Surf.Scan` / `Surf.Status` / `Surf.GetResults` / `Surf.Cancel` 的真实实现。
    - 交付结论：当前 release 下的 `surf-service` 仍无法用于验证 JSON-RPC happy path；需在具备正常 Rust 依赖环境的机器上重新构建并同步 release 二进制后再重跑该脚本。

    - 现实状态注记（本次 Ralph 第 3 轮 / delivery）：
      - 构建尝试：在仓库根目录执行 `cargo build -p surf-service --release`，构建失败；关键报错为 `failed to parse the edition key`，提示当前工具链仅支持 `2015`/`2018`，无法识别 `edition = "2021"`，说明本运行环境的 Rust/Cargo 版本过旧，不满足本项目要求的 2021 edition。
      - 离线重试：执行 `cargo build -p surf-service --release --offline` 仍然失败，错误为无法下载 `anyhow v1.0.100` 等依赖且本地无缓存（`can't make HTTP request in the offline mode`），佐证当前环境既缺少新版工具链，也缺少完整依赖缓存。
      - 二进制与脚本：由于构建失败，本轮未能替换 `release/linux-x86_64/service/surf-service`，继续沿用占位二进制；在此基础上运行 `bash test/scripts/service_jsonrpc_basic.sh` 与 `bash test/scripts/service_jsonrpc_invalid_params.sh`，两个脚本均以退出码 `1` 失败，客户端侧仅看到空响应，服务 stdout 仍打印 `"JSON-RPC methods (Surf.Scan / Surf.Status / Surf.GetResults / Surf.Cancel) are not implemented yet"` 类提示。
    - 交付结论：在当前交付环境下，服务二进制仍停留在“占位实现 + 旧工具链”的状态，无法通过 JSON-RPC 基本/错误路径脚本；要让 `SVC-JSONRPC-001` 在交付阶段真正闭环，需在具备 Rust 2021 edition 且可访问 crates.io（或有完整镜像/缓存）的机器上构建并同步新的 `surf-service` 至 `release/linux-x86_64/service/` 后，再复跑上述脚本。

    - 现实状态注记（本次 Ralph 第 4 轮 / delivery）：
      - CLI 冒烟验证：在仓库根目录依次运行 `bash test/scripts/cli_oneoff_basic.sh` 与 `bash test/scripts/cli_json_mode.sh`，两个脚本在当前环境下均 PASS（退出码均为 0），输出中包含 `PASS` 与 `EXIT_CODE:0` 标记，表明 `release/linux-x86_64/cli/surf` 二进制存在且至少在帮助输出与最小 JSON 模式路径上工作正常。
      - 服务二进制现状：本轮未再尝试构建 `surf-service`，仍沿用上一轮结论——`release/linux-x86_64/service/surf-service` 仍为早期占位实现，缺乏真实的 JSON-RPC 方法；在当前运行环境缺少 Rust 2021 edition 且无法访问 crates.io 的前提下，服务相关脚本（`test/scripts/service_jsonrpc_basic.sh` / `test/scripts/service_jsonrpc_invalid_params.sh`）预计继续 FAIL，其失败应继续理解为交付工件版本落后，而非 `workspaces/dev-service-api/surf-service` 源码逻辑本身的回归。
      - 人工后续建议：要在交付层面真正完成 `SVC-JSONRPC-001` 的验证，建议在具备 Rust 2021 edition 且已预热依赖缓存的开发机或 CI 上执行以下步骤：
        1. 在仓库根目录运行 `cargo build -p surf-service --release` 生成新的服务二进制；
        2. 将生成的 `target/release/surf-service` 覆盖到 `release/linux-x86_64/service/surf-service`；
        3. 在该环境中运行 `bash test/scripts/service_jsonrpc_basic.sh` 与 `bash test/scripts/service_jsonrpc_invalid_params.sh`，以确认 JSON-RPC 基本路径和 INVALID_PARAMS 错误路径均能通过；
        4. 若上述脚本仍失败，再回溯到 `workspaces/dev-service-api/surf-service/src/main.rs` 与配套测试用例定位逻辑问题，并在本节追加新的“现实状态注记”。

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
