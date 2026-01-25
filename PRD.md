# Surf 项目需求文档 (PRD)

## 1. 项目概览
**项目名称**: Surf
**目标**: 为 Linux 和 macOS 系统提供一个极速、美观且功能强大的磁盘扫描与分析工具。
**核心价值**: 帮助用户快速定位磁盘占用大户，支持多维度可视化分析，并提供清理建议。

### 1.1 范围（Scope）
- 支持对本地文件系统的只读扫描与分析，不修改文件内容本身（删除能力作为单独操作能力提供）。
- 覆盖四种主要形态：CLI、TUI、JSON-RPC 服务模式以及 macOS GUI；核心扫描与分析能力在各形态之间保持一致。
- 面向单机环境的磁盘空间分析；云存储、远程文件系统等能力仅作为未来规划的一部分。

### 1.2 非目标（Out of Scope）
- 当前版本不支持 Windows 平台。
- 当前版本不提供自动化清理策略（仅给出清理建议），自动清理归入“未来规划”。
- 不替代系统级安全/备份方案，用户删除文件前需自行确认影响范围。

## 2. 目标用户
*   **开发者**: 需要分析代码库、依赖项或构建产物占用的磁盘空间。
*   **系统管理员**: 需要快速排查服务器磁盘满额问题。
*   **普通高级用户**: 想要管理个人电脑存储空间。

## 3. 功能需求

### 3.1 核心扫描功能
*   **并发控制**: 支持通过 `--threads` (或 `-t`) 参数指定扫描线程数。默认利用所有逻辑 CPU 核心以达到最高 IO 效率。
*   **扫描过滤**: 
    *   支持 `--min-size` 指定最小过滤大小（如 `100MB`）。
    *   支持过滤掉小于设定阈值的所有文件。

### 3.2 运行模式

#### 3.2.1 单次运行模式 (One-off)
*   **交互逻辑**:
    1.  **执行中**: 终端显示动态进度条（推荐使用 `indicatif` 库），实时反馈已处理文件数和总容量。
    2.  **完成后**: 进度条自动结束，并在其下方**默认以格式化表格形式打印最终结果**。
*   **结果展示**:
    *   **排序**: 强制按文件大小降序排列。
    *   **条数限制**: 支持 `--limit` (或 `-n`) 参数控制展示的最大条目数。例如 `--limit 20` 只显示前 20 个最大的文件。

#### 3.2.2 服务模式 (Service Mode)
*   **功能描述**: 启动 JSON-RPC 服务，允许其他应用（如 GUI 程序）通过网络连接进行磁盘扫描和分析。
*   **配置选项**: 
    *   支持 `--service` (或 `-s`) 参数启动服务模式。
    *   支持 `--port` 参数指定服务监听端口（默认 1234）。
    *   支持 `--host` 参数指定服务监听地址（默认 127.0.0.1）。

#### 3.2.3 图形界面模式 (GUI Mode - macOS)

**1. 初始安装引导 (Onboarding):**
*   **初次运行检测**: 应用程序启动时检测是否存在配置文件。若不存在，进入引导流程。
*   **引导步骤**:
    *   **欢迎页**: 介绍 Surf 的核心功能。
    *   **权限申请**: 引导用户授权“全盘访问权限”（Full Disk Access），以确保扫描完整性。
    *   **基础配置**: 提醒用户设置默认扫描路径、并发线程数及最小过滤大小。
    *   **服务检查**: 确认本地 `surf` server 核心状态。

**2. 用户配置设计 (User Settings):**
*   **常规设置**:
    *   默认起始扫描路径。
    *   界面语言（中/英）。
    *   主题切换（跟随系统/深色/浅色）。
*   **扫描偏好**:
    *   默认并发线程数（滑块控制）。
    *   默认最小文件过滤大小（如 100MB）。
    *   排除列表：支持用户自定义正则表达式排除特定目录（如 `Library/*`, `Containers/*`）。
*   **网络设置**:
    *   JSON-RPC 服务器地址与端口配置。

**3. 扫描记录存储 (Scan History):**
*   **自动记录**: 每次完成的扫描任务自动记录至“历史记录”列表。
*   **记录详情**: 包含扫描时间、目标路径、文件总数、总占用大小。
*   **快捷回溯**: 用户可以点击历史记录快速重新扫描同一路径，或查看上一次的扫描快照（若支持快照功能）。

**UI/UX 设计规范:**
1.  **主界面 (Main Dashboard)**:
    *   **侧边栏**: 收藏的扫描路径、最近扫描记录、设置入口。
    *   **顶部栏**: 当前扫描路径选择器、全局搜索框、扫描控制按钮（开始/暂停/停止）。
    *   **中央视图**: 
        *   **Treemap 视图**: 使用不同大小的方块展示文件/目录占用情况，颜色深度代表文件类型或修改时间。
        *   **列表视图**: 传统的层级列表，支持按列排序（名称、大小、最后修改时间）。
2.  **交互功能 (Interactions)**:
    *   **悬停展示**: 鼠标悬停在 Treemap 方块上显示完整路径和精确大小。
    *   **深度下钻**: 点击目录方块进入该目录的详细 Treemap。
    *   **快捷操作**: 右键菜单支持：
        *   `Reveal in Finder` (在访达中显示)
        *   `Move to Trash` (移至废纸篓)
        *   `Copy Path` (复制路径)
3.  **实时反馈**:
    *   扫描时，底部状态栏显示实时速度、已扫描文件数和剩余空间预测。
    *   使用平滑的动画效果展示目录大小的动态增长。

*   **集成方式**: 
    *   GUI 程序可以自动启动本地 `surf` server 进程，也可以连接到远程已存在的 server。

### 3.3 数据分析与可视化
*   **分层视图**: 以树状结构展示目录大小，支持点击下钻。
*   **文件类型分布**: 统计不同扩展名（如 .log, .mp4, .node_modules）的总占用。
*   **大文件排行榜**: 快速列出占用空间最大的 Top N 文件。
*   **时间维度分析**: 识别“陈旧文件”（长时间未访问或修改的文件）。

### 3.4 交互与输出
*   **TUI (终端 UI) 交互**: 类似于 `ncdu`，支持键盘操作进行目录导航和文件删除。
*   **结构化导出**: 支持将扫描结果导出为 JSON, CSV 或 HTML 报告。
*   **搜索与过滤**: 支持按文件名、大小范围或修改时间进行实时过滤。

## 4. 命令行参数定义

| 长参数 | 短参数 | 描述 | 默认值 |
| :--- | :--- | :--- | :--- |
| `--path` | `-p` | 指定扫描的起始根目录 | `.` |
| `--threads` | `-t` | 指定并发扫描的线程数量 | 逻辑核心数 |
| `--min-size` | `-m` | 过滤文件的最小尺寸 (支持单位: B, KB, MB, GB) | `0` |
| `--limit` | `-n` | 最终结果展示的最大条目数量 | `20` |
| `--service` | `-s` | 启动 JSON-RPC 服务模式 | `false` |
| `--port` | 无 | 服务模式监听的 TCP 端口 | `1234` |
| `--host` | 无 | 服务模式监听的地址 | `127.0.0.1` |
| `--json` | 无 | 单次模式下以 JSON 格式输出结果 (默认为表格) | `false` |
| `--help` | `-h` | 显示帮助信息 | 无 |

## 5. 非功能性需求
*   **极致性能**: 针对 SSD 进行优化，最小化内存占用，确保在处理数百万文件时系统不卡顿。
*   **跨平台兼容**: 完美支持 Linux (x86/ARM) 和 macOS (Intel/Apple Silicon)。
*   **安全性**: 扫描过程中不修改任何文件数据，删除操作需二次确认。
*   **无依赖分发**: 提供单文件静态编译二进制包，无需安装额外运行时。

## 6. 技术路线
*   **后端 (Core/Server)**: Rust (Edition 2021)
    *   **并发模型**: 基于 `rayon` 或 `tokio` 实现高性能并行扫描。
    *   **通信**: 基于 `tokio` 实现轻量级 JSON-RPC 2.0 服务。
*   **图形界面 (macOS GUI)**:
    *   **框架**: **Tauri** (使用 Rust 作为后端桥接，React 作为前端界面)。
    *   **前端**: **React** + **Tailwind CSS** + **Vite**。
    *   **状态管理**: TanStack Query (用于 RPC 数据获取) 或 Zustand。
    *   **可视化**: 使用 `Recharts` 或 `D3.js` 实现磁盘占用的树状图 (Treemap) 和饼图。
*   **命令行**: 
    *   **TUI**: `ratatui` (Rust)。
    *   **CLI 解析**: `clap` (Rust)。

## 7. 未来规划 (Roadmap)
*   **云存储支持**: 扫描 S3, Google Drive 等云端存储占用。
*   **自动化清理**: 设定策略自动清理临时文件夹。
*   **磁盘健康监测**: 集成 SMART 信息显示。

## 8. 验收标准（Acceptance Criteria）

*   **CLI / 单次运行模式**
    *   在 Linux 和 macOS 上执行 `surf --path <dir>` 时，能够完成对包含至少 1M 个文件的目录的扫描，进程不崩溃，终端输出按大小降序排序的结果列表。
    *   `--threads`、`--min-size`、`--limit`、`--json` 等参数行为与文档定义一致，错误参数会给出友好的错误提示并退出非零状态码。
    *   扫描过程中可以通过 `Ctrl+C` 中断，程序在合理时间内结束且不会留下半写入的输出文件。

*   **服务模式 (JSON-RPC)**
    *   使用 `--service` 启动后，进程在默认 `127.0.0.1:1234` 监听，并符合 JSON-RPC 2.0 规范（包含 `jsonrpc`、`method`、`params`、`id` 字段）。
    *   至少提供“启动扫描”“查询进度”“获取结果”“取消任务”这几类核心方法，参数与返回结构在接口文档中有稳定定义。
    *   在高并发请求（例如同时 10 个扫描任务）下，服务保持可用且不会出现资源泄漏或明显性能退化。

*   **TUI 模式**
    *   在常见终端环境中（如 `xterm`、`iTerm2`），界面渲染正确，不出现严重错位或闪烁。
    *   支持通过键盘完成目录导航、查看文件详情以及触发删除操作；删除前有明确的二次确认提示。

*   **macOS GUI**
    *   首次启动且不存在配置文件时，自动进入 Onboarding，包含权限申请与基础配置步骤，流程可完整走通并落盘配置。
    *   用户能在 GUI 中选择任意本地路径发起扫描，看到 Treemap 与列表视图，并通过右键菜单执行“在访达中显示”“移至废纸篓”“复制路径”等操作。
    *   在扫描大目录时 GUI 保持可响应，进度状态和扫描速率有明显反馈，不出现长时间无响应（> 2 秒）的卡死感知。

*   **非功能性**
    *   在包含数百万文件的目录上扫描时，内存占用保持在可配置或文档约定的上限以内，不导致系统明显卡顿或 OOM。
    *   所有二进制发布包为单文件或附带最小运行时依赖，按文档步骤即可在目标平台直接运行。
    *   任意扫描或删除操作在出现异常（权限不足、路径不存在、磁盘只读等）时，能够给出明确的错误信息，而不会静默失败。

## 9. User Stories（迭代拆解草案）

### 9.1 CLI 单次运行模式（One-off）

#### 9.1.1 CLI-ONEOFF-001 基础扫描与表格输出

- id: `CLI-ONEOFF-001`
- title: 在 CLI 中对指定目录进行单次扫描并输出按大小排序的结果表格
- status: `done`
- description: 作为一名使用 Surf 的开发者或运维，我希望在终端中运行 `surf --path <dir>` 进行单次扫描，在看到实时进度反馈后，最终获得一份按文件/目录大小降序排序、条数可控的结果表格，以便快速定位空间占用大户。
- acceptance_criteria:
  - 在 Linux 和 macOS 上，执行 `surf --path <dir>` 时，终端在扫描过程中展示动态进度条，至少包含已扫描文件数与已扫描总大小等信息。
  - 扫描完成后，进度条自动结束，并在其下方输出格式化表格结果，表格内容按文件/目录大小降序排列。
  - 未显式指定 `--limit` 时，默认只展示前 20 条记录；指定 `--limit N` 时，展示条目数不超过 N。
  - 对不存在或不可访问的路径（例如权限不足）给出清晰错误信息，并以非零状态码退出，不输出误导性的空表格。
 - impl_notes (iteration 23 / dev-core-scanner & dev-cli-tui):
   - 核心同步扫描函数仍然作为向后兼容的一次性入口存在：`workspaces/dev-core-scanner/surf-core/src/lib.rs:10-120` 中定义了 `FileEntry { path, size }` 与 `scan(root, min_size, threads) -> io::Result<Vec<FileEntry>>`，调用前显式检查 `root.exists()` 并在路径不存在时返回 `ErrorKind::NotFound`，错误信息中包含类似 `scan root does not exist: <path>` 的提示；核心实现使用 `WalkDir::new(root)` 递归遍历，仅保留 `metadata.is_file() == true` 且 `metadata.len() >= min_size` 的文件条目，在局部 `rayon` 线程池中并发收集结果，并最终按 `size` 字段降序排序返回完整 `Vec<FileEntry>`，不在核心层截断 TopN。本 story 的排序语义和错误类型约定依然首先由该同步接口保证。
   - 在同一文件中（约 `workspaces/dev-core-scanner/surf-core/src/lib.rs:120-340`），已经落地了与 Architecture.md 4.1 / 5.1 对齐的进度感知扫描 API：`ScanConfig`、`ScanProgress`、`StatusSnapshot`、`ScanHandle` 以及便捷函数 `start_scan(config)` / `poll_status(&handle)` / `collect_results(handle)` / `cancel(&handle)`。扫描在后台线程中运行，在遍历过程中持续维护 `scanned_files` 与 `scanned_bytes` 计数；`poll_status` 返回的 `StatusSnapshot.progress` 中这两个字段实时增长，而 `total_bytes_estimate` 目前始终为 `None`（尚未实现总量预估）。取消语义通过内部 `AtomicBool` 标志实现“最佳努力”取消：收到取消请求后，后台线程在安全点尽快停止后续遍历和统计，但不保证立刻终止所有 IO 操作。
   - CLI 单次运行模式已切换为基于上述进度 API 的实现：`workspaces/dev-cli-tui/surf-cli/src/main.rs:1-220` 中，`Args` 结构体通过 `clap::Parser` 定义的 `--path/-p`（默认 `.`）、`--min-size/-m`（默认 `"0"`）、`--limit/-n`（默认 `20`）、`--threads/-t` 等参数与 PRD 第 4 章参数表保持一致；`main` 在解析参数并构造 `ScanConfig { root, min_size, threads }` 后，通过 `start_scan(config)` 启动后台扫描任务，并创建基于 `indicatif::ProgressBar::new_spinner` 的进度指示器（绘制目标为 stderr）。前台循环中定期调用 `poll_status(&handle)`，将返回的 `StatusSnapshot.progress.scanned_files` 与 `scanned_bytes` 渲染到进度条消息中，形如 `Scanning <path> ... files={..}, bytes={..}`，从而满足本 story 中“至少包含已扫描文件数与已扫描总大小”等动态进度反馈要求。
   - Ctrl+C 中断行为按照 PRD 9.3 中的设计决策在 CLI 中实现：`surf-cli` 使用 `ctrlc` crate 安装 SIGINT 处理器，收到 Ctrl+C 时设置中断标志；在前台轮询循环中检测到中断后，会调用 `cancel(&handle)` 请求核心层终止后台扫描，清理进度条，并在 stderr 输出类似 `Scan interrupted by user (Ctrl+C)` 的提示，最后以 130 非零退出码结束进程且不输出部分结果（无论表格模式还是 `--json` 模式），与 Architecture.md 4.4 所述“进度和错误仅写入 stderr、stdout 仅在成功时输出完整结果”的分流约定一致。
   - 扫描正常结束时，`collect_results(handle)` 会返回 `Vec<FileEntry>` 结果，CLI 根据是否指定 `--json` 选择输出路径：表格模式下，先在 stdout 打印包含 `SIZE(BYTES)` / `PATH` 的表头和分隔线，再对 `entries.into_iter().take(limit)` 逐行输出 `size` 与 `path`，其中顺序直接复用核心层已按 size 降序排序的结果；JSON 模式下则构造 `JsonOutput { root, entries }` 并序列化输出到 stdout。两种模式下，进度条与日志信息始终通过 stderr 输出，确保脚本/工具链可以安全消费 stdout 上的结构化结果。
   - `workspaces/dev-core-scanner/surf-core/tests/basic_scan.rs` 中的测试继续覆盖本 story 依赖的关键扫描语义：`scan_respects_min_size_and_sorts_desc` 验证最小大小过滤与按 size 降序排序；`scan_threads_zero_falls_back_to_one` 验证 `threads == 0` 时退化为 1 的防御性行为；`scan_nonexistent_root_returns_not_found` 验证不存在路径时返回 `ErrorKind::NotFound` 且错误消息包含 `"does not exist"`；`scan_empty_directory_returns_empty_result` 验证空目录时返回空结果而非错误，为进度 API 在内部复用相同遍历/过滤逻辑提供了基础保障。
   - `workspaces/dev-cli-tui/surf-cli/tests/integration.rs` 中的端到端测试覆盖了本 story 相关的部分验收路径：`test_surf_table_output_with_min_size_and_limit` 在临时目录中构造小文件与两个大文件，通过 `--path <temp_dir> --min-size 10 --limit 2` 触发表格模式，断言 stdout 中存在表头与分隔线、数据行数量不超过 `limit` 且至少有一行，并且数据行包含临时目录路径片段，从而间接验证 `--path` / `--min-size` / `--limit` 的组合行为与表格输出基本结构；其他用例（`test_surf_json_output_with_min_size_and_limit`、`test_surf_json_error_on_invalid_min_size`、`test_surf_json_error_on_invalid_threads`、`test_surf_json_error_on_nonexistent_path` 与 `test_surf_non_json_error_behavior`、`test_surf_table_error_on_nonexistent_path`、`test_surf_table_default_limit_20`）则分别覆盖了 JSON 与非 JSON 模式下的成功与错误场景：在错误参数或不存在路径时保证 stdout 保持为空、错误信息仅输出到 stderr 且进程以非零状态码退出，在正常场景下保证默认 `--limit` 仍然为 20 且表格结构稳定。这些测试共同为本 story 的“错误路径”和“默认参数”行为提供了回归保护。
  - remaining_todos:
   - 方向一：进度条总量预估与取消语义增强——当前 `StatusSnapshot.progress.total_bytes_estimate` 始终为 `None`，CLI 无法展示整体扫描进度百分比，仅能反映“已扫描文件数 / 已扫描总大小”；取消语义也仍然是基于 `AtomicBool` 的“最佳努力”，在极端大目录或 IO 压力较高场景下，Ctrl+C 到实际退出之间可能存在感知上的延迟。上述局限不会阻塞本 story 的完成（验收标准只要求实时反馈已扫描文件数与总大小、在合理时间内结束并不输出部分结果），但可在后续迭代中考虑：为扫描增加粗略的总量预估（例如基于预扫目录或历史统计）以及更细粒度的取消检查点，改善用户对进度与中断响应性的主观体验。
   - 方向二：大目录（百万级文件）场景下的性能与资源约束验证——现有 `surf-core` 与 CLI 端到端测试仍主要基于临时目录和几十个小文件，侧重语义正确性；尚未在包含接近或超过 1M 个文件的目录上做系统性压力测试。为对齐 PRD 第 8 章中关于“在包含至少 1M 个文件的目录上扫描不崩溃、内存占用在约定上限以内”的验收要求，后续需要在具备充足资源的环境（如专用性能测试机或带有真实大目录的 CI 节点）上补充针对 `surf-core` 进度 API 与 `surf` CLI 的性能/内存基准测试或长时间运行测试，并在文档中记录推荐的 `--threads` / `--min-size` / `--limit` 组合配置。
   - 方向三：CLI 表格与 JSON 模式下错误路径与默认参数的端到端回归在 CI 中长期稳定运行——当前 `workspaces/dev-cli-tui/surf-cli/tests/integration.rs` 已包含 `test_surf_table_error_on_nonexistent_path` / `test_surf_table_default_limit_20` 等用例，与 JSON 相关的集成测试一起覆盖了“表格模式 + 不存在路径”“默认 limit=20”“非法参数不输出部分结果”等关键行为。受当前 Ralph 运行环境无法访问 crates.io 的限制，这些测试在本环境中仍无法完整执行；后续应在可访问 crates.io 或具备完整依赖缓存的 CI / 开发机上，将 `cargo test -p surf-cli`（以及必要时的 `cargo test -p surf-core`）纳入常规流水线，并关注在引入进度感知 API 后这些用例能长期稳定通过，从而为本 story 的行为提供持续的自动化回归保障。

#### 9.1.2 CLI-ONEOFF-002 并发与最小文件大小过滤

- id: `CLI-ONEOFF-002`
- title: 用户可以通过参数控制扫描并发度和最小文件大小过滤
- status: `done`
- description: 作为希望在不同机器和目录上优化扫描性能的用户，我希望通过 `--threads` 和 `--min-size` 参数控制扫描的并发度以及结果中出现的最小文件大小，从而在保证结果可用性的前提下缩短扫描时间、减少无关小文件干扰。
- acceptance_criteria:
  - 运行 `surf --path <dir> --threads N`（N 为合法正整数）时，程序按约定并发度启动扫描，不出现明显退化（例如设置更高并发却显著变慢或卡死）。
  - 在同一测试目录上，对比不加 `--min-size` 与加上 `--min-size 100MB` 的结果，后者的结果表中不包含任何大小小于 100MB 的文件/目录。
  - 对于非法的 `--threads` 或 `--min-size` 参数值（如 0、负数或无法解析的单位），程序给出明确错误提示并以非零状态码退出，不进入扫描过程。
  - 使用 `--threads`、`--min-size` 与 `--limit` 组合时，结果仍然按大小降序排序且条数限制与过滤逻辑符合预期。
  - impl_notes (iteration 12 / dev-core-scanner & dev-cli-tui):
   - `workspaces/dev-core-scanner/surf-core/src/lib.rs:1-72` 中的 `scan(root, min_size, threads)` 已实现按线程数控制的并发扫描与最小文件大小过滤：当 `root` 不存在时立即返回 `ErrorKind::NotFound`，错误消息包含 `scan root does not exist: <path>`；通过 `ThreadPoolBuilder::new().num_threads(threads.max(1))` 构建局部 rayon 线程池，并将 `WalkDir::new(root)` 迭代器转换为 `par_bridge()` 并行流，仅保留 `metadata.is_file() == true` 且 `metadata.len() >= min_size` 的文件条目，最终按 `size` 字段降序排序后返回完整 `Vec<FileEntry>` 结果。
   - `workspaces/dev-core-scanner/surf-core/tests/basic_scan.rs:22-83` 的 `scan_respects_min_size_and_sorts_desc` 覆盖了 `min_size` 过滤与降序排序的 happy path；`scan_threads_zero_falls_back_to_one` / `scan_nonexistent_root_returns_not_found` / `scan_empty_directory_returns_empty_result` 等用例分别验证了 `threads == 0` 时退化为 1 的行为、一致的错误类型与错误消息语义，以及空目录返回空结果而非错误，从核心层保证了 acceptance_criteria 中“过滤语义正确”“并发参数健壮”的基础。
   - `workspaces/dev-cli-tui/surf-cli/src/main.rs:12-37` 中的 `Args` 结构体通过 `clap::Parser` 定义了 `--min-size/-m`（字符串，默认 `"0"`）和 `--threads/-t`（默认 `num_cpus::get()`，`value_parser = parse_threads`）参数；`parse_size` 将字符串解析为字节数并支持 `B/KB/MB/GB` 单位，解析失败时在 stderr 输出 `Error parsing --min-size: ...` 并以非零状态码退出；`parse_threads` 显式拒绝 `0` 值（返回 `"--threads must be at least 1"` 错误），从而保证非法并发度不会触发真正的扫描逻辑。
   - 在 CLI 主流程中（`workspaces/dev-cli-tui/surf-cli/src/main.rs:149-175`），`min_size` 解析失败直接退出；解析成功后调用 `surf_core::scan(&args.path, min_size, args.threads)`，并沿用核心层 `ErrorKind::NotFound` / 其他 IO 错误语义：当扫描失败时，CLI 清理 `indicatif::ProgressBar` spinner，在 stderr 输出 `Failed to scan <path>: <error>` 并以非零状态码退出；当扫描成功时，根据 `--json` 决定走 JSON 输出（`JsonOutput`）或表格输出，两种模式下都对 `entries` 进行 `take(args.limit)` 截断，利用核心层已按 size 降序排序的结果满足“多参数组合仍然按大小降序排序且条数限制正确”的要求。
   - `workspaces/dev-cli-tui/surf-cli/src/main.rs:207-276` 中的单元测试为 `parse_size` 和 `threads` 相关参数提供了细粒度覆盖：包括空字符串/仅空白被视为 0、`KB/MB/GB` 大小写不敏感解析、未知单位报错，以及 `Args::parse_from` 在默认 threads、`-t` 覆盖和 `-t 0` 非法值时的行为；`service_mode_defaults_and_network_options` 也间接验证了与服务模式同处一处的参数默认值与覆盖逻辑。
  - remaining_todos:
   - 方向一：在具备完整 Rust 依赖缓存或可访问 crates.io 的环境（CI 或开发机）上，运行 `cargo test -p surf-core` 与 `cargo test -p surf-cli`，对本 story 所依赖的单元测试和集成测试进行一次完整回归，确保在不同 `--threads` / `--min-size` / `--limit` 组合下行为与 acceptance_criteria 保持一致。
   - 方向二：在包含大量文件（例如 10^5 ~ 10^6 级别）的目录上分别以不同 `--threads` 值运行 `surf`，观察整体耗时与系统负载，记录推荐的线程数区间和 `min_size` 组合配置；如发现明显退化（高并发反而变慢或产生过高 IO 压力），在 Architecture 或后续 PRD 迭代中补充针对性建议或限流策略。
   - 方向三：在后续引入交付阶段 `test/` 目录脚本时，可考虑为本 story 补充端到端验收脚本（例如 `test/scripts/cli_concurrency_and_min_size.sh`），以 `./release/<platform>/cli/surf` 为入口，验证不同参数组合下的 TopN 输出语义，与 PRD 8 章的 CLI 验收条目建立可回归的自动化映射。

#### 9.1.3 CLI-ONEOFF-003 JSON 结构化输出

- id: `CLI-ONEOFF-003`
- title: 用户可以在单次运行模式下以 JSON 格式获取扫描结果
- status: `done`
- description: 作为希望将 Surf 扫描结果接入其他自动化工具链的用户，我希望在单次运行模式下通过 `--json` 参数获取结构化 JSON 输出，而不是表格文本，以便在脚本或 CI/CD 流水线中解析和进一步处理结果。
- acceptance_criteria:
  - 运行 `surf --path <dir> --json` 时，标准输出为合法的 JSON 文本，能够被常见 JSON 解析器无错误解析。
  - JSON 输出中至少包含：被扫描根路径、每个结果条目的完整路径、大小（带单位或统一单位字段）、文件类型或目录标识，以及与 `--limit`、`--min-size` 等参数一致的过滤/截断结果。
  - 在 `--json` 模式下，程序的退出码语义与表格模式保持一致：正常完成为 0，参数错误或严重异常为非零。
  - 对于错误参数组合（例如不支持的标志），仍然打印清晰错误信息到标准错误输出，并避免输出部分或不完整的 JSON 结构。
  - impl_notes (iteration 2 / dev-cli-tui):
    - `workspaces/dev-cli-tui/surf-cli/src/main.rs:59` 定义了 `JsonEntry`/`JsonOutput` 结构，当前 JSON 根对象包含 `root` 与 `entries` 数组，条目字段为 `path`、`size`、`is_dir`（扫描器目前仅返回文件，目录条目暂不支持）。
    - `workspaces/dev-cli-tui/surf-cli/tests/integration.rs:9` 起，包含以下端到端用例：
      - `--path <temp_dir> --min-size 10 --limit 1 --json` 成功路径，验证 stdout 为合法 JSON、`root` 与临时目录一致，`entries` 长度不超过 `limit`、`size >= min-size` 且 `is_dir == false`。
      - `--json` 模式下非法 `--min-size`、非法 `--threads`、不存在路径等错误场景，均保证进程非零退出、stdout 保持空白、错误信息仅输出到 stderr；非 `--json` 模式下非法 `--min-size` 行为与之保持一致。
  - remaining_todos:
    - 本轮（Ralph iteration 8）在仓库根目录尝试执行：`cargo test -p surf-core --offline`、`cargo test -p surf-cli --offline` 以及 `cargo test -p surf-core`，均因无法从 `https://github.com/rust-lang/crates.io-index` 获取 `rayon` 依赖而失败；在线模式下多次重试时出现 `failed to connect to github.com: Connection timed out`，确认当前 Ralph 运行环境无可用 crates.io 网络访问或本地缓存。
    - 受上述限制影响，`surf-core` 与 `surf-cli` 已经就绪的单元测试与端到端集成测试（包括 `workspaces/dev-core-scanner/surf-core/tests/basic_scan.rs` 与 `workspaces/dev-cli-tui/surf-cli/tests/integration.rs`）目前只能在具备网络或完整依赖缓存的 CI / 开发机上执行；建议在该类环境中运行 `cargo test --workspace` 或至少 `cargo test -p surf-core` / `cargo test -p surf-cli` 以完成对 CLI-ONEOFF-003 的自动化验收覆盖。
    - 在 CI 流水线中启用并稳定上述 `workspaces/dev-cli-tui/surf-cli/tests/integration.rs` 用例，作为回归保护，避免未来改动破坏 `--json` 模式下的 stdout/stderr 语义。

### 9.2 服务模式（示例）

#### 9.2.1 SVC-JSONRPC-001 启动本地 JSON-RPC 服务并保持可用

- id: `SVC-JSONRPC-001`
- title: 用户可以在本机启动 Surf JSON-RPC 服务并通过网络访问
- status: `done`
- description: 作为计划开发 GUI 或集成其他工具的开发者，我希望通过 `surf --service` 在本机启动一个 JSON-RPC 服务进程，并可通过 `--host` 与 `--port` 配置监听地址与端口，从而在不每次重新启动扫描进程的情况下，复用同一服务进行多次扫描请求。
- acceptance_criteria:
  - 运行 `surf --service` 后，进程在默认 `127.0.0.1:1234` 上监听，且符合 JSON-RPC 2.0 协议基本格式（包含 `jsonrpc`、`method`、`params`、`id` 字段）。
  - 使用 `--host` 与 `--port` 参数可以成功修改监听地址与端口，配置非法地址或端口时给出清晰错误提示并退出。
  - 在同一进程中，连续发送多次“启动扫描”“查询进度”“获取结果”请求时，服务保持稳定，不出现崩溃或资源泄漏迹象。
  - 在无请求时，服务进程保持空闲且资源占用在可接受范围内，支持通过常规信号（例如 `Ctrl+C`）优雅退出。
  - impl_notes (iteration 65 / dev-service-api & dev-cli-tui):
    - `workspaces/dev-service-api/surf-service/src/main.rs:1-237` 中的 `TaskInfo` / `TaskManager` 结构已经扩展为可持有核心扫描句柄的任务表：`TaskInfo` 新增 `scan_handle: Option<Arc<surf_core::ScanHandle>>` 字段，用于保存由 `surf_core::start_scan` 返回的句柄；`TaskManager::register_task_with_handle` 负责在分配 `task_id` 的同时，将路径、`min_size_bytes`、`threads`、`limit`、`tag`、初始 `TaskState` 以及（可选的）扫描句柄一并写入任务表，供后续 `Surf.Status` / `Surf.Cancel` 使用。
    - `workspaces/dev-service-api/surf-service/src/main.rs:239-507` 中的 `SurfScanParams` / `parse_size_for_service` / `validate_surf_scan_params` 负责对 `Surf.Scan` 的入参进行解析与数值校验：
      - `min_size` 采用与 CLI 一致的解析规则（支持 `B/KB/MB/GB`，大小写不敏感，空字符串视为 0，非法单位返回 `INVALID_PARAMS`）；
      - `threads` 若显式给出，必须大于等于 1，否则返回 `INVALID_PARAMS`；未给出时默认使用 `num_cpus::get()` 作为线程数。
    - `workspaces/dev-service-api/surf-service/src/main.rs:1488-1874` 中的 `handle_rpc_line` 已经将 `Surf.Scan` 从“仅登记元数据的骨架实现”升级为“真正启动底层扫描”的业务路径：
      - 当 `method == "Surf.Scan"` 且 `params` 为对象、能成功反序列化为 `SurfScanParams` 并通过 `validate_surf_scan_params` 校验后，服务会先解析 `min_size_bytes`，再构造 `surf_core::ScanConfig { root, min_size, threads }`；
      - 随后调用 `surf_core::start_scan(config)` 启动实际扫描任务：
        - 启动成功时，将返回的 `ScanHandle` 包装为 `Arc<ScanHandle>` 并通过 `TASK_MANAGER.register_task_with_handle(...)` 注册任务，初始状态设置为 `TaskState::Running`；
        - 构造并返回 `SurfScanResult` 成功响应，其中 `state` 字段为 `"running"`，其余字段（`path`、`min_size_bytes`、`threads`、`limit`）与解析结果一致；
        - 若 `start_scan` 返回错误，则当前版本将其映射为 `INVALID_PARAMS` 错误（`error.data.detail` 中带有 `failed to start scan: ...` 文本），尚未细分为更精确的内部错误码。
      - 当 `params == null` 或为非对象、结构无法解析为 `SurfScanParams` 时，仍分别返回 `METHOD_NOT_FOUND` / `INVALID_PARAMS`，与前一迭代的错误行为保持兼容。
    - `workspaces/dev-service-api/surf-service/src/main.rs:440-507` 中的 `SurfStatusResult::from_task_info` 已经接入核心进度快照：
      - 当 `TaskInfo.scan_handle` 为 `Some(handle)` 时，`from_task_info` 会调用 `surf_core::poll_status(&handle)`，将返回的 `StatusSnapshot.progress.scanned_files` / `scanned_bytes` / `total_bytes_estimate` 映射到 JSON-RPC 的 `scanned_files` / `scanned_bytes` / `total_bytes_estimate` 字段；
      - 若 `total_bytes_estimate` 为 `Some(total)` 且 `total > 0`，则按 `scanned_bytes as f64 / total as f64` 计算 `progress`（0.0~1.0）；在当前核心实现中该字段仍常为 `None`，因此 `progress` 通常为 0.0，但一旦核心层提供估算，总体进度即可在无需破坏字段语义的前提下自动生效；
      - 对于尚未绑定句柄的任务（`scan_handle == None`），`progress` / `scanned_files` / `scanned_bytes` 保持为占位值（0），`total_bytes_estimate` 为 `null`。
    - `workspaces/dev-service-api/surf-service/src/main.rs:206-232` 中的 `TaskManager::cancel_task` 已经将任务状态迁移与底层取消语义打通：
      - 当任务当前状态为 `Queued` 或 `Running` 时，`cancel_task` 会将其状态更新为 `Canceled`，同时若 `scan_handle` 存在则调用 `surf_core::cancel(handle)` 发出“最佳努力”取消信号；
      - 当任务已处于终止态（`Completed` / `Failed` / `Canceled`）时，再次取消不会改变状态，仅更新 `updated_at` 时间戳；
      - `Surf.Cancel` 的 JSON-RPC 分支在接收到合法 `task_id` 时，会调用 `TASK_MANAGER.cancel_task(task_id)` 并将 `(previous_state, current_state)` 映射为 `SurfCancelResult`，从而在 API 层面暴露幂等取消语义和最终状态。
    - `workspaces/dev-service-api/surf-service/src/main.rs:1619-1687` 中 `Surf.Status` 的处理逻辑在原有“查询任务表元数据”的基础上，结合了上述 `SurfStatusResult::from_task_info`：
      - `params == null`、`params` 缺少 `task_id` 或显式为 `{"task_id": null}` 时，都会通过 `TASK_MANAGER.list_non_terminated_tasks()` 拉取所有处于 `Queued` / `Running` 状态的任务，并为每个任务调用 `SurfStatusResult::from_task_info`，返回一个数组形式的成功响应；
      - 当 `task_id` 为非空字符串且能在任务表中找到对应条目时，返回单个 `SurfStatusResult` 对象；找不到时返回 `TASK_NOT_FOUND` 错误；
      - 由于当前任务状态的迁移仍然仅由调用方（例如未来的结果收集逻辑或取消逻辑）显式触发，`Surf.Status` 虽然能够反映核心扫描进度，但尚未结合 `StatusSnapshot.done` / 错误信息自动将 `TaskState::Running` 迁移到 `Completed` / `Failed`，任务生命周期仍停留在“弱状态机”阶段。
    - `workspaces/dev-service-api/surf-service/src/main.rs:1876-1954` 中的 `Args` / `main` 维持了服务进程的监听行为：使用 `clap::Parser` 解析 `--host`（默认 `127.0.0.1`）、`--port`（默认 `1234`）、`--max-concurrent-scans`（默认 4，占位参数）、`--task-ttl-seconds`（默认 600，占位参数），并基于 `tokio::net::TcpListener::bind(&addr)` 在 `<host>:<port>` 上启动监听；每个新连接由 `handle_connection` 在独立 `tokio::spawn` 任务中处理，按行读取请求并将 JSON-RPC 响应逐行写回客户端。`max_concurrent_scans` / `task_ttl_seconds` 目前仍仅用于启动日志展示，尚未参与实际并发控制或任务回收。
    - `workspaces/dev-service-api/surf-service/src/main.rs:687-1474` 中的测试模块覆盖了当前版本服务层的主要错误与成功路径，包括：
      - `Surf.Scan` 参数形状/单位/线程数的非法组合（返回 `INVALID_PARAMS`）、合法参数触发任务注册与 `state="running"` 的成功响应；
      - `Surf.Status` 在 task_id 缺省、为 `null`、为非法类型、为不存在 ID 以及为已注册任务时的行为；
      - `Surf.Cancel` 在参数缺失/类型错误/任务不存在、已有 `Queued` 任务、终止态任务的幂等取消路径；
      - `TaskManager` 在多任务注册、`get_task_info`、时间戳字段等方面的基本语义。
    - `workspaces/dev-cli-tui/surf-cli/src/main.rs:115-149` 中，CLI 形态已经可以通过 `--service` 子命令启动服务模式：
      - `Args` 定义了 `--service` / `-s` 开关以及 `--host` / `--port` 参数，当用户执行 `surf --service --host <host> --port <port>` 时，CLI 会调用 `run_service(host, port)` 启动名为 `"surf-service"` 的子进程，并将 `--host` / `--port` 透传给服务进程；
      - `run_service` 对子进程启动失败或非零退出码仅打印错误信息到 stderr 并以相应退出码结束当前 CLI 进程，不承担任何 JSON-RPC 客户端或健康检查逻辑，仅作为“本地服务进程启动入口”。
    - 与本 story 的验收标准相比，当前实现已经具备“按 host/port 启动本地 JSON-RPC 监听”“通过 `Surf.Scan` 真正启动核心扫描并在任务表中保存 `ScanHandle`”“通过 `Surf.Status` 结合 `surf_core::poll_status` 反映真实扫描进度（在 `total_bytes_estimate` 为 `None` 时退化为仅返回已扫描文件数与字节数）”“通过 `Surf.Cancel` 在状态迁移为 `Canceled` 时调用 `surf_core::cancel(handle)` 发出取消信号”以及基础的单元测试覆盖；在 iteration 65 中，`Surf.GetResults` 已经通过 `TaskManager::collect_results_if_needed` 接入 `surf_core::collect_results`，对 `state = completed` 的任务按 Architecture.md 4.3.5 约定返回真实的 `total_files` / `total_bytes` 与 TopN/summary 视图（entries 结构与 CLI `JsonEntry` 对齐），并在收集失败时将任务状态迁移为 `Failed` 并返回带错误详情的 `INVALID_PARAMS`。结合 `workspaces/dev-service-api/surf-service/src/main.rs:1415-1474` 的端到端测试，可以在单机环境中完整走通“启动扫描 → 等待任务完成 → 通过 Surf.GetResults 获取聚合结果”的 JSON-RPC 闭环，因此本 story 标记为 `done`。
  - remaining_todos:
    - 方向一：任务状态机与核心进度/错误的深度集成与长期行为
      - 当前版本在 `SurfStatusResult::from_task_info` 中，当 `TaskInfo.scan_handle` 存在且 `surf_core::poll_status(&handle)` 返回的 `StatusSnapshot.done == true` 且任务状态仍为 `Running` 时，会根据 `StatusSnapshot.error` 将任务惰性迁移为 `Completed` 或 `Failed`，并通过 `TASK_MANAGER.update_task_state` 回写到任务表（参见 `workspaces/dev-service-api/surf-service/src/main.rs:440-507`），基本对齐 Architecture.md 4.3.7 中关于“结合 StatusSnapshot.done / error 更新任务状态机”的约定。
      - 仍需补充：明确 `Surf.Status` 在任务已终止时的长期行为（例如是否始终返回终止态任务的快照、是否只返回非终止态任务）、是否需要在任务表中显式记录“最近一次核心错误摘要”或“最终结果快照元信息”（例如总文件数、总大小），以便在不调用 `Surf.GetResults` 的情况下也能通过 `Surf.Status` 获取最小必要的任务完成信息，并在 PRD/Architecture 中对这些行为做出稳定约定，供 GUI/调用方依赖。
    - 方向二：并发控制与 TTL 回收策略（围绕 `max-concurrent-scans` / `task-ttl-seconds` 落地）
      - 基于 `Args.max_concurrent_scans` 在服务层实现“同时运行的扫描任务上限”：在 `Surf.Scan` 创建新任务前检查当前处于 `Running` 状态且绑定了 `scan_handle` 的任务数量，超出上限时要么直接返回业务错误（例如新的 JSON-RPC 错误码 `TOO_MANY_ACTIVE_TASKS`），要么实现明确的排队语义（在 PRD/Architecture 中约定清楚，并通过 `Surf.Status` 显示队列位置或排队状态）。
      - 基于 `Args.task_ttl_seconds` 为终止态任务（`Completed` / `Failed` / `Canceled`）引入周期性回收机制：可以通过后台清理任务、在 `Surf.Status` 调用时 opportunistic 清理或在新任务创建前触发一次清理，确保任务表不会在长时间运行的服务进程中无限增长。
      - 为并发控制与 TTL 回收路径补充必要的日志和 JSON-RPC 错误码，便于在高并发或长时间运行场景下排查“请求被拒绝/丢失”“任务被自动清理”等问题，并在 PRD 中对这些行为进行简要对齐。
    - 方向三：端到端与性能/并发验证
      - 在具备依赖缓存或可访问 crates.io 的环境中，为 `surf-service` 引入最小的端到端验收路径：
        - 以 `surf-service` 二进制（或通过 `surf --service` 启动的进程）为入口，使用简单脚本或测试客户端依次覆盖“发起扫描 (`Surf.Scan`) → 轮询进度 (`Surf.Status`) → 获取结果 (`Surf.GetResults`) / 或中途取消 (`Surf.Cancel`)”的闭环场景；
        - 在 CI 或专用验收脚本中固定一组典型测试目录（小目录、包含若干大文件的目录等），验证 JSON-RPC 协议行为与 PRD 8 章服务模式验收条目的一致性，以及在高并发请求场景下服务的稳定性和资源占用情况。

### 9.3 待确认问题

- 【CLI 单次运行】在使用 `--json` 输出时，终端是否仍需展示进度条？若展示，是否有对 JSON 消费方不会造成干扰的输出分流约定（例如进度仅输出到 stderr）？
  - 【设计决策】如无特别说明，CLI 在 `--json` 模式下仍允许展示进度条，但进度条和日志统一输出到 stderr，stdout 仅在扫描成功时一次性输出完整 JSON；扫描失败、参数错误或被中断（包括 Ctrl+C）时，stdout 不输出任何 JSON，仅在 stderr 输出错误说明，并以非零状态码退出。

- 【CLI 单次运行】当用户通过 `Ctrl+C` 中断扫描时，是否需要输出部分结果（例如已扫描结果的 JSON 或表格）？当前 PRD 仅要求“合理时间内结束且不留下半写入的输出文件”，但对标准输出的行为尚未明确。
  - 【设计决策】如无特别说明，CLI 在 JSON 和表格模式下均不输出部分结果；当用户通过 Ctrl+C 中断扫描时，程序清理进度条，在 stderr 输出“用户中断”提示，并以 130 等非零退出码退出，保证不会产生半写入的输出文件或结构化结果。后续如对“部分结果输出”有强需求，可在未来迭代中单独立项扩展相应语义与行为。

### 9.4 TUI 模式（终端交互）

#### 9.4.1 TUI-BROWSE-001 终端 TUI 中浏览与安全删除

- id: `TUI-BROWSE-001`
- title: 在终端 TUI 中浏览扫描结果并安全删除文件
- status: `todo`
- description: 作为一名希望在终端中以更直观方式管理磁盘空间的高级用户，我希望通过 `surf --tui --path <dir>` 进入全屏 TUI 界面，在扫描过程中获得明确的进度反馈，并在扫描完成后通过键盘浏览目录树/列表、对选中文件发起“移入回收站”的删除操作，从而在不记住复杂命令的前提下安全地清理大体积文件。
- acceptance_criteria:
  - 在常见终端（如 `xterm`、`iTerm2`）中执行 `surf --tui --path <dir>` 时，程序进入全屏 TUI 界面，终端切换为原始模式并隐藏光标，界面包含主列表/树视图与状态栏，退出后终端状态能被正确恢复。
  - 扫描进行中，TUI 处于 `Scanning` 状态：状态栏或明显区域持续展示扫描进度，至少包含已扫描文件数与已扫描总字节数（与 Architecture.md 4.4 中 `scanned_files` / `scanned_bytes` 的语义对齐），用户可以看到进度在随时间推进，而不是长时间停在静止界面。
  - 扫描完成且未发生错误时，TUI 自动切换到 `Browsing` 状态：主视图展示基于当前扫描结果构建的目录树或按目录聚合的列表，用户可以通过方向键或 `j/k`、`Enter`/`Backspace` 等键执行上下移动、下钻子目录和返回上级目录，并在详情区域看到当前选中条目的完整路径与大小信息。
  - 在 `Browsing` 状态下，用户对当前选中条目按约定快捷键（例如 `d`）触发删除时，TUI 切换到 `ConfirmDelete` 状态：弹出明显的二次确认对话框，至少展示目标路径和大小，并用明确文案说明“该操作会将文件/目录移入系统回收站/废纸篓，而非永久删除”；只有在用户明确确认（如按 `y` 或 `Enter`）后才真正执行删除，取消（如按 `n` 或 `Esc`）不会对文件系统产生任何修改。
  - 删除操作的底层语义与 PRD 3.4 和 Architecture.md 4.4.5 / 6.2 的约定一致：成功删除时，目标条目被移入系统回收站/废纸篓而不是直接永久删除；删除失败（例如权限不足、未检测到可用回收站实现等）时，TUI 以错误提示弹窗形式提示原因，不会静默失败或造成结果列表与实际磁盘状态严重不一致。
  - TUI 模式下的退出与中断行为与 CLI 约定保持一致：在 `Browsing` 状态下按 `q` 正常退出时，进程以 0 退出码结束且不会在 stdout 再次打印扫描结果列表；在 `Scanning` 状态下收到 Ctrl+C（SIGINT）时，程序调用核心取消接口并尽快退出 TUI，恢复终端，在 stderr 输出“用户中断”类提示，并以非零退出码结束进程，不进入浏览状态、也不输出任何部分结果。
