# Surf CLI/TUI 开发任务清单

## 任务状态说明
- ✅ 已完成
- 🔄 进行中
- ⏳ 待办

## 本轮迭代任务（基于 PRD.md 和 Architecture.md）

### 1. 项目初始化与依赖配置
- [x] 创建工作区目录 `/Users/bytedance/GitHub/surf/workspaces/dev-cli-tui/`
- [x] 初始化 Rust 二进制项目 (`cargo init --bin`)
- [x] 配置 Cargo.toml：添加 clap、indicatif、ratatui、serde、anyhow、thiserror 依赖
- [x] 添加对 surf-core 的本地路径依赖 (`../dev-core-scanner`)

### 2. 命令行参数解析
- [x] 使用 clap 定义 CLI 参数结构，支持：
  - `--path, -p` (默认 ".")
  - `--threads, -t` (默认逻辑核心数)
  - `--min-size, -m` (支持单位 B/KB/MB/GB)
  - `--limit, -n` (默认 20)
  - `--service, -s` (服务模式开关)
  - `--port` (默认 1234)
  - `--host` (默认 "127.0.0.1")
  - `--json` (JSON 输出开关)
  - `--help, -h`
- [ ] 参数验证逻辑（路径存在性、数值范围等）
- [ ] 错误信息友好输出

### 3. 核心扫描引擎集成
- [ ] 调用 surf-core 的同步扫描接口 (`Scanner::scan_sync`)
- [ ] 处理扫描结果，提取摘要、Top N 文件等
- [ ] 适配 surf-core 的数据结构（ScanResult, FileEntry 等）

### 4. 进度条实现
- [ ] 使用 indicatif 创建动态进度条
- [ ] 在扫描过程中实时更新已处理文件数和总容量
- [ ] 处理扫描中断（Ctrl+C）并优雅停止进度条

### 5. 结果输出
- [ ] 默认表格输出：按文件大小降序排列，限制条目数
- [ ] 表格格式化（列对齐、单位转换等）
- [ ] JSON 输出：支持 `--json` 参数，序列化 ScanResult 结构
- [ ] 输出到 stdout，确保无残留半写入文件

### 6. TUI 模式基础
- [ ] 使用 ratatui 构建终端用户界面框架
- [ ] 实现目录树导航视图
- [ ] 实现文件详情查看面板
- [ ] 键盘绑定（方向键导航、Enter 进入目录、Esc 返回等）
- [ ] 当前版本不包含删除操作

### 7. 单元测试与自测
- [ ] 编写 CLI 参数解析的单元测试
- [ ] 编写进度条和输出格式的集成测试
- [ ] 创建自测脚本，验证基本功能
- [ ] 在工作区记录自测结果

### 8. 文档与构建说明
- [ ] 编写 README.md，说明如何构建和运行
- [ ] 确保二进制产物为 `target/release/surf`
- [ ] 提供简单使用示例

## 完成判定标准
- [ ] 所有以上任务完成并自测通过
- [ ] 可执行二进制 `surf` 能够成功构建
- [ ] 支持基本 CLI 参数和同步扫描
- [ ] 进度条和表格输出正常工作
- [ ] TUI 模式可以启动并浏览结果

## 备注
- 当前 surf-core 版本仅提供同步扫描，top_files、by_extension、stale_files 等功能暂未实现，后续迭代需要更新。
- 服务模式（--service）在本轮中可能仅作占位，实际实现留待后续迭代。
- 删除功能不在本轮范围内，TUI 中不暴露删除入口。