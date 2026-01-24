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
- status: `pending`
- description: 作为一名使用 Surf 的开发者或运维，我希望在终端中运行 `surf --path <dir>` 进行单次扫描，在看到实时进度反馈后，最终获得一份按文件/目录大小降序排序、条数可控的结果表格，以便快速定位空间占用大户。
- acceptance_criteria:
  - 在 Linux 和 macOS 上，执行 `surf --path <dir>` 时，终端在扫描过程中展示动态进度条，至少包含已扫描文件数与已扫描总大小等信息。
  - 扫描完成后，进度条自动结束，并在其下方输出格式化表格结果，表格内容按文件/目录大小降序排列。
  - 未显式指定 `--limit` 时，默认只展示前 20 条记录；指定 `--limit N` 时，展示条目数不超过 N。
  - 对不存在或不可访问的路径（例如权限不足）给出清晰错误信息，并以非零状态码退出，不输出误导性的空表格。
 - impl_notes (iteration 10 / dev-core-scanner & dev-cli-tui):
   - `workspaces/dev-core-scanner/surf-core/src/lib.rs:8-71` 定义了 `FileEntry { path, size }` 与同步扫描函数 `scan(root, min_size, threads) -> io::Result<Vec<FileEntry>>`：调用前显式检查 `root.exists()` 并在路径不存在时返回 `ErrorKind::NotFound`，错误信息中包含类似 `scan root does not exist: <path>` 的提示；核心实现使用 `WalkDir::new(root)` 递归遍历，仅保留 `metadata.is_file() == true` 且 `metadata.len() >= min_size` 的文件条目，在局部 `rayon` 线程池中并发收集结果，并最终按 `size` 字段降序排序返回完整 `Vec<FileEntry>`，不在核心层截断 TopN。
   - `workspaces/dev-core-scanner/surf-core/tests/basic_scan.rs` 中的测试覆盖了本 story 依赖的关键扫描语义：`scan_respects_min_size_and_sorts_desc` 验证最小大小过滤与按 size 降序排序；`scan_threads_zero_falls_back_to_one` 验证 `threads == 0` 时退化为 1 的防御性行为；`scan_nonexistent_root_returns_not_found` 验证不存在路径时返回 `ErrorKind::NotFound` 且错误消息包含 `"does not exist"`；`scan_empty_directory_returns_empty_result` 验证空目录时返回空结果而非错误。
   - `workspaces/dev-cli-tui/surf-cli/src/main.rs` 中，CLI 单次运行模式与表格输出已实现如下行为：`Args` 结构体通过 `clap::Parser` 定义了 `--path/-p`（默认 `.`）、`--min-size/-m`（默认 `"0"`，由 `parse_size` 解析为字节数）、`--limit/-n`（默认 `20`）和 `--threads/-t`（默认逻辑 CPU 数，使用 `parse_threads` 禁止 `0` 值），与 PRD 第 4 章参数表保持一致；`main` 在进入扫描前创建基于 `indicatif::ProgressBar::new_spinner` 的进度指示器，绘制目标设置为 stderr，并以 `"Scanning <path> ..."` 文本提示当前扫描根路径，随后调用 `surf_core::scan(&args.path, min_size, args.threads)` 完成实际扫描；当 `scan` 返回错误时，CLI 清理 spinner、在 stderr 输出 `"Failed to scan <path>: <error>"` 并以非零状态码退出；当扫描成功时，CLI 清理 spinner 并在 stdout 打印表格：先输出带 `SIZE(BYTES)` / `PATH` 两列表头与分隔线，再对 `entries.into_iter().take(args.limit)` 逐行输出 `size` 与 `path`，其中行顺序直接复用核心层已经按 size 降序排序好的 `Vec<FileEntry>`。
   - `workspaces/dev-cli-tui/surf-cli/tests/integration.rs` 中的端到端测试覆盖了 CLI 行为的部分验收路径：`test_surf_table_output_with_min_size_and_limit` 在临时目录中构造小文件与两个大文件，通过 `--path <temp_dir> --min-size 10 --limit 2` 触发表格模式，断言 stdout 中存在表头与分隔线、数据行数量不超过 `limit` 且至少有一行，并且数据行包含临时目录路径片段，从而间接验证 `--path` / `--min-size` / `--limit` 的组合行为与表格输出基本结构；其他用例（`test_surf_json_output_with_min_size_and_limit`、`test_surf_json_error_on_invalid_min_size`、`test_surf_json_error_on_invalid_threads`、`test_surf_json_error_on_nonexistent_path` 与 `test_surf_non_json_error_behavior`）验证了 JSON 与非 JSON 模式下对非法 `--min-size` / `--threads`、不存在路径等错误场景的处理：stdout 保持为空、错误信息仅输出到 stderr 且进程以非零状态码退出，为本 story 的“错误路径”行为提供了共享回归保护。
  - remaining_todos:
   - 方向一：进度条展示“已扫描文件数 / 已扫描总大小”等指标——当前 CLI 仅在进入扫描前创建 `indicatif::ProgressBar::new_spinner` 并设置文本 `"Scanning <path> ..."`，没有从核心扫描器获取实时的 `scanned_files` / `scanned_bytes` 统计信息；核心层 `surf_core::scan` 仍然是一次性同步接口，也没有暴露进度回调或快照。为满足 PRD 9.1.1 中“终端在扫描过程中展示动态进度条，至少包含已扫描文件数与已扫描总大小”等要求，后续需要在 `dev-core-scanner` 和 `dev-cli-tui` 之间设计最小的进度上报机制（例如：在扫描过程中维护计数器并周期性通过回调/通道向 CLI 报告），并在 CLI 侧更新进度条提示内容，同时继续遵守 Architecture.md 4.4 中“进度输出写入 stderr、结果写入 stdout”的分流约定。
   - 方向二：大目录（百万级文件）场景下的性能与资源约束验证——现有 `surf-core` 与 CLI 端到端测试均基于临时目录和几十个小文件，主要验证语义正确性（过滤、排序、错误类型等），未在包含接近或超过 1M 个文件的目录上做压力测试。为对齐 PRD 第 8 章中关于“在包含至少 1M 个文件的目录上扫描不崩溃、内存占用在约定上限以内”的验收要求，需要在具有充足资源的环境中补充针对 `surf_core::scan` 和 `surf` CLI 的性能/内存基准测试或长时间运行测试，并在文档中记录推荐的 `--threads` / `--min-size` / `--limit` 组合配置。
   - 方向三：CLI 表格模式下错误路径与默认参数的端到端回归——在迭代 11 中，`workspaces/dev-cli-tui/surf-cli/tests/integration.rs` 已新增 `test_surf_table_error_on_nonexistent_path` 与 `test_surf_table_default_limit_20` 两个用例：前者覆盖“表格模式 + 不存在路径”场景（断言 stdout 不含表头/数据行、stderr 含有 `"Failed to scan"`/`"does not exist"` 且退出码非零），后者覆盖“未显式传入 `--limit` 时默认展示前 20 条”的行为（在包含超过 20 个符合条件文件的目录上断言数据行数不超过 20 且至少有 1 行）。受当前 Ralph 运行环境无法访问 crates.io 的限制，这些测试尚未在本环境中成功执行 `cargo test`，但预计可在具备完整依赖缓存的 CI/本地开发机上通过；后续可在 CI 流水线中启用并稳定运行上述用例，将本 story 的相关验收条目纳入自动化测试覆盖范围。

#### 9.1.2 CLI-ONEOFF-002 并发与最小文件大小过滤

- id: `CLI-ONEOFF-002`
- title: 用户可以通过参数控制扫描并发度和最小文件大小过滤
- status: `pending`
- description: 作为希望在不同机器和目录上优化扫描性能的用户，我希望通过 `--threads` 和 `--min-size` 参数控制扫描的并发度以及结果中出现的最小文件大小，从而在保证结果可用性的前提下缩短扫描时间、减少无关小文件干扰。
- acceptance_criteria:
  - 运行 `surf --path <dir> --threads N`（N 为合法正整数）时，程序按约定并发度启动扫描，不出现明显退化（例如设置更高并发却显著变慢或卡死）。
  - 在同一测试目录上，对比不加 `--min-size` 与加上 `--min-size 100MB` 的结果，后者的结果表中不包含任何大小小于 100MB 的文件/目录。
  - 对于非法的 `--threads` 或 `--min-size` 参数值（如 0、负数或无法解析的单位），程序给出明确错误提示并以非零状态码退出，不进入扫描过程。
  - 使用 `--threads`、`--min-size` 与 `--limit` 组合时，结果仍然按大小降序排序且条数限制与过滤逻辑符合预期。

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
- status: `pending`
- description: 作为计划开发 GUI 或集成其他工具的开发者，我希望通过 `surf --service` 在本机启动一个 JSON-RPC 服务进程，并可通过 `--host` 与 `--port` 配置监听地址与端口，从而在不每次重新启动扫描进程的情况下，复用同一服务进行多次扫描请求。
- acceptance_criteria:
  - 运行 `surf --service` 后，进程在默认 `127.0.0.1:1234` 上监听，且符合 JSON-RPC 2.0 协议基本格式（包含 `jsonrpc`、`method`、`params`、`id` 字段）。
  - 使用 `--host` 与 `--port` 参数可以成功修改监听地址与端口，配置非法地址或端口时给出清晰错误提示并退出。
  - 在同一进程中，连续发送多次“启动扫描”“查询进度”“获取结果”请求时，服务保持稳定，不出现崩溃或资源泄漏迹象。
  - 在无请求时，服务进程保持空闲且资源占用在可接受范围内，支持通过常规信号（例如 `Ctrl+C`）优雅退出。
  - impl_notes (iteration 8 / dev-service-api & dev-cli-tui):
    - `workspaces/dev-service-api/surf-service/src/main.rs` 中的 `surf-service` 二进制，当前已实现较完整的 JSON-RPC 骨架能力：
      - 使用 `clap::Parser` 定义 `Args` 结构体，支持 `--host`（默认 `127.0.0.1`）、`--port`（默认 `1234`）、`--max-concurrent-scans`（默认 4，占位用于后续并发控制）、`--task-ttl-seconds`（默认 600，对应 Architecture.md 中的 `task_ttl_seconds`），并在启动日志中打印当前配置。
      - 基于 `tokio::net::TcpListener` 在 `<host>:<port>` 上启动 TCP 监听；每当接受到新连接时，为该连接启动独立的 async 任务，通过 `handle_connection` 按行读取请求并处理。
      - 对于每一行入站数据，使用 `handle_rpc_line` 执行 JSON 解析和 JSON-RPC 2.0 校验：
        - 空行或仅空白行会被忽略（返回 `None`），仅在 stderr 记录 `empty line skipped` 日志；
        - 无法解析为合法 JSON 时，返回 JSON-RPC 2.0 错误响应，`error.code = -32600 (INVALID_REQUEST)`，`id = null`；
        - 能解析为 JSON 但结构无法反序列化为 `JsonRpcRequest`，同样以 `INVALID_REQUEST` 形式返回；
        - `jsonrpc` 字段存在但不等于 `"2.0"` 时，返回 `INVALID_REQUEST`，并保留原始 `id`；
        - 当 `method` 不在 `SUPPORTED_METHODS = ["Surf.Scan", "Surf.Status", "Surf.GetResults", "Surf.Cancel"]` 中时，返回 `METHOD_NOT_FOUND`，`error.data.detail` 中包含实际方法名；
        - 当方法名在上述支持列表中但尚未真正实现时，同样返回 `METHOD_NOT_FOUND`，`error.data.detail = "method not implemented yet"`。
      - 对于需要返回响应的请求，服务会将 JSON-RPC 错误响应行写回客户端，并在 stderr 打印一条包含 peer 地址、原始请求行与响应内容的日志，便于后续排查协议与调用问题。
      - 文件顶部定义了 `JsonRpcRequest`、`JsonRpcError`、`JsonRpcErrorResponse` 等结构体以及 `INVALID_REQUEST` / `METHOD_NOT_FOUND` 常量，基本与 Architecture.md 4.3.2 中的错误模型保持一致（当前仅实现了部分标准错误码）。
      - 在同一文件的 `#[cfg(test)] mod tests` 中，已针对 `handle_rpc_line` 编写多条单元测试，用于覆盖“无效 JSON”“缺少 `jsonrpc` 字段”“错误版本号”“未知方法”“已知方法但未实现”“空行跳过”等典型场景，确保当前错误处理逻辑在代码层面可回归。
    - `workspaces/dev-cli-tui/surf-cli/src/main.rs` 中，CLI 形态已支持服务模式开关与参数透传：
      - `Args` 定义了 `--service` / `-s` 布尔开关，以及 `--host` / `--port` 参数；当 `--service` 为真时，CLI 调用 `run_service(host, port)` 启动名为 `"surf-service"` 的子进程，并在子进程退出后直接结束当前进程。
      - `run_service` 通过 `Command::new("surf-service").arg("--host").arg(host).arg("--port").arg(port).status()` 启动服务，对子进程启动失败或非零退出码仅打印错误信息到 stderr；当前 CLI 仍仅扮演“本地服务进程启动入口”，尚未提供任何 JSON-RPC 客户端封装或健康检查命令。
    - 与本 story 的验收标准相比，当前实现已经具备“按 host/port 绑定本地 TCP 监听”“基于行分隔的 JSON-RPC 2.0 请求解析与错误响应骨架”和“针对基础校验逻辑的单元测试”，但仍然只是一个“更完整的 JSON-RPC 服务骨架”，尚未提供任何实际的扫描任务管理或结果查询能力。
      - `Surf.Scan` / `Surf.Status` / `Surf.GetResults` / `Surf.Cancel` 四个方法目前仅在错误模型中占位，所有实际请求都会收到 `METHOD_NOT_FOUND` 类型的错误响应；
      - 尚未引入任务状态机、并发控制、任务 TTL 或与 `workspaces/dev-core-scanner/surf-core` 的集成，也未实现优雅关闭逻辑，因此**当前服务二进制仍不满足本 story 的验收标准**。
  - remaining_todos:
    - 方向一：任务管理与并发控制（围绕 `max_concurrent_scans` / `task_ttl_seconds` 等参数落地）
      - 在服务进程内引入最小可用的任务管理器：为每个扫描请求分配唯一 `task_id`，维护任务状态（如 Pending / Running / Completed / Failed / Canceled）及必要的元数据/结果引用；
      - 基于 `--max-concurrent-scans` 限制同时运行的扫描任务数量，对超出并发上限的请求给出明确的错误或排队语义；
      - 基于 `--task-ttl-seconds` 为完成/失败任务实现 TTL 回收策略，避免长期堆积在内存中；
      - 在任务状态管理和并发控制上补充必要的日志与错误码，以便在高并发场景下排查问题。
    - 方向二：与 `surf-core` 的扫描 API 集成
      - 将 JSON-RPC 中的 `Surf.Scan` 映射为对 `workspaces/dev-core-scanner/surf-core` 的一次扫描调用，负责构造输入参数并启动实际扫描任务；
      - 在任务管理器中落地对扫描进度和结果的追踪，使 `Surf.Status` 能够通过 `task_id` 返回任务当前状态和关键指标（进度百分比、已扫描文件数/大小等）；
      - 为 `Surf.GetResults` 提供结构化结果返回通道（可复用 CLI/单次运行模式中的结果结构或其子集），并在必要时支持分页或裁剪；
      - 为 `Surf.Cancel` 提供取消正在运行扫描任务的能力（或至少实现“标记为取消并在安全点终止”的语义），并在取消结果中给出可诊断信息。
    - 方向三：为 JSON-RPC 方法提供初始业务实现与端到端验证路径
      - 在现有错误处理骨架基础上，为 `Surf.Scan` / `Surf.Status` / `Surf.GetResults` / `Surf.Cancel` 提供最小可用的业务实现，确保在单机环境下可以完整走通“发起扫描 → 查询进度 → 获取结果/取消”的闭环；
      - 制定并实现统一的 JSON-RPC 错误模型和日志策略（包括协议错误、业务参数错误、内部错误等），使错误码与 `error.data.detail` 能够为调用方提供稳定且可诊断的信息；
      - 补充服务模式的基础端到端验证路径（可先以手工和简单脚本为主）：在本机运行 `surf --service --host 127.0.0.1 --port 1234` 启动服务后，使用 `netcat`、`socat` 或自定义小型 CLI 客户端向 `127.0.0.1:1234` 发送 JSON-RPC 请求，覆盖成功/失败及边界场景；
      - 在具备网络访问或依赖缓存的环境中，为 `surf-service` 引入最小的集成测试或脚本化验收步骤（例如放在 `workspaces/dev-service-api/surf-service/tests` 或上层 CI 流水线中），确保上述端到端路径可在 CI 中自动回归；
      - 继续评估是否需要在 `surf` CLI 中增加简单的 JSON-RPC 客户端子命令（例如 `surf rpc status --host ... --port ... --task-id ...`），以便在没有 GUI 的环境下也能完成健康检查与任务管理；如纳入本 story，则在实现与测试完成后更新本节验收标准，否则在后续 story 中单独拆分并在 PRD 中补充边界说明。

### 9.3 待确认问题

- 【CLI 单次运行】在使用 `--json` 输出时，终端是否仍需展示进度条？若展示，是否有对 JSON 消费方不会造成干扰的输出分流约定（例如进度仅输出到 stderr）？
  - 【设计决策】如无特别说明，CLI 在 `--json` 模式下仍允许展示进度条，但进度条和日志统一输出到 stderr，stdout 仅在扫描成功时一次性输出完整 JSON；扫描失败、参数错误或被中断（包括 Ctrl+C）时，stdout 不输出任何 JSON，仅在 stderr 输出错误说明，并以非零状态码退出。

- 【CLI 单次运行】当用户通过 `Ctrl+C` 中断扫描时，是否需要输出部分结果（例如已扫描结果的 JSON 或表格）？当前 PRD 仅要求“合理时间内结束且不留下半写入的输出文件”，但对标准输出的行为尚未明确。
  - 【设计决策】如无特别说明，CLI 在 JSON 和表格模式下均不输出部分结果；当用户通过 Ctrl+C 中断扫描时，程序清理进度条，在 stderr 输出“用户中断”提示，并以 130 等非零退出码退出，保证不会产生半写入的输出文件或结构化结果。后续如对“部分结果输出”有强需求，可在未来迭代中单独立项扩展相应语义与行为。
