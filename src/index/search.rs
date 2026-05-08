use std::collections::HashMap;
use regex::Regex;
use roaring::RoaringBitmap;

use crate::error::MlocateError;
use crate::filter::{CmpOp, MimeFilter, ModifiedFilter, SizeFilter};

use super::format::{DiskFileEntry, IndexReader};
use super::trigram;

#[derive(Debug, Clone, Default)]
pub struct SearchOptions {
    pub ignore_case: bool,
    pub basename: bool,
    pub regex: bool,
    pub existing: bool,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Default)]
pub struct SearchFilters {
    pub size: Option<SizeFilter>,
    pub modified: Option<ModifiedFilter>,
    pub mime: Option<MimeFilter>,
}

pub struct SearchResults<'a> {
    reader: &'a IndexReader<'a>,
    candidate_ids: RoaringBitmap,
    patterns: Vec<String>,
    options: SearchOptions,
    filters: SearchFilters,
    position: usize,
    emitted: usize,
    now: i64,
}

impl<'a> Iterator for SearchResults<'a> {
    type Item = Result<DiskFileEntry, MlocateError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.position >= self.candidate_ids.len() as usize {
                return None;
            }
            if let Some(limit) = self.options.limit {
                if self.emitted >= limit {
                    return None;
                }
            }

            let doc_id = self.candidate_ids.select(self.position as u32)?;
            self.position += 1;

            let entry = match self.reader.file_entry(doc_id) {
                Ok(e) => e,
                Err(e) => return Some(Err(MlocateError::IndexFormatError { details: e })),
            };

            if !self.options.regex && !self.patterns.is_empty() && !self.patterns.iter().any(|pat| path_matches(&entry.full_path, pat, &self.options)) {
                continue;
            }

            if !filter_matches(&entry, &self.filters, self.now) {
                continue;
            }

            if self.options.existing && !std::path::Path::new(&entry.full_path).exists() {
                continue;
            }

            self.emitted += 1;
            return Some(Ok(entry));
        }
    }
}

fn path_matches(path: &str, pattern: &str, opts: &SearchOptions) -> bool {
    let target = if opts.basename {
        match path.rfind('/') {
            Some(idx) => &path[idx + 1..],
            None => path,
        }
    } else {
        path
    };

    if opts.ignore_case {
        target.to_ascii_lowercase().contains(&pattern.to_ascii_lowercase())
    } else {
        target.contains(pattern)
    }
}

fn filter_matches(entry: &DiskFileEntry, filters: &SearchFilters, now: i64) -> bool {
    if let Some(ref sf) = filters.size {
        let ok = match sf.operator {
            CmpOp::Eq => entry.size == sf.bytes,
            CmpOp::Ge => entry.size >= sf.bytes,
            CmpOp::Le => entry.size <= sf.bytes,
        };
        if !ok { return false; }
    }

    if let Some(ref mf) = filters.modified {
        let cutoff = now - mf.seconds;
        let ok = match mf.operator {
            CmpOp::Ge => entry.mtime >= cutoff,
            CmpOp::Le => entry.mtime <= cutoff,
            CmpOp::Eq => entry.mtime == cutoff,
        };
        if !ok { return false; }
    }

    if let Some(ref mime) = filters.mime {
        let ok = if mime.is_glob {
            let glob = mime.pattern.replace('*', "");
            entry.mime_type.starts_with(&glob)
        } else {
            entry.mime_type == mime.pattern
        };
        if !ok { return false; }
    }

    true
}

pub fn search<'a>(
    reader: &'a IndexReader<'a>,
    patterns: &[String],
    options: &SearchOptions,
    filters: &SearchFilters,
) -> Result<SearchResults<'a>, MlocateError> {
    let now = chrono::Utc::now().timestamp();

    let empty_result = SearchResults {
        reader,
        candidate_ids: RoaringBitmap::new(),
        patterns: patterns.to_vec(),
        options: options.clone(),
        filters: filters.clone(),
        position: 0,
        emitted: 0,
        now,
    };

    if reader.num_files() == 0 {
        return Ok(empty_result);
    }

    let candidate_ids: RoaringBitmap = if options.regex {
        let regexes: Vec<Regex> = patterns.iter().map(|p| {
            let pattern = if options.ignore_case {
                format!("(?i){}", p)
            } else {
                p.clone()
            };
            Regex::new(&pattern).map_err(|e| MlocateError::Other(format!("Invalid regex '{}': {}", p, e)))
        }).collect::<Result<Vec<_>, _>>()?;

        let mut bm = RoaringBitmap::new();
        for i in 0..reader.num_files() as u32 {
            let path = reader.file_path(i).map_err(|e| MlocateError::IndexFormatError { details: e })?;
            let target = if options.basename {
                match path.rfind('/') {
                    Some(idx) => &path[idx + 1..],
                    None => &path,
                }
            } else {
                &path
            };
            if regexes.iter().any(|r| r.is_match(target)) {
                bm.insert(i);
            }
            if let Some(limit) = options.limit {
                if bm.len() as usize >= limit {
                    break;
                }
            }
        }
        bm
    } else if patterns.is_empty() {
        (0..reader.num_files() as u32).collect()
    } else {
        let mut trigram_cache: HashMap<[u8; 3], RoaringBitmap> = HashMap::new();
        let mut candidates: Option<RoaringBitmap> = None;
        let mut attempted_trigrams = false;

        for pattern in patterns {
            if pattern.len() < 3 {
                continue;
            }
            let tris = trigram::generate_trigrams_lowercase(pattern);
            if tris.is_empty() {
                continue;
            }

            attempted_trigrams = true;

            let mut pattern_and: Option<RoaringBitmap> = None;
            for tri in &tris {
                let key = trigram::trigram_to_bytes(tri);
                let bm = match trigram_cache.get(&key) {
                    Some(bm) => bm.clone(),
                    None => {
                        let bm = reader.trigram_bitmap(key).unwrap_or_default();
                        trigram_cache.insert(key, bm.clone());
                        bm
                    }
                };
                pattern_and = match pattern_and {
                    None => Some(bm),
                    Some(existing) => Some(existing & bm),
                };
            }

            if let Some(ids) = pattern_and {
                candidates = match candidates {
                    None => Some(ids),
                    Some(existing) => Some(existing | ids),
                };
            }
        }

        if !attempted_trigrams {
            let all: RoaringBitmap = (0..reader.num_files() as u32).collect();
            candidates = Some(all);
        }

        candidates.unwrap_or_default()
    };

    Ok(SearchResults {
        reader,
        candidate_ids,
        patterns: patterns.to_vec(),
        options: options.clone(),
        filters: filters.clone(),
        position: 0,
        emitted: 0,
        now,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::format::IndexWriter;
    use crate::filter::{self};

    struct TestData {
        bytes: Vec<u8>,
    }

    impl TestData {
        fn read<'a>(&'a self) -> IndexReader<'a> {
            IndexReader::from_bytes(&self.bytes).unwrap()
        }
    }

    fn build_test_data() -> TestData {
        let mut writer = IndexWriter::new();
        let files = vec![
            ("/home/user/foo.rs", 1234i64, 1746720000i64, 0o644u32, "text/x-rust"),
            ("/home/user/bar.txt", 5678i64, 1746720100i64, 0o644u32, "text/plain"),
            ("/etc/config.toml", 100i64, 1746700000i64, 0o600u32, "application/octet-stream"),
            ("/home/user/README.md", 2000i64, 1746800000i64, 0o644u32, "text/markdown"),
            ("/var/log/syslog", 500000i64, 1746600000i64, 0o600u32, "text/plain"),
        ];
        for (path, size, mtime, mode, mime) in &files {
            let id = writer.add_file(path, *size as u64, *mtime, *mode, mime);
            let tris = trigram::generate_trigrams_lowercase(path);
            for tri in &tris {
                writer.add_trigram_doc(trigram::trigram_to_bytes(tri), id);
            }
        }
        writer.prune_empty();
        let config = super::super::format::IndexConfig {
            indexed_paths: vec![],
            pruned_paths: vec![],
            timestamp: 0,
            hostname: "test".into(),
            total_bytes_indexed: 0,
            mlocate_version: "0.1.0".into(),
        };
        let bytes = writer.into_bytes(&config).unwrap();
        TestData { bytes }
    }

    #[test]
    fn test_search_finds_foo() {
        let data = build_test_data();
        let reader = data.read();
        let results = search(
            &reader,
            &["foo".to_string()],
            &SearchOptions::default(),
            &SearchFilters::default(),
        ).unwrap();
        let paths: Vec<String> = results.map(|r| r.unwrap().full_path).collect();
        assert_eq!(paths, vec!["/home/user/foo.rs"]);
    }

    #[test]
    fn test_search_case_insensitive() {
        let data = build_test_data();
        let reader = data.read();
        let results = search(
            &reader,
            &["README".to_string()],
            &SearchOptions { ignore_case: true, ..Default::default() },
            &SearchFilters::default(),
        ).unwrap();
        let paths: Vec<String> = results.map(|r| r.unwrap().full_path).collect();
        assert_eq!(paths, vec!["/home/user/README.md"]);
    }

    #[test]
    fn test_search_basename() {
        let data = build_test_data();
        let reader = data.read();
        let results = search(
            &reader,
            &["syslog".to_string()],
            &SearchOptions { basename: true, ..Default::default() },
            &SearchFilters::default(),
        ).unwrap();
        let paths: Vec<String> = results.map(|r| r.unwrap().full_path).collect();
        assert_eq!(paths, vec!["/var/log/syslog"]);
    }

    #[test]
    fn test_search_short_pattern() {
        let data = build_test_data();
        let reader = data.read();
        let results = search(
            &reader,
            &["rs".to_string()],
            &SearchOptions::default(),
            &SearchFilters::default(),
        ).unwrap();
        let paths: Vec<String> = results.map(|r| r.unwrap().full_path).collect();
        assert_eq!(paths, vec!["/home/user/foo.rs"]);
    }

    #[test]
    fn test_search_mime_filter() {
        let data = build_test_data();
        let reader = data.read();
        let results = search(
            &reader,
            &[".md".to_string()],
            &SearchOptions::default(),
            &SearchFilters {
                mime: Some(filter::parse_mime_type("text/markdown").unwrap()),
                ..Default::default()
            },
        ).unwrap();
        let paths: Vec<String> = results.map(|r| r.unwrap().full_path).collect();
        assert_eq!(paths, vec!["/home/user/README.md"]);
    }

    #[test]
    fn test_search_limit() {
        let data = build_test_data();
        let reader = data.read();
        let results = search(
            &reader,
            &["user".to_string()],
            &SearchOptions { limit: Some(2), ..Default::default() },
            &SearchFilters::default(),
        ).unwrap();
        let paths: Vec<String> = results.map(|r| r.unwrap().full_path).collect();
        assert_eq!(paths.len(), 2);
    }

    #[test]
    fn test_search_empty_patterns_filters_only() {
        let data = build_test_data();
        let reader = data.read();
        let results = search(
            &reader,
            &[],
            &SearchOptions::default(),
            &SearchFilters {
                size: Some(SizeFilter { bytes: 500000, operator: CmpOp::Ge }),
                ..Default::default()
            },
        ).unwrap();
        let paths: Vec<String> = results.map(|r| r.unwrap().full_path).collect();
        assert_eq!(paths, vec!["/var/log/syslog"]);
    }

    #[test]
    fn test_search_regex() {
        let data = build_test_data();
        let reader = data.read();
        let results = search(
            &reader,
            &[r"\.rs$".to_string()],
            &SearchOptions { regex: true, ..Default::default() },
            &SearchFilters::default(),
        ).unwrap();
        let paths: Vec<String> = results.map(|r| r.unwrap().full_path).collect();
        assert_eq!(paths, vec!["/home/user/foo.rs"]);
    }

    #[test]
    fn test_search_regex_case_insensitive() {
        let data = build_test_data();
        let reader = data.read();
        let results = search(
            &reader,
            &[r"README".to_string()],
            &SearchOptions { regex: true, ignore_case: true, ..Default::default() },
            &SearchFilters::default(),
        ).unwrap();
        let paths: Vec<String> = results.map(|r| r.unwrap().full_path).collect();
        assert_eq!(paths, vec!["/home/user/README.md"]);
    }

    #[test]
    fn test_search_multi_pattern_or() {
        let data = build_test_data();
        let reader = data.read();
        let results = search(
            &reader,
            &["foo".to_string(), "syslog".to_string()],
            &SearchOptions::default(),
            &SearchFilters::default(),
        ).unwrap();
        let paths: Vec<String> = results.map(|r| r.unwrap().full_path).collect();
        assert_eq!(paths, vec!["/home/user/foo.rs", "/var/log/syslog"]);
    }
}
