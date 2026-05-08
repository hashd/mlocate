use std::collections::HashSet;
use std::fs;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::Arc;

use crossbeam_channel::Receiver;

use crate::crawl::WalkStats;
use crate::error::MlocateError;
use crate::pipeline::FileEntry;

use super::format::{IndexConfig, IndexWriter};
use super::trigram;

pub fn build_index(
    rx: Receiver<FileEntry>,
    db_path: &Path,
    config: IndexConfig,
    stats: Arc<WalkStats>,
) -> Result<(), MlocateError> {
    let mut entries: Vec<FileEntry> = Vec::new();
    while let Ok(entry) = rx.recv() {
        entries.push(entry);
    }

    if entries.is_empty() {
        return write_empty_index(db_path, &config);
    }

    let mut seen: HashSet<String> = HashSet::new();
    entries.retain(|e| seen.insert(e.full_path.clone()));
    entries.sort_by(|a, b| a.full_path.cmp(&b.full_path));

    let mut writer = IndexWriter::new();
    for (i, entry) in entries.iter().enumerate() {
        let doc_id = writer.add_file(
            &entry.full_path,
            entry.size,
            entry.mtime,
            entry.mode as u32,
            &entry.mime_type,
        );

        let tris = trigram::generate_trigrams_lowercase(&entry.full_path);
        for tri in &tris {
            let key = trigram::trigram_to_bytes(tri);
            writer.add_trigram_doc(key, doc_id);
        }

        stats.files_added.store(i + 1, std::sync::atomic::Ordering::Relaxed);
    }
    drop(entries);

    writer.prune_empty();

    let tmp_path = db_path.with_extension("db.tmp");
    {
        let file = fs::File::create(&tmp_path).map_err(|e| MlocateError::Other(format!(
            "Cannot create temp index {}: {}", tmp_path.display(), e
        )))?;
        let mut buf = BufWriter::new(file);
        let _num_files = writer.write_to(&mut buf, &config).map_err(|e| MlocateError::Other(format!(
            "Failed to write index: {}", e
        )))?;
        buf.flush().map_err(|e| MlocateError::Other(format!("Failed to flush: {}", e)))?;
        buf.into_inner()
            .map_err(|e| MlocateError::Other(format!("Failed to sync: {}", e.into_error())))?
            .sync_all()
            .map_err(|e| MlocateError::Other(format!(
                "Failed to fsync temp index: {}", e
            )))?;
    }

    if let Some(parent) = db_path.parent() {
        if let Ok(dir) = fs::File::open(parent) {
            if let Err(e) = dir.sync_all() {
                eprintln!("Warning: failed to fsync directory: {}", e);
            }
        }
    }

    fs::rename(&tmp_path, db_path).map_err(|e| MlocateError::Other(format!(
        "Failed to rename {} to {}: {}",
        tmp_path.display(),
        db_path.display(),
        e
    )))?;

    Ok(())
}

fn write_empty_index(db_path: &Path, config: &IndexConfig) -> Result<(), MlocateError> {
    let writer = IndexWriter::new();
    let tmp_path = db_path.with_extension("db.tmp");
    {
        let file = fs::File::create(&tmp_path).map_err(|e| MlocateError::Other(format!(
            "Cannot create empty temp index: {}", e
        )))?;
        let mut buf = BufWriter::new(file);
        writer.write_to(&mut buf, config).map_err(|e| MlocateError::Other(format!(
            "Failed to write empty index: {}", e
        )))?;
        buf.flush().map_err(|e| MlocateError::Other(format!("Failed to flush: {}", e)))?;
        buf.into_inner()
            .map_err(|e| MlocateError::Other(format!("Failed to sync: {}", e.into_error())))?
            .sync_all()
            .map_err(|e| MlocateError::Other(format!(
                "Failed to fsync: {}", e
            )))?;
    }
    if let Some(parent) = db_path.parent() {
        if let Ok(dir) = fs::File::open(parent) {
            if let Err(e) = dir.sync_all() {
                eprintln!("Warning: failed to fsync directory: {}", e);
            }
        }
    }
    fs::rename(&tmp_path, db_path).map_err(|e| MlocateError::Other(format!(
        "Failed to rename empty index: {}", e
    )))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::bounded;
    use super::super::format::IndexReader;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn test_build_and_read_back() {
        let (tx, rx) = bounded::<FileEntry>(10);
        tx.send(FileEntry {
            full_path: "/tmp/a/hello.rs".into(),
            size: 100,
            mtime: 1746720000,
            mode: 0o644,
            mime_type: "text/x-rust".into(),
        }).unwrap();
        tx.send(FileEntry {
            full_path: "/tmp/a/world.md".into(),
            size: 200,
            mtime: 1746720100,
            mode: 0o644,
            mime_type: "text/markdown".into(),
        }).unwrap();
        drop(tx);

        let config = IndexConfig {
            indexed_paths: vec!["/tmp/a".into()],
            pruned_paths: vec![],
            timestamp: 1746720000,
            hostname: "test".into(),
            total_bytes_indexed: 300,
            mlocate_version: "0.1.0".into(),
        };
        let stats = Arc::new(WalkStats {
            files_scanned: AtomicUsize::new(0),
            files_added: AtomicUsize::new(0),
            dirs_skipped: AtomicUsize::new(0),
            permission_denied: AtomicUsize::new(0),
            ino_set_size: AtomicUsize::new(0),
        });
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let db_path = tmp.path();
        build_index(rx, db_path, config, stats).unwrap();

        let mmap = unsafe { memmap2::Mmap::map(&std::fs::File::open(db_path).unwrap()).unwrap() };
        let reader = IndexReader::new(&mmap).unwrap();
        assert_eq!(reader.num_files(), 2);
        assert_eq!(reader.file_path(0).unwrap(), "/tmp/a/hello.rs");
        assert_eq!(reader.file_path(1).unwrap(), "/tmp/a/world.md");
    }

    #[test]
    fn test_dedup_by_path() {
        let (tx, rx) = bounded::<FileEntry>(10);
        let mut entry = FileEntry {
            full_path: "/dup".into(),
            size: 0,
            mtime: 0,
            mode: 0,
            mime_type: "text/plain".into(),
        };
        tx.send(entry.clone()).unwrap();
        entry.size = 999;
        tx.send(entry).unwrap();
        drop(tx);

        let config = IndexConfig {
            indexed_paths: vec!["/".into()],
            pruned_paths: vec![],
            timestamp: 0,
            hostname: "test".into(),
            total_bytes_indexed: 0,
            mlocate_version: "0.1.0".into(),
        };
        let stats = Arc::new(WalkStats {
            files_scanned: AtomicUsize::new(0),
            files_added: AtomicUsize::new(0),
            dirs_skipped: AtomicUsize::new(0),
            permission_denied: AtomicUsize::new(0),
            ino_set_size: AtomicUsize::new(0),
        });
        let tmp = tempfile::NamedTempFile::new().unwrap();
        build_index(rx, tmp.path(), config, stats).unwrap();

        let mmap = unsafe { memmap2::Mmap::map(&std::fs::File::open(tmp.path()).unwrap()).unwrap() };
        let reader = IndexReader::new(&mmap).unwrap();
        assert_eq!(reader.num_files(), 1);
    }
}
