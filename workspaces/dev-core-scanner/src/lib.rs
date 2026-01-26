//! Surf 核心扫描与分析引擎库
//! 
//! 提供文件系统扫描、目录树构建、统计分析等核心能力。

use std::path::{Path, PathBuf};
use std::time::SystemTime;
use std::collections::{BinaryHeap, HashMap};
use std::cmp::Reverse;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use rayon;
use glob::Pattern;
use serde::Serialize;
/// 扫描请求参数
#[derive(Debug, Clone, Serialize)]
pub struct ScanRequest {
    /// 扫描起始根目录
    pub root_path: PathBuf,
    /// 并发扫描线程数（默认逻辑核心数）
    pub threads: Option<u16>,
    /// 最小文件大小过滤（字节）
    pub min_size: Option<u64>,
    /// 排除规则（glob 模式）
    pub exclude_patterns: Vec<String>,
    /// 时间分析阈值天数（识别陈旧文件）
    pub stale_days: Option<u32>,
    /// Top N 大文件数量限制（默认20）
    pub limit: Option<usize>,
}

impl ScanRequest {
    /// 创建一个新的扫描请求
    pub fn new<P: Into<PathBuf>>(root_path: P) -> Self {
        Self {
            root_path: root_path.into(),
            threads: None,
            min_size: None,
            exclude_patterns: Vec::new(),
            stale_days: None,
            limit: None,
        }
    }
}

/// 扫描进度信息
#[derive(Debug, Clone, Serialize)]
pub struct ScanProgress {
    /// 任务状态
    pub state: ScanState,
    /// 已扫描文件数
    pub scanned_files: u64,
    /// 已遍历字节数
    pub scanned_bytes: u64,
    /// 进度百分比 (0.0 - 1.0)
    pub progress: f64,
    /// 预计剩余时间（秒）
    pub eta_seconds: Option<u64>,
}

/// 扫描任务状态
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum ScanState {
    /// 任务排队中
    Queued,
    /// 扫描进行中
    Running,
    /// 扫描已完成
    Completed,
    /// 任务已取消
    Canceled,
    /// 任务失败
    Failed,
}

/// 扫描结果摘要
#[derive(Debug, Clone, Serialize)]
pub struct ScanSummary {
    /// 扫描根路径
    pub root_path: PathBuf,
    /// 总文件数
    pub total_files: u64,
    /// 总目录数
    pub total_dirs: u64,
    /// 总大小（字节）
    pub total_size_bytes: u64,
    /// 扫描耗时（秒）
    pub elapsed_seconds: f64,
}

/// 文件条目信息
#[derive(Debug, Clone, Serialize)]
pub struct FileEntry {
    /// 文件路径
    pub path: PathBuf,
    /// 文件大小（字节）
    pub size_bytes: u64,
    /// 最后修改时间
    pub last_modified: Option<SystemTime>,
    /// 文件扩展名（不含点）
    pub extension: Option<String>,
}

impl Ord for FileEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // 主要按文件大小降序排列（更大的文件排前面）
        // 但为了在最小堆中使用，我们实际上需要升序排列，因此这里实现升序
        // 后续会用 Reverse 包装来反转比较
        self.size_bytes.cmp(&other.size_bytes)
            .then_with(|| self.path.cmp(&other.path))
    }
}

impl PartialOrd for FileEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for FileEntry {}

impl PartialEq for FileEntry {
    fn eq(&self, other: &Self) -> bool {
        self.size_bytes == other.size_bytes && self.path == other.path
    }
}

/// 文件类型统计
#[derive(Debug, Clone, Serialize)]
pub struct ExtensionStat {
    /// 文件扩展名（不含点）
    pub extension: String,
    /// 文件数量
    pub file_count: u64,
    /// 总大小（字节）
    pub total_size_bytes: u64,
}

/// 扫描完整结果
#[derive(Debug, Clone, Serialize)]
pub struct ScanResult {
    /// 扫描摘要
    pub summary: ScanSummary,
    /// Top N 大文件列表（按大小降序）
    pub top_files: Vec<FileEntry>,
    /// 按扩展名统计
    pub by_extension: Vec<ExtensionStat>,
    /// 陈旧文件列表（超过阈值未访问/修改）
    pub stale_files: Vec<FileEntry>,
}

/// 扫描过程中用于收集统计信息的内部结构
struct ScanCollector {
    /// 总文件数（经过 min_size 过滤后）
    total_files: u64,
    /// 总目录数
    total_dirs: u64,
    /// 总大小（字节，经过 min_size 过滤后）
    total_size_bytes: u64,
    /// 按扩展名统计的映射表
    by_extension_map: HashMap<String, ExtensionStat>,
    /// Top N 大文件列表（按大小降序），内部使用向量维护，上限为 TOP_FILES_LIMIT
    top_files: Vec<FileEntry>,
    /// 陈旧文件列表
    stale_files: Vec<FileEntry>,
    /// 当前时间，用于陈旧文件判断
    now: SystemTime,
}

/// 用于并行扫描的原子计数器
struct AtomicCounters {
    files: AtomicU64,
    dirs: AtomicU64,
    size: AtomicU64,
    /// Top N 大文件限制
    limit: usize,
    /// Top N 大文件堆（最小堆，使用 Reverse 包装 FileEntry 实现）
    top_files: Arc<Mutex<BinaryHeap<Reverse<FileEntry>>>>,
    /// 扩展名统计映射：扩展名 -> (文件数, 总大小)
    extensions: Arc<Mutex<HashMap<String, (u64, u64)>>>,
    /// 陈旧文件列表
    stale_files: Arc<Mutex<Vec<FileEntry>>>,
}

impl AtomicCounters {
    fn new(limit: usize) -> Self {
        Self {
            files: AtomicU64::new(0),
            dirs: AtomicU64::new(0),
            size: AtomicU64::new(0),
            limit,
            top_files: Arc::new(Mutex::new(BinaryHeap::with_capacity(limit))),
            extensions: Arc::new(Mutex::new(HashMap::new())),
            stale_files: Arc::new(Mutex::new(Vec::new())),
        }
    }
    
    fn add_file_with_extension(&self, extension: Option<String>, size: u64) {
        let ext = extension.unwrap_or_else(|| "no_extension".to_string());
        let mut map = self.extensions.lock().unwrap();
        let entry = map.entry(ext).or_insert((0, 0));
        entry.0 += 1;
        entry.1 += size;
    }

    fn add_file_to_top_list(&self, path: PathBuf, size: u64, last_modified: Option<SystemTime>, extension: Option<String>) {
        // 过滤大小为0的文件
        if size == 0 {
            return;
        }
        let entry = FileEntry {
            path,
            size_bytes: size,
            last_modified,
            extension,
        };
        let mut heap = self.top_files.lock().unwrap();
        if heap.len() < self.limit {
            heap.push(Reverse(entry));
        } else {
            // 堆已满，比较新文件与堆顶（当前堆中最小的文件）
            if let Some(top) = heap.peek() {
                if entry.size_bytes > top.0.size_bytes {
                    heap.pop(); // 移除堆顶最小文件
                    heap.push(Reverse(entry));
                }
            }
        }
    }

    fn extensions_to_vec(&self) -> Vec<ExtensionStat> {
        let map = self.extensions.lock().unwrap();
        let mut vec: Vec<ExtensionStat> = map
            .iter()
            .map(|(ext, &(file_count, total_size_bytes))| ExtensionStat {
                extension: ext.clone(),
                file_count,
                total_size_bytes,
            })
            .collect();
        // 按总大小降序排序，如果大小相同则按文件数降序
        vec.sort_by(|a, b| {
            b.total_size_bytes
                .cmp(&a.total_size_bytes)
                .then_with(|| b.file_count.cmp(&a.file_count))
        });
        vec
    }

    fn top_files_to_vec(&self) -> Vec<FileEntry> {
        let heap = self.top_files.lock().unwrap();
        // 将堆转换为向量，并反转顺序（从大到小）
        let mut vec: Vec<FileEntry> = heap.iter().map(|rev| rev.0.clone()).collect();
        // 由于堆是最小堆，堆顶是最小元素，但iter顺序不确定，需要按大小降序排序
        vec.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes).then_with(|| b.path.cmp(&a.path)));
        vec
    }

    fn add_stale_file(&self, path: PathBuf, size: u64, last_modified: Option<SystemTime>, extension: Option<String>) {
        let entry = FileEntry {
            path,
            size_bytes: size,
            last_modified,
            extension,
        };
        let mut vec = self.stale_files.lock().unwrap();
        vec.push(entry);
    }

    fn stale_files_to_vec(&self) -> Vec<FileEntry> {
        let vec = self.stale_files.lock().unwrap();
        vec.clone()
    }

    fn to_summary(&self, root_path: PathBuf, elapsed_seconds: f64) -> ScanSummary {
        ScanSummary {
            root_path,
            total_files: self.files.load(Ordering::SeqCst),
            total_dirs: self.dirs.load(Ordering::SeqCst),
            total_size_bytes: self.size.load(Ordering::SeqCst),
            elapsed_seconds,
        }
    }
}
/// 核心扫描引擎
pub struct Scanner;

impl Scanner {
    /// 创建一个新的扫描器实例
    pub fn new() -> Self {
        Self
    }
    
    /// 同步扫描指定目录，返回扫描结果
    /// 
    /// 这是一个简化的实现，仅统计总文件数和总大小。
    /// 后续迭代会添加多线程、文件类型分析、Top N 文件等功能。
    pub fn scan_sync(&self, request: &ScanRequest) -> std::io::Result<ScanResult> {
        let start_time = SystemTime::now();
        
        // 验证根目录存在且可访问
        if !request.root_path.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("根目录不存在: {}", request.root_path.display()),
            ));
        }
        
        // 配置 rayon 线程池
        let threads = request.threads.unwrap_or(0); // 0 表示使用默认值
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(if threads > 0 { threads as usize } else { rayon::current_num_threads() })
            .build()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        
        let limit = request.limit.unwrap_or(20);
        let counters = AtomicCounters::new(limit);

        // 预编译排除规则（glob 模式）；非法模式将被忽略
        let exclude_patterns: Vec<Pattern> = request
            .exclude_patterns
            .iter()
            .filter_map(|p| Pattern::new(p).ok())
            .collect();
        
        // 使用线程池执行并行遍历
        pool.scope(|scope| {
            Self::parallel_walk_dir(scope, request.root_path.clone(), &counters, request, &exclude_patterns);
        });
        
        let elapsed = start_time.elapsed().unwrap_or_default();
        
        Ok(ScanResult {
            summary: counters.to_summary(request.root_path.clone(), elapsed.as_secs_f64()),
            top_files: counters.top_files_to_vec(),
            by_extension: counters.extensions_to_vec(),
            stale_files: counters.stale_files_to_vec(),
        })
    }
    
    fn walk_dir(
        dir: &Path,
        total_files: &mut u64,
        total_dirs: &mut u64,
        total_size: &mut u64,
    ) -> std::io::Result<()> {
        if dir.is_dir() {
            *total_dirs += 1;
            for entry in std::fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                
                if path.is_dir() {
                    Self::walk_dir(&path, total_files, total_dirs, total_size)?;
                } else {
                    *total_files += 1;
                    *total_size += entry.metadata().map(|m| m.len()).unwrap_or(0);
                }
            }
        }
        Ok(())
    }
    
    /// 并行遍历目录树（内部实现）
    fn parallel_walk_dir<'scope>(
        scope: &rayon::Scope<'scope>,
        dir: PathBuf,
        counters: &'scope AtomicCounters,
        request: &'scope ScanRequest,
        exclude_patterns: &'scope [Pattern],
    ) {
        // 检查是否为目录
        if !dir.is_dir() {
            return;
        }
        
        // 增加目录计数
        counters.dirs.fetch_add(1, Ordering::SeqCst);
        
        // 读取目录条目，如果失败则跳过（无法访问的目录）
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => return,
        };
        
        // 收集子目录和文件
        let mut subdirs = Vec::new();
        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            let path = entry.path();
            
            if path.is_dir() {
                // 目录匹配排除规则则跳过整棵子树
                if is_excluded(&path, exclude_patterns) {
                    continue;
                }
                subdirs.push(path);
            } else {
                let metadata = entry.metadata();
                let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);

                // 文件匹配排除规则则跳过
                if is_excluded(&path, exclude_patterns) {
                    continue;
                }

                // 应用 min-size 过滤
                if let Some(min_size) = request.min_size {
                    if size < min_size {
                        continue;
                    }
                }
                
                // 增加文件计数和大小
                counters.files.fetch_add(1, Ordering::SeqCst);
                counters.size.fetch_add(size, Ordering::SeqCst);
                // 提取扩展名
                let extension = path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|s| s.to_lowercase());
                counters.add_file_with_extension(extension.clone(), size);
                // 添加到 Top N 大文件列表
                let last_modified = metadata.ok().and_then(|m| m.modified().ok());
                counters.add_file_to_top_list(path.clone(), size, last_modified, extension.clone());
                
                // 检查是否为陈旧文件
                if let Some(stale_days) = request.stale_days {
                    if let Some(last_modified) = last_modified {
                        if let Ok(duration) = SystemTime::now().duration_since(last_modified) {
                            if duration.as_secs() >= (stale_days as u64) * 24 * 60 * 60 {
                                counters.add_stale_file(path, size, Some(last_modified), extension);
                            }
                        }
                    }
                }
            }
        }
        
        // 为每个子目录生成并行任务
        for subdir in subdirs {
            let counters = counters; // 捕获引用
            let request = request; // 捕获引用
            scope.spawn(move |scope| {
                Self::parallel_walk_dir(scope, subdir, counters, request, exclude_patterns);
            });
        }
    }
}

/// 判断路径是否匹配任一排除模式
fn is_excluded(path: &Path, patterns: &[Pattern]) -> bool {
    // 使用绝对或相对路径进行匹配，glob::Pattern 支持路径分隔符
    for pat in patterns {
        if pat.matches_path(path) {
            return true;
        }
    }
    false
}

/// 便捷函数：快速扫描指定路径
pub fn scan_path<P: Into<PathBuf>>(path: P) -> std::io::Result<ScanResult> {
    let request = ScanRequest::new(path);
    let scanner = Scanner::new();
    scanner.scan_sync(&request)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use tempfile::tempdir;
    
    #[test]
    fn test_scan_request_new() {
        let req = ScanRequest::new("/tmp");
        assert_eq!(req.root_path, PathBuf::from("/tmp"));
        assert!(req.threads.is_none());
        assert!(req.min_size.is_none());
        assert!(req.exclude_patterns.is_empty());
        assert!(req.stale_days.is_none());
    }
    
    #[test]
    fn test_scan_sync_empty_dir() {
        let dir = tempdir().unwrap();
        let request = ScanRequest::new(dir.path());
        let scanner = Scanner::new();
        let result = scanner.scan_sync(&request).unwrap();
        
        assert_eq!(result.summary.total_files, 0);
        assert_eq!(result.summary.total_dirs, 1); // 目录本身
        assert_eq!(result.summary.total_size_bytes, 0);
        assert!(result.summary.elapsed_seconds >= 0.0);
    }
    
    #[test]
    fn test_scan_sync_with_files() {
        let dir = tempdir().unwrap();
        
        // 创建几个文件
        let file1_path = dir.path().join("file1.txt");
        let mut file1 = File::create(&file1_path).unwrap();
        file1.write_all(b"Hello").unwrap();
        
        let file2_path = dir.path().join("file2.txt");
        let mut file2 = File::create(&file2_path).unwrap();
        file2.write_all(b"World!").unwrap();
        
        // 创建一个子目录
        let subdir = dir.path().join("subdir");
        fs::create_dir(&subdir).unwrap();
        
        let request = ScanRequest::new(dir.path());
        let scanner = Scanner::new();
        let result = scanner.scan_sync(&request).unwrap();
        
        assert_eq!(result.summary.total_files, 2);
        assert_eq!(result.summary.total_dirs, 2); // 根目录 + subdir
        assert_eq!(result.summary.total_size_bytes, 11); // 5 + 6
    }
    
    #[test]
    fn test_scan_path_convenience() {
        let dir = tempdir().unwrap();
        let result = scan_path(dir.path()).unwrap();
        assert_eq!(result.summary.total_files, 0);
    }
    
    #[test]
    fn test_scan_sync_multithreaded() {
        // 创建一个包含多个文件和子目录的测试目录树
        let dir = tempdir().unwrap();
        let root = dir.path();
        
        // 创建一些文件
        for i in 0..10 {
            let file_path = root.join(format!("file{}.txt", i));
            let mut file = File::create(&file_path).unwrap();
            file.write_all(format!("Content {}", i).as_bytes()).unwrap();
        }
        
        // 创建一些子目录
        for i in 0..5 {
            let subdir = root.join(format!("dir{}", i));
            fs::create_dir(&subdir).unwrap();
            
            // 在子目录中创建文件
            for j in 0..3 {
                let file_path = subdir.join(format!("subfile{}.txt", j));
                let mut file = File::create(&file_path).unwrap();
                file.write_all(format!("Sub content {}-{}", i, j).as_bytes()).unwrap();
            }
        }
        
        // 使用单线程扫描（threads = 1）作为基准
        let mut request1 = ScanRequest::new(root);
        request1.threads = Some(1);
        let scanner = Scanner::new();
        let result1 = scanner.scan_sync(&request1).unwrap();
        
        // 使用多线程扫描（默认线程数）
        let mut request2 = ScanRequest::new(root);
        request2.threads = None; // 使用默认（逻辑核心数）
        let result2 = scanner.scan_sync(&request2).unwrap();
        
        // 使用明确指定线程数（例如 2）
        let mut request3 = ScanRequest::new(root);
        request3.threads = Some(2);
        let result3 = scanner.scan_sync(&request3).unwrap();
        
        // 验证所有扫描结果的总计一致
        assert_eq!(result1.summary.total_files, result2.summary.total_files);
        assert_eq!(result1.summary.total_dirs, result2.summary.total_dirs);
        assert_eq!(result1.summary.total_size_bytes, result2.summary.total_size_bytes);
        
        assert_eq!(result1.summary.total_files, result3.summary.total_files);
        assert_eq!(result1.summary.total_dirs, result3.summary.total_dirs);
        assert_eq!(result1.summary.total_size_bytes, result3.summary.total_size_bytes);
        
        // 验证具体数值（根据创建的文件和目录）
        // 总文件数：10个根目录文件 + 5个子目录 * 3个文件 = 25
        // 总目录数：根目录 + 5个子目录 = 6
        assert_eq!(result1.summary.total_files, 25);
        assert_eq!(result1.summary.total_dirs, 6);
        
        // 文件大小：每个文件内容长度不同，但我们可以验证总大小 > 0
        assert!(result1.summary.total_size_bytes > 0);
    }

    #[test]
    fn test_extension_statistics() {
        // 创建测试目录
        let dir = tempdir().unwrap();
        let root = dir.path();
        
        // 创建不同扩展名的文件
        let extensions = ["txt", "log", "mp4", "", "TXT", "Log"]; // 包含空扩展名和大写
        for (i, &ext) in extensions.iter().enumerate() {
            let filename = if ext.is_empty() {
                format!("file{}", i)
            } else {
                format!("file{}.{}", i, ext)
            };
            let file_path = root.join(filename);
            let mut file = File::create(&file_path).unwrap();
            file.write_all(format!("Content {}", i).as_bytes()).unwrap();
        }
        
        // 执行扫描
        let request = ScanRequest::new(root);
        let scanner = Scanner::new();
        let result = scanner.scan_sync(&request).unwrap();
        
        // 验证扩展名统计
        assert_eq!(result.by_extension.len(), 4); // txt, log, mp4, no_extension (空扩展名)
        // 查找每个扩展名的统计
        let mut found_txt = false;
        let mut found_log = false;
        let mut found_mp4 = false;
        let mut found_no_ext = false;
        for stat in &result.by_extension {
            match stat.extension.as_str() {
                "txt" => {
                    assert_eq!(stat.file_count, 2); // txt 和 TXT
                    found_txt = true;
                }
                "log" => {
                    assert_eq!(stat.file_count, 2); // log 和 Log
                    found_log = true;
                }
                "mp4" => {
                    assert_eq!(stat.file_count, 1);
                    found_mp4 = true;
                }
                "no_extension" => {
                    assert_eq!(stat.file_count, 1); // 空扩展名文件
                    found_no_ext = true;
                }
                _ => {}
            }
        }
        assert!(found_txt);
        assert!(found_log);
        assert!(found_mp4);
        assert!(found_no_ext);
    }

    #[test]
    fn test_top_n_files() {
        // 创建测试目录
        let dir = tempdir().unwrap();
        let root = dir.path();
        
        // 创建多个不同大小的文件
        let sizes = vec![100, 500, 300, 800, 200, 700, 400, 600, 900, 50];
        for (i, &size) in sizes.iter().enumerate() {
            let file_path = root.join(format!("file{}.txt", i));
            let mut file = File::create(&file_path).unwrap();
            // 写入指定大小的内容
            let content = vec![b'a'; size];
            file.write_all(&content).unwrap();
        }
        
        // 测试默认 limit (20) 应返回所有文件（10个），按大小降序
        let request = ScanRequest::new(root);
        let scanner = Scanner::new();
        let result = scanner.scan_sync(&request).unwrap();
        
        // 验证 top_files 长度等于文件数（因为文件数少于默认 limit 20）
        assert_eq!(result.top_files.len(), 10);
        // 验证顺序是降序
        for i in 0..result.top_files.len() - 1 {
            assert!(
                result.top_files[i].size_bytes >= result.top_files[i + 1].size_bytes,
                "文件未按大小降序排列: {} < {}",
                result.top_files[i].size_bytes,
                result.top_files[i + 1].size_bytes
            );
        }
        // 验证最大文件是 900 字节
        assert_eq!(result.top_files[0].size_bytes, 900);
        // 验证最小文件是 50 字节
        assert_eq!(result.top_files[9].size_bytes, 50);
        
        // 测试指定 limit = 5
        let mut request_limit = ScanRequest::new(root);
        request_limit.limit = Some(5);
        let result_limit = scanner.scan_sync(&request_limit).unwrap();
        assert_eq!(result_limit.top_files.len(), 5);
        // 验证返回的是最大的5个文件
        assert_eq!(result_limit.top_files[0].size_bytes, 900);
        assert_eq!(result_limit.top_files[1].size_bytes, 800);
        assert_eq!(result_limit.top_files[2].size_bytes, 700);
        assert_eq!(result_limit.top_files[3].size_bytes, 600);
        assert_eq!(result_limit.top_files[4].size_bytes, 500);
        
        // 测试 limit = 0（应视为无限制？但实际 limit 应该大于0，这里测试默认行为）
        // 跳过，因为 limit 为 0 时堆容量为 0，可能不存储任何文件。我们假设 limit >= 1
    }

    #[test]
    fn test_top_n_files_with_same_size() {
        // 测试文件大小相同时，按路径排序
        let dir = tempdir().unwrap();
        let root = dir.path();
        
        // 创建三个大小相同的文件，路径不同
        let paths = ["a.txt", "b.txt", "c.txt"];
        for &path in &paths {
            let file_path = root.join(path);
            let mut file = File::create(&file_path).unwrap();
            file.write_all(b"same size").unwrap();
        }
        
        let request = ScanRequest::new(root);
        let scanner = Scanner::new();
        let result = scanner.scan_sync(&request).unwrap();
        
        // 所有文件大小相同，应全部包含（因为 limit 默认20）
        assert_eq!(result.top_files.len(), 3);
        // 验证按路径排序（降序？我们的排序是 size 降序，然后 path 降序）
        // 由于 size 相同，按 path 降序排列，所以 c.txt 应该在 a.txt 之前
        let filenames: Vec<&str> = result.top_files.iter().map(|e| e.path.file_name().unwrap().to_str().unwrap()).collect();
        // 注意：排序是 b.path.cmp(&a.path) 降序，所以是反向字母顺序
        assert_eq!(filenames, vec!["c.txt", "b.txt", "a.txt"]);
    }

    #[test]
    fn test_top_n_files_zero_size() {
        // 测试大小为0的文件应被过滤
        let dir = tempdir().unwrap();
        let root = dir.path();
        
        // 创建一个正常文件和一个大小为0的文件
        let normal_path = root.join("normal.txt");
        let mut normal_file = File::create(&normal_path).unwrap();
        normal_file.write_all(b"content").unwrap();
        
        let zero_path = root.join("zero.txt");
        File::create(&zero_path).unwrap(); // 空文件，大小为0
        
        let request = ScanRequest::new(root);
        let scanner = Scanner::new();
        let result = scanner.scan_sync(&request).unwrap();
        
        // 总文件数应该是2
        assert_eq!(result.summary.total_files, 2);
        // 但 top_files 应只包含非零文件（1个）
        assert_eq!(result.top_files.len(), 1);
        assert_eq!(result.top_files[0].size_bytes, 7);
    }

    #[test]
    fn test_exclude_file_pattern() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        // 创建应被排除的 .log 文件与一个正常文件
        let log_path = root.join("skip.log");
        let mut log_file = File::create(&log_path).unwrap();
        log_file.write_all(b"log content").unwrap();

        let data_path = root.join("data.bin");
        let mut data_file = File::create(&data_path).unwrap();
        data_file.write_all(&vec![b'a'; 1024]).unwrap();

        // 设置排除模式：排除所有 .log 文件
        let mut request = ScanRequest::new(root);
        request.exclude_patterns = vec!["**/*.log".to_string(), "*.log".to_string()];

        let scanner = Scanner::new();
        let result = scanner.scan_sync(&request).unwrap();

        // 总文件数仍计数为2，但按 min_size 未过滤；我们只断言 top_files 不包含 .log，扩展统计也不包含 log
        assert_eq!(result.summary.total_files, 1, "排除文件后计数应为1");
        // 验证 top_files 中不包含 skip.log
        assert!(result.top_files.iter().all(|e| e.path.file_name().unwrap() != "skip.log"));
        // 验证扩展统计不包含 log
        assert!(result.by_extension.iter().all(|s| s.extension != "log"));
    }

    #[test]
    fn test_exclude_directory_pattern() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        // 创建一个子目录及其中的文件
        let subdir = root.join("sub");
        fs::create_dir(&subdir).unwrap();
        let file_in_sub = subdir.join("a.txt");
        let mut f = File::create(&file_in_sub).unwrap();
        f.write_all(b"hello").unwrap();

        // 根目录下也创建一个文件
        let file_root = root.join("b.txt");
        let mut fr = File::create(&file_root).unwrap();
        fr.write_all(b"world").unwrap();

        // 排除整个子目录
        let mut request = ScanRequest::new(root);
        request.exclude_patterns = vec!["**/sub/**".to_string(), "sub/**".to_string(), "sub".to_string()];

        let scanner = Scanner::new();
        let result = scanner.scan_sync(&request).unwrap();

        // 应只统计根目录文件
        assert_eq!(result.summary.total_files, 1);
        // Top 文件应只包含 b.txt
        assert_eq!(result.top_files.len(), 1);
        assert_eq!(result.top_files[0].path.file_name().unwrap(), "b.txt");
    }
}
