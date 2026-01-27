# dev-macos-gui 本轮开发任务（todo）

**本轮状态：本轮阻塞（环境）**  
- 当前 rustc 版本：`rustc 1.86.0`（< 1.88.0，Tauri 后端需 `rustc >= 1.88.0`，参见 Architecture.md 10.1）；  
- 受影响命令：`cargo check --manifest-path src-tauri/Cargo.toml`、`cargo build`、`npm run tauri:dev`、`npm run tauri:build` 等依赖 Tauri 宿主编译的命令；  
- 现有前端 Vite + `/rpc` 代理联调能力与历史自测记录保持有效（详见下文“自测记录”段落）。

## 本轮新增任务：GUI Onboarding 初始化配置与统一配置路径

- [x] 在 Tauri 后端（`src-tauri/src/main.rs`）实现 `read_config` / `write_config` 命令：
  - 解析并写入统一配置路径 `~/.config/surf/config.json`；
  - 使用 `tauri::api::path::home_dir()` 展开 `~`，若目录不存在则创建 `~/.config/surf/`；
  - 配置结构与 Architecture.md 4.5.1 的核心字段保持一致（`default_path` / `threads` / `min_size` / `rpc_host` / `rpc_port` / `theme` / `language` / `cli_path`）；
  - 对不可解析的旧文件尝试备份为 `config.json.bak`，并让前端进入 Onboarding 流程重新生成配置。
- [x] 在前端 `ServiceClient.tsx` 中新增配置模块（类型与工具函数）：
  - 定义 `SurfConfig` 类型和 `createDefaultConfig()`，按 4.5.1 约定提供默认值：`default_path="~/"`、`min_size="100MB"`、`rpc_host="127.0.0.1"`、`rpc_port=1234`、`threads=hardwareConcurrency`；
  - 封装 `readConfig()` / `writeConfig()`，通过 Tauri `invoke("read_config")` / `invoke("write_config")` 访问后端；
  - 在非 Tauri 环境（仅 Vite 浏览器开发）下优雅降级：`readConfig()` 返回 `null`，`writeConfig()` 仅输出 warning，不阻塞 GUI 使用。
- [x] 改造 `App.tsx` 启动流程：
  - 应用启动时调用 `readConfig()` 检查 `~/.config/surf/config.json` 是否存在且可解析；
  - 若读取成功则直接进入主界面并将配置透传给子组件；若不存在/不可解析则基于 `createDefaultConfig()` 预填一份默认配置并进入 Onboarding 流程；
  - 将 Onboarding 完成回调与侧边栏设置的修改统一通过 `persistConfig()` 落盘到配置文件。
- [x] 将 Onboarding 从纯展示页改造为配置表单：
  - 接收 `initialConfig` 并渲染默认扫描路径、线程数、最小过滤大小和 JSON-RPC host/port 输入；
  - 点击「开始使用 Surf」时回调上层并触发配置写入；
  - 文案中明确本步骤会生成 `~/.config/surf/config.json`，作为 GUI / 服务 / CLI 共享的配置文件。
- [x] 在 `Sidebar.tsx` 中加入“全局配置（占位设置）”面板：
  - 展示并允许编辑当前 `default_path` / `rpc_host` / `rpc_port`；
  - 点击「保存设置」时通过 `onConfigChange` 回调写回 `~/.config/surf/config.json`，满足“设置面板修改后回写同一路径”的要求；
  - 保留下方收藏路径区为占位说明，等待后续设计与实现。

## 自测记录（本轮：Onboarding + 配置路径）

- 执行命令（在 `workspaces/dev-macos-gui/`）：
  - `npm run build`：验证前端 TypeScript 代码与 React 组件在新增配置模块后仍能正常构建；
  - `cargo check --manifest-path src-tauri/Cargo.toml`：验证 Tauri 后端新增 `read_config` / `write_config` 后的编译情况（受全局 Rust 工具链版本限制，详见下文）。

- 典型流程 1：缺配置 → 进入 Onboarding → 写入 config.json → 重启读取
  - 步骤设计：
    1. 删除或重命名本机 `~/.config/surf/config.json`（若存在）；
    2. 通过 `npm run tauri:dev` 启动 Tauri 应用（当前环境下该步骤受 rustc 版本限制，仅在具备 >=1.88.0 的环境中可完整执行）；
    3. 启动后应用调用 `read_config`，由于配置不存在返回 `None`，前端进入 Onboarding 流程，并基于 `createDefaultConfig()` 预填配置表单；
    4. 在 Onboarding 页面确认或修改默认扫描路径、线程数、最小过滤大小和 JSON-RPC 地址，点击「开始使用 Surf」，前端通过 `writeConfig` 调用 Tauri 后端写入 `~/.config/surf/config.json`；
    5. 应用切换到主界面，侧边栏“全局配置”面板展示刚才写入的配置；
    6. 重启应用后再次调用 `read_config`，成功解析配置并直接进入主界面（不再显示 Onboarding），验证配置持久化路径闭环。
  - 在当前开发环境中的真实执行情况：
    - `npm run build` 成功，前端新增的 `SurfConfig` / Onboarding / Sidebar 逻辑均通过编译；
    - `cargo check --manifest-path src-tauri/Cargo.toml` 仍然失败，错误为全局 `rustc 1.86.0` 版本低于 Tauri 依赖链中 `time` crate 所需的 `1.88.0`，属于已知环境问题；
    - 受上述限制，本轮无法在本机真正运行 `npm run tauri:dev` 来验证 Tauri 侧文件读写，但从代码路径上已实现「缺配置 → Onboarding → 写入 config.json → 重启读取」的完整链路。

- 典型流程 2：存在配置 → 跳过 Onboarding 直接读取
  - 预期行为：当 `~/.config/surf/config.json` 存在且可被 `SurfConfig` 成功解析时：
    1. 应用启动调用 `read_config` 返回 `Some(config)`；
    2. `App.tsx` 将 `isOnboarding` 置为 `false`，直接渲染 `MainLayout`，并将配置通过 props 传入 `Sidebar`；
    3. 侧边栏“全局配置”面板中显示当前配置值，用户修改后点击「保存设置」会通过 `writeConfig` 回写同一路径；
    4. 再次启动应用时，新的配置值会被读取并填充到 UI 中。
  - 在当前环境下，由于无法运行 Tauri 宿主，上述流程在真实文件系统上的验证仍待具备更高 `rustc` 版本的环境完成；前端侧逻辑（状态切换与表单更新）已在构建与代码审查层面完成自检。

- 与历史自测的衔接：
  - 之前一轮已通过 `npm run dev` + `dev-service-api` 验证了 `/rpc` 代理与 JSON-RPC 端到端路径（详见本文件下方旧记录）；
  - 本轮在此基础上补齐了配置文件读写路径与 Onboarding 流程，未来在具备可用 Tauri 构建环境时可合并验证两条路径：
    - GUI 启动 → 读取配置 → 直连 `http://<rpc_host>:<rpc_port>/rpc` → 执行扫描任务。

# 历史任务与自测记录（保留）

- [x] 初始化 Tauri + React 的 macOS GUI 项目骨架（创建 `src-tauri` 与前端基础结构）。
- [x] 实现 Onboarding 与主界面（侧边栏、顶部栏、中央视图）基础占位 UI。
- [x] 实现 JSON-RPC 客户端占位（Rust `rpc_client` 与前端 `ServiceClient`），预留与服务端集成接口。
- [x] 编写 `README.md`，记录依赖安装、开发与打包命令以及本地服务连接说明。
- [x] 自测基础构建/运行路径，更新本文件状态并将发现的问题记录到 `bug.md`（如有）。

## 历史新增任务：JSON-RPC 客户端占位 → 最小端到端集成

- [x] 在前端 `ServiceClient.tsx` 中实现 JSON-RPC 客户端：
  - 提供 `startScan(path, options?)` / `getStatus(taskId)` / `getResult(taskId)` / `cancel(taskId)` 方法；
  - 使用 JSON-RPC 2.0 请求体 `{ jsonrpc, method, params, id }` 直连本地服务（通过 `/rpc` → `127.0.0.1:1234` 代理）；
  - 统一处理网络错误与 RPC `error` 字段，抛出结构化 `RpcError` 异常。
- [x] 在 `CentralView.tsx` 中接入最小端到端流程：
  - 提供扫描路径输入框与「开始扫描」「取消当前任务」按钮；
  - 周期性轮询 `scan.status` 并在任务状态栏展示 `state` / `progress` / `scanned_files` / `scanned_bytes` 等信息；
  - 在任务完成后调用 `scan.result` 并将 `summary.top_files` / `top_files` 渲染为列表。
- [x] 在 `TopBar.tsx` 中增加 JSON-RPC 连接指示点与简要状态文案。
- [x] 更新 `vite.config.ts`，为 `/rpc` 增加到 `http://127.0.0.1:1234` 的代理以规避浏览器 CORS 限制。
- [x] 更新本工作区 `README.md`，新增「本地 JSON-RPC 连接自测」章节，记录端到端验证步骤与预期结果。

## 历史自测记录（JSON-RPC 联调）

- 已在本工作区执行：
  - `npm install`（历史已完成，本轮未变更依赖）；
  - `npm run build`；
  - `cd ../dev-service-api && cargo run --release -- --service --host 127.0.0.1 --port 1234`；
  - `npm run dev -- --host 127.0.0.1 --port 5173`，通过浏览器访问 `http://127.0.0.1:5173/`；
  - 在 Onboarding 页面点击「开始使用 Surf」，进入主界面后点击 TopBar 右侧「开始扫描（联调）」按钮，观察控制台日志与 TopBar 简要摘要。
- 结果：
  - 前端 Vite 构建成功，`ServiceClient` / `CentralView` / `TopBar` 的 TypeScript 代码在构建阶段无类型错误，生成 `dist/` 目录成功；
  - `dev-service-api` 成功在 `http://127.0.0.1:1234/rpc` 暴露 HTTP JSON-RPC 服务；
  - 通过 Vite 代理 `fetch("/rpc")` 成功访问后端，TopBar 联调按钮完成一次最小端到端扫描（`scan.start` → 周期性 `scan.status` → `scan.result`），浏览器控制台可见 `demo scanStart` / `demo scanStatus` / `demo scanResult` 日志，且 TopBar 显示 Demo 总大小与 TopFiles 数量摘要；
  - 联调成功界面的截图已保存为 `workspaces/dev-macos-gui/e2e-tc1.png`；
  - `cargo check --manifest-path src-tauri/Cargo.toml` 在当前环境下仍失败，原因是全局 `rustc 1.86.0` 版本低于 Tauri 依赖链中 `time` crate 所需的 `1.88.0`（详见 `bug.md`），该问题与本轮 JSON-RPC 联调无直接关系，保持为已知环境限制。

- 结论：
  - 历史迭代已完成「开发模式下通过 `fetch("/rpc")` 联通本地 `surf-service`」的最小端到端路径；
  - Rust Tauri 宿主编译限制仍然存在，需在具备更高 `rustc` 版本的环境中进行打包与 Tauri 侧进一步开发。
