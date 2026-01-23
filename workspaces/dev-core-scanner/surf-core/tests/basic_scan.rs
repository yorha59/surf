use std::collections::BTreeSet;
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

// 验证 threads = 0 时不会 panic/报错，且行为与 threads = 1 等价：
// - 构造一个包含 small.txt（< min_size）和两个大文件的目录；
// - 分别以 threads = 0 和 threads = 1 调用 scan；
// - 断言两次调用均返回 Ok，且：
//   * 返回结果都包含预期的大文件名；
//   * 两次结果在是否为空、元素数量以及路径/size 集合上完全一致。
#[test]
fn scan_threads_zero_falls_back_to_one() -> Result<(), Box<dyn std::error::Error>> {
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

    // 分别以 threads = 0 和 threads = 1 调用被测 API
    let result_zero = scan(root, min_size, 0);
    let result_one = scan(root, min_size, 1);

    // 1. 显式断言两次调用都返回 Ok
    assert!(
        result_zero.is_ok(),
        "scan with threads=0 returned error: {:?}",
        result_zero
    );
    assert!(
        result_one.is_ok(),
        "scan with threads=1 returned error: {:?}",
        result_one
    );

    let entries_zero = result_zero?;
    let entries_one = result_one?;

    // 2. 两次结果都包含预期的大文件
    let names_zero: Vec<String> = entries_zero
        .iter()
        .filter_map(|e| e.path.file_name())
        .map(|name| name.to_string_lossy().to_string())
        .collect();
    let names_one: Vec<String> = entries_one
        .iter()
        .filter_map(|e| e.path.file_name())
        .map(|name| name.to_string_lossy().to_string())
        .collect();

    for target in ["big1.bin", "big2.bin"] {
        assert!(
            names_zero.contains(&target.to_string()),
            "threads=0 result missing {target}"
        );
        assert!(
            names_one.contains(&target.to_string()),
            "threads=1 result missing {target}"
        );
    }

    // 3. 两次结果在是否为空和元素数量上保持一致
    assert_eq!(
        entries_zero.is_empty(),
        entries_one.is_empty(),
        "threads=0 and threads=1 emptiness differ"
    );
    assert_eq!(
        entries_zero.len(),
        entries_one.len(),
        "threads=0 and threads=1 length differ"
    );

    // 4. 路径集合和 size 集合完全一致（行为等价）
    let paths_zero: BTreeSet<_> = entries_zero.iter().map(|e| e.path.clone()).collect();
    let paths_one: BTreeSet<_> = entries_one.iter().map(|e| e.path.clone()).collect();
    assert_eq!(paths_zero, paths_one, "paths differ between threads=0 and threads=1");

    let sizes_zero: BTreeSet<_> = entries_zero.iter().map(|e| e.size).collect();
    let sizes_one: BTreeSet<_> = entries_one.iter().map(|e| e.size).collect();
    assert_eq!(sizes_zero, sizes_one, "sizes differ between threads=0 and threads=1");

    Ok(())
}

// 验证当 root 路径不存在时，scan 会返回清晰的 NotFound 错误，而不是静默成功：
// - 在一个临时目录下构造一个尚未创建的子目录路径；
// - 调用 scan 并断言返回 Err；
// - 进一步断言错误种类为 ErrorKind::NotFound，且错误消息包含 "does not exist"，
//   便于 CLI 层为用户展示明确的问题原因。
#[test]
fn scan_nonexistent_root_returns_not_found() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let nonexistent_root = temp_dir.path().join("nonexistent-subdir");

    let result = scan(&nonexistent_root, 0, 4);

    assert!(
        result.is_err(),
        "expected error for nonexistent root, got: {:?}",
        result
    );

    let err = result.unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    let msg = err.to_string();
    assert!(
        msg.contains("does not exist"),
        "error message should mention 'does not exist', got: {}",
        msg
    );

    Ok(())
}

// 验证空目录场景：
// - 在一个临时目录中不创建任何文件或子目录；
// - 调用 scan 并断言返回 Ok 且结果为空；
// - 这保证了在没有任何符合条件文件时，调用方可以通过 entries.is_empty()
//   清晰地区分「扫描成功但无命中」与「扫描失败」。
#[test]
fn scan_empty_directory_returns_empty_result() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let root = temp_dir.path();

    let entries = scan(root, 0, 4)?;

    assert!(
        entries.is_empty(),
        "expected empty result for empty directory, got: {:?}",
        entries
    );

    Ok(())
}
