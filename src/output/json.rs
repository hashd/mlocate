#[derive(serde::Serialize)]
pub struct JsonFileEntry {
    pub path: String,
    pub size: i64,
    pub mtime: i64,
    pub mode: i32,
    pub mime_type: String,
}

pub fn render_json(paths: &[JsonFileEntry]) -> String {
    serde_json::to_string_pretty(paths).unwrap_or_else(|_| "[]".to_string())
}

pub fn render_json_count(count: i64) -> String {
    format!("{{\"count\": {}}}", count)
}

pub fn render_json_schema(
    db_path: &str,
    db_size_bytes: u64,
    indexed_files: i64,
    last_indexed: &str,
) -> String {
    let schema = serde_json::json!({
        "database": db_path,
        "db_size_bytes": db_size_bytes,
        "db_size_human": crate::output::human::format_size(db_size_bytes as i64),
        "indexed_files": indexed_files,
        "last_indexed": last_indexed,
        "schema_version": crate::db::schema::SCHEMA_VERSION,
        "columns": [
            {"name": "full_path", "type": "TEXT", "filterable": true, "description": "Absolute file path"},
            {"name": "size", "type": "INTEGER", "filterable": true, "description": "File size in bytes"},
            {"name": "mtime", "type": "INTEGER", "filterable": true, "description": "Modification time (unix timestamp)"},
            {"name": "mode", "type": "INTEGER", "filterable": false, "description": "File mode/permissions"},
            {"name": "mime_type", "type": "TEXT", "filterable": true, "description": "MIME type (e.g., text/plain)"}
        ]
    });
    serde_json::to_string_pretty(&schema).unwrap_or_else(|_| "{}".to_string())
}
