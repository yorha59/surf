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

/// 解析大小字符串（与 CLI 中的 parse_size 语义保持一致）
///
/// 支持的单位：B/KB/MB/GB（不区分大小写），空字符串或纯空白视为 0。
/// 返回解析后的字节数，若解析失败则返回包含错误信息的字符串。
fn parse_size_for_service(input: &str) -> Result<u64, String> {
    let s = input.trim();
    if s.is_empty() {
        return Ok(0);
    }

    let split_at = s
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or_else(|| s.len());

    let (num_part, unit_part) = s.split_at(split_at);
    let value: u64 = num_part
        .parse()
        .map_err(|_| format!("invalid size number: {}", num_part))?;

    let unit = unit_part.trim().to_ascii_uppercase();
    let multiplier: u64 = match unit.as_str() {
        "" | "B" => 1,
        "K" | "KB" => 1024,
        "M" | "MB" => 1024 * 1024,
        "G" | "GB" => 1024 * 1024 * 1024,
        other => return Err(format!("unsupported size unit: {}", other)),
    };

    Ok(value.saturating_mul(multiplier))
}

/// 校验 Surf.Scan 参数的数值合法性（本轮仅校验 min_size 和 threads）
///
/// 若校验通过返回 `Ok(())`，否则返回 `INVALID_PARAMS` 错误。
/// 注意：此校验不涉及业务逻辑（如路径是否存在、任务管理器是否就绪等）。
fn validate_surf_scan_params(params: &SurfScanParams) -> Result<(), JsonRpcError> {
    // 校验 min_size（如果存在）
    if let Some(ref min_size_str) = params.min_size {
        match parse_size_for_service(min_size_str) {
            Ok(_) => {} // 解析成功，值合法
            Err(e) => {
                let detail = format!("invalid min_size: {}", e);
                return Err(JsonRpcError::invalid_params(Some(detail)));
            }
        }
    }
    
    // 校验 threads（如果存在）
    if let Some(threads) = params.threads {
        if threads == 0 {
            let detail = "invalid threads: must be >= 1".to_string();
            return Err(JsonRpcError::invalid_params(Some(detail)));
        }
    }
    
    Ok(())
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
    fn test_surf_scan_params_object_but_invalid_shape() {
        // params 是对象但缺少必填字段 path -> INVALID_PARAMS
        let request = r#"{"jsonrpc": "2.0", "method": "Surf.Scan", "params": {"threads": 4}, "id": 1}"#;
        let response = handle_rpc_line(request).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["error"]["code"], INVALID_PARAMS);
        assert_eq!(parsed["error"]["message"], "INVALID_PARAMS");
        let detail = parsed["error"]["data"]["detail"].as_str().unwrap();
        assert!(detail.contains("invalid Surf.Scan params"));
        assert_eq!(parsed["id"], 1);
    }

    #[test]
    fn test_surf_scan_invalid_min_size_unit() {
        // min_size 单位非法 -> INVALID_PARAMS
        let request = r#"{"jsonrpc":"2.0","method":"Surf.Scan","params":{"path":"/tmp","min_size":"10XB"},"id":1}"#;
        let response = handle_rpc_line(request).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["error"]["code"], INVALID_PARAMS);
        assert_eq!(parsed["error"]["message"], "INVALID_PARAMS");
        let detail = parsed["error"]["data"]["detail"].as_str().unwrap();
        assert!(detail.contains("min_size"));
        assert!(detail.contains("unsupported"));
        assert_eq!(parsed["id"], 1);
    }

    #[test]
    fn test_surf_scan_invalid_threads_zero() {
        // threads 为 0 -> INVALID_PARAMS
        let request = r#"{"jsonrpc":"2.0","method":"Surf.Scan","params":{"path":"/tmp","threads":0},"id":2}"#;
        let response = handle_rpc_line(request).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["error"]["code"], INVALID_PARAMS);
        assert_eq!(parsed["error"]["message"], "INVALID_PARAMS");
        let detail = parsed["error"]["data"]["detail"].as_str().unwrap();
        assert!(detail.contains("threads"));
        assert!(detail.contains(">= 1"));
        assert_eq!(parsed["id"], 2);
    }

    #[test]
    fn test_surf_scan_valid_params_still_not_implemented() {
        // params 结构完整且类型正确时，应视为参数合法，但方法仍未实现 -> METHOD_NOT_FOUND
        let request = r#"{
            "jsonrpc": "2.0",
            "method": "Surf.Scan",
            "params": {
                "path": "/tmp",
                "min_size": "10MB",
                "threads": 4,
                "limit": 10,
                "exclude_patterns": ["**/node_modules/**"],
                "tag": "test"
            },
            "id": 42
        }"#;

        let response = handle_rpc_line(request).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["error"]["code"], METHOD_NOT_FOUND);
        assert_eq!(parsed["error"]["message"], "METHOD_NOT_FOUND");
        let detail = parsed["error"]["data"]["detail"].as_str().unwrap();
        assert_eq!(detail, "method not implemented yet");
        // id 应保留调用方提供的值
        assert_eq!(parsed["id"], 42);
    }

    #[test]
    fn test_empty_line_skipped() {
        // 空行应该返回 None
        let response = handle_rpc_line("");
        assert!(response.is_none());
        let response = handle_rpc_line("   ");
        assert!(response.is_none());
    }

    #[test]
    fn test_parse_size_for_service_basic_units() {
        // 空字符串或仅空白视为 0
        assert_eq!(parse_size_for_service("").unwrap(), 0);
        assert_eq!(parse_size_for_service("   ").unwrap(), 0);

        // 不同大小写的合法单位
        assert_eq!(parse_size_for_service("1").unwrap(), 1);
        assert_eq!(parse_size_for_service("1b").unwrap(), 1);
        assert_eq!(parse_size_for_service("1B").unwrap(), 1);

        assert_eq!(parse_size_for_service("1k").unwrap(), 1024);
        assert_eq!(parse_size_for_service("1KB").unwrap(), 1024);

        assert_eq!(parse_size_for_service("2m").unwrap(), 2 * 1024 * 1024);
        assert_eq!(parse_size_for_service("2MB").unwrap(), 2 * 1024 * 1024);

        assert_eq!(parse_size_for_service("3g").unwrap(), 3 * 1024 * 1024 * 1024);
        assert_eq!(parse_size_for_service("3GB").unwrap(), 3 * 1024 * 1024 * 1024);
    }

    #[test]
    fn test_parse_size_for_service_invalid_inputs() {
        // 非数字前缀
        let err = parse_size_for_service("abcMB").unwrap_err();
        assert!(err.contains("invalid size number"));

        // 不支持的单位
        let err = parse_size_for_service("10XB").unwrap_err();
        assert!(err.contains("unsupported size unit"));
    }

    #[test]
    fn test_validate_surf_scan_params_ok_and_invalid() {
        // 合法的 min_size / threads 组合
        let ok_params = SurfScanParams {
            path: "/tmp".to_string(),
            min_size: Some("10MB".to_string()),
            threads: Some(4),
            limit: Some(10),
            exclude_patterns: None,
            tag: None,
        };
        assert!(validate_surf_scan_params(&ok_params).is_ok());

        // 非法的 min_size 单位
        let bad_min_size = SurfScanParams {
            path: "/tmp".to_string(),
            min_size: Some("10XB".to_string()),
            threads: Some(4),
            limit: None,
            exclude_patterns: None,
            tag: None,
        };
        let err = validate_surf_scan_params(&bad_min_size).unwrap_err();
        assert_eq!(err.code, INVALID_PARAMS);
        assert_eq!(err.message, "INVALID_PARAMS");
        let detail = err.data.unwrap()["detail"].as_str().unwrap();
        assert!(detail.contains("invalid min_size"));

        // 非法的 threads 值（0）
        let bad_threads = SurfScanParams {
            path: "/tmp".to_string(),
            min_size: None,
            threads: Some(0),
            limit: None,
            exclude_patterns: None,
            tag: None,
        };
        let err = validate_surf_scan_params(&bad_threads).unwrap_err();
        assert_eq!(err.code, INVALID_PARAMS);
        assert_eq!(err.message, "INVALID_PARAMS");
        let detail = err.data.unwrap()["detail"].as_str().unwrap();
        assert!(detail.contains("threads"));
        assert!(detail.contains(">= 1"));
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
    let method = req.method.as_str();
    let is_supported = SUPPORTED_METHODS.iter().any(|&m| m == method);

    let error = if !is_supported {
        // 不支持的方法 -> METHOD_NOT_FOUND
        JsonRpcError::method_not_found(Some(format!("method \"{}\" not found", method)))
    } else {
        match method {
            // Surf.Scan 对参数形状有更严格的约束：
            // - 缺少 params（即为 null）时，优先视为“方法尚未实现”的骨架占位；
            // - params 存在但不是对象 -> INVALID_PARAMS；
            // - params 为对象时，尝试反序列化为 SurfScanParams，失败则 INVALID_PARAMS，成功则暂时仍返回
            //   METHOD_NOT_FOUND（扫描任务管理尚未实现）。
            "Surf.Scan" => {
                if req.params.is_null() {
                    // 缺少参数但方法本身受支持：当前仅作为“尚未实现”的占位
                    JsonRpcError::method_not_found(Some("method not implemented yet".to_string()))
                } else if !req.params.is_object() {
                    // params 不是对象（数组/字符串/数字等） -> INVALID_PARAMS
                    let detail = format!("params must be a JSON object for method {}", method);
                    JsonRpcError::invalid_params(Some(detail))
                } else {
                    // params 为对象，尝试解析为 SurfScanParams；解析失败视为 INVALID_PARAMS
                    match serde_json::from_value::<SurfScanParams>(req.params.clone()) {
                        Ok(scan_params) => {
                            // 参数结构正确，进一步校验数值合法性
                            match validate_surf_scan_params(&scan_params) {
                                Ok(()) => {
                                    // 数值也合法，但业务逻辑尚未落地 -> METHOD_NOT_FOUND 占位
                                    JsonRpcError::method_not_found(Some("method not implemented yet".to_string()))
                                }
                                Err(err) => {
                                    // 数值校验失败 -> INVALID_PARAMS
                                    err
                                }
                            }
                        }
                        Err(e) => {
                            let detail = format!("invalid Surf.Scan params: {}", e);
                            JsonRpcError::invalid_params(Some(detail))
                        }
                    }
                }
            }
            // 其他支持的方法当前仍仅作为骨架存在：无论是否携带 params，一律返回 METHOD_NOT_FOUND
            _ => {
                JsonRpcError::method_not_found(Some("method not implemented yet".to_string()))
            }
        }
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
