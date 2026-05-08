use crossbeam_channel::{bounded, Receiver, Sender};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub full_path: String,
    pub size: u64,
    pub mtime: i64,
    pub mode: u32,
    pub mime_type: String,
}

pub fn channel_sizes(extractor_threads: usize) -> (usize, usize) {
    let crawl_to_extract = extractor_threads * 500;
    let extract_to_batch = extractor_threads * 2000;
    (crawl_to_extract, extract_to_batch)
}

pub fn create_channels(extractor_threads: usize, _batch_size: usize) -> (Sender<PathBuf>, Receiver<PathBuf>, Sender<FileEntry>, Receiver<FileEntry>) {
    let (crawl_size, extract_size) = channel_sizes(extractor_threads);
    let (crawl_tx, crawl_rx) = bounded::<PathBuf>(crawl_size);
    let (extract_tx, extract_rx) = bounded::<FileEntry>(extract_size);
    (crawl_tx, crawl_rx, extract_tx, extract_rx)
}

pub fn run_extractor(
    rx: Receiver<PathBuf>,
    tx: Sender<FileEntry>,
) {
    while let Ok(path) = rx.recv() {
        if let Some(entry) = extract_metadata(&path) {
            if tx.send(entry).is_err() {
                break;
            }
        }
    }
}

fn extract_metadata(path: &std::path::Path) -> Option<FileEntry> {
    let meta = match std::fs::metadata(path) {
        Ok(meta) => meta,
        Err(_) => return None,
    };

    use std::os::unix::fs::MetadataExt;
    let size = meta.len();
    let mtime = meta.mtime();
    let mode = meta.mode();

    let mime_type = detect_mime(path);

    let full_path = path.to_string_lossy().to_string();

    Some(FileEntry {
        full_path,
        size,
        mtime,
        mode,
        mime_type,
    })
}

fn detect_mime(path: &std::path::Path) -> String {
    let guessed = mime_guess::from_path(path).first_or_octet_stream();
    if guessed.essence_str() != "application/octet-stream" {
        return guessed.essence_str().to_string();
    }

    if path.extension().is_none() {
        let mut buf = vec![0u8; 512];
        if let Ok(mut file) = std::fs::File::open(path) {
            use std::io::Read;
            let bytes_read = file.read(&mut buf).unwrap_or(0);
            if bytes_read > 0 {
                buf.truncate(bytes_read);
                if let Some(kind) = infer::get(&buf) {
                    return kind.mime_type().to_string();
                }
            }
        }
    }

    "application/octet-stream".to_string()
}
