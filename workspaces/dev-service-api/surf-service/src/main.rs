use clap::Parser;
use tokio::net::TcpListener;

/// Surf 服务进程：提供基于 JSON-RPC 的磁盘扫描服务（骨架实现）。
///
/// 当前版本仅完成：
/// - 命令行参数解析（host/port 等），与 PRD / Architecture 中的约定对齐；
/// - 启动一个 TCP 监听并接受连接，但不解析 JSON-RPC 请求；
/// - 通过日志输出提示该服务仍处于骨架阶段。
#[derive(Parser, Debug)]
#[command(name = "surf-service", version, about = "Surf JSON-RPC service (skeleton)")]
struct Args {
    /// 服务监听地址，默认仅监听本地回环地址 127.0.0.1
    ///
    /// 对应 Architecture.md 4.3.1 中的安全默认值约定。
    #[arg(long = "host", default_value = "127.0.0.1")]
    host: String,

    /// 服务监听端口，默认 1234。
    ///
    /// 与 PRD 4. 命令行参数定义中的 `--port` 对齐。
    #[arg(long = "port", default_value_t = 1234)]
    port: u16,

    /// 最大并发扫描任务数（占位参数）。
    ///
    /// 当前仅解析并记录，实际并发控制将在后续实现任务管理器时生效。
    #[arg(long = "max-concurrent-scans", default_value_t = 4)]
    max_concurrent_scans: usize,

    /// 单个任务在内存中保留的 TTL 秒数（占位参数）。
    ///
    /// 与 Architecture.md 4.3.1 中的 task_ttl_seconds 设计一致，目前尚未真正用于回收逻辑。
    #[arg(long = "task-ttl-seconds", default_value_t = 600)]
    task_ttl_seconds: u64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let addr = format!("{}:{}", args.host, args.port);

    // 绑定 TCP 监听，后续将基于此实现 JSON-RPC 2.0 协议。
    let listener = TcpListener::bind(&addr).await?;

    println!(
        "surf-service listening on {addr} (max_concurrent_scans={max}, task_ttl_seconds={ttl}).\nJSON-RPC methods (Surf.Scan / Surf.Status / Surf.GetResults / Surf.Cancel) are not implemented yet; this binary currently serves as a service skeleton.",
        addr = addr,
        max = args.max_concurrent_scans,
        ttl = args.task_ttl_seconds,
    );

    loop {
        let (socket, peer) = listener.accept().await?;
        println!(
            "Accepted connection from {peer}, closing immediately (service skeleton only)",
            peer = peer
        );

        // 当前版本不处理任何请求，直接丢弃连接。
        drop(socket);
    }
}

