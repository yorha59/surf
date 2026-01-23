use std::fs::{self, File};
use std::io::Write;

use surf_core::scan;

// 基础 happy path 集成测试：
// - 构造一个临时目录，其中包含：
//   * small.txt：小于 min_size，应被过滤掉；
//   * big1.bin：位于根目录，大于等于 min_size；
//   * nested/big2.bin：位于子目录，大于等于 min_size；
// - 断言：
//   * 返回结果只包含 size >= min_size 的文件；
//   * 返回列表按 size 降序排序；
//   * big1.bin / big2.bin 均被包含，small.txt 不被包含。
//
// TODO: 后续可补充更多场景测试，例如：
// - 空目录；
// - 权限不足/损坏的目录项；
// - 含符号链接的大型目录树等。

#[test]
fn scan_respects_min_size_and_sorts_desc() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let root = temp_dir.path();

    let min_size: u64 = 10;

    // 小文件：应被过滤掉
    let small_path = root.join("small.txt");
    let mut small_file = File::create(&small_path)?;
    write!(small_file, "12345")?; // 5 字节 < min_size

    // 大文件 1：位于根目录
    let big1_path = root.join("big1.bin");
    let mut big1_file = File::create(&big1_path)?;
    write!(big1_file, "{}", "X".repeat(20))?; // 20 字节 >= min_size

    // 大文件 2：位于子目录，验证递归扫描能力
    let nested_dir = root.join("nested");
    fs::create_dir(&nested_dir)?;

    let big2_path = nested_dir.join("big2.bin");
    let mut big2_file = File::create(&big2_path)?;
    write!(big2_file, "{}", "Y".repeat(30))?; // 30 字节 >= min_size

    // 调用被测 API，使用一个小的线程数验证并发扫描路径
    let entries = scan(root, min_size, 4)?;

    // 1. 所有返回的文件都满足 size >= min_size
    assert!(
        entries.iter().all(|e| e.size >= min_size),
        "found entry with size < min_size: {:?}",
        entries
            .iter()
            .find(|e| e.size < min_size)
    );

    // 2. 返回结果包含 big1.bin / big2.bin，但不包含 small.txt
    let names: Vec<String> = entries
        .iter()
        .filter_map(|e| e.path.file_name())
        .map(|name| name.to_string_lossy().to_string())
        .collect();

    assert!(names.contains(&"big1.bin".to_string()));
    assert!(names.contains(&"big2.bin".to_string()));
    assert!(!names.contains(&"small.txt".to_string()));

    // 3. 结果按 size 降序排序
    for window in entries.windows(2) {
        let first = &window[0];
        let second = &window[1];
        assert!(
            first.size >= second.size,
            "entries not sorted by size desc: first={:?}, second={:?}",
            first,
            second
        );
    }

    Ok(())
}
