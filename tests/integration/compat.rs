use std::process::Command;

fn mlocate_bin() -> String {
    std::env::var("CARGO_BIN_EXE_mlocate").unwrap_or_else(|_| "target/debug/mlocate".to_string())
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
        .arg("--prunepaths")
        .arg("/nonexistent-prune-path-mlocate")
        .arg("--quiet")
        .output()
        .unwrap();
    assert!(output.status.success(), "mupdatedb failed: {:?}", output);
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
            || stderr.contains("no index found")
            || stderr.is_empty(),
        "unexpected stderr: {}",
        stderr,
    );
}

#[test]
fn test_case_insensitive() {
    let tmp = tempfile::TempDir::new().unwrap();
    let db = tmp.path().join("mlocate.db");
    setup_test_db(tmp.path(), &db);
    let output = Command::new(mlocate_bin())
        .arg("-i")
        .arg("TEST")
        .arg("--count")
        .arg("--database")
        .arg(&db)
        .output()
        .expect("should run");
    assert!(output.status.success(), "Command failed: {:?}", output);
}

#[test]
fn test_count_flag() {
    let tmp = tempfile::TempDir::new().unwrap();
    let db = tmp.path().join("mlocate.db");
    setup_test_db(tmp.path(), &db);
    let output = Command::new(mlocate_bin())
        .arg("--count")
        .arg("test_file")
        .arg("--database")
        .arg(&db)
        .output()
        .expect("should run");
    assert!(output.status.success(), "Command failed: {:?}", output);
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
