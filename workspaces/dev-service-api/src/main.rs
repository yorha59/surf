use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use clap::{CommandFactory, Parser};
use serde::{de::Error as DeError, Deserialize, Deserializer, Serialize};
use serde_json::{json, Value};
use surf_core::{ScanRequest, ScanResult, ScanState, Scanner};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use uuid::Uuid;

// JSON-RPC 请求
#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Value,
    method: String,
    params: Option<Value>,
}

// JSON-RPC 响应
#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Value,
    result: Option<Value>,
    error: Option<JsonRpcError>,
}

// JSON-RPC 错误
#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    data: Option<Value>,
}

impl JsonRpcError {
    fn new(code: i32, message: &str) -> Self {
        Self {
            code,
            message: message.to_string(),
            data: None,
        }
    }
}

// 任务状态跟踪
#[derive(Debug, Clone, Serialize)]
struct TaskInfo {
    task_id: String,
    state: ScanState,
    progress: f64,
    scanned_files: u64,
    scanned_bytes: u64,
    eta_seconds: Option<u64>,
    result: Option<ScanResult>,
    error: Option<String>,
}

// 共享任务存储
type TaskStore = Arc<RwLock<HashMap<String, TaskInfo>>>;

// scan.start 参数
#[derive(Debug, Deserialize)]
struct StartScanParams {
    /// 扫描根路径，对应 Architecture.md 中的 `path`
    #[serde(alias = "path")]
    root_path: String,
    threads: Option<u16>,
    /// 最小文件大小，兼容数值字节和带单位字符串（如 "100MB"）
    #[serde(default, deserialize_with = "deserialize_size_opt")]
    min_size: Option<u64>,
    exclude_patterns: Option<Vec<String>>,
    stale_days: Option<u32>,
    limit: Option<usize>,
}

/// 解析带可选单位的文件大小字符串，支持纯数字或带单位后缀（B/KB/MB/GB/TB）。
fn parse_size_string(s: &str) -> Result<u64, String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Err("min_size cannot be empty".to_string());
    }

    // 拆分数字部分和单位部分
    let mut num_part = String::new();
    let mut unit_part = String::new();
    for ch in trimmed.chars() {
        if ch.is_ascii_digit() || ch == '.' {
            if unit_part.is_empty() {
                num_part.push(ch);
            } else {
                return Err(format!("invalid min_size format: {}", s));
            }
        } else if !ch.is_whitespace() {
            unit_part.push(ch);
        }
    }

    if num_part.is_empty() {
        return Err(format!("invalid min_size format: {}", s));
    }

    let base: f64 = num_part
        .parse()
        .map_err(|_| format!("invalid min_size number: {}", num_part))?;

    let unit = unit_part.to_ascii_uppercase();
    let multiplier: f64 = match unit.as_str() {
        "" | "B" => 1.0,
        "K" | "KB" => 1024.0,
        "M" | "MB" => 1024.0 * 1024.0,
        "G" | "GB" => 1024.0 * 1024.0 * 1024.0,
        "T" | "TB" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        _ => return Err(format!("unsupported size unit in min_size: {}", unit_part)),
    };

    let bytes = base * multiplier;
    if bytes < 0.0 {
        return Err("min_size must be non-negative".to_string());
    }

    Ok(bytes as u64)
}

/// 自定义反序列化：兼容 `u64` 或带单位的字符串。
fn deserialize_size_opt<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: Deserializer<'de>,
{
    let v = Option::<Value>::deserialize(deserializer)?;
    match v {
        None => Ok(None),
        Some(Value::Number(num)) => num
            .as_u64()
            .ok_or_else(|| DeError::custom("min_size must be a non-negative integer"))
            .map(Some),
        Some(Value::String(s)) => parse_size_string(&s)
            .map(Some)
            .map_err(|e| DeError::custom(e)),
        Some(other) => Err(DeError::custom(format!(
            "invalid min_size type: {}",
            other
        ))),
    }
}

// scan.start 响应
#[derive(Debug, Serialize)]
struct StartScanResponse {
    task_id: String,
}

// scan.status 参数
#[derive(Debug, Deserialize)]
struct GetStatusParams {
    task_id: String,
}

// scan.result 参数
#[derive(Debug, Deserialize)]
struct GetResultParams {
    task_id: String,
}

// scan.cancel 参数
#[derive(Debug, Deserialize)]
struct CancelScanParams {
    task_id: String,
}

// 处理 scan.start 方法
async fn handle_scan_start(
    params: Value,
    task_store: TaskStore,
) -> Result<JsonRpcResponse> {
    let params: StartScanParams = serde_json::from_value(params)?;

    let task_id = Uuid::new_v4().to_string();

    // 创建扫描请求
    let mut request = ScanRequest::new(params.root_path);
    request.threads = params.threads;
    request.min_size = params.min_size;
    request.exclude_patterns = params.exclude_patterns.unwrap_or_default();
    request.stale_days = params.stale_days;
    request.limit = params.limit;

    // 在任务存储中添加新任务（排队状态）
    {
        let mut store = task_store.write().await;
        store.insert(
            task_id.clone(),
            TaskInfo {
                task_id: task_id.clone(),
                state: ScanState::Queued,
                progress: 0.0,
                scanned_files: 0,
                scanned_bytes: 0,
                eta_seconds: None,
                result: None,
                error: None,
            },
        );
    }

    // 启动异步扫描任务
    let task_store_clone = task_store.clone();
    let task_id_clone = task_id.clone();
    tokio::spawn(async move {
        let scanner = Scanner::new();
        let mut store = task_store_clone.write().await;
        let task = store.get_mut(&task_id_clone).unwrap();
        task.state = ScanState::Running;
        task.progress = 0.1; // 初始进度
        drop(store);

        // 执行同步扫描（在 tokio 任务中）
        let result = scanner.scan_sync(&request);

        let mut store = task_store_clone.write().await;
        let task = store.get_mut(&task_id_clone).unwrap();
        match result {
            Ok(scan_result) => {
                task.state = ScanState::Completed;
                task.progress = 1.0;
                task.scanned_files = scan_result.summary.total_files;
                task.scanned_bytes = scan_result.summary.total_size_bytes;
                task.result = Some(scan_result);
            }
            Err(e) => {
                task.state = ScanState::Failed;
                task.error = Some(e.to_string());
            }
        }
    });

    Ok(JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: Value::Null,
        result: Some(serde_json::to_value(StartScanResponse { task_id }).unwrap()),
        error: None,
    })
}

// 处理 scan.status 方法
async fn handle_scan_status(
    params: Value,
    task_store: TaskStore,
) -> Result<JsonRpcResponse> {
    let params: GetStatusParams = serde_json::from_value(params)?;

    let store = task_store.read().await;
    match store.get(&params.task_id) {
        Some(info) => Ok(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: Value::Null,
            result: Some(serde_json::to_value(info.clone()).unwrap()),
            error: None,
        }),
        None => Ok(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: Value::Null,
            result: None,
            error: Some(JsonRpcError::new(-32602, "Invalid task_id")),
        }),
    }
}

// 处理 scan.result 方法
async fn handle_scan_result(
    params: Value,
    task_store: TaskStore,
) -> Result<JsonRpcResponse> {
    let params: GetResultParams = serde_json::from_value(params)?;

    let store = task_store.read().await;
    match store.get(&params.task_id) {
        Some(info) => {
            // 对齐 Architecture.md 6.2.3：返回 task_id + 扁平化的 ScanResult 字段
            if let Some(scan_result) = &info.result {
                let payload = json!({
                    "task_id": info.task_id,
                    "summary": scan_result.summary.clone(),
                    "top_files": scan_result.top_files.clone(),
                    "by_extension": scan_result.by_extension.clone(),
                    "stale_files": scan_result.stale_files.clone(),
                });

                Ok(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Value::Null,
                    result: Some(payload),
                    error: None,
                })
            } else {
                Ok(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Value::Null,
                    result: None,
                    error: Some(JsonRpcError::new(
                        -32001,
                        "Result not ready for the given task_id",
                    )),
                })
            }
        }
        None => Ok(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: Value::Null,
            result: None,
            error: Some(JsonRpcError::new(-32602, "Invalid task_id")),
        }),
    }
}

// 处理 scan.cancel 方法
async fn handle_scan_cancel(
    params: Value,
    task_store: TaskStore,
) -> Result<JsonRpcResponse> {
    let params: CancelScanParams = serde_json::from_value(params)?;

    let mut store = task_store.write().await;
    match store.get_mut(&params.task_id) {
        Some(info) => {
            if info.state == ScanState::Running || info.state == ScanState::Queued {
                info.state = ScanState::Canceled;
            }
            Ok(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: Value::Null,
                result: Some(Value::Null),
                error: None,
            })
        }
        None => Ok(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: Value::Null,
            result: None,
            error: Some(JsonRpcError::new(-32602, "Invalid task_id")),
        }),
    }
}

// 处理 JSON-RPC 请求
async fn handle_request(
    request: JsonRpcRequest,
    task_store: TaskStore,
) -> Result<JsonRpcResponse> {
    // 先拷贝一份请求 id，避免在 match 分支中 move
    let req_id = request.id.clone();

    let mut response = match request.method.as_str() {
        "scan.start" => {
            let params = request.params.ok_or_else(|| anyhow::anyhow!("Missing params"))?;
            handle_scan_start(params, task_store).await?
        }
        "scan.status" => {
            let params = request.params.ok_or_else(|| anyhow::anyhow!("Missing params"))?;
            handle_scan_status(params, task_store).await?
        }
        "scan.result" => {
            let params = request.params.ok_or_else(|| anyhow::anyhow!("Missing params"))?;
            handle_scan_result(params, task_store).await?
        }
        "scan.cancel" => {
            let params = request.params.ok_or_else(|| anyhow::anyhow!("Missing params"))?;
            handle_scan_cancel(params, task_store).await?
        }
        _ => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: req_id.clone(),
            result: None,
            error: Some(JsonRpcError::new(-32601, "Method not found")),
        },
    };

    // 遵循 JSON-RPC 2.0 规范：回显请求 id
    response.id = req_id;

    Ok(response)
}

/// 命令行参数解析
#[derive(Parser, Debug)]
#[command(name = "surf-service", version = "0.1.0", about = "Surf JSON-RPC 服务端")]
struct Args {
    /// 启动服务模式
    #[arg(short = 's', long = "service", help = "启动 JSON-RPC 服务模式")]
    service: bool,

    /// 服务监听地址
    #[arg(long = "host", default_value = "127.0.0.1", help = "服务监听地址（默认: 127.0.0.1）")]
    host: String,

    /// 服务监听端口
    #[arg(long = "port", default_value = "1234", help = "服务监听端口（默认: 1234）")]
    port: u16,
}

#[tokio::main]
async fn main() -> Result<()> {
    // 解析命令行参数
    let args = Args::parse();

    // 如果没有指定 --service 标志，显示帮助信息并退出
    if !args.service {
        eprintln!("Surf JSON-RPC 服务需要使用 --service 标志启动。");
        // 打印 clap 自动生成的帮助信息
        let _ = Args::command().print_help();
        eprintln!();
        return Ok(());
    }

    let task_store = Arc::new(RwLock::new(HashMap::new()));

    println!("Surf JSON-RPC Server listening on {}:{}", args.host, args.port);

    let addr = format!("{}:{}", args.host, args.port);
    let listener = TcpListener::bind(addr).await?;

    loop {
        let (mut socket, _) = listener.accept().await?;
        let task_store = task_store.clone();

        tokio::spawn(async move {
            let mut buf = [0; 4096];
            loop {
                match socket.read(&mut buf).await {
                    Ok(0) => {
                        // 连接关闭
                        break;
                    }
                    Ok(n) => {
                        let data = &buf[..n];
                        let req_str = match String::from_utf8(data.to_vec()) {
                            Ok(s) => s,
                            Err(e) => {
                                let err = JsonRpcResponse {
                                    jsonrpc: "2.0".to_string(),
                                    id: Value::Null,
                                    result: None,
                                    error: Some(JsonRpcError::new(-32700, &format!("Parse error: {}", e))),
                                };
                                let err_json = serde_json::to_string(&err).unwrap();
                                let _ = socket.write_all(err_json.as_bytes()).await;
                                break;
                            }
                        };

                        println!("Received request: {}", req_str.trim());

                        let req: JsonRpcRequest = match serde_json::from_str(&req_str) {
                            Ok(r) => r,
                            Err(e) => {
                                let err = JsonRpcResponse {
                                    jsonrpc: "2.0".to_string(),
                                    id: Value::Null,
                                    result: None,
                                    error: Some(JsonRpcError::new(-32700, &format!("Parse error: {}", e))),
                                };
                                let err_json = serde_json::to_string(&err).unwrap();
                                let _ = socket.write_all(err_json.as_bytes()).await;
                                continue;
                            }
                        };

                        let resp = match handle_request(req, task_store.clone()).await {
                            Ok(r) => r,
                            Err(e) => JsonRpcResponse {
                                jsonrpc: "2.0".to_string(),
                                id: Value::Null,
                                result: None,
                                error: Some(JsonRpcError::new(-32603, &format!("Internal error: {}", e))),
                            },
                        };

                        let resp_json = serde_json::to_string(&resp).unwrap();
                        println!("Sending response: {}", resp_json);
                        let _ = socket.write_all(resp_json.as_bytes()).await;
                    }
                    Err(e) => {
                        eprintln!("Error reading from socket: {}", e);
                        break;
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use tokio::sync::RwLock;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_handle_scan_start() {
        let task_store = Arc::new(RwLock::new(HashMap::new()));
        let dir = tempdir().unwrap();

        let params = serde_json::json!({
            "root_path": dir.path().to_str().unwrap(),
            "threads": 1,
            "min_size": 0,
            "exclude_patterns": [],
            "stale_days": 30,
            "limit": 10
        });

        let resp = handle_scan_start(params, task_store.clone()).await.unwrap();
        assert!(resp.error.is_none());
        assert!(resp.result.is_some());

        let result_value = resp.result.unwrap();
        let task_id = result_value
            .get("task_id")
            .and_then(Value::as_str)
            .expect("task_id should be a string");
        assert!(!task_id.is_empty());

        let store = task_store.read().await;
        assert!(store.contains_key(task_id));
    }

    #[tokio::test]
    async fn test_handle_scan_status_not_found() {
        let task_store = Arc::new(RwLock::new(HashMap::new()));
        let params = serde_json::json!({"task_id": "invalid-task-id"});
        let resp = handle_scan_status(params, task_store.clone()).await.unwrap();
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, -32602);
    }
}
