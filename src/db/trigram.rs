pub fn generate_trigrams(path: &str) -> Vec<String> {
    let s = if path.len() < 3 {
        let padded = format!("_{}_", path);
        padded.chars().take(3).collect::<String>()
    } else {
        path.to_string()
    };

    let chars: Vec<char> = s.chars().collect();
    if chars.len() < 3 {
        return vec![s];
    }

    let mut trigrams = Vec::with_capacity(chars.len() - 2);
    for i in 0..=(chars.len() - 3) {
        let tri: String = chars[i..i + 3].iter().collect();
        trigrams.push(tri);
    }

    trigrams.sort();
    trigrams.dedup();

    trigrams
}

pub fn generate_trigrams_lowercase(path: &str) -> Vec<String> {
    let lowered: String = path.chars().map(|c| c.to_ascii_lowercase()).collect();
    generate_trigrams(&lowered)
}

pub fn build_trigram_query(
    pattern: &str,
    ignore_case: bool,
    basename: bool,
) -> Option<String> {
    if pattern.len() < 3 {
        return None;
    }

    let trigrams = if ignore_case {
        generate_trigrams_lowercase(pattern)
    } else {
        generate_trigrams(pattern)
    };

    if trigrams.is_empty() {
        return None;
    }

    if trigrams.len() == 1 {
        let condition = if basename {
            if ignore_case {
                "LOWER(full_path) LIKE LOWER(?)"
            } else {
                "full_path LIKE ?"
            }
        } else if ignore_case {
            "LOWER(full_path) LIKE LOWER(?)"
        } else {
            "full_path LIKE ?"
        };

        return Some(format!(
            "SELECT f.id FROM files f JOIN (SELECT file_id FROM trigrams WHERE trigram = ?) t \
             ON f.id = t.file_id WHERE {}",
            condition
        ));
    }

    let intersect_clauses: Vec<String> = trigrams
        .iter()
        .map(|_| "SELECT file_id FROM trigrams WHERE trigram = ?".to_string())
        .collect();
    let intersect_sql = intersect_clauses.join(" INTERSECT ");

    let final_condition = if basename {
        if ignore_case {
            "LOWER(full_path) LIKE LOWER(?) ESCAPE '\\'".to_string()
        } else {
            "full_path LIKE ? ESCAPE '\\'".to_string()
        }
    } else if ignore_case {
        "LOWER(full_path) LIKE LOWER(?) ESCAPE '\\'".to_string()
    } else {
        "full_path LIKE ? ESCAPE '\\'".to_string()
    };

    Some(format!(
        "SELECT f.id FROM files f WHERE f.id IN ({}) AND {}",
        intersect_sql, final_condition
    ))
}

pub fn escape_like(pattern: &str) -> String {
    pattern
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_trigrams_basic() {
        let tris = generate_trigrams("foo");
        assert_eq!(tris, vec!["foo"]);
    }

    #[test]
    fn test_generate_trigrams_longer() {
        let tris = generate_trigrams("foobar");
        assert_eq!(tris, vec!["bar", "foo", "oba", "oob"]);
    }

    #[test]
    fn test_generate_trigrams_short() {
        let tris = generate_trigrams("/a");
        assert_eq!(tris, vec!["_/a"]);
    }

    #[test]
    fn test_generate_trigrams_two_char() {
        let tris = generate_trigrams("/ab");
        assert_eq!(tris, vec!["/ab"]);
    }

    #[test]
    fn test_generate_trigrams_dedup() {
        let tris = generate_trigrams("/aaa");
        assert_eq!(tris, vec!["/aa", "aaa"]);
    }

    #[test]
    fn test_generate_trigrams_lowercase() {
        let tris = generate_trigrams_lowercase("README.md");
        assert_eq!(tris, vec![".md", "adm", "dme", "e.m", "ead", "me.", "rea"]);
    }

    #[test]
    fn test_escape_like() {
        assert_eq!(escape_like("foo%bar"), "foo\\%bar");
        assert_eq!(escape_like("foo_bar"), "foo\\_bar");
        assert_eq!(escape_like("foo\\bar"), "foo\\\\bar");
    }
}
