# dev-macos-gui 本轮开发任务（todo）

- [x] 初始化 Tauri + React 的 macOS GUI 项目骨架（创建 `src-tauri` 与前端基础结构）。
- [x] 实现 Onboarding 与主界面（侧边栏、顶部栏、中央视图）基础占位 UI。
- [x] 实现 JSON-RPC 客户端占位（Rust `rpc_client` 与前端 `ServiceClient`），预留与服务端集成接口。
- [x] 编写 `README.md`，记录依赖安装、开发与打包命令以及本地服务连接说明。
- [x] 自测基础构建/运行路径，更新本文件状态并将发现的问题记录到 `bug.md`（如有）。

## 本轮新增任务：JSON-RPC 客户端占位 → 最小端到端集成

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

## 自测记录（本轮）

- 已在本工作区执行：
  - `npm install`
  - `npm run build`
- 结果：
  - 前端 Vite 构建成功，`ServiceClient` 与 `CentralView` / `TopBar` 的 TypeScript 代码在构建阶段无类型错误，生成 `dist/` 目录成功；
  - `cargo check --manifest-path src-tauri/Cargo.toml` 在当前环境下仍失败，原因是全局 `rustc 1.86.0` 版本低于 Tauri 依赖链中 `time` crate 所需的 `1.88.0`（详见 `bug.md`）。
- 端到端联调说明：
  - 当前环境下未在同一机位同时启动浏览器与 `surf-service` 进行真实 GUI 联调，端到端流程按 Architecture/PRD + `dev-service-api` README 及集成测试契约实现；
  - 在具备图形环境与已运行 `surf-service`（监听 `127.0.0.1:1234`）的机器上，按 `README.md` 第 8 节步骤操作，可完成路径输入 → `scan.start` → 轮询 `scan.status` → `scan.result` → 渲染 `top_files` 的最小闭环验证；
  - 若服务端仅提供裸 TCP JSON-RPC 而非 HTTP，`/rpc` 代理将无法直接工作，此情况在 `README.md` 8.4 节中标记为已知限制，后续可通过本地 HTTP 代理或调整服务实现解决。

- 结论：
  - 本轮在 GUI 工作区内已完成前端 JSON-RPC 客户端与最小 UI 流程打通，确保在 `surf-service` 满足 HTTP JSON-RPC 入口的前提下，可实现「输入路径 → 查看进度 → 查看 Top 文件列表」的端到端闭环；
  - Rust Tauri 宿主编译仍受全局工具链版本限制，需在具备更高 `rustc` 版本的环境中进行后续打包与 Tauri 侧集成开发。
