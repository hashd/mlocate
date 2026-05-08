use super::reader::IndexReader;

#[derive(Debug, Clone)]
pub struct DirTableEntry {
    pub path: String,
    pub mtime: i64,
    pub ino: u64,
    pub first_file_id: u32,
    pub num_files: u32,
}

impl IndexReader<'_> {
    pub fn dir_entry(&self, dir_path: &str) -> Option<DirTableEntry> {
        if !self.has_feature(super::header::FEATURE_DIR_TABLE) || self.header.num_dir_entries == 0 {
            return None;
        }
        let dir_start = self.header.dir_table_offset as usize;
        let num_entries = self.header.num_dir_entries as usize;

        let mut lo = 0usize;
        let mut hi = num_entries;
        while lo < hi {
            let mid = (lo + hi) / 2;
            let (path, mtime, ino, first_file_id, num_files) =
                read_dir_entry_at(self.data, dir_start, mid)?;
            match path.as_str().cmp(dir_path) {
                std::cmp::Ordering::Less => lo = mid + 1,
                std::cmp::Ordering::Greater => hi = mid,
                std::cmp::Ordering::Equal => {
                    return Some(DirTableEntry {
                        path,
                        mtime,
                        ino,
                        first_file_id,
                        num_files,
                    });
                }
            }
        }
        None
    }

    pub fn dir_entries_for_prefix(&self, prefix: &str) -> Vec<DirTableEntry> {
        let mut results = Vec::new();
        if !self.has_feature(super::header::FEATURE_DIR_TABLE) || self.header.num_dir_entries == 0 {
            return results;
        }
        let dir_start = self.header.dir_table_offset as usize;
        let num_entries = self.header.num_dir_entries as usize;

        let start_idx = match find_first_prefix(self.data, dir_start, num_entries, prefix) {
            Some(idx) => idx,
            None => return results,
        };

        for i in start_idx..num_entries {
            if let Some((path, mtime, ino, first_file_id, num_files)) =
                read_dir_entry_at(self.data, dir_start, i)
            {
                if !path.starts_with(prefix) {
                    break;
                }
                results.push(DirTableEntry {
                    path,
                    mtime,
                    ino,
                    first_file_id,
                    num_files,
                });
            } else {
                break;
            }
        }
        results
    }
}

fn read_dir_entry_at(
    data: &[u8],
    dir_start: usize,
    idx: usize,
) -> Option<(String, i64, u64, u32, u32)> {
    let mut pos = dir_start;
    for counter in 0..=idx {
        if pos + 4 > data.len() {
            return None;
        }
        let path_len =
            u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        pos += 4;
        if pos + path_len > data.len() {
            return None;
        }
        let path = std::str::from_utf8(&data[pos..pos + path_len])
            .ok()?
            .to_string();
        pos += path_len;
        if pos + 28 > data.len() {
            return None;
        }
        let mtime = i64::from_le_bytes(data[pos..pos + 8].try_into().ok()?);
        let ino = u64::from_le_bytes(data[pos + 8..pos + 16].try_into().ok()?);
        let first_file_id = u32::from_le_bytes(data[pos + 16..pos + 20].try_into().ok()?);
        let num_files = u32::from_le_bytes(data[pos + 20..pos + 24].try_into().ok()?);
        pos += 28;
        if counter == idx {
            return Some((path, mtime, ino, first_file_id, num_files));
        }
    }
    None
}

fn find_first_prefix(
    data: &[u8],
    dir_start: usize,
    num_entries: usize,
    prefix: &str,
) -> Option<usize> {
    let mut lo = 0usize;
    let mut hi = num_entries;
    while lo < hi {
        let mid = (lo + hi) / 2;
        let path = match read_dir_entry_at(data, dir_start, mid) {
            Some((p, ..)) => p,
            None => return None,
        };
        if path.as_str() < prefix {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    if lo < num_entries {
        Some(lo)
    } else {
        None
    }
}
