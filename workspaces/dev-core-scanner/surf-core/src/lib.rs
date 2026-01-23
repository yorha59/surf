use std::path::{Path, PathBuf};

use rayon::iter::{ParallelBridge, ParallelIterator};
use rayon::ThreadPoolBuilder;
use walkdir::WalkDir;

/// A single file entry discovered by the scanner.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FileEntry {
    pub path: PathBuf,
    pub size: u64,
}

/// Recursively scan `root` and return files with size >= `min_size` bytes,
/// sorted by size in descending order.
///
/// `threads` 参数用于控制扫描时使用的工作线程数。为了健壮性，如果传入 0，
/// 将自动退化为使用单线程。
pub fn scan(root: &Path, min_size: u64, threads: usize) -> std::io::Result<Vec<FileEntry>> {
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
