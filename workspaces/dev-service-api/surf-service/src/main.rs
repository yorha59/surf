use clap::Parser;
use tokio::net::TcpListener;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::time::{SystemTime, UNIX_EPOCH};
use once_cell::sync::Lazy;
use num_cpus;
use std::path::PathBuf;
use surf_core;

/// Surf 服务进程：提供基于 JSON-RPC 的磁盘扫描服务。
///
/// 当前版本已经具备：
/// - 命令行参数解析（host/port/max-concurrent-scans/task-ttl-seconds 等），与 PRD / Architecture 中的约定对齐；
/// - 启动 TCP 监听并按连接拆分异步任务，按行读取 JSON-RPC 请求；
/// - 对所有 JSON-RPC 请求做结构与版本校验，统一使用 JSON-RPC 2.0 错误模型；
/// - 针对 `Surf.Scan`：解析/校验参数（含 `min_size` 单位与 `threads` 下限），构造 `surf_core::ScanConfig` 并调用 `surf_core::start_scan` 启动实际扫描任务，将返回的 `ScanHandle` 保存到任务表中；
/// - 针对 `Surf.Status`：支持查询单个任务或列出所有处于非终止态（queued/running）的任务，结合 `surf_core::poll_status` 返回实时进度信息，并在底层扫描结束后惰性推进任务状态（running → completed/failed）；
/// - 针对 `Surf.Cancel`：校验 `task_id` 并通过 `TASK_MANAGER.cancel_task` 触发任务状态迁移及 `surf_core::cancel` 调用，实现幂等取消；
/// - 针对 `Surf.GetResults`：实现了参数与任务状态校验，仅在任务处于 `completed` 状态时返回占位性的聚合结果结构（total_files/total_bytes 为 0，entries 为空数组），真实结果聚合与缓存仍在后续迭代中补充。
///
/// 参数结构体 `Args` 的 Clap 属性定义见文件靠后的 `struct Args`。
/// JSON-RPC 2.0 标准错误码（部分）
const INVALID_REQUEST: i32 = -32600;
const METHOD_NOT_FOUND: i32 = -32601;
const INVALID_PARAMS: i32 = -32602;
const TASK_NOT_FOUND: i32 = -32001;

/// 支持的 JSON-RPC 方法名称（来自 Architecture.md 4.3.*）
const SUPPORTED_METHODS: [&str; 4] = [
    "Surf.Scan",
    "Surf.Status",
    "Surf.GetResults",
    "Surf.Cancel",
];

/// 扫描任务状态枚举，对齐 Architecture.md 4.3.1 中的任务状态机。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TaskState {
    Queued,
    Running,
    Completed,
    Failed,
    Canceled,
}

/// 单个扫描任务的元数据信息快照。
///
/// 当前仅在内存中维护这些字段，用于后续实现 `Surf.Status` / `Surf.GetResults`
/// 时作为基础信息来源；尚未与 `surf-core` 的进度快照或结果结构体打通。
#[derive(Debug, Clone)]
struct TaskInfo {
    /// 起始扫描路径（与 Surf.Scan.params.path 一致）。
    path: String,
    /// 解析后的最小文件大小阈值（字节）。
    min_size_bytes: u64,
    /// 扫描线程数。
    threads: usize,
    /// TopN 限制（可选，对应 Surf.Scan.params.limit）。
    limit: Option<usize>,
    /// 客户端打标（可选）。
    tag: Option<String>,
    /// 任务创建时间（Unix 秒）。
    started_at: u64,
    /// 最近一次状态更新的时间（Unix 秒）。
    updated_at: u64,
    /// 当前任务状态（queued/running/completed/failed/canceled）。
    state: TaskState,
    /// 底层扫描任务的句柄（如果有）。直接持有 `ScanHandle`，
    /// 具体的并发共享由其内部的 `Arc` 负责。
    scan_handle: Option<surf_core::ScanHandle>,
}

/// 内存中的简单任务管理器骨架。
///
/// - 当前负责分配任务 ID 并记录 `TaskInfo` 元数据；
/// - 后续迭代会在此基础上补充对 `surf-core::ScanHandle` 的持有与进度快照，
///   以便真正实现 Architecture.md 4.3.7 所描述的任务生命周期与 `Surf.Status` 映射。
#[derive(Debug, Default)]
struct TaskManager {
    inner: Mutex<HashMap<String, TaskInfo>>,
}

/// 全局递增任务 ID 计数器，用于生成简单的字符串 task_id（"1"、"2"...）。
static TASK_ID_SEQ: AtomicU64 = AtomicU64::new(1);

impl TaskManager {
    /// 创建一个空的任务管理器实例。
    fn new() -> Self {
        TaskManager {
            inner: Mutex::new(HashMap::new()),
        }
    }

    /// 分配一个新的任务 ID。
    ///
    /// 当前实现采用进程内递增数字字符串，后续可以无损演进为 UUID 等形式。
    fn next_task_id(&self) -> String {
        let id = TASK_ID_SEQ.fetch_add(1, AtomicOrdering::Relaxed);
        id.to_string()
    }

    /// 注册一个新的任务元数据，包含扫描句柄，并返回分配的 task_id。
    fn register_task_with_handle(
        &self,
        path: String,
        min_size_bytes: u64,
        threads: usize,
        limit: Option<usize>,
        tag: Option<String>,
        state: TaskState,
        scan_handle: Option<surf_core::ScanHandle>,
    ) -> String {
        let task_id = self.next_task_id();
        let now = current_unix_timestamp();
        let info = TaskInfo {
            path,
            min_size_bytes,
            threads,
            limit,
            tag,
            started_at: now,
            updated_at: now,
            state,
            scan_handle,
        };

        let mut inner = self.inner.lock().expect("TaskManager mutex poisoned");
        inner.insert(task_id.clone(), info);
        task_id
    }

    /// 注册一个新的任务元数据，并返回分配的 task_id。
    ///
    /// 该方法目前只记录基础字段和初始状态，不涉及 `surf-core` 的扫描句柄；
    /// 在后续实现 `Surf.Scan` 业务逻辑时，可在调用处先启动实际扫描任务，
    /// 再将相关配置与状态信息一起写入 `TaskInfo`。
    fn register_task(
        &self,
        path: String,
        min_size_bytes: u64,
        threads: usize,
        limit: Option<usize>,
        tag: Option<String>,
        state: TaskState,
    ) -> String {
        self.register_task_with_handle(path, min_size_bytes, threads, limit, tag, state, None)
    }

    /// 读取某个任务的元数据快照。
    ///
    /// 返回值为克隆的 `TaskInfo`，避免在调用方持有互斥锁。
    fn get_task_info(&self, task_id: &str) -> Option<TaskInfo> {
        let inner = self.inner.lock().ok()?;
        inner.get(task_id).cloned()
    }

    /// 更新任务状态并返回更新前后的状态及任务信息快照。
    ///
    /// 若任务存在，则更新其 `state` 字段为 `new_state`，同时将 `updated_at`
    /// 设置为当前 Unix 时间戳（秒），并返回 `(previous_state, updated_info)` 元组。
    /// 若任务不存在，返回 `None`。
    fn update_task_state(&self, task_id: &str, new_state: TaskState) -> Option<(TaskState, TaskInfo)> {
        let mut inner = self.inner.lock().ok()?;
        let info = inner.get_mut(task_id)?;
        let previous_state = info.state;
        info.state = new_state;
       info.updated_at = current_unix_timestamp();
       // 返回克隆，避免持有锁
       Some((previous_state, info.clone()))
   }

    /// 取消指定任务，遵循 Architecture.md 4.3.6 的幂等性约定。
    ///
    /// - 若任务不存在，返回 `None`；
    /// - 若任务存在：
    ///   - 若当前状态为 `Queued` 或 `Running`，则将其状态更新为 `Canceled`；
    ///   - 若当前状态已是终止态（`Completed` / `Failed` / `Canceled`），则状态保持不变；
    ///   - 无论状态是否改变，`updated_at` 都会更新为当前时间戳（表示最近一次操作时间）；
    ///   - 返回 `(previous_state, updated_info)` 元组。
    fn cancel_task(&self, task_id: &str) -> Option<(TaskState, TaskInfo)> {
        let mut inner = self.inner.lock().ok()?;
        let info = inner.get_mut(task_id)?;
        let previous_state = info.state;
        let new_state = match previous_state {
            TaskState::Queued | TaskState::Running => TaskState::Canceled,
            TaskState::Completed | TaskState::Failed | TaskState::Canceled => previous_state,
        };
        // 如果状态从 Queued/Running 迁移到 Canceled，尝试取消底层扫描
        if new_state == TaskState::Canceled && previous_state != TaskState::Canceled {
            if let Some(handle) = &info.scan_handle {
                surf_core::cancel(handle);
            }
        }
        info.state = new_state;
        info.updated_at = current_unix_timestamp();
        // 返回克隆，避免持有锁
        Some((previous_state, info.clone()))
    }

    /// 列出所有处于非终止态的任务（queued/running）。
    ///
    /// 该方法用于 Surf.Status 在 task_id 为空或缺省时返回任务列表，
    /// 对 Completed / Failed / Canceled 等终止态任务不再返回状态。
    fn list_non_terminated_tasks(&self) -> Vec<(String, TaskInfo)> {
        let inner = match self.inner.lock() {
            Ok(guard) => guard,
            Err(_) => {
                // 若互斥锁已中毒，则返回空列表，避免因单个任务异常影响整个接口。
                return Vec::new();
            }
        };

        inner
            .iter()
            .filter(|(_, info)| match info.state {
                TaskState::Queued | TaskState::Running => true,
                TaskState::Completed | TaskState::Failed | TaskState::Canceled => false,
            })
            .map(|(id, info)| (id.clone(), info.clone()))
            .collect()
    }
}

/// 全局任务管理器实例
static TASK_MANAGER: Lazy<TaskManager> = Lazy::new(|| TaskManager::new());

/// 获取当前 Unix 时间戳（秒）。
fn current_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| std::time::Duration::from_secs(0))
        .as_secs()
}

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

/// Surf.Scan 方法的成功响应结果结构体
#[derive(Debug, Serialize)]
struct SurfScanResult {
    /// 任务 ID（字符串）
    task_id: String,
    /// 任务状态（"queued"）
    state: String,
    /// 扫描路径（原始 params.path）
    path: String,
    /// 解析后的最小文件大小阈值（字节）
    min_size_bytes: u64,
    /// 实际使用的扫描线程数
    threads: usize,
    /// TopN 限制（可选，若为 None 则序列化为 null）
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<usize>,
}

/// Surf.Status 方法的成功响应结果结构体
///
/// 当前仅针对已有的单个任务查询返回最小可用信息：
/// - 任务 ID
/// - 状态（queued/running/completed/failed/canceled）
/// - 进度相关字段使用占位值（queued 任务视为 0）
///
/// 后续在接入 `surf-core` 的进度快照后，可在不破坏字段语义的前提下
/// 补充真实的 `scanned_files` / `scanned_bytes` 等数据。
#[derive(Debug, Serialize)]
struct SurfStatusResult {
    task_id: String,
    state: String,
    /// 估算进度（0.0 ~ 1.0），当前 queued 任务统一视为 0.0。
    progress: f64,
    scanned_files: u64,
    scanned_bytes: u64,
    /// 当前仍无法估算总字节数，使用 null 表示。
    total_bytes_estimate: Option<u64>,
    started_at: u64,
    updated_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    tag: Option<String>,
}

impl SurfStatusResult {
    /// 从 TaskInfo 构造状态结果快照。
    ///
    /// 当前尚未集成 surf-core 的进度快照，因此进度字段统一使用占位值。
    fn from_task_info(task_id: &str, info: TaskInfo) -> Self {
        // 默认使用当前 TaskState 作为返回状态；对于 Running 任务，在
        // 关联了扫描句柄且底层扫描已结束时，根据核心快照将其惰性迁移为
        // Completed/Failed，并同步回 TASK_MANAGER（与 Architecture.md 4.3.7
        // 中“结合 StatusSnapshot.done / error 更新任务状态机”的约定对齐）。

        let mut effective_state = info.state;

        // 如果有扫描句柄，查询真实进度
        let (progress, scanned_files, scanned_bytes, total_bytes_estimate) = match info.scan_handle {
            Some(handle) => {
                let snapshot = surf_core::poll_status(&handle);

                // 根据快照惰性推进任务状态：仅当任务当前处于 Running
                // 且底层扫描已结束时，才根据 error 与否迁移到
                // Completed/Failed；Queued/Completed/Failed/Canceled 保持不变。
                if snapshot.done {
                    match info.state {
                        TaskState::Running => {
                            effective_state = if snapshot.error.is_some() {
                                TaskState::Failed
                            } else {
                                TaskState::Completed
                            };

                            // 尝试同步更新全局任务表中的状态；若互斥锁异常
                            // 或任务在此期间被删除，则忽略更新失败。
                            let _ = TASK_MANAGER.update_task_state(task_id, effective_state);
                        }
                        _ => { /* 其他状态不在此处修改 */ }
                    }
                }

                let scanned_files = snapshot.progress.scanned_files;
                let scanned_bytes = snapshot.progress.scanned_bytes;
                let total_bytes_estimate = snapshot.progress.total_bytes_estimate;
                // 计算进度百分比（0.0-1.0），如果总字节数未知则返回 0.0
                let progress = match total_bytes_estimate {
                    Some(total) if total > 0 => scanned_bytes as f64 / total as f64,
                    _ => 0.0,
                };
                (progress, scanned_files, scanned_bytes, total_bytes_estimate)
            }
            None => (0.0, 0, 0, None),
        };

        SurfStatusResult {
            task_id: task_id.to_string(),
            state: match effective_state {
                TaskState::Queued => "queued".to_string(),
                TaskState::Running => "running".to_string(),
                TaskState::Completed => "completed".to_string(),
                TaskState::Failed => "failed".to_string(),
                TaskState::Canceled => "canceled".to_string(),
            },
            progress,
            scanned_files,
            scanned_bytes,
            total_bytes_estimate,
            started_at: info.started_at,
            updated_at: info.updated_at,
            tag: info.tag,
        }
    }
}

/// Surf.Cancel 方法的成功响应结果结构体
#[derive(Debug, Serialize)]
struct SurfCancelResult {
    task_id: String,
    previous_state: String,
    current_state: String,
}

/// Surf.GetResults 方法的参数结构体
#[derive(Debug, Deserialize)]
struct SurfGetResultsParams {
    /// 任务 ID（必填）
    task_id: String,
    /// 结果模式，当前仅支持 "flat"（可选）
    #[serde(default)]
    mode: Option<String>,
    /// 返回条目数量限制（可选）
    #[serde(default)]
    limit: Option<usize>,
}

/// Surf.GetResults 方法的成功响应结果结构体
#[derive(Debug, Serialize)]
struct SurfGetResultsResult {
    task_id: String,
    state: String,
    path: String,
    /// 总文件数（占位，暂为 0）
    total_files: u64,
    /// 总字节数（占位，暂为 0）
    total_bytes: u64,
    /// 结果条目数组（占位，暂为空数组）
    entries: Vec<serde_json::Value>,
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

/// JSON-RPC 成功响应（完整响应体）
#[derive(Debug, Serialize)]
struct JsonRpcSuccessResponse<T> {
    /// 必须为 "2.0"
    jsonrpc: String,
    /// 结果对象
    result: T,
    /// 请求 ID（string, number, null）
    id: Value,
}

impl<T> JsonRpcSuccessResponse<T> {
    /// 根据结果和请求 ID 构造成功响应
    fn from_result(result: T, id: Option<Value>) -> Self {
        JsonRpcSuccessResponse {
            jsonrpc: "2.0".to_string(),
            result,
            id: id.unwrap_or(Value::Null),
        }
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

    /// 构造一个标准 TASK_NOT_FOUND 错误
    fn task_not_found(detail: Option<String>) -> Self {
        JsonRpcError {
            code: TASK_NOT_FOUND,
            message: "TASK_NOT_FOUND".to_string(),
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
    fn test_surf_scan_valid_params_returns_success_and_registers_task() {
        // params 结构完整且类型正确时，应创建一个排队中的任务并返回成功响应
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
        // 成功路径应返回 result，而不是 error
        assert!(parsed.get("error").is_none());

        let result = parsed["result"].as_object().expect("result should be an object");

        // task_id 应为非空字符串
        let task_id = result["task_id"].as_str().expect("task_id should be a string");
        assert!(!task_id.is_empty());

        // state / path 与请求参数一致
        assert_eq!(result["state"], "running");
        assert_eq!(result["path"], "/tmp");

        // min_size_bytes 按 10MB 解析
        let expected_min_size = parse_size_for_service("10MB").unwrap();
        assert_eq!(result["min_size_bytes"].as_u64().unwrap(), expected_min_size);

        // threads / limit 透传
        assert_eq!(result["threads"].as_u64().unwrap(), 4);
        assert_eq!(result["limit"].as_u64().unwrap(), 10);

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

    #[test]
    fn test_surf_status_params_not_object_invalid_params() {
        // 构造请求：params 是数组而不是对象
        let request = r#"{"jsonrpc":"2.0","method":"Surf.Status","params":[],"id":1}"#;
        let response = handle_rpc_line(request).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["error"]["code"], INVALID_PARAMS);
        assert_eq!(parsed["error"]["message"], "INVALID_PARAMS");
        let detail = parsed["error"]["data"]["detail"].as_str().unwrap();
        assert!(detail.contains("params must be a JSON object for method Surf.Status"));
        assert_eq!(parsed["id"], 1);
    }

    #[test]
    fn test_surf_status_missing_or_bad_task_id_invalid_params() {
        // 测试 task_id 是数字而不是 string/null
        let request = r#"{"jsonrpc":"2.0","method":"Surf.Status","params":{"task_id":42},"id":3}"#;
        let response = handle_rpc_line(request).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert_eq!(parsed["error"]["code"], INVALID_PARAMS);
        let detail = parsed["error"]["data"]["detail"].as_str().unwrap();
        assert!(detail.contains("task_id must be a string or null"));
        assert_eq!(parsed["id"], 3);

        // 测试 task_id 是空字符串（视为无效参数）
        let request = r#"{"jsonrpc":"2.0","method":"Surf.Status","params":{"task_id":""},"id":4}"#;
        let response = handle_rpc_line(request).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert_eq!(parsed["error"]["code"], INVALID_PARAMS);
        let detail = parsed["error"]["data"]["detail"].as_str().unwrap();
        assert!(detail.contains("task_id must be a non-empty string or null"));
        assert_eq!(parsed["id"], 4);
    }

    #[test]
    fn test_surf_status_task_not_found_for_unknown_id() {
        // 请求一个不存在的 task_id 应返回 TASK_NOT_FOUND
        let request = r#"{"jsonrpc":"2.0","method":"Surf.Status","params":{"task_id":"non-existent"},"id":3}"#;
        let response = handle_rpc_line(request).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["error"]["code"], TASK_NOT_FOUND);
        assert_eq!(parsed["error"]["message"], "TASK_NOT_FOUND");
        let detail = parsed["error"]["data"]["detail"].as_str().unwrap();
        assert!(detail.contains("task_id not found: non-existent"));
        assert_eq!(parsed["id"], 3);
    }

    #[test]
    fn test_surf_status_returns_success_for_existing_task() {
        // 先通过全局 TASK_MANAGER 注册一个任务
        let task_id = TASK_MANAGER.register_task(
            "/tmp".to_string(),
            0,
            4,
            Some(10),
            Some("status-test".to_string()),
            TaskState::Queued,
        );

        let request = format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"Surf.Status\",\"params\":{{\"task_id\":\"{}\"}},\"id\":7}}",
            task_id
        );

        let response = handle_rpc_line(&request).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();

        assert_eq!(parsed["jsonrpc"], "2.0");
        // 成功路径应返回 result，而不是 error
        assert!(parsed.get("error").is_none());

        let result = parsed["result"].as_object().expect("result should be an object");
        assert_eq!(result["task_id"], task_id);
        assert_eq!(result["state"], "queued");

        // 进度相关字段当前为占位值
        assert_eq!(result["progress"].as_f64().unwrap(), 0.0);
        assert_eq!(result["scanned_files"], 0);
        assert_eq!(result["scanned_bytes"], 0);
        assert!(result["total_bytes_estimate"].is_null());

        // 时间戳字段应为正数
        assert!(result["started_at"].as_u64().unwrap() > 0);
        assert!(result["updated_at"].as_u64().unwrap() >= result["started_at"].as_u64().unwrap());

        // tag 字段应与注册时一致
        assert_eq!(result["tag"].as_str().unwrap(), "status-test");

        assert_eq!(parsed["id"], 7);
    }

    #[test]
    fn test_surf_status_missing_task_id_lists_non_terminated_tasks() {
        // 创建三种不同状态的任务，其中 Completed 为终止态，应被过滤掉
        let _completed_id = TASK_MANAGER.register_task(
            "/tmp/status-missing-completed".to_string(),
            0,
            1,
            None,
            Some("status-missing-completed".to_string()),
            TaskState::Completed,
        );

        let queued_id = TASK_MANAGER.register_task(
            "/tmp/status-missing-queued".to_string(),
            0,
            1,
            None,
            Some("status-missing-queued".to_string()),
            TaskState::Queued,
        );

        let running_id = TASK_MANAGER.register_task(
            "/tmp/status-missing-running".to_string(),
            0,
            1,
            None,
            Some("status-missing-running".to_string()),
            TaskState::Running,
        );

        let request = r#"{"jsonrpc":"2.0","method":"Surf.Status","params":{},"id":10}"#;
        let response = handle_rpc_line(request).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();

        assert_eq!(parsed["jsonrpc"], "2.0");
        assert!(parsed.get("error").is_none(), "expected success result when task_id is missing");
        assert_eq!(parsed["id"], 10);

        let result = parsed["result"].as_array().expect("result should be an array");
        assert!(result.len() >= 2);

        let mut seen_queued = false;
        let mut seen_running = false;

        for entry in result {
            let state = entry["state"].as_str().unwrap_or("");
            // 列表中不应出现终止态 canceled；Completed 任务已在服务层过滤
            assert_ne!(state, "canceled");

            if let Some(tag) = entry.get("tag").and_then(|v| v.as_str()) {
                if tag == "status-missing-queued" {
                    seen_queued = true;
                    assert_eq!(state, "queued");
                    assert_eq!(entry["task_id"].as_str().unwrap(), queued_id);
                }
                if tag == "status-missing-running" {
                    seen_running = true;
                    assert_eq!(state, "running");
                    assert_eq!(entry["task_id"].as_str().unwrap(), running_id);
                }
                // Completed 任务的 tag 不应出现在结果中
                assert_ne!(tag, "status-missing-completed");
            }
        }

        assert!(seen_queued, "queued task should be present in status list");
        assert!(seen_running, "running task should be present in status list");
    }

    #[test]
    fn test_surf_status_null_task_id_lists_non_terminated_tasks() {
        // task_id 显式为 null 时，应与缺省 task_id 行为一致，返回所有非终止态任务
        let queued_id = TASK_MANAGER.register_task(
            "/tmp/status-null-queued".to_string(),
            0,
            1,
            None,
            Some("status-null-queued".to_string()),
            TaskState::Queued,
        );

        let request = r#"{"jsonrpc":"2.0","method":"Surf.Status","params":{"task_id":null},"id":11}"#;
        let response = handle_rpc_line(request).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();

        assert_eq!(parsed["jsonrpc"], "2.0");
        assert!(parsed.get("error").is_none());
        assert_eq!(parsed["id"], 11);

        let result = parsed["result"].as_array().expect("result should be an array");
        assert!(!result.is_empty());

        let mut seen_queued = false;
        for entry in result {
            if let Some(tag) = entry.get("tag").and_then(|v| v.as_str()) {
                if tag == "status-null-queued" {
                    seen_queued = true;
                    assert_eq!(entry["state"], "queued");
                    assert_eq!(entry["task_id"].as_str().unwrap(), queued_id);
                }
            }
        }

        assert!(seen_queued, "queued task should be present when task_id is null");
    }

    #[test]
    fn test_surf_status_params_null_lists_non_terminated_tasks() {
        // params 整体为 null（或缺失）时，也应返回所有非终止态任务
        let running_id = TASK_MANAGER.register_task(
            "/tmp/status-params-null-running".to_string(),
            0,
            1,
            None,
            Some("status-params-null-running".to_string()),
            TaskState::Running,
        );

        let request = r#"{"jsonrpc":"2.0","method":"Surf.Status","id":12}"#;
        let response = handle_rpc_line(request).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();

        assert_eq!(parsed["jsonrpc"], "2.0");
        assert!(parsed.get("error").is_none());
        assert_eq!(parsed["id"], 12);

        let result = parsed["result"].as_array().expect("result should be an array");
        assert!(!result.is_empty());

        let mut seen_running = false;
        for entry in result {
            if let Some(tag) = entry.get("tag").and_then(|v| v.as_str()) {
                if tag == "status-params-null-running" {
                    seen_running = true;
                    assert_eq!(entry["state"], "running");
                    assert_eq!(entry["task_id"].as_str().unwrap(), running_id);
                }
            }
        }

        assert!(seen_running, "running task should be present when params is null");
    }

    #[test]
    fn test_surf_cancel_params_not_object_invalid_params() {
        // 构造请求：params 是数组而不是对象
        let request = r#"{"jsonrpc":"2.0","method":"Surf.Cancel","params":[],"id":1}"#;
        let response = handle_rpc_line(request).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["error"]["code"], INVALID_PARAMS);
        assert_eq!(parsed["error"]["message"], "INVALID_PARAMS");
        let detail = parsed["error"]["data"]["detail"].as_str().unwrap();
        assert!(detail.contains("params must be a JSON object for method Surf.Cancel"));
        assert_eq!(parsed["id"], 1);
    }

    #[test]
    fn test_surf_cancel_missing_or_bad_task_id_invalid_params() {
        // 缺少 task_id 字段
        let request = r#"{"jsonrpc":"2.0","method":"Surf.Cancel","params":{},"id":2}"#;
        let response = handle_rpc_line(request).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert_eq!(parsed["error"]["code"], INVALID_PARAMS);
        let detail = parsed["error"]["data"]["detail"].as_str().unwrap();
        assert!(detail.contains("task_id must be a non-empty string"));
        assert_eq!(parsed["id"], 2);

        // task_id 为数字而不是字符串
        let request = r#"{"jsonrpc":"2.0","method":"Surf.Cancel","params":{"task_id":42},"id":3}"#;
        let response = handle_rpc_line(request).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert_eq!(parsed["error"]["code"], INVALID_PARAMS);
        let detail = parsed["error"]["data"]["detail"].as_str().unwrap();
        assert!(detail.contains("task_id must be a string"));
        assert_eq!(parsed["id"], 3);

        // task_id 为空字符串
        let request = r#"{"jsonrpc":"2.0","method":"Surf.Cancel","params":{"task_id":""},"id":4}"#;
        let response = handle_rpc_line(request).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert_eq!(parsed["error"]["code"], INVALID_PARAMS);
        let detail = parsed["error"]["data"]["detail"].as_str().unwrap();
        assert!(detail.contains("task_id must be a non-empty string"));
        assert_eq!(parsed["id"], 4);
    }

    #[test]
    fn test_surf_cancel_task_not_found_for_unknown_id() {
        // 合法的非空 task_id，目前一律视为不存在任务 -> TASK_NOT_FOUND
        let request = r#"{"jsonrpc":"2.0","method":"Surf.Cancel","params":{"task_id":"non-existent"},"id":5}"#;
        let response = handle_rpc_line(request).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["error"]["code"], TASK_NOT_FOUND);
        assert_eq!(parsed["error"]["message"], "TASK_NOT_FOUND");
        let detail = parsed["error"]["data"]["detail"].as_str().unwrap();
        assert!(detail.contains("task_id not found: non-existent"));
        assert_eq!(parsed["id"], 5);
    }

    #[test]
    fn test_surf_cancel_success_for_existing_queued_task() {
        // 先注册一个排队中的任务
        let task_id = TASK_MANAGER.register_task(
            "/tmp".to_string(),
            0,
            4,
            Some(10),
            Some("cancel-test".to_string()),
            TaskState::Queued,
        );

        let request = format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"Surf.Cancel\",\"params\":{{\"task_id\":\"{}\"}},\"id\":6}}",
            task_id
        );

        let response = handle_rpc_line(&request).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();

        assert_eq!(parsed["jsonrpc"], "2.0");
        // 成功路径应返回 result，而不是 error
        assert!(parsed.get("error").is_none());

        let result = parsed["result"].as_object().expect("result should be an object");
        assert_eq!(result["task_id"], task_id);
        assert_eq!(result["previous_state"], "queued");
        assert_eq!(result["current_state"], "canceled");
        assert_eq!(parsed["id"], 6);

        // 再次查询状态应反映为 canceled
        let status_request = format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"Surf.Status\",\"params\":{{\"task_id\":\"{}\"}},\"id\":7}}",
            task_id
        );
        let status_response = handle_rpc_line(&status_request).unwrap();
        let status_parsed: serde_json::Value = serde_json::from_str(&status_response).unwrap();
        let status_result = status_parsed["result"].as_object().expect("status result should be an object");
        assert_eq!(status_result["state"], "canceled");
    }

    #[test]
    fn test_surf_cancel_is_idempotent_for_terminated_task() {
        // 创建一个任务并先标记为 Canceled
        let task_id = TASK_MANAGER.register_task(
            "/tmp/idempotent".to_string(),
            0,
            2,
            None,
            None,
            TaskState::Canceled,
        );

        let request = format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"Surf.Cancel\",\"params\":{{\"task_id\":\"{}\"}},\"id\":8}}",
            task_id
        );

        let response = handle_rpc_line(&request).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();

        assert_eq!(parsed["jsonrpc"], "2.0");
        assert!(parsed.get("error").is_none());

        let result = parsed["result"].as_object().expect("result should be an object");
        assert_eq!(result["task_id"], task_id);
        // 终止态任务再次取消时，previous_state 与 current_state 相同
        assert_eq!(result["previous_state"], "canceled");
        assert_eq!(result["current_state"], "canceled");
        assert_eq!(parsed["id"], 8);
    }

    #[test]
    fn task_manager_registers_tasks_and_generates_incrementing_ids() {
        let manager = TaskManager::new();

        let id1 = manager.register_task(
            "/path/one".to_string(),
            1024,
            4,
            Some(10),
            Some("tag-one".to_string()),
            TaskState::Queued,
        );

        let id2 = manager.register_task(
            "/path/two".to_string(),
            2048,
            8,
            None,
            None,
            TaskState::Running,
        );

        assert_ne!(id1, id2, "task ids should be unique");

        let info1 = manager.get_task_info(&id1).expect("task id1 should exist");
        assert_eq!(info1.path, "/path/one");
        assert_eq!(info1.min_size_bytes, 1024);
        assert_eq!(info1.threads, 4);
        assert_eq!(info1.limit, Some(10));
        assert_eq!(info1.tag.as_deref(), Some("tag-one"));
        assert_eq!(info1.state, TaskState::Queued);

        let info2 = manager.get_task_info(&id2).expect("task id2 should exist");
        assert_eq!(info2.path, "/path/two");
        assert_eq!(info2.min_size_bytes, 2048);
        assert_eq!(info2.threads, 8);
        assert_eq!(info2.limit, None);
        assert_eq!(info2.tag, None);
        assert_eq!(info2.state, TaskState::Running);

        // started_at / updated_at 为时间戳，应该大于 0
        assert!(info1.started_at > 0);
        assert!(info1.updated_at >= info1.started_at);
        assert!(info2.started_at > 0);
        assert!(info2.updated_at >= info2.started_at);
    }

    #[test]
    fn test_surf_getresults_params_not_object_invalid_params() {
        // params 是数组而不是对象 -> INVALID_PARAMS
        let request = r#"{"jsonrpc":"2.0","method":"Surf.GetResults","params":[],"id":1}"#;
        let response = handle_rpc_line(request).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["error"]["code"], INVALID_PARAMS);
        assert_eq!(parsed["error"]["message"], "INVALID_PARAMS");
        let detail = parsed["error"]["data"]["detail"].as_str().unwrap();
        assert!(detail.contains("params must be a JSON object for method Surf.GetResults"));
        assert_eq!(parsed["id"], 1);
    }

    #[test]
    fn test_surf_getresults_unsupported_mode_invalid_params() {
        // 不支持的 mode 值应返回 INVALID_PARAMS
        let request = r#"{"jsonrpc":"2.0","method":"Surf.GetResults","params":{"task_id":"t1","mode":"unknown"},"id":2}"#;
        let response = handle_rpc_line(request).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert_eq!(parsed["error"]["code"], INVALID_PARAMS);
        let detail = parsed["error"]["data"]["detail"].as_str().unwrap();
        assert!(detail.contains("unsupported mode"));
        assert_eq!(parsed["id"], 2);
    }

    #[test]
    fn test_surf_getresults_task_not_found() {
        // 合法参数但 task_id 不存在 -> TASK_NOT_FOUND
        let request = r#"{"jsonrpc":"2.0","method":"Surf.GetResults","params":{"task_id":"non-existent"},"id":3}"#;
        let response = handle_rpc_line(request).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["error"]["code"], TASK_NOT_FOUND);
        assert_eq!(parsed["error"]["message"], "TASK_NOT_FOUND");
        let detail = parsed["error"]["data"]["detail"].as_str().unwrap();
        assert!(detail.contains("task_id not found: non-existent"));
        assert_eq!(parsed["id"], 3);
    }

    #[test]
    fn test_surf_getresults_task_not_completed_invalid_params() {
        // 任务存在但未处于 Completed 状态 -> INVALID_PARAMS
        let task_id = TASK_MANAGER.register_task(
            "/tmp/getresults-running".to_string(),
            0,
            1,
            None,
            Some("getresults-running".to_string()),
            TaskState::Running,
        );

        let request = format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"Surf.GetResults\",\"params\":{{\"task_id\":\"{}\"}},\"id\":4}}",
            task_id
        );
        let response = handle_rpc_line(&request).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert_eq!(parsed["error"]["code"], INVALID_PARAMS);
        let detail = parsed["error"]["data"]["detail"].as_str().unwrap();
        assert!(detail.contains("task is not in completed state"));
        assert!(detail.contains("running"));
        assert_eq!(parsed["id"], 4);
    }

    #[test]
    fn test_surf_getresults_completed_task_returns_placeholder_result() {
        // 对于 Completed 状态的任务，应返回占位性的聚合结果结构
        let task_id = TASK_MANAGER.register_task(
            "/tmp/getresults-completed".to_string(),
            0,
            1,
            None,
            Some("getresults-completed".to_string()),
            TaskState::Completed,
        );

        let request = format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"Surf.GetResults\",\"params\":{{\"task_id\":\"{}\"}},\"id\":5}}",
            task_id
        );
        let response = handle_rpc_line(&request).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();

        assert_eq!(parsed["jsonrpc"], "2.0");
        assert!(parsed.get("error").is_none());
        assert_eq!(parsed["id"], 5);

        let result = parsed["result"].as_object().expect("result should be an object");
        assert_eq!(result["task_id"].as_str().unwrap(), task_id);
        assert_eq!(result["state"].as_str().unwrap(), "completed");
        assert_eq!(result["path"].as_str().unwrap(), "/tmp/getresults-completed");
        assert_eq!(result["total_files"].as_u64().unwrap(), 0);
        assert_eq!(result["total_bytes"].as_u64().unwrap(), 0);
        let entries = result["entries"].as_array().expect("entries should be an array");
        assert!(entries.is_empty());
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
                                    // 数值也合法，执行成功路径
                                    // 解析 min_size_bytes
                                    let min_size_bytes = match scan_params.min_size {
                                        Some(ref s) => parse_size_for_service(s).unwrap_or(0),
                                        None => 0,
                                    };
                                    // 计算最终线程数
                                    let threads = scan_params.threads.unwrap_or_else(|| num_cpus::get());
                                    // 构造 surf-core 扫描配置
                    let config = surf_core::ScanConfig {
                        root: PathBuf::from(&scan_params.path),
                        min_size: min_size_bytes,
                        threads,
                    };
                    // 启动真实扫描任务
                    match surf_core::start_scan(config) {
                        Ok(handle) => {
                            // 扫描已启动，注册任务并传入扫描句柄。`ScanHandle`
                            // 内部基于 `Arc` 共享状态，这里无需再额外包一层 `Arc`。
                            let task_id = TASK_MANAGER.register_task_with_handle(
                                scan_params.path.clone(),
                                min_size_bytes,
                                threads,
                                scan_params.limit,
                                scan_params.tag.clone(),
                                TaskState::Running,
                                Some(handle),
                            );
                            // 构造成功响应
                            let result = SurfScanResult {
                                task_id,
                                state: "running".to_string(),
                                path: scan_params.path,
                                min_size_bytes,
                                threads,
                                limit: scan_params.limit,
                            };
                            let success_response = JsonRpcSuccessResponse::from_result(result, req.id);
                            // 提前返回成功响应
                            return Some(serde_json::to_string(&success_response).unwrap_or_else(|_| String::new()));
                        }
                        Err(e) => JsonRpcError::invalid_params(Some(format!("failed to start scan: {}", e))),
                    }
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
            // Surf.Status 对参数形状有基本校验，并在 task_id 为非空字符串时
            // 尝试从内存任务管理器中读取对应任务的元数据：
            // - params 为 null 时，返回所有处于非终止态（queued/running）的任务列表；
            // - params 不是对象 -> INVALID_PARAMS；
            // - params 为对象且 task_id 为非空字符串时：
            //   - 若能在 TASK_MANAGER 中找到对应任务，返回成功响应；
            //   - 否则返回 TASK_NOT_FOUND；
            // - task_id 类型错误或为空字符串 -> INVALID_PARAMS；
            // - task_id 字段缺失或显式为 null -> 返回所有处于非终止态的任务列表。
            "Surf.Status" => {
                // 公共逻辑：构造“列出所有非终止态任务”的成功响应
                let list_all_non_terminated = || {
                    let tasks = TASK_MANAGER.list_non_terminated_tasks();
                    let results: Vec<SurfStatusResult> = tasks
                        .into_iter()
                        .map(|(id, info)| SurfStatusResult::from_task_info(&id, info))
                        .collect();
                    let success_response =
                        JsonRpcSuccessResponse::from_result(results, req.id.clone());
                    Some(
                        serde_json::to_string(&success_response)
                            .unwrap_or_else(|_| String::new()),
                    )
                };

                if req.params.is_null() {
                    // 缺少参数：按 Architecture.md 4.3.4 约定，视为查询所有活跃任务
                    return list_all_non_terminated();
                } else if !req.params.is_object() {
                    // params 不是对象（数组/字符串/数字等） -> INVALID_PARAMS
                    let detail = format!("params must be a JSON object for method {}", method);
                    JsonRpcError::invalid_params(Some(detail))
                } else {
                    // params 为对象，检查 task_id 字段
                    let obj = req.params.as_object().unwrap();
                    let task_id_value = obj.get("task_id");
                    match task_id_value {
                        None => {
                            // task_id 缺失：按约定列出所有活跃任务
                            return list_all_non_terminated();
                        }
                        Some(v) => {
                            if v.is_string() {
                                let task_id_str = v.as_str().unwrap();
                                if task_id_str.is_empty() {
                                    // 空字符串视为无效参数
                                    let detail = "task_id must be a non-empty string or null".to_string();
                                    JsonRpcError::invalid_params(Some(detail))
                                } else {
                                    // 非空字符串：尝试从任务管理器查询任务元数据
                                    match TASK_MANAGER.get_task_info(task_id_str) {
                                        Some(info) => {
                                            let result =
                                                SurfStatusResult::from_task_info(task_id_str, info);
                                            let success_response =
                                                JsonRpcSuccessResponse::from_result(result, req.id);
                                            return Some(
                                                serde_json::to_string(&success_response)
                                                    .unwrap_or_else(|_| String::new()),
                                            );
                                        }
                                        None => {
                                            let detail = format!("task_id not found: {}", task_id_str);
                                            JsonRpcError::task_not_found(Some(detail))
                                        }
                                    }
                                }
                            } else if v.is_null() {
                                // task_id 显式为 null：列出所有活跃任务
                                return list_all_non_terminated();
                            } else {
                                // task_id 既不是 string 也不是 null -> INVALID_PARAMS
                                let detail = "task_id must be a string or null".to_string();
                                JsonRpcError::invalid_params(Some(detail))
                            }
                        }
                    }
                }
            }
            "Surf.Cancel" => {
                if req.params.is_null() {
                    // 缺少参数但方法本身受支持：当前仅作为“尚未实现”的占位
                    JsonRpcError::method_not_found(Some("method not implemented yet".to_string()))
                } else if !req.params.is_object() {
                    // params 不是对象（数组/字符串/数字等） -> INVALID_PARAMS
                    let detail = format!("params must be a JSON object for method {}", method);
                    JsonRpcError::invalid_params(Some(detail))
                } else {
                    // params 为对象，检查 task_id 字段
                    let obj = req.params.as_object().unwrap();
                    match obj.get("task_id") {
                        None => {
                            // 缺少 task_id -> INVALID_PARAMS
                            let detail = "task_id must be a non-empty string".to_string();
                            JsonRpcError::invalid_params(Some(detail))
                        }
                        Some(v) => {
                            if v.is_string() {
                                let task_id_str = v.as_str().unwrap();
                                if task_id_str.is_empty() {
                                    // 空字符串视为无效参数
                                    let detail = "task_id must be a non-empty string".to_string();
                                    JsonRpcError::invalid_params(Some(detail))
                                } else {
                                    // 使用全局 TASK_MANAGER 执行幂等取消逻辑
                                    match TASK_MANAGER.cancel_task(task_id_str) {
                                        Some((previous_state, updated_info)) => {
                                            let previous_state_str = match previous_state {
                                                TaskState::Queued => "queued",
                                                TaskState::Running => "running",
                                                TaskState::Completed => "completed",
                                                TaskState::Failed => "failed",
                                                TaskState::Canceled => "canceled",
                                            }
                                            .to_string();

                                            let current_state_str = match updated_info.state {
                                                TaskState::Queued => "queued",
                                                TaskState::Running => "running",
                                                TaskState::Completed => "completed",
                                                TaskState::Failed => "failed",
                                                TaskState::Canceled => "canceled",
                                            }
                                            .to_string();

                                            let result = SurfCancelResult {
                                                task_id: task_id_str.to_string(),
                                                previous_state: previous_state_str,
                                                current_state: current_state_str,
                                            };

                                            let success_response =
                                                JsonRpcSuccessResponse::from_result(result, req.id);
                                            return Some(
                                                serde_json::to_string(&success_response)
                                                    .unwrap_or_else(|_| String::new()),
                                            );
                                        }
                                        None => {
                                            // 未找到任务 -> TASK_NOT_FOUND
                                            let detail =
                                                format!("task_id not found: {}", task_id_str);
                                            JsonRpcError::task_not_found(Some(detail))
                                        }
                                    }
                                }
                            } else {
                                // task_id 既不是 string 也不是 null -> INVALID_PARAMS
                                let detail = "task_id must be a string".to_string();
                                JsonRpcError::invalid_params(Some(detail))
                            }
                        }
                    }
                }
            }
            "Surf.GetResults" => {
                // 根据 Architecture.md 4.3.5，对 Surf.GetResults 进行参数与状态校验。
                // 当前仅返回占位性的聚合结果（total_files/total_bytes 为 0，entries 为空），
                // 真正的结果聚合与缓存将在后续迭代中实现。
                if req.params.is_null() || !req.params.is_object() {
                    let detail = format!(
                        "params must be a JSON object for method {}",
                        method
                    );
                    JsonRpcError::invalid_params(Some(detail))
                } else {
                    // params 为对象，尝试解析为 SurfGetResultsParams
                    match serde_json::from_value::<SurfGetResultsParams>(req.params.clone()) {
                        Err(e) => {
                            let detail = format!("invalid Surf.GetResults params: {}", e);
                            JsonRpcError::invalid_params(Some(detail))
                        }
                        Ok(get_params) => {
                            // 仅支持 mode 缺省或 "flat"/"summary"，其他取值视为无效
                            if let Some(ref mode) = get_params.mode {
                                let mode_lc = mode.to_lowercase();
                                if mode_lc != "flat" && mode_lc != "summary" {
                                    let detail = format!(
                                        "unsupported mode for Surf.GetResults: {}",
                                        mode
                                    );
                                    return Some(
                                        serde_json::to_string(&JsonRpcErrorResponse::from_error(
                                            JsonRpcError::invalid_params(Some(detail)),
                                            req.id,
                                        ))
                                        .unwrap_or_else(|_| String::new()),
                                    );
                                }
                            }

                            // 查询任务信息
                            match TASK_MANAGER.get_task_info(&get_params.task_id) {
                                None => {
                                    let detail = format!(
                                        "task_id not found: {}",
                                        get_params.task_id
                                    );
                                    JsonRpcError::task_not_found(Some(detail))
                                }
                                Some(info) => {
                                    // 仅在任务已处于 Completed 状态时返回结果；其他状态一律视为
                                    // INVALID_PARAMS，与 Architecture.md 4.3.5 中的约定对齐。
                                    if info.state != TaskState::Completed {
                                        let state_str = match info.state {
                                            TaskState::Queued => "queued",
                                            TaskState::Running => "running",
                                            TaskState::Completed => "completed",
                                            TaskState::Failed => "failed",
                                            TaskState::Canceled => "canceled",
                                        };
                                        let detail = format!(
                                            "task is not in completed state (current: {})",
                                            state_str
                                        );
                                        JsonRpcError::invalid_params(Some(detail))
                                    } else {
                                        // 当前版本尚未集成真实的聚合结果，仅返回占位数据：
                                        // - total_files/total_bytes: 0
                                        // - entries: 空数组
                                        let result = SurfGetResultsResult {
                                            task_id: get_params.task_id,
                                            state: "completed".to_string(),
                                            path: info.path,
                                            total_files: 0,
                                            total_bytes: 0,
                                            entries: Vec::new(),
                                        };
                                        let success_response =
                                            JsonRpcSuccessResponse::from_result(result, req.id);
                                        return Some(
                                            serde_json::to_string(&success_response)
                                                .unwrap_or_else(|_| String::new()),
                                        );
                                    }
                                }
                            }
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

    // 绑定 TCP 监听，基于行分隔 JSON 实现 JSON-RPC 2.0 协议骨架。
    let listener = TcpListener::bind(&addr).await?;

    println!(
        "surf-service listening on {addr} (max_concurrent_scans={max}, task_ttl_seconds={ttl}).\nJSON-RPC service ready: Surf.Scan / Surf.Status / Surf.Cancel 已与 surf-core 进度感知扫描 API 打通（含任务登记与取消语义）；Surf.GetResults 及结果聚合/缓存接口仍待在后续迭代中实现。",
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
