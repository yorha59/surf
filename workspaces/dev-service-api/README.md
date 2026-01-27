# Surf JSON-RPC 服务

## 概述

提供 Surf 项目的 JSON-RPC 服务接口，支持文件扫描任务的创建、查询、取消和结果获取。

## 启动服务

```bash
cd /Users/bytedance/GitHub/surf/workspaces/dev-service-api
cargo run --release -- --service --host 127.0.0.1 --port 1234
```

不显式传递 `--host` / `--port` 时，默认监听在 `127.0.0.1:1234`。

## 接口文档

### scan.start

**功能**：创建新的扫描任务

**参数**：
```json
{
  "root_path": "/path/to/scan",
  "threads": 4,
  "min_size": 0,
  "exclude_patterns": ["*.log", "node_modules/**"],
  "stale_days": 30,
  "limit": 20
}
```

**返回**：
```json
{
  "jsonrpc": "2.0",
  "id": null,
  "result": {"task_id": "uuid-1234"},
  "error": null
}
```

### scan.status

**功能**：查询任务状态

**参数**：
```json
{
  "task_id": "uuid-1234"
}
```

**返回**：
```json
{
  "jsonrpc": "2.0",
  "id": null,
  "result": {
    "task_id": "uuid-1234",
    "state": "Running",
    "progress": 0.5,
    "result": null,
    "error": null
  },
  "error": null
}
```

### scan.result

**功能**：获取任务结果

**参数**：
```json
{
  "task_id": "uuid-1234"
}
```

**返回**：
```json
{
  "jsonrpc": "2.0",
  "id": null,
  "result": {
    "summary": {
      "root_path": "/path/to/scan",
      "total_files": 100,
      "total_dirs": 20,
      "total_size_bytes": 1024000,
      "elapsed_seconds": 5.2
    },
    "top_files": [...],
    "by_extension": [...],
    "stale_files": [...]
  },
  "error": null
}
```

### scan.cancel

**功能**：取消任务

**参数**：
```json
{
  "task_id": "uuid-1234"
}
```

**返回**：
```json
{
  "jsonrpc": "2.0",
  "id": null,
  "result": null,
  "error": null
}
```

## 测试示例

### 使用 netcat 发送请求

1. 启动服务：
```bash
cargo run --release -- --service
```

2. 在另一个终端发送请求（原始 TCP JSON-RPC，而非 HTTP）：
```bash
nc 127.0.0.1 1234
{"jsonrpc":"2.0","id":1,"method":"scan.start","params":{"root_path":"/tmp","threads":1,"min_size":0,"exclude_patterns":[],"stale_days":30,"limit":10}}
```

3. 接收响应：
```json
{"jsonrpc":"2.0","id":null,"result":{"task_id":"123e4567-e89b-12d3-a456-426614174000"},"error":null}
```

## 运行测试

```bash
cargo test
```
