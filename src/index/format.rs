use std::io::{self, Write, Seek};
use roaring::RoaringBitmap;
use std::collections::BTreeMap;
use memmap2::Mmap;

pub const MAGIC: &[u8; 7] = b"mlocate";
pub const MAGIC_FULL: &[u8; 8] = b"mlocate\0";
pub const FORMAT_VERSION: u32 = 2;

pub const HEADER_SIZE_V1: usize = 72;
pub const HEADER_SIZE_V2: usize = 128;

pub const FEATURE_DIR_TABLE: u64 = 1 << 0;
pub const FEATURE_EXT_INDEX: u64 = 1 << 1;
pub const FEATURE_BIGRAM_INDEX: u64 = 1 << 2;

#[derive(Debug, Clone)]
pub struct Header {
    pub version: u32,
    pub num_files: u64,
    pub num_trigrams: u64,
    pub config_offset: u64,
    pub config_len: u32,
    pub file_dir_offset: u64,
    pub file_offset_dir_offset: u64,
    pub trigram_dir_offset: u64,
    pub bitmap_data_offset: u64,
    pub dir_table_offset: u64,
    pub num_dir_entries: u64,
    pub ext_bitmap_dir_offset: u64,
    pub num_ext_entries: u64,
    pub bi_gram_dir_offset: u64,
    pub num_bi_gram_entries: u64,
    pub features: u64,
}

impl Header {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = vec![0u8; HEADER_SIZE_V2];
        buf[0..8].copy_from_slice(MAGIC_FULL);
        buf[8..12].copy_from_slice(&self.version.to_le_bytes());
        buf[12..20].copy_from_slice(&self.num_files.to_le_bytes());
        buf[20..28].copy_from_slice(&self.num_trigrams.to_le_bytes());
        buf[28..36].copy_from_slice(&self.config_offset.to_le_bytes());
        buf[36..40].copy_from_slice(&self.config_len.to_le_bytes());
        buf[40..48].copy_from_slice(&self.file_dir_offset.to_le_bytes());
        buf[48..56].copy_from_slice(&self.file_offset_dir_offset.to_le_bytes());
        buf[56..64].copy_from_slice(&self.trigram_dir_offset.to_le_bytes());
        buf[64..72].copy_from_slice(&self.bitmap_data_offset.to_le_bytes());
        buf[72..80].copy_from_slice(&self.dir_table_offset.to_le_bytes());
        buf[80..88].copy_from_slice(&self.num_dir_entries.to_le_bytes());
        buf[88..96].copy_from_slice(&self.ext_bitmap_dir_offset.to_le_bytes());
        buf[96..104].copy_from_slice(&self.num_ext_entries.to_le_bytes());
        buf[104..112].copy_from_slice(&self.bi_gram_dir_offset.to_le_bytes());
        buf[112..120].copy_from_slice(&self.num_bi_gram_entries.to_le_bytes());
        buf[120..128].copy_from_slice(&self.features.to_le_bytes());
        buf
    }

    pub fn from_bytes(buf: &[u8]) -> Result<Self, &'static str> {
        if buf.len() < HEADER_SIZE_V1 {
            return Err("header too short");
        }
        if &buf[0..7] != MAGIC {
            return Err("invalid magic bytes");
        }
        let version = u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]);
        if version != 1 && version != FORMAT_VERSION {
            return Err("unsupported format version");
        }

        let read_u64 = |off: usize| -> u64 {
            u64::from_le_bytes(buf[off..off + 8].try_into().unwrap())
        };
        let read_u32 = |off: usize| -> u32 {
            u32::from_le_bytes(buf[off..off + 4].try_into().unwrap())
        };

        let v2 = version == FORMAT_VERSION;
        Ok(Header {
            version,
            num_files: read_u64(12),
            num_trigrams: read_u64(20),
            config_offset: read_u64(28),
            config_len: read_u32(36),
            file_dir_offset: read_u64(40),
            file_offset_dir_offset: read_u64(48),
            trigram_dir_offset: read_u64(56),
            bitmap_data_offset: read_u64(64),
            dir_table_offset: if v2 { read_u64(72) } else { 0 },
            num_dir_entries: if v2 { read_u64(80) } else { 0 },
            ext_bitmap_dir_offset: if v2 { read_u64(88) } else { 0 },
            num_ext_entries: if v2 { read_u64(96) } else { 0 },
            bi_gram_dir_offset: if v2 { read_u64(104) } else { 0 },
            num_bi_gram_entries: if v2 { read_u64(112) } else { 0 },
            features: if v2 { read_u64(120) } else { 0 },
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

#[derive(Debug, Clone)]
pub struct BigramEntry {
    pub bigram: [u8; 2],
    pub bitmap_offset: u64,
    pub bitmap_len: u32,
}

impl BigramEntry {
    pub const ENTRY_SIZE: usize = 14;

    pub fn from_bytes(buf: &[u8]) -> Self {
        let bigram = [buf[0], buf[1]];
        let bitmap_offset = u64::from_le_bytes(buf[2..10].try_into().unwrap());
        let bitmap_len = u32::from_le_bytes(buf[10..14].try_into().unwrap());
        BigramEntry { bigram, bitmap_offset, bitmap_len }
    }
}

#[derive(Debug, Clone)]
pub struct ExtBitmapEntry {
    pub ext: [u8; 8],
    pub bitmap_offset: u64,
    pub bitmap_len: u32,
}

impl ExtBitmapEntry {
    pub const ENTRY_SIZE: usize = 20;

    pub fn from_bytes(buf: &[u8]) -> Self {
        let ext = buf[0..8].try_into().unwrap();
        let bitmap_offset = u64::from_le_bytes(buf[8..16].try_into().unwrap());
        let bitmap_len = u32::from_le_bytes(buf[16..20].try_into().unwrap());
        ExtBitmapEntry { ext, bitmap_offset, bitmap_len }
    }
}

#[derive(Debug, Clone)]
pub struct DirTableEntry {
    pub path: String,
    pub mtime: i64,
    pub ino: u64,
    pub first_file_id: u32,
    pub num_files: u32,
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
    bigrams: BTreeMap<[u8; 2], RoaringBitmap>,
    ext_bitmaps: BTreeMap<[u8; 8], RoaringBitmap>,
    dir_entries: Vec<DirTableEntry>,
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
            bigrams: BTreeMap::new(),
            ext_bitmaps: BTreeMap::new(),
            dir_entries: Vec::new(),
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

    pub fn add_bigram_doc(&mut self, bigram: [u8; 2], doc_id: u32) {
        self.bigrams.entry(bigram).or_default().insert(doc_id);
    }

    pub fn add_ext_doc(&mut self, ext: [u8; 8], doc_id: u32) {
        self.ext_bitmaps.entry(ext).or_default().insert(doc_id);
    }

    pub fn add_dir(&mut self, path: &str, mtime: i64, ino: u64, first_file_id: u32, num_files: u32) {
        self.dir_entries.push(DirTableEntry {
            path: path.to_string(),
            mtime,
            ino,
            first_file_id,
            num_files,
        });
    }

    pub fn prune_empty(&mut self) {
        self.trigrams.retain(|_, bm| !bm.is_empty());
        self.bigrams.retain(|_, bm| !bm.is_empty());
        self.ext_bitmaps.retain(|_, bm| !bm.is_empty());
    }

    pub fn write_to<W: Write + Seek>(&self, w: &mut W, config: &IndexConfig) -> io::Result<u64> {
        let num_files = self.file_offsets.len() as u64;
        let file_dir_data = self.file_dir_buf.get_ref();

        let features = FEATURE_DIR_TABLE | FEATURE_EXT_INDEX | FEATURE_BIGRAM_INDEX;

        let placeholder = vec![0u8; HEADER_SIZE_V2];
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
        w.write_all(&(file_dir_data.len() as u64).to_le_bytes())?;

        let trigram_dir_offset = w.stream_position()?;
        let num_trigrams = self.trigrams.len() as u64;
        let bitmap_data_base = trigram_dir_offset + (num_trigrams * 15);

        let bigram_dir_offset = bitmap_data_base;
        let bigram_data_base = bigram_dir_offset + (self.bigrams.len() as u64) * 14;
        let ext_dir_offset = bigram_data_base;
        let ext_data_base = ext_dir_offset + (self.ext_bitmaps.len() as u64) * 20;
        let dir_table_offset = ext_data_base;

        let trigram_dir_start = w.stream_position()?;
        w.write_all(&vec![0u8; (num_trigrams as usize) * 15])?;
        let _bigram_dir_start = w.stream_position()?;
        w.write_all(&vec![0u8; self.bigrams.len() * 14])?;
        let _ext_dir_start = w.stream_position()?;
        w.write_all(&vec![0u8; self.ext_bitmaps.len() * 20])?;

        let _dir_table_start = w.stream_position()?;
        for de in &self.dir_entries {
            let path_bytes = de.path.as_bytes();
            w.write_all(&(path_bytes.len() as u32).to_le_bytes())?;
            w.write_all(path_bytes)?;
            w.write_all(&de.mtime.to_le_bytes())?;
            w.write_all(&de.ino.to_le_bytes())?;
            w.write_all(&de.first_file_id.to_le_bytes())?;
            w.write_all(&de.num_files.to_le_bytes())?;
        }
        let dir_table_end = w.stream_position()?;
        let num_dir_entries = self.dir_entries.len() as u64;

        let bitmap_data_offset = dir_table_end;

        let mut trigram_entries: Vec<(u64, u32)> = Vec::with_capacity(num_trigrams as usize);
        for bitmap in self.trigrams.values() {
            let offset = w.stream_position()? - bitmap_data_offset;
            let len_before = w.stream_position()?;
            bitmap.serialize_into(&mut *w).map_err(io::Error::other)?;
            let len_after = w.stream_position()?;
            let len = (len_after - len_before) as u32;
            trigram_entries.push((offset, len));
        }

        let mut bigram_entries: Vec<(u64, u32)> = Vec::with_capacity(self.bigrams.len());
        for bitmap in self.bigrams.values() {
            let offset = w.stream_position()? - bitmap_data_offset;
            let len_before = w.stream_position()?;
            bitmap.serialize_into(&mut *w).map_err(io::Error::other)?;
            let len_after = w.stream_position()?;
            let len = (len_after - len_before) as u32;
            bigram_entries.push((offset, len));
        }

        let mut ext_entries: Vec<(u64, u32)> = Vec::with_capacity(self.ext_bitmaps.len());
        for bitmap in self.ext_bitmaps.values() {
            let offset = w.stream_position()? - bitmap_data_offset;
            let len_before = w.stream_position()?;
            bitmap.serialize_into(&mut *w).map_err(io::Error::other)?;
            let len_after = w.stream_position()?;
            let len = (len_after - len_before) as u32;
            ext_entries.push((offset, len));
        }
        let bitmap_end = w.stream_position()?;

        w.seek(io::SeekFrom::Start(trigram_dir_start))?;
        for ((trigram, _), (offset, len)) in self.trigrams.iter().zip(trigram_entries.iter()) {
            w.write_all(&trigram[..])?;
            w.write_all(&offset.to_le_bytes())?;
            w.write_all(&len.to_le_bytes())?;
        }

        for ((bigram, _), (offset, len)) in self.bigrams.iter().zip(bigram_entries.iter()) {
            w.write_all(&bigram[..])?;
            w.write_all(&offset.to_le_bytes())?;
            w.write_all(&len.to_le_bytes())?;
        }

        for ((ext, _), (offset, len)) in self.ext_bitmaps.iter().zip(ext_entries.iter()) {
            w.write_all(&ext[..])?;
            w.write_all(&offset.to_le_bytes())?;
            w.write_all(&len.to_le_bytes())?;
        }

        w.seek(io::SeekFrom::Start(bitmap_end))?;

        let header = Header {
            version: FORMAT_VERSION,
            num_files,
            num_trigrams,
            config_offset,
            config_len,
            file_dir_offset,
            file_offset_dir_offset,
            trigram_dir_offset,
            bitmap_data_offset,
            dir_table_offset,
            num_dir_entries,
            ext_bitmap_dir_offset: ext_dir_offset,
            num_ext_entries: self.ext_bitmaps.len() as u64,
            bi_gram_dir_offset: bigram_dir_offset,
            num_bi_gram_entries: self.bigrams.len() as u64,
            features,
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

    pub fn file_count(&self) -> usize {
        self.file_offsets.len()
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

    pub fn has_feature(&self, feature: u64) -> bool {
        self.header.features & feature != 0
    }

    pub fn file_entry(&self, doc_id: u32) -> Result<DiskFileEntry, String> {
        if doc_id as u64 >= self.header.num_files {
            return Err(format!("doc_id {} out of range (max {})", doc_id, self.header.num_files));
        }
        let off_dir = self.header.file_offset_dir_offset as usize;
        let off_idx = off_dir + (doc_id as usize) * 8;
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
        binary_search_bitmap(
            self.data,
            self.header.trigram_dir_offset as usize,
            self.header.num_trigrams as usize,
            TrigramEntry::ENTRY_SIZE,
            &trigram,
            3,
            self.header.bitmap_data_offset as usize,
        )
    }

    pub fn bigram_bitmap(&self, bigram: [u8; 2]) -> Option<RoaringBitmap> {
        if !self.has_feature(FEATURE_BIGRAM_INDEX) || self.header.num_bi_gram_entries == 0 {
            return None;
        }
        binary_search_bitmap(
            self.data,
            self.header.bi_gram_dir_offset as usize,
            self.header.num_bi_gram_entries as usize,
            BigramEntry::ENTRY_SIZE,
            &bigram,
            2,
            self.header.bitmap_data_offset as usize,
        )
    }

    pub fn ext_bitmap(&self, ext: &[u8]) -> Option<RoaringBitmap> {
        if !self.has_feature(FEATURE_EXT_INDEX) || self.header.num_ext_entries == 0 {
            return None;
        }
        let mut key = [0u8; 8];
        let len = ext.len().min(8);
        key[..len].copy_from_slice(&ext[..len]);
        binary_search_bitmap(
            self.data,
            self.header.ext_bitmap_dir_offset as usize,
            self.header.num_ext_entries as usize,
            ExtBitmapEntry::ENTRY_SIZE,
            &key,
            8,
            self.header.bitmap_data_offset as usize,
        )
    }

    pub fn dir_entry(&self, dir_path: &str) -> Option<DirTableEntry> {
        if !self.has_feature(FEATURE_DIR_TABLE) || self.header.num_dir_entries == 0 {
            return None;
        }
        let dir_start = self.header.dir_table_offset as usize;
        let mut pos = dir_start;
        let dir_end = self.header.bitmap_data_offset as usize;

        while pos + 4 <= dir_end && pos < self.data.len() {
            let path_len = u32::from_le_bytes([
                self.data[pos], self.data[pos+1], self.data[pos+2], self.data[pos+3]
            ]) as usize;
            pos += 4;
            if pos + path_len > self.data.len() { break; }
            let path = std::str::from_utf8(&self.data[pos..pos + path_len]).ok()?;
            pos += path_len;

            if pos + 28 > self.data.len() { break; }
            let mtime = i64::from_le_bytes(self.data[pos..pos+8].try_into().ok()?);
            let ino = u64::from_le_bytes(self.data[pos+8..pos+16].try_into().ok()?);
            let first_file_id = u32::from_le_bytes(self.data[pos+16..pos+20].try_into().ok()?);
            let num_files = u32::from_le_bytes(self.data[pos+20..pos+24].try_into().ok()?);
            pos += 28;

            if path == dir_path {
                return Some(DirTableEntry {
                    path: path.to_string(),
                    mtime,
                    ino,
                    first_file_id,
                    num_files,
                });
            }
        }
        None
    }

    pub fn dir_entries_for_prefix(&self, prefix: &str) -> Vec<DirTableEntry> {
        let mut results = Vec::new();
        if !self.has_feature(FEATURE_DIR_TABLE) || self.header.num_dir_entries == 0 {
            return results;
        }
        let dir_start = self.header.dir_table_offset as usize;
        let mut pos = dir_start;
        let dir_end = self.header.bitmap_data_offset as usize;

        while pos + 4 <= dir_end && pos < self.data.len() {
            let path_len = u32::from_le_bytes([
                self.data[pos], self.data[pos+1], self.data[pos+2], self.data[pos+3]
            ]) as usize;
            pos += 4;
            if pos + path_len > self.data.len() { break; }
            let path = match std::str::from_utf8(&self.data[pos..pos + path_len]) {
                Ok(p) => p,
                Err(_) => break,
            };
            pos += path_len;

            if pos + 28 > self.data.len() { break; }
            let mtime = i64::from_le_bytes(self.data[pos..pos+8].try_into().unwrap());
            let ino = u64::from_le_bytes(self.data[pos+8..pos+16].try_into().unwrap());
            let first_file_id = u32::from_le_bytes(self.data[pos+16..pos+20].try_into().unwrap());
            let num_files = u32::from_le_bytes(self.data[pos+20..pos+24].try_into().unwrap());
            pos += 28;

            if path.starts_with(prefix) {
                results.push(DirTableEntry {
                    path: path.to_string(),
                    mtime,
                    ino,
                    first_file_id,
                    num_files,
                });
            }
        }
        results
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

    pub fn get_files_in_dir(&self, dir_entry: &DirTableEntry) -> Result<Vec<DiskFileEntry>, String> {
        let mut entries = Vec::with_capacity(dir_entry.num_files as usize);
        for i in 0..dir_entry.num_files {
            let doc_id = dir_entry.first_file_id + i;
            entries.push(self.file_entry(doc_id)?);
        }
        Ok(entries)
    }

    pub fn get_file_paths_in_dir(&self, dir_entry: &DirTableEntry) -> Result<Vec<String>, String> {
        let mut paths = Vec::with_capacity(dir_entry.num_files as usize);
        for i in 0..dir_entry.num_files {
            let doc_id = dir_entry.first_file_id + i;
            paths.push(self.file_path(doc_id)?);
        }
        Ok(paths)
    }
}

fn binary_search_bitmap(
    data: &[u8],
    dir_start: usize,
    num_entries: usize,
    entry_size: usize,
    key: &[u8],
    key_len: usize,
    bitmap_data_offset: usize,
) -> Option<RoaringBitmap> {
    let mut lo = 0usize;
    let mut hi = num_entries;
    while lo < hi {
        let mid = (lo + hi) / 2;
        let entry_off = dir_start + mid * entry_size;
        if entry_off + entry_size > data.len() {
            return None;
        }
        let mid_key = &data[entry_off..entry_off + key_len];
        match mid_key.cmp(key) {
            std::cmp::Ordering::Less => lo = mid + 1,
            std::cmp::Ordering::Greater => hi = mid,
            std::cmp::Ordering::Equal => {
                let bm_off = u64::from_le_bytes(
                    data[entry_off + key_len..entry_off + key_len + 8].try_into().ok()?
                ) as usize;
                let bm_len = u32::from_le_bytes(
                    data[entry_off + key_len + 8..entry_off + key_len + 12].try_into().ok()?
                ) as usize;
                let bm_start = bitmap_data_offset + bm_off;
                let bm_end = bm_start + bm_len;
                if bm_end > data.len() {
                    return None;
                }
                return RoaringBitmap::deserialize_from(&data[bm_start..bm_end]).ok();
            }
        }
    }
    None
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
            version: FORMAT_VERSION,
            num_files: 100,
            num_trigrams: 5000,
            config_offset: 128,
            config_len: 200,
            file_dir_offset: 328,
            file_offset_dir_offset: 800,
            trigram_dir_offset: 900,
            bitmap_data_offset: 1000,
            dir_table_offset: 1200,
            num_dir_entries: 5,
            ext_bitmap_dir_offset: 1400,
            num_ext_entries: 10,
            bi_gram_dir_offset: 1600,
            num_bi_gram_entries: 200,
            features: FEATURE_DIR_TABLE | FEATURE_EXT_INDEX | FEATURE_BIGRAM_INDEX,
        };
        let bytes = h.to_bytes();
        assert_eq!(bytes.len(), HEADER_SIZE_V2);
        let h2 = Header::from_bytes(&bytes).unwrap();
        assert_eq!(h2.num_files, 100);
        assert_eq!(h2.num_trigrams, 5000);
        assert_eq!(h2.config_offset, 128);
        assert_eq!(h2.config_len, 200);
        assert_eq!(h2.features, FEATURE_DIR_TABLE | FEATURE_EXT_INDEX | FEATURE_BIGRAM_INDEX);
    }

    #[test]
    fn test_v1_header_read() {
        let mut buf = vec![0u8; HEADER_SIZE_V1];
        buf[0..8].copy_from_slice(MAGIC_FULL);
        buf[8..12].copy_from_slice(&1u32.to_le_bytes());
        buf[12..20].copy_from_slice(&10u64.to_le_bytes());
        let h = Header::from_bytes(&buf).unwrap();
        assert_eq!(h.version, 1);
        assert_eq!(h.num_files, 10);
        assert_eq!(h.features, 0);
        assert_eq!(h.num_dir_entries, 0);
    }

    #[test]
    fn test_invalid_magic_rejected() {
        let bad = [0u8; HEADER_SIZE_V1];
        assert!(Header::from_bytes(&bad).is_err());
    }

    #[test]
    fn test_wrong_version_rejected() {
        let mut buf = vec![0u8; HEADER_SIZE_V1];
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
            let bis = trigram::generate_bigrams_lowercase(path);
            for bi in &bis {
                let key = trigram::bigram_to_bytes(bi);
                writer.add_bigram_doc(key, id as u32);
            }
            if let Some(ext) = std::path::Path::new(path).extension().and_then(|e| e.to_str()) {
                let mut ext_key = [0u8; 8];
                let ext_bytes = ext.as_bytes();
                ext_key[..ext_bytes.len().min(8)].copy_from_slice(&ext_bytes[..ext_bytes.len().min(8)]);
                writer.add_ext_doc(ext_key, id as u32);
            }
        }
        writer.add_dir("/home/user", 1746720000, 12345, 0, 2);
        writer.add_dir("/etc", 1746700000, 67890, 2, 1);
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

        let rs_ext = [b'r', b's', 0, 0, 0, 0, 0, 0];
        let ext_bm = reader.ext_bitmap(&rs_ext).unwrap();
        assert!(ext_bm.contains(0));

        let config2 = reader.config().unwrap();
        assert_eq!(config2.indexed_paths[0], "/home");
        assert_eq!(config2.hostname, "test");

        let dir = reader.dir_entry("/home/user").unwrap();
        assert_eq!(dir.first_file_id, 0);
        assert_eq!(dir.num_files, 2);
        assert_eq!(dir.mtime, 1746720000);
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
