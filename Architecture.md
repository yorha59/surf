# Surf 架构设计（初稿）

> 说明：本文件是基于当前 `PRD.md` 与 `AGENTS.md` 的最小可用架构设计骨架，用于指导后续开发 / 交付阶段的模块拆分、接口形态和工作区规划。当前仓库尚不包含任何代码或构建脚本，下面描述的模块、工作区与产物均为**规划目标**，后续迭代可在本文件基础上增量细化。

## 1. 设计目标与范围

1. 对齐 PRD 中的核心目标：提供在 Linux 和 macOS 上运行的极速磁盘扫描与分析工具，覆盖 CLI、TUI、JSON-RPC 服务和 macOS GUI 四种形态。
2. 将需求拆分为若干可独立实现、可并行开发的模块（或开发 Agent 工作区），并确保它们在交付阶段可以通过构建产物自然拼接成完整的端到端能力。
3. 明确关键接口形态（CLI 参数族、JSON-RPC 方法族、GUI 与服务之间的交互方式），但暂不进入具体代码级实现细节。
4. 为后续交付阶段规划统一的构建与发布视图：交付工作区如何从各开发工作区收集二进制/前端产物，并在 `release/` 目录下组织最终可交付包。

本初稿优先覆盖模块边界、职责与数据流，部分细节（错误码枚举、具体持久化方案等）留待后续迭代补充或由人类确认。

## 2. 全局架构视图

从宏观上，Surf 被拆分为以下几个主要子系统：

1. **核心扫描与分析引擎（Core Scanner & Analyzer）**
   - 负责以只读方式遍历本地文件系统，统计目录与文件大小，计算文件类型分布、Top N 大文件等分析指标。
   - 为上层形态（CLI/TUI、JSON-RPC 服务、GUI）提供统一的数据模型与分析结果。

2. **服务接口层（JSON-RPC Service）**
   - 提供长生命周期的 JSON-RPC 2.0 服务进程，支持“启动扫描 / 查询进度 / 获取结果 / 取消任务”等核心远程调用。
   - 作为 GUI 和其他外部客户端的主要接入点。

3. **CLI & TUI 前端（CLI/TUI Frontend）**
   - 提供单次运行模式的命令行工具（含纯 CLI 与 TUI 两种交互形式）。
   - 负责参数解析、进度展示、结果表格/JSON 输出，以及 TUI 模式下的结果浏览与导航（当前版本不支持删除操作）。

4. **macOS GUI 前端（macOS GUI via Tauri）**
   - 基于 Tauri + React 的桌面应用，负责 Onboarding、权限申请、图形可视化（Treemap、列表视图）、交互操作（Reveal in Finder / Move to Trash / Copy Path 等）。
   - 通过 JSON-RPC 与服务接口层通信，可自动启动本地 server 或连接远程 server。

5. **共享模型与配置 / 历史存储（Shared Model & Config/History）**
   - 定义跨模式统一的数据契约（扫描任务、目录节点、文件条目、统计摘要等）。
   - 负责用户配置（默认路径、线程数、过滤规则、GUI 偏好等）与扫描历史记录的落盘与加载。

6. **交付与构建视图（Delivery & Release View）**
   - 交付阶段的专用工作区，从各开发工作区收集构建产物，统一打包到 `release/` 目录，并在该工作区内执行基于 PRD 的独立测试。

各子系统之间的依赖自下而上：**核心扫描引擎**为基础，其上叠加服务接口层与各类前端；共享模型与配置贯穿各层；交付视图则聚焦于构建产物和测试，不参与运行时调用链。

## 3. 技术栈与运行形态（对齐 PRD）

> 说明：本节只总结 PRD 已经确定的技术路线，并将其映射到架构模块，不引入新的强制约束。

1. **后端 / 核心**
   - 语言与运行时：Rust（Edition 2021）。
   - 并发模型：
     - 文件系统遍历与 CPU 密集计算：建议基于 `rayon` 或线程池模型实现多线程扫描；
     - 服务模式与 IO：建议基于 `tokio` 提供异步网络与 JSON-RPC 处理。
   - 通信协议与传输层：
     - 统一应用层协议为 **JSON-RPC 2.0**；
     - **主路径（当前版本）**：HTTP + JSON-RPC 2.0，服务以 `POST /rpc` 形式在 `http://<host>:<port>` 暴露接口（默认 `http://127.0.0.1:1234/rpc`），供 macOS GUI 以及其他 HTTP 客户端使用；
     - **兼容路径（可选扩展）**：原始 TCP JSON-RPC 2.0，服务可在同一进程中额外暴露一个 TCP 监听端点（仍建议绑定在 `127.0.0.1` 上的端口），供 CLI/TUI 或其他需要更轻量本地接入的客户端直接复用 TCP 通道；
     - HTTP 与 TCP 监听共享同一套 JSON-RPC 方法调度层（见第 6.2 节），在方法族与数据结构层面保持完全一致，仅传输层不同。

> 说明：PRD 第 6 节仅要求提供“基于 `tokio` 的轻量 JSON-RPC 2.0 服务”，并未限定传输层是纯 TCP 还是 HTTP。本设计将默认暴露方式细化为“HTTP + JSON-RPC（主路径）+ 原始 TCP JSON-RPC（兼容路径）”，属于实现层设计细化，不需要同步修改 PRD，仅在本节与第 4.2、6.2 节中记录具体约定。

2. **命令行 / TUI**
   - CLI 参数解析：`clap` 系列库（或等价方案）。
   - TUI 渲染：`ratatui`（或等价方案）。
   - 运行平台：Linux (x86/ARM)、macOS (Intel/Apple Silicon)。

3. **macOS GUI**
   - 宿主框架：Tauri（Rust 后端 + Web 前端）。
   - 前端技术栈：React + Tailwind CSS + Vite；状态管理推荐 TanStack Query 或 Zustand；可视化推荐 Recharts 或 D3.js。
   - 主要运行平台：macOS（Intel/Apple Silicon）。

4. **交付与发布**
   - 非功能性要求：尽可能提供单文件或最小依赖的二进制分发；
   - 构建与打包细节将在交付工作区内按本文件的交付视图规划落地。

## 4. 模块划分与职责

### 4.1 核心扫描与分析引擎（Core Scanner & Analyzer）

**职责：**

1. 以只读方式遍历指定根路径下的文件系统：
   - 支持多线程/异步并发扫描（对齐 `--threads`）。
   - 支持按 `--min-size` 过滤小文件，避免无意义的细粒度统计。

2. 构建内存中的目录树模型：
   - 每个节点记录路径、类型（目录/文件）、大小、子节点聚合信息。
   - 为分层视图、Treemap、列表视图提供统一数据源。

3. 执行分析计算：
   - **文件类型统计**：按扩展名聚合大小和数量（支持 `.log`、`.mp4` 等常见类型）。
   - **大文件排行榜**：按大小降序返回 Top N 文件（对齐 `--limit`）。
   - **时间维度分析**：对接文件元数据，识别“陈旧文件”（未访问/未修改时长超阈值）。

4. 对上层提供统一 API / 数据模型：
   - 输入：扫描请求（路径、线程数、过滤条件、排除规则等）。
   - 输出：
     - 扫描摘要（总文件数、总大小、目录级统计）；
     - 文件级结果列表（可分页或流式）；
     - 类型分布与时间分析结果。

5. 文件删除能力（受平台与策略约束）：
   - 提供统一的库级删除接口（示意）：
     - `delete_entry(path: &Path, options: DeleteOptions) -> DeleteResult`；
     - `DeleteOptions` 字段：
       - `mode: DeleteMode`：`MoveToTrash` / `Permanent`；
       - `origin: DeleteOrigin`：调用来源（`Cli` / `Tui` / `Gui` / `Service`），用于审计与策略演进；
       - `dry_run: bool`：只做权限与可行性检查，不真正删除。
   - 核心层不直接决定「是否需要二次确认」，而是由具有删除能力的上层 UI（例如 macOS GUI，未来如开放 CLI/TUI 删除能力亦可复用）在完成确认后再调用删除接口；
   - 平台默认语义：
     - macOS：当 `mode = MoveToTrash` 时使用系统废纸篓语义（与 Finder 的 Move to Trash 一致），不直接执行永久删除；
     - Linux：优先尝试按照 XDG Trash 规范实现「移动到回收站」，若检测到环境不支持（无标准 Trash 目录等），则退化为永久删除，并在 `DeleteResult` 中显式标记 `effective_mode = Permanent`，由上层 UI 告知用户「将被永久删除」。
   - Linux 平台上是否启用回收站语义及默认模式已在 `human.md` 中由人类决策，并在第 5.3 节中固化为当前版本的策略；如未来调整，仅需更新配置与本节文案，接口契约保持不变。
**不负责的内容：**

- 不直接处理网络协议与 JSON-RPC 编解码；
- 不负责终端或 GUI 渲染细节；
- 不直接管理长期任务队列（由服务层负责）。

### 4.2 服务接口层（JSON-RPC Service）

**职责：**

1. 提供长运行进程，监听 JSON-RPC 2.0 请求：
   - 默认地址 `127.0.0.1`，默认端口 `1234`（可通过 `--host` / `--port` 覆盖）。
   - 保证在高并发请求（例如同时 10 个扫描任务）下的稳定性与资源控制。

2. 管理扫描任务生命周期：
   - 任务创建：根据请求参数生成新的扫描任务，调用核心扫描引擎以异步方式执行；
   - 任务状态：跟踪任务处于队列中、扫描中、已完成、已取消或失败状态；
   - 任务结果：缓存或持久化扫描结果摘要，以便后续查询与 GUI 展示；
   - 任务取消：将取消请求传递到核心扫描引擎，并安全终止扫描流程。

3. 对外暴露稳定的 JSON-RPC 方法族（见第 6.2 节）。

4. 与共享配置 / 历史模块协作：
   - 记录服务端的历史任务和配置（例如默认扫描参数）；
   - 为 GUI 提供任务列表与历史查询接口（可通过扩展的 JSON-RPC 方法实现）。

**传输层形态与端口约定：**

1. 服务二进制（规划名：`surf-service`）在 `--service` 模式下，至少提供一个 HTTP 监听端点：
   - 形态：`POST /rpc`；
   - 地址：`http://<host>:<port>/rpc`，其中 `<host>`、`<port>` 由命令行 `--host` / `--port` 控制（默认 `127.0.0.1:1234`）。

2. 为兼容原始 TCP JSON-RPC 客户端，服务可在同一进程内额外暴露一个 TCP 监听端点：
   - 形态：原始 TCP 连接上传输 JSON-RPC 2.0 请求/响应帧；
   - 地址：建议同样绑定在 `127.0.0.1` 上的端口，具体端口是否与 HTTP 共用或分离可在实现阶段通过内部配置或后续扩展参数细化，本轮设计仅要求 **不会改变现有 `--host` / `--port` 的对外语义**；
   - 该 TCP 端点与 HTTP 端点复用同一套 JSON-RPC 方法族与调度逻辑。

3. 当前迭代的验收重点：
   - **HTTP JSON-RPC 端点必须可用**，以支撑 `dev-macos-gui` 中基于 `fetch("/rpc")` 的集成路径；
   - 是否在同一版本内同时落地原始 TCP 端点由开发阶段按工作量权衡，但架构上已预留扩展空间。

**不负责的内容：**

- 不直接负责终端或 GUI 渲染；
- 不负责本地 Onboarding 界面与权限引导（由 GUI 前端负责）。

### 4.3 CLI & TUI 前端

**职责：**

1. 单次运行 CLI 模式：
   - 解析命令行参数（`--path`、`--threads`、`--min-size`、`--limit`、`--json` 等）；
   - 调用核心扫描与分析引擎执行一次性扫描；
   - 扫描过程中在终端展示进度条（例如 `indicatif`），显示已处理文件数和累计大小；
   - 扫描完成后：
     - 默认以格式化表格形式输出按大小降序排序的结果（Top N 由 `--limit` 控制）；
     - 若指定 `--json`，则以结构化 JSON 格式输出（契约见第 6.1 节）。

2. TUI 模式：
   - 在终端内以交互式界面展示扫描结果目录树，支持键盘导航、查看文件详情；
   - 支持搜索 / 过滤（按文件名、大小范围、修改时间等）；
   - 当前版本仅提供扫描结果的浏览与筛选能力，不暴露任何删除入口；如未来引入删除操作，将在 `Architecture.md` 与 `human.md` 中重新确认策略后再开放。

3. 错误处理与退出码：
   - 对非法参数、路径不存在、权限不足等情况，输出友好错误信息并以非零退出码终止进程；
   - 对 `Ctrl+C` 中断进行捕获和优雅退出，避免产生半写入输出文件。

**不负责的内容：**

- 不提供长运行 JSON-RPC 服务；
- 不负责 GUI 侧的任何渲染或交互逻辑；
- 不直接管理历史记录（可在未来视需要与共享配置模块集成）。

### 4.4 macOS GUI 前端

**职责：**

1. 用户 Onboarding 流程：
   - 检测配置文件是否存在，不存在时进入引导流程；
   - 引导用户授予“全盘访问权限”（Full Disk Access）；
   - 完成默认扫描路径、线程数、最小过滤大小等基础配置；
   - 检查本地 Surf server（JSON-RPC 服务）的状态，如未运行则尝试自动启动。

2. 主界面（Main Dashboard）：
   - 左侧边栏：收藏路径、最近扫描记录、设置入口；
   - 顶部栏：当前路径选择器、搜索框、扫描控制按钮（开始/暂停/停止）；
   - 中央视图：
     - Treemap 视图：根据目录树与文件类型统计呈现磁盘占用；
     - 列表视图：以表格形式展示当前层级的文件/目录，支持排序与过滤。

3. 交互功能：
   - 悬停提示：显示完整路径与精确大小；
   - 下钻浏览：点击 Treemap 或列表项进入子目录；
   - 右键操作：`Reveal in Finder`、`Move to Trash`、`Copy Path` 等，通过 JSON-RPC 调用服务或本地桥接逻辑完成。

4. 配置与历史管理：
   - GUI 中的用户设置（语言、主题、默认路径、排除规则、JSON-RPC 服务器地址等）写入本地配置存储；
   - 扫描历史记录存储并可视化展示，支持快速重新扫描或查看历史快照（如支持时）。

**不负责的内容：**

- 不直接实现文件扫描逻辑（通过 JSON-RPC 或本地桥接调用核心引擎 / 服务实现）；
- 不负责终端侧交互；
- 不负责系统级安装 / 卸载逻辑（可在交付阶段补充）。

#### 4.4.1 Onboarding 初始化与配置写入

1. 配置文件存在性检测与路径约定：
   - macOS GUI 在每次启动时按统一约定检查配置文件路径 `~/.config/surf/config.json`（与 Linux 一致，遵循 Surf 自定义约定而非系统偏好）；
   - 若文件存在且可解析，则作为 GUI、服务与 CLI 的默认配置来源之一（具体字段见第 4.5.1 节）；
   - 若文件不存在或无法解析，将按“首次启动”路径处理，并在 Onboarding 结束时生成新的配置文件；无法解析的旧配置可按实现需要备份到同目录下的备份文件（例如 `config.json.bak`）。

2. 首次启动与默认配置生成：
   - 当检测到配置文件缺失/不可用时，GUI 进入 Onboarding 流程（欢迎页、权限引导、基础配置）；
   - Onboarding 过程至少收集：
     - 默认扫描路径（映射到配置字段 `default_path`，建议预填为当前用户主目录，如 `~/`）；
     - 默认并发线程数（`threads`，建议预填为逻辑核心数）；
     - 默认最小过滤大小（`min_size`，如 `"100MB"`，可由用户调整）；
     - JSON-RPC 服务地址与端口（`rpc_host` / `rpc_port`），用于 GUI 与服务的默认连接，建议默认 `127.0.0.1:1234`；
   - Onboarding 完成后，GUI 将在 `~/.config/surf/config.json` 写入一份符合第 4.5.1 节约定的 JSON 配置文件，作为后续运行的默认配置基础。

3. GUI 对配置的读取与写入行为：
   - GUI 启动时从配置文件中读取 `default_path` / `threads` / `min_size` / `rpc_host` / `rpc_port` / `theme` / `language` 等字段，填充设置面板与默认扫描参数：
     - 若 `theme` 缺失，GUI 默认“跟随系统主题”；
     - 若 `language` 缺失，GUI 默认“跟随系统语言”，在中英之间自动选择；
   - 用户在设置面板中修改以上选项后，GUI 应回写同一路径配置文件，保证下次启动时行为一致；
   - GUI 可在 Onboarding 或设置中额外收集并写入 `exclude_patterns`、`stale_days` 等扩展字段，但这些字段的解释以共享模型模块的定义为准。

4. 与服务和 CLI 行为的映射（概览）：
   - GUI 在“启动/连接服务”时，优先使用配置中的 `rpc_host` / `rpc_port` 作为默认连接目标；用户在 GUI 中覆盖后可选择同步回写配置；
   - 当 GUI 提供“一键启动 CLI”或“从配置启动 CLI”能力时，应将配置中的 `cli_path`（如 `cli/surf` 或系统 PATH 中的 `surf`）作为默认可执行路径，并以 `default_path` / `threads` / `min_size` 作为缺省命令行参数来源；
   - 对详细字段含义、默认值与跨模块映射，见第 4.5.1 节。

#### 4.4.2 `dev-macos-gui` 集成指导（HTTP 主路径）

1. 前端调用方式（当前主路径）：
   - `workspaces/dev-macos-gui/src/services/ServiceClient.tsx` 作为 GUI 与服务之间的统一 JSON-RPC 调用封装，固定使用 `fetch("/rpc")` 以 HTTP POST 方式发送 JSON-RPC 请求；
   - **开发模式**：Vite 开发服务器通过 `vite.config.ts` 中的 `proxy["/rpc"]` 将所有 `/rpc` 请求代理到 `http://127.0.0.1:1234/rpc`，要求 `dev-service-api` 在该地址暴露 HTTP JSON-RPC 入口；
   - **打包后的 Tauri 应用**：前端仍通过相对路径 `fetch("/rpc")` 调用，由 Tauri 后端负责将 `/rpc` 请求转发到本机运行的 `surf-service` HTTP 端点（例如在应用启动时自动拉起 `surf-service` 子进程并监听配置中的 `<host>:<port>`）。

2. 配置与用户可见行为：
   - GUI 中的“JSON-RPC 服务器地址与端口”应直接映射到 `http://<host>:<port>/rpc`，默认值与服务模式 `--host` / `--port` 保持一致（`127.0.0.1:1234`）；
   - 当 `/rpc` 不可用时，`ServiceClient` 需要在状态栏/TopBar 中给出清晰的连接失败提示，并引导用户检查或启动本地 JSON-RPC 服务。

3. 对交付阶段的要求：
   - `release/gui/` 下的发布说明需包含“本地 JSON-RPC 服务依赖说明”，明确 GUI 默认期望的 HTTP 入口为 `http://127.0.0.1:1234/rpc`（或用户在设置中自定义的地址）；
   - 若发布包内附带 `surf-service` 二进制，建议在 GUI 中提供“自动启动本地服务”的开关，并在文档中说明其默认行为和可能的安全提示（仅本机监听、不对外暴露）。

4. 备用方案（未来可选扩展）：
   - 保留 `src-tauri/src/rpc_client.rs` 作为通过原始 TCP JSON-RPC 接入服务的备用路径；
   - 如未来决定采用“GUI → Tauri `invoke` → TCP 客户端 → 服务”的链路，则：
     - 在 Tauri 后端实现 `scan_start` / `scan_status` / `scan_result` / `scan_cancel` 等 `invoke` 命令，内部调用 `RpcClient` 完成 TCP JSON-RPC 通信；
     - 在前端 `ServiceClient.tsx` 中将 `fetch("/rpc")` 替换为对上述 `invoke` 命令的调用；
   - 当前迭代不要求落地该备用方案，仅在架构层面保留演进空间。

### 4.5 共享模型与配置 / 历史存储

**职责：**

1. 数据模型定义（逻辑层）：
   - `ScanRequest`：包含路径、线程数、最小文件大小、排除规则、时间分析开关等；
   - `ScanProgress`：包含任务状态、已扫描文件数、已遍历字节数、预计剩余时间等；
   - `ScanResult`：包含整体摘要、目录树快照、Top N 文件列表、文件类型统计、陈旧文件列表等；
   - `UserSettings`：语言、主题、默认路径、默认线程数、默认过滤大小、排除规则、JSON-RPC 服务器地址等；
   - `ScanHistoryEntry`：包含扫描时间、目标路径、总文件数、总大小等。

2. 存储抽象：
   - 初期可采用本地文件存储（例如基于配置目录中的 JSON/二进制文件或轻量级数据库）实现配置与历史记录的持久化；
   - 实际选型（纯 JSON 文件 vs SQLite 等）可在后续迭代中根据性能与复杂度进行权衡（该点可由人类进一步确认）。

3. 统一序列化约定：
   - 确保 CLI JSON 输出、JSON-RPC 返回、GUI 前端消费的数据结构保持一致；
   - 避免不同模块各自定义不兼容的字段命名与类型。

#### 4.5.1 配置文件路径与 JSON Schema（约定）

1. 配置文件路径约定（macOS 与 Linux）：
   - 全局配置文件统一存放在 `~/.config/surf/config.json`，适用于 macOS 与 Linux，遵循本项目自定义约定，避免因不同平台偏好目录差异导致配置分裂；
   - 当前版本不支持 Windows 平台，未来如扩展 Windows 时可在本节新增对应路径约定，但不改变现有 macOS/Linux 行为。

2. 核心字段与默认值（逻辑语义）：

   - `default_path: string`
     - 含义：命令行 / GUI 默认扫描路径；
     - 推荐默认值：GUI Onboarding 首次生成配置时预填为当前用户主目录（例如 `~/`）；
     - CLI 行为：若用户未通过 `--path` 指定路径且配置文件存在，可选择使用 `default_path` 作为默认扫描路径；如配置缺失或解析失败，CLI 仍保留 `.` 作为最终兜底默认（保证与 PRD 一致）。

   - `threads: number`
     - 含义：默认扫描线程数；
     - 默认值：逻辑 CPU 核心数（与 PRD 中 CLI 默认一致）；
     - 作用：
       - CLI：在用户未指定 `--threads` 时，可从配置读取该值；
       - 服务：在 `--threads` 未显式配置的情况下，可将其作为扫描任务的默认线程数；
       - GUI：作为 Onboarding 和设置页的默认线程建议值。

   - `min_size: string`
     - 含义：最小过滤大小，支持 `B/KB/MB/GB` 等后缀（与 CLI `--min-size` 语义一致）；
     - 默认值：`"0"`（表示不过滤），GUI Onboarding 可根据 UX 建议给出更高的默认值（例如 `"100MB"`），并写入该字段；
     - 作用：被 CLI、服务、GUI 作为 `--min-size` 或 JSON-RPC `min_size` 的默认来源。

   - `rpc_host: string`
     - 含义：JSON-RPC 服务默认监听地址；
     - 默认值：`"127.0.0.1"`；
     - 作用：
       - 服务模式：在未显式指定 `--host` 时，服务可从该字段读取默认监听地址；
       - GUI：作为默认连接目标；
       - 其他客户端：可复用该字段与 `rpc_port` 组合构造默认服务地址。

   - `rpc_port: number`
     - 含义：JSON-RPC 服务默认端口；
     - 默认值：`1234`（对齐 PRD）；
     - 作用：同 `rpc_host`，被服务、GUI、CLI 等作为默认端口使用。

   - `cli_path: string`（可选）
     - 含义：指向交付包内 CLI 可执行文件的路径，用于 GUI 中“一键启动 CLI”或对照运行；
     - 示例：`"/usr/local/bin/surf"` 或 DMG/应用包内的相对路径 `"cli/surf"`；
     - 作用：GUI 或其他工具可通过该字段找到 CLI 可执行文件并按配置拼装命令行参数。

   - `theme: string`（可选）
     - 取值范围：`"light"` / `"dark"`；
     - 含义：GUI 首选主题；
     - 默认行为：若字段缺失，GUI 视为“跟随系统主题”。

   - `language: string`（可选）
     - 取值范围：`"en"` / `"zh-CN"`；
     - 含义：GUI 首选界面语言；
     - 默认行为：若字段缺失，GUI 视为“跟随系统语言”。

   - `exclude_patterns: string[]`（可选，扩展字段）
     - 含义：排除目录/文件的模式集合，可使用 glob 或正则语法（与 JSON-RPC `exclude_patterns` 字段语义一致）；
     - 作用：
       - CLI：在用户未通过命令行显式传入排除规则时，可将该字段作为默认排除集；
       - 服务 / GUI：在创建扫描任务时，为 `ScanRequest` 提供默认排除模式。

   - `stale_days: number`（可选，扩展字段）
     - 含义：时间维度分析中“陈旧文件”判定的阈值天数；
     - 推荐默认值：例如 `365`（一年），可在 Onboarding 或设置中由用户调整；
     - 作用：用于核心引擎判断 `stale_files` 列表的时间阈值。

3. 跨模块配置使用与优先级规则（摘要）：

   - CLI / TUI：
     - 参数优先级：命令行参数 > 环境变量（如未来引入）> 配置文件 > 内建默认值；
     - 当配置文件存在时，`default_path` / `threads` / `min_size` 可作为默认值来源，但不得改变 PRD 规定的参数语义（即显式参数始终优先）。

   - JSON-RPC 服务：
     - 启动时优先解析命令行中的 `--host` / `--port` / 其他服务级参数；
     - 若未显式提供，则尝试从配置文件中读取 `rpc_host` / `rpc_port` 等字段；
     - 若配置缺失或不可用，则退回编译期默认（`127.0.0.1:1234`），确保服务仍可启动。

   - macOS GUI：
     - 启动时先读取配置文件并将字段映射到设置页和默认扫描参数；
     - 首次启动无配置时，通过 Onboarding 生成配置文件（见第 4.4.1 节）；
     - 在设置页修改配置后，通过共享模型模块负责将变更写回 `~/.config/surf/config.json`。

**不负责的内容：**

- 不决定 UI 展示细节；
- 不直接负责网络监听或命令行参数解析。

## 5. 典型数据流与端到端路径

### 5.1 CLI / TUI 单次扫描流程

1. 用户在终端执行 `surf --path <dir> [--threads N] [--min-size SIZE] [--limit N] [--json]`。
2. CLI/TUI 模块解析参数，构造 `ScanRequest` 并调用核心扫描与分析引擎。
3. 核心引擎并发扫描文件系统，持续向调用方报告 `ScanProgress`（供进度条或 TUI 状态栏展示）。
4. 扫描完成后，核心引擎返回 `ScanResult`：
   - CLI 模式：
     - 默认将结果格式化为表格打印到 stdout；
     - 若 `--json` 为 true，则输出 JSON 序列化后的 `ScanResult`；
   - TUI 模式：
     - 将 `ScanResult` 加载到内存，驱动目录树视图与列表视图；
     - 用户借助键盘继续浏览与筛选。

### 5.2 JSON-RPC 服务 + GUI 扫描流程

1. 用户通过 GUI 发起扫描：选择目录并设置参数。
2. GUI 通过 `ServiceClient` 将参数封装为 JSON-RPC 请求 `scan.start`，并以 HTTP POST 方式调用 `POST /rpc`：
   - 开发模式下，请求路径为 `fetch("/rpc")`，由 Vite 代理到 `http://127.0.0.1:1234/rpc`；
   - 打包后的 Tauri 应用中，`fetch("/rpc")` 由 Tauri 后端转发到本机运行的 `surf-service` HTTP 端点（`http://<host>:<port>/rpc`）。
3. 服务层创建新的扫描任务 ID，调用核心引擎异步执行扫描并立即返回 `task_id` 给 GUI。
4. GUI 周期性调用 `scan.status` 查询进度，更新进度条与状态栏，同时可允许用户取消任务（调用 `scan.cancel`）。
5. 扫描完成后，GUI 同样通过 `POST /rpc` 调用 `scan.result` 获取 `ScanResult`，用于渲染 Treemap 和列表视图。
6. 服务层可将 `ScanResult` 摘要写入历史存储，以供 GUI 列表展示与快速重跑。

> 备用链路说明：如未来切换到“GUI → Tauri `invoke` → TCP 客户端 → 服务”的方案，本节中的 HTTP 调用将由本机进程内的 TCP JSON-RPC 调用替代，但 `scan.*` 方法族与请求/返回结构保持不变。

### 5.3 删除操作（跨形态，一致策略）

1. **全局原则**
   - 采用「软删除优先」策略：在支持的情况下优先移动到回收站/废纸篓，仅在用户显式选择或环境不支持时执行永久删除；
   - 所有形态在执行删除前必须明确告知本次操作是「移至回收站/废纸篓」还是「永久删除」。

2. **平台语义**
   - **macOS**：
     - macOS GUI（Tauri）通过核心库的 `DeleteMode::MoveToTrash` 模式调用删除接口，对应系统废纸篓语义；
     - 初始版本不提供默认永久删除入口，如需永久删除必须通过额外选项（例如「按住修饰键 + 点击」或显式切换为“永久删除”模式）触发，并附带更强的二次确认文案。
   - **Linux**：
     - 操作系统本身没有统一 API，但常见桌面环境遵循 XDG Trash 规范；核心/服务层在 `mode = MoveToTrash` 时应按 XDG 规范最佳努力实现「移动到回收站」；
     - 若检测到环境不具备 Trash 条件（例如纯服务器环境），则退化为永久删除，删除结果中通过 `effective_mode = "permanent"` 标记，由上层 UI 告知用户「将被永久删除」。

3. **形态与确认流程**
   - **CLI**：
     - 当前版本 CLI 不直接暴露删除入口，仅提供扫描与结果展示能力，避免脚本化场景误删；
     - 如未来扩展 CLI 删除子命令，应遵循：显式子命令（如 `surf delete`）、默认软删除（如可用）、并要求 `--yes` / `--permanent` 等强提示参数后才执行，无隐式删除行为。
   - **TUI**：
     - 当前版本 TUI 仅用于扫描结果的浏览与分析，不提供任何删除入口，不会调用核心删除接口；
     - 如未来需要在 TUI 中引入删除能力，需重新在 `human.md` 中进行人类决策，并在本节更新具体确认流程与默认策略后方可开放。
   - **macOS GUI**：
     - 仅在 macOS GUI 中暴露「Move to Trash」入口，对应 `mode = MoveToTrash`；
     - 初始版本不提供 GUI 侧「永久删除」操作，避免与用户对 Finder 习惯相违背；如未来新增「永久删除」，必须通过单独菜单项 + 更严格二次确认实现。

4. **核心与服务接口契约**
   - 核心扫描引擎：
     - 提供 `delete_entry(path: &Path, options: DeleteOptions) -> DeleteResult` 接口；
     - `DeleteOptions` 关键字段：
       - `mode: "trash" | "permanent"`（必填或由调用方依策略填充）；
       - `origin: "cli" | "tui" | "gui" | "service"`；
       - `dry_run: bool`；
     - `DeleteResult` 至少包含：
       - `success: bool`；
       - `effective_mode: "trash" | "permanent"`；
       - `error: { code: i32, message: string }?`。
   - JSON-RPC 服务层：
     - 新增 `file.delete` 方法（示意）：
       - **请求参数（params 对象）：**
         - `path: string`：待删除的文件或目录路径；
         - `mode: string?`：`"trash" | "permanent"`，可选，缺省时由服务根据平台与配置选择默认策略；
         - `dry_run: bool?`：可选，若为 `true` 则仅做检查、返回是否可删除；
       - **返回结果（result 对象）：**
         - `success: bool`；
         - `effective_mode: "trash" | "permanent"`；
         - `error: { code: i32, message: string }?`。

5. **人类可配置项与未来可调整点**
   - Linux 平台上是否对所有调用 `mode = "trash"` 的前端统一启用 XDG Trash 流程，或仅在用户在配置中显式开启「使用回收站」时才走 Trash 流程；当前版本建议：交互式前端在 Linux 上默认采用「永久删除 + 强二次确认」的 UI 行为，用户可通过配置项（如 `linux.use_trash_by_default`）开启「尽可能移动到 Trash」；
   - 是否允许通过全局配置文件字段（如 `default_delete_mode`、`allow_permanent_delete`）对不同前端的默认删除模式进行细粒度覆盖（例如为 GUI 保留软删除，为未来可能出现的 CLI 删除命令启用强制 `--permanent` 显式声明）。

> 当前版本基于 `human.md` 的人类决策，固化的策略为：
> - macOS：仅 macOS GUI 暴露删除能力，统一采用「Move to Trash」作为默认删除动作，不暴露无保护的永久删除入口；TUI 不提供删除能力；
> - Linux：核心/服务层在收到 `mode = "trash"` 时按 XDG Trash 规范最佳努力实现「移动到回收站」，若失败则退化为永久删除并在 `effective_mode` 中标记；交互式前端在 Linux 上的默认 UI 建议为「永久删除 + 强提示」，用户可通过配置开启「尽可能移动到 Trash」；
> - CLI/TUI：本轮不提供删除能力，仅提供扫描与浏览能力，避免破坏现有脚本兼容性或造成误删；未来如调整由 `human.md` 中的人类决策驱动，并在本节同步更新。
## 6. 外部接口与数据契约（初稿）

### 6.1 CLI 参数族与输出约定

**CLI 参数（与 PRD 第 4 节对齐）：**

- `--path, -p`：扫描起始根目录，默认 `.`；
- `--threads, -t`：并发扫描线程数，默认逻辑核心数；
- `--min-size, -m`：过滤最小文件尺寸；
- `--limit, -n`：结果展示的最大条目数，默认 20；
- `--service, -s`：启动 JSON-RPC 服务模式（不进入单次扫描流程）；
- `--port` / `--host`：服务模式监听端口与地址；
- `--json`：单次模式下以 JSON 格式输出 `ScanResult`；
- `--help, -h`：显示帮助信息。

**CLI JSON 输出结构（示意）：**

> 说明：以下仅为字段级契约示意，具体类型与命名可在后续迭代中进一步细化。

```jsonc
{
  "summary": {
    "root_path": "string",
    "total_files": "u64",
    "total_dirs": "u64",
    "total_size_bytes": "u64"
  },
  "top_files": [
    {
      "path": "string",
      "size_bytes": "u64",
      "last_modified": "RFC3339-timestamp"
    }
  ],
  "by_extension": [
    {
      "extension": "string",
      "file_count": "u64",
      "total_size_bytes": "u64"
    }
  ],
  "stale_files": [
    {
      "path": "string",
      "size_bytes": "u64",
      "last_modified": "RFC3339-timestamp"
    }
  ]
}
```

CSV / HTML 导出格式可在后续设计中进一步补充，仅要求字段集合与 JSON 输出在语义上保持一致。

### 6.2 JSON-RPC 服务接口（方法族草案）

**基本约定：**

- 遵循 JSON-RPC 2.0 规范，所有请求包含 `jsonrpc`、`method`、`params`、`id` 字段；
- 错误响应使用标准 `error` 字段，`code`/`message` 语义在后续迭代中细化；
- 本节仅定义核心方法族，后续可扩展查询历史、配置管理等辅助接口。

- 传输层与 HTTP 封装：
  - 当前版本的 **主路径** 为 HTTP + JSON-RPC：客户端通过 `POST /rpc` 调用服务端，`Content-Type: application/json`，请求体为 JSON-RPC Request 对象，响应体为 JSON-RPC Response 对象；
  - HTTP 层的最小实现建议（面向 `dev-service-api`）：
    - 使用 `hyper` 或 `axum` 实现一个轻量级 HTTP 服务器，监听在 `http://<host>:<port>`（默认 `127.0.0.1:1234`）；
    - 为 `/rpc` 注册一个 POST 处理函数，将请求 body 读取为字节数组并交给已有的 JSON-RPC 调度函数，例如：`handle_jsonrpc(payload: &[u8]) -> Vec<u8>`；
    - 无论业务成功还是失败，HTTP 层通常返回 `200 OK`，错误信息通过 JSON-RPC 的 `error` 字段表达；仅在解析层面发生严重错误（如非 JSON 请求）时返回非 2xx 状态码；
    - HTTP 与（未来可能存在的）原始 TCP JSON-RPC 通道必须共享同一套方法族与数据结构，保证 GUI 与其他客户端在协议层面无感知差异。

- 原始 TCP JSON-RPC（兼容路径）：
  - 对于仍希望通过原始 TCP 通信的客户端，服务可以在同一进程内维护一个 TCP 监听；
  - 建议采用“按行分隔的 JSON 文本”或“长度前缀帧”作为 framing 协议，具体细节在 `dev-service-api` 的 README 中记录即可；
  - 所有 TCP 请求在进入核心调度前，与 HTTP 路径一样被解析为 JSON-RPC Request 对象，输出 JSON-RPC Response 对象。

#### 6.2.1 `scan.start`

- **用途**：创建新的扫描任务并异步执行。
- **请求参数（params 对象）：**
  - `path: string`：扫描根路径；
  - `threads: u16?`：可选，覆盖默认线程数；
  - `min_size: string?`：可选，带单位的最小文件大小（例如 `"100MB"`）；
  - `limit: u32?`：可选，Top N 文件条数；
  - `exclude_patterns: string[]?`：可选，排除目录/文件模式（支持正则或 glob）；
  - `stale_days: u32?`：可选，用于时间维度分析的阈值天数。
- **返回结果（result 对象）：**
  - `task_id: string`：服务内部唯一的任务 ID。

#### 6.2.2 `scan.status`

- **用途**：查询指定任务的当前状态与进度。
- **请求参数：**
  - `task_id: string`。
- **返回结果：**
  - `task_id: string`；
  - `state: string`：`queued` / `running` / `completed` / `canceled` / `failed`；
  - `progress: f64`：0.0–1.0；
  - `scanned_files: u64`；
  - `scanned_bytes: u64`；
  - `eta_seconds: u64?`：可选，预计剩余时间；
  - `error: { code: i32, message: string }?`：若任务失败时填充。

#### 6.2.3 `scan.result`

- **用途**：获取已完成任务的扫描结果。
- **请求参数：**
  - `task_id: string`。
- **返回结果：**
  - `task_id: string`；
  - `summary` / `top_files` / `by_extension` / `stale_files` 等结构，与 CLI JSON 输出保持一致；
  - （可选）`tree_snapshot`：用于 GUI 构建 Treemap 的目录树快照。

#### 6.2.4 `scan.cancel`

- **用途**：请求取消尚未完成的扫描任务。
- **请求参数：**
  - `task_id: string`。
- **返回结果：**
  - `task_id: string`；
  - `canceled: bool`：是否成功标记任务为已取消；
  - `reason: string?`：若取消失败，给出原因（例如任务已完成）。

> 以上方法族满足 PRD 中“至少提供启动扫描、查询进度、获取结果、取消任务”四类核心方法的要求，后续可在保持向后兼容的前提下扩展配置管理与历史查询相关方法。

## 7. 开发 Agent 拆分与工作区规划

> 本节从设计层面给出未来在开发阶段将采用的 `feature-developer` 子 Agent 拆分方案。具体调用与并行调度由编排 Agent 按 `AGENTS.md` 执行。

### 7.1 开发 Agent 列表

1. **`dev-core-scanner`**
   - **工作区路径（规划）**：`workspaces/dev-core-scanner/`
   - **负责范围：**
     - 实现“核心扫描与分析引擎”（见第 4.1 节）；
     - 定义并维护共享数据模型（`ScanRequest` / `ScanProgress` / `ScanResult` 等逻辑结构）；
     - 提供稳定的库级接口供 CLI/TUI 与服务模块调用；
     - 初步支持文件类型统计、Top N 文件、陈旧文件识别等能力。
   - **主要输入文档：**
     - `PRD.md` 第 3.1、3.3、3.4、5、6 节；
     - 本文件第 3、4.1、4.5、5、6.1 节。
   - **预期构建产物（交付阶段使用）：**
     - 类型：Rust 库（静态或动态），供同一交付工作区内其它二进制链接或复用；
     - 工作区内建议产物路径示例：`target/release/libsurf_core.*`（具体文件名由实际 crate 决定）。
   - **本轮完成判定（对单次迭代而言）：**
     - 提供稳定的扫描/分析库接口，覆盖 PRD 规定的基本统计能力；
     - 在本工作区内有自测说明（例如 `todo.md` / `README` 等），证明在代表性目录上完成了性能与正确性自查。

2. **`dev-cli-tui`**
   - **工作区路径（规划）**：`workspaces/dev-cli-tui/`
   - **负责范围：**
     - 实现命令行单次模式与 TUI 模式的二进制程序；
     - 参数解析与校验（对齐 `PRD.md` 第 4 节）；
     - 进度条渲染、表格结果输出、JSON 输出；
     - TUI 导航与交互浏览流程（不包含删除操作）；
   - **主要输入文档：**
     - `PRD.md` 第 3.2.1、3.4、4、7、8 节；
     - 本文件第 3、4.3、5.1、6.1 节。
   - **预期构建产物：**
     - 类型：CLI/TUI 可执行二进制；
     - 工作区内建议产物路径示例：`target/release/surf`。
   - **本轮完成判定：**
     - 支持在 Linux/macOS 上以 CLI/TUI 方式完成一次扫描，并满足 PRD 的基本验收标准（性能与参数行为）；
     - 在工作区内记录自测命令与结果。

3. **`dev-service-api`**
   - **工作区路径（规划）**：`workspaces/dev-service-api/`
   - **负责范围：**
     - 实现 JSON-RPC 2.0 服务进程，负责任务管理与核心库的调用；
     - 实现本文件第 6.2 节定义的 `scan.start` / `scan.status` / `scan.result` / `scan.cancel` 方法族；
     - 管理任务并发与资源限制，确保高并发下服务稳定；
     - 为 GUI 提供必要的历史与配置访问接口（可在后续迭代中扩展）。
   - **主要输入文档：**
     - `PRD.md` 第 3.2.2、3.3、5、6、8 节；
     - 本文件第 3、4.2、4.5、5.2、6.2 节。
   - **预期构建产物：**
     - 类型：长运行服务二进制；
     - 工作区内建议产物路径示例：`target/release/surf-service`。
   - **本轮完成判定：**
     - 能稳定启动在 `--host`/`--port` 指定地址，并通过基本 JSON-RPC 测试用例验证核心方法；
     - 在工作区内记录调用示例和自测结果。

4. **`dev-macos-gui`**
   - **工作区路径（规划）**：`workspaces/dev-macos-gui/`
   - **负责范围：**
     - 实现基于 Tauri + React 的 macOS GUI 应用；
     - Onboarding 流程（权限申请、默认配置）；
     - 主界面布局与 Treemap/列表视图；
     - 与 JSON-RPC 服务的集成（启动本地 server 或连接远程 server）；
     - GUI 内的配置与扫描历史管理。
   - **主要输入文档：**
     - `PRD.md` 第 3.2.3、3.3、3.4、5、6、8 节；
     - 本文件第 3、4.4、4.5、5.2、6.2 节。
   - **预期构建产物：**
     - 类型：macOS 应用包和/或压缩包；
     - 工作区内建议产物路径示例：
       - `src-tauri/target/release/bundle/macos/Surf.app`；
       - 或打包后的 `dist/Surf-macos-universal.zip` 等。
   - **本轮完成判定：**
     - 在本机可启动 GUI，完成一个端到端扫描流程（含 Onboarding），并通过 GUI 看到 Treemap 与列表视图；
     - 在工作区内记录构建和运行步骤、自测结果。

### 7.2 Agent 拆分的一致性原则

1. 各开发 Agent 的职责与本文件第 4–6 节的模块与接口设计一一对应，不引入额外“悬空能力”。
2. 同一功能点（例如扫描统计、JSON-RPC 方法、Treemap 渲染）在架构设计中有唯一归属：
   - 计算逻辑归 `dev-core-scanner`；
   - 网络接口归 `dev-service-api`；
   - 命令行交互归 `dev-cli-tui`；
   - 桌面交互归 `dev-macos-gui`。
3. 当所有开发 Agent 按各自职责完成实现并通过交付阶段的统一构建与测试后，应自然拼接成 PRD 所要求的一条或多条端到端使用路径。

## 8. 交付/构建视图与 release 目录规划

> 本节面向未来的交付节点（如 `delivery-runner`），说明如何在独立交付工作区内从各开发工作区收集产物并组装最终发布包。具体路径可在实现阶段根据工具链调整，但原则应保持不变。

### 8.1 交付工作区与输入

1. 交付工作区建议路径：`workspaces/delivery-runner/`（具体命名可由编排 Agent 与交付节点在实现时确定）。
2. 交付节点从以下工作区收集构建产物：
   - `workspaces/dev-core-scanner/target/release/libsurf_core.*`（或等价核心库）；
   - `workspaces/dev-cli-tui/target/release/surf`（CLI/TUI 二进制）；
   - `workspaces/dev-service-api/target/release/surf-service`（JSON-RPC 服务二进制）；
   - `workspaces/dev-macos-gui/src-tauri/target/release/bundle/macos/Surf.app` 及可能的 `dist/` 打包文件。

### 8.2 release 目录结构（交付工作区内）

在交付工作区内，`release/` 目录建议组织为：

- `release/cli/`
  - `surf-<platform>-<arch>`：面向终端用户的 CLI/TUI 可执行文件（例如 `surf-macos-arm64`、`surf-linux-x86_64`）。

- `release/service/`
  - `surf-service-<platform>-<arch>`：JSON-RPC 服务二进制。

- `release/gui/`
  - `Surf.app`：macOS 应用包（可直接拖入应用程序目录）；
  - 其他用于 GUI 分发的打包形式（例如 `Surf-macos-universal.zip`），通常仅包含 GUI 自身及必要的运行时资源。

- `release/installer/`
  - `Surf-macos.dmg` 或 `Surf-macos-universal.dmg`：面向最终用户的 macOS 安装镜像，统一包含：
    - `Surf.app`：GUI 应用包；
    - `cli/surf`：CLI 二进制（可为实际文件或指向 `release/cli/` 中对应二进制的符号链接）；
    - `README.txt` 或等价的最小使用说明：简要说明 DMG 中同时包含 GUI 与 CLI 入口、推荐的安装步骤（例如将 `Surf.app` 拖入“应用程序”目录，可选地将 `cli/surf` 拖入 `/usr/local/bin`）。

- `release/metadata/`
  - 版本信息、构建信息和最小使用说明（例如运行 CLI 或启动服务/GUI 的命令示例）。

> 说明：
> - 当前根仓库未包含具体构建脚本，交付阶段可在交付工作区内按上述结构新增构建脚本与说明文件，不要求在本仓库根目录直接添加新的构建脚本；
> - macOS 安装包（DMG）的构建由交付工作区脚本负责，典型流程为：在临时目录中布置 `Surf.app`、`cli/surf` 与 `README`，然后基于 macOS 自带的 `hdiutil` 等工具创建只读 DMG；
> - 构建环境依赖：需要在 macOS 上执行，通常要求安装 Xcode Command Line Tools；
> - 签名与未签名差异：
>   - 未签名 DMG 在首次运行时可能触发 Gatekeeper 提示，需要用户通过“仍要打开”流程确认；
>   - 如需提供已签名/公证的 DMG，则应在交付脚本中增加基于 Apple Developer ID 的签名与公证步骤，但这属于发行流程扩展，不改变本节产物路径与内容约定；
> - 从 DMG 安装后的预期行为：用户将 `Surf.app` 拖入“应用程序”后，GUI 启动时默认读取 `~/.config/surf/config.json`；若该文件不存在，则按第 4.4.1 节所述创建默认配置并进入 Onboarding 流程；同时，DMG 中的 `cli/surf` 在被复制到系统 PATH 后应可直接运行，并与同一配置文件协同工作（例如复用 `default_path` / `threads` / `min_size` 等默认值）。

### 8.3 交付阶段内的测试视图（与 PRD 对齐）

1. 交付节点基于 `PRD.md` 在交付工作区的 `test/` 目录下维护：
   - `test/case.md`：覆盖 CLI、服务模式、TUI、GUI 的代表性验收用例；
   - 自动/半自动测试脚本，用于验证大目录扫描性能、并发任务稳定性、删除操作确认流程等。
2. 测试运行针对从 `release/` 目录获取的产物，不直接修改各开发工作区。
3. 若测试失败，交付节点在 `test/failures.md` 中记录失败用例与上下文，交由编排 Agent 广播给各开发 Agent，以决定是否回退到设计或需求阶段。

## 9. 本轮设计范围与后续扩展

**本轮已覆盖：**

- 基于 PRD 的整体模块划分与职责说明；
- CLI、JSON-RPC 服务、GUI 之间的典型数据流与调用链；
- 初步的 JSON-RPC 方法族与 CLI JSON 输出契约；
- 面向多开发 Agent 并行开发的工作区划分与构建产物规划；
- 交付阶段的 release 目录结构与测试视图骨架。

**本轮尚未细化（留待后续迭代或需要人类确认）：**

1. 详细错误码体系与跨模块统一的错误语义；
2. 删除操作在未来可能新增的形态（例如 Linux 桌面 GUI 或支持删除的 CLI/TUI 模式）上的扩展策略（当前仅 macOS GUI 支持删除，基础策略已在第 5.3 节结合 `human.md` 决策固化，仅对未来扩展保留空间）；
3. 扫描历史与配置的具体持久化选型（纯文件 vs SQLite 等）；
4. TUI 交互细节（具体按键绑定、视图切换方式等）；
5. GUI 内 Treemap 与列表视图的具体视觉规范与交互微细节。

后续迭代可在本初稿基础上，按模块/章节逐步补充实现级设计（例如关键数据结构字段列表、错误码表、具体导出格式等），同时根据开发与交付阶段的反馈进行调整。

## 10. 工具链与构建环境要求（补充）

> 本节旨在将当前已知的工具链与构建环境前置条件集中记录，便于开发/交付节点及人类协作者在本机或 CI 环境中准备一致的构建基础。以下约束均为**实现层面的环境要求**，不改变已有的外部接口契约（包括 HTTP `/rpc` 主路径）。

### 10.1 Rust 工具链版本约束（特别是 macOS GUI / Tauri）

1. 全局约定：
   - 项目整体后端与核心模块使用 **Rust Edition 2021**（见第 3 节），各工作区在无特殊说明时默认依赖稳定版 Rust 工具链（stable channel）。

2. macOS GUI / Tauri 后端最小版本要求：
   - 依据 `workspaces/dev-macos-gui/` 当前依赖链（包含 Tauri 及其间接依赖 `time` crate），Tauri 后端的编译在 `rustc 1.86.0` 下无法通过，错误原因是依赖链要求 **`rustc >= 1.88.0`**；
   - 为保障 macOS GUI 工作区可以完成 **Tauri 后端编译、自测与打包**，本架构设计将：
     - **`rustc 1.88.0 及以上` 视为 macOS GUI / Tauri 相关 crate 的最低版本要求；**
     - 推荐在同一开发/交付环境中，将所有 Rust 工作区（`dev-core-scanner`、`dev-cli-tui`、`dev-service-api`、`dev-macos-gui`）统一使用 `rustc >= 1.88.0` 的稳定版工具链，以降低版本漂移带来的隐性兼容风险。

3. 对当前阻塞的判定：
   - 在仅具备 `rustc 1.86.0` 的环境中：
     - `dev-core-scanner`、`dev-cli-tui`、`dev-service-api` 等工作区已经能够完成构建与自测（见各自 `todo.md`）；
     - `dev-macos-gui` 的 **Tauri 后端** 编译与 `cargo check --manifest-path src-tauri/Cargo.toml` 仍会因 Rust 版本不足而失败，导致无法在本机完成 GUI 形态的端到端运行与验收；
   - 因此，本问题被归类为：
     - **环境/工具链依赖不足，而非架构或接口设计缺陷**；
     - 一旦目标环境升级至 `rustc >= 1.88.0`，现有 `Architecture.md` 中关于 GUI 与服务之间通过 HTTP `/rpc` 路径集成的设计即可按既定方案落地，无需调整接口契约。

4. 对后续开发与交付节点的建议：
   - 在为 macOS GUI 相关工作（包括本地开发、自测与交付构建）准备环境时，应显式执行：
     - `rustup update stable`，并确认 `rustc --version` 输出中的版本号不低于 `1.88.0`；
   - 若 CI 或交付机上使用固定版本的 Rust 工具链，应将 `1.88.0` 或更高版本作为镜像/环境配置的一部分，并在交付工作区的构建脚本或 README 中补充说明（该补充属于交付阶段文档，不改变本文件的接口设计）。

### 10.2 macOS DMG 构建环境前置条件

> 本小节在第 8.2 节的基础上，将 macOS 安装镜像（DMG）构建所需的环境前置条件集中列出，仅作为对交付节点与人类协作者的补充说明，不改变 `release/` 目录结构与已有交付视图约定。

1. 平台与系统工具：
   - DMG 构建必须在 **macOS 环境** 中进行，支持 Intel 与 Apple Silicon；
   - 需要安装 **Xcode Command Line Tools**，以便获得 `clang`、`codesign` 等基础工具链；
   - 依赖系统自带的 **`hdiutil`** 工具创建只读 DMG 镜像（第 8.2 节已给出典型脚本流程，本处仅重申其为环境前置条件）。

2. 相关构建链路依赖（摘要）：
   - 在交付工作区内构建 macOS GUI 相关产物时，通常需要：
     - Rust 稳定版工具链 `rustc >= 1.88.0`（用于编译 Tauri 后端与 `surf-service` 等 Rust 二进制）；
     - Node.js / npm（或等价工具）以执行 `npm run build`、`npm run tauri:build` 等前端/Tauri 打包命令（具体命令由 `workspaces/dev-macos-gui/` 中的实现与文档决定）；
   - 这些依赖用于生成：
     - `Surf.app`（Tauri 打包生成的 macOS 应用包）；
     - 基于 `Surf.app` 与 CLI 二进制在交付工作区组合出来的 DMG（`release/installer/Surf-macos*.dmg`）。

3. 与 HTTP `/rpc` 主路径的关系：
   - 本节所述工具链与构建环境要求仅影响：
     - GUI 与服务二进制是否能够在目标环境中成功构建与打包；
     - DMG 是否能够在交付阶段顺利产出。
   - **不改变** 已在第 3、4.2、4.4.2、5.2 与 6.2 节中约定的对外接口契约：
     - JSON-RPC 服务仍以 `POST /rpc` 形式在 `http://<host>:<port>/rpc`（默认 `http://127.0.0.1:1234/rpc`）暴露；
     - macOS GUI 在开发模式与打包后形态中，继续通过 `fetch("/rpc")` 调用该 HTTP 入口（开发模式由 Vite 代理到 `127.0.0.1:1234/rpc`，打包后由 Tauri 后端转发到本机运行的服务）；
   - 换言之：
     - 若环境暂时无法满足本节工具链要求，GUI 形态的构建与本地端到端自测会受阻；
     - 但服务接口与 CLI/TUI 形态的对外行为与路径约定保持不变，后续在环境满足要求后可按既定架构自然完成 GUI 与服务的拼接。
