pub const SCHEMA_VERSION: i32 = 1;

pub const CREATE_DIRECTORIES: &str = "
    CREATE TABLE IF NOT EXISTS directories (
        id      INTEGER PRIMARY KEY,
        path    TEXT UNIQUE,
        mtime   INTEGER
    );
";

pub const CREATE_FILES: &str = "
    CREATE TABLE IF NOT EXISTS files (
        id          INTEGER PRIMARY KEY,
        full_path   TEXT UNIQUE,
        size        INTEGER,
        mtime       INTEGER,
        mode        INTEGER,
        mime_type   TEXT
    );
";

pub const CREATE_SEQUENCES: &str = "
    CREATE SEQUENCE IF NOT EXISTS files_id_seq;
";

pub const CREATE_TRIGRAMS: &str = "
    CREATE TABLE IF NOT EXISTS trigrams (
        trigram     CHAR(3),
        file_id     INTEGER REFERENCES files(id)
    );
    CREATE INDEX IF NOT EXISTS idx_trigrams_trigram ON trigrams(trigram);
";

pub const CREATE_SCHEMA_VERSION: &str = "
    CREATE TABLE IF NOT EXISTS schema_version (version INTEGER);
";

pub const GET_USER_VERSION: &str = "SELECT version FROM schema_version";
pub const SET_USER_VERSION: &str = "INSERT INTO schema_version (version) VALUES (1)";
