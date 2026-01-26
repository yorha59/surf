#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT_DIR"

if [[ ! -f "PRD.md" || ! -f "AGENTS.md" ]]; then
  echo "[ralph] 必须在 Surf 仓库根目录运行，并且需要 PRD.md / AGENTS.md" >&2
  exit 1
fi

STATE_FILE="ralph_state.json"
if [[ ! -f "$STATE_FILE" ]]; then
  cat >"$STATE_FILE" <<'EOF'
{
  "phase": "design",
  "iteration": 0,
  "reason": "初始状态：如 Architecture.md 缺失或设计尚未完成，应先进入设计阶段由 design-architect 创建/更新架构文档。",
  "notes": []
}
EOF
  echo "[ralph] 初始化全局状态文件 ralph_state.json，phase=design" >&2
fi

if [[ ! -f "Architecture.md" ]]; then
  echo "[ralph] 提示：未找到 Architecture.md，本轮应从设计阶段重新构建架构文档" >&2
fi

echo "[ralph] Surf Ralph-loop 启动，无最大迭代次数限制（需手动停止或任务完成）" >&2

# 将任意文本做最小 JSON 转义，用于飞书文本消息
ralph_json_escape() {
  local s="$1"
  s=${s//\\/\\\\}   # 反斜杠
  s=${s//"/\\"}       # 双引号
  s=${s//$'\n'/\\n}   # 换行
  s=${s//$'\r'/\\r}   # 回车
  s=${s//$'\t'/\\t}   # 制表符
  printf '%s' "$s"
}

send_feishu_text() {
  local text="$1"
  local webhook="${FEISHU_WEBHOOK:-}"

  # 未配置 FEISHU_WEBHOOK 时不发送
  if [[ -z "$webhook" ]]; then
    return 0
  fi

  local escaped
  escaped="$(ralph_json_escape "$text")"

  curl -sS -X POST "$webhook" \
    -H 'Content-Type: application/json' \
    -d "{\"msg_type\":\"text\",\"content\":{\"text\":\"$escaped\"}}" \
    >/dev/null || true
}

# 保留旧命名，向后兼容：状态文本和完整日志都通过同一发送逻辑
send_feishu_status() {
  send_feishu_text "$1"
}

ralph_git_guard_with_coco() {
  # 通过 RALPH_GIT_GUARD=0 可关闭该检查
  if [[ "${RALPH_GIT_GUARD:-1}" != "1" ]]; then
    return 0
  fi

  if ! command -v coco >/dev/null 2>&1; then
    echo "[ralph] 警告：未找到 coco CLI，跳过 git guard 检查" >&2
    return 0
  fi

  # 让 Coco 作为编排者检查待提交文件中是否包含构建产物目录，
  # 并维护根目录 .gitignore：
  # - 对确认为构建产物且存在于仓库中的目录/路径，追加或保留 ignore 规则；
  # - 对 .gitignore 中已经不存在且不再需要的路径，可以适当清理；
  # 然后在回复中总结变更，并给出 GIT_GUARD_OK=<true|false> 标记。
  local GUARD_PROMPT
  GUARD_PROMPT=$(cat <<'EOF'
你现在的角色是 Surf 仓库的编排者，专门负责在自动提交前进行一次「git 提交安全检查」。

当前工作目录是 Surf 仓库根目录，请你在本轮中只做下面这件事：

1. 使用 Bash 工具运行：git status --porcelain
   - 检查当前待提交/已修改的文件和目录。
   - 重点识别「明显属于构建产物或缓存」的路径，例如但不限于：
     - Rust / Cargo 构建目录：target/ 及其子目录；
     - 一般构建输出：dist/、build/、release/ 等；
     - 前端常见产物：.next/、out/、node_modules/ 等；
     - 各 workspaces/* 子 crate 或子工程中的上述目录；
     - 其它通过 git status 可以明显判断为构建产物的目录。

2. 使用 Read / ApplyPatch 工具检查并维护根目录 .gitignore：
   - 为被你判定为构建产物、且实际存在于仓库中的目录或文件模式，追加或保持 ignore 规则；
   - 对 .gitignore 中指向已经不存在且不再需要的具体路径，可以适度清理；
   - 保持规则「最小必要修改」：
     - 尽量只增加缺失的构建产物模式；
     - 删除时仅删除明显无效且不会影响现有忽略行为的条目。

3. 再次运行 git status --porcelain，自检：
   - 确认输出中不再包含明显的构建产物目录或文件；
   - 注意：源码、配置、文档等正常文件可以继续保留在 git status 中，这里只关心构建产物是否仍然可见。

4. 在回复末尾，按照下面格式单独输出一行标记：
   - 若你认为当前 git status 中已经没有明显构建产物，请输出：
     GIT_GUARD_OK=true
   - 若仍然发现构建产物难以通过 .gitignore 排除，或存在你无法安全判断的路径，请输出：
     GIT_GUARD_OK=false

5. 在标记行之前，用简明的中文小结：
   - 本轮你对 .gitignore 做了哪些新增或删除；
   - 若 GIT_GUARD_OK=false，说明原因（例如某些路径难以判断是否为构建产物）。

只执行上述检查和 .gitignore 维护工作，不要再触发额外的需求/设计/开发/交付阶段操作。
EOF
)

  echo "[ralph] 调用 coco 执行 git guard 检查 .gitignore 与构建产物..." >&2
  # 失败时不阻断后续流程，只打印警告
  if ! coco -y --query-timeout "${COCO_GIT_GUARD_TIMEOUT:-5m}" -p "$GUARD_PROMPT"; then
    echo "[ralph] git guard 调用 coco 失败，后续自动提交将按当前 .gitignore 执行" >&2
  fi
}

ralph_git_auto_commit() {
  # 通过 RALPH_AUTO_COMMIT=0 关闭自动提交
  if [[ "${RALPH_AUTO_COMMIT:-1}" != "1" ]]; then
    return 0
  fi

  # 在自动提交前调用一轮 git guard，由 Coco 维护 .gitignore 中的构建产物规则
  ralph_git_guard_with_coco

  if ! command -v git >/dev/null 2>&1; then
    echo "[ralph] 警告：未找到 git，跳过本轮自动提交" >&2
    return 0
  fi

  # 若无变更则不提交
  if [[ -z "$(git status --porcelain)" ]]; then
    echo "[ralph] 本轮无代码变更，跳过自动提交" >&2
    return 0
  fi

  echo "[ralph] 检测到本轮有变更，准备自动提交并推送" >&2

  # 依赖 .gitignore（target、__pycache__ 等）避免构建产物被加入版本控制
  if ! git add -A; then
    echo "[ralph] git add 失败，跳过本轮自动提交" >&2
    return 0
  fi

  local step="$1"
  local ts
  ts="$(date +'%Y-%m-%d %H:%M:%S' 2>/dev/null || echo '')"
  local msg="chore(ralph): iteration ${step:-?} ${ts:+at $ts}"

  if ! git commit -m "$msg"; then
    echo "[ralph] git commit 失败（可能无变更或钩子错误），跳过本轮自动提交" >&2
    return 0
  fi

  if ! git push; then
    echo "[ralph] git push 失败，请稍后手动检查远端同步" >&2
  fi
}

step=1
while :; do
  echo "" >&2
  echo "================ Ralph 第 $step 轮 ================" >&2

  # 将当前轮次开始事件同步到飞书（如配置了 FEISHU_WEBHOOK）
  send_feishu_status "[ralph] 第 $step 轮开始"

  # 为本轮构造编排 Agent 提示词
  # 使用带引号的 EOF，避免其中的反引号等被 Bash 误当作命令执行
  PROMPT=$(cat <<'EOF'
你是 Surf 仓库中的"编排 Agent"（orchestrator）。

当前仓库根目录包含以下关键文件（其中部分在当前轮次可能尚未存在）：
- PRD.md：最新需求文档（必需）
- Architecture.md：最新架构设计与开发 Agent 拆分（如不存在，说明设计阶段尚未完成或需要从头补齐）
- AGENTS.md：编排/节点协作规则与 Ralph 事件循环说明
- ralph_state.json：全局状态文件，用于记录当前处于需求/设计/开发/交付的哪一阶段，以及最近一轮 Ralph 的原因说明（详见 AGENTS.md 3.2）。

本次调用是 Ralph 事件循环中的「第 $step 轮」。

请你在**单次调用**内，围绕 Surf 项目推进**一小步、可闭环**的工作，遵守 AGENTS.md 中的状态机与回退规则：
- 你自己扮演编排 Agent
- 在做任何其他事情之前，先使用 Read 工具读取根目录下的 ralph_state.json，理解当前全局阶段（phase）以及上一次 Ralph 的原因说明；
- 若当前不存在 Architecture.md，或 ralph_state.json 中的 phase 被标记为 "design" 且 reason 显示需要补充/修正架构，则应优先进入「设计阶段」，通过 Task 调用 design-architect 子 Agent，在本轮内至少产出一个最小可用的 Architecture.md 初稿（包含核心模块划分和开发 Agent 列表），为后续迭代打基础，而不是跳过设计直接进入开发/交付；
- 若 Architecture.md 已存在，且 ralph_state.json 中的 phase 指向 requirements/development/delivery，则结合当前 PRD / Architecture / 各工作区状态，选择本轮最合适的阶段（需求/设计/开发/交付）
- 如需要调用节点 Agent（requirements-manager / design-architect / feature-developer / delivery-runner），通过 Task 工具完成
- 只推进一个清晰的子目标（例如：澄清一个需求点、补全一段设计、实现一个小功能、在交付阶段增加/执行一组测试等）
- 完成后，在回复中简要说明：你做了什么、更改了哪些文件、还有哪些风险或 TODO

重要：
- 不要尝试等待命令行里的「人工即时回答」；你无法在本次调用中与用户进行交互式问答。
- 如需「PRD 确认」「架构确认」或其它需要人类决策/操作的交互，请：
  - 将问题汇总写入根目录的 `human.md` 中（遵循 AGENTS.md 和 human.md 中的格式与约定），包括：报告问题的 Agent 或模块、问题发生的目录/工作区、问题描述和建议的人类动作；
  - 在你的回复中简要提示本轮新增了哪些人类待办事项，方便外部查看。
  所有这些信息会在下一轮 Ralph 调用时，通过 `human.md` 再次被你读到，并在下一轮优先处理。

特别要求（供 Ralph-loop 脚本解析）：
- 请在思考与说明的最后，单独输出一行：
  RALPH_DONE=<true|false>
- 当你认为：
  - 当前 PRD 范围内的所有 story/功能点
  - 已经完成「需求 → 设计 → 开发 → 交付（含独立测试）」完整闭环
  - 或本轮已无有意义的下一步可执行工作
  时，请输出 RALPH_DONE=true；否则输出 RALPH_DONE=false。
- 如果你认为本轮产生了需要人类显式决策或修改文档的内容（例如 PRD 或 Architecture 中存在待确认问题），
  请在 RALPH_DONE 行之后再单独输出一行：
  HUMAN_REQUIRED=<true|false>
  当你希望外部 Ralph 循环在本轮结束后暂停，等待人类处理这些事项时，请输出 HUMAN_REQUIRED=true。
- 请在 HUMAN_REQUIRED 行之后再单独输出一行：
  AGENTS_USED=<逗号分隔的 agent 名称>
  其中必须包含 orchestrator（表示你自己作为主编排 Agent），
  若本轮通过 Task 调用了节点 Agent，请按实际情况追加节点类型名称，例如：
    AGENTS_USED=orchestrator,requirements-manager
    AGENTS_USED=orchestrator,design-architect,feature-developer
    AGENTS_USED=orchestrator,feature-developer,delivery-runner
  若本轮未调用任何节点 Agent，仅输出：
    AGENTS_USED=orchestrator

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

  done_flag="false"
  human_flag="false"
  agents="orchestrator"

  # 提取本轮使用的 Agent 列表（如果 Coco 遵循了 AGENTS_USED 约定）
  if agent_line=$(echo "$RESPONSE" | grep -E "AGENTS_USED=" | tail -n1); then
    agents="${agent_line#AGENTS_USED=}"
  fi

  echo "[ralph] 第 $step 轮 Agent: $agents" >&2

  if echo "$RESPONSE" | grep -q "RALPH_DONE=true"; then
    done_flag="true"
    echo "[ralph] 检测到 RALPH_DONE=true，本轮需求已闭环，退出循环" >&2
  fi

  if echo "$RESPONSE" | grep -q "RALPH_DONE=false"; then
    done_flag="false"
    echo "[ralph] 本轮仍有剩余工作，准备进入下一轮" >&2
  else
    echo "[ralph] 警告：未检测到 RALPH_DONE 标记，默认认为任务尚未完成，继续下一轮" >&2
  fi

  if echo "$RESPONSE" | grep -q "HUMAN_REQUIRED=true"; then
    human_flag="true"
    echo "[ralph] 检测到 HUMAN_REQUIRED=true，本轮需要人类确认，暂停循环" >&2
  fi

  # 将本轮完整控制台信息同步到飞书（如配置了 FEISHU_WEBHOOK）
  # 内容包含轮次标记、Coco 输出以及结束状态，便于在飞书侧完整查看本轮执行情况
  if [[ -n "${FEISHU_WEBHOOK:-}" ]]; then
    full_round_msg=$(cat <<EOF
================ Ralph 第 $step 轮 ================
[ralph] 第 $step 轮开始

$RESPONSE

[ralph] 第 $step 轮结束: RALPH_DONE=${done_flag}, HUMAN_REQUIRED=${human_flag}, AGENTS_USED=${agents}
EOF
)
    send_feishu_text "$full_round_msg"
  fi

  # 每轮结束后由编排脚本执行一次自动提交/推送（如有变更），避免提交构建产物
  ralph_git_auto_commit "$step"

  if [[ "$done_flag" == "true" || "$human_flag" == "true" ]]; then
    break
  fi

  step=$((step + 1))
done

echo "[ralph] Ralph-loop 结束，共运行 $((step - 1)) 轮" >&2
