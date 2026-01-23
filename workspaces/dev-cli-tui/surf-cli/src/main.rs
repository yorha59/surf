use std::path::PathBuf;

use clap::Parser;
use surf_core::scan;

/// Surf CLI: disk usage scanner (minimal initial implementation).
#[derive(Parser, Debug)]
#[command(name = "surf", version, about = "Disk space analyzer (Surf)")]
struct Args {
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

    /// Number of threads to use for scanning (>= 1). Defaults to logical CPU count.
    #[arg(
        long = "threads",
        short = 't',
        value_parser = parse_threads,
        default_value_t = num_cpus::get(),
    )]
    threads: usize,
}

fn parse_size(input: &str) -> Result<u64, String> {
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

fn main() {
    let args = Args::parse();

    let min_size = match parse_size(&args.min_size) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Error parsing --min-size: {}", e);
            std::process::exit(1);
        }
    };

    // 将线程数参数传递给核心扫描器，由其控制实际并发度。
    let entries = match scan(&args.path, min_size, args.threads) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Failed to scan {}: {}", args.path.display(), e);
            std::process::exit(1);
        }
    };

    if args.json {
        // JSON output: full list (respecting limit).
        let to_emit = &entries[..entries.len().min(args.limit)];
        if let Err(e) = serde_json::to_writer_pretty(std::io::stdout(), to_emit) {
            eprintln!("Failed to write JSON output: {}", e);
            std::process::exit(1);
        }
        println!();
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
}
