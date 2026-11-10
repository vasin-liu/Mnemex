//! `mnemex index` against a real temp directory and a real SQLite file.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use mnemex_core::event::{EventKind, EventSource};
use mnemex_ingest::scan_text;
use mnemex_store::sqlite::SqliteStore;

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "mnemex-index-{tag}-{}-{nanos}-{seq}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("temp dir");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn run(args: &[String]) -> std::process::ExitStatus {
    Command::new(env!("CARGO_BIN_EXE_mnemex"))
        .args(args)
        .status()
        .expect("spawn mnemex")
}

fn arg(value: &str) -> String {
    value.to_string()
}

#[test]
fn missing_dir_fails_without_creating_a_database() {
    let dir = TempDir::new("missing");
    let db = dir.path().join("mnemex.db");
    let db_arg = db.to_str().expect("utf-8 path").to_string();
    let status = run(&[arg("index"), arg("--db"), db_arg]);
    assert!(!status.success());
    assert!(!db.exists(), "a missing --dir must not create the database");
}

#[test]
fn index_writes_searchable_pending_chunks_once() {
    let dir = TempDir::new("index");
    let docs = dir.path().join("docs");
    std::fs::create_dir_all(&docs).expect("docs");
    std::fs::write(docs.join("note.md"), "# Decision\ngamma rule\n").expect("note");
    let db_dir = dir.path().join("store");
    std::fs::create_dir_all(&db_dir).expect("store dir");
    let db = db_dir.join("mnemex.db");

    let docs_arg = docs.to_str().expect("utf-8").to_string();
    let db_arg = db.to_str().expect("utf-8").to_string();
    let status = run(&[
        arg("index"),
        arg("--dir"),
        docs_arg.clone(),
        arg("--db"),
        db_arg.clone(),
    ]);
    assert!(status.success(), "index failed");

    let scanned = scan_text(&docs).expect("scan");
    let section = scanned.first().expect("section");
    let chunk = section.chunks.first().expect("chunk");
    let store = SqliteStore::open(&db).expect("open");
    let hits = store.search_chunks("gamma").expect("search");
    assert_eq!(hits, vec![chunk.id]);
    assert_eq!(store.chunk_pending(&chunk.id).expect("pending"), Some(true));
    let events = store.events_for(&section.id).expect("events");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].1.kind(), EventKind::Created);
    assert_eq!(events[0].1.source(), EventSource::Rule);

    for entry in std::fs::read_dir(&db_dir).expect("list store dir") {
        let entry = entry.expect("entry");
        assert!(
            !entry.file_type().expect("type").is_dir(),
            "index must not create a vector dataset directory at {}",
            entry.path().display()
        );
    }

    let objects: i64 = store
        .query_scalar("SELECT COUNT(*) FROM knowledge_objects")
        .expect("objects");
    let chunks: i64 = store
        .query_scalar("SELECT COUNT(*) FROM chunks")
        .expect("chunks");
    drop(store);

    let again = run(&[arg("index"), arg("--dir"), docs_arg, arg("--db"), db_arg]);
    assert!(again.success(), "second index failed");
    let store = SqliteStore::open(&db).expect("reopen");
    let objects_again: i64 = store
        .query_scalar("SELECT COUNT(*) FROM knowledge_objects")
        .expect("objects");
    let chunks_again: i64 = store
        .query_scalar("SELECT COUNT(*) FROM chunks")
        .expect("chunks");
    assert_eq!(objects_again, objects);
    assert_eq!(chunks_again, chunks);
    assert_eq!(store.events_for(&section.id).expect("events").len(), 1);
}
