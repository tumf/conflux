#![cfg(windows)]

use std::process::Command;

#[test]
fn unix_domain_client_is_explicitly_unavailable() {
    let output = Command::new(env!("CARGO_BIN_EXE_cflx"))
        .args(["client", "status", "--json"])
        .output()
        .expect("the compiled cflx binary must be runnable");
    let stdout = String::from_utf8(output.stdout).expect("stdout must be UTF-8 JSON");
    let envelope: serde_json::Value = serde_json::from_str(stdout.trim()).expect("valid JSON");

    assert_eq!(output.status.code(), Some(20));
    assert_eq!(envelope["outcome"], "feature_unavailable");
    assert_eq!(envelope["operation"], "status");
}

#[test]
fn unix_domain_mcp_is_explicitly_unavailable() {
    let output = Command::new(env!("CARGO_BIN_EXE_cflx"))
        .args(["client", "mcp"])
        .output()
        .expect("the compiled cflx binary must be runnable");
    let stderr = String::from_utf8(output.stderr).expect("stderr must be UTF-8");

    assert_eq!(output.status.code(), Some(20));
    assert!(output.stdout.is_empty(), "protocol stdout must stay empty");
    assert!(stderr.contains("Unix-domain local /api/v2 client support"));
}
