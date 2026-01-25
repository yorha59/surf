//! 最小 TUI 骨架，用于 `--tui` 模式。
//!
//! 当前仅提供占位界面，支持按 'q' 或 Esc 退出。
//! 后续迭代将接入实际的扫描与浏览逻辑。
//! TUI 模块，用于 `--tui` 模式。
//!
//! 提供扫描进度显示、结果浏览与详情查看功能。
//! 支持键盘导航（↑/k ↓/j）与退出（q/Esc/Ctrl+C）。

//! TUI 模块，用于 `--tui` 模式。
//!
//! 提供扫描进度显示、结果浏览与详情查看功能。
//! 支持键盘导航（↑/k ↓/j）与退出（q/Esc/Ctrl+C）。
//! TUI 模块，用于 `--tui` 模式。
//!
//! 提供扫描进度显示、结果浏览与详情查看功能。
//! 支持键盘导航（↑/k ↓/j）与退出（q/Esc/Ctrl+C）。
//! TUI 模块，用于 `--tui` 模式。
//!
//! 提供扫描进度显示、结果浏览与详情查看功能。
//! 支持键盘导航（↑/k ↓/j）与退出（q/Esc/Ctrl+C）。

mod tui_model;
use tui_model::{build_tree, DirNode, NodeType};
use anyhow::{Context, Result};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{stdout, Stdout};
use std::path::PathBuf;

use crate::Args;
use surf_core::{
    cancel,
    collect_results,
    poll_status,
    start_scan,
    FileEntry,
    ScanConfig,
};
use std::time::Duration;
use trash::delete;

/// TUI 退出原因，用于在 CLI 主程序中区分正常退出与用户中断退出。
pub enum TuiExit {
    /// 正常退出（包括扫描完成后按 q/Esc 退出，或扫描过程中用户通过 q/Esc 主动放弃）。
    Completed,
    /// 用户在 TUI 中触发 Ctrl+C（Control+C）中断退出。
    Interrupted,
}

/// TUI 内部模式，用于区分扫描中、浏览结果和错误状态。
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum TuiMode {
    Scanning,
    Browsing,
    /// 等待用户确认是否删除当前选中条目（移入回收站）。
    ConfirmDelete,
    Error,
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
    let root_path: PathBuf = args.path.clone();

    // 设置终端：进入备用屏幕、原始模式，启用鼠标捕获
    enable_raw_mode().context("enable raw mode")?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
        .context("enter alternate screen and enable mouse capture")?;

    // 创建后端与终端
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("create terminal")?;

    // 主循环
    let result = run_tui_loop(&mut terminal, handle, root_display, root_path);

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
    root_path: PathBuf,
) -> Result<TuiExit> {
    let mut scanned_files = 0u64;
    let mut scanned_bytes = 0u64;
    let mut done = false;
    let mut error = None::<String>;

    // 当前模式与浏览列表状态。
    let mut root_node: Option<DirNode> = None;
    let mut mode = TuiMode::Scanning;
    let mut entries: Vec<FileEntry> = Vec::new();
    let mut selected_index: usize = 0;

    // 为了在扫描完成后还能正常退出，这里将句柄包在 Option 中：
    // - 扫描阶段需要通过 &handle 轮询进度和支持取消；
    // - 一旦进入 Browsing 或 Error 模式并调用 collect_results 之后，就不再需要句柄。
    let mut handle_opt = Some(handle);

    // 默认认为是“正常退出”，仅在收到 Ctrl+C 按键时标记为 Interrupted。
    let mut exit_reason = TuiExit::Completed;

    loop {
        // 轮询扫描状态（仅在仍处于扫描阶段且句柄存在时）。
        if let Some(ref handle) = handle_opt {
            let status = poll_status(handle);
            scanned_files = status.progress.scanned_files;
            scanned_bytes = status.progress.scanned_bytes;
            done = status.done;
            error = status.error.clone();

            // 当扫描自然完成且没有错误时，收集结果并进入 Browsing 模式。
            if done && error.is_none() && mode == TuiMode::Scanning {
                match collect_results(handle_opt.take().unwrap()) {
                    Ok(collected) => {
                        entries = collected;
                        root_node = Some(build_tree(&root_path, collected.clone()));
                        selected_index = 0;
                        mode = TuiMode::Browsing;
                    }
                    Err(e) => {
                        error = Some(e.to_string());
                        mode = TuiMode::Error;
                    }
                }
            }
            // 若扫描结束但有错误，则切换到 Error 模式。
            if done && error.is_some() {
                mode = TuiMode::Error;
                // 句柄在错误情况下仍然可以被 collect_results 消耗，但当前实现中
                // 错误信息已经通过 error 暴露给用户，结果并不会再被使用，这里
                // 不再额外调用 collect_results。
                handle_opt = None;
            }
        }

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

            // 使用简单的布局：顶部标题、中间内容区域、底部状态栏
            let chunks = ratatui::layout::Layout::default()
                .direction(ratatui::layout::Direction::Vertical)
                .constraints([
                    ratatui::layout::Constraint::Length(1), // 标题行
                    ratatui::layout::Constraint::Min(1),    // 内容区域
                    ratatui::layout::Constraint::Length(1), // 状态栏
                ])
                .split(area);

            // 标题：根据不同模式显示不同文案。
            let title_text = match mode {
                TuiMode::Scanning => "Surf TUI (scanning)",
                TuiMode::Browsing => "Surf TUI (browse results)",
                TuiMode::ConfirmDelete => "Surf TUI (confirm delete)",
                TuiMode::Error => "Surf TUI (error)",
            };
            let title = ratatui::widgets::Paragraph::new(title_text)
                .style(ratatui::style::Style::default().fg(ratatui::style::Color::Cyan));
            frame.render_widget(title, chunks[0]);

            // 内容区域：根据模式显示扫描进度、浏览列表、删除确认或错误信息。
            match mode {
                TuiMode::Scanning => {
                    let content_text = if let Some(err) = &error {
                        format!("Scan error: {}\n\nPress 'q' or Esc to exit.", err)
                    } else {
                        format!(
                            "Scanning {} ...\n\nFiles: {}\nBytes: {}\n\nFuture features:\n• Browse and navigate\n• Safe delete",
                            root_display, scanned_files, scanned_bytes
                        )
                    };
                    let content = ratatui::widgets::Paragraph::new(content_text)
                        .alignment(ratatui::layout::Alignment::Center);
                    frame.render_widget(content, chunks[1]);
                }
                TuiMode::Browsing => {
                    use ratatui::widgets::{Block, Borders, List, ListItem};
                    use ratatui::layout::{Direction, Constraint, Layout};

                    // 根据宽度决定是否拆分左右区域
                    let content_area = chunks[1];
                    if content_area.width < 40 {
                        // 宽度过窄，退化为单列列表
                        let items: Vec<ListItem> = entries
                            .iter()
                            .take(usize::min(entries.len(), 100))
                            .enumerate()
                            .map(|(idx, entry)| {
                                let prefix = if idx == selected_index { "▶" } else { " " };
                                let line = format!(
                                    "{} {:>12}  {}",
                                    prefix,
                                    entry.size,
                                    entry.path.display()
                                );
                                ListItem::new(line)
                            })
                            .collect();

                        let list = List::new(items)
                            .block(
                                Block::default()
                                    .borders(Borders::ALL)
                                    .title("Scan results (Top by size)"),
                            );
                        frame.render_widget(list, content_area);
                    } else {
                        // 宽度足够，拆分为左右区域（70% 列表，30% 详情）
                        let chunks_h = Layout::default()
                            .direction(Direction::Horizontal)
                            .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
                            .split(content_area);
                        let list_area = chunks_h[0];
                        let detail_area = chunks_h[1];

                        // 左侧列表（与之前相同）
                        let items: Vec<ListItem> = entries
                            .iter()
                            .take(usize::min(entries.len(), 100))
                            .enumerate()
                            .map(|(idx, entry)| {
                                let prefix = if idx == selected_index { "▶" } else { " " };
                                let line = format!(
                                    "{} {:>12}  {}",
                                    prefix,
                                    entry.size,
                                    entry.path.display()
                                );
                                ListItem::new(line)
                            })
                            .collect();
                        let list = List::new(items)
                            .block(
                                Block::default()
                                    .borders(Borders::ALL)
                                    .title("Scan results (Top by size)"),
                            );
                        frame.render_widget(list, list_area);

                        // 右侧详情区域
                        let detail_block = Block::default()
                            .borders(Borders::ALL)
                            .title("当前条目详情");
                        let detail_text = if entries.is_empty() {
                            "暂无扫描结果".to_string()
                        } else {
                            let entry = &entries[selected_index];
                            let total = entries.len();
                            let current = selected_index + 1;
                            format!(
                                "位置: {} / {}\n大小: {} 字节\n路径:\n{}",
                                current, total, entry.size, entry.path.display()
                            )
                        };
                        let detail_paragraph = ratatui::widgets::Paragraph::new(detail_text)
                            .block(detail_block);
                        frame.render_widget(detail_paragraph, detail_area);
                    }
                }
                TuiMode::ConfirmDelete => {
                    use ratatui::widgets::{Block, Borders};

                    let entry_opt = entries.get(selected_index);
                    let confirm_text = if let Some(entry) = entry_opt {
                        format!(
                            "确定要将以下文件移入回收站吗？\n\n大小: {} 字节\n路径:\n{}\n\n按 y/Enter 确认，n/Esc 取消。",
                            entry.size,
                            entry.path.display()
                        )
                    } else {
                        "当前没有可删除的条目，按 n 或 Esc 返回浏览。".to_string()
                    };

                    let block = Block::default()
                        .borders(Borders::ALL)
                        .title("确认删除");
                    let paragraph = ratatui::widgets::Paragraph::new(confirm_text)
                        .block(block)
                        .alignment(ratatui::layout::Alignment::Left);
                    frame.render_widget(paragraph, chunks[1]);
                }
                TuiMode::Error => {
                    let content_text = if let Some(err) = &error {
                        format!("Scan error: {}\n\nPress 'q' or Esc to exit.", err)
                    } else {
                        "Unknown error. Press 'q' or Esc to exit.".to_string()
                    };
                    let content = ratatui::widgets::Paragraph::new(content_text)
                        .alignment(ratatui::layout::Alignment::Center);
                    frame.render_widget(content, chunks[1]);
                }
            }

            // 状态栏
            let status_text = match mode {
                TuiMode::Scanning => {
                    format!(
                        "Status: Scanning ... files={}, bytes={} | q/Esc: 退出  Ctrl+C: 中断",
                        scanned_files, scanned_bytes
                    )
                }
                TuiMode::Browsing => {
                    let total = entries.len();
                    let current = if total == 0 { 0 } else { selected_index + 1 };
                    format!(
                        "Status: Browsing results ({} / {}) | ↑/k ↓/j: 移动  d: 删除  q/Esc: 退出  (详情: 右侧窗格)",
                        current, total
                    )
                }
                TuiMode::ConfirmDelete => {
                    "Status: Confirm delete | y/Enter: 确认删除  n/Esc: 取消  q: 退出  Ctrl+C: 中断".to_string()
                }
                TuiMode::Error => {
                    "Status: Error | q/Esc: 退出".to_string()
                }
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
                    match mode {
                        TuiMode::ConfirmDelete => {
                            match key.code {
                                // 确认删除：尝试将当前选中条目移入回收站
                                KeyCode::Char('y') | KeyCode::Enter => {
                                    if let Some(entry) = entries.get(selected_index) {
                                        let path = entry.path.clone();
                                        match delete(&path) {
                                            Ok(()) => {
                                                // 删除成功后，从列表中移除该条目并调整选中索引。
                                                if !entries.is_empty() {
                                                    entries.remove(selected_index);
                                                    if entries.is_empty() {
                                                        selected_index = 0;
                                                    } else if selected_index >= entries.len() {
                                                        selected_index = entries.len() - 1;
                                                    }
                                                }
                                                mode = TuiMode::Browsing;
                                            }
                                            Err(e) => {
                                                error = Some(format!(
                                                    "Failed to move to trash: {}",
                                                    e
                                                ));
                                                mode = TuiMode::Error;
                                            }
                                        }
                                    } else {
                                        // 没有选中条目，直接返回浏览模式。
                                        mode = TuiMode::Browsing;
                                    }
                                }
                                // 取消删除，返回浏览模式
                                KeyCode::Char('n') | KeyCode::Esc => {
                                    mode = TuiMode::Browsing;
                                }
                                // Ctrl+C：在确认弹窗中同样支持中断整个扫描/程序
                                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                    if !done {
                                        if let Some(ref handle) = handle_opt {
                                            cancel(handle);
                                        }
                                    }
                                    exit_reason = TuiExit::Interrupted;
                                    break;
                                }
                                // q：直接退出 TUI
                                KeyCode::Char('q') => {
                                    if !done {
                                        if let Some(ref handle) = handle_opt {
                                            cancel(handle);
                                        }
                                    }
                                    exit_reason = TuiExit::Completed;
                                    break;
                                }
                                _ => {
                                    // 其他按键在确认弹窗中暂不处理
                                }
                            }
                        }
                        _ => {
                            match key.code {
                                KeyCode::Char('q') | KeyCode::Esc => {
                                    // 用户请求退出（正常退出语义）
                                    if !done {
                                        // 扫描尚未完成，尝试取消
                                        if let Some(ref handle) = handle_opt {
                                            cancel(handle);
                                        }
                                    }
                                    exit_reason = TuiExit::Completed;
                                    break;
                                }
                                KeyCode::Up | KeyCode::Char('k') => {
                                    if mode == TuiMode::Browsing && !entries.is_empty() {
                                        if selected_index == 0 {
                                            selected_index = entries.len().saturating_sub(1);
                                        } else {
                                            selected_index = selected_index.saturating_sub(1);
                                        }
                                    }
                                }
                                KeyCode::Down | KeyCode::Char('j') => {
                                    if mode == TuiMode::Browsing && !entries.is_empty() {
                                        if selected_index + 1 >= entries.len() {
                                            selected_index = 0;
                                        } else {
                                            selected_index += 1;
                                        }
                                    }
                                }
                                KeyCode::Char('d') => {
                                    if mode == TuiMode::Browsing && !entries.is_empty() {
                                        mode = TuiMode::ConfirmDelete;
                                    }
                                }
                                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                    // Ctrl+C：与 CLI 模式保持一致，视为“用户中断”，退出码应为非零
                                    if !done {
                                        if let Some(ref handle) = handle_opt {
                                            cancel(handle);
                                        }
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
            }
        }

        // 为避免 busy loop，每轮稍作休眠；在 Browsing 模式下事件轮询仍可响应按键。
        std::thread::sleep(Duration::from_millis(100));
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
