use std::path::PathBuf;

use clap::Parser;
use surf_core::{ScanConfig, start_scan, poll_status, collect_results, cancel};
use serde::Serialize;
use std::time::Duration;
use indicatif::{ProgressBar, ProgressStyle};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use ctrlc;

mod tui;

/// Surf CLI: disk usage scanner (minimal initial implementation).
#[derive(Parser, Debug)]
#[command(name = "surf", version, about = "Disk space analyzer (Surf)")]
pub struct Args {
    /// Path to scan
    #[arg(long = "path", short = 'p', default_value = ".")]
    path: PathBuf,

    /// Minimum file size to include (e.g. 100MB, 1G). Defaults to 0.
    #[arg(long = "min-size", short = 'm', default_value = "0")]
    min_size: String,

    /// Maximum number of entries to display (default: 20)
    #[arg(long = "limit", short = 'n', default_value_t = 20)]
    limit: usize,

    /// Output results as JSON instead of a table
    #[arg(long = "json")]
    json: bool,

    /// Run interactive terminal UI (TUI) instead of one-off scan.
    ///
    /// 当前仅作为占位入口，TUI 仍在开发中；指定该参数时会给出明确错误提示并退出非零状态码。
    #[arg(long = "tui")]
    tui: bool,

    /// Number of threads to use for scanning (>= 1). Defaults to logical CPU count.
    #[arg(
        long = "threads",
        short = 't',
        value_parser = parse_threads,
        default_value_t = num_cpus::get(),
    )]
    threads: usize,

    /// Start JSON-RPC service mode instead of one-off scan.
    ///
    /// 对应 PRD 3.2.2 / 参数表中的 `--service` / `-s`，用于启动服务模式。
    /// 当前实现仅完成参数解析和错误信息提示，具体服务逻辑将在 dev-service-api
    /// 工作区中实现后再接入。
    #[arg(long = "service", short = 's')]
    service: bool,

    /// Service listen port (default: 1234).
    ///
    /// 对应 PRD 中的 `--port`，仅在 `--service` 模式下生效。
    #[arg(long = "port", default_value_t = 1234)]
    port: u16,

    /// Service listen host (default: 127.0.0.1).
    ///
    /// 对应 PRD 中的 `--host`，仅在 `--service` 模式下生效。
    #[arg(long = "host", default_value = "127.0.0.1")]
    host: String,
}

/// JSON 输出条目，对应 PRD 3.4 / 9.1.3 CLI-ONEOFF-003 的结构化输出要求。
#[derive(Serialize)]
struct JsonEntry {
    path: String,
    size: u64,
    is_dir: bool,
}

/// JSON 输出的根对象，包含扫描根路径和条目数组。
#[derive(Serialize)]
struct JsonOutput {
    root: String,
    entries: Vec<JsonEntry>,
}

pub(crate) fn parse_size(input: &str) -> Result<u64, String> {
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

fn parse_threads(input: &str) -> Result<usize, String> {
    let value: usize = input
        .parse()
        .map_err(|_| format!("invalid value for --threads: {}", input))?;

    if value == 0 {
        Err("--threads must be at least 1".to_string())
    } else {
        Ok(value)
    }
}

/// 启动 surf-service 子进程，传递给定的 host 和 port 参数。
///
/// 返回子进程的退出状态（如果成功启动并等待完成）。
/// 若启动失败（例如找不到 surf-service 二进制），则返回 `Err`。
fn run_service(host: &str, port: u16) -> std::io::Result<std::process::ExitStatus> {
    std::process::Command::new("surf-service")
        .arg("--host")
        .arg(host)
        .arg("--port")
        .arg(port.to_string())
        .status()
}

fn main() {
    let args = Args::parse();

    // 服务模式：启动 surf-service 子进程。
    if args.service {
        eprintln!("Starting surf-service on {}:{} ...", args.host, args.port);
        match run_service(&args.host, args.port) {
            Ok(status) => {
                if status.success() {
                    eprintln!("surf-service exited successfully.");
                    std::process::exit(0);
                } else {
                    eprintln!("surf-service exited with non-zero status: {}", status);
                    std::process::exit(status.code().unwrap_or(1));
                }
            }
            Err(e) => {
                eprintln!("failed to start surf-service: {}", e);
                std::process::exit(1);
            }
        }
    }

    // TUI 模式：进入全屏终端 UI。当前已实现最小扫描进度视图，并在本分支中
    // 根据 TUI 退出原因设置不同的退出码（正常退出 0，Ctrl+C 中断为 130）。
    if args.tui {
        // 与架构设计 4.4.1 中的约定保持一致：TUI 模式不应与 --json / --limit 组合使用，
        // 因为 TUI 自身负责结果展示与分页。
        if args.json {
            eprintln!("--json cannot be used together with --tui; TUI mode manages its own output.");
            std::process::exit(1);
        }

        if args.limit != 20 {
            eprintln!("--limit is not supported in --tui mode; TUI will control list pagination.");
            std::process::exit(1);
        }

        // 调用 TUI：根据退出原因区分正常退出与用户中断退出。
        match tui::run_tui(&args) {
            Ok(tui::TuiExit::Completed) => {
                // 正常退出（扫描完成后退出，或用户在扫描过程中按 q/Esc 放弃）。
                std::process::exit(0);
            }
            Ok(tui::TuiExit::Interrupted) => {
                // 用户在 TUI 中触发 Ctrl+C（Control+C），与 CLI 单次模式保持一致，使用 130 退出码。
                eprintln!("Scan interrupted by user (Ctrl+C) in TUI mode");
                std::process::exit(130);
            }
            Err(e) => {
                eprintln!("TUI error: {}", e);
                std::process::exit(1);
            }
        }
    }

    // 创建中断标志，用于响应 Ctrl+C
    let interrupted = Arc::new(AtomicBool::new(false));
    let interrupted_clone = interrupted.clone();
    if let Err(e) = ctrlc::set_handler(move || {
        interrupted_clone.store(true, Ordering::SeqCst);
    }) {
        eprintln!("Failed to set Ctrl+C handler: {}", e);
        std::process::exit(1);
    }

    let min_size = match parse_size(&args.min_size) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Error parsing --min-size: {}", e);
            std::process::exit(1);
        }
    };

    // 将线程数参数传递给核心扫描器，由其控制实际并发度。
    // 创建 spinner 进度指示器，输出到 stderr
    let pb = ProgressBar::new_spinner();
    pb.set_draw_target(indicatif::ProgressDrawTarget::stderr());
    pb.enable_steady_tick(Duration::from_millis(100));
    pb.set_message(format!("Scanning {} ...", args.path.display()));
    pb.set_style(ProgressStyle::default_spinner());

    // 构造扫描配置
    let config = ScanConfig {
        root: args.path.clone(),
        min_size,
        threads: args.threads,
    };
    
    // 启动异步扫描任务
    // 检查是否已经收到中断信号
    if interrupted.load(Ordering::SeqCst) {
        pb.finish_and_clear();
        eprintln!("Scan interrupted by user (Ctrl+C)");
        std::process::exit(130);
    }

    let handle = match start_scan(config) {
        Ok(v) => {
            v
        }
        Err(e) => {
            pb.finish_and_clear();
            eprintln!("Failed to scan {}: {}", args.path.display(), e);
            std::process::exit(1);
        }
    };
    
    // 轮询扫描进度，更新进度条消息
    loop {
        // 检查用户是否按下了 Ctrl+C
        if interrupted.load(Ordering::SeqCst) {
            pb.finish_and_clear();
            cancel(&handle);
            eprintln!("Scan interrupted by user (Ctrl+C)");
            std::process::exit(130);
        }

        let status = poll_status(&handle);
        pb.set_message(format!(
            "Scanning {} ... files={}, bytes={}",
            args.path.display(),
            status.progress.scanned_files,
            status.progress.scanned_bytes
        ));
        
        if status.done {
            break;
        }
        
        std::thread::sleep(Duration::from_millis(100));
    }
    
    // 获取最终结果
    let entries = match collect_results(handle) {
        Ok(v) => {
            pb.finish_and_clear();
            v
        }
        Err(e) => {
            pb.finish_and_clear();
            eprintln!("Failed to scan {}: {}", args.path.display(), e);
            std::process::exit(1);
        }
    };

    if args.json {
        // JSON output: full list (respecting limit).
        let to_emit = &entries[..entries.len().min(args.limit)];
        // 构建符合 PRD 3.4 / 9.1.3 要求的 JSON 输出结构
        let json_output = JsonOutput {
            root: args.path.display().to_string(),
            entries: to_emit
                .iter()
                .map(|entry| JsonEntry {
                    path: entry.path.display().to_string(),
                    size: entry.size,
                    is_dir: false, // 当前扫描器仅返回文件，目录条目暂不支持
                })
                .collect(),
        };
        if let Err(e) = serde_json::to_writer_pretty(std::io::stdout(), &json_output) {
            eprintln!("Failed to write JSON output: {}", e);
            std::process::exit(1);
        }
    } else {
        // Simple table output: size and path.
        println!("{:<12}  {}", "SIZE(BYTES)", "PATH");
        println!("{:-<12}  {:-<}", "", "");

        for entry in entries.into_iter().take(args.limit) {
            println!("{:<12}  {}", entry.size, entry.path.display());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_size, Args};
    use clap::Parser;

    #[test]
    fn parse_size_handles_plain_bytes_without_unit() {
        assert_eq!(parse_size("0").unwrap(), 0);
        assert_eq!(parse_size("42").unwrap(), 42);
    }

    #[test]
    fn parse_size_supports_kilobytes_case_insensitively() {
        assert_eq!(parse_size("1KB").unwrap(), 1024);
        assert_eq!(parse_size("1kb").unwrap(), 1024);
    }

    #[test]
    fn parse_size_supports_megabytes_case_insensitively() {
        assert_eq!(parse_size("2MB").unwrap(), 2 * 1024 * 1024);
        assert_eq!(parse_size("2mb").unwrap(), 2 * 1024 * 1024);
    }

    #[test]
    fn parse_size_supports_gigabytes_case_insensitively() {
        assert_eq!(parse_size("3GB").unwrap(), 3 * 1024 * 1024 * 1024);
        assert_eq!(parse_size("3gb").unwrap(), 3 * 1024 * 1024 * 1024);
    }

    #[test]
    fn parse_size_trims_whitespace_and_internal_spaces_before_unit() {
        assert_eq!(parse_size(" 10  MB").unwrap(), 10 * 1024 * 1024);
        assert_eq!(parse_size("\t5 kb \n").unwrap(), 5 * 1024);
    }

    #[test]
    fn parse_size_treats_empty_or_whitespace_only_as_zero() {
        assert_eq!(parse_size("").unwrap(), 0);
        assert_eq!(parse_size("   ").unwrap(), 0);
    }

    #[test]
    fn parse_size_rejects_non_numeric_prefix() {
        let err = parse_size("abc").expect_err("expected invalid size error");
        assert!(err.contains("invalid size number"));
    }

    #[test]
    fn parse_size_rejects_unknown_unit() {
        let err = parse_size("10XB").expect_err("expected unsupported unit error");
        assert!(err.contains("unsupported size unit"));
    }

    #[test]
    fn threads_default_is_logical_cpu_count() {
        let args = Args::parse_from(["surf"]);
        assert_eq!(args.threads, num_cpus::get());
    }

    #[test]
    fn threads_can_be_overridden_via_short_flag() {
        let args = Args::parse_from(["surf", "-t", "8"]);
        assert_eq!(args.threads, 8);
    }

    #[test]
    fn threads_rejects_zero_value() {
        let res = Args::try_parse_from(["surf", "-t", "0"]);
        assert!(res.is_err());
    }

    #[test]
    fn service_mode_defaults_and_network_options() {
        let args = Args::parse_from(["surf"]);
        assert!(!args.service, "service mode should be disabled by default");
        assert_eq!(args.port, 1234);
        assert_eq!(args.host, "127.0.0.1");
    }

    #[test]
    fn service_mode_flags_can_be_set() {
        let args = Args::parse_from([
            "surf",
            "--service",
            "--port",
            "4321",
            "--host",
            "0.0.0.0",
        ]);

        assert!(args.service);
        assert_eq!(args.port, 4321);
        assert_eq!(args.host, "0.0.0.0");
    }

    #[test]
    fn tui_flag_defaults_to_false() {
        let args = Args::parse_from(["surf"]);
        assert!(!args.tui, "tui mode should be disabled by default");
    }

    #[test]
    fn tui_flag_can_be_enabled() {
        let args = Args::parse_from(["surf", "--tui"]);
        assert!(args.tui);
    }
}
