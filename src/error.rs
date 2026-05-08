use thiserror::Error;

#[derive(Error, Debug)]
pub enum MlocateError {
    #[error("No database found at {path}. Run 'mupdatedb' to create one.")]
    DatabaseNotFound { path: String },

    #[error("Database schema version {found} is not compatible (expected {expected}). Re-index with 'mupdatedb --force'.")]
    SchemaVersionMismatch { found: i32, expected: i32 },

    #[error("Database query failed: {details}. The index may be corrupt. Try running 'mupdatedb --force' to rebuild.")]
    DatabaseQueryFailed { details: String },

    #[error("Invalid size filter '{input}'. Expected format: <value><unit><suffix> (e.g., '10MB+', '1KB-', '500MB').")]
    InvalidSizeFilter { input: String },

    #[error("Invalid time filter '{input}'. Expected format: <value><unit><suffix> (e.g., '2d-', '1w+', '30m').")]
    InvalidTimeFilter { input: String },

    #[error("Invalid MIME type '{input}'. Expected format: 'image/png' (exact) or 'image/*' (glob).")]
    InvalidMimeType { input: String },

    #[error("A search pattern is required. Usage: mlocate [OPTIONS] <pattern>")]
    PatternRequired,

    #[error("Cannot open database at {path}: {details}.")]
    CannotOpenDatabase { path: String, details: String },

    #[error("{flag1} and {flag2} cannot be used together.")]
    ConflictingFlags { flag1: String, flag2: String },

    #[error("Stale temp database found at {path} from a previous crashed run. Delete it and retry.")]
    StaleTempDatabase { path: String },

    #[error("Failed to checkpoint WAL file after {attempts} retries. Database at {path} may be inconsistent.")]
    WalCheckpointFailed { path: String, attempts: u32 },

    #[error("No default paths configured for this platform.")]
    NoDefaultPaths,

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, anyhow::Error>;
