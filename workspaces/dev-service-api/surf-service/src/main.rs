use clap::Parser;
use tokio::net::TcpListener;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// Surf 服务进程：提供基于 JSON-RPC 的磁盘扫描服务（骨架实现）。
///
/// 当前版本仅完成：
/// - 命令行参数解析（host/port 等），与 PRD / Architecture 中的约定对齐；
/// - 启动一个 TCP 监听并接受连接；
/// - 对每一行 JSON-RPC 请求做基础校验，并对未实现/未知方法返回标准错误响应；
/// - 通过日志输出提示该服务仍处于骨架阶段。
///
/// 参数结构体 `Args` 的 Clap 属性定义见文件靠后的 `struct Args`。
/// JSON-RPC 2.0 标准错误码（部分）
const INVALID_REQUEST: i32 = -32600;
const METHOD_NOT_FOUND: i32 = -32601;
const INVALID_PARAMS: i32 = -32602;

/// 支持的 JSON-RPC 方法名称（来自 Architecture.md 4.3.*）
const SUPPORTED_METHODS: [&str; 4] = [
    "Surf.Scan",
    "Surf.Status",
    "Surf.GetResults",
    "Surf.Cancel",
];

/// JSON-RPC 2.0 请求结构（宽松解析，允许 params 为任意 JSON 值）
#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    /// 必须为 "2.0"
    jsonrpc: String,
    /// 方法名
    method: String,
    /// 参数（可选，可以是任意 JSON 值）
    #[serde(default)]
    params: Value,
    /// 请求 ID（string, number, null）
    #[serde(default)]
    id: Option<Value>,
}
/// 用于 `Surf.Scan` 方法的参数结构体（基于 Architecture.md 4.3.3）
#[derive(Debug, Deserialize)]
struct SurfScanParams {
    /// 起始扫描根目录（必填）
    path: String,
    /// 最小文件大小阈值，字符串形式如 "100MB"（可选）
    #[serde(default)]
    min_size: Option<String>,
    /// 并发扫描线程数（可选）
    #[serde(default)]
    threads: Option<usize>,
    /// 结果 TopN 限制（可选）
    #[serde(default)]
    limit: Option<usize>,
    /// 路径排除规则（可选）
    #[serde(default)]
    exclude_patterns: Option<Vec<String>>,
    /// 客户端打标（可选）
    #[serde(default)]
    tag: Option<String>,
}

/// JSON-RPC 错误对象（对应 error 字段）
#[derive(Debug, Serialize)]
struct JsonRpcError {
    /// 错误码
    code: i32,
    /// 错误消息
    message: String,
    /// 可选错误详情
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

/// JSON-RPC 错误响应（完整响应体）
#[derive(Debug, Serialize)]
struct JsonRpcErrorResponse {
    /// 必须为 "2.0"
    jsonrpc: String,
    /// 错误对象
    error: JsonRpcError,
    /// 请求 ID（若无法解析则为 null）
    id: Value,
}

impl JsonRpcError {
    /// 构造一个标准 INVALID_REQUEST 错误
    fn invalid_request(detail: Option<String>) -> Self {
        JsonRpcError {
            code: INVALID_REQUEST,
            message: "INVALID_REQUEST".to_string(),
            data: detail.map(|d| json!({ "detail": d })),
        }
    }

    /// 构造一个标准 METHOD_NOT_FOUND 错误
    fn method_not_found(detail: Option<String>) -> Self {
        JsonRpcError {
            code: METHOD_NOT_FOUND,
            message: "METHOD_NOT_FOUND".to_string(),
            data: detail.map(|d| json!({ "detail": d })),
        }
    }

    /// 构造一个标准 INVALID_PARAMS 错误
    fn invalid_params(detail: Option<String>) -> Self {
        JsonRpcError {
            code: INVALID_PARAMS,
            message: "INVALID_PARAMS".to_string(),
            data: detail.map(|d| json!({ "detail": d })),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_json() {
        // 无效的 JSON 应该返回 INVALID_REQUEST 错误
        let response = handle_rpc_line("{ invalid json }").unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["error"]["code"], INVALID_REQUEST);
        assert_eq!(parsed["error"]["message"], "INVALID_REQUEST");
        assert_eq!(parsed["id"], serde_json::Value::Null);
    }

    #[test]
    fn test_missing_jsonrpc_field() {
        // 缺少 jsonrpc 字段应该返回 INVALID_REQUEST
        let request = r#"{"method": "Surf.Scan", "id": 1}"#;
        let response = handle_rpc_line(request).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert_eq!(parsed["error"]["code"], INVALID_REQUEST);
    }

    #[test]
    fn test_wrong_jsonrpc_version() {
        // jsonrpc 不是 "2.0" 应该返回 INVALID_REQUEST
        let request = r#"{"jsonrpc": "1.0", "method": "Surf.Scan", "id": 1}"#;
        let response = handle_rpc_line(request).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert_eq!(parsed["error"]["code"], INVALID_REQUEST);
    }

    #[test]
    fn test_unknown_method() {
        // 未知方法应该返回 METHOD_NOT_FOUND
        let request = r#"{"jsonrpc": "2.0", "method": "Unknown.Method", "id": 1}"#;
        let response = handle_rpc_line(request).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert_eq!(parsed["error"]["code"], METHOD_NOT_FOUND);
        assert_eq!(parsed["error"]["message"], "METHOD_NOT_FOUND");
        // 检查 data.detail 是否包含方法名
        let detail = parsed["error"]["data"]["detail"].as_str().unwrap();
        assert!(detail.contains("Unknown.Method"));
    }

    #[test]
    fn test_supported_method_not_implemented() {
        // 支持的方法但未实现应该返回 METHOD_NOT_FOUND，并提示 "method not implemented yet"
        let request = r#"{"jsonrpc": "2.0", "method": "Surf.Scan", "id": 1}"#;
        let response = handle_rpc_line(request).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert_eq!(parsed["error"]["code"], METHOD_NOT_FOUND);
        let detail = parsed["error"]["data"]["detail"].as_str().unwrap();
        assert_eq!(detail, "method not implemented yet");
    }

    #[test]
    fn test_invalid_params_for_supported_method() {
        // 构造一个支持的方法（Surf.Scan），但 params 是数组（应为对象） -> INVALID_PARAMS
        let request = r#"{"jsonrpc": "2.0", "method": "Surf.Scan", "params": [], "id": 1}"#;
        let response = handle_rpc_line(request).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["error"]["code"], INVALID_PARAMS);
        assert_eq!(parsed["error"]["message"], "INVALID_PARAMS");
        // 检查 data.detail 是否包含对 params 类型的说明
        let detail = parsed["error"]["data"]["detail"].as_str().unwrap();
        assert!(detail.contains("params must be a JSON object for method Surf.Scan"));
        // 检查 id 是否保留原值
        assert_eq!(parsed["id"], 1);
    }

    #[test]
    fn test_empty_line_skipped() {
        // 空行应该返回 None
        let response = handle_rpc_line("");
        assert!(response.is_none());
        let response = handle_rpc_line("   ");
        assert!(response.is_none());
    }
}

impl JsonRpcErrorResponse {
    /// 根据错误和请求 ID 构造错误响应
    fn from_error(error: JsonRpcError, id: Option<Value>) -> Self {
        JsonRpcErrorResponse {
            jsonrpc: "2.0".to_string(),
            error,
            id: id.unwrap_or(Value::Null),
        }
    }
}

/// 解析一行 JSON-RPC 请求并生成相应的错误响应（如果请求无效或方法未实现）
fn handle_rpc_line(line: &str) -> Option<String> {
    // 空行或仅空白则跳过
    if line.trim().is_empty() {
        return None;
    }

    // 尝试解析为 JSON
    let json_value: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => {
            // JSON 解析失败 -> INVALID_REQUEST
            let error = JsonRpcError::invalid_request(Some(format!("JSON parse error: {}", e)));
            let response = JsonRpcErrorResponse::from_error(error, None);
            return Some(serde_json::to_string(&response).unwrap_or_else(|_| String::new()));
        }
    };

    // 尝试反序列化为 JsonRpcRequest
    let req: JsonRpcRequest = match serde_json::from_value(json_value.clone()) {
        Ok(r) => r,
        Err(e) => {
            // 结构不符合要求 -> INVALID_REQUEST
            let error = JsonRpcError::invalid_request(Some(format!("Invalid request structure: {}", e)));
            let response = JsonRpcErrorResponse::from_error(error, None);
            return Some(serde_json::to_string(&response).unwrap_or_else(|_| String::new()));
        }
    };

    // 检查 jsonrpc 字段
    if req.jsonrpc != "2.0" {
        let error = JsonRpcError::invalid_request(Some(format!("jsonrpc must be \"2.0\", got \"{}\"", req.jsonrpc)));
        let response = JsonRpcErrorResponse::from_error(error, req.id);
        return Some(serde_json::to_string(&response).unwrap_or_else(|_| String::new()));
    }

    // 检查 method 是否为支持的四个方法之一
    let is_supported = SUPPORTED_METHODS.iter().any(|&m| m == req.method);
    
    let error = if is_supported {
        // 对于支持的方法，检查参数形状是否符合预期（本项目仅使用 named params，即 JSON 对象）
        if !req.params.is_object() {
            // params 不是对象（null、数组、字符串、数字等） -> INVALID_PARAMS
            let detail = format!("params must be a JSON object for method {}", req.method);
            JsonRpcError::invalid_params(Some(detail))
        } else {
            // 参数形状正确，但方法尚未实现 -> METHOD_NOT_FOUND
            JsonRpcError::method_not_found(Some("method not implemented yet".to_string()))
        }
    } else {
        // 不支持的方法 -> METHOD_NOT_FOUND
        JsonRpcError::method_not_found(Some(format!("method \"{}\" not found", req.method)))
    };
    
    let response = JsonRpcErrorResponse::from_error(error, req.id);
    Some(serde_json::to_string(&response).unwrap_or_else(|_| String::new()))
}

#[derive(Parser, Debug)]
#[command(name = "surf-service", version, about = "Surf JSON-RPC service (skeleton)")]
struct Args {
    /// 服务监听地址，默认仅监听本地回环地址 127.0.0.1
    ///
    /// 对应 Architecture.md 4.3.1 中的安全默认值约定。
    #[arg(long = "host", default_value = "127.0.0.1")]
    host: String,

    /// 服务监听端口，默认 1234。
    ///
    /// 与 PRD 4. 命令行参数定义中的 `--port` 对齐。
    #[arg(long = "port", default_value_t = 1234)]
    port: u16,

    /// 最大并发扫描任务数（占位参数）。
    ///
    /// 当前仅解析并记录，实际并发控制将在后续实现任务管理器时生效。
    #[arg(long = "max-concurrent-scans", default_value_t = 4)]
    max_concurrent_scans: usize,

    /// 单个任务在内存中保留的 TTL 秒数（占位参数）。
    ///
    /// 与 Architecture.md 4.3.1 中的 task_ttl_seconds 设计一致，目前尚未真正用于回收逻辑。
    #[arg(long = "task-ttl-seconds", default_value_t = 600)]
    task_ttl_seconds: u64,
}

/// 处理单个 TCP 连接，读取行分隔的 JSON-RPC 请求并返回错误响应
async fn handle_connection(socket: tokio::net::TcpStream, peer: std::net::SocketAddr) -> anyhow::Result<()> {
    let (read_half, mut write_half) = socket.into_split();
    let reader = BufReader::new(read_half);
    let mut lines = reader.lines();

    while let Some(line) = lines.next_line().await? {
        // 使用 handle_rpc_line 处理每一行
        if let Some(response) = handle_rpc_line(&line) {
            // 记录请求摘要（方法名或错误类型）
            eprintln!("[{}] request line: {} -> response: {}", peer, line.trim(), response);
            write_half.write_all(response.as_bytes()).await?;
            write_half.write_all(b"\n").await?;
            write_half.flush().await?;
        } else {
            // 空行，跳过
            eprintln!("[{}] empty line skipped", peer);
        }
    }

    // 客户端关闭连接
    eprintln!("[{}] connection closed by client", peer);
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let addr = format!("{}:{}", args.host, args.port);

    // 绑定 TCP 监听，后续将基于此实现 JSON-RPC 2.0 协议。
    let listener = TcpListener::bind(&addr).await?;

    println!(
        "surf-service listening on {addr} (max_concurrent_scans={max}, task_ttl_seconds={ttl}).\nJSON-RPC methods (Surf.Scan / Surf.Status / Surf.GetResults / Surf.Cancel) are not implemented yet; this binary currently serves as a service skeleton.",
        addr = addr,
        max = args.max_concurrent_scans,
        ttl = args.task_ttl_seconds,
    );

    loop {
        let (socket, peer) = listener.accept().await?;
        eprintln!("Accepted connection from {}", peer);

        // 为每个连接启动独立任务
        tokio::spawn(async move {
            let _ = handle_connection(socket, peer).await;
        });
    }
}
