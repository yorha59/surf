#!/bin/bash

# dev-service-api 集成测试脚本
# 启动服务，发送示例请求，验证响应

set -e

# 确保相对路径始终以本脚本所在目录为基准，
# 便于从仓库根目录或任意工作目录直接调用。
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

SERVICE_BIN="./target/release/surf-service"
HOST="127.0.0.1"
PORT="1234"

# 简单的 JSON-RPC 请求发送工具：优先使用 nc，不存在时回退到 python3
send_rpc() {
    local request="$1"

    if command -v nc >/dev/null 2>&1; then
        # 使用 nc 发送单次请求并等待响应
        printf '%s' "$request" | nc "$HOST" "$PORT"
    elif command -v python3 >/dev/null 2>&1; then
        python3 - "$HOST" "$PORT" "$request" << 'PY'
import socket
import sys

host = sys.argv[1]
port = int(sys.argv[2])
request = sys.argv[3].encode('utf-8')

with socket.create_connection((host, port), timeout=5) as s:
    s.sendall(request)
    s.shutdown(socket.SHUT_WR)
    chunks = []
    while True:
        data = s.recv(4096)
        if not data:
            break
        chunks.append(data)

sys.stdout.buffer.write(b"".join(chunks))
PY
    else
        echo "错误: 未找到 nc 或 python3，无法发送 JSON-RPC 请求" >&2
        return 1
    fi
}

# 检查二进制是否存在
if [ ! -f "$SERVICE_BIN" ]; then
    echo "错误: 服务二进制未找到，请先运行 cargo build --release"
    exit 1
fi

# 启动服务（后台运行）
echo "启动服务..."
"$SERVICE_BIN" --service --host "$HOST" --port "$PORT" &
SERVICE_PID=$!

# 等待服务启动
sleep 2

# 检查服务是否在运行
if ! kill -0 $SERVICE_PID 2>/dev/null; then
    echo "错误: 服务进程未运行，可能启动失败"
    exit 1
fi

echo "服务已启动，PID: $SERVICE_PID"

# 发送 scan.start 请求
echo -e "\n=== 测试 scan.start ==="
START_REQUEST='{"jsonrpc":"2.0","id":1,"method":"scan.start","params":{"root_path":"/tmp","threads":2,"min_size":1024,"limit":10}}'
echo "请求: $START_REQUEST"
START_RESPONSE=$(send_rpc "$START_REQUEST" 2>/dev/null || echo "连接失败")
echo "响应: $START_RESPONSE"

# 提取 task_id
TASK_ID=$(echo "$START_RESPONSE" | grep -o '"task_id":"[^"]*"' | cut -d'"' -f4)
if [ -n "$TASK_ID" ]; then
    echo "提取到 task_id: $TASK_ID"
else
    echo "警告: 未提取到 task_id"
fi

# 发送 scan.status 请求
if [ -n "$TASK_ID" ]; then
    echo -e "\n=== 测试 scan.status ==="
    STATUS_REQUEST="{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"scan.status\",\"params\":{\"task_id\":\"$TASK_ID\"}}"
    echo "请求: $STATUS_REQUEST"
    STATUS_RESPONSE=$(send_rpc "$STATUS_REQUEST" 2>/dev/null || echo "连接失败")
    echo "响应: $STATUS_RESPONSE"
fi

# 等待扫描完成（模拟扫描需要时间）
echo -e "\n等待扫描完成（5秒）..."
sleep 5

# 发送 scan.result 请求
if [ -n "$TASK_ID" ]; then
    echo -e "\n=== 测试 scan.result ==="
    RESULT_REQUEST="{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"scan.result\",\"params\":{\"task_id\":\"$TASK_ID\"}}"
    echo "请求: $RESULT_REQUEST"
    RESULT_RESPONSE=$(send_rpc "$RESULT_REQUEST" 2>/dev/null || echo "连接失败")
    echo "响应: $RESULT_RESPONSE"
fi

# 发送 scan.cancel 请求（使用新任务）
echo -e "\n=== 测试 scan.cancel ==="
CANCEL_REQUEST='{"jsonrpc":"2.0","id":4,"method":"scan.cancel","params":{"task_id":"test-cancel-id"}}'
echo "请求: $CANCEL_REQUEST"
CANCEL_RESPONSE=$(send_rpc "$CANCEL_REQUEST" 2>/dev/null || echo "连接失败")
echo "响应: $CANCEL_RESPONSE"

# 停止服务
echo -e "\n停止服务..."
kill $SERVICE_PID
wait $SERVICE_PID 2>/dev/null || true

echo -e "\n测试完成！"
