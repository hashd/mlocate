use std::fs;
use std::io::{BufWriter, Write, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crossbeam_channel::Receiver;

use crate::crawl::WalkStats;
use crate::error::MlocateError;
use crate::pipeline::FileEntry;

use super::format::{IndexConfig, IndexWriter, IndexReader, DirTableEntry};
use super::trigram;

const CHUNK_SIZE: usize = 100_000;

#[derive(Debug)]
struct ChunkEntry {
    path: String,
    size: u64,
    mtime: i64,
    mode: u32,
    mime_type: String,
}

pub fn build_index(
    rx: Receiver<FileEntry>,
    db_path: &Path,
    config: IndexConfig,
    stats: Arc<WalkStats>,
) -> Result<(), MlocateError> {
    let run_id = std::time::SystemTime::now().duration_since(std::time::SystemTime::UNIX_EPOCH).unwrap_or_default().as_nanos();
    let chunk_files = sort_chunks_to_disk(rx, db_path.parent().unwrap_or(Path::new(".")), stats.clone(), run_id)?;

    let tmp_path = db_path.with_extension("db.tmp");

    if chunk_files.is_empty() {
        let writer = IndexWriter::new();
        write_atomic(db_path, &tmp_path, &config, &writer)?;
        return Ok(());
    }

    let writer = merge_chunks(&chunk_files, stats)?;

    for f in &chunk_files {
        let _ = std::fs::remove_file(f);
    }

    write_atomic(db_path, &tmp_path, &config, &writer)?;

    Ok(())
}

pub fn build_index_incremental(
    rx: Receiver<FileEntry>,
    db_path: &Path,
    old_reader: &IndexReader,
    config: IndexConfig,
    stats: Arc<WalkStats>,
    skipped_dirs: &[DirTableEntry],
) -> Result<(), MlocateError> {
    let run_id = std::time::SystemTime::now().duration_since(std::time::SystemTime::UNIX_EPOCH).unwrap_or_default().as_nanos();
    let chunk_files = sort_chunks_to_disk(rx, db_path.parent().unwrap_or(Path::new(".")), stats.clone(), run_id)?;

    let tmp_path = db_path.with_extension("db.tmp");

    if chunk_files.is_empty() && skipped_dirs.is_empty() {
        let writer = IndexWriter::new();
        write_atomic(db_path, &tmp_path, &config, &writer)?;
        return Ok(());
    }

    let mut writer = merge_chunks(&chunk_files, stats)?;

    for dir_entry in skipped_dirs {
        let paths = old_reader.get_file_paths_in_dir(dir_entry)
            .map_err(|e| MlocateError::Other(format!("Failed to read old entries for {}: {}", dir_entry.path, e)))?;
        let entries = old_reader.get_files_in_dir(dir_entry)
            .map_err(|e| MlocateError::Other(format!("Failed to read old entries for {}: {}", dir_entry.path, e)))?;

        for (path, entry) in paths.iter().zip(entries.iter()) {
            let doc_id = writer.add_file(path, entry.size, entry.mtime, entry.mode, &entry.mime_type);
            index_file_trigrams(&mut writer, path, doc_id);
        }
    }

    for f in &chunk_files {
        let _ = std::fs::remove_file(f);
    }

    write_atomic(db_path, &tmp_path, &config, &writer)?;

    Ok(())
}

fn sort_chunks_to_disk(
    rx: Receiver<FileEntry>,
    chunk_dir: &Path,
    stats: Arc<WalkStats>,
    run_id: u128,
) -> Result<Vec<PathBuf>, MlocateError> {
    let mut chunks = Vec::new();
    let mut buffer: Vec<ChunkEntry> = Vec::with_capacity(CHUNK_SIZE);
    let mut total_added = 0usize;
    let mut chunk_idx = 0u32;

    while let Ok(entry) = rx.recv() {
        buffer.push(ChunkEntry {
            path: entry.full_path,
            size: entry.size,
            mtime: entry.mtime,
            mode: entry.mode,
            mime_type: entry.mime_type,
        });

        if buffer.len() >= CHUNK_SIZE {
            chunks.push(flush_chunk(&mut buffer, chunk_dir, chunk_idx, run_id)?);
            chunk_idx += 1;
            total_added += CHUNK_SIZE;
            stats.files_added.store(total_added, std::sync::atomic::Ordering::Relaxed);
        }
    }

    if !buffer.is_empty() {
        let pre_drain = buffer.len();
        chunks.push(flush_chunk(&mut buffer, chunk_dir, chunk_idx, run_id)?);
        total_added += pre_drain;
        stats.files_added.store(total_added, std::sync::atomic::Ordering::Relaxed);
    }

    Ok(chunks)
}

fn flush_chunk(
    buffer: &mut Vec<ChunkEntry>,
    chunk_dir: &Path,
    idx: u32,
    run_id: u128,
) -> Result<PathBuf, MlocateError> {
    buffer.sort_by(|a, b| a.path.cmp(&b.path));
    buffer.dedup_by(|a, b| a.path == b.path);

    let path = chunk_dir.join(format!("mlocate_chunk_{:04}_{}.tmp", idx, run_id));
    let file = fs::File::create(&path).map_err(|e| MlocateError::Other(format!(
        "Cannot create chunk file {}: {}", path.display(), e
    )))?;
    let mut writer = BufWriter::new(file);

    let count = buffer.len() as u32;
    writer.write_all(&count.to_le_bytes()).map_err(|e| MlocateError::Other(e.to_string()))?;

    for entry in buffer.drain(..) {
        let path_bytes = entry.path.as_bytes();
        writer.write_all(&(path_bytes.len() as u32).to_le_bytes()).map_err(|e| MlocateError::Other(e.to_string()))?;
        writer.write_all(path_bytes).map_err(|e| MlocateError::Other(e.to_string()))?;
        writer.write_all(&entry.size.to_le_bytes()).map_err(|e| MlocateError::Other(e.to_string()))?;
        writer.write_all(&entry.mtime.to_le_bytes()).map_err(|e| MlocateError::Other(e.to_string()))?;
        writer.write_all(&entry.mode.to_le_bytes()).map_err(|e| MlocateError::Other(e.to_string()))?;
        let mime_bytes = entry.mime_type.as_bytes();
        writer.write_all(&(mime_bytes.len() as u8).to_le_bytes()).map_err(|e| MlocateError::Other(e.to_string()))?;
        writer.write_all(mime_bytes).map_err(|e| MlocateError::Other(e.to_string()))?;
    }

    writer.flush().map_err(|e| MlocateError::Other(e.to_string()))?;
    Ok(path)
}

struct ChunkReader {
    file: std::fs::File,
    remaining: u32,
    current: Option<ChunkEntry>,
}

impl ChunkReader {
    fn open(path: &Path) -> Result<Self, MlocateError> {
        let mut file = fs::File::open(path).map_err(|e| MlocateError::Other(format!(
            "Cannot open chunk {}: {}", path.display(), e
        )))?;

        let mut count_buf = [0u8; 4];
        file.read_exact(&mut count_buf).map_err(|e| MlocateError::Other(e.to_string()))?;
        let remaining = u32::from_le_bytes(count_buf);

        let mut reader = ChunkReader { file, remaining, current: None };
        reader.advance()?;
        Ok(reader)
    }

    fn advance(&mut self) -> Result<(), MlocateError> {
        if self.remaining == 0 {
            self.current = None;
            return Ok(());
        }

        let mut len_buf = [0u8; 4];
        self.file.read_exact(&mut len_buf).map_err(|e| MlocateError::Other(e.to_string()))?;
        let path_len = u32::from_le_bytes(len_buf) as usize;

        let mut path_buf = vec![0u8; path_len];
        self.file.read_exact(&mut path_buf).map_err(|e| MlocateError::Other(e.to_string()))?;
        let path = String::from_utf8_lossy(&path_buf).to_string();

        let mut num_buf = [0u8; 8];
        self.file.read_exact(&mut num_buf).map_err(|e| MlocateError::Other(e.to_string()))?;
        let size = u64::from_le_bytes(num_buf);

        self.file.read_exact(&mut num_buf).map_err(|e| MlocateError::Other(e.to_string()))?;
        let mtime = i64::from_le_bytes(num_buf);

        let mut mode_buf = [0u8; 4];
        self.file.read_exact(&mut mode_buf).map_err(|e| MlocateError::Other(e.to_string()))?;
        let mode = u32::from_le_bytes(mode_buf);

        let mut mime_len_buf = [0u8; 1];
        self.file.read_exact(&mut mime_len_buf).map_err(|e| MlocateError::Other(e.to_string()))?;
        let mime_len = mime_len_buf[0] as usize;
        let mut mime_buf = vec![0u8; mime_len];
        self.file.read_exact(&mut mime_buf).map_err(|e| MlocateError::Other(e.to_string()))?;
        let mime_type = String::from_utf8_lossy(&mime_buf).to_string();

        self.remaining -= 1;
        self.current = Some(ChunkEntry { path, size, mtime, mode, mime_type });
        Ok(())
    }

    fn peek(&self) -> Option<(&str, u64, i64, u32, &str)> {
        self.current.as_ref().map(|e| (e.path.as_str(), e.size, e.mtime, e.mode, e.mime_type.as_str()))
    }
}

fn merge_chunks(
    chunk_files: &[PathBuf],
    stats: Arc<WalkStats>,
) -> Result<IndexWriter, MlocateError> {
    let mut readers: Vec<ChunkReader> = Vec::with_capacity(chunk_files.len());
    for f in chunk_files {
        readers.push(ChunkReader::open(f)?);
    }

    // Estimate memory for in-memory trigram bitmap construction:
    // ~15 trigrams per path × ~32 bytes per RoaringBitmap doc_id entry
    let total_files: usize = readers.iter().map(|r| r.remaining as usize).sum();
    let est_trigram_mem_mb = (total_files as u64 * 15 * 32) / (1024 * 1024);

    const TRIGRAM_MEM_WARN_MB: u64 = 256; // 256 MB trigger for diagnostic

    if est_trigram_mem_mb > TRIGRAM_MEM_WARN_MB {
        eprintln!(
            "mlocate: indexing {} files (~{} MB estimated trigram memory). \
             For very large indexes (>1M files), consider implementing external sort for trigram bitmap construction (see Issue 9.1).",
            total_files, est_trigram_mem_mb
        );
    }

    let mut heap = std::collections::BinaryHeap::new();
    for (i, reader) in readers.iter().enumerate() {
        if let Some((path, ..)) = reader.peek() {
            heap.push(HeapItem {
                path: path.to_string(),
                reader_idx: i,
            });
        }
    }

    let mut writer = IndexWriter::new();
    let mut i = 0u32;

    while let Some(item) = heap.pop() {
        let reader = &mut readers[item.reader_idx];
        if let Some((path, size, mtime, mode, mime_type)) = reader.peek() {
            let doc_id = writer.add_file(path, size, mtime, mode, mime_type);
            index_file_trigrams(&mut writer, path, doc_id);
            i += 1;
            if (i as usize).is_multiple_of(50_000) {
                stats.files_added.store(i as usize, std::sync::atomic::Ordering::Relaxed);
            }
        }
        readers[item.reader_idx].advance()?;
        if let Some((path, ..)) = readers[item.reader_idx].peek() {
            heap.push(HeapItem {
                path: path.to_string(),
                reader_idx: item.reader_idx,
            });
        }
    }

    writer.prune_empty();
    Ok(writer)
}

fn index_file_trigrams(writer: &mut IndexWriter, path: &str, doc_id: u32) {
    let tris = trigram::generate_trigrams_lowercase(path);
    for tri in &tris {
        let key = trigram::trigram_to_bytes(tri);
        writer.add_trigram_doc(key, doc_id);
    }

    let bis = trigram::generate_bigrams_lowercase(path);
    for bi in &bis {
        let key = trigram::bigram_to_bytes(bi);
        writer.add_bigram_doc(key, doc_id);
    }

    if let Some(ext) = Path::new(path).extension().and_then(|e| e.to_str()) {
        let mut ext_key = [0u8; 8];
        let eb = ext.as_bytes();
        if eb.len() <= 8 {
            ext_key[..eb.len()].copy_from_slice(eb);
        } else {
            let hash = fxhash::hash64(eb);
            ext_key.copy_from_slice(&hash.to_le_bytes());
        }
        writer.add_ext_doc(ext_key, doc_id);
    }
}

fn write_atomic(
    db_path: &Path,
    tmp_path: &Path,
    config: &IndexConfig,
    writer: &IndexWriter,
) -> Result<(), MlocateError> {
    {
        let data = writer.into_bytes(config).map_err(|e| MlocateError::Other(format!(
            "Failed to write index: {}", e
        )))?;
        std::fs::write(tmp_path, &data).map_err(|e| MlocateError::Other(format!(
            "Cannot create temp index {}: {}", tmp_path.display(), e
        )))?;
    }

    if let Some(parent) = db_path.parent() {
        if let Ok(dir) = fs::File::open(parent) {
            if let Err(e) = dir.sync_all() {
                eprintln!("Warning: failed to fsync directory: {}", e);
            }
        }
    }

    fs::rename(tmp_path, db_path).map_err(|e| MlocateError::Other(format!(
        "Failed to rename {} to {}: {}",
        tmp_path.display(),
        db_path.display(),
        e
    )))?;

    Ok(())
}

#[derive(Eq)]
struct HeapItem {
    path: String,
    reader_idx: usize,
}

// Reversed comparison to make BinaryHeap (max-heap by default) behave as min-heap.
// This ensures the smallest path is popped first during k-way merge.
impl Ord for HeapItem {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other.path.cmp(&self.path)
    }
}

impl PartialOrd for HeapItem {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for HeapItem {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
    }
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

    #[test]
    fn test_large_chunk_merge() {
        let (tx, rx) = bounded::<FileEntry>(CHUNK_SIZE * 2 + 100);
        for i in 0..(CHUNK_SIZE + 500) {
            tx.send(FileEntry {
                full_path: format!("/tmp/file_{:08}.txt", i),
                size: (i % 1000) as u64,
                mtime: 1746720000,
                mode: 0o644,
                mime_type: "text/plain".into(),
            }).unwrap();
        }
        drop(tx);

        let config = IndexConfig {
            indexed_paths: vec!["/tmp".into()],
            pruned_paths: vec![],
            timestamp: 1746720000,
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
        assert_eq!(reader.num_files(), (CHUNK_SIZE + 500) as u64);
    }
}
