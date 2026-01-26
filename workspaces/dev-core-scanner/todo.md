# dev-core-scanner 工作区待办清单

## 本轮完成（2026-01-26）

### 1. 初始化工作区骨架
- 创建 Rust crate `surf-core`（库类型）
- 设置 Cargo.toml：edition = "2021"，添加 dev-dependencies（tempfile）

### 2. 定义核心数据结构（对齐 Architecture.md 4.5 节）
- `ScanRequest`：扫描请求参数（路径、线程数、最小大小、排除规则、陈旧天数）
- `ScanProgress`：扫描进度信息（状态、已扫描文件数、字节数、进度百分比、预计剩余时间）
- `ScanState`：任务状态枚举（Queued/Running/Completed/Canceled/Failed）
- `ScanSummary`：扫描结果摘要（根路径、总文件数、总目录数、总大小、耗时）
- `FileEntry`：文件条目信息（路径、大小、最后修改时间、扩展名）
- `ExtensionStat`：文件类型统计（扩展名、文件数、总大小）
- `ScanResult`：完整扫描结果（摘要、Top N 文件、按扩展名统计、陈旧文件列表）

### 3. 实现最小同步扫描功能
- `Scanner` 结构体提供 `scan_sync` 方法
- 当前实现：递归遍历目录，统计文件数、目录数和总大小
- 支持便捷函数 `scan_path` 快速扫描

### 4. 编写单元测试
- 测试 `ScanRequest::new`
- 测试空目录扫描
- 测试含文件和子目录的扫描
- 测试便捷函数 `scan_path`

### 5. 自测验证
- 运行 `cargo build` 成功编译
- 运行 `cargo test` 所有测试通过

### 6. 实现多线程扫描
- 添加 `rayon` 依赖，支持并行目录遍历
- 使用原子计数器 (`AtomicU64`) 实现线程安全的统计累加
- 基于 `rayon::ThreadPoolBuilder` 支持通过 `ScanRequest::threads` 控制并发度
- 保持 API 兼容性：`scan_sync` 方法签名不变
- 添加多线程扫描单元测试，验证与单线程结果一致性

### 7. 实现文件类型分析
- 在 `AtomicCounters` 中添加 `extensions` 映射（`Arc<Mutex<HashMap>>`）用于线程安全的扩展名统计
- 添加 `add_file_with_extension` 方法，处理扩展名提取（小写转换）和统计更新
- 修改 `parallel_walk_dir` 在遇到文件时提取扩展名并调用统计方法
- 添加 `extensions_to_vec` 方法将映射转换为按总大小降序排序的 `Vec<ExtensionStat>`
- 更新 `scan_sync` 方法，填充 `ScanResult` 的 `by_extension` 字段
- 编写单元测试 `test_extension_statistics`，验证扩展名统计的正确性（包括大小写不敏感、空扩展名处理）

### 8. 实现 Top N 大文件功能
- 为 `ScanRequest` 添加 `limit` 字段（默认20）
- 为 `FileEntry` 实现 `Ord`、`PartialOrd`、`Eq`、`PartialEq` trait
- 在 `AtomicCounters` 中添加线程安全的 Top N 文件收集机制（`Mutex<BinaryHeap<Reverse<FileEntry>>>`）
- 实现 `add_file_to_top_list` 方法，比较文件大小并维护堆
- 修改 `parallel_walk_dir` 函数，对每个文件调用 `add_file_to_top_list`
- 在 `to_summary` 或新方法中转换堆为按大小降序排列的向量，填充 `ScanResult` 的 `top_files` 字段
- 添加单元测试，验证 Top N 功能正确性（不同大小、相同大小、零大小文件）

## 待办事项（后续迭代）

### 高优先级
1. **陈旧文件识别**：读取文件元数据的最后修改/访问时间，筛选超过阈值的文件
2. **过滤与排除**：支持 `--min-size` 过滤和 `--exclude` 模式排除

### 中优先级
5. **进度回调**：提供进度通知机制，供上层（CLI/TUI/服务）展示进度条
6. **错误处理**：细化错误类型（权限不足、路径不存在、IO 错误等）
7. **性能优化**：避免重复 stat 调用，缓存目录条目信息
8. **符号链接处理**：决定是否跟随符号链接（默认不跟随）

### 低优先级
10. **删除能力接口**：定义删除文件/目录的接口（需与人类确认统一策略）
11. **配置持久化**：与共享配置模块对接，读取默认扫描参数
12. **基准测试**：在大目录上测试扫描性能，确保满足 PRD 非功能性要求

## 自测记录

### 构建测试
```bash
$ cargo build
warning: struct `ScanCollector` is never constructed
   --> src/lib.rs:123:8
    |
123 | struct ScanCollector {
    |        ^^^^^^^^^^^^^
    |
    = note: `#[warn(dead_code)]` on by default

warning: associated function `walk_dir` is never used
   --> src/lib.rs:244:8
    |
199 | impl Scanner {
    | ------------ associated function in this implementation
...
244 |     fn walk_dir(
    |        ^^^^^^^^

warning: `surf-core` (lib) generated 2 warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.11s
```

### 单元测试
```bash
$ cargo test
running 6 tests
test tests::test_scan_request_new ... ok
test tests::test_scan_sync_empty_dir ... ok
test tests::test_scan_path_convenience ... ok
test tests::test_scan_sync_with_files ... ok
test tests::test_extension_statistics ... ok
test tests::test_scan_sync_multithreaded ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.82s
```

### 功能验证
- 在当前工作区目录执行扫描（示例）：
  ```rust
  use surf_core::scan_path;
  let result = scan_path(".").unwrap();
  println!("总文件数: {}", result.summary.total_files);
  ```

## 已知问题与风险点
1. **递归遍历可能栈溢出**：对于深度极大的目录树，递归可能导致栈溢出，后续需改为迭代或使用栈数据结构
2. **无并发控制**：当前同步扫描为单线程，无法利用多核，不满足 PRD 中“极速扫描”要求
3. **内存占用未优化**：扫描结果目前仅包含摘要，未来存储完整文件列表时需考虑内存限制
4. **错误处理简单**：目前使用 `std::io::Result`，未定义细粒度的错误枚举

## 与架构设计的对应关系
- 数据结构与 Architecture.md 4.5 节“共享模型与配置”基本一致
- `Scanner` 类对应 Architecture.md 4.1 节“核心扫描与分析引擎”的入口
- 当前实现覆盖了“统计指定目录下的总文件数和总大小”的最小功能片段，并已实现按扩展名聚合统计（满足 PRD 3.3 节要求），为后续 Top N 文件、陈旧文件识别等功能打下基础

