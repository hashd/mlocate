pub mod schema;
pub mod trigram;
pub mod query;

use crate::error::MlocateError;
use duckdb::Connection;
use std::path::Path;
use std::time::Duration;

fn open_connection(path: &str) -> Result<Connection, MlocateError> {
    Connection::open(path).map_err(|e| MlocateError::CannotOpenDatabase {
        path: path.to_string(),
        details: e.to_string(),
    })
}

pub fn open_or_create(path: &str) -> Result<Connection, MlocateError> {
    let exists = Path::new(path).exists();
    let conn = open_connection(path)?;

    if exists {
        let version: i32 = conn
            .query_row(schema::GET_USER_VERSION, [], |row| row.get(0))
            .map_err(|e| MlocateError::DatabaseQueryFailed {
                details: format!("Failed to read schema version: {}", e),
            })?;

        if version != schema::SCHEMA_VERSION {
            return Err(MlocateError::SchemaVersionMismatch {
                found: version,
                expected: schema::SCHEMA_VERSION,
            });
        }
    } else {
        conn.execute_batch(schema::CREATE_DIRECTORIES)?;
        conn.execute_batch(schema::CREATE_FILES)?;
        conn.execute_batch(schema::CREATE_TRIGRAMS)?;
        conn.execute_batch(schema::CREATE_SCHEMA_VERSION)?;
        conn.execute_batch(schema::SET_USER_VERSION)?;
        conn.execute_batch(schema::CREATE_SEQUENCES)?;
    }

    Ok(conn)
}

pub fn create_temp(path: &str) -> Result<Connection, MlocateError> {
    let conn = open_connection(path)?;
    conn.execute_batch(schema::CREATE_DIRECTORIES)?;
    conn.execute_batch(schema::CREATE_FILES)?;
    conn.execute_batch(schema::CREATE_TRIGRAMS)?;
    conn.execute_batch(schema::CREATE_SCHEMA_VERSION)?;
    conn.execute_batch(schema::SET_USER_VERSION)?;
    conn.execute_batch(schema::CREATE_SEQUENCES)?;
    Ok(conn)
}

pub fn close_and_checkpoint(conn: Connection) -> Result<(), MlocateError> {
    conn.execute_batch("CHECKPOINT;").map_err(|e| {
        MlocateError::DatabaseQueryFailed {
            details: e.to_string(),
        }
    })?;
    drop(conn);
    Ok(())
}

pub fn retry_checkpoint_wal(db_path: &str, max_retries: u32) -> Result<(), MlocateError> {
    let wal_path = format!("{}.wal", db_path);
    for attempt in 0..max_retries {
        if !Path::new(&wal_path).exists() {
            return Ok(());
        }
        if attempt > 0 {
            std::thread::sleep(Duration::from_millis(100));
        }
        match Connection::open(db_path) {
            Ok(conn) => {
                let _ = conn.execute_batch("CHECKPOINT;");
                drop(conn);
            }
            Err(_) => {}
        }
    }
    if Path::new(&wal_path).exists() {
        return Err(MlocateError::WalCheckpointFailed {
            path: db_path.to_string(),
            attempts: max_retries,
        });
    }
    Ok(())
}

pub fn rename_atomic(tmp_path: &str, final_path: &str) -> Result<(), MlocateError> {
    std::fs::rename(tmp_path, final_path).map_err(|e| {
        MlocateError::Other(format!(
            "Failed to rename {} to {}: {}",
            tmp_path, final_path, e
        ))
    })
}

pub fn cleanup_stale_tmp(db_dir: &str) -> Result<(), MlocateError> {
    let tmp_db = format!("{}/mlocate.db.tmp", db_dir);
    let tmp_wal = format!("{}/mlocate.db.tmp.wal", db_dir);
    if Path::new(&tmp_db).exists() {
        std::fs::remove_file(&tmp_db).map_err(|e| {
            MlocateError::Other(format!(
                "Failed to remove stale tmp database {}: {}",
                tmp_db, e
            ))
        })?;
    }
    if Path::new(&tmp_wal).exists() {
        std::fs::remove_file(&tmp_wal).map_err(|e| {
            MlocateError::Other(format!(
                "Failed to remove stale tmp WAL {}: {}",
                tmp_wal, e
            ))
        })?;
    }
    Ok(())
}

pub fn atomic_swap(tmp_path: &str, final_path: &str) -> Result<(), MlocateError> {
    retry_checkpoint_wal(tmp_path, 3)?;
    rename_atomic(tmp_path, final_path)?;
    Ok(())
}
