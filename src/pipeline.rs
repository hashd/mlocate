use crate::crawl::WalkStats;
use crossbeam_channel::{bounded, Receiver, Sender};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub full_path: String,
    pub size: i64,
    pub mtime: i64,
    pub mode: i32,
    pub mime_type: String,
    pub dir_path: String,
}

#[derive(Debug, Clone)]
pub struct TrigramRow {
    pub trigram: String,
    pub file_id: i64,
}

pub fn channel_sizes(extractor_threads: usize, batch_size: usize) -> (usize, usize) {
    let crawl_to_extract = extractor_threads * 500;
    let extract_to_batch = extractor_threads * batch_size * 2;
    (crawl_to_extract, extract_to_batch)
}

pub fn create_channels(extractor_threads: usize, batch_size: usize) -> (Sender<PathBuf>, Receiver<PathBuf>, Sender<FileEntry>, Receiver<FileEntry>) {
    let (crawl_size, extract_size) = channel_sizes(extractor_threads, batch_size);
    let (crawl_tx, crawl_rx) = bounded::<PathBuf>(crawl_size);
    let (extract_tx, extract_rx) = bounded::<FileEntry>(extract_size);
    (crawl_tx, crawl_rx, extract_tx, extract_rx)
}

pub fn run_extractor(
    rx: Receiver<PathBuf>,
    tx: Sender<FileEntry>,
    _stats: Arc<WalkStats>,
) {
    while let Ok(path) = rx.recv() {
        let entry = extract_metadata(&path);
        if tx.send(entry).is_err() {
            break;
        }
    }
}

fn extract_metadata(path: &std::path::Path) -> FileEntry {
    let (size, mtime, mode) = match std::fs::metadata(path) {
        Ok(meta) => {
            use std::os::unix::fs::MetadataExt;
            (
                meta.len() as i64,
                meta.mtime(),
                meta.mode() as i32,
            )
        }
        Err(_) => (0i64, 0i64, 0i32),
    };

    let mime_type = detect_mime(path);

    let full_path = path.to_string_lossy().to_string();
    let dir_path = path
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    FileEntry {
        full_path,
        size,
        mtime,
        mode,
        mime_type,
        dir_path,
    }
}

fn detect_mime(path: &std::path::Path) -> String {
    let mut buf = vec![0u8; 4096];
    let bytes_read = match std::fs::File::open(path) {
        Ok(mut file) => {
            use std::io::Read;
            file.read(&mut buf).unwrap_or(0)
        }
        Err(_) => 0,
    };

    if bytes_read > 0 {
        buf.truncate(bytes_read);
        match infer::get(&buf) {
            Some(kind) => kind.mime_type().to_string(),
            None => guess_from_ext(path),
        }
    } else {
        guess_from_ext(path)
    }
}

fn guess_from_ext(path: &std::path::Path) -> String {
    match path.extension().and_then(|e| e.to_str()) {
        Some("rs") => "text/x-rust".to_string(),
        Some("py") => "text/x-python".to_string(),
        Some("js") => "application/javascript".to_string(),
        Some("ts") => "application/typescript".to_string(),
        Some("json") => "application/json".to_string(),
        Some("yaml") | Some("yml") => "application/x-yaml".to_string(),
        Some("md") => "text/markdown".to_string(),
        Some("pdf") => "application/pdf".to_string(),
        Some("png") => "image/png".to_string(),
        Some("jpg") | Some("jpeg") => "image/jpeg".to_string(),
        Some("gif") => "image/gif".to_string(),
        Some("svg") => "image/svg+xml".to_string(),
        Some("zip") => "application/zip".to_string(),
        Some("tar") => "application/x-tar".to_string(),
        Some("gz") => "application/gzip".to_string(),
        Some("bz2") => "application/x-bzip2".to_string(),
        Some("sh") | Some("bash") | Some("zsh") => "text/x-shellscript".to_string(),
        Some("c") | Some("h") => "text/x-c".to_string(),
        Some("cpp") | Some("hpp") => "text/x-c++src".to_string(),
        Some("go") => "text/x-go".to_string(),
        Some("rb") => "text/x-ruby".to_string(),
        Some("css") => "text/css".to_string(),
        Some("html") => "text/html".to_string(),
        Some("toml") => "application/toml".to_string(),
        _ => "application/octet-stream".to_string(),
    }
}
