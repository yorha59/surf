#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::api::path::home_dir;

/// 与 Architecture.md 4.5.1 中约定的配置结构相对应的最小子集。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SurfConfig {
  pub default_path: String,
  pub threads: u32,
  pub min_size: String,
  pub rpc_host: String,
  pub rpc_port: u16,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub cli_path: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub theme: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub language: Option<String>,
}

fn config_file_path() -> Result<PathBuf, String> {
  let home = home_dir().ok_or_else(|| "无法获取用户主目录".to_string())?;
  Ok(home.join(".config").join("surf").join("config.json"))
}

fn ensure_config_dir(path: &PathBuf) -> Result<(), String> {
  if let Some(dir) = path.parent() {
    fs::create_dir_all(dir).map_err(|e| format!("创建配置目录失败: {e}"))?;
  }
  Ok(())
}

/// 从统一路径 `~/.config/surf/config.json` 读取配置。
///
/// 返回值语义：
/// - Ok(Some(config))：存在且成功解析；
/// - Ok(None)：文件不存在或不可解析（不可解析时会尝试备份为 `config.json.bak`），
///   由前端进入 Onboarding 流程重新生成配置；
/// - Err(msg)：发生 IO / 其他不可恢复错误。
#[tauri::command]
fn read_config() -> Result<Option<SurfConfig>, String> {
  let path = config_file_path()?;
  if !path.exists() {
    return Ok(None);
  }

  let content = fs::read_to_string(&path)
    .map_err(|e| format!("读取配置文件失败: {e}"))?;

  match serde_json::from_str::<SurfConfig>(&content) {
    Ok(cfg) => Ok(Some(cfg)),
    Err(e) => {
      // 无法解析旧配置时，按设计建议备份为 `config.json.bak`，然后视为“无配置”。
      if let Some(dir) = path.parent() {
        let backup_path = dir.join("config.json.bak");
        let _ = fs::rename(&path, &backup_path).or_else(|_| {
          // 某些文件系统上 rename 可能失败，退回为 copy。
          fs::copy(&path, &backup_path).map(|_| ())
        });
      }
      eprintln!("[surf tauri] 解析配置失败，将进入 Onboarding 流程: {e}");
      Ok(None)
    }
  }
}

/// 将配置写入统一路径 `~/.config/surf/config.json`。
#[tauri::command]
fn write_config(config: SurfConfig) -> Result<(), String> {
  let path = config_file_path()?;
  ensure_config_dir(&path)?;

  let json = serde_json::to_string_pretty(&config)
    .map_err(|e| format!("序列化配置失败: {e}"))?;

  fs::write(&path, json).map_err(|e| format!("写入配置文件失败: {e}"))
}

fn main() {
  tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![read_config, write_config])
    .setup(|_app| {
      // 预留与 JSON-RPC 客户端 (`rpc_client` 模块) 的集成位置。
      // 后续可在此注册更多 Tauri 命令（例如 scan_*），调用 Rust 侧的 TCP/HTTP 客户端实现。
      Ok(())
    })
    .run(tauri::generate_context!())
    .expect("error while running Surf Tauri application");
}
