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