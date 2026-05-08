use thiserror::Error;

#[derive(Error, Debug)]
pub enum MlocateError {
    #[error("No index found at {path}. Run 'mupdatedb' to create one.")]
    IndexNotFound { path: String },

    #[error("Index version {found} incompatible (expected {expected}). Re-index with 'mupdatedb'.")]
    IndexVersionMismatch { found: u32, expected: u32 },

    #[error("Cannot open index at {path}: {details}")]
    CannotOpenIndex { path: String, details: String },

    #[error("Index format error: {details}. The index may be corrupt. Try 'mupdatedb' to rebuild.")]
    IndexFormatError { details: String },

    #[error("Invalid size filter '{input}'. Expected format: <value><unit><suffix> (e.g., '10MB+', '1KB-', '500MB').")]
    InvalidSizeFilter { input: String },

    #[error("Invalid time filter '{input}'. Expected format: <value><unit><suffix> (e.g., '2d-', '1w+', '30m').")]
    InvalidTimeFilter { input: String },

    #[error("Invalid MIME type '{input}'. Expected format: 'image/png' (exact) or 'image/*' (glob).")]
    InvalidMimeType { input: String },

    #[error("A search pattern is required. Usage: mlocate [OPTIONS] <pattern>")]
    PatternRequired,

    #[error("{flag1} and {flag2} cannot be used together.")]
    ConflictingFlags { flag1: String, flag2: String },

    #[error("Stale temp index at {path} from a previous crashed run. Delete it and retry.")]
    StaleTempIndex { path: String },

    #[error("No default paths configured for this platform.")]
    NoDefaultPaths,

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, anyhow::Error>;
