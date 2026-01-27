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
  - 本轮已在 macOS GUI 开发工作区内完成「开发模式下通过 `fetch("/rpc")` 联通本地 `surf-service`」的最小端到端路径，并通过实际运行 `dev-service-api` + Vite + 浏览器点击 TopBar 按钮完成一次真实扫描；
  - 从 GUI 视角看，「联调路径打通」顶层任务已完成，后续可在此基础上扩展为完整的扫描任务管理与结果可视化；
  - Rust Tauri 宿主编译仍受全局工具链版本限制，需在具备更高 `rustc` 版本的环境中进行后续打包与 Tauri 侧集成开发（属于环境问题，不阻塞本轮联调目标）。
