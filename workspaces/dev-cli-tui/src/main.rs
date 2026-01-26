use clap::Parser;
use std::path::PathBuf;
use anyhow::{Context, Result};
use serde_json::json;
use surf_core::{ScanRequest, Scanner};

/// Surf CLI & TUI frontend for disk scanning and analysis
#[derive(Parser, Debug)]
#[command(name = "surf", version = "0.1.0", about = "极速磁盘扫描与分析工具", long_about = None)]
struct Cli {
    /// 扫描起始根目录
    #[arg(short, long, default_value = ".", value_name = "PATH")]
    path: PathBuf,
    
    /// 并发扫描线程数
    #[arg(short, long, value_name = "N")]
    threads: Option<u16>,
    
    /// 过滤最小文件尺寸（支持单位：B, KB, MB, GB）
    #[arg(short, long, value_name = "SIZE")]
    min_size: Option<String>,
    
    /// 结果展示的最大条目数
    #[arg(short, long, default_value = "20", value_name = "N")]
    limit: usize,
    
    /// 启动 JSON-RPC 服务模式
    #[arg(short, long)]
    service: bool,
    
    /// 服务模式监听端口
    #[arg(long, default_value = "1234", value_name = "PORT")]
    port: u16,
    
    /// 服务模式监听地址
    #[arg(long, default_value = "127.0.0.1", value_name = "HOST")]
    host: String,
    
    /// 单次模式下以 JSON 格式输出结果
    #[arg(long)]
    json: bool,
}

/// 解析带单位的大小字符串（例如 "100MB"）为字节数
fn parse_size_string(size_str: &str) -> Result<u64> {
    let size_str = size_str.trim();
    if size_str.is_empty() {
        anyhow::bail!("空大小字符串");
    }
    
    // 分离数字和单位
    let mut split_idx = 0;
    for (i, ch) in size_str.char_indices() {
        if ch.is_ascii_digit() || ch == '.' {
            split_idx = i + ch.len_utf8();
        } else {
            break;
        }
    }
    
    let num_part = &size_str[..split_idx];
    let unit_part = &size_str[split_idx..].trim().to_uppercase();
    
    let num: f64 = num_part.parse().context("解析数字部分失败")?;
    
    let multiplier = match unit_part.as_str() {
        "B" | "" => 1u64,
        "KB" => 1024u64,
        "MB" => 1024u64 * 1024,
        "GB" => 1024u64 * 1024 * 1024,
        "TB" => 1024u64 * 1024 * 1024 * 1024,
        _ => anyhow::bail!("不支持的单位: {}", unit_part),
    };
    
    Ok((num * multiplier as f64) as u64)
}

impl Cli {
    /// 转换为核心扫描请求
    fn to_scan_request(&self) -> Result<ScanRequest> {
        let mut request = ScanRequest::new(&self.path);
        
        if let Some(threads) = self.threads {
            request.threads = Some(threads);
        }
        
        if let Some(ref min_size_str) = self.min_size {
            let bytes = parse_size_string(min_size_str)
                .context("解析 --min-size 参数失败")?;
            request.min_size = Some(bytes);
        }
        
        // 当前版本暂不支持 exclude_patterns 和 stale_days
        // 后续迭代可添加对应参数
        
        Ok(request)
    }
}

/// 格式化字节数为人类可读字符串
fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;
    
    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }
    
    if unit_idx == 0 {
        format!("{} {}", bytes, UNITS[unit_idx])
    } else {
        format!("{:.2} {}", size, UNITS[unit_idx])
    }
}

/// 打印扫描结果表格
fn print_table(result: &surf_core::ScanResult, limit: usize) -> Result<()> {
    let summary = &result.summary;
    
    println!("\n扫描结果摘要:");
    println!("根路径: {}", summary.root_path.display());
    println!("总文件数: {}", summary.total_files);
    println!("总目录数: {}", summary.total_dirs);
    println!("总大小: {}", format_bytes(summary.total_size_bytes));
    println!("扫描耗时: {:.2} 秒", summary.elapsed_seconds);
    
    // 显示 Top N 文件（如果结果中有）
    if !result.top_files.is_empty() {
        println!("\nTop {} 大文件:", limit);
        println!("{:<60} {:<12}", "路径", "大小");
        println!("{}", "-".repeat(80));
        
        for file in result.top_files.iter().take(limit) {
            let size_str = format_bytes(file.size_bytes);
            println!("{:<60} {:<12}", file.path.display(), size_str);
        }
    } else {
        println!("\n（Top N 文件功能尚未在核心扫描引擎中实现）");
    }
    
    // 显示文件类型分布（如果结果中有）
    if !result.by_extension.is_empty() {
        println!("\n文件类型分布:");
        println!("{:<10} {:<10} {:<12}", "扩展名", "文件数", "总大小");
        println!("{}", "-".repeat(40));
        
        for stat in result.by_extension.iter() {
            println!("{:<10} {:<10} {:<12}", 
                stat.extension, 
                stat.file_count, 
                format_bytes(stat.total_size_bytes));
        }
    }
    
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    
    if cli.service {
        // 服务模式：当前版本仅占位
        println!("服务模式尚未实现，将在后续迭代中完成");
        return Ok(());
    }
    
    // 单次扫描模式
    let request = cli.to_scan_request()?;
    
    // 创建进度条
    let pb = indicatif::ProgressBar::new_spinner();
    pb.set_style(
        indicatif::ProgressStyle::default_spinner()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
            .template("{spinner} 扫描中... {msg}")?
    );
    pb.enable_steady_tick(std::time::Duration::from_millis(100));
    
    // 执行扫描
    let scanner = Scanner::new();
    let result = scanner.scan_sync(&request).context("扫描失败")?;
    
    pb.finish_with_message("扫描完成");
    
    // 输出结果
    if cli.json {
        // JSON 输出（简化版，待核心扫描引擎支持 Serialize 后完善）
        let json_value = json!({
            "summary": {
                "root_path": result.summary.root_path.to_string_lossy(),
                "total_files": result.summary.total_files,
                "total_dirs": result.summary.total_dirs,
                "total_size_bytes": result.summary.total_size_bytes,
                "elapsed_seconds": result.summary.elapsed_seconds,
            },
            "top_files": result.top_files.iter().map(|f| json!({
                "path": f.path.to_string_lossy(),
                "size_bytes": f.size_bytes,
                "last_modified": f.last_modified.map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()),
            })).collect::<Vec<_>>(),
            "by_extension": result.by_extension.iter().map(|e| json!({
                "extension": e.extension,
                "file_count": e.file_count,
                "total_size_bytes": e.total_size_bytes,
            })).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&json_value)?);
    } else {
        // 表格输出
        print_table(&result, cli.limit)?;
    }
    
    Ok(())
}
