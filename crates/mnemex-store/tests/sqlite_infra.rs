//! Integration tests for the SQLite infrastructure (change add-sqlite-infra).
//!
//! Real SQLite on real disk in a real temp directory — the file system is
//! never mocked (AGENTS integration-layer rule).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use mnemex_store::sqlite::SqliteStore;

/// Minimal hand-rolled temp dir helper (design D4): `tempfile` is not
/// admitted in this change.
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "mnemex-test-{tag}-{}-{nanos}-{seq}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("create temp dir");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        // Connections are dropped before the temp dir guard, so no open
        // handles should remain (Windows).
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn new_path_creates_usable_db() {
    let dir = TempDir::new("new");
    let db = dir.path().join("mnemex.db");
    assert!(!db.exists(), "precondition: db file must not exist yet");
    let store = SqliteStore::open(&db).expect("open on a fresh path");
    store
        .execute("CREATE TABLE probe (id INTEGER PRIMARY KEY, note TEXT NOT NULL)")
        .expect("ddl must run on the new db");
    store
        .execute("INSERT INTO probe (note) VALUES ('hello')")
        .expect("dml must run on the new db");
    let count: i64 = store
        .query_scalar("SELECT COUNT(*) FROM probe")
        .expect("scalar query");
    assert_eq!(count, 1);
}

#[test]
fn reopen_is_idempotent_and_preserves_data() {
    let dir = TempDir::new("reopen");
    let db = dir.path().join("mnemex.db");
    {
        let store = SqliteStore::open(&db).expect("first open");
        store
            .execute("CREATE TABLE probe (note TEXT NOT NULL)")
            .expect("ddl");
        store
            .execute("INSERT INTO probe (note) VALUES ('kept')")
            .expect("dml");
    } // dropping the store closes the connection
    let store = SqliteStore::open(&db).expect("reopen of an existing db must succeed");
    let note: String = store
        .query_scalar("SELECT note FROM probe")
        .expect("data must survive reopen");
    assert_eq!(note, "kept");
}

#[test]
fn invalid_path_fails_with_store_error() {
    // Parent directory does not exist: SQLite cannot create the file.
    let bad = std::env::temp_dir()
        .join("mnemex-missing-parent-for-tests")
        .join("child")
        .join("mnemex.db");
    let result = SqliteStore::open(&bad);
    let err = result.expect_err("unusable path must fail");
    // Error object, not a panic; the failure reason is identifiable.
    assert!(!err.to_string().is_empty());
}

#[test]
fn pragmas_are_in_effect() {
    // Spec: WAL 生效 / busy_timeout 生效.
    let dir = TempDir::new("pragmas");
    let store = SqliteStore::open(&dir.path().join("mnemex.db")).expect("open");
    let mode = store.journal_mode().expect("journal_mode query");
    assert_eq!(mode.to_ascii_lowercase(), "wal");
    let timeout_ms = store.busy_timeout_ms().expect("busy_timeout query");
    assert_eq!(timeout_ms, 5_000);
}

#[test]
fn fts5_is_usable_end_to_end() {
    // Spec: bundled 构建冒烟通过 — open already refuses to start without
    // FTS5, so reaching here proves the check passed; then exercise FTS5
    // itself through the public API.
    let dir = TempDir::new("fts5");
    let store = SqliteStore::open(&dir.path().join("mnemex.db"))
        .expect("open succeeds only when FTS5 verification passed");
    let enabled: i64 = store
        .query_scalar(
            "SELECT COUNT(*) FROM pragma_compile_options \
             WHERE compile_options = 'ENABLE_FTS5'",
        )
        .expect("pragma table function query");
    assert!(enabled > 0, "bundled SQLite must compile with ENABLE_FTS5");
    store
        .execute("CREATE VIRTUAL TABLE notes USING fts5(title)")
        .expect("fts5 virtual table");
    store
        .execute("INSERT INTO notes(title) VALUES ('lancedb consistency protocol')")
        .expect("insert");
    let hits: i64 = store
        .query_scalar("SELECT COUNT(*) FROM notes WHERE notes MATCH 'lancedb'")
        .expect("match query");
    assert_eq!(hits, 1);
}
