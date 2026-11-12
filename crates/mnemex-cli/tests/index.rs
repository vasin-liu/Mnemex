//! `mnemex index` against a real temp directory and a real SQLite file.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use mnemex_core::event::{EventKind, EventSource};
use mnemex_core::ids::{ChunkId, ContentHash, KoId, SourceRefs};
use mnemex_core::ko::{CodeSymbol, KoCore, SymbolKind};
use mnemex_core::lifecycle::{Lifecycle, UnixMillis};
use mnemex_ingest::scan_text;
use mnemex_store::schema::StoredKo;
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
    let section = scanned.sections.first().expect("section");
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

fn index_ok(dir: &Path, db: &Path) {
    let status = run(&[
        arg("index"),
        arg("--dir"),
        dir.to_str().expect("utf-8").to_string(),
        arg("--db"),
        db.to_str().expect("utf-8").to_string(),
    ]);
    assert!(status.success(), "index failed");
}

fn assert_no_vector_dir(db: &Path) {
    let parent = db.parent().expect("db parent");
    for entry in std::fs::read_dir(parent).expect("list store dir") {
        let entry = entry.expect("entry");
        assert!(
            !entry.file_type().expect("type").is_dir(),
            "index must not create a vector dataset directory at {}",
            entry.path().display()
        );
    }
}

fn counts(store: &SqliteStore) -> (i64, i64) {
    let objects: i64 = store
        .query_scalar("SELECT COUNT(*) FROM knowledge_objects")
        .expect("objects");
    let chunks: i64 = store
        .query_scalar("SELECT COUNT(*) FROM chunks")
        .expect("chunks");
    (objects, chunks)
}

#[test]
fn deleted_file_is_forgotten() {
    let dir = TempDir::new("delete");
    let docs = dir.path().join("docs");
    std::fs::create_dir_all(&docs).expect("docs");
    std::fs::write(docs.join("note.md"), "alpha word\n").expect("note");
    let store_dir = dir.path().join("store");
    std::fs::create_dir_all(&store_dir).expect("store");
    let db = store_dir.join("mnemex.db");
    index_ok(&docs, &db);
    let old_id = scan_text(&docs).expect("scan").sections[0].id;
    std::fs::remove_file(docs.join("note.md")).expect("remove");
    index_ok(&docs, &db);
    let store = SqliteStore::open(&db).expect("open");
    assert!(store.search_chunks("alpha").expect("search").is_empty());
    assert!(store.read_ko(&old_id).expect("read").is_none());
    assert_no_vector_dir(&db);
}

#[test]
fn retitled_section_replaces_the_object() {
    let dir = TempDir::new("title");
    let docs = dir.path().join("docs");
    std::fs::create_dir_all(&docs).expect("docs");
    std::fs::write(docs.join("note.md"), "# Old\nalpha word\n").expect("note");
    let db = dir.path().join("mnemex.db");
    index_ok(&docs, &db);
    let old_id = scan_text(&docs).expect("scan").sections[0].id;
    std::fs::write(docs.join("note.md"), "# New\nbravo word\n").expect("rewrite");
    let new_id = scan_text(&docs).expect("scan").sections[0].id;
    index_ok(&docs, &db);
    let store = SqliteStore::open(&db).expect("open");
    assert!(store.read_ko(&old_id).expect("old").is_none());
    assert!(store.read_ko(&new_id).expect("new").is_some());
    assert!(store.search_chunks("alpha").expect("old word").is_empty());
    assert!(!store.search_chunks("bravo").expect("new word").is_empty());
}

#[test]
fn rewritten_body_updates_the_hash_and_keeps_one_created_event() {
    let dir = TempDir::new("body");
    let docs = dir.path().join("docs");
    std::fs::create_dir_all(&docs).expect("docs");
    std::fs::write(docs.join("note.md"), "# Title\nalpha word\n").expect("note");
    let db = dir.path().join("mnemex.db");
    index_ok(&docs, &db);
    let store = SqliteStore::open(&db).expect("open");
    let before = counts(&store);
    drop(store);
    index_ok(&docs, &db);
    let store = SqliteStore::open(&db).expect("reopen");
    assert_eq!(counts(&store), before);
    drop(store);

    std::fs::write(docs.join("note.md"), "# Title\nbravo word\n").expect("rewrite");
    let section = scan_text(&docs)
        .expect("scan")
        .sections
        .into_iter()
        .next()
        .expect("section");
    index_ok(&docs, &db);
    let store = SqliteStore::open(&db).expect("open");
    assert!(store.search_chunks("alpha").expect("old word").is_empty());
    let hits = store.search_chunks("bravo").expect("new word");
    assert_eq!(hits.len(), 1);
    assert_eq!(store.chunk_pending(&hits[0]).expect("pending"), Some(true));
    let StoredKo::DocumentSection(stored) =
        store.read_ko(&section.id).expect("read").expect("object")
    else {
        panic!("expected a document section");
    };
    assert_eq!(
        stored.core.refs.content_hash.as_ref(),
        Some(&section.content_hash)
    );
    let events = store.events_for(&section.id).expect("events");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].1.kind(), EventKind::Created);
    assert_eq!(events[0].1.source(), EventSource::Rule);
}

#[test]
fn undecodable_file_keeps_the_indexed_section() {
    let dir = TempDir::new("utf8");
    let docs = dir.path().join("docs");
    std::fs::create_dir_all(&docs).expect("docs");
    std::fs::write(docs.join("note.txt"), "alpha word\n").expect("note");
    let db = dir.path().join("mnemex.db");
    index_ok(&docs, &db);
    let old_id = scan_text(&docs).expect("scan").sections[0].id;
    std::fs::write(docs.join("note.txt"), [0xff, 0xfe, 0xfd]).expect("corrupt");
    index_ok(&docs, &db);
    let store = SqliteStore::open(&db).expect("open");
    assert!(store.read_ko(&old_id).expect("read").is_some());
    assert!(!store.search_chunks("alpha").expect("search").is_empty());
}

#[test]
fn emptied_file_drops_the_section() {
    let dir = TempDir::new("empty");
    let docs = dir.path().join("docs");
    std::fs::create_dir_all(&docs).expect("docs");
    std::fs::write(docs.join("note.txt"), "alpha word\n").expect("note");
    let db = dir.path().join("mnemex.db");
    index_ok(&docs, &db);
    let old_id = scan_text(&docs).expect("scan").sections[0].id;
    std::fs::write(docs.join("note.txt"), "").expect("empty");
    index_ok(&docs, &db);
    let store = SqliteStore::open(&db).expect("open");
    assert!(store.read_ko(&old_id).expect("read").is_none());
    assert!(store.search_chunks("alpha").expect("search").is_empty());
}

#[test]
fn missing_directory_leaves_the_database() {
    let dir = TempDir::new("missing");
    let docs = dir.path().join("docs");
    std::fs::create_dir_all(&docs).expect("docs");
    std::fs::write(docs.join("note.txt"), "alpha word\n").expect("note");
    let db = dir.path().join("mnemex.db");
    index_ok(&docs, &db);
    let store = SqliteStore::open(&db).expect("open");
    let before = counts(&store);
    drop(store);
    let status = run(&[
        arg("index"),
        arg("--dir"),
        docs.join("gone").to_str().expect("utf-8").to_string(),
        arg("--db"),
        db.to_str().expect("utf-8").to_string(),
    ]);
    assert!(!status.success());
    let store = SqliteStore::open(&db).expect("reopen");
    assert_eq!(counts(&store), before);
    assert!(!store.search_chunks("alpha").expect("search").is_empty());
}

#[test]
fn other_kind_survives_when_the_text_file_is_removed() {
    let dir = TempDir::new("kind");
    let docs = dir.path().join("docs");
    std::fs::create_dir_all(&docs).expect("docs");
    std::fs::write(docs.join("note.md"), "alpha word\n").expect("note");
    let db = dir.path().join("mnemex.db");
    index_ok(&docs, &db);
    let symbol_id = KoId::new([9; 16]);
    let symbol_chunk = ChunkId::new([8; 16]);
    let store = SqliteStore::open(&db).expect("open");
    store
        .insert_ko(&StoredKo::CodeSymbol(CodeSymbol {
            core: KoCore {
                id: symbol_id,
                refs: SourceRefs {
                    content_hash: Some(ContentHash::new(vec![9; 32]).expect("hash")),
                    commit: None,
                    path: Some(PathBuf::from("src/lib.rs")),
                },
                lifecycle: Lifecycle::default(),
                created_at: UnixMillis::new(1_700_000_000_000),
            },
            name: "open".to_string(),
            symbol_kind: SymbolKind::Function,
            container: None,
        }))
        .expect("symbol");
    store
        .insert_chunk(
            &symbol_chunk,
            &symbol_id,
            &ContentHash::new(vec![8; 32]).expect("hash"),
            "symbolkw",
        )
        .expect("symbol chunk");
    drop(store);
    std::fs::remove_file(docs.join("note.md")).expect("remove");
    index_ok(&docs, &db);
    let store = SqliteStore::open(&db).expect("reopen");
    assert!(store.read_ko(&symbol_id).expect("symbol").is_some());
    assert_eq!(
        store.chunk_body(&symbol_chunk).expect("chunk").as_deref(),
        Some("symbolkw")
    );
    assert!(store.search_chunks("alpha").expect("text").is_empty());
}

#[test]
fn indexing_another_directory_drops_the_previous_documents() {
    let dir = TempDir::new("swap");
    let dir_a = dir.path().join("a");
    let dir_b = dir.path().join("b");
    std::fs::create_dir_all(&dir_a).expect("a");
    std::fs::create_dir_all(&dir_b).expect("b");
    std::fs::write(dir_a.join("a.md"), "alpha word\n").expect("a");
    std::fs::write(dir_b.join("b.md"), "bravo word\n").expect("b");
    let db = dir.path().join("mnemex.db");
    index_ok(&dir_a, &db);
    index_ok(&dir_b, &db);
    let store = SqliteStore::open(&db).expect("open");
    assert!(store.search_chunks("alpha").expect("a").is_empty());
    assert!(!store.search_chunks("bravo").expect("b").is_empty());
}
