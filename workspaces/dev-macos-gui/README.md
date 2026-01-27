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
  - `services/ServiceClient.ts`：前端侧 JSON-RPC 客户端封装（JSON-RPC 集成）。
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
   - `cd workspaces/dev-macos-gui && cargo check --manifest-path src-tauri/Cargo.toml`（当前受 `rustc 1.86.0` 限制，详见 `bug.md`）
2. 启动前端 dev server，确认 UI 占位渲染正常：
   - `cd workspaces/dev-macos-gui && npm run dev`
   - 访问本地地址，确认 Onboarding、主界面骨架布局以及 JSON-RPC 相关状态栏渲染正常。

> 当前环境下若无法完整执行上述命令，请在本地 macOS 开发机上验证。自测结果与已知问题会同步记录在本工作区的 `todo.md` 与 `bug.md` 中。

## 8. 本地 JSON-RPC 连接自测

本节描述在不升级全局 `rustc`（当前为 1.86.0，Tauri 侧无法编译）的前提下，仅通过前端 React 应用直连 JSON-RPC 服务完成最小端到端验证的推荐步骤。

### 8.1 启动 JSON-RPC 服务

在仓库根目录下，优先使用 `dev-service-api` 工作区内已有的测试脚本或已构建好的二进制：

```bash
# 方式一：使用测试脚本（推荐）
./workspaces/dev-service-api/test_service.sh

# 方式二：直接运行已构建的服务二进制
./workspaces/dev-service-api/target/release/surf-service \
  --service --host 127.0.0.1 --port 1234
```

预期：服务在 `127.0.0.1:1234` 以 JSON-RPC 2.0 协议监听请求。

### 8.2 启动 GUI 前端（仅前端，不启动 Tauri）

```bash
cd workspaces/dev-macos-gui
npm install   # 如已安装依赖可跳过
npm run dev
```

在浏览器中访问终端输出中的本地地址（通常为 `http://localhost:5173`）。

> `vite.config.ts` 中已配置将 `/rpc` 代理到 `http://127.0.0.1:1234`，因此在 dev 模式下无需额外处理 CORS 问题。

### 8.3 端到端扫描验证步骤

1. 在 GUI 中进入主界面（通过 Onboarding 后）。
2. 在中央「最小端到端扫描（JSON-RPC）」区域：
   - 在「扫描路径」输入框中填入一个本地目录路径，例如：
     - `workspaces/delivery-runner/test/tmp/tc1.Wp8z`，或
     - 你本机上的任意测试目录（建议包含少量文件以便快速完成）。
   - 点击「开始扫描」。
3. 观察右侧状态栏与列表：
   - 顶部任务状态栏应显示任务状态（`queued`/`running`/`completed` 等）、进度百分比、`scanned_files` 与 `scanned_bytes`；
   - 当任务状态进入 `completed` 后，下方「Top 文件列表」会展示从 JSON-RPC `scan.result` 返回的 `summary.top_files` / `top_files` 列表（路径 + 大小等信息）。
4. 如在浏览器控制台或 UI 中看到错误提示（例如无法连接服务、端口占用等），可参考以下排查方向：
   - 确认 `surf-service` 是否仍在 `127.0.0.1:1234` 监听；
   - 检查是否有防火墙或安全软件拦截本地连接；
   - 如需更改端口，可同步调整 `vite.config.ts` 代理配置或在未来迭代表层增加可配置项。

### 8.4 预期结果与已知限制

- 预期结果：
  - 成功发起扫描并在 GUI 中看到进度更新；
  - 扫描完成后，在「Top 文件列表」中渲染来自 JSON-RPC 服务的文件列表。
- 已知限制：
  - 当前迭代仅实现最小端到端流程，尚未接入 Treemap 视图与完整目录树展示；
  - Tauri 宿主应用仍受 `rustc 1.86.0` 限制，未在本环境中编译/运行，相关问题记录在 `bug.md`；
  - 若 JSON-RPC 服务端使用的传输层与浏览器期望不一致（例如纯 TCP 而非 HTTP），则可能需要在后续迭代中引入额外的本地代理或调整服务实现。
