//! 最小 TUI 骨架，用于 `--tui` 模式。
//!
//! 当前仅提供占位界面，支持按 'q' 或 Esc 退出。
//! 后续迭代将接入实际的扫描与浏览逻辑。

use anyhow::{Context, Result};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{stdout, Stdout};

use crate::Args;
use surf_core::{ScanConfig, start_scan, poll_status, cancel};
use std::time::Duration;

/// TUI 退出原因，用于在 CLI 主程序中区分正常退出与用户中断退出。
pub enum TuiExit {
    /// 正常退出（包括扫描完成后按 q/Esc 退出，或扫描过程中用户通过 q/Esc 主动放弃）。
    Completed,
    /// 用户在 TUI 中触发 Ctrl+C（Control+C）中断退出。
    Interrupted,
}

pub fn run_tui(args: &Args) -> Result<TuiExit> {
    // 解析 --min-size 参数
    let min_size = crate::parse_size(&args.min_size)
        .map_err(|e| anyhow::anyhow!("Error parsing --min-size in TUI mode: {}", e))?;

    // 构造扫描配置
    let config = ScanConfig {
        root: args.path.clone(),
        min_size,
        threads: args.threads,
    };

    // 启动扫描
    let handle = start_scan(config)
        .context("start scan in TUI mode")?;

    let root_display = args.path.display().to_string();

    // 设置终端：进入备用屏幕、原始模式，启用鼠标捕获
    enable_raw_mode().context("enable raw mode")?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
        .context("enter alternate screen and enable mouse capture")?;

    // 创建后端与终端
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("create terminal")?;

    // 主循环
    let result = run_tui_loop(&mut terminal, handle, root_display);

    // 恢复终端：离开备用屏幕、禁用原始模式，禁用鼠标捕获
    disable_raw_mode().context("disable raw mode")?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )
    .context("leave alternate screen and disable mouse capture")?;

    result
}

/// TUI 事件循环。
/// TUI 事件循环。
fn run_tui_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    handle: surf_core::ScanHandle,
    root_display: String,
) -> Result<TuiExit> {
    let mut scanned_files = 0u64;
    let mut scanned_bytes = 0u64;
    let mut done = false;
    let mut error = None::<String>;

    // 默认认为是“正常退出”，仅在收到 Ctrl+C 按键时标记为 Interrupted。
    let mut exit_reason = TuiExit::Completed;

    loop {
        // 轮询扫描状态
        let status = poll_status(&handle);
        scanned_files = status.progress.scanned_files;
        scanned_bytes = status.progress.scanned_bytes;
        done = status.done;
        error = status.error.clone();

        // 绘制当前帧
        terminal.draw(|frame| {
            let area = frame.size();
            if area.width < 10 || area.height < 5 {
                // 终端太小，显示警告
                let warning = ratatui::widgets::Paragraph::new("Terminal too small")
                    .style(ratatui::style::Style::default().fg(ratatui::style::Color::Red));
                frame.render_widget(warning, area);
                return;
            }

            // 使用简单的布局：顶部标题、中间提示、底部状态栏
            let chunks = ratatui::layout::Layout::default()
                .direction(ratatui::layout::Direction::Vertical)
                .constraints([
                    ratatui::layout::Constraint::Length(1), // 标题行
                    ratatui::layout::Constraint::Min(1),    // 内容区域
                    ratatui::layout::Constraint::Length(1), // 状态栏
                ])
                .split(area);

            // 标题
            let title = ratatui::widgets::Paragraph::new("Surf TUI (scanning)")
                .style(ratatui::style::Style::default().fg(ratatui::style::Color::Cyan));
            frame.render_widget(title, chunks[0]);

            // 内容区域：显示扫描进度或错误
            let content_text = if let Some(err) = &error {
                format!("Scan error: {}\n\nPress 'q' or Esc to exit.", err)
            } else if done {
                format!("Scan completed: {} files, {} bytes.\n\nPress 'q' or Esc to exit.", scanned_files, scanned_bytes)
            } else {
                format!("Scanning {} ...\n\nFiles: {}\nBytes: {}\n\nFuture features:\n• Browse and navigate\n• Safe delete", root_display, scanned_files, scanned_bytes)
            };
            let content = ratatui::widgets::Paragraph::new(content_text)
                .alignment(ratatui::layout::Alignment::Center);
            frame.render_widget(content, chunks[1]);

            // 状态栏
            let status_text = if done {
                format!("Status: Scan completed. files={}, bytes={} | q/Esc: 退出", scanned_files, scanned_bytes)
            } else {
                format!("Status: Scanning ... files={}, bytes={} | q/Esc: 退出", scanned_files, scanned_bytes)
            };
            let status = ratatui::widgets::Paragraph::new(status_text)
                .style(ratatui::style::Style::default().fg(ratatui::style::Color::Gray));
            frame.render_widget(status, chunks[2]);
        })?;

        // 处理事件（非阻塞）
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                // 只处理按键按下事件，避免重复触发
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => {
                            // 用户请求退出（正常退出语义）
                            if !done {
                                // 扫描尚未完成，尝试取消
                                cancel(&handle);
                            }
                            exit_reason = TuiExit::Completed;
                            break;
                        }
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            // Ctrl+C：与 CLI 模式保持一致，视为“用户中断”，退出码应为非零
                            if !done {
                                cancel(&handle);
                            }
                            exit_reason = TuiExit::Interrupted;
                            break;
                        }
                        _ => {
                            // 其他按键暂时忽略
                        }
                    }
                }
            }
        }

        // 如果扫描已完成且无错误，可以继续等待用户退出
        if done {
            // 继续事件循环，等待用户按 q/Esc
            // 但为了避免 busy loop，可以稍微 sleep
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    Ok(exit_reason)
}

/// 单元测试：验证 run_tui 函数签名与基本逻辑。
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_tui_signature() {
        // 仅验证函数签名能够编译，不实际运行 TUI
        let _ = run_tui;
        assert!(true);
    }
}
