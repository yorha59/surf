#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT_DIR"

if [[ ! -f "PRD.md" || ! -f "Architecture.md" || ! -f "AGENTS.md" ]]; then
  echo "[ralph] 必须在 Surf 仓库根目录运行，并且需要 PRD.md / Architecture.md / AGENTS.md" >&2
  exit 1
fi

echo "[ralph] Surf Ralph-loop 启动，无最大迭代次数限制（需手动停止或任务完成）" >&2

step=1
while :; do
  echo "" >&2
  echo "================ Ralph 第 $step 轮 ================" >&2

  # 为本轮构造编排 Agent 提示词
  PROMPT=$(cat <<EOF
你是 Surf 仓库中的“编排 Agent”（orchestrator）。

当前仓库根目录包含以下关键文件：
- PRD.md：最新需求文档
- Architecture.md：最新架构设计与开发 Agent 拆分
- AGENTS.md：编排/节点协作规则与 Ralph 事件循环说明

本次调用是 Ralph 事件循环中的「第 $step 轮」。

请你在**单次调用**内，围绕 Surf 项目推进**一小步、可闭环**的工作，遵守 AGENTS.md 中的状态机与回退规则：
- 你自己扮演编排 Agent
- 根据当前 PRD / Architecture / 代码状态，选择本轮最合适的阶段（需求/设计/开发/交付）
- 如需要调用节点 Agent（requirements-manager / design-architect / feature-developer / delivery-runner），通过 Task 工具完成
- 只推进一个清晰的子目标（例如：澄清一个需求点、补全一段设计、实现一个小功能、在交付阶段增加/执行一组测试等）
- 完成后，在回复中简要说明：你做了什么、更改了哪些文件、还有哪些风险或 TODO

重要：
- 不要尝试等待命令行里的「人工即时回答」；你无法在本次调用中与用户进行交互式问答。
- 如需「PRD 确认」「架构确认」这类交互，请：
  - 直接在 PRD.md / Architecture.md 中补充建议、问题列表或 TODO；或
  - 在你的回复中明确写出「人类下一步需要执行的动作」（例如需要用户在线确认的点）。
  所有这些信息会在下一轮 Ralph 调用时，通过仓库文件再次被你读到。

特别要求（供 Ralph-loop 脚本解析）：
- 请在思考与说明的最后，单独输出一行：
  RALPH_DONE=<true|false>
- 当你认为：
  - 当前 PRD 范围内的所有 story/功能点
  - 已经完成「需求 → 设计 → 开发 → 交付（含独立测试）」完整闭环
  - 或本轮已无有意义的下一步可执行工作
  时，请输出 `RALPH_DONE=true`；否则输出 `RALPH_DONE=false`。

请现在开始本轮工作，并遵守以上输出约定。
EOF
)

  # 调用 coco 执行本轮编排工作
  # 使用 -y 自动允许工具调用，避免交互式确认导致脚本卡住
  # 使用 --query-timeout 限制单轮查询时间，默认 10 分钟，可通过 COCO_QUERY_TIMEOUT 环境变量覆写
  # 通过 tee 让用户实时看到编排者输出，同时保存在临时文件中供后续解析
  echo "[ralph] 调用 coco 执行第 $step 轮编排..." >&2
  TMPFILE="$(mktemp -t surf-ralph-XXXXXX)"

  set +e
  coco -y --query-timeout "${COCO_QUERY_TIMEOUT:-10m}" -p "$PROMPT" | tee "$TMPFILE"
  COCO_EXIT=$?
  set -e

  if (( COCO_EXIT != 0 )); then
    echo "[ralph] 调用 coco 失败，退出码: $COCO_EXIT" >&2
    # 将本轮已产生的输出也打印到 stderr，方便排查
    if [[ -s "$TMPFILE" ]]; then
      echo "[ralph] coco 部分输出如下：" >&2
      cat "$TMPFILE" >&2 || true
    fi
    rm -f "$TMPFILE"
    exit "$COCO_EXIT"
  fi

  RESPONSE="$(cat "$TMPFILE")"
  rm -f "$TMPFILE"

  if echo "$RESPONSE" | grep -q "RALPH_DONE=true"; then
    echo "[ralph] 检测到 RALPH_DONE=true，本轮需求已闭环，退出循环" >&2
    break
  fi

  if echo "$RESPONSE" | grep -q "RALPH_DONE=false"; then
    echo "[ralph] 本轮仍有剩余工作，准备进入下一轮" >&2
  else
    echo "[ralph] 警告：未检测到 RALPH_DONE 标记，默认认为任务尚未完成，继续下一轮" >&2
  fi

  step=$((step + 1))
done

echo "[ralph] Ralph-loop 结束，共运行 $((step - 1)) 轮" >&2
