use mlocate::db::{open_or_create, schema};

#[test]
fn test_create_in_memory_db() {
    let conn = open_or_create(":memory:").expect("should create in-memory DB");
    let version: i32 = conn
        .query_row(schema::GET_USER_VERSION, [], |row| row.get(0))
        .expect("should read version");
    assert_eq!(version, schema::SCHEMA_VERSION);
}

#[test]
fn test_tables_exist() {
    let conn = open_or_create(":memory:").expect("should create in-memory DB");
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('directories', 'files', 'trigrams')",
            [],
            |row| row.get(0),
        )
        .expect("should count tables");
    assert_eq!(count, 3);
}

#[test]
fn test_trigram_index_exists() {
    let conn = open_or_create(":memory:").expect("should create in-memory DB");
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_trigrams_trigram'",
            [],
            |row| row.get(0),
        )
        .expect("should count indexes");
    assert_eq!(count, 1);
}

#[test]
fn test_schema_version_mismatch_with_file() {
    let dir = tempfile::tempdir().expect("should create temp dir");
    let db_path = dir.path().join("test.db");
    let db_path_str = db_path.to_str().unwrap();

    {
        let conn = duckdb::Connection::open(db_path_str).unwrap();
        conn.execute_batch(schema::CREATE_DIRECTORIES).unwrap();
        conn.execute_batch(schema::CREATE_FILES).unwrap();
        conn.execute_batch(schema::CREATE_TRIGRAMS).unwrap();
        conn.execute_batch(schema::CREATE_SCHEMA_VERSION).unwrap();
        conn.execute_batch("INSERT INTO schema_version (version) VALUES (999);").unwrap();
    }

    let result = open_or_create(db_path_str);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("999"));
    assert!(err.contains("1"));
}
