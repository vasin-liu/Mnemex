//! SQLite infrastructure: bootstrap of the source-of-truth store (ADR-002).
//!
//! Scope of this module: opening the local database, library-level runtime
//! parameters (WAL, busy timeout, foreign keys), and FTS5 availability
//! enforcement. Table DDL and row mapping live in [`crate::schema`].
//!
//! SQLite is the source of truth (ADR-002): every other store (the vector
//! store) is a derived, rebuildable index over it.

use std::error::Error;
use std::fmt;
use std::path::Path;
use std::time::Duration;

use rusqlite::Connection;

/// Busy timeout applied at open time, in milliseconds (design D5).
///
/// Desktop-perceptible ceiling, aimed at Windows file-lock contention
/// between background incremental writes and foreground reads (ADR-001
/// exit-criteria scenarios). Constant, revisited with the Phase 0 device
/// matrix measurements.
const BUSY_TIMEOUT_MS: u64 = 5_000;

/// Error returned by the SQLite infrastructure layer.
///
/// Hand-written on the std path on purpose (design D3): no third-party
/// error helper is admitted in this change.
#[derive(Debug)]
pub enum StoreError {
    /// An error raised by the SQLite driver.
    Sqlite(rusqlite::Error),
    /// The SQLite build lacks FTS5, which is the sole BM25 source for the
    /// Fast Path (ADR-001 item 5). Startup refuses to continue instead of
    /// silently degrading.
    Fts5Unavailable {
        /// What exactly was missing or failed during verification.
        detail: String,
    },
    /// The database schema is newer than the migrations this binary knows.
    /// Opening it would mean guessing at an unknown layout, so startup stops.
    Migration {
        /// `PRAGMA user_version` read from the file.
        db_version: u32,
    },
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sqlite(err) => write!(f, "sqlite error: {err}"),
            Self::Fts5Unavailable { detail } => {
                write!(f, "FTS5 unavailable: {detail}")
            }
            Self::Migration { db_version } => {
                write!(f, "schema version {db_version} is ahead of this build")
            }
        }
    }
}

impl Error for StoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Sqlite(err) => Some(err),
            Self::Fts5Unavailable { .. } | Self::Migration { .. } => None,
        }
    }
}

impl From<rusqlite::Error> for StoreError {
    fn from(err: rusqlite::Error) -> Self {
        Self::Sqlite(err)
    }
}

/// Opened local SQLite database (the source of truth, ADR-002).
///
/// The raw [`Connection`] stays inside the crate: callers go through named
/// methods. [`crate::schema`] uses it for migrations and row access.
#[derive(Debug)]
pub struct SqliteStore {
    pub(crate) conn: Connection,
}

impl SqliteStore {
    /// Creates or opens the SQLite database at `path`.
    ///
    /// Idempotent: a fresh path creates a new database; an existing
    /// database is migrated up to [`crate::schema::SCHEMA_VERSION`].
    /// The open sequence is: open, WAL journal mode, busy timeout,
    /// `foreign_keys=ON`, migrate, then FTS5 verification.
    /// A `user_version` newer than this binary returns [`StoreError::Migration`]
    /// and does not yield a handle.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        let mut conn = Connection::open(path)?;

        // WAL: background incremental indexing writes coexist with
        // foreground reads. The pragma returns the achieved mode; anything
        // other than WAL is a hard failure, not a downgrade.
        let mode: String = conn.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
        if !mode.eq_ignore_ascii_case("wal") {
            return Err(StoreError::Sqlite(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CANTOPEN),
                Some(format!("journal_mode is {mode}, expected WAL")),
            )));
        }

        conn.busy_timeout(Duration::from_millis(BUSY_TIMEOUT_MS))?;
        conn.pragma_update(None, "foreign_keys", 1_i64)?;
        crate::schema::migrate(&mut conn)?;
        verify_fts5(&conn)?;

        Ok(Self { conn })
    }

    /// Runs one statement (DDL or DML) and returns the number of changed
    /// rows. Parameter-less diagnostic and test surface. Row APIs live on
    /// [`crate::schema`] methods of this struct.
    pub fn execute(&self, sql: &str) -> Result<usize, StoreError> {
        Ok(self.conn.execute(sql, [])?)
    }

    /// Reads a single scalar value. Test and diagnostic surface.
    pub fn query_scalar<T: rusqlite::types::FromSql>(&self, sql: &str) -> Result<T, StoreError> {
        Ok(self.conn.query_row(sql, [], |row| row.get::<_, T>(0))?)
    }

    /// Returns the current `journal_mode` (diagnostic; spec scenario).
    pub fn journal_mode(&self) -> Result<String, StoreError> {
        Ok(self
            .conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))?)
    }

    /// Returns the current `busy_timeout` in milliseconds (diagnostic;
    /// spec scenario).
    pub fn busy_timeout_ms(&self) -> Result<i32, StoreError> {
        Ok(self
            .conn
            .query_row("PRAGMA busy_timeout", [], |row| row.get(0))?)
    }
}

/// Verifies that FTS5 is usable on this connection (design D2).
///
/// Two halves, both must hold:
/// 1. `PRAGMA compile_options` contains `ENABLE_FTS5`;
/// 2. a temp-schema fts5 virtual table accepts an insert and answers a
///    MATCH query (the smoke; never touches the user's database).
///
/// FTS5 is the sole BM25 source of the Fast Path (ADR-001 item 5), so a
/// missing extension is a hard startup failure — never a silent degrade.
pub(crate) fn verify_fts5(conn: &Connection) -> Result<(), StoreError> {
    let mut stmt = conn.prepare("PRAGMA compile_options")?;
    let options = stmt.query_map([], |row| row.get::<_, String>(0))?;
    let enabled = options
        .filter_map(|row| row.ok())
        .any(|opt| opt.trim() == "ENABLE_FTS5");
    if !enabled {
        return Err(StoreError::Fts5Unavailable {
            detail: "SQLite build lacks ENABLE_FTS5 in compile options".to_string(),
        });
    }

    conn.execute_batch(
        "CREATE VIRTUAL TABLE temp.mnemex_fts5_smoke USING fts5(text);
         INSERT INTO temp.mnemex_fts5_smoke(text) VALUES ('mnemex fts5 smoke');",
    )?;
    let hits: i64 = conn.query_row(
        "SELECT COUNT(*) FROM temp.mnemex_fts5_smoke \
         WHERE mnemex_fts5_smoke MATCH 'mnemex'",
        [],
        |row| row.get(0),
    )?;
    conn.execute_batch("DROP TABLE temp.mnemex_fts5_smoke;")?;
    if hits != 1 {
        return Err(StoreError::Fts5Unavailable {
            detail: format!("fts5 smoke query matched {hits} rows, expected 1"),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_error_display_and_source() {
        let err = StoreError::Sqlite(rusqlite::Error::QueryReturnedNoRows);
        assert!(err.to_string().to_lowercase().contains("sqlite"));
        assert!(
            err.source().is_some(),
            "Sqlite variant must chain the rusqlite error"
        );
    }

    #[test]
    fn store_error_from_rusqlite_propagates() {
        fn propagate() -> Result<(), StoreError> {
            Err(rusqlite::Error::QueryReturnedNoRows)?; // From<rusqlite::Error> via `?`
            Ok(())
        }
        let err = propagate().expect_err("must propagate as Err");
        assert!(matches!(err, StoreError::Sqlite(_)));
    }

    #[test]
    fn migration_error_names_the_ahead_version() {
        let err = StoreError::Migration { db_version: 9 };
        let text = err.to_string();
        assert!(text.contains('9'), "display must name the database version");
        assert!(
            text.to_lowercase().contains("schema"),
            "display must name the schema"
        );
        assert!(err.source().is_none(), "Migration has no underlying error");
    }

    #[test]
    fn fts5_error_category_is_identifiable() {
        let err = StoreError::Fts5Unavailable {
            detail: "SQLite build lacks ENABLE_FTS5".to_string(),
        };
        assert!(
            err.to_string().contains("FTS5"),
            "error text must name FTS5"
        );
        assert!(
            err.source().is_none(),
            "Fts5Unavailable has no underlying error"
        );
    }

    #[test]
    fn verify_fts5_passes_on_bundled_build() {
        // Task 4.1 (red at authoring time): verifies both halves of the
        // FTS5 check — the ENABLE_FTS5 compile option and the temp-schema
        // virtual-table smoke — against the bundled build.
        let conn = Connection::open_in_memory().expect("in-memory db");
        verify_fts5(&conn).expect("bundled build must pass FTS5 verification");
    }
}
