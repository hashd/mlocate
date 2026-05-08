use std::io::{self, Write, Seek};
use roaring::RoaringBitmap;
use std::collections::BTreeMap;
use memmap2::Mmap;

pub const MAGIC: &[u8; 7] = b"mlocate";
pub const MAGIC_FULL: &[u8; 8] = b"mlocate\0";
pub const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone)]
pub struct Header {
    pub num_files: u64,
    pub num_trigrams: u64,
    pub config_offset: u64,
    pub config_len: u32,
    pub file_dir_offset: u64,
    pub file_offset_dir_offset: u64,
    pub trigram_dir_offset: u64,
    pub bitmap_data_offset: u64,
}

impl Header {
    pub const SIZE: usize = 72;

    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        let mut buf = [0u8; Self::SIZE];
        buf[0..8].copy_from_slice(MAGIC_FULL);
        buf[8..12].copy_from_slice(&FORMAT_VERSION.to_le_bytes());
        buf[12..20].copy_from_slice(&self.num_files.to_le_bytes());
        buf[20..28].copy_from_slice(&self.num_trigrams.to_le_bytes());
        buf[28..36].copy_from_slice(&self.config_offset.to_le_bytes());
        buf[36..40].copy_from_slice(&self.config_len.to_le_bytes());
        buf[40..48].copy_from_slice(&self.file_dir_offset.to_le_bytes());
        buf[48..56].copy_from_slice(&self.file_offset_dir_offset.to_le_bytes());
        buf[56..64].copy_from_slice(&self.trigram_dir_offset.to_le_bytes());
        buf[64..72].copy_from_slice(&self.bitmap_data_offset.to_le_bytes());
        buf
    }

    pub fn from_bytes(buf: &[u8]) -> Result<Self, &'static str> {
        if buf.len() < Self::SIZE {
            return Err("header too short");
        }
        if &buf[0..7] != MAGIC {
            return Err("invalid magic bytes");
        }
        let version = u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]);
        if version != FORMAT_VERSION {
            return Err("unsupported format version");
        }
        Ok(Header {
            num_files:        u64::from_le_bytes(buf[12..20].try_into().unwrap()),
            num_trigrams:     u64::from_le_bytes(buf[20..28].try_into().unwrap()),
            config_offset:    u64::from_le_bytes(buf[28..36].try_into().unwrap()),
            config_len:       u32::from_le_bytes(buf[36..40].try_into().unwrap()),
            file_dir_offset:  u64::from_le_bytes(buf[40..48].try_into().unwrap()),
            file_offset_dir_offset: u64::from_le_bytes(buf[48..56].try_into().unwrap()),
            trigram_dir_offset: u64::from_le_bytes(buf[56..64].try_into().unwrap()),
            bitmap_data_offset:  u64::from_le_bytes(buf[64..72].try_into().unwrap()),
        })
    }
}

#[derive(Debug, Clone)]
pub struct DiskFileEntry {
    pub full_path: String,
    pub size: u64,
    pub mtime: i64,
    pub mode: u32,
    pub mime_type: String,
}

#[derive(Debug, Clone)]
pub struct TrigramEntry {
    pub trigram: [u8; 3],
    pub bitmap_offset: u64,
    pub bitmap_len: u32,
}

impl TrigramEntry {
    pub const ENTRY_SIZE: usize = 15;

    pub fn from_bytes(buf: &[u8]) -> Self {
        let trigram = [buf[0], buf[1], buf[2]];
        let bitmap_offset = u64::from_le_bytes(buf[3..11].try_into().unwrap());
        let bitmap_len = u32::from_le_bytes(buf[11..15].try_into().unwrap());
        TrigramEntry { trigram, bitmap_offset, bitmap_len }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct IndexConfig {
    pub indexed_paths: Vec<String>,
    pub pruned_paths: Vec<String>,
    pub timestamp: i64,
    pub hostname: String,
    pub total_bytes_indexed: u64,
    pub mlocate_version: String,
}

impl IndexConfig {
    pub fn to_compressed_json(&self) -> io::Result<Vec<u8>> {
        let json = serde_json::to_vec(self)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        zstd::encode_all(&json[..], 3)
            .map_err(io::Error::other)
    }

    pub fn from_compressed(data: &[u8]) -> io::Result<Self> {
        let json = zstd::decode_all(data)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        serde_json::from_slice(&json)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }
}

pub struct IndexWriter {
    file_dir_buf: io::Cursor<Vec<u8>>,
    file_offsets: Vec<u64>,
    trigrams: BTreeMap<[u8; 3], RoaringBitmap>,
}

impl Default for IndexWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl IndexWriter {
    pub fn new() -> Self {
        IndexWriter {
            file_dir_buf: io::Cursor::new(Vec::new()),
            file_offsets: Vec::new(),
            trigrams: BTreeMap::new(),
        }
    }

    pub fn add_file(&mut self, full_path: &str, size: u64, mtime: i64, mode: u32, mime_type: &str) -> u32 {
        let id = self.file_offsets.len() as u32;
        self.file_offsets.push(self.file_dir_buf.position());

        let path_bytes = full_path.as_bytes();
        let mime_bytes = mime_type.as_bytes();
        self.file_dir_buf.write_all(&(path_bytes.len() as u32).to_le_bytes()).unwrap();
        self.file_dir_buf.write_all(path_bytes).unwrap();
        self.file_dir_buf.write_all(&size.to_le_bytes()).unwrap();
        self.file_dir_buf.write_all(&mtime.to_le_bytes()).unwrap();
        self.file_dir_buf.write_all(&mode.to_le_bytes()).unwrap();
        self.file_dir_buf.write_all(&(mime_bytes.len() as u8).to_le_bytes()).unwrap();
        self.file_dir_buf.write_all(mime_bytes).unwrap();

        id
    }

    pub fn add_trigram_doc(&mut self, trigram: [u8; 3], doc_id: u32) {
        self.trigrams.entry(trigram).or_default().insert(doc_id);
    }

    pub fn prune_empty(&mut self) {
        self.trigrams.retain(|_, bm| !bm.is_empty());
    }

    pub fn write_to<W: Write + Seek>(&self, w: &mut W, config: &IndexConfig) -> io::Result<u64> {
        let num_files = self.file_offsets.len() as u64;
        let num_trigrams = self.trigrams.len() as u64;
        let file_dir_data = self.file_dir_buf.get_ref();
        let file_dir_total_len = file_dir_data.len() as u64;

        let placeholder = [0u8; Header::SIZE];
        w.write_all(&placeholder)?;

        let config_data = config.to_compressed_json()?;
        let config_offset = w.stream_position()?;
        w.write_all(&config_data)?;
        let config_len = config_data.len() as u32;

        let file_dir_offset = w.stream_position()?;
        w.write_all(file_dir_data)?;

        let file_offset_dir_offset = w.stream_position()?;
        for &off in &self.file_offsets {
            w.write_all(&off.to_le_bytes())?;
        }
        w.write_all(&file_dir_total_len.to_le_bytes())?;

        let trigram_dir_offset = w.stream_position()?;
        let bitmap_data_offset = trigram_dir_offset + (num_trigrams * 15);
        let trigram_dir_start = w.stream_position()?;
        w.write_all(&vec![0u8; (num_trigrams as usize) * 15])?;

        let mut trigram_entries: Vec<(u64, u32)> = Vec::with_capacity(num_trigrams as usize);
        for bitmap in self.trigrams.values() {
            let offset = w.stream_position()? - bitmap_data_offset;
            let len_before = w.stream_position()?;
            bitmap.serialize_into(&mut *w).map_err(io::Error::other)?;
            let len_after = w.stream_position()?;
            let len = (len_after - len_before) as u32;
            trigram_entries.push((offset, len));
        }
        let bitmap_end = w.stream_position()?;

        w.seek(io::SeekFrom::Start(trigram_dir_start))?;
        for ((trigram, _), (offset, len)) in self.trigrams.iter().zip(trigram_entries.iter()) {
            w.write_all(&trigram[..])?;
            w.write_all(&offset.to_le_bytes())?;
            w.write_all(&len.to_le_bytes())?;
        }
        w.seek(io::SeekFrom::Start(bitmap_end))?;

        let header = Header {
            num_files,
            num_trigrams,
            config_offset,
            config_len,
            file_dir_offset,
            file_offset_dir_offset,
            trigram_dir_offset,
            bitmap_data_offset,
        };
        w.seek(io::SeekFrom::Start(0))?;
        w.write_all(&header.to_bytes())?;
        w.seek(io::SeekFrom::Start(bitmap_end))?;

        Ok(num_files)
    }

    pub fn into_bytes(&self, config: &IndexConfig) -> io::Result<Vec<u8>> {
        let mut buf = io::Cursor::new(Vec::new());
        self.write_to(&mut buf, config)?;
        Ok(buf.into_inner())
    }
}

pub struct IndexReader<'a> {
    data: &'a [u8],
    header: Header,
}

impl<'a> IndexReader<'a> {
    pub fn new(mmap: &'a Mmap) -> Result<Self, String> {
        let data: &[u8] = mmap.as_ref();
        let header = Header::from_bytes(data).map_err(|e| e.to_string())?;
        Ok(IndexReader { data, header })
    }

    pub fn from_bytes(data: &'a [u8]) -> Result<Self, String> {
        let header = Header::from_bytes(data).map_err(|e| e.to_string())?;
        Ok(IndexReader { data, header })
    }

    pub fn num_files(&self) -> u64 {
        self.header.num_files
    }

    pub fn header(&self) -> &Header {
        &self.header
    }

    pub fn file_entry(&self, doc_id: u32) -> Result<DiskFileEntry, String> {
        if doc_id as u64 >= self.header.num_files {
            return Err(format!("doc_id {} out of range (max {})", doc_id, self.header.num_files));
        }
        let off_dir = self.header.file_offset_dir_offset as usize;
        let entry_size = 8;
        let off_idx = off_dir + (doc_id as usize) * entry_size;
        let entry_off = u64::from_le_bytes(
            self.data[off_idx..off_idx + 8].try_into().unwrap()
        );
        let next_off = u64::from_le_bytes(
            self.data[off_idx + 8..off_idx + 16].try_into().unwrap()
        );
        let entry_len = (next_off - entry_off) as usize;

        let start = self.header.file_dir_offset as usize + entry_off as usize;
        let entry_data = &self.data[start..start + entry_len];
        Self::parse_file_entry(entry_data)
    }

    pub fn file_path(&self, doc_id: u32) -> Result<String, String> {
        self.file_entry(doc_id).map(|e| e.full_path)
    }

    fn parse_file_entry(data: &[u8]) -> Result<DiskFileEntry, String> {
        if data.len() < 4 { return Err("file entry too short".into()); }
        let path_len = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
        let off = 4 + path_len;
        if data.len() < off + 21 { return Err("file entry truncated".into()); }
        let path = String::from_utf8_lossy(&data[4..off]).to_string();
        let size = u64::from_le_bytes(data[off..off+8].try_into().unwrap());
        let mtime = i64::from_le_bytes(data[off+8..off+16].try_into().unwrap());
        let mode = u32::from_le_bytes(data[off+16..off+20].try_into().unwrap());
        let mime_len = data[off+20] as usize;
        let mime_off = off + 21;
        if data.len() < mime_off + mime_len { return Err("mime field truncated".into()); }
        let mime_type = String::from_utf8_lossy(&data[mime_off..mime_off + mime_len]).to_string();
        Ok(DiskFileEntry { full_path: path, size, mtime, mode, mime_type })
    }

    pub fn trigram_bitmap(&self, trigram: [u8; 3]) -> Option<RoaringBitmap> {
        let n = self.header.num_trigrams as usize;
        let dir_start = self.header.trigram_dir_offset as usize;
        let mut lo = 0usize;
        let mut hi = n;
        while lo < hi {
            let mid = (lo + hi) / 2;
            let entry_off = dir_start + mid * TrigramEntry::ENTRY_SIZE;
            let entry_bytes = &self.data[entry_off..entry_off + TrigramEntry::ENTRY_SIZE];
            let mid_tri = [entry_bytes[0], entry_bytes[1], entry_bytes[2]];
            match mid_tri.cmp(&trigram) {
                std::cmp::Ordering::Less => lo = mid + 1,
                std::cmp::Ordering::Greater => hi = mid,
                std::cmp::Ordering::Equal => {
                    let entry = TrigramEntry::from_bytes(entry_bytes);
                    let bm_start = self.header.bitmap_data_offset as usize + entry.bitmap_offset as usize;
                    let bm_data = &self.data[bm_start..bm_start + entry.bitmap_len as usize];
                    return Some(RoaringBitmap::deserialize_from(bm_data).unwrap_or_default());
                }
            }
        }
        None
    }

    pub fn config(&self) -> Result<IndexConfig, String> {
        let start = self.header.config_offset as usize;
        let end = start + self.header.config_len as usize;
        if end > self.data.len() {
            return Err("config blob out of bounds".into());
        }
        IndexConfig::from_compressed(&self.data[start..end])
            .map_err(|e| e.to_string())
    }

    pub fn trigram_stats(&self) -> TrigramStats {
        let n = self.header.num_trigrams as usize;
        let dir_start = self.header.trigram_dir_offset as usize;

        let mut stats = TrigramStats {
            count: n,
            ..Default::default()
        };

        for i in 0..n {
            let entry_off = dir_start + i * TrigramEntry::ENTRY_SIZE;
            if entry_off + TrigramEntry::ENTRY_SIZE > self.data.len() {
                break;
            }
            let entry_bytes = &self.data[entry_off..entry_off + TrigramEntry::ENTRY_SIZE];
            let entry = TrigramEntry::from_bytes(entry_bytes);

            stats.total_bitmap_bytes += entry.bitmap_len as u64;

            let bm_start = self.header.bitmap_data_offset as usize + entry.bitmap_offset as usize;
            let bm_end = bm_start + entry.bitmap_len as usize;
            if bm_end <= self.data.len() {
                let bm_data = &self.data[bm_start..bm_end];
                if let Ok(bm) = RoaringBitmap::deserialize_from(bm_data) {
                    let card = bm.len();
                    stats.total_docs_indexed += card;
                    if n == 1 || stats.min_docs == 0 || card < stats.min_docs {
                        stats.min_docs = card;
                    }
                    if card > stats.max_docs {
                        stats.max_docs = card;
                    }
                }
            }
        }

        if n > 0 {
            stats.avg_docs = stats.total_docs_indexed / n as u64;
        }

        stats
    }
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct TrigramStats {
    pub count: usize,
    pub total_bitmap_bytes: u64,
    pub min_docs: u64,
    pub max_docs: u64,
    pub avg_docs: u64,
    pub total_docs_indexed: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_roundtrip() {
        let h = Header {
            num_files: 100,
            num_trigrams: 5000,
            config_offset: 72,
            config_len: 200,
            file_dir_offset: 272,
            file_offset_dir_offset: 808,
            trigram_dir_offset: 816,
            bitmap_data_offset: 1000,
        };
        let bytes = h.to_bytes();
        assert_eq!(bytes.len(), Header::SIZE);
        let h2 = Header::from_bytes(&bytes).unwrap();
        assert_eq!(h2.num_files, 100);
        assert_eq!(h2.num_trigrams, 5000);
        assert_eq!(h2.config_offset, 72);
        assert_eq!(h2.config_len, 200);
        assert_eq!(h2.file_dir_offset, 272);
        assert_eq!(h2.file_offset_dir_offset, 808);
        assert_eq!(h2.trigram_dir_offset, 816);
        assert_eq!(h2.bitmap_data_offset, 1000);
    }

    #[test]
    fn test_invalid_magic_rejected() {
        let bad = [0u8; Header::SIZE];
        assert!(Header::from_bytes(&bad).is_err());
    }

    #[test]
    fn test_wrong_version_rejected() {
        let mut buf = [0u8; Header::SIZE];
        buf[0..8].copy_from_slice(MAGIC_FULL);
        buf[8..12].copy_from_slice(&99u32.to_le_bytes());
        assert!(Header::from_bytes(&buf).is_err());
    }

    #[test]
    fn test_index_writer_reader_roundtrip() {
        let mut writer = IndexWriter::new();
        writer.add_file("/home/user/foo.rs", 1234, 1746720000, 0o644, "text/x-rust");
        writer.add_file("/home/user/bar.txt", 5678, 1746720100, 0o644, "text/plain");
        writer.add_file("/etc/config.toml", 100, 1746700000, 0o600, "application/octet-stream");

        use crate::index::trigram;
        for (id, path) in ["/home/user/foo.rs", "/home/user/bar.txt", "/etc/config.toml"]
            .iter().enumerate()
        {
            let tris = trigram::generate_trigrams_lowercase(path);
            for tri in &tris {
                writer.add_trigram_doc(trigram::trigram_to_bytes(tri), id as u32);
            }
        }
        writer.prune_empty();

        let config = IndexConfig {
            indexed_paths: vec!["/home".into(), "/etc".into()],
            pruned_paths: vec![],
            timestamp: 1746720000,
            hostname: "test".into(),
            total_bytes_indexed: 7000,
            mlocate_version: "0.1.0".into(),
        };

        let bytes = writer.into_bytes(&config).unwrap();

        let reader = IndexReader::from_bytes(&bytes).unwrap();
        assert_eq!(reader.num_files(), 3);

        let entry0 = reader.file_entry(0).unwrap();
        assert_eq!(entry0.full_path, "/home/user/foo.rs");
        assert_eq!(entry0.size, 1234);
        assert_eq!(entry0.mime_type, "text/x-rust");

        let entry2 = reader.file_entry(2).unwrap();
        assert_eq!(entry2.full_path, "/etc/config.toml");

        let foo_tri = trigram::trigram_to_bytes("foo");
        let bm = reader.trigram_bitmap(foo_tri).unwrap();
        assert!(bm.contains(0));
        assert!(!bm.contains(2));

        let config2 = reader.config().unwrap();
        assert_eq!(config2.indexed_paths[0], "/home");
        assert_eq!(config2.hostname, "test");
    }

    #[test]
    fn test_single_file_index() {
        let mut writer = IndexWriter::new();
        writer.add_file("/foo", 100, 1, 0, "text/plain");
        use crate::index::trigram;
        let tris = trigram::generate_trigrams_lowercase("/foo");
        for tri in &tris {
            writer.add_trigram_doc(trigram::trigram_to_bytes(tri), 0);
        }
        writer.prune_empty();

        let config = IndexConfig {
            indexed_paths: vec!["/".into()],
            pruned_paths: vec![],
            timestamp: 1,
            hostname: "test".into(),
            total_bytes_indexed: 100,
            mlocate_version: "0.1.0".into(),
        };

        let bytes = writer.into_bytes(&config).unwrap();
        let reader = IndexReader::from_bytes(&bytes).unwrap();
        assert_eq!(reader.num_files(), 1);
        assert!(reader.num_trigrams() > 0);
    }

    impl IndexReader<'_> {
        fn num_trigrams(&self) -> usize {
            self.header.num_trigrams as usize
        }
    }
}
