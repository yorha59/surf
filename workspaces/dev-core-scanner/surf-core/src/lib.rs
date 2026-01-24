use std::path::{Path, PathBuf};

use rayon::iter::{ParallelBridge, ParallelIterator};
use rayon::ThreadPoolBuilder;
use walkdir::WalkDir;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

/// A single file entry discovered by the scanner.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FileEntry {
    pub path: PathBuf,
    pub size: u64,
}

/// 配置扫描任务的参数。
#[derive(Debug, Clone)]
pub struct ScanConfig {
    pub root: PathBuf,
    pub min_size: u64,
    pub threads: usize,
}

/// 扫描进度快照。
#[derive(Debug, Clone)]
pub struct ScanProgress {
    pub scanned_files: u64,
    pub scanned_bytes: u64,
    pub total_bytes_estimate: Option<u64>,
}

/// 扫描状态快照。
#[derive(Debug, Clone)]
pub struct StatusSnapshot {
    /// 仅反映底层扫描是否已经自然结束；
    /// 任务级状态（queued/running/completed/failed/canceled）仍由服务层维护。
    pub done: bool,
    pub progress: ScanProgress,
    /// 若底层扫描因 IO 等原因失败，这里给出摘要信息（例如 `ErrorKind` + 文本描述），
    /// 供服务层映射为 JSON-RPC 错误码；具体结构在实现时可细化。
    pub error: Option<String>,
}

/// Recursively scan `root` and return files with size >= `min_size` bytes,
/// sorted by size in descending order.
///
/// `threads` 参数用于控制扫描时使用的工作线程数。为了健壮性，如果传入 0，
/// 将自动退化为使用单线程。
pub fn scan(root: &Path, min_size: u64, threads: usize) -> std::io::Result<Vec<FileEntry>> {
    // 显式校验 root 是否存在，避免对不存在路径进行静默扫描。
    if !root.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("scan root does not exist: {}", root.display()),
        ));
    }

    // 额外保护：虽然 CLI 层已经禁止了 0，但库层仍做一次防御性处理。
    let threads = threads.max(1);

    // 为本次扫描构建一个局部线程池，避免全局线程池多次初始化的问题。
    let pool = ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("failed to build thread pool: {e}"),
            )
        })?;

    // 在局部线程池内，将 WalkDir 的迭代器通过 `par_bridge` 转换为并行迭代，
    // 并发获取元数据、过滤和收集结果。
    let mut entries: Vec<FileEntry> = pool.install(|| {
        WalkDir::new(root)
            .into_iter()
            .filter_map(Result::ok)
            .par_bridge()
            .filter_map(|entry| {
                let metadata = entry.metadata().ok()?;

                if !metadata.is_file() {
                    return None;
                }

                let size = metadata.len();
                if size < min_size {
                    return None;
                }

                Some(FileEntry {
                    path: entry.into_path(),
                    size,
                })
            })
            .collect()
    });

    // Sort by size descending.
    entries.sort_by(|a, b| b.size.cmp(&a.size));
    Ok(entries)
}

/// 扫描任务的句柄，用于查询进度、获取结果或取消任务。
/// 
/// 该类型实现了 `Send` 和 `Sync`，可以在线程间安全传递；
/// 同时通过 `Clone` 共享同一内部状态（基于 `Arc`）。
#[derive(Clone)]
pub struct ScanHandle {
    inner: Arc<ScanState>,
}

/// 扫描任务的内部状态。
struct ScanState {
    /// 后台线程的 JoinHandle，用于等待扫描完成。
    thread: Mutex<Option<thread::JoinHandle<()>>>,
    /// 扫描结果，在后台线程完成后填充。
    result: Mutex<Option<std::io::Result<Vec<FileEntry>>>>,
    /// 当前进度。
    progress: Mutex<ScanProgress>,
    /// 错误信息（如果有）。
    error: Mutex<Option<String>>,
    /// 取消标志。
    cancelled: AtomicBool,
    /// 扫描是否已完成（无论成功或失败）。
    done: AtomicBool,
}

impl ScanHandle {
    /// 启动一个新的扫描任务。
    ///
    /// 如果 `config.root` 不存在，或者线程池构建失败，会立即返回错误。
    /// 成功时返回一个 `ScanHandle`，后台扫描线程已经开始运行。
    pub fn start_scan(config: ScanConfig) -> std::io::Result<ScanHandle> {
        // 检查 root 是否存在（复用 scan 中的检查逻辑）
        if !config.root.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("scan root does not exist: {}", config.root.display()),
            ));
        }

        // 构建线程池（复用 scan 中的逻辑）
        let threads = config.threads.max(1);
        let pool = ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .map_err(|e| {
                std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("failed to build thread pool: {e}"),
                )
            })?;

        // 创建内部状态
        let state = Arc::new(ScanState {
            thread: Mutex::new(None),
            result: Mutex::new(None),
            progress: Mutex::new(ScanProgress {
                scanned_files: 0,
                scanned_bytes: 0,
                total_bytes_estimate: None,
            }),
            error: Mutex::new(None),
            cancelled: AtomicBool::new(false),
            done: AtomicBool::new(false),
        });

        // 克隆 Arc 用于后台线程
        let thread_state = Arc::clone(&state);
        let config_clone = config.clone();

        // 启动后台线程
        let join_handle = thread::spawn(move || {
            // 在线程池中执行扫描
            let scan_result = pool.install(|| {
                // 直接实现扫描逻辑，以便在扫描过程中逐步更新进度
                let root = config_clone.root.clone();
                let min_size = config_clone.min_size;
                
                // 如果已经取消，直接返回 Interrupted 错误
                if thread_state.cancelled.load(Ordering::SeqCst) {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Interrupted,
                        "scan was cancelled",
                    ));
                }
                
                // 收集符合条件的文件条目
                let entries: Vec<FileEntry> = WalkDir::new(&root)
                    .into_iter()
                    .filter_map(Result::ok)
                    .par_bridge()
                    .filter_map(|entry| {
                        let metadata = entry.metadata().ok()?;
                        // 检查取消标志，如果已取消则跳过后续处理
                        if thread_state.cancelled.load(Ordering::SeqCst) {
                            return None;
                        }

                        if !metadata.is_file() {
                            return None;
                        }

                        let size = metadata.len();
                        if size < min_size {
                            return None;
                        }

                        // 更新进度：文件数+1，字节数+size
                        {
                            let mut progress = thread_state.progress.lock().unwrap();
                            progress.scanned_files += 1;
                            progress.scanned_bytes += size;
                        }

                        Some(FileEntry {
                            path: entry.into_path(),
                            size,
                        })
                    })
                    .collect();
                
                // 按大小降序排序
                let mut sorted_entries = entries;
                sorted_entries.sort_by(|a, b| b.size.cmp(&a.size));
                
                // 检查是否在扫描过程中被取消
                if thread_state.cancelled.load(Ordering::SeqCst) {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::Interrupted,
                        "scan was cancelled",
                    ))
                } else {
                    Ok(sorted_entries)
                }
            });

            // 存储结果
            *thread_state.result.lock().unwrap() = Some(scan_result);
            
            // 根据扫描结果设置 error 字段
            let mut error_lock = thread_state.error.lock().unwrap();
            *error_lock = match &scan_result {
                Ok(_) => None,
                Err(e) => Some(e.to_string()),
            };
            
            // 标记完成
            thread_state.done.store(true, Ordering::SeqCst);
        });

        // 存储线程句柄
        *state.thread.lock().unwrap() = Some(join_handle);

        Ok(ScanHandle { inner: state })
    }

    /// 查询扫描任务的当前状态。
    ///
    /// 返回一个快照，包含进度、是否完成以及可能的错误信息。
    pub fn poll_status(&self) -> StatusSnapshot {
        let progress = self.inner.progress.lock().unwrap().clone();
        let error = self.inner.error.lock().unwrap().clone();
        let done = self.inner.done.load(Ordering::SeqCst);
        
        StatusSnapshot {
            done,
            progress,
            error,
        }
    }

    /// 等待扫描完成并返回结果。
    ///
    /// 如果扫描过程中出现错误，返回对应的 `std::io::Error`。
    /// 如果扫描被取消，返回一个 `ErrorKind::Interrupted` 错误。
    /// 注意：调用此函数后，`ScanHandle` 将被消耗，无法再用于查询状态。
    pub fn collect_results(self) -> std::io::Result<Vec<FileEntry>> {
        // 等待线程结束（如果还在运行）
        if let Some(handle) = self.inner.thread.lock().unwrap().take() {
            let _ = handle.join();
        }
        
        // 获取结果
        match self.inner.result.lock().unwrap().take() {
            Some(Ok(entries)) => Ok(entries),
            Some(Err(e)) => Err(e),
            None => {
                // 如果没有结果，可能是被取消了
                if self.inner.cancelled.load(Ordering::SeqCst) {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::Interrupted,
                        "scan was cancelled",
                    ))
                } else {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "scan completed without result",
                    ))
                }
            }
        }
    }

    /// 尝试取消扫描任务。
    ///
    /// 这是一个“最佳努力”的取消：设置取消标志，后台线程会在合适的检查点
    /// 读取该标志并提前结束。当前 MVP 版本中，扫描逻辑可能不会检查取消标志，
    /// 因此取消可能不会立即生效。未来版本可以增强实时取消能力。
    pub fn cancel(&self) {
        self.inner.cancelled.store(true, Ordering::SeqCst);
    }
}

/// 启动一个新的扫描任务（便捷函数）。
///
/// 这是 `ScanHandle::start_scan` 的别名，提供更符合 Rust 命名习惯的 API。
pub fn start_scan(config: ScanConfig) -> std::io::Result<ScanHandle> {
    ScanHandle::start_scan(config)
}

/// 查询扫描任务的当前状态（便捷函数）。
///
/// 这是 `ScanHandle::poll_status` 的别名。
pub fn poll_status(handle: &ScanHandle) -> StatusSnapshot {
    handle.poll_status()
}

/// 等待扫描完成并返回结果（便捷函数）。
///
/// 这是 `ScanHandle::collect_results` 的别名。
pub fn collect_results(handle: ScanHandle) -> std::io::Result<Vec<FileEntry>> {
    handle.collect_results()
}

/// 尝试取消扫描任务（便捷函数）。
///
/// 这是 `ScanHandle::cancel` 的别名。
pub fn cancel(handle: &ScanHandle) {
    handle.cancel()
}
