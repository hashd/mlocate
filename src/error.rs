use thiserror::Error;

#[derive(Error, Debug)]
pub enum MlocateError {
    #[error("No index found at {path}. Run 'mupdatedb' to create one.")]
    IndexNotFound { path: String },

    #[error(
        "Index version {found} incompatible (expected {expected}). Re-index with 'mupdatedb'."
    )]
    IndexVersionMismatch { found: u32, expected: u32 },

    #[error(
        "Index format error: {details}. The index may be corrupt. Try 'mupdatedb' to rebuild."
    )]
    IndexFormatError { details: String },

    #[error("Invalid size filter '{input}'. Expected format: <value><unit><suffix> (e.g., '10MB+', '1KB-', '500MB').")]
    InvalidSizeFilter { input: String },

    #[error("Invalid time filter '{input}'. Expected format: <value><unit><suffix> (e.g., '2d-', '1w+', '30m').")]
    InvalidTimeFilter { input: String },

    #[error(
        "Invalid MIME type '{input}'. Expected format: 'image/png' (exact) or 'image/*' (glob)."
    )]
    InvalidMimeType { input: String },

    #[error("Index is locked by another process. If no other process is running, remove {path} and retry.")]
    IndexLocked { path: String },

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, MlocateError>;
