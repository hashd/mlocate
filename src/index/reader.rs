use roaring::RoaringBitmap;
use memmap2::Mmap;

use super::header::{Header, FEATURE_EXT_INDEX, FEATURE_BIGRAM_INDEX};
use super::dir::DirTableEntry;
use super::format::{DiskFileEntry, IndexConfig, TrigramEntry, BigramEntry, ExtBitmapEntry};
use super::stats::TrigramStats;

pub struct IndexReader<'a> {
    pub data: &'a [u8],
    pub header: Header,
}

impl<'a> IndexReader<'a> {
    pub fn new(mmap: &'a Mmap) -> Result<Self, String> {
        let data: &[u8] = mmap.as_ref();
        if data.len() < 4 {
            return Err("index too short (missing CRC footer)".into());
        }
        let stored_crc = u32::from_le_bytes([data[data.len()-4], data[data.len()-3], data[data.len()-2], data[data.len()-1]]);
        let computed_crc = crc32fast::hash(&data[..data.len()-4]);
        if stored_crc != computed_crc {
            return Err(format!(
                "index checksum mismatch (stored: {:08x}, computed: {:08x}). The index may be corrupt or truncated.",
                stored_crc, computed_crc
            ));
        }
        let header = Header::from_bytes(data).map_err(|e| e.to_string())?;
        Ok(IndexReader { data, header })
    }

    pub fn from_bytes(data: &'a [u8]) -> Result<Self, String> {
        if data.len() < 4 {
            return Err("index too short (missing CRC footer)".into());
        }
        let crc_bytes = &data[data.len() - 4..];
        let stored_crc = u32::from_le_bytes([crc_bytes[0], crc_bytes[1], crc_bytes[2], crc_bytes[3]]);
        let computed_crc = crc32fast::hash(&data[..data.len() - 4]);
        if stored_crc != computed_crc {
            return Err(format!(
                "index checksum mismatch (stored: {:08x}, computed: {:08x}). The index may be corrupt or truncated.",
                stored_crc, computed_crc
            ));
        }
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

    pub fn file_path_only(&self, doc_id: u32) -> Result<String, String> {
        if doc_id as u64 >= self.header.num_files {
            return Err(format!("doc_id {} out of range (max {})", doc_id, self.header.num_files));
        }
        let off_dir = self.header.file_offset_dir_offset as usize;
        let off_idx = off_dir + (doc_id as usize) * 8;
        let entry_off = u64::from_le_bytes(
            self.data[off_idx..off_idx + 8].try_into().unwrap()
        );
        let start = self.header.file_dir_offset as usize + entry_off as usize;
        if start + 4 > self.data.len() {
            return Err("file entry too short".into());
        }
        let path_len = u32::from_le_bytes([
            self.data[start], self.data[start+1], self.data[start+2], self.data[start+3]
        ]) as usize;
        let path_start = start + 4;
        if path_start + path_len > self.data.len() {
            return Err("path truncated".into());
        }
        Ok(String::from_utf8_lossy(&self.data[path_start..path_start + path_len]).to_string())
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

    pub fn trigram_bitmaps_batch(&self, keys: &[[u8; 3]], results: &mut std::collections::HashMap<[u8; 3], RoaringBitmap>) {
        if keys.is_empty() { return; }
        let num_entries = self.header.num_trigrams as usize;
        let dir_start = self.header.trigram_dir_offset as usize;
        let entry_size = TrigramEntry::ENTRY_SIZE;
        let bm_base = self.header.bitmap_data_offset as usize;

        let mut sorted_keys: Vec<&[u8; 3]> = keys.iter().collect();
        sorted_keys.sort();
        sorted_keys.dedup();

        let mut ki = 0;
        for i in 0..num_entries {
            let entry_off = dir_start + i * entry_size;
            if entry_off + entry_size > self.data.len() { break; }
            let entry_key: &[u8; 3] = &self.data[entry_off..entry_off + 3].try_into().unwrap();

            while ki < sorted_keys.len() && sorted_keys[ki].as_slice() < entry_key.as_slice() {
                ki += 1;
            }
            if ki >= sorted_keys.len() { break; }
            if sorted_keys[ki].as_slice() > entry_key.as_slice() { continue; }

            let bm_off = u64::from_le_bytes(
                self.data[entry_off + 3..entry_off + 11].try_into().unwrap()
            ) as usize;
            let bm_len = u32::from_le_bytes(
                self.data[entry_off + 11..entry_off + 15].try_into().unwrap()
            ) as usize;
            let bm_start = bm_base + bm_off;
            let bm_end = bm_start + bm_len;
            if bm_end <= self.data.len() {
                if let Ok(bm) = RoaringBitmap::deserialize_from(&self.data[bm_start..bm_end]) {
                    results.insert(*sorted_keys[ki], bm);
                }
            }
            ki += 1;
        }
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
