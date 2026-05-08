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
