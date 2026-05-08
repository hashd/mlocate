use crossbeam_channel::{bounded, Receiver, Sender};

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

pub fn create_channels(extractor_threads: usize, batch_size: usize) -> (Sender<String>, Receiver<String>, Sender<FileEntry>, Receiver<FileEntry>) {
    let (crawl_size, extract_size) = channel_sizes(extractor_threads, batch_size);
    let (crawl_tx, crawl_rx) = bounded::<String>(crawl_size);
    let (extract_tx, extract_rx) = bounded::<FileEntry>(extract_size);
    (crawl_tx, crawl_rx, extract_tx, extract_rx)
}
