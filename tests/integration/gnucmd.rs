use std::process::Command;

fn mlocate_bin() -> String {
    std::env::var("CARGO_BIN_EXE_mlocate")
        .unwrap_or_else(|_| "target/debug/mlocate".to_string())
}

#[test]
fn test_gnu_mode_plain_output() {
    let output = Command::new(mlocate_bin())
        .arg("--gnu")
        .arg("test")
        .output()
        .expect("should run");
    assert!(output.status.success(), "Command failed: {:?}", output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.is_empty(), "Expected non-empty output");
}

#[test]
fn test_gnu_s_short_flag() {
    let output = Command::new(mlocate_bin())
        .arg("-S")
        .output()
        .expect("should run");
    assert!(output.status.success(), "Command failed: {:?}", output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"schema_version\""), "Expected JSON schema in: {}", stdout);
}
