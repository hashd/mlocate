use regex::Regex;
use roaring::RoaringBitmap;
use std::collections::HashMap;
use std::sync::Arc;

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

            if !self.options.regex
                && !self.patterns.is_empty()
                && !self
                    .patterns
                    .iter()
                    .any(|pat| path_matches(&entry.full_path, pat, &self.options))
            {
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
        let folded_target = trigram::casefold(target);
        let folded_pat = trigram::casefold(pattern);
        folded_target.contains(&folded_pat)
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
        if !ok {
            return false;
        }
    }

    if let Some(ref mf) = filters.modified {
        let cutoff = now - mf.seconds;
        let ok = match mf.operator {
            CmpOp::Ge => entry.mtime >= cutoff,
            CmpOp::Le => entry.mtime <= cutoff,
            CmpOp::Eq => entry.mtime == cutoff,
        };
        if !ok {
            return false;
        }
    }

    if let Some(ref mime) = filters.mime {
        let ok = if mime.is_glob {
            let glob = mime.pattern.replace('*', "");
            entry.mime_type.starts_with(&glob)
        } else {
            entry.mime_type == mime.pattern
        };
        if !ok {
            return false;
        }
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
        regex_search(reader, patterns, options, filters)?
    } else if patterns.is_empty() {
        (0..reader.num_files() as u32).collect()
    } else {
        trigram_search(reader, patterns, options, filters)?
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

fn trigram_search(
    reader: &IndexReader,
    patterns: &[String],
    _options: &SearchOptions,
    _filters: &SearchFilters,
) -> Result<RoaringBitmap, MlocateError> {
    let mut candidates: Option<RoaringBitmap> = None;
    let mut attempted = false;

    // Collect all trigram keys needed across all patterns
    let mut all_trigram_keys: Vec<[u8; 3]> = Vec::new();
    let mut all_bigram_keys: Vec<[u8; 2]> = Vec::new();
    let mut pattern_trigrams: Vec<Vec<[u8; 3]>> = Vec::new();
    let mut pattern_bigrams: Vec<Vec<[u8; 2]>> = Vec::new();

    for pattern in patterns {
        if pattern.len() >= 3 {
            attempted = true;
            let tris = trigram::generate_trigrams_lowercase(pattern);
            let keys: Vec<[u8; 3]> = tris.iter().map(|t| trigram::trigram_to_bytes(t)).collect();
            all_trigram_keys.extend(keys.clone());
            pattern_trigrams.push(keys);
            pattern_bigrams.push(Vec::new());
        } else if pattern.len() == 2 {
            attempted = true;
            let bis = trigram::generate_bigrams_lowercase(pattern);
            let keys: Vec<[u8; 2]> = bis.iter().map(|b| trigram::bigram_to_bytes(b)).collect();
            all_bigram_keys.extend(keys.clone());
            pattern_bigrams.push(keys);
            pattern_trigrams.push(Vec::new());
        } else {
            pattern_trigrams.push(Vec::new());
            pattern_bigrams.push(Vec::new());
        }
    }

    // Batch-fetch all trigram bitmaps in one directory traversal
    let mut trigram_results: HashMap<[u8; 3], Arc<RoaringBitmap>> = HashMap::new();
    if !all_trigram_keys.is_empty() {
        let mut batch_raw: HashMap<[u8; 3], RoaringBitmap> = HashMap::new();
        reader.trigram_bitmaps_batch(&all_trigram_keys, &mut batch_raw);
        for (k, v) in batch_raw {
            trigram_results.insert(k, Arc::new(v));
        }
    }

    // Batch-fetch all bigram bitmaps
    let mut bigram_results: HashMap<[u8; 2], Arc<RoaringBitmap>> = HashMap::new();
    for key in &all_bigram_keys {
        if let Some(bm) = reader.bigram_bitmap(*key) {
            bigram_results.insert(*key, Arc::new(bm));
        }
    }

    // Process each pattern with pre-fetched bitmaps
    for (tris_keys, bis_keys) in pattern_trigrams.iter().zip(pattern_bigrams.iter()) {
        if !tris_keys.is_empty() {
            let mut pattern_and: Option<RoaringBitmap> = None;
            for key in tris_keys {
                let bm = trigram_results.get(key).cloned().unwrap_or_default();
                pattern_and = match pattern_and {
                    None => Some((*bm).clone()),
                    Some(existing) => Some(existing & &*bm),
                };
            }
            if let Some(ids) = pattern_and {
                candidates = match candidates {
                    None => Some(ids),
                    Some(existing) => Some(existing | ids),
                };
            }
        } else if !bis_keys.is_empty() {
            let mut pattern_and: Option<RoaringBitmap> = None;
            for key in bis_keys {
                let bm = bigram_results.get(key).cloned().unwrap_or_default();
                pattern_and = match pattern_and {
                    None => Some((*bm).clone()),
                    Some(existing) => Some(existing & &*bm),
                };
            }
            if let Some(ids) = pattern_and {
                candidates = match candidates {
                    None => Some(ids),
                    Some(existing) => Some(existing | ids),
                };
            }
        }
    }

    // For single-character patterns (no trigrams or bigrams generated),
    // fall through to a linear scan of all document IDs.
    // This is O(n) but single-char searches cannot be accelerated by trigram indexing.
    if !attempted {
        let all: RoaringBitmap = (0..reader.num_files() as u32).collect();
        candidates = Some(all);
    }

    Ok(candidates.unwrap_or_default())
}

fn extract_regex_literals(pattern: &str) -> Vec<String> {
    let mut literals = Vec::new();
    let mut current = String::new();
    let special = "+*?|^$.[](){}\\";

    for ch in pattern.chars() {
        if special.contains(ch) {
            if current.len() >= 2 {
                literals.push(current.clone());
            }
            current.clear();
        } else {
            current.push(ch);
        }
    }
    if current.len() >= 2 {
        literals.push(current);
    }

    // Remove duplicates: when one literal is a substring of another, keep the longer one.
    literals.sort_by_key(|b| std::cmp::Reverse(b.len()));
    let mut result = Vec::new();
    for lit in literals {
        if !result.iter().any(|r: &String| r.contains(&lit)) {
            result.push(lit);
        }
    }
    result
}

fn regex_search(
    reader: &IndexReader,
    patterns: &[String],
    options: &SearchOptions,
    _filters: &SearchFilters,
) -> Result<RoaringBitmap, MlocateError> {
    let regexes: Vec<Regex> = patterns
        .iter()
        .map(|p| {
            let pattern = if options.ignore_case {
                format!("(?i){}", p)
            } else {
                p.clone()
            };
            Regex::new(&pattern)
                .map_err(|e| MlocateError::Other(format!("Invalid regex '{}': {}", p, e)))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut candidates: Option<RoaringBitmap> = None;
    for pattern in patterns {
        let literals = extract_regex_literals(pattern);
        for lit in &literals {
            if lit.len() >= 3 {
                let tris = trigram::generate_trigrams_lowercase(lit);
                let mut lit_and: Option<RoaringBitmap> = None;
                for tri in &tris {
                    let key = trigram::trigram_to_bytes(tri);
                    let bm = reader.trigram_bitmap(key).unwrap_or_default();
                    lit_and = match lit_and {
                        None => Some(bm),
                        Some(existing) => Some(existing & bm),
                    };
                }
                if let Some(ids) = lit_and {
                    candidates = match candidates {
                        None => Some(ids),
                        Some(existing) => Some(existing & ids),
                    };
                }
            }
        }
    }

    let pre_filtered = candidates.unwrap_or_else(|| (0..reader.num_files() as u32).collect());

    let mut bm = RoaringBitmap::new();
    for doc_id in pre_filtered.iter() {
        let path = reader
            .file_path_only(doc_id)
            .map_err(|e| MlocateError::IndexFormatError { details: e })?;
        let target = if options.basename {
            match path.rfind('/') {
                Some(idx) => &path[idx + 1..],
                None => &path,
            }
        } else {
            &path
        };
        if regexes.iter().any(|r| r.is_match(target)) {
            bm.insert(doc_id);
        }
        if let Some(limit) = options.limit {
            if bm.len() as usize >= limit {
                break;
            }
        }
    }
    Ok(bm)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filter::{self};
    use crate::index::format::IndexWriter;

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
            (
                "/home/user/foo.rs",
                1234u64,
                1746720000i64,
                0o644u32,
                "text/x-rust",
            ),
            (
                "/home/user/bar.txt",
                5678u64,
                1746720100i64,
                0o644u32,
                "text/plain",
            ),
            (
                "/etc/config.toml",
                100u64,
                1746700000i64,
                0o600u32,
                "application/octet-stream",
            ),
            (
                "/home/user/README.md",
                2000u64,
                1746800000i64,
                0o644u32,
                "text/markdown",
            ),
            (
                "/var/log/syslog",
                500000u64,
                1746600000i64,
                0o600u32,
                "text/plain",
            ),
        ];
        for (path, size, mtime, mode, mime) in &files {
            let id = writer.add_file(path, *size, *mtime, *mode, mime);
            let tris = trigram::generate_trigrams_lowercase(path);
            for tri in &tris {
                writer.add_trigram_doc(trigram::trigram_to_bytes(tri), id);
            }
            let bis = trigram::generate_bigrams_lowercase(path);
            for bi in &bis {
                writer.add_bigram_doc(trigram::bigram_to_bytes(bi), id);
            }
            if let Some(ext) = std::path::Path::new(path)
                .extension()
                .and_then(|e| e.to_str())
            {
                let mut ext_key = [0u8; 8];
                let eb = ext.as_bytes();
                ext_key[..eb.len().min(8)].copy_from_slice(eb);
                writer.add_ext_doc(ext_key, id);
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
        )
        .unwrap();
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
            &SearchOptions {
                ignore_case: true,
                ..Default::default()
            },
            &SearchFilters::default(),
        )
        .unwrap();
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
            &SearchOptions {
                basename: true,
                ..Default::default()
            },
            &SearchFilters::default(),
        )
        .unwrap();
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
        )
        .unwrap();
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
        )
        .unwrap();
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
            &SearchOptions {
                limit: Some(2),
                ..Default::default()
            },
            &SearchFilters::default(),
        )
        .unwrap();
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
                size: Some(SizeFilter {
                    bytes: 500000,
                    operator: CmpOp::Ge,
                }),
                ..Default::default()
            },
        )
        .unwrap();
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
            &SearchOptions {
                regex: true,
                ..Default::default()
            },
            &SearchFilters::default(),
        )
        .unwrap();
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
            &SearchOptions {
                regex: true,
                ignore_case: true,
                ..Default::default()
            },
            &SearchFilters::default(),
        )
        .unwrap();
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
        )
        .unwrap();
        let paths: Vec<String> = results.map(|r| r.unwrap().full_path).collect();
        assert_eq!(paths, vec!["/home/user/foo.rs", "/var/log/syslog"]);
    }
}
