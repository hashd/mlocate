#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct TrigramStats {
    pub count: usize,
    pub total_bitmap_bytes: u64,
    pub min_docs: u64,
    pub max_docs: u64,
    pub avg_docs: u64,
    pub total_docs_indexed: u64,
}
