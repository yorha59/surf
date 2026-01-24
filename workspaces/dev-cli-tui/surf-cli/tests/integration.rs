//! 端到端 CLI 集成测试，验证 `surf` 二进制在 `--json` 模式下的行为。

use assert_cmd::Command;
use serde_json::Value;
use std::fs::File;
use std::io::Write;
use tempfile::tempdir;

#[test]
fn test_surf_json_output_with_min_size_and_limit() {
    // 1. 创建临时目录
    let temp_dir = tempdir().expect("failed to create temp directory");
    let temp_path = temp_dir.path();

    // 2. 在临时目录中创建若干不同大小的文件
    //    文件1: 5 字节（小于 min-size）
    let small_file = temp_path.join("small.txt");
    File::create(&small_file)
        .expect("failed to create small file")
        .write_all(b"12345")
        .expect("failed to write small file");

    //    文件2: 20 字节（大于等于 min-size）
    let medium_file = temp_path.join("medium.txt");
    File::create(&medium_file)
        .expect("failed to create medium file")
        .write_all(&[b'x'; 20])
        .expect("failed to write medium file");

    //    文件3: 30 字节（大于等于 min-size）
    let large_file = temp_path.join("large.txt");
    File::create(&large_file)
        .expect("failed to create large file")
        .write_all(&[b'y'; 30])
        .expect("failed to write large file");

    // 3. 运行 `surf` CLI，参数：--path <temp_dir> --min-size 10 --limit 1 --json
    let mut cmd = Command::cargo_bin("surf").expect("failed to find surf binary");
    cmd.args([
        "--path",
        temp_path.to_str().unwrap(),
        "--min-size",
        "10",
        "--limit",
        "1",
        "--json",
    ]);

    // 4. 断言进程以退出码 0 成功结束，并捕获标准输出
    let output = cmd.output().expect("failed to execute surf");
    assert!(output.status.success(), "surf exited with non-zero status: {:?}", output.status);

    let stdout = String::from_utf8(output.stdout).expect("output is not valid UTF-8");

    // 5. 断言标准输出是合法 JSON
    let parsed: Value = serde_json::from_str(&stdout).expect("output is not valid JSON");

    // 6. 断言 JSON 解析后是一个对象（对应新的 JsonOutput 结构）
    let obj = parsed.as_object().expect("JSON root is not an object");

    // 7. 验证根路径字段
    let root = obj.get("root").expect("missing 'root' field").as_str().expect("'root' is not a string");
    assert!(root.starts_with(temp_path.to_str().unwrap()), "root {} is not under temp directory", root);

    // 8. 验证条目数组字段
    let entries = obj.get("entries").expect("missing 'entries' field").as_array().expect("'entries' is not an array");
    assert!(entries.len() <= 1, "entries length {} exceeds limit 1", entries.len());

    // 9. 对每个条目，验证：
    //    - 包含 `path` 和 `size` 字段
    //    - 包含 `is_dir` 字段且为 false（当前扫描器仅返回文件）
    //    - size >= min_size (10)
    //    - 路径落在临时目录下
    for entry in entries {
        let entry_obj = entry.as_object().expect("entry is not an object");
        let path = entry_obj.get("path").expect("missing 'path' field").as_str().expect("'path' is not a string");
        let size = entry_obj.get("size").expect("missing 'size' field").as_u64().expect("'size' is not a u64");
        let is_dir = entry_obj.get("is_dir").expect("missing 'is_dir' field").as_bool().expect("'is_dir' is not a boolean");

        assert!(size >= 10, "size {} is less than min-size 10", size);
        assert!(path.starts_with(temp_path.to_str().unwrap()), "path {} is not under temp directory", path);
        assert!(!is_dir, "is_dir should be false for file entries");
    }

    // 可选：验证确实只返回了一个条目（因为 limit=1，且有两个符合条件的文件）
    // 注意：由于扫描顺序可能不确定，我们只检查长度不超过 limit，不假设具体数量。
}
/// 测试 `--json` 模式下，传入无法解析的 `--min-size` 参数时的行为。
#[test]
fn test_surf_json_error_on_invalid_min_size() {
    let mut cmd = Command::cargo_bin("surf").expect("failed to find surf binary");
    cmd.args(["--min-size", "invalid", "--json"]);

    let output = cmd.output().expect("failed to execute surf");
    // 进程应以非零退出码结束
    assert!(!output.status.success(), "surf should exit with non-zero status");
    // stdout 不应包含任何 JSON 片段（应为空或仅空白）
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.trim().is_empty(),
        "stdout should be empty on error, got: {}",
        stdout
    );
    // stderr 应包含错误信息（检查是否包含 "Error parsing --min-size" 或类似内容）
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Error parsing --min-size") || stderr.contains("invalid size"),
        "stderr should contain error message about min-size, got: {}",
        stderr
    );
}

/// 测试 `--json` 模式下，传入非法 `--threads` 值（如 0）时的行为。
#[test]
fn test_surf_json_error_on_invalid_threads() {
    let mut cmd = Command::cargo_bin("surf").expect("failed to find surf binary");
    cmd.args(["--threads", "0", "--json"]);

    let output = cmd.output().expect("failed to execute surf");
    assert!(!output.status.success(), "surf should exit with non-zero status");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.trim().is_empty(),
        "stdout should be empty on error, got: {}",
        stdout
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--threads must be at least 1") || stderr.contains("invalid value"),
        "stderr should contain error message about threads, got: {}",
        stderr
    );
}

/// 测试 `--json` 模式下，传入不存在或不可访问的路径时的行为。
#[test]
fn test_surf_json_error_on_nonexistent_path() {
    let mut cmd = Command::cargo_bin("surf").expect("failed to find surf binary");
    // 使用一个几乎不可能存在的路径
    cmd.args(["--path", "/tmp/this_path_should_not_exist_123456789", "--json"]);

    let output = cmd.output().expect("failed to execute surf");
    assert!(!output.status.success(), "surf should exit with non-zero status");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.trim().is_empty(),
        "stdout should be empty on error, got: {}",
        stdout
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    // 错误信息可能来自扫描器，检查是否包含 "Failed to scan" 或 "No such file"
    assert!(
        stderr.contains("Failed to scan") || stderr.contains("No such file") || stderr.contains("failed to scan"),
        "stderr should contain error message about scanning, got: {}",
        stderr
    );
}

/// 测试非 `--json` 模式下，错误参数的行为是否与 `--json` 模式一致（stdout 无输出，stderr 有错误）。
#[test]
fn test_surf_non_json_error_behavior() {
    // 测试非法 min-size 参数
    let mut cmd = Command::cargo_bin("surf").expect("failed to find surf binary");
    cmd.args(["--min-size", "invalid"]);
    // 不传递 --json

    let output = cmd.output().expect("failed to execute surf");
    assert!(!output.status.success(), "surf should exit with non-zero status");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.trim().is_empty(),
        "stdout should be empty on error, got: {}",
        stdout
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Error parsing --min-size") || stderr.contains("invalid size"),
        "stderr should contain error message about min-size, got: {}",
        stderr
    );
}

/// 测试非 `--json` 表格模式下的基础成功路径：
/// - 使用临时目录与不同大小文件；
/// - 通过 `--path` / `--min-size` / `--limit` 触发扫描；
/// - 验证表头存在且数据行数不超过 limit，stdout 不为空。
#[test]
fn test_surf_table_output_with_min_size_and_limit() {
    let temp_dir = tempdir().expect("failed to create temp directory");
    let temp_path = temp_dir.path();

    // 创建若干文件：一个小文件（应被过滤掉），两个大文件（应被保留）。
    let small_file = temp_path.join("small.txt");
    File::create(&small_file)
        .expect("failed to create small file")
        .write_all(b"12345")
        .expect("failed to write small file");

    let medium_file = temp_path.join("medium.txt");
    File::create(&medium_file)
        .expect("failed to create medium file")
        .write_all(&[b'x'; 20])
        .expect("failed to write medium file");

    let large_file = temp_path.join("large.txt");
    File::create(&large_file)
        .expect("failed to create large file")
        .write_all(&[b'y'; 30])
        .expect("failed to write large file");

    // 不使用 --json，走表格输出路径。
    let mut cmd = Command::cargo_bin("surf").expect("failed to find surf binary");
    cmd.args([
        "--path",
        temp_path.to_str().unwrap(),
        "--min-size",
        "10",
        "--limit",
        "2",
    ]);

    let output = cmd.output().expect("failed to execute surf");
    assert!(
        output.status.success(),
        "surf exited with non-zero status: {:?}",
        output.status
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout is not valid UTF-8");

    // stdout 至少应包含两行表头（标题行 + 分隔行）。
    let mut lines = stdout.lines();
    let header_line = lines.next().expect("missing header line");
    let separator_line = lines.next().expect("missing separator line");

    assert!(
        header_line.contains("SIZE") && header_line.contains("PATH"),
        "header line does not contain expected columns: {}",
        header_line
    );
    assert!(
        separator_line.chars().any(|c| c == '-'),
        "separator line does not look like a divider: {}",
        separator_line
    );

    // 剩余的数据行数量不应超过 limit（这里为 2），且至少有一行数据。
    let data_lines: Vec<&str> = lines.collect();
    assert!(
        !data_lines.is_empty(),
        "expected at least one data line in table output"
    );
    assert!(
        data_lines.len() <= 2,
        "data lines length {} exceeds limit 2",
        data_lines.len()
    );

    // 数据行中应至少包含临时目录路径片段，以确认扫描目标正确。
    let temp_str = temp_path.to_str().unwrap();
    assert!(
        data_lines.iter().any(|line| line.contains(temp_str)),
        "no data line contains temp directory path {}; lines: {:?}",
        temp_str,
        data_lines
    );
}
