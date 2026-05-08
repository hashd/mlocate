use std::collections::HashSet;
use regex::Regex;

use crate::error::MlocateError;
use crate::filter::{CmpOp, MimeFilter, ModifiedFilter, SizeFilter};

use super::format::{DiskFileEntry, IndexReader};
use super::trigram;

#[derive(Debug, Clone)]
pub struct SearchOptions {
    pub ignore_case: bool,
    pub basename: bool,
    pub regex: bool,
    pub existing: bool,
    pub limit: Option<usize>,
}

impl Default for SearchOptions {
    fn default() -> Self {
        SearchOptions {
            ignore_case: false,
            basename: false,
            regex: false,
            existing: false,
            limit: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SearchFilters {
    pub size: Option<SizeFilter>,
    pub modified: Option<ModifiedFilter>,
    pub mime: Option<MimeFilter>,
}

pub struct SearchResults<'a> {
    reader: &'a IndexReader<'a>,
    candidate_ids: Vec<u32>,
    patterns: Vec<String>,
    options: SearchOptions,
    filters: SearchFilters,
    position: usize,
    emitted: usize,
}

impl<'a> Iterator for SearchResults<'a> {
    type Item = Result<DiskFileEntry, MlocateError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.position >= self.candidate_ids.len() {
                return None;
            }
            if let Some(limit) = self.options.limit {
                if self.emitted >= limit {
                    return None;
                }
            }

            let doc_id = self.candidate_ids[self.position];
            self.position += 1;

            let entry = match self.reader.file_entry(doc_id) {
                Ok(e) => e,
                Err(e) => return Some(Err(MlocateError::IndexFormatError { details: e })),
            };

            if !self.patterns.is_empty() && !self.patterns.iter().any(|pat| path_matches(&entry.full_path, pat, &self.options)) {
                continue;
            }

            if !filter_matches(&entry, &self.filters) {
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

fn filter_matches(entry: &DiskFileEntry, filters: &SearchFilters) -> bool {
    if let Some(ref sf) = filters.size {
        let ok = match sf.operator {
            CmpOp::Eq => entry.size == sf.bytes,
            CmpOp::Ge => entry.size >= sf.bytes,
            CmpOp::Le => entry.size <= sf.bytes,
        };
        if !ok { return false; }
    }

    if let Some(ref mf) = filters.modified {
        let cutoff = (chrono::Utc::now().timestamp() - mf.seconds) as i64;
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
    if reader.num_files() == 0 {
        return Ok(SearchResults {
            reader,
            candidate_ids: Vec::new(),
            patterns: patterns.to_vec(),
            options: options.clone(),
            filters: filters.clone(),
            position: 0,
            emitted: 0,
        });
    }

    let candidate_ids: Vec<u32> = if options.regex {
        let regexes: Vec<Regex> = patterns.iter().map(|p| {
            Regex::new(p).map_err(|e| MlocateError::Other(format!("Invalid regex '{}': {}", p, e)))
        }).collect::<Result<Vec<_>, _>>()?;

        let mut ids = Vec::new();
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
                ids.push(i);
            }
            if let Some(limit) = options.limit {
                if ids.len() >= limit {
                    break;
                }
            }
        }
        ids
    } else if patterns.is_empty() {
        (0..reader.num_files() as u32).collect()
    } else {
        let mut candidates: Option<HashSet<u32>> = None;

        for pattern in patterns {
            if pattern.len() < 3 {
                continue;
            }
            let tris = trigram::generate_trigrams_lowercase(pattern);
            if tris.is_empty() {
                continue;
            }

            let mut pattern_and: Option<Vec<u32>> = None;
            for tri in &tris {
                let key = trigram::trigram_to_bytes(tri);
                if let Some(bm) = reader.trigram_bitmap(key) {
                    let ids: Vec<u32> = bm.iter().collect();
                    pattern_and = match pattern_and {
                        None => Some(ids),
                        Some(existing) => {
                            let set: HashSet<u32> = existing.into_iter().collect();
                            Some(ids.into_iter().filter(|id| set.contains(id)).collect())
                        }
                    };
                }
            }

            if let Some(ids) = pattern_and {
                let mut union = candidates.unwrap_or_default();
                for id in ids {
                    union.insert(id);
                }
                candidates = Some(union);
            }
        }

        if patterns.iter().any(|p| p.len() < 3) {
            let all_ids: HashSet<u32> = (0..reader.num_files() as u32).collect();
            candidates = match candidates {
                Some(mut c) => { c.extend(all_ids); Some(c) }
                None => Some(all_ids),
            };
        }

        let mut ids: Vec<u32> = candidates.unwrap_or_default().into_iter().collect();
        ids.sort_unstable();
        ids
    };

    Ok(SearchResults {
        reader,
        candidate_ids,
        patterns: patterns.to_vec(),
        options: options.clone(),
        filters: filters.clone(),
        position: 0,
        emitted: 0,
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
}
