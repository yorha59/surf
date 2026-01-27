#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use tauri::Manager;

fn main() {
  tauri::Builder::default()
    .setup(|_app| {
      // 预留与 JSON-RPC 客户端 (`rpc_client` 模块) 的集成位置。
      // 后续可在此注册 Tauri 命令，调用 Rust 侧的 scan_* 接口。
      Ok(())
    })
    .run(tauri::generate_context!())
    .expect("error while running Surf Tauri application");
}

