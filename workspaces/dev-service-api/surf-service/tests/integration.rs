//! `surf-service` 二进制的基础集成测试。
//!
//! 由于当前环境可能缺少实际运行所需的网络/工具链，本测试仅验证：
//! - `surf-service --help` 能正常运行并输出 host/port 参数说明；
//! - `surf-service --version` 能正常运行并输出 crate 名称。

use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_cli_help_includes_host_flag() {
    let mut cmd = Command::cargo_bin("surf-service").expect("failed to find surf-service binary");
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("--host"));
}

#[test]
fn test_cli_version_mentions_crate_name() {
    let mut cmd = Command::cargo_bin("surf-service").expect("failed to find surf-service binary");
    cmd.arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("surf-service"));
}

