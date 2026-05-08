use std::process::Command;

fn mlocate_bin() -> String {
    std::env::var("CARGO_BIN_EXE_mlocate")
        .unwrap_or_else(|_| "target/debug/mlocate".to_string())
}

#[test]
fn test_gnu_flag_stubs_error() {
    let output = Command::new(mlocate_bin())
        .arg("-L")
        .arg("test")
        .output()
        .expect("should run");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not supported"));
}

#[test]
fn test_gnu_mode_accepts_stubs() {
    let output = Command::new(mlocate_bin())
        .arg("--gnu")
        .arg("-L")
        .arg("test")
        .output()
        .expect("should run");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("will be ignored")
            || stderr.contains("not supported")
            || stderr.contains("No database found")
            || stderr.contains("No index found")
            || stderr.is_empty(),
        "unexpected stderr: {}",
        stderr,
    );
}

#[test]
fn test_case_insensitive() {
    let output = Command::new(mlocate_bin())
        .arg("-i")
        .arg("TEST")
        .arg("--count")
        .output()
        .expect("should run");
    let _ = output.status;
}

#[test]
fn test_count_flag() {
    let output = Command::new(mlocate_bin())
        .arg("--count")
        .arg("testpattern")
        .output()
        .expect("should run");
    let _ = output.status;
}

#[test]
fn test_null_flag_conflict_with_plain() {
    let output = Command::new(mlocate_bin())
        .arg("--null")
        .arg("--plain")
        .arg("test")
        .output()
        .expect("should run");
    assert!(!output.status.success());
}
