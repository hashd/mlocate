use roaring::RoaringBitmap;
use std::collections::BTreeMap;
use std::io::{self, Seek, Write};

use super::dir::DirTableEntry;
use super::format::IndexConfig;
use super::header::{
    Header, FEATURE_BIGRAM_INDEX, FEATURE_DIR_TABLE, FEATURE_EXT_INDEX, FORMAT_VERSION,
    HEADER_SIZE_V2,
};

pub struct IndexWriter {
    pub file_dir_buf: io::Cursor<Vec<u8>>,
    pub file_offsets: Vec<u64>,
    pub trigrams: BTreeMap<[u8; 3], RoaringBitmap>,
    pub bigrams: BTreeMap<[u8; 2], RoaringBitmap>,
    pub ext_bitmaps: BTreeMap<[u8; 8], RoaringBitmap>,
    pub dir_entries: Vec<DirTableEntry>,
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

    pub fn add_file(
        &mut self,
        full_path: &str,
        size: u64,
        mtime: i64,
        mode: u32,
        mime_type: &str,
    ) -> u32 {
        let id = self.file_offsets.len() as u32;
        self.file_offsets.push(self.file_dir_buf.position());

        let path_bytes = full_path.as_bytes();
        let mime_bytes = mime_type.as_bytes();
        self.file_dir_buf
            .write_all(&(path_bytes.len() as u32).to_le_bytes())
            .unwrap();
        self.file_dir_buf.write_all(path_bytes).unwrap();
        self.file_dir_buf.write_all(&size.to_le_bytes()).unwrap();
        self.file_dir_buf.write_all(&mtime.to_le_bytes()).unwrap();
        self.file_dir_buf.write_all(&mode.to_le_bytes()).unwrap();
        self.file_dir_buf
            .write_all(&(mime_bytes.len() as u8).to_le_bytes())
            .unwrap();
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

    pub fn add_dir(
        &mut self,
        path: &str,
        mtime: i64,
        ino: u64,
        first_file_id: u32,
        num_files: u32,
    ) {
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

        // Sort directory table entries by path for binary search
        let mut sorted_dir_entries: Vec<&DirTableEntry> = self.dir_entries.iter().collect();
        sorted_dir_entries.sort_by(|a, b| a.path.cmp(&b.path));

        let _dir_table_start = w.stream_position()?;
        for de in &sorted_dir_entries {
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
        let mut data = buf.into_inner();
        let crc = crc32fast::hash(&data);
        data.extend_from_slice(&crc.to_le_bytes());
        Ok(data)
    }

    pub fn file_count(&self) -> usize {
        self.file_offsets.len()
    }
}
