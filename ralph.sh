#!/usr/bin/env bash

set -euo pipefail

# 解析命令行参数
SHOW_HELP=0

for arg in "$@"; do
    case "$arg" in
        --help|-h)
            SHOW_HELP=1
            ;;
        *)
            echo "[ralph] 未知参数: $arg" >&2
            exit 1
            ;;
    esac
done

# 显示帮助信息
if [[ $SHOW_HELP -eq 1 ]]; then
    echo "Surf Ralph-loop 编排脚本"
    echo "用法: ./ralph.sh"
    echo ""
    echo "说明:"
    echo "  本脚本总是在 tmux 会话中执行 coco，便于实时观察执行流程和调试"
    echo ""
    echo "环境变量:"
    echo "  COCO_QUERY_TIMEOUT  设置 coco 查询超时（默认: 10m）"
    echo "  FEISHU_WEBHOOK    飞书 webhook 地址，用于通知"
    echo ""
    exit 0
fi

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

# 在 tmux 会话中运行 coco，便于观察执行流程
run_coco_in_tmux() {
    local step="$1"
    local prompt="$2"
    local session_name="surf-ralph-${step}"
    
    # 清理所有旧的 surf-ralph-* 会话
    echo "[ralph] 清理旧的 tmux 会话..." >&2
    local old_sessions
    old_sessions=$(tmux list-sessions 2>/dev/null | grep -E '^surf-ralph-' | cut -d: -f1) || true
    for old_session in $old_sessions; do
        echo "[ralph] 终止旧会话: $old_session" >&2
        tmux kill-session -t "$old_session" 2>/dev/null || true
    done
    sleep 1
    
    # 创建 tmux 会话，运行 coco 但不传递 prompt
    tmux new-session -d -s "$session_name" -n "coco" \
        "coco -y --query-timeout \"${COCO_QUERY_TIMEOUT:-10m}\""
    
    # 等待 coco 启动并完全加载
    echo "[ralph] 等待 coco 启动完成..." >&2
    local max_wait=30  # 最多等待30秒
    local wait_start=$(date +%s)
    local coco_ready=0
    
    while (( $(date +%s) - wait_start < max_wait )); do
        # 捕获当前窗格内容
        local pane_content=$(tmux capture-pane -t "$session_name:coco" -p 2>/dev/null || echo "")
        
        # 检查coco是否已就绪（出现shell mode或command mode提示）
        if echo "$pane_content" | grep -q "\$ shell mode\|command mode\|⏵⏵ accept all tools"; then
            coco_ready=1
            echo "[ralph] coco 已就绪，开始发送prompt" >&2
            break
        fi
        
        # 检查是否仍在加载中（显示Working状态）
        if echo "$pane_content" | grep -q "Working.*s • ESC to interrupt"; then
            echo "[ralph] coco 仍在加载中，等待..." >&2
        fi
        
        sleep 2
    done
    
    if [[ $coco_ready -eq 0 ]]; then
        echo "[ralph] 警告：coco 启动等待超时，尝试发送prompt" >&2
    fi
    
    # 清除窗格历史，避免后续解析时匹配到 prompt 文本
    echo "[ralph] 清除窗格历史..." >&2
    tmux clear-history -t "$session_name:coco" 2>/dev/null || true
    
    # 一次性发送整个 prompt 内容
    echo "[ralph] 发送完整prompt..." >&2
    # 使用 -l 选项发送字面量字符串，避免特殊字符被解析
    tmux send-keys -t "$session_name:coco" -l "$prompt"
    tmux send-keys -t "$session_name:coco" "Enter"
    
    # 等待 coco 完成（检测 RALPH_DONE 标记）
    echo "[ralph] 等待coco执行完成（无超时限制）..." >&2
    
    # 等待一小段时间，确保prompt开始处理
    sleep 3
    
    local output=""
    while true; do
        # 检查tmux窗格是否已死（进程已退出）
        local pane_dead=$(tmux list-panes -t "$session_name:coco" -F "#{pane_dead}" 2>/dev/null || echo "1")
        if [[ "$pane_dead" == "1" ]]; then
            echo "[ralph] 检测到tmux窗格进程已退出" >&2
            # 即使没有RALPH_DONE标记，也退出等待
            break
        fi
        
        # 捕获整个窗格历史（从开头到现在）
        local current_output=$(tmux capture-pane -t "$session_name:coco" -p -S - 2>/dev/null || echo "")
        
        if [[ -n "$current_output" ]]; then
            output="$current_output"
            
            # 检查是否包含完成标记（允许前后空格和等号前后空格，精确匹配）
            if echo "$output" | grep -E -q "^[[:space:]]*RALPH_DONE[[:space:]]*=[[:space:]]*(true|false)[[:space:]]*$"; then
                echo "[ralph] 检测到 RALPH_DONE 标记" >&2
                # 再等待2秒，确保所有输出都被捕获
                sleep 2
                # 最后捕获一次完整输出
                output=$(tmux capture-pane -t "$session_name:coco" -p -S - 2>/dev/null || echo "")
                break
            fi
            
            # 清除屏幕，避免重复显示
            clear >&2
            # 显示当前窗格内容
            echo "[ralph] tmux 窗格实时输出：" >&2
            echo "$output" >&2
        fi
        
        sleep 2
    done
    
    
    # 输出结果
    if [[ -n "$output" ]]; then
        echo "$output"
    else
        echo "无输出"
    fi
    
    # 解析 flag 并写入文件（如果提供了 flag_file 参数）
    local flag_file="${3:-}"
    if [[ -n "$flag_file" ]]; then
        # 提取 RALPH_DONE 值（精确匹配整行，允许前后空格和等号前后空格）
        local ralph_done="false"
        if echo "$output" | grep -E -q "^[[:space:]]*RALPH_DONE[[:space:]]*=[[:space:]]*true[[:space:]]*$"; then
            ralph_done="true"
        elif echo "$output" | grep -E -q "^[[:space:]]*RALPH_DONE[[:space:]]*=[[:space:]]*false[[:space:]]*$"; then
            ralph_done="false"
        fi
        
        # 提取 HUMAN_REQUIRED 值（精确匹配整行，允许前后空格和等号前后空格）
        local human_required="false"
        if echo "$output" | grep -E -q "^[[:space:]]*HUMAN_REQUIRED[[:space:]]*=[[:space:]]*true[[:space:]]*$"; then
            human_required="true"
        elif echo "$output" | grep -E -q "^[[:space:]]*HUMAN_REQUIRED[[:space:]]*=[[:space:]]*false[[:space:]]*$"; then
            human_required="false"
        fi
        
        # 提取 AGENTS_USED 值（匹配行首，允许等号前后空格，取最后一行）
        local agents_used="orchestrator"
        if agent_line=$(echo "$output" | grep -E "^[[:space:]]*AGENTS_USED[[:space:]]*=" | tail -n1); then
            # 移除行首空格和AGENTS_USED=前缀（注意等号前后可能有空格）
            # 先找到等号的位置
            local prefix="${agent_line%%=*}"
            # 移除前缀和等号
            agents_used="${agent_line#*=}"
            agents_used="${agents_used#"${agents_used%%[![:space:]]*}"}"  # 去除前导空格
        fi
        
        # 写入 flag 文件
        cat > "$flag_file" <<EOF
RALPH_DONE=$ralph_done
HUMAN_REQUIRED=$human_required
AGENTS_USED=$agents_used
EOF
        echo "[ralph] Flag 已写入: $flag_file" >&2
    fi
    
    # 调试模式：保留 tmux 会话供观察
    echo "[ralph] 调试模式：tmux 会话 '$session_name' 已保留，请使用 'tmux attach -t $session_name' 查看" >&2
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

**编排 Agent 核心职责**：
- 作为主编排者，你**只负责协调调度和状态机流转**，不执行任何具体的代码更新、构建、测试或文档编辑工作
- 所有具体工作必须通过 Task 工具调用相应的节点 Agent 完成
- 你的主要工作是：读取全局状态 → 判断当前阶段 → 调用相应节点 Agent → 更新全局状态

**工作流程**：
1. **首先读取全局状态**：使用 Read 工具读取根目录下的 `ralph_state.json`，理解当前全局阶段（phase）以及上一次 Ralph 的原因说明
2. **状态机决策**：
   - 若当前不存在 `Architecture.md`，或 `ralph_state.json` 中的 phase 被标记为 "design" 且 reason 显示需要补充/修正架构，则应优先进入「设计阶段」，通过 Task 调用 `design-architect` 子 Agent
   - 若 `Architecture.md` 已存在，结合当前 PRD / Architecture / 各工作区状态，选择本轮最合适的阶段（需求/设计/开发/交付）
3. **调用节点 Agent**：
   - 需求阶段：调用 `requirements-manager`
   - 设计阶段：调用 `design-architect`
   - 开发阶段：根据 `Architecture.md` 中的开发 Agent 列表，并行调用相应的 `feature-developer`
   - 交付阶段：调用 `delivery-runner`
4. **向上反馈原则**：
   - 交付节点发现问题 → 反馈给研发 Agent（feature-developer）
   - 研发 Agent 发现问题 → 反馈给架构 Agent（design-architect）
   - 架构 Agent 判断：如果是产品设计问题 → 反馈给设计师 Agent（requirements-manager）
   - 设计师 Agent 判断：如果确实需要用户协作（如无网络、环境问题等）→ 写入 `human.md`
   - **重要**：只有架构师和设计师才能判断是否需要用户协助，其他 Agent 必须遵循向上反馈原则

**重要规则**：
- **禁止编排者直接工作**：你不得执行任何代码更新、测试、构建或文档编辑工作，所有具体工作必须通过 Task 调用节点 Agent 完成
- **人类待办问题限制**：只有 `design-architect`（架构师）和 `requirements-manager`（设计师）可以反馈需要人类决策的问题到 `human.md`
- **向上反馈链条**：交付 → 研发 → 架构 → 设计师 → 用户（如必要）
- **单步推进**：每轮只推进一个清晰的子目标，例如：调用一个节点 Agent 完成其阶段内的一小步工作
- **状态更新**：在本轮结束时，使用 ApplyPatch 工具更新 `ralph_state.json` 中的 phase / iteration / reason / notes

**输出约定**：
- 请在思考与说明的最后，单独输出一行：
  RALPH_DONE=<true|false>
- 当你认为：
  - 当前 PRD 范围内的所有 story/功能点已经完成「需求 → 设计 → 开发 → 交付（含独立测试）」完整闭环
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

请现在开始本轮工作，严格遵守以上编排规则。
EOF
)

  # 调用 coco 执行本轮编排工作
  # 使用 -y 自动允许工具调用，避免交互式确认导致脚本卡住
  # 使用 --query-timeout 限制单轮查询时间，默认 10 分钟，可通过 COCO_QUERY_TIMEOUT 环境变量覆写
  echo "[ralph] 调用 coco 执行第 ${step} 轮编排..." >&2
  FLAGFILE="$(mktemp -t surf-ralph-flags-XXXXXX)"

  set +e
  echo "[ralph] 在 tmux 中执行（会话名: surf-ralph-${step}）" >&2
  # 直接捕获 run_coco_in_tmux 的输出，不通过 tee 和临时文件
  RESPONSE=$(run_coco_in_tmux "$step" "$PROMPT" "$FLAGFILE")
  COCO_EXIT=$?
  set -e

  if (( COCO_EXIT != 0 )); then
    echo "[ralph] 调用 coco 失败，退出码: $COCO_EXIT" >&2
    # 将本轮已产生的输出也打印到 stderr，方便排查
    if [[ -n "$RESPONSE" ]]; then
      echo "[ralph] coco 部分输出如下：" >&2
      echo "$RESPONSE" >&2 || true
    fi
    rm -f "$FLAGFILE"
    exit "$COCO_EXIT"
  fi
  
  # 读取 flag 文件（如果存在）
  if [[ -f "$FLAGFILE" ]]; then
    echo "[ralph] 从 flag 文件读取解析结果: $FLAGFILE" >&2
    # 解析 flag 文件
    while IFS='=' read -r key value; do
      case "$key" in
        RALPH_DONE)
          if [[ "$value" == "true" ]]; then
            done_flag="true"
          else
            done_flag="false"
          fi
          ;;
        HUMAN_REQUIRED)
          if [[ "$value" == "true" ]]; then
            human_flag="true"
          else
            human_flag="false"
          fi
          ;;
        AGENTS_USED)
          agents="$value"
          ;;
      esac
    done < "$FLAGFILE"
    rm -f "$FLAGFILE"
  else
    echo "[ralph] 警告：未找到 flag 文件，使用原始解析逻辑" >&2
    # 设置默认值
    done_flag="false"
    human_flag="false"
    agents="orchestrator"
    
    # 尝试从 RESPONSE 中提取 AGENTS_USED
    if agent_line=$(echo "$RESPONSE" | grep -E "AGENTS_USED=" | tail -n1); then
      agents="${agent_line#AGENTS_USED=}"
    fi
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
