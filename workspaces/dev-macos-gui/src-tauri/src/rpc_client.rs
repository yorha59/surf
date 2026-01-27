//! JSON-RPC 客户端占位模块。
+ //!
+ //! 当前仅返回模拟数据，默认目标地址为 `127.0.0.1:1234`，用于为后续
+ //! 与 `dev-service-api` 的真实集成预留接口位置。

use serde::{Deserialize, Serialize};

const DEFAULT_ADDR: &str = "127.0.0.1:1234";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanStatus {
  pub task_id: String,
  pub state: String,
  pub progress: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanSummaryMock {
  pub root_path: String,
  pub total_files: u64,
  pub total_dirs: u64,
  pub total_size_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct RpcClient {
  addr: String,
}

impl Default for RpcClient {
  fn default() -> Self {
    Self {
      addr: DEFAULT_ADDR.to_string(),
    }
  }
}

impl RpcClient {
  pub fn new(addr: impl Into<String>) -> Self {
    Self { addr: addr.into() }
  }

  /// 启动扫描任务（占位实现）。
  ///
  /// 未来将在此通过 TCP JSON-RPC 调用 `scan.start`，当前仅返回模拟 task_id。
  pub async fn scan_start(&self, _path: &str) -> Result<String, String> {
    let _ = &self.addr; // 仅为占位，避免未使用字段警告。
    Ok("mock-task-id".to_string())
  }

  /// 查询扫描任务状态（占位实现）。
  pub async fn scan_status(&self, task_id: &str) -> Result<ScanStatus, String> {
    let _ = &self.addr;
    Ok(ScanStatus {
      task_id: task_id.to_string(),
      state: "disconnected".to_string(),
      progress: 0.0,
    })
  }

  /// 获取扫描结果（占位实现）。
  pub async fn scan_result(&self, task_id: &str) -> Result<ScanSummaryMock, String> {
    let _ = &self.addr;
    Ok(ScanSummaryMock {
      root_path: "/".to_string(),
      total_files: 0,
      total_dirs: 0,
      total_size_bytes: 0,
    })
  }

  /// 取消扫描任务（占位实现）。
  pub async fn scan_cancel(&self, task_id: &str) -> Result<bool, String> {
    let _ = (&self.addr, task_id);
    Ok(false)
  }
}
