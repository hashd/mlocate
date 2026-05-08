use std::process::Command;

fn mlocate_bin() -> String {
    std::env::var("CARGO_BIN_EXE_mlocate")
        .unwrap_or_else(|_| "target/debug/mlocate".to_string())
}

fn mupdatedb_bin() -> String {
    std::env::var("CARGO_BIN_EXE_mupdatedb")
        .unwrap_or_else(|_| "target/debug/mupdatedb".to_string())
}

#[test]
fn test_help_output() {
    let output = Command::new(mlocate_bin())
        .arg("--help")
        .output()
        .expect("should run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--regex"));
    assert!(stdout.contains("--json"));
}

#[test]
fn test_mupdatedb_help() {
    let output = Command::new(mupdatedb_bin())
        .arg("--help")
        .output()
        .expect("should run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--localpaths"));
    assert!(stdout.contains("--incremental"));
}

#[test]
fn test_version() {
    let output = Command::new(mlocate_bin())
        .arg("--version")
        .output()
        .expect("should run");
    assert!(output.status.success());
}

#[test]
fn test_count_no_db() {
    let db_path = std::env::temp_dir().join("nonexistent_mlocate_test.db");
    // Clean up from any previous run
    let _ = std::fs::remove_file(&db_path);
    let output = Command::new(mlocate_bin())
        .arg("--count")
        .arg("--database")
        .arg(db_path.to_str().unwrap())
        .arg("test")
        .output()
        .expect("should run");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Error") || output.status.code() == Some(2));
}

#[test]
fn test_end_to_end() {
    let tmp = tempfile::TempDir::new().expect("should create temp dir");
    let db_path = tmp.path().join("mlocate.db");

    // Create some test files
    let test_dir = tmp.path().join("testdata");
    std::fs::create_dir_all(&test_dir).unwrap();
    std::fs::write(test_dir.join("hello.rs"), "fn main() {}").unwrap();
    std::fs::write(test_dir.join("readme.md"), "# Test").unwrap();
    std::fs::write(test_dir.join("large_file.bin"), vec![0u8; 2000]).unwrap();

    // Index
    let output = Command::new(mupdatedb_bin())
        .arg("--localpaths")
        .arg(test_dir.to_str().unwrap())
        .arg("--database")
        .arg(db_path.to_str().unwrap())
        .arg("--force")
        .arg("--quiet")
        .output()
        .expect("should run");
    assert!(output.status.success(), "mupdatedb failed: {}", String::from_utf8_lossy(&output.stderr));

    assert!(db_path.exists(), "Database was not created");

    // Search
    let output = Command::new(mlocate_bin())
        .arg("--database")
        .arg(db_path.to_str().unwrap())
        .arg("hello")
        .arg("--plain")
        .output()
        .expect("should run");
    assert!(output.status.success(), "search failed: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("hello.rs"), "Search should find hello.rs, got: {}", stdout);
}
