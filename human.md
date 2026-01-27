# human 待确认事项

> 作用：本文件是 **当前迭代唯一的人类待办清单**，只记录「需要人类显式处理或决策」的问题。
>
> 人类与 Agent 的分工约定：
> - 你（人类）**不会直接修改 `PRD.md` / `Architecture.md` 等规范文档**，只通过本文件与编排者/各子 Agent 协作；
> - Coco / 各节点 Agent 负责基于你的决策，去更新各自负责的文档或代码工作区（例如 `PRD.md`、`Architecture.md`、`workspaces/<dev-id>/...`）。
>
> 抛出问题的约束（非常重要）：
> - 在任意一轮 Ralph 调用中，**只能由当前正在执行的 Agent 抛出自己的问题**，且这些问题必须与该 Agent 负责的工作区或文档直接相关；
> - Coco 作为 orchestrator 在该轮中可整理问题并写入 `human.md`，但每条问题的「报告 Agent」字段必须准确标注为本轮实际发现问题的节点（例如 `requirements-manager`、`design-architect`、`dev-cli-tui` 等），不得帮别的 Agent 代为“举报”；
> - 每一轮 Ralph 内，针对某个节点 Agent，只能新增反映该 Agent 当前工作上下文的有限问题集，而不一次性罗列所有历史问题，避免失去“当前轮聚焦”的语义。
>
> 使用约定：
> 1. 当当前执行的 Agent（包括 Coco 自身）判断「需要人类确认/介入」时，**必须在这里新增一条问题记录**，而不是继续往 `PRD.md` / `Architecture.md` 写 TODO。
> 2. 每条问题记录推荐包含：
>    - 报告 Agent：例如 `requirements-manager` / `design-architect` / `dev-cli-tui` / `delivery-runner` 等（必须是当前轮实际在工作的那个 Agent）；
>    - 相关目录：例如 `workspaces/dev-cli-tui/surf-cli/`、`release/linux-x86_64/cli/` 等；
>    - 问题描述：当前 Agent 在自己职责范围内发现了什么问题或需要人类决策的点；
>    - 人类决策：**由你填写**，格式为 `人类决策: ...`，紧跟在问题描述后面。
> 3. 只要本文件**非空**，下一轮 Ralph 调用 Coco 时，应优先围绕这里列出的事项工作：
>    - 解析每条「问题 + 人类决策」；
>    - 按照「报告 Agent」字段，将该问题交还给**同一个 Agent** 来处理（例如 `dev-cli-tui` 抛出的问题，后续仍由 `dev-cli-tui` 来更新自己的代码和文档，而不是简单按“产品/架构类型”重路由给其他 Agent）；
>    - 在需要时回退全局阶段（例如：如果是 `design-architect` 抛出的架构问题，则从开发/交付回退到设计阶段，由该架构 Agent 更新 `Architecture.md`；如果是 `requirements-manager` 抛出的产品问题，则回退到需求阶段）；
>    - 在回复中显式说明对每条问题的处理结果。
> 4. 当 Coco 认为本文件中的所有问题都已经按你的决策、并由对应「报告 Agent」处理完，且当前轮不再需要人类额外动作时，应当：
>    - 清空本文件中的具体问题条目，只保留标题和上述使用说明段落；
>    - 在本轮回复中说明「human.md 已处理完毕并清空」，让后续 Ralph 迭代可以重新聚焦新的 story。

---

## 待办：GUI-01 连接失败提示 - 人工测试执行

- 标题：GUI-01 连接失败提示 - 人工测试执行
- 报告 Agent：delivery-runner
- 背景：
  - 需求引用：PRD 3.2.3（macOS GUI）、8（GUI 验收标准中「首次启动」「界面保持可响应、错误信息明确」）、Architecture 4.4 / 4.4.1（`dev-macos-gui` 通过 `fetch("/rpc")` 调用 JSON-RPC 服务，`/rpc` 不可用时需在 TopBar/状态栏提示连接失败）。
  - 测试用例来源：`workspaces/delivery-runner/test/case.md` 中「用例 GUI-01 连接失败提示（服务未运行时的可见反馈）」。
  - 当前交付状态：`ralph_state.json` 显示 phase = `delivery`，`test/test-log.md` 中该用例在交付脚本下标记为 `SKIP(MANUAL)`，需在有人值守的 macOS GUI 环境中人工执行。
- 执行环境要求：
  - 平台：macOS，本机具备可运行图形界面的环境。
  - GUI：可通过以下任一方式运行 Surf GUI：
    - 开发模式：在仓库根目录下进入 `workspaces/dev-macos-gui`，执行 `npm install`（如已安装可跳过）后运行 `npm run dev`，在浏览器访问 Vite 输出地址（通常为 `http://localhost:5173`）。
    - 打包应用：如已有 `Surf.app` 或等价打包产物，可直接在 Finder 中启动应用。
  - 服务进程：**刻意不启动** JSON-RPC 服务，确保本机 `127.0.0.1:1234` 上没有 `surf-service` 在监听 `POST /rpc`。
- 操作步骤（建议）：
  1. 确认本机没有运行中的 `surf-service`：关闭可能存在的 GUI 或服务进程；如有需要，可通过 `ps`/`lsof` 等工具确认 `127.0.0.1:1234` 无监听。
  2. 启动 Surf GUI：
     - 开发模式示例：
       - 在仓库根目录执行：`cd workspaces/dev-macos-gui`
       - 如首次运行：`npm install`
       - 启动开发服务：`npm run dev`
       - 在浏览器访问 `http://localhost:5173`。
     - 或直接启动打包好的 `Surf.app`。
  3. 进入主界面或包含顶部栏/状态栏的视图，无需修改任何设置，等待界面尝试连接默认的 `/rpc` 端点。
  4. 观察界面中与服务连接状态相关的区域（TopBar 状态区、中央占位提示、弹窗等），确认在服务不可用时的用户可见反馈。
  5. 如界面提供「刷新连接」「重试」等操作，可点击一次并再次观察提示内容与交互行为。
- 预期结果：
  - 在未启动任何 JSON-RPC 服务的情况下，GUI 能在明显位置展示清晰的连接失败文案，例如「无法连接服务」「Service unavailable」等，而不是静默失败。
  - 整体界面保持可响应，不应出现长时间 loading 或卡死感知（例如指示一直转圈但无任何错误提示）。
  - 如有「重试」或「检查服务」入口，其文案能够明确指引用户去启动本地 `surf-service` 或检查网络设置。
- 人类执行记录：
  - 人类决策: PASS/FAIL + 备注（问题或证据路径，例如截图、`test/test-log.md` 片段或 GUI 日志文件路径）。
- 后续处理：
  - 如结果为 FAIL，请在本待办条目下补充问题概述（触发路径、具体 UI 表现、预期与实际差异）和相关证据路径；后续由编排 Agent 将问题回退给 `dev-macos-gui` / `dev-service-api` 或设计节点进行修复与重新设计。

---

## 待办：GUI-02 连接成功与扫描流程 - 人工测试执行

- 标题：GUI-02 连接成功与扫描流程 - 人工测试执行
- 报告 Agent：delivery-runner
- 背景：
  - 需求引用：PRD 3.2.3（图形界面模式）、3.3（数据分析与可视化）、8（macOS GUI 验收标准中「端到端扫描流程」「Treemap/列表视图」「进度反馈」）；Architecture 4.4、4.4.1、5.2（GUI → HTTP JSON-RPC → 服务端扫描数据流）。
  - 测试用例来源：`workspaces/delivery-runner/test/case.md` 中「用例 GUI-02 连接成功与扫描流程（scan.start / scan.status / scan.result）」；示例目录为 `workspaces/delivery-runner/test/tmp/tc3.l0gy`。
  - 当前交付状态：`test/test-log.md` 中该用例被交付脚本挂接并标记为 `SKIP(MANUAL)`，说明 CLI 测试已通过，但 GUI 端到端扫描流程尚未在真实 GUI 环境中验证。
- 执行环境要求：
  - 平台：macOS，具备可运行 Surf GUI 的图形环境。
  - JSON-RPC 服务：需在本机 `127.0.0.1:1234` 启动可用的 `surf-service`，推荐命令示例：
    - 优先使用交付产物（如存在）：
      - `./workspaces/delivery-runner/release/service/surf-service --service --host 127.0.0.1 --port 1234`
    - 或使用开发服务二进制：
      - `./workspaces/dev-service-api/target/release/surf-service --service --host 127.0.0.1 --port 1234`
  - GUI：
    - 开发模式示例：在仓库根目录执行：
      - `cd workspaces/dev-macos-gui`
      - `npm install`（如已安装可跳过）
      - `npm run dev`
      - 在浏览器访问 `http://localhost:5173`
    - 或使用打包好的 `Surf.app` / 其他 GUI 产物，确保其内部 `/rpc` 调用最终指向 `http://127.0.0.1:1234/rpc`。
  - 测试数据：确认 `workspaces/delivery-runner/test/tmp/tc3.l0gy` 示例目录存在（如缺失，可先通过 `workspaces/delivery-runner/test/run-tests.sh` 重新生成示例目录）。
- 操作步骤（建议）：
  1. 在仓库根目录启动 JSON-RPC 服务，例如：
     - `./workspaces/dev-service-api/target/release/surf-service --service --host 127.0.0.1 --port 1234`
  2. 启动 Surf GUI（开发模式或打包应用），并确认 GUI 中的 JSON-RPC 服务器地址配置指向 `http://127.0.0.1:1234/rpc`（如有单独配置页面）。
  3. 在 GUI 中选择扫描路径：`workspaces/delivery-runner/test/tmp/tc3.l0gy`（路径相对于仓库根目录；在 GUI 中可通过文件选择器定位到该目录）。
  4. 在 GUI 中发起一次完整扫描：点击「开始扫描」或等价操作，观察状态栏/进度指示是否开始更新。
  5. 在扫描过程中，观察：
     - 进度百分比、已扫描文件数/大小等是否随着时间推进而更新；
     - 界面是否保持可响应（可切换视图、移动窗口等）。
  6. 扫描完成后，观察中央视图：
     - Treemap 或列表视图中是否出现 `tc3.l0gy` 的扫描结果；
     - 可简要对比 CLI JSON 输出（可选：`./workspaces/delivery-runner/release/cli/surf --path workspaces/delivery-runner/test/tmp/tc3.l0gy --json`）在总文件数、目录数等统计上是否一致（允许 UI 展示形式不同）。
  7. 如 GUI 提供历史记录或重新扫描入口，可尝试点击一次，确认不会导致异常错误。
- 预期结果：
  - GUI 能成功连接本地 JSON-RPC 服务，状态栏或等价区域有清晰的「已连接」或正常状态指示，不再显示连接失败提示。
  - 发起扫描后，`scan.start` / `scan.status` / `scan.result` 调用链工作正常：进度信息可见且持续更新，无明显长时间卡死或无反馈的状态。
  - 扫描完成后，Treemap 和/或列表视图能展示 `tc3.l0gy` 的扫描结果，关键统计（文件数、目录数、总大小）与 CLI 用例 A 的结果保持一致或在可解释范围内一致。
  - 在正常操作路径下不会出现明显错误弹窗或服务崩溃；如存在错误，应有友好的错误文案而不是静默失败。
- 人类执行记录：
  - 人类决策: PASS/FAIL + 备注（例如是否对比过 CLI JSON 输出、是否观察到异常日志，附上 `test/test-log.md` 追加记录或 GUI 截图/日志路径）。
- 后续处理：
  - 如结果为 FAIL，请在本条目下补充：失败步骤（例如无法连接服务、扫描中卡死、结果与 CLI 明显不一致等）、观察到的错误文案或日志片段。后续由编排 Agent 将问题回退给 `dev-macos-gui` / `dev-service-api` 或设计节点，协助定位是前端集成问题、服务实现问题还是需求/架构不一致。

---

## 待办：GUI-03 Onboarding 流程 - 人工测试执行

- 标题：GUI-03 Onboarding 流程 - 人工测试执行
- 报告 Agent：delivery-runner
- 背景：
  - 需求引用：PRD 3.2.3 中「初始安装引导 (Onboarding)」与「用户配置设计」，以及 8 节中关于「首次启动进入 Onboarding 并落盘配置」的验收标准；Architecture 4.4（Onboarding 与配置/历史管理职责）。
  - 测试用例来源：`workspaces/delivery-runner/test/case.md` 中「用例 GUI-03 Onboarding 流程（首启与配置引导）」。
  - 当前交付状态：交付脚本在 `test/test-log.md` 中将 GUI-03 记录为 `SKIP(MANUAL)`，说明 Onboarding UI 与配置持久化尚未在真实 GUI 环境中由人工确认。
- 执行环境要求：
  - 平台：macOS，具备可运行 Surf GUI 的图形环境。
  - JSON-RPC 服务：建议在执行本用例前先按 GUI-02 的方式启动 `surf-service`，例如：
    - `./workspaces/dev-service-api/target/release/surf-service --service --host 127.0.0.1 --port 1234`
  - GUI 配置目录：需根据当前实现确认 GUI 使用的配置存储路径（例如 `~/Library/Application Support/Surf` 或 Tauri 默认配置目录），以便在测试前清空/备份。
  - GUI 运行方式：与 GUI-02 相同，可使用开发模式（`workspaces/dev-macos-gui` + `npm run dev`）或打包应用 `Surf.app`。
- 操作步骤（建议）：
  1. 关闭所有正在运行的 Surf GUI 实例，并停止可能关联的后台进程。
  2. 找到当前实现使用的 GUI 配置目录，将其删除或临时移动到备份位置（例如：`mv "~/Library/Application Support/Surf" "~/Library/Application Support/Surf.bak-gui03"`；具体路径以实际实现为准）。
  3. 确认 JSON-RPC 服务已按前述命令在本机启动（可通过日志或端口监听确认）。
  4. 在配置被清空的前提下首次启动 Surf GUI：
     - 开发模式：在 `workspaces/dev-macos-gui` 下执行 `npm run dev` 并访问 `http://localhost:5173`；或
     - 直接启动打包应用 `Surf.app`。
  5. 观察应用启动后的首屏，确认是否自动进入 Onboarding 流程，包含至少以下步骤：
     - 欢迎页介绍 Surf 核心功能；
     - Full Disk Access 权限申请/引导提示；
     - 默认扫描路径、线程数、最小过滤大小等基础配置；
     - 本地 `surf-service` 状态检查（如服务未运行，是否有相应提示或自动启动行为）。
  6. 按 Onboarding 流程完成全部步骤，并在结束时确认应用是否进入主界面（Main Dashboard）。
  7. 关闭 GUI，确认配置目录中生成了新的配置文件/数据（如 JSON/SQLite 等）。
  8. 再次启动 GUI（在不删除配置的前提下），观察是否直接进入主界面而非重复 Onboarding，并检查：
     - 默认扫描路径、线程数、最小过滤大小等是否与前一次 Onboarding 中的选择一致；
     - 如有 JSON-RPC 地址配置，是否保持为上次设置的值。
- 预期结果：
  - 在无配置的首启场景下，应用自动进入 Onboarding 流程，所有步骤可顺利完成且无明显阻塞或崩溃。
  - 完成 Onboarding 后，用户的基础配置（默认路径、线程数、最小过滤大小、JSON-RPC 地址等）被正确持久化到配置目录中。
  - 后续正常启动时，默认直接进入主界面，并复用之前的配置；如需要重新进入 Onboarding，应在设置或菜单中提供清晰入口，而不是每次启动都重复 Onboarding。
  - 整个过程中，如遇权限不足或服务不可用，应通过清晰的文案提示用户，而非静默失败。
- 人类执行记录：
  - 人类决策: 你来测试, 而不是让我来测试
- 后续处理：
  - 如结果为 FAIL，请在本条目下简要说明：
    - Onboarding 未自动出现、流程无法完成或存在致命错误的具体表现；
    - 配置未落盘或落盘后未被 GUI 正确加载的细节。
  - 后续由编排 Agent 将问题回退给 `dev-macos-gui` 和/或 `dev-service-api`，必要时回退到设计/需求阶段以调整 Onboarding 相关约定。

---

## 待办：ENV-01 macOS GUI/Tauri 工具链升级（rustc >= 1.88.0）

- 标题：ENV-01 macOS GUI/Tauri 工具链升级（rustc >= 1.88.0）
- 报告 Agent：requirements-manager
- 相关目录：workspaces/dev-macos-gui/
- 背景/依据：
  - 引用 Architecture.md 第 10.1、10.2 节：macOS GUI / Tauri 编译至少需要 rustc >= 1.88.0，DMG 构建依赖 macOS + Xcode Command Line Tools + hdiutil。
  - 当前阻塞属于本地开发/构建环境问题，而非架构设计或 HTTP /rpc 接口契约问题；现有 HTTP /rpc 主路径保持不变。
- 执行环境要求：
  - 平台：macOS。
  - Rust 工具链：通过 rustup stable 通道提供的 rustc，版本要求 >= 1.88.0。
  - Xcode Command Line Tools：已安装，用于后续 DMG 构建等相关工具链支持。
  - 系统工具：内置 hdiutil 可用。
- 建议操作步骤（人类）：
  1. 执行 `rustup update stable` 升级 Rust 工具链，升级完成后运行 `rustc --version`，确认版本号 >= 1.88.0。
  2. 如尚未安装 Xcode Command Line Tools，执行 `xcode-select --install` 并按提示完成安装；如已安装可跳过此步骤。
  3. 环境升级完成后，在仓库根目录进入 `workspaces/dev-macos-gui`，按照该目录下 README 或现有脚本说明执行构建命令（当前建议命令：`npm install && npm run tauri build`），并在成功构建后完成一次 macOS GUI 端到端自测。
- 人类决策: ...
