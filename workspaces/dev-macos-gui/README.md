# Surf macOS GUI（dev-macos-gui 工作区）

本工作区包含 Surf 项目的 macOS GUI 应用（基于 **Tauri + React + Vite**）的初始骨架实现。

> 本轮目标：提供可编译的 GUI 项目骨架、基础 UI 布局以及与 JSON-RPC 服务的调用占位，方便后续迭代逐步接入真实扫描能力。

## 1. 目录结构概览

相对于仓库根目录 `workspaces/dev-macos-gui/`：

- `src-tauri/`：Tauri 宿主应用（Rust）
  - `Cargo.toml`：Rust crate 配置（应用名 `surf_gui`）。
  - `tauri.conf.json`：Tauri 配置文件（产品名 `Surf`，标识 `dev.surf`）。
  - `src/main.rs`：Tauri 应用入口。
  - `src/rpc_client.rs`：面向 JSON-RPC 服务的客户端占位模块（TCP `127.0.0.1:1234`，返回模拟数据）。
- `src/`：前端代码（React + Vite）
  - `main.tsx`：前端入口。
  - `App.tsx`：顶层应用组件，负责在 Onboarding 与主界面之间切换。
  - `components/Onboarding.tsx`：Onboarding 占位页面。
  - `components/MainLayout.tsx`、`Sidebar.tsx`、`TopBar.tsx`、`CentralView.tsx`：主界面布局骨架组件。
  - `services/ServiceClient.ts`：前端侧 JSON-RPC 客户端封装（占位实现）。
- 根目录下辅助文件：
  - `package.json` / `tsconfig*.json` / `vite.config.ts` / `index.html`：前端与 Tauri 构建配置。
  - `todo.md`：本工作区开发任务清单。
  - `bug.md`：本工作区缺陷记录。

## 2. 环境依赖

### 2.1 基础依赖

- Node.js（建议 18+）
- npm（或兼容的包管理器，如 pnpm/yarn）
- Rust 工具链（建议 `rustup` + stable toolchain，Edition 2021）

### 2.2 Tauri 相关依赖（macOS）

参见官方文档：https://tauri.app/ ，典型依赖包括：

- Xcode Command Line Tools
- `cargo` / Rust 编译器
- `@tauri-apps/cli`（由 `devDependencies` 安装）

## 3. 安装依赖

在仓库根目录下执行（或先 `cd` 到本工作区）：

```bash
cd workspaces/dev-macos-gui
npm install
```

> 若当前环境无法访问 npm / crates.io，依赖安装可能失败。这种情况下可以仅在本地具备完整环境的机器上执行构建与运行。

## 4. 开发与运行

### 4.1 仅启动前端开发服务器（不启用 Tauri 窗口）

用于调试 React UI 布局：

```bash
cd workspaces/dev-macos-gui
npm run dev
```

访问终端输出中的本地地址（通常为 `http://localhost:5173`），可看到 Onboarding 与主界面占位视图。

### 4.2 启动 Tauri 应用（推荐在 macOS 上）

```bash
cd workspaces/dev-macos-gui
npm run tauri dev
```

该命令将：

- 编译 Rust 侧 Tauri 应用（`src-tauri/`）。
- 启动前端 dev server，并在 Tauri 窗口中加载 GUI。

> 注意：若本机尚未安装 Tauri CLI 或相关系统依赖，命令可能失败。可参考 Tauri 文档安装依赖，或先只使用 `npm run dev` 进行纯前端 UI 骨架调试。

## 5. 打包构建

在 macOS 环境中构建 `.app` 或打包产物：

```bash
cd workspaces/dev-macos-gui
npm run tauri build
```

构建成功后，Tauri 默认会在 `src-tauri/target/release/bundle/macos/` 下生成 `Surf.app` 等产物。交付阶段可在 `delivery-runner` 工作区中收集这些产物并复制到 `release/gui/` 目录。

> 如果当前环境不具备完整的 macOS 图形构建链路（例如在 Linux 或 CI 中运行），上述命令可能失败。此时建议仅在本地 macOS 开发机上执行打包，并将产物同步到本仓库对应的交付工作区。

## 6. JSON-RPC 服务集成占位

### 6.1 Rust 侧 `rpc_client` 占位模块

- 默认目标地址：`127.0.0.1:1234`（与 Architecture/PRD 中 JSON-RPC 服务约定一致）。
- 当前实现为 **占位实现**：
  - 未真正建立 TCP 连接；
  - `scan_start` / `scan_status` / `scan_result` / `scan_cancel` 等函数返回模拟数据或占位错误；
  - 为后续集成 `dev-service-api` 提供统一的调用接口与数据结构位置。

未来迭代中，可以在该模块内引入 `tokio` / `serde_json` 等依赖，通过 JSON-RPC 2.0 协议与实际的服务端通信。

### 6.2 前端侧 `ServiceClient` 占位封装

- 提供 TypeScript 类 `ServiceClient`，对 GUI 侧使用暴露：
  - `scanStart` / `scanStatus` / `scanResult` / `scanCancel` 等方法（返回 Promise）。
- 当前实现仅返回模拟数据或「服务未连接」占位信息。
- 未来可切换为：
  - 通过 Tauri `invoke` 调用 Rust 侧命令；或
  - 直接通过 JSON-RPC over TCP/WebSocket 与服务端通信（由后续设计/实现决策）。

在主界面中，当检测到 `ServiceClient` 报告服务未连接时，会在中央视图展示明显的占位提示，提醒用户启动本地 `surf-service` 或配置正确的服务地址。

## 7. 自测说明

本轮自测建议步骤：

1. 确认 TypeScript 与 Rust 源码均能在本地通过基本编译（视环境而定）：
   - `cd workspaces/dev-macos-gui && npm run build`（或 `npm run lint` 等）
   - `cd workspaces/dev-macos-gui && cargo check --manifest-path src-tauri/Cargo.toml`
2. 启动前端 dev server，确认 UI 占位渲染正常：
   - `cd workspaces/dev-macos-gui && npm run dev`
   - 访问本地地址，确认 Onboarding、主界面骨架布局以及「服务未连接」提示显示正常。

> 当前环境下若无法完整执行上述命令，请在本地 macOS 开发机上验证。自测结果与已知问题会同步记录在本工作区的 `todo.md` 与 `bug.md` 中。
