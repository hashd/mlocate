use std::io;

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
        TrigramEntry {
            trigram,
            bitmap_offset,
            bitmap_len,
        }
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
        BigramEntry {
            bigram,
            bitmap_offset,
            bitmap_len,
        }
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
        ExtBitmapEntry {
            ext,
            bitmap_offset,
            bitmap_len,
        }
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
        let json =
            serde_json::to_vec(self).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        zstd::encode_all(&json[..], 3).map_err(io::Error::other)
    }

    pub fn from_compressed(data: &[u8]) -> io::Result<Self> {
        let json =
            zstd::decode_all(data).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        serde_json::from_slice(&json).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }
}

pub use super::dir::DirTableEntry;
pub use super::header::{
    Header, FEATURE_BIGRAM_INDEX, FEATURE_DIR_TABLE, FEATURE_EXT_INDEX, FORMAT_VERSION,
    HEADER_SIZE_V1, HEADER_SIZE_V2, MAGIC, MAGIC_FULL,
};
pub use super::reader::IndexReader;
pub use super::stats::TrigramStats;
pub use super::writer::IndexWriter;
