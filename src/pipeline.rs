use crossbeam_channel::{bounded, Receiver, Sender};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub full_path: String,
    pub size: i64,
    pub mtime: i64,
    pub mode: i32,
    pub mime_type: String,
    pub dir_path: String,
}

pub fn channel_sizes(extractor_threads: usize, _batch_size: usize) -> (usize, usize) {
    let crawl_to_extract = extractor_threads * 500;
    let extract_to_batch = extractor_threads * 2000;
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
    let guessed = mime_guess::from_path(path).first_or_octet_stream();
    if guessed.essence_str() != "application/octet-stream" {
        return guessed.essence_str().to_string();
    }

    if path.extension().is_none() {
        let path_buf = path.to_path_buf();
        let handle = std::thread::spawn(move || {
            let mut buf = vec![0u8; 4096];
            match std::fs::File::open(&path_buf) {
                Ok(mut file) => {
                    use std::io::Read;
                    let bytes_read = file.read(&mut buf).unwrap_or(0);
                    if bytes_read > 0 {
                        buf.truncate(bytes_read);
                        infer::get(&buf).map(|k| k.mime_type().to_string())
                    } else {
                        None
                    }
                }
                Err(_) => None,
            }
        });
        if let Ok(Some(mime)) = handle.join() {
            return mime;
        }
    }

    "application/octet-stream".to_string()
}
