use mlocate::db::query::build_query;
use mlocate::filter;

#[test]
fn test_simple_substring_query() {
    let q = build_query(
        &["foo".to_string()],
        false, false, false, false,
        None, false,
        None, None, None,
    );
    assert!(q.sql.contains("full_path LIKE"));
    assert!(q.sql.contains("trigrams WHERE trigram"));
    assert_eq!(q.params.len(), 2);
}

#[test]
fn test_regex_query() {
    let q = build_query(
        &[r"\.rs$".to_string()],
        false, false, true, false,
        None, false,
        None, None, None,
    );
    assert!(q.sql.contains("regexp_matches"));
}

#[test]
fn test_or_patterns() {
    let q = build_query(
        &["foo".to_string(), "bar".to_string()],
        false, false, false, false,
        None, false,
        None, None, None,
    );
    assert!(q.sql.contains("OR"));
    assert_eq!(q.params.len(), 4);
}

#[test]
fn test_count_query() {
    let q = build_query(
        &["foo".to_string()],
        false, false, false, false,
        None, true,
        None, None, None,
    );
    assert!(q.sql.starts_with("SELECT COUNT(*)"));
    assert!(!q.sql.contains("LIMIT"));
}

#[test]
fn test_limit_query() {
    let q = build_query(
        &["foo".to_string()],
        false, false, false, false,
        Some(50), false,
        None, None, None,
    );
    assert!(q.sql.contains("LIMIT 50"));
}

#[test]
fn test_size_filter() {
    let sf = filter::parse_size("10MB+").unwrap();
    let q = build_query(
        &["foo".to_string()],
        false, false, false, false,
        None, false,
        Some(&sf), None, None,
    );
    assert!(q.sql.contains("size >="));
}

#[test]
fn test_modified_filter() {
    let mf = filter::parse_modified("2d-").unwrap();
    let q = build_query(
        &["foo".to_string()],
        false, false, false, false,
        None, false,
        None, Some(&mf), None,
    );
    assert!(q.sql.contains("mtime >="));
}

#[test]
fn test_mime_filter() {
    let mime = filter::parse_mime_type("image/png").unwrap();
    let q = build_query(
        &["foo".to_string()],
        false, false, false, false,
        None, false,
        None, None, Some(&mime),
    );
    assert!(q.sql.contains("mime_type ="));
}

#[test]
fn test_case_insensitive() {
    let q = build_query(
        &["foo".to_string()],
        true, false, false, false,
        None, false,
        None, None, None,
    );
    assert!(q.sql.contains("LOWER"));
}

#[test]
fn test_basename() {
    let q = build_query(
        &["foo".to_string()],
        false, true, false, false,
        None, false,
        None, None, None,
    );
    assert!(q.params.iter().any(|p| {
        format!("{:?}", p).contains("%/")
    }));
}
