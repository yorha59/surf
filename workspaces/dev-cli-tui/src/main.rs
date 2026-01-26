use clap::Parser;
use std::path::PathBuf;
use anyhow::{Context, Result};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
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
    
    /// 识别陈旧文件的天数阈值（文件最后修改时间超过此天数则视为陈旧）
    #[arg(long, value_name = "DAYS")]
    stale_days: Option<u32>,
    
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

    /// 启动终端用户界面（TUI）模式
    #[arg(long)]
    tui: bool,
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
        
        if let Some(stale_days) = self.stale_days {
            request.stale_days = Some(stale_days);
        }
        
        request.limit = Some(self.limit);
        
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
    
    if cli.tui {
        // TUI 模式
        return run_tui(&cli);
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
    let result = match scanner.scan_sync(&request) {
        Ok(result) => {
            pb.finish_with_message("扫描完成");
            result
        }
        Err(e) => {
            pb.finish_with_message("扫描失败");
            anyhow::bail!("扫描失败: {}", e);
        }
    };
    
    // 输出结果
    if cli.json {
        // JSON 输出（直接序列化 ScanResult）
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        // 表格输出
        print_table(&result, cli.limit)?;
    }
    
    Ok(())
}

/// 运行终端用户界面（TUI）模式
fn run_tui(cli: &Cli) -> Result<()> {
    // 初始化终端
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // 执行扫描（带进度条）
    let request = cli.to_scan_request()?;
    let pb = indicatif::ProgressBar::new_spinner();
    pb.set_style(
        indicatif::ProgressStyle::default_spinner()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
            .template("{spinner} 扫描中... {msg}")?
    );
    pb.enable_steady_tick(std::time::Duration::from_millis(100));

    let scanner = Scanner::new();
    let result = match scanner.scan_sync(&request) {
        Ok(result) => {
            pb.finish_with_message("扫描完成");
            result
        }
        Err(e) => {
            pb.finish_with_message("扫描失败");
            anyhow::bail!("扫描失败: {}", e);
        }
    };

    // 清理进度条
    drop(pb);

    // 运行 TUI 主循环
    let res = run_tui_loop(&mut terminal, &result);

    // 恢复终端状态
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    res
}

/// 目录树节点
#[derive(Debug, Clone)]
struct TreeNode {
    /// 节点名称（目录名或文件名）
    name: String,
    /// 完整路径
    path: PathBuf,
    /// 文件大小（字节），目录为0
    size_bytes: u64,
    /// 是否为目录
    is_dir: bool,
    /// 子节点
    children: Vec<TreeNode>,
    /// 是否展开
    expanded: bool,
}

impl TreeNode {
    /// 创建一个新的目录节点
    fn new_dir(name: String, path: PathBuf) -> Self {
        Self {
            name,
            path,
            size_bytes: 0,
            is_dir: true,
            children: Vec::new(),
            expanded: false,
        }
    }
    
    /// 创建一个新的文件节点
    fn new_file(name: String, path: PathBuf, size_bytes: u64) -> Self {
        Self {
            name,
            path,
            size_bytes,
            is_dir: false,
            children: Vec::new(),
            expanded: false,
        }
    }
    
    /// 从文件路径列表构建目录树（模拟数据用）
    fn from_paths(paths: &[PathBuf]) -> Self {
        let mut root = TreeNode::new_dir("root".to_string(), PathBuf::from("."));
        
        for path in paths {
            let components: Vec<_> = path.components().collect();
            if components.is_empty() {
                continue;
            }
            
            // 使用递归辅助函数插入路径
            Self::insert_path(&mut root, &components, 0);
        }
        
        root
    }
    
    /// 从 FileEntry 列表构建目录树（真实数据用）
    fn from_file_entries(entries: &[surf_core::FileEntry]) -> Self {
        let mut root = TreeNode::new_dir("root".to_string(), PathBuf::from("."));
        
        for entry in entries {
            let path = &entry.path;
            let components: Vec<_> = path.components().collect();
            if components.is_empty() {
                continue;
            }
            
            // 使用递归辅助函数插入路径，并传递文件大小信息
            Self::insert_file_entry(&mut root, &components, 0, entry);
        }
        
        root
    }
    
    /// 递归插入路径组件
    fn insert_path(node: &mut TreeNode, components: &[std::path::Component], depth: usize) {
        if depth >= components.len() {
            return;
        }
        
        let component = &components[depth];
        let component_str = component.as_os_str().to_string_lossy().to_string();
        
        // 查找是否已存在该子节点
        let child_index = node.children.iter().position(|child| child.name == component_str);
        
        if let Some(index) = child_index {
            // 节点已存在，继续递归
            Self::insert_path(&mut node.children[index], components, depth + 1);
        } else {
            // 创建新节点
            let is_dir = depth < components.len() - 1;
            let child_path = node.path.join(&component_str);
            let new_node = if is_dir {
                TreeNode::new_dir(component_str.clone(), child_path.clone())
            } else {
                TreeNode::new_file(component_str.clone(), child_path.clone(), 0)
            };
            
            node.children.push(new_node);
            
            // 继续递归（如果是目录）
            if is_dir {
                let last_index = node.children.len() - 1;
                Self::insert_path(&mut node.children[last_index], components, depth + 1);
            }
        }
    }
    
    /// 递归插入文件条目路径组件，携带文件大小信息
    fn insert_file_entry(node: &mut TreeNode, components: &[std::path::Component], depth: usize, entry: &surf_core::FileEntry) {
        if depth >= components.len() {
            return;
        }
        
        let component = &components[depth];
        let component_str = component.as_os_str().to_string_lossy().to_string();
        
        // 查找是否已存在该子节点
        let child_index = node.children.iter().position(|child| child.name == component_str);
        
        if let Some(index) = child_index {
            // 节点已存在，继续递归
            Self::insert_file_entry(&mut node.children[index], components, depth + 1, entry);
        } else {
            // 创建新节点
            let is_dir = depth < components.len() - 1;
            let child_path = node.path.join(&component_str);
            let new_node = if is_dir {
                TreeNode::new_dir(component_str.clone(), child_path.clone())
            } else {
                // 文件节点：设置实际文件大小
                TreeNode::new_file(component_str.clone(), child_path.clone(), entry.size_bytes)
            };
            
            node.children.push(new_node);
            
            // 继续递归（如果是目录）
            if is_dir {
                let last_index = node.children.len() - 1;
                Self::insert_file_entry(&mut node.children[last_index], components, depth + 1, entry);
            }
        }
    }
    
    /// 将树扁平化为带缩进的节点列表
    fn flatten(&self) -> Vec<FlatNode> {
        let mut result = Vec::new();
        self.flatten_internal(0, &mut result);
        result
    }
    
    /// 内部递归扁平化函数
    fn flatten_internal(&self, depth: usize, result: &mut Vec<FlatNode>) {
        // 添加当前节点（跳过根节点"root"）
        if self.name != "root" {
            result.push(FlatNode {
                name: self.name.clone(),
                path: self.path.clone(),
                size_bytes: self.size_bytes,
                is_dir: self.is_dir,
                depth: depth - 1, // 因为根节点深度为0，实际显示时减1
                expanded: self.expanded,
                has_children: !self.children.is_empty(),
            });
        }
        
        // 如果当前节点是目录且已展开，递归添加子节点
        if self.is_dir && self.expanded {
            for child in &self.children {
                child.flatten_internal(depth + 1, result);
            }
        }
    }
}

/// TUI 状态
struct TuiState {
    /// 目录树根节点
    tree_root: TreeNode,
    /// 当前选中节点在扁平化列表中的索引
    selected_index: usize,
    /// 扁平化的节点列表（用于渲染）
    flat_nodes: Vec<FlatNode>,
    /// 当前选中的文件条目（用于详情显示）
    selected_file: Option<surf_core::FileEntry>,
}

/// 扁平化的树节点，用于渲染
struct FlatNode {
    /// 节点引用（为了简化，存储路径和名称）
    name: String,
    path: PathBuf,
    size_bytes: u64,
    is_dir: bool,
    /// 缩进层级
    depth: usize,
    /// 是否展开（仅对目录有效）
    expanded: bool,
    /// 是否有子节点
    has_children: bool,
}

/// 递归切换树中指定路径节点的展开状态
fn toggle_node_expansion(node: &mut TreeNode, target_path: &PathBuf) -> bool {
    if node.path == *target_path {
        // 找到目标节点，切换展开状态（仅对目录有效）
        if node.is_dir {
            node.expanded = !node.expanded;
        }
        return true;
    }
    
    // 递归搜索子节点
    for child in &mut node.children {
        if toggle_node_expansion(child, target_path) {
            return true;
        }
    }
    
    false
}

/// TUI 主事件循环
fn run_tui_loop(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>, result: &surf_core::ScanResult) -> Result<()> {
    // 使用真实扫描结果构建目录树
    let mut tree_root = TreeNode::from_file_entries(&result.top_files);
    // 默认展开根节点的直接子节点（即第一级目录/文件）
    for child in &mut tree_root.children {
        child.expanded = true;
    }
    
    // 初始化 TUI 状态
    let mut state = TuiState {
        tree_root: tree_root.clone(),
        selected_index: 0,
        flat_nodes: tree_root.flatten(),
        selected_file: None,
    };
    
    loop {
        terminal.draw(|f| {
            let size = f.size();
            
            // 创建布局：左右面板，底部状态栏
            let main_chunks = ratatui::layout::Layout::default()
                .direction(ratatui::layout::Direction::Vertical)
                .constraints([
                    ratatui::layout::Constraint::Min(3), // 主内容区
                    ratatui::layout::Constraint::Length(1), // 状态栏
                ])
                .split(size);
                
            let content_chunks = ratatui::layout::Layout::default()
                .direction(ratatui::layout::Direction::Horizontal)
                .constraints([
                    ratatui::layout::Constraint::Percentage(50), // 左侧文件列表
                    ratatui::layout::Constraint::Percentage(50), // 右侧文件详情
                ])
                .split(main_chunks[0]);
            
            // 左侧：目录树视图（使用 List widget）
            let list_block = ratatui::widgets::Block::default()
                .title(format!("目录树 (Top {} 大文件)", result.top_files.len()))
                .borders(ratatui::widgets::Borders::ALL);
            
            // 创建 List 项，带缩进
            let items: Vec<ratatui::widgets::ListItem> = state.flat_nodes
                .iter()
                .enumerate()
                .map(|(i, node)| {
                    // 根据节点类型和展开状态构造前缀
                    let prefix = if node.is_dir {
                        if node.expanded { "[-] " } else { "[+] " }
                    } else {
                        "    "
                    };
                    // 缩进空格
                    let indent = "  ".repeat(node.depth);
                    // 显示名称和大小
                    let display_name = if node.is_dir {
                        format!("{}{}{}", indent, prefix, node.name)
                    } else {
                        let size_str = format_bytes(node.size_bytes);
                        format!("{}{}{} ({})", indent, prefix, node.name, size_str)
                    };
                    let content = ratatui::text::Line::from(display_name);
                    if i == state.selected_index {
                        ratatui::widgets::ListItem::new(content)
                            .style(ratatui::style::Style::default().bg(ratatui::style::Color::Blue))
                    } else {
                        ratatui::widgets::ListItem::new(content)
                    }
                })
                .collect();
            
            let list = ratatui::widgets::List::new(items)
                .block(list_block)
                .highlight_symbol("> ")
                .highlight_style(ratatui::style::Style::default().bg(ratatui::style::Color::DarkGray));
            
            f.render_widget(list, content_chunks[0]);
            
            // 右侧：文件详情
            let detail_block = ratatui::widgets::Block::default()
                .title("文件详情")
                .borders(ratatui::widgets::Borders::ALL);
            
            // 更新详情文本
            let detail_text = if let Some(ref file) = state.selected_file {
                let size_str = format_bytes(file.size_bytes);
                let ext_str = file.extension.as_deref().unwrap_or("无扩展名");
                let modified_str = file.last_modified
                    .and_then(|t| t.elapsed().ok())
                    .map(|d| format!("{} 天前", d.as_secs() / 86400))
                    .unwrap_or_else(|| "未知".to_string());
                format!("选中文件: {}\n\n路径: {}\n大小: {}\n扩展名: {}\n最后修改: {}", 
                    file.path.file_name().and_then(|n| n.to_str()).unwrap_or(""),
                    file.path.display(),
                    size_str,
                    ext_str,
                    modified_str)
            } else if state.selected_index < state.flat_nodes.len() {
                let node = &state.flat_nodes[state.selected_index];
                let size_str = format_bytes(node.size_bytes);
                let node_type = if node.is_dir { "目录" } else { "文件" };
                format!("选中节点: {}\n\n路径: {}\n类型: {}\n大小: {}", 
                    node.name,
                    node.path.display(),
                    node_type,
                    size_str)
            } else {
                "无选中节点".to_string()
            };
            
            let detail_paragraph = ratatui::widgets::Paragraph::new(detail_text)
                .block(detail_block);
            f.render_widget(detail_paragraph, content_chunks[1]);
            
            // 底部状态栏
            let status_text = format!("Surf TUI | 扫描: {} 文件, {} 目录, {} | 选中: {}/{} | ↑↓ 导航, Enter 展开/折叠, Enter 查看, q/Esc 退出", 
                result.summary.total_files,
                result.summary.total_dirs,
                format_bytes(result.summary.total_size_bytes),
                state.selected_index + 1,
                state.flat_nodes.len());
            let status_bar = ratatui::widgets::Paragraph::new(status_text)
                .style(ratatui::style::Style::default().bg(ratatui::style::Color::DarkGray));
            f.render_widget(status_bar, main_chunks[1]);
        })?;

        // 处理键盘事件
        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => {
                    break;
                }
                KeyCode::Down => {
                    // 向下移动选中项
                    if state.selected_index < state.flat_nodes.len() - 1 {
                        state.selected_index += 1;
                    }
                }
                KeyCode::Up => {
                    // 向上移动选中项
                    if state.selected_index > 0 {
                        state.selected_index -= 1;
                    }
                }
                KeyCode::Enter => {
                    // Enter 键：处理目录展开/折叠或文件详情
                    if state.selected_index < state.flat_nodes.len() {
                        let node = &state.flat_nodes[state.selected_index];
                        if node.is_dir {
                            // 目录：切换展开状态
                            toggle_node_expansion(&mut state.tree_root, &node.path);
                            // 重新扁平化树
                            state.flat_nodes = state.tree_root.flatten();
                            // 保持选中索引不变（如果可能）
                        } else {
                            // 文件：查找对应的 FileEntry
                            state.selected_file = result.top_files.iter()
                                .find(|entry| entry.path == node.path)
                                .cloned();
                        }
                    }
                }
                _ => {}
            }
        }
    }

    Ok(())
}
