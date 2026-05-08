pub mod cli;
pub mod compat;
pub mod cron;
pub mod crawl;
pub mod error;
pub mod filter;
pub mod index;
pub mod output;
pub mod pipeline;
pub mod platform;

pub use error::MlocateError;

pub type Result<T> = std::result::Result<T, anyhow::Error>;
