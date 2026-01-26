//! Surf 核心扫描与分析引擎库
//! 
//! 提供文件系统扫描、目录树构建、统计分析等核心能力。

use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// 扫描请求参数
#[derive(Debug, Clone)]
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
        }
    }
}

/// 扫描进度信息
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone, PartialEq)]
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
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
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

/// 文件类型统计
#[derive(Debug, Clone)]
pub struct ExtensionStat {
    /// 文件扩展名（不含点）
    pub extension: String,
    /// 文件数量
    pub file_count: u64,
    /// 总大小（字节）
    pub total_size_bytes: u64,
}

/// 扫描完整结果
#[derive(Debug, Clone)]
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
        let mut total_files = 0;
        let mut total_dirs = 0;
        let mut total_size = 0;
        
        // 简单的递归遍历（后续会改为多线程）
        Self::walk_dir(&request.root_path, &mut total_files, &mut total_dirs, &mut total_size)?;
        
        let elapsed = start_time.elapsed().unwrap_or_default();
        
        Ok(ScanResult {
            summary: ScanSummary {
                root_path: request.root_path.clone(),
                total_files,
                total_dirs,
                total_size_bytes: total_size,
                elapsed_seconds: elapsed.as_secs_f64(),
            },
            top_files: Vec::new(), // 暂未实现
            by_extension: Vec::new(), // 暂未实现
            stale_files: Vec::new(), // 暂未实现
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
}
