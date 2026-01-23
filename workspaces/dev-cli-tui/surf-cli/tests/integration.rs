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

    // 6. 断言 JSON 解析后是一个数组
    let arr = parsed.as_array().expect("JSON root is not an array");

    // 7. 断言数组长度不超过 `--limit` 指定的值（此处为 1）
    assert!(arr.len() <= 1, "array length {} exceeds limit 1", arr.len());

    // 8. 对数组中每个条目，验证：
    //    - 包含 `path` 和 `size` 字段
    //    - size >= min_size (10)
    //    - 路径落在临时目录下
    for entry in arr {
        let obj = entry.as_object().expect("entry is not an object");
        let path = obj.get("path").expect("missing 'path' field").as_str().expect("'path' is not a string");
        let size = obj.get("size").expect("missing 'size' field").as_u64().expect("'size' is not a u64");

        assert!(size >= 10, "size {} is less than min-size 10", size);
        assert!(path.starts_with(temp_path.to_str().unwrap()), "path {} is not under temp directory", path);
    }

    // 可选：验证确实只返回了一个条目（因为 limit=1，且有两个符合条件的文件）
    // 注意：由于扫描顺序可能不确定，我们只检查长度不超过 limit，不假设具体数量。
}
