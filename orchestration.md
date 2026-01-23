# Surf 编排流程总览（Orchestration）

重要: 所有的agent需要忽略这个文档, 这个文档是用来描述agent编排流程的, 没有任何与agent相关的内容, 也不作为agent的输入.

本文档总结 Surf 项目中**编排 Agent**（Orchestrator）与各工作节点 Agent 的交互方式，
以及整体「需求 → 设计 → 开发 → 交付（构建 + 独立测试）」状态机如何闭环。

## 1. 编排 Agent 总体职责

- 直接与用户/上游系统对话，将自然语言诉求拆解为阶段性任务。
- 按固定顺序调度各阶段：
  - 需求 → 设计 → 开发 → 交付（构建 + 独立测试）。
- 所有状态流转（包括回退）**只能由编排 Agent 发起**，节点之间不得私自跳转或互相调用。
- 调用的工作节点包括（当前已定义）：
  - 需求节点：`requirements-manager`
  - 设计节点：`design-architect`
  - 开发节点：多个并行的 `feature-developer`
  - 交付节点：`delivery-runner`
- 为各节点准备输入文档（`PRD.md`、`Architecture.md`、各工作区路径与产物规划等），
  收集它们的输出（文档、构建产物、测试结果），并传递给下一阶段。
- 在多开发 Agent 并行的前提下，仍然要从全局视角保证：
  - 设计拆分可以拼接成端到端路径；
  - 各开发工作区的产物可以在交付阶段统一构建/打包；
  - 整体结果可交付且可解释。

## 2. 与需求节点（requirements-manager）的交互

**阶段：需求阶段**

- 触发：
  - 用户输入中出现新增/修改需求；
  - 或从设计阶段回退，需调整需求。
- 编排 Agent 通过 Task 调用 `requirements-manager`，提供：
  - 当前仓库是否存在 `PRD.md`；
  - 当前对本轮需求的简要描述或历史上下文。
- `requirements-manager`：
  - 直接和用户对话，复述理解、提出问题、补充边界；
  - 读取/更新 `PRD.md`，采用最小必要修改维护需求基线；
  - 当用户确认需求后输出信号：
    - 「需求已确认并写入 PRD.md，可以进入设计阶段……」。
- 编排 Agent：
  - 看到确认信号后，读取最新版 `PRD.md`，作为设计阶段输入传给 `design-architect`。

## 3. 与设计节点（design-architect）的交互

**阶段：设计阶段**

### 3.1 从需求前进到设计

- 触发：
  - 需求阶段完成，`requirements-manager` 已给出「可以进入设计阶段」信号。
- 编排 Agent 调用 `design-architect`，传入：
  - 最新 `PRD.md`；
  - 当前 `Architecture.md`（如存在）；
  - 本轮新增/调整需求的重点说明。
- `design-architect` 在 `Architecture.md` 中完成：
  - 技术栈与全局架构说明；
  - 模块/Agent 拆分（后端/前端/服务层等，可拼接的一体化设计）；
  - 接口与数据契约（方法名/URL、参数、返回结构、错误码等）；
  - 开发 Agent 规划：
    - 开发 Agent 标识 dev-id（也是工作区名的一部分，如 `workspaces/dev-backend-core/`）；
    - 各 dev-id 负责范围与需参考的设计片段；
    - 各 dev-id 的完成判定标准；
  - 全局交付/编译视图：各工作区产物类型/路径，如何在交付阶段组装为最终 release。
- 设计完成后：
  - `design-architect` 向用户展示设计要点并请求确认；
  - 用户确认后输出信号：
    - 「【设计节点-完成】……建议编排者流转到开发节点。」
- 编排 Agent：
  - 在用户确认后，读取更新后的 `Architecture.md`，作为开发阶段的输入基线。

### 3.2 从研发反馈回退到设计

- 触发：
  - 开发阶段汇总多个 `feature-developer` 的设计问题；
  - 或交付/测试阶段被标记为「设计级问题」。
- 编排 Agent：
  - 在开发阶段结束后，将所有设计相关问题汇总成结构化列表；
  - 回退到设计阶段时，将该列表连同相关 `PRD.md` / `Architecture.md` 片段一并传给 `design-architect`。
- `design-architect`：
  - 针对反馈精读问题，定位到 `Architecture.md` 中的具体章节；
  - 优先在设计层面修复（补全接口、边界条件、错误处理等）；
  - 若发现需求本身有问题，在输出中标记，并建议编排 Agent 回退到需求阶段；
  - 更新设计后输出信号：
    - 「【设计节点-完成】已根据研发反馈澄清并更新架构设计，建议编排者将任务流转回开发节点。」
- 编排 Agent：
  - 更新设计基线后，按新的 `Architecture.md` 重新调用相关开发 Agent。

### 3.3 外部问题与用户协助

- 若设计中遇到无法在自动化环境中自行解决的外部问题（权限、依赖安装失败、外部系统接入等）：
  - `design-architect` 在输出中将其列为「需要用户/运维协助」的前置条件；
  - 说明问题来源、影响范围与建议方向；
  - 编排 Agent 负责将这些条件转述给用户，直到用户协助完成，依赖这些条件的能力才视为真正可交付。

## 4. 与开发节点（feature-developer）的交互

**阶段：开发阶段（唯一允许并行的阶段）**

### 4.1 并行启动与工作区划分

- 触发：
  - 设计阶段完成并经用户确认后，进入开发阶段。
- 编排 Agent：
  - 根据 `Architecture.md` 中的开发 Agent 规划，为每个 dev-id：
    - 创建对应开发工作区 `workspaces/<dev-id>/`；
    - 整理 dev-plan（该 dev-id 负责的模块/接口/目标）；
    - 提取与该 dev-id 相关的设计片段（architecture-scope）。
  - 通过 Task 并行调用多个 `feature-developer`，每个调用提供：
    - `dev-id`；
    - `workspace-root = workspaces/<dev-id>/`；
    - dev-plan；
    - architecture-scope；
    - （若为缺陷修复场景）测试反馈信息。

### 4.2 开发者节点的内部责任（编排侧关注点）

- 每个 `feature-developer`：
  - 只在自己的 `workspace-root` 下工作，维护 `todo.md` 与 `bug.md`；
  - 基于 dev-plan 和 `Architecture.md` 细化任务，完成实现与自测；
  - 完成后输出信号：
    - 「【开发节点-完成】<dev-id>：本轮开发和自测已经完成……」。
- 编排 Agent：
  - 跟踪所有 dev-id 的状态；
  - 只要有任一 dev 未完成，本轮状态机留在开发阶段，不进入交付。

### 4.3 测试回退后的处理

- 当交付阶段返回测试失败文档（例如交付工作区 `test/failures.md`）：
  - 编排 Agent 将该失败文档广播给本轮所有参与开发的 dev-id；
  - 每个 `feature-developer`：
    - 对每条失败用例，基于 `Architecture.md` 与 dev-plan 判断是否属于自己负责的模块；
    - 对认为属于本模块的用例：
      - 记录到本工作区 `bug.md`，修复并回归自测；
      - 修复后输出「【开发节点-缺陷已修复】<dev-id>……」；
    - 对认为是架构/需求问题的用例：
      - 在 `bug.md` 和输出中标记为「架构问题/需求问题」，交由编排 Agent 汇总。
- 编排 Agent：
  - 汇集各开发 Agent 的「架构/需求问题」标记后，视情况回退到设计或需求阶段。

### 4.4 回退到设计的规则

- 若任一 dev 在开发过程中发现设计难以实现或冲突：
  - dev 输出「需要设计回退」信号及问题说明；
  - 编排 Agent 记录但不立即回退，等待本轮所有 dev 完成本轮工作；
  - 结束后，汇总所有此类问题一次性回退到设计阶段。

## 5. 与交付节点（delivery-runner）的交互

**阶段：交付阶段（构建 + 独立测试 + 发布产物）**

### 5.1 进入交付阶段

- 触发：
  - 所有 `feature-developer` 均输出「【开发节点-完成】」，且无挂起的设计回退请求。
- 编排 Agent 调用 `delivery-runner`，提供：
  - 交付工作区路径 `delivery-workspace-root`；
  - 开发工作区与产物规划列表（dev-workspaces + artifacts-plan）；
  - 与交付相关的 `Architecture.md` 摘要（模块依赖和产物依赖结构）；
  - 最新 `PRD.md`；
  - 若为回归测试场景，可传入上一轮交付的 `test/case.md`/`test/failures.md`。

### 5.2 构建与 release 目录

- `delivery-runner` 在交付工作区内：
  - 从各 `workspaces/<dev-id>/` **只读**拉取构建产物；
  - 在 `release/` 目录树下完成统一构建/打包：
    - 生成最终可执行、安装包或压缩包；
    - 生成运行脚本（如 `release/run.sh`）；
    - 生成使用说明（如 `release/README.md`）。
- 构建失败：
  - 在 `release/BUILD-REPORT.md` 中记录命令与错误摘要；
  - 返回「【交付节点-构建失败】」及错误信息给编排 Agent；
  - 编排 Agent 再将问题按模块/工作区拆分，回退给相应开发 Agent 修复后重新尝试交付。

### 5.3 基于 PRD 的独立测试与 test 目录

- 构建成功后，`delivery-runner`：
  - 使用 `PRD.md` 设计测试用例，写入 `test/case.md`；
  - 在 `test/` 下创建或更新测试脚本（如 `test/run-tests.sh`、`test/run-api-tests.py`）；
  - 若涉及 Web UI/浏览器行为：
    - 在 `test/` 下的脚本中可调用已配置的 `chrome-devtools-mcp` 驱动浏览器会话；
    - 相关配置/日志也写入 `test/`（如 `test/devtools-config.json`、`test/devtools-log.md`）。
  - 执行所有用例，将结果写回 `test/case.md`。
- 若存在失败用例：
  - 在 `test/failures.md` 中集中记录所有失败用例（编号、需求、步骤、现象、期望差异）；
  - 返回「【交付节点-测试失败】」及 `test/failures.md` 给编排 Agent。
- 若全部通过：
  - 返回「【交付节点-成功】」，总结 release 产物与测试覆盖范围，留给编排 Agent 与用户沟通发布/部署。

### 5.4 编排者在测试失败时的分发策略

- 编排 Agent 接收到交付节点测试失败信号及 `test/failures.md` 后：
  - 不在交付阶段预先裁决“谁的锅”；
  - 将同一份 `test/failures.md` 广播给本轮所有参与开发的 dev-id；
  - 各 `feature-developer` 自行判断哪些失败用例属于自己负责模块，决定是否修复或标记为架构/需求问题；
  - 编排 Agent 再根据开发节点的反馈，统一决定是否回退到设计/需求阶段。

---

以上流程保证：

- 所有节点只在各自阶段内各司其职；
- 状态流转清晰且只能通过编排 Agent；
- 在多开发 Agent 并行、独立交付工作区构建 + 测试的前提下，仍能保证最终产物的一致性、可拼接性和可交付性。

