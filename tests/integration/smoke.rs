use std::process::Command;

fn mlocate_bin() -> String {
    std::env::var("CARGO_BIN_EXE_mlocate")
        .unwrap_or_else(|_| "target/debug/mlocate".to_string())
}

fn mupdatedb_bin() -> String {
    std::env::var("CARGO_BIN_EXE_mupdatedb")
        .unwrap_or_else(|_| "target/debug/mupdatedb".to_string())
}

fn index_test_dir(dir: &std::path::Path, db_path: &std::path::Path) {
    let output = Command::new(mupdatedb_bin())
        .arg("--localpaths")
        .arg(dir.to_str().unwrap())
        .arg("--database")
        .arg(db_path.to_str().unwrap())
        .arg("--force")
        .arg("--quiet")
        .output()
        .expect("should run mupdatedb");
    assert!(output.status.success(), "mupdatedb failed: {}", String::from_utf8_lossy(&output.stderr));
    assert!(db_path.exists(), "Database was not created");
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
fn test_mupdatedb_incremental_warns() {
    let tmp = tempfile::TempDir::new().expect("should create temp dir");
    let db_path = tmp.path().join("mlocate.db");
    let test_dir = tmp.path().join("testdata");
    std::fs::create_dir_all(&test_dir).unwrap();
    std::fs::write(test_dir.join("file.txt"), "content").unwrap();

    let output = Command::new(mupdatedb_bin())
        .arg("--incremental")
        .arg("--localpaths")
        .arg(test_dir.to_str().unwrap())
        .arg("--database")
        .arg(db_path.to_str().unwrap())
        .arg("--quiet")
        .output()
        .expect("should run");
    assert!(output.status.success(), "--incremental should warn and fall back to full rebuild");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not supported") || stderr.contains("Falling back"));
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

    let test_dir = tmp.path().join("testdata");
    std::fs::create_dir_all(&test_dir).unwrap();
    std::fs::write(test_dir.join("hello.rs"), "fn main() {}").unwrap();
    std::fs::write(test_dir.join("readme.md"), "# Test").unwrap();
    std::fs::write(test_dir.join("large_file.bin"), vec![0u8; 2000]).unwrap();

    index_test_dir(&test_dir, &db_path);

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

#[test]
fn test_existing_filter() {
    let tmp = tempfile::TempDir::new().expect("should create temp dir");
    let db_path = tmp.path().join("mlocate.db");
    let test_dir = tmp.path().join("testdata");
    std::fs::create_dir_all(&test_dir).unwrap();
    let doomed_path = test_dir.join("doomed.txt");
    std::fs::write(&doomed_path, "will be deleted").unwrap();
    std::fs::write(test_dir.join("keeper.txt"), "stays").unwrap();

    index_test_dir(&test_dir, &db_path);

    let output = Command::new(mlocate_bin())
        .arg("--database")
        .arg(db_path.to_str().unwrap())
        .arg("--existing")
        .arg("--plain")
        .arg("txt")
        .output()
        .expect("should run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("doomed.txt"), "should find doomed.txt while it exists");
    assert!(stdout.contains("keeper.txt"));

    std::fs::remove_file(&doomed_path).unwrap();

    let output = Command::new(mlocate_bin())
        .arg("--database")
        .arg(db_path.to_str().unwrap())
        .arg("--existing")
        .arg("--plain")
        .arg("txt")
        .output()
        .expect("should run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("doomed.txt"), "deleted file should be excluded by --existing");
    assert!(stdout.contains("keeper.txt"));
}

#[test]
fn test_limit_with_filters() {
    let tmp = tempfile::TempDir::new().expect("should create temp dir");
    let db_path = tmp.path().join("mlocate.db");
    let test_dir = tmp.path().join("testdata");
    std::fs::create_dir_all(&test_dir).unwrap();
    std::fs::write(test_dir.join("small_a.rs"), "fn a() {}").unwrap();
    std::fs::write(test_dir.join("small_b.rs"), "fn b() {}").unwrap();
    std::fs::write(test_dir.join("big.rs"), vec![0u8; 5000]).unwrap();

    index_test_dir(&test_dir, &db_path);

    let output = Command::new(mlocate_bin())
        .arg("--database")
        .arg(db_path.to_str().unwrap())
        .arg("--limit")
        .arg("2")
        .arg("--plain")
        .arg("rs")
        .output()
        .expect("should run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 2, "limit should cap results at 2, got {:?}", lines);
}

#[test]
fn test_empty_directory() {
    let tmp = tempfile::TempDir::new().expect("should create temp dir");
    let db_path = tmp.path().join("mlocate.db");
    let test_dir = tmp.path().join("emptydir");
    std::fs::create_dir_all(&test_dir).unwrap();

    index_test_dir(&test_dir, &db_path);

    let output = Command::new(mlocate_bin())
        .arg("--database")
        .arg(db_path.to_str().unwrap())
        .arg("--plain")
        .arg("anything")
        .output()
        .expect("should run");
    assert_eq!(output.status.code(), Some(1), "empty DB should exit code 1 (no matches)");
}

#[test]
fn test_corrupted_index() {
    let tmp = tempfile::TempDir::new().expect("should create temp dir");
    let db_path = tmp.path().join("mlocate.db");
    let test_dir = tmp.path().join("testdata");
    std::fs::create_dir_all(&test_dir).unwrap();
    std::fs::write(test_dir.join("file.txt"), "content").unwrap();

    index_test_dir(&test_dir, &db_path);

    let mut raw = std::fs::read(&db_path).unwrap();
    if raw.len() > 16 {
        raw.truncate(16);
    }
    std::fs::write(&db_path, &raw).unwrap();

    let output = Command::new(mlocate_bin())
        .arg("--database")
        .arg(db_path.to_str().unwrap())
        .arg("test")
        .output()
        .expect("should run");
    assert!(!output.status.success(), "corrupted index should exit with error");
}

#[test]
fn test_modified_filter() {
    let tmp = tempfile::TempDir::new().expect("should create temp dir");
    let db_path = tmp.path().join("mlocate.db");
    let test_dir = tmp.path().join("testdata");
    std::fs::create_dir_all(&test_dir).unwrap();
    std::fs::write(test_dir.join("recent.txt"), "brand new file").unwrap();
    std::fs::write(test_dir.join("old.txt"), "old content").unwrap();

    index_test_dir(&test_dir, &db_path);

    let output = Command::new(mlocate_bin())
        .arg("--database")
        .arg(db_path.to_str().unwrap())
        .arg("--modified")
        .arg("1d-")
        .arg("--plain")
        .arg("txt")
        .output()
        .expect("should run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("recent.txt"), "1d- should find recently-indexed files");
    assert!(stdout.contains("old.txt"), "both files should be within 1 day");

    let output = Command::new(mlocate_bin())
        .arg("--database")
        .arg(db_path.to_str().unwrap())
        .arg("--modified")
        .arg("0s+")
        .arg("--plain")
        .arg("txt")
        .output()
        .expect("should run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.trim().lines().filter(|l| !l.is_empty()).collect();
    assert!(lines.is_empty(), "0s+ (older than 0s) should find nothing");
}

#[test]
fn test_json_and_null_output() {
    let tmp = tempfile::TempDir::new().expect("should create temp dir");
    let db_path = tmp.path().join("mlocate.db");
    let test_dir = tmp.path().join("testdata");
    std::fs::create_dir_all(&test_dir).unwrap();
    std::fs::write(test_dir.join("file.txt"), "content").unwrap();

    index_test_dir(&test_dir, &db_path);

    let output = Command::new(mlocate_bin())
        .arg("--database")
        .arg(db_path.to_str().unwrap())
        .arg("--json")
        .arg("file")
        .output()
        .expect("should run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"path\""));
    assert!(stdout.contains("file.txt"));

    let output = Command::new(mlocate_bin())
        .arg("--database")
        .arg(db_path.to_str().unwrap())
        .arg("--null")
        .arg("file")
        .output()
        .expect("should run");
    let stdout = output.stdout;
    assert!(stdout.ends_with(&[0u8]), "null output should end with NUL byte");
}

#[test]
fn test_count_with_json() {
    let tmp = tempfile::TempDir::new().expect("should create temp dir");
    let db_path = tmp.path().join("mlocate.db");
    let test_dir = tmp.path().join("testdata");
    std::fs::create_dir_all(&test_dir).unwrap();
    std::fs::write(test_dir.join("a.txt"), "a").unwrap();
    std::fs::write(test_dir.join("b.txt"), "b").unwrap();

    index_test_dir(&test_dir, &db_path);

    let output = Command::new(mlocate_bin())
        .arg("--database")
        .arg(db_path.to_str().unwrap())
        .arg("--count")
        .arg("--json")
        .arg("txt")
        .output()
        .expect("should run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"count\": 2"), "got: {}", stdout);
}

#[test]
fn test_regex_search() {
    let tmp = tempfile::TempDir::new().expect("should create temp dir");
    let db_path = tmp.path().join("mlocate.db");
    let test_dir = tmp.path().join("testdata");
    std::fs::create_dir_all(&test_dir).unwrap();
    std::fs::write(test_dir.join("hello.rs"), "fn main() {}").unwrap();
    std::fs::write(test_dir.join("hello.md"), "# hi").unwrap();
    std::fs::write(test_dir.join("world.txt"), "text").unwrap();

    index_test_dir(&test_dir, &db_path);

    let output = Command::new(mlocate_bin())
        .arg("--database")
        .arg(db_path.to_str().unwrap())
        .arg("-r")
        .arg(r"\.rs$")
        .arg("--plain")
        .output()
        .expect("should run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("hello.rs"), "regex .rs$ should match hello.rs, got: {}", stdout);
    assert!(!stdout.contains("hello.md"), "regex .rs$ should NOT match hello.md, got: {}", stdout);
}

#[test]
fn test_regex_case_insensitive() {
    let tmp = tempfile::TempDir::new().expect("should create temp dir");
    let db_path = tmp.path().join("mlocate.db");
    let test_dir = tmp.path().join("testdata");
    std::fs::create_dir_all(&test_dir).unwrap();
    std::fs::write(test_dir.join("README.md"), "# Test").unwrap();
    std::fs::write(test_dir.join("notes.txt"), "notes").unwrap();

    index_test_dir(&test_dir, &db_path);

    let output = Command::new(mlocate_bin())
        .arg("--database")
        .arg(db_path.to_str().unwrap())
        .arg("-r")
        .arg("-i")
        .arg("readme")
        .arg("--plain")
        .output()
        .expect("should run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("README.md"), "-i -r readme should match README.md, got: {}", stdout);
}
