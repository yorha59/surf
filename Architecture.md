# Surf 技术架构设计

## 1. 核心模块设计

### 1.1 扫描引擎 (Scanner Engine)
*   **多线程递归算法**: 采用基于消息队列或工作池的并发遍历。主线程负责分发目录任务，工作线程执行 `readdir` 并返回统计数据。
*   **缓存机制**: 为了提升重复扫描速度，可以引入简单的文件元数据缓存（如 `mtime` 校验）。
*   **权限处理**: 优雅处理权限不足的目录，记录错误但不中断扫描。

### 1.2 数据存储 (Data Aggregator)
*   **内存树结构**: 在内存中维护一份轻量级的目录树，节点仅存储路径哈希、大小、子节点指针。
*   **延迟加载**: 只有在用户展开目录时，才从汇总数据中读取详细信息。

### 1.3 服务层 (Service Layer)
*   **JSON-RPC 接口**: 实现标准的 JSON-RPC 2.0 协议，暴露以下方法：
    *   `Surf.Scan(path, minSize)`: 启动异步扫描。
    *   `Surf.Status()`: 返回当前扫描进度、速度及已扫描文件数。
    *   `Surf.GetResults()`: 获取按大小排序后的扫描结果。
*   **并发安全**: 确保扫描引擎在服务模式下可以被安全地触发、取消和查询。

### 1.4 图形界面层 (GUI Layer - Tauri + React)
*   **Frontend (React)**:
    *   **框架**: Vite + React 18。
    *   **通信桥接**: 使用 Tauri 的 `invoke` 系统调用 Rust 后端封装好的 JSON-RPC 客户端。
    *   **状态管理**: 
        *   `React Query` 用于管理扫描任务状态和结果集的缓存。
        *   `Zustand` 用于管理全局 UI 状态（如当前视图切换、主题色）。
*   **Backend Bridge (Rust)**:
    *   **Tauri Commands**: 定义一系列暴露给前端的函数（如 `start_scan`, `stop_scan`, `fetch_results`）。
    *   **子进程管理**: 在后台静默管理 `surf --service` 进程，或直接以库的形式调用扫描逻辑。
    *   **原生集成**: 使用 `std::process` 或 macOS 特定的 `AppKit` API 实现文件操作（如移至废纸篓）。

### 1.5 持久化存储层 (Persistence Layer)
*   **存储实现**: 使用 **SQLite** (通过 `rusqlite` 或 `sqlx`) 存储非临时数据。
*   **数据范围**:
    *   **App Config**: 用户偏好设置（并发数、过滤规则、主题、语言）。
    *   **Scan History**: 历史扫描记录的元数据（路径、结果摘要、完成时间）。
*   **存储位置**: 遵循 macOS 规范，存储于 `~/Library/Application Support/surf/`。

## 2. 核心算法逻辑 (Rust 示例)

```rust
use std::path::Path;
use rayon::prelude::*;
use walkdir::WalkDir;

pub struct Scanner {
    min_size: u64,
}

impl Scanner {
    pub fn scan(&self, root: &Path) -> Vec<FileInfo> {
        // 使用 WalkDir 获取条目，结合 Rayon 进行并行处理
        WalkDir::new(root)
            .into_iter()
            .filter_map(|e| e.ok())
            .par_bridge() // 将迭代器转换为并行桥接
            .filter(|entry| {
                entry.file_type().is_file() && 
                entry.metadata().map(|m| m.len() >= self.min_size).unwrap_or(false)
            })
            .map(|entry| {
                FileInfo {
                    path: entry.path().to_path_buf(),
                    size: entry.metadata().unwrap().len(),
                }
            })
            .collect()
    }
}
```

## 3. 性能优化策略
*   **系统调用减少**: 批量读取文件元数据。
*   **内存池**: 预分配常用结构体，减少 GC 或分配压力。
*   **分片扫描**: 对于巨大的目录（如百万级文件），采用分片处理以避免单个任务执行时间过长。
