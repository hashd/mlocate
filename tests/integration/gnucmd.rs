use std::process::Command;

fn mlocate_bin() -> String {
    std::env::var("CARGO_BIN_EXE_mlocate")
        .unwrap_or_else(|_| "target/debug/mlocate".to_string())
}

fn mupdatedb_bin() -> String {
    std::env::var("CARGO_BIN_EXE_mupdatedb")
        .unwrap_or_else(|_| "target/debug/mupdatedb".to_string())
}

fn setup_test_db(dir: &std::path::Path, db: &std::path::Path) {
    std::fs::write(dir.join("test_file.txt"), "test content").unwrap();
    let output = Command::new(mupdatedb_bin())
        .arg("--localpaths")
        .arg(dir)
        .arg("--database")
        .arg(db)
        .arg("--quiet")
        .output()
        .unwrap();
    assert!(output.status.success(), "mupdatedb failed: {:?}", output);
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
