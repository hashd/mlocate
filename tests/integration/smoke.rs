use std::process::Command;

fn mlocate_bin() -> String {
    std::env::var("CARGO_BIN_EXE_mlocate").unwrap_or_else(|_| "target/debug/mlocate".to_string())
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
        .arg("--prunepaths")
        .arg("/nonexistent-prune-path-mlocate")
        .arg("--force")
        .arg("--quiet")
        .output()
        .expect("should run mupdatedb");
    assert!(
        output.status.success(),
        "mupdatedb failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
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
        .arg("--prunepaths")
        .arg("/nonexistent-prune-path-mlocate")
        .arg("--quiet")
        .output()
        .expect("should run");
    assert!(
        output.status.success(),
        "--incremental should warn and fall back to full rebuild"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not supported")
            || stderr.contains("Falling back")
            || stderr.contains("No existing index")
    );
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
    assert!(
        output.status.success(),
        "search failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("hello.rs"),
        "Search should find hello.rs, got: {}",
        stdout
    );
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
    assert!(
        stdout.contains("doomed.txt"),
        "should find doomed.txt while it exists"
    );
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
    assert!(
        !stdout.contains("doomed.txt"),
        "deleted file should be excluded by --existing"
    );
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
    assert_eq!(
        lines.len(),
        2,
        "limit should cap results at 2, got {:?}",
        lines
    );
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
    assert_eq!(
        output.status.code(),
        Some(1),
        "empty DB should exit code 1 (no matches)"
    );
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
    assert!(
        !output.status.success(),
        "corrupted index should exit with error"
    );
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
    assert!(
        stdout.contains("recent.txt"),
        "1d- should find recently-indexed files"
    );
    assert!(
        stdout.contains("old.txt"),
        "both files should be within 1 day"
    );

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
    assert!(
        stdout.ends_with(&[0u8]),
        "null output should end with NUL byte"
    );
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
    assert!(stdout.contains("\"count\":2"), "got: {}", stdout);
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
    assert!(
        stdout.contains("hello.rs"),
        "regex .rs$ should match hello.rs, got: {}",
        stdout
    );
    assert!(
        !stdout.contains("hello.md"),
        "regex .rs$ should NOT match hello.md, got: {}",
        stdout
    );
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
    assert!(
        stdout.contains("README.md"),
        "-i -r readme should match README.md, got: {}",
        stdout
    );
}

#[test]
fn test_size_filter() {
    let tmp = tempfile::TempDir::new().expect("should create temp dir");
    let db_path = tmp.path().join("mlocate.db");
    let test_dir = tmp.path().join("testdata");
    std::fs::create_dir_all(&test_dir).unwrap();
    std::fs::write(test_dir.join("small.txt"), "x").unwrap();
    std::fs::write(test_dir.join("big.txt"), vec![0u8; 10000]).unwrap();

    index_test_dir(&test_dir, &db_path);

    let output = Command::new(mlocate_bin())
        .arg("--database")
        .arg(db_path.to_str().unwrap())
        .arg("--size")
        .arg("5KB+")
        .arg("--plain")
        .arg("txt")
        .output()
        .expect("should run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("big.txt"),
        "5KB+ should match big.txt, got: {}",
        stdout
    );
    assert!(
        !stdout.contains("small.txt"),
        "5KB+ should NOT match small.txt, got: {}",
        stdout
    );

    let output = Command::new(mlocate_bin())
        .arg("--database")
        .arg(db_path.to_str().unwrap())
        .arg("--size")
        .arg("1KB-")
        .arg("--plain")
        .arg("txt")
        .output()
        .expect("should run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("small.txt"),
        "1KB- should match small.txt, got: {}",
        stdout
    );
    assert!(
        !stdout.contains("big.txt"),
        "1KB- should NOT match big.txt, got: {}",
        stdout
    );
}

#[test]
fn test_type_filter() {
    let tmp = tempfile::TempDir::new().expect("should create temp dir");
    let db_path = tmp.path().join("mlocate.db");
    let test_dir = tmp.path().join("testdata");
    std::fs::create_dir_all(&test_dir).unwrap();
    std::fs::write(test_dir.join("hello.rs"), "fn main() {}").unwrap();
    std::fs::write(test_dir.join("readme.md"), "# Test").unwrap();
    std::fs::write(test_dir.join("notes.txt"), "notes").unwrap();

    index_test_dir(&test_dir, &db_path);

    let output = Command::new(mlocate_bin())
        .arg("--database")
        .arg(db_path.to_str().unwrap())
        .arg("-t")
        .arg("text/x-rust")
        .arg("--plain")
        .arg("")
        .output()
        .expect("should run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("hello.rs"),
        "type text/x-rust should match hello.rs, got: {}",
        stdout
    );
    assert!(
        !stdout.contains("readme.md"),
        "type text/x-rust should NOT match readme.md"
    );

    let output = Command::new(mlocate_bin())
        .arg("--database")
        .arg(db_path.to_str().unwrap())
        .arg("-t")
        .arg("text/*")
        .arg("--plain")
        .arg("")
        .output()
        .expect("should run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("hello.rs"), "text/* should match hello.rs");
    assert!(
        stdout.contains("readme.md"),
        "text/* should match readme.md"
    );
    assert!(
        stdout.contains("notes.txt"),
        "text/* should match notes.txt"
    );
}

fn run_mlocate(db: &std::path::Path, args: &[&str]) -> String {
    let output = run_mlocate_raw(db, args);
    String::from_utf8_lossy(&output).to_string()
}

fn run_mlocate_raw(db: &std::path::Path, args: &[&str]) -> Vec<u8> {
    let mut cmd = Command::new(mlocate_bin());
    cmd.arg("--database").arg(db);
    for a in args {
        cmd.arg(a);
    }
    cmd.output().unwrap().stdout
}

fn index_fixture(dir: &std::path::Path, db: &std::path::Path) {
    let mut cmd = Command::new(mupdatedb_bin());
    cmd.arg("--database")
        .arg(db)
        .arg("--localpaths")
        .arg(dir)
        .arg("--prunepaths")
        .arg("/nonexistent-prune-path-mlocate")
        .arg("--quiet");
    let output = cmd.output().unwrap();
    assert!(output.status.success(), "mupdatedb failed: {:?}", output);
}

#[test]
fn test_mime_filter_finds_image() {
    let tmp = tempfile::TempDir::new().unwrap();
    let fixture = std::path::Path::new("tests/fixtures/common-types");
    let db = tmp.path().join("test.db");
    let mut cmd = Command::new(mupdatedb_bin());
    cmd.arg("--database")
        .arg(&db)
        .arg("--localpaths")
        .arg(fixture)
        .arg("--quiet");
    let output = cmd.output().unwrap();
    assert!(output.status.success(), "mupdatedb failed: {:?}", output);

    let mut search = Command::new(mlocate_bin());
    search
        .arg("--database")
        .arg(&db)
        .arg("--type")
        .arg("image/*")
        .arg("--json")
        .arg("");
    let result = search.output().unwrap();
    assert!(
        result.status.success(),
        "mlocate search failed: {:?}",
        result
    );
    let stdout = String::from_utf8_lossy(&result.stdout);
    assert!(
        stdout.contains("image/png") || stdout.contains("image/"),
        "Expected image MIME in: {}",
        stdout
    );
}

#[test]
fn test_prunepaths_with_gitignore() {
    let tmp = tempfile::TempDir::new().unwrap();
    let fixture = std::path::Path::new("tests/fixtures/gitignore-test");
    let db = tmp.path().join("test.db");
    let mut cmd = Command::new(mupdatedb_bin());
    cmd.arg("--database")
        .arg(&db)
        .arg("--localpaths")
        .arg(fixture)
        .arg("--prunepaths")
        .arg(fixture.join("src"))
        .arg("--quiet");
    let output = cmd.output().unwrap();
    assert!(output.status.success(), "mupdatedb failed: {:?}", output);

    let mut search = Command::new(mlocate_bin());
    search
        .arg("--database")
        .arg(&db)
        .arg("-i")
        .arg("Cargo.toml")
        .arg("--json");
    let result = search.output().unwrap();
    let stdout = String::from_utf8_lossy(&result.stdout);
    assert!(
        stdout.contains("Cargo.toml"),
        "Cargo.toml should be indexed"
    );
    assert!(!stdout.contains("src/main.rs"), "src/ should be pruned");
}

#[test]
fn test_symlink_handling() {
    let tmp = tempfile::TempDir::new().unwrap();
    let fixture = std::path::Path::new("tests/fixtures/symlink-test");
    let db = tmp.path().join("test.db");
    let mut cmd = Command::new(mupdatedb_bin());
    cmd.arg("--database")
        .arg(&db)
        .arg("--localpaths")
        .arg(fixture)
        .arg("--quiet");
    let output = cmd.output().unwrap();
    assert!(output.status.success(), "mupdatedb failed: {:?}", output);

    let mut search = Command::new(mlocate_bin());
    search
        .arg("--database")
        .arg(&db)
        .arg("link-to-file")
        .arg("--json");
    let result = search.output().unwrap();
    let stdout = String::from_utf8_lossy(&result.stdout);
    assert!(
        stdout.trim().starts_with("[]"),
        "Symlinks should not be indexed by default, got: {}",
        stdout
    );
}

#[test]
fn test_mupdatedb_incremental_updates_file() {
    let tmp = tempfile::TempDir::new().unwrap();
    let file_path = tmp.path().join("original.txt");
    std::fs::write(&file_path, "hello").unwrap();

    let db = tmp.path().join("mlocate.db");

    let mut cmd = Command::new(mupdatedb_bin());
    cmd.arg("--database")
        .arg(&db)
        .arg("--localpaths")
        .arg(tmp.path())
        .arg("--prunepaths")
        .arg("/nonexistent-prune-path-mlocate")
        .arg("--quiet");
    assert!(cmd.status().unwrap().success());

    let result = run_mlocate(&db, &["original.txt"]);
    assert!(
        result.contains("original.txt"),
        "Original file should be found"
    );

    std::fs::write(tmp.path().join("newfile.txt"), "world").unwrap();

    let mut cmd2 = Command::new(mupdatedb_bin());
    cmd2.arg("--database")
        .arg(&db)
        .arg("--localpaths")
        .arg(tmp.path())
        .arg("--prunepaths")
        .arg("/nonexistent-prune-path-mlocate")
        .arg("--quiet")
        .arg("--incremental");
    assert!(cmd2.status().unwrap().success());

    let result2 = run_mlocate(&db, &["newfile.txt"]);
    assert!(
        result2.contains("newfile.txt"),
        "New file should be found after incremental"
    );
    let result3 = run_mlocate(&db, &["original.txt"]);
    assert!(
        result3.contains("original.txt"),
        "Original file should still be found"
    );
}

#[test]
fn test_basename_flag() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(tmp.path().join("sub")).unwrap();
    std::fs::write(tmp.path().join("sub/hello.txt"), "data").unwrap();
    std::fs::write(tmp.path().join("goodbye.txt"), "data").unwrap();

    let db = tmp.path().join("mlocate.db");
    index_fixture(tmp.path(), &db);

    let result = run_mlocate(&db, &["--basename", "hello"]);
    assert!(
        result.contains("hello.txt"),
        "Basename should match: {}",
        result
    );
    assert!(!result.contains("goodbye.txt"), "Should not match goodbye");
}

#[test]
fn test_multi_pattern_or() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("a.txt"), "a").unwrap();
    std::fs::write(tmp.path().join("b.txt"), "b").unwrap();
    std::fs::write(tmp.path().join("c.txt"), "c").unwrap();

    let db = tmp.path().join("mlocate.db");
    index_fixture(tmp.path(), &db);

    let result = run_mlocate(&db, &["a.txt", "b.txt"]);
    assert!(result.contains("a.txt"), "Should contain a");
    assert!(result.contains("b.txt"), "Should contain b");
    assert!(!result.contains("c.txt"), "Should not contain c");
}

#[test]
fn test_null_output() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("test.txt"), "data").unwrap();

    let db = tmp.path().join("mlocate.db");
    index_fixture(tmp.path(), &db);

    let output = run_mlocate_raw(&db, &["--null", "test"]);
    assert!(
        output.ends_with(b"\0"),
        "NUL output should end with null byte"
    );
}

#[test]
fn test_color_never() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("test.txt"), "data").unwrap();
    let db = tmp.path().join("mlocate.db");
    index_fixture(tmp.path(), &db);
    let output = run_mlocate_raw(&db, &["--color=never", "test"]);
    assert!(!output.is_empty());
}

#[test]
fn test_schema_output() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("test.txt"), "data").unwrap();
    let db = tmp.path().join("mlocate.db");
    index_fixture(tmp.path(), &db);

    let output = run_mlocate_raw(&db, &["--statistics"]);
    let stdout = String::from_utf8_lossy(&output);
    assert!(
        stdout.contains("\"schema_version\""),
        "Schema should have schema_version: {}",
        stdout
    );
    assert!(
        stdout.contains("\"format_version\""),
        "Schema should have format_version: {}",
        stdout
    );
}

#[test]
fn test_no_magic_mime_flag() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("test.txt"), "hello world").unwrap();

    let db = tmp.path().join("mlocate.db");
    let mut cmd = Command::new(mupdatedb_bin());
    cmd.arg("--database")
        .arg(&db)
        .arg("--localpaths")
        .arg(tmp.path())
        .arg("--prunepaths")
        .arg("/nonexistent-prune-path-mlocate")
        .arg("--quiet")
        .arg("--no-magic-mime");
    let output = cmd.output().unwrap();
    assert!(
        output.status.success(),
        "--no-magic-mime should not crash: {:?}",
        output
    );
}
