# dev-macos-gui 本轮开发任务（todo）

- [x] 初始化 Tauri + React 的 macOS GUI 项目骨架（创建 `src-tauri` 与前端基础结构）。
- [x] 实现 Onboarding 与主界面（侧边栏、顶部栏、中央视图）基础占位 UI。
- [x] 实现 JSON-RPC 客户端占位（Rust `rpc_client` 与前端 `ServiceClient`），预留与服务端集成接口。
- [x] 编写 `README.md`，记录依赖安装、开发与打包命令以及本地服务连接说明。
- [x] 自测基础构建/运行路径，更新本文件状态并将发现的问题记录到 `bug.md`（如有）。

## 自测记录（本轮）

- 已在本工作区执行：
  - `npm install`
  - `npm run build`
- 结果：
  - 前端 Vite 构建成功，生成 `dist/` 目录，可正常打包 React 占位页面。
  - `cargo check --manifest-path src-tauri/Cargo.toml` 在当前环境下失败，原因是全局 `rustc 1.86.0` 版本低于 Tauri 依赖链中 `time` crate 所需的 `1.88.0`。已尝试将 `tauri` 版本固定为 `1.5.0`，仍受同一限制影响。
- 结论：
  - 本轮在代码层面已完成 Tauri 宿主与前端骨架搭建；Rust 侧编译受环境工具链版本限制，需在具备更高 `rustc` 版本的环境中重新运行构建/打包命令。
