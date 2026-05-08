use super::trigram;
use crate::filter::{CmpOp, MimeFilter, ModifiedFilter, SizeFilter};

pub struct SearchQuery {
    pub sql: String,
    pub params: Vec<String>,
}

#[allow(clippy::too_many_arguments)]
pub fn build_query(
    patterns: &[String],
    ignore_case: bool,
    basename: bool,
    regex: bool,
    existing: bool,
    limit: Option<usize>,
    count: bool,
    size_filter: Option<&SizeFilter>,
    modified_filter: Option<&ModifiedFilter>,
    mime_filter: Option<&MimeFilter>,
) -> SearchQuery {
    let mut parts: Vec<String> = Vec::new();
    let mut params: Vec<String> = Vec::new();

    for pattern in patterns {
        if regex {
            parts.push("id IN (SELECT id FROM files WHERE regexp_matches(full_path, ?))".to_string());
            params.push(pattern.clone());
        } else if let Some(trigram_sql) = trigram::build_trigram_query(pattern, ignore_case, basename) {
            let sql = format!("id IN ({})", trigram_sql);
            for tri in trigram::generate_trigrams_lowercase(pattern) {
                params.push(tri);
            }
            let like_pattern = if basename {
                if ignore_case {
                    format!("%%/{pattern}%%", pattern = pattern.to_ascii_lowercase())
                } else {
                    format!("%%/{pattern}%%")
                }
            } else if ignore_case {
                format!("%%{pattern}%%", pattern = pattern.to_ascii_lowercase())
            } else {
                format!("%%{pattern}%%")
            };
            params.push(like_pattern);
            parts.push(sql);
        } else {
            if ignore_case {
                parts.push("id IN (SELECT id FROM files WHERE LOWER(full_path) LIKE LOWER(?))".to_string());
            } else {
                parts.push("id IN (SELECT id FROM files WHERE full_path LIKE ?)".to_string());
            }
            let like_pattern = if basename {
                if ignore_case {
                    format!("%%/{pattern}%%", pattern = pattern.to_ascii_lowercase())
                } else {
                    format!("%%/{pattern}%%")
                }
            } else if ignore_case {
                format!("%%{pattern}%%", pattern = pattern.to_ascii_lowercase())
            } else {
                format!("%%{pattern}%%")
            };
            params.push(like_pattern);
        }
    }

    let mut where_clause = if parts.len() == 1 {
        parts.pop().unwrap()
    } else {
        format!("({})", parts.join(" OR "))
    };

    if let Some(sf) = size_filter {
        let op = match sf.operator {
            CmpOp::Eq => "=",
            CmpOp::Ge => ">=",
            CmpOp::Le => "<=",
        };
        where_clause.push_str(&format!(" AND size {} {}", op, sf.bytes));
    }

    if let Some(mf) = modified_filter {
        let cutoff = chrono::Utc::now().timestamp() - mf.seconds;
        let op = match mf.operator {
            CmpOp::Eq => "=",
            CmpOp::Ge => "<=",
            CmpOp::Le => ">=",
        };
        where_clause.push_str(&format!(" AND mtime {} {}", op, cutoff));
    }

    if let Some(mime) = mime_filter {
        if mime.is_glob {
            let like_val = mime.pattern.replace('*', "%");
            where_clause.push_str(&format!(" AND mime_type LIKE '{}'", like_val));
        } else {
            where_clause.push_str(&format!(" AND mime_type = '{}'", mime.pattern));
        }
    }

    if existing {
        where_clause.push_str(" AND 1=1 /* existing check done in Rust */");
    }

    let select = if count {
        "SELECT COUNT(*) FROM files".to_string()
    } else {
        "SELECT id, full_path, size, mtime, mode, mime_type FROM files".to_string()
    };

    let mut sql = format!("{} WHERE {}", select, where_clause);

    if let Some(n) = limit {
        if !count {
            sql.push_str(&format!(" LIMIT {}", n));
        }
    }

    SearchQuery { sql, params }
}
