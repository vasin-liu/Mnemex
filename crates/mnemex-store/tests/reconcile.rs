//! Document-section reconcile against a real SQLite file.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use mnemex_core::event::{EventSource, KnowledgeEvent};
use mnemex_core::ids::{ChunkId, ContentHash, KoId, SourceRefs};
use mnemex_core::ko::{CodeSymbol, DocumentSection, KoCore, SymbolKind};
use mnemex_core::lifecycle::{Freshness, Lifecycle, UnixMillis};
use mnemex_store::schema::{DocumentSync, StoredKo};
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
            "mnemex-reconcile-{tag}-{}-{nanos}-{seq}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("temp dir");
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn hash(byte: u8) -> ContentHash {
    ContentHash::new(vec![byte; 32]).expect("hash")
}

fn document(id: KoId, path: &str, hash_byte: u8) -> StoredKo {
    StoredKo::DocumentSection(DocumentSection {
        core: KoCore {
            id,
            refs: SourceRefs {
                content_hash: Some(hash(hash_byte)),
                commit: None,
                path: Some(PathBuf::from(path)),
            },
            lifecycle: Lifecycle {
                freshness: Freshness::Fresh,
                ..Lifecycle::default()
            },
            created_at: UnixMillis::new(1_700_000_000_000),
        },
        title: "Title".to_string(),
        heading_path: "Title".to_string(),
    })
}

fn symbol(id: KoId) -> StoredKo {
    StoredKo::CodeSymbol(CodeSymbol {
        core: KoCore {
            id,
            refs: SourceRefs {
                content_hash: Some(hash(9)),
                commit: None,
                path: Some(PathBuf::from("src/lib.rs")),
            },
            lifecycle: Lifecycle::default(),
            created_at: UnixMillis::new(1_700_000_000_000),
        },
        name: "open".to_string(),
        symbol_kind: SymbolKind::Function,
        container: None,
    })
}

fn open() -> (TempDir, SqliteStore) {
    let dir = TempDir::new("db");
    let store = SqliteStore::open(&dir.0.join("mnemex.db")).expect("open");
    (dir, store)
}

#[test]
fn deleting_a_chunk_drops_it_from_full_text_search() {
    let (_dir, store) = open();
    let doc_id = KoId::new([1; 16]);
    let doc = document(doc_id, "doc.md", 2);
    let keep = ChunkId::new([3; 16]);
    let drop = ChunkId::new([4; 16]);
    store.insert_ko(&doc).expect("object");
    store
        .insert_chunk(&keep, &doc_id, &hash(3), "alpha stays")
        .expect("keep");
    store
        .insert_chunk(&drop, &doc_id, &hash(4), "beta leaves")
        .expect("drop");

    store
        .sync_documents(|_| DocumentSync {
            delete_chunks: vec![drop],
            ..DocumentSync::default()
        })
        .expect("sync");

    assert!(store.search_chunks("beta").expect("search").is_empty());
    assert_eq!(store.search_chunks("alpha").expect("search"), vec![keep]);
    let version: i64 = store.query_scalar("PRAGMA user_version").expect("version");
    assert_eq!(version, i64::from(mnemex_store::schema::SCHEMA_VERSION));
}

#[test]
fn removing_a_document_section_drops_its_rows_and_keeps_other_kinds() {
    let (_dir, store) = open();
    let doc_id = KoId::new([1; 16]);
    let other_id = KoId::new([9; 16]);
    let doc = document(doc_id, "doc.md", 2);
    let other = symbol(other_id);
    let doc_chunk = ChunkId::new([3; 16]);
    let other_chunk = ChunkId::new([8; 16]);
    let at = UnixMillis::new(1_700_000_000_000);
    store.insert_ko(&doc).expect("doc");
    store.insert_ko(&other).expect("symbol");
    store
        .insert_chunk(&doc_chunk, &doc_id, &hash(3), "doc body")
        .expect("doc chunk");
    store
        .insert_chunk(&other_chunk, &other_id, &hash(8), "sym body")
        .expect("symbol chunk");
    store
        .append_event(&KnowledgeEvent::created(doc_id, at, EventSource::Rule))
        .expect("doc event");
    store
        .append_event(&KnowledgeEvent::created(other_id, at, EventSource::Rule))
        .expect("symbol event");
    store.insert_edge(&doc_id, &other_id, 1).expect("edge");

    store
        .sync_documents(|_| DocumentSync {
            delete_objects: vec![doc_id],
            ..DocumentSync::default()
        })
        .expect("sync");

    assert!(store.read_ko(&doc_id).expect("read").is_none());
    assert!(store.chunk_body(&doc_chunk).expect("chunk").is_none());
    assert!(store.events_for(&doc_id).expect("events").is_empty());
    let edges: i64 = store
        .query_scalar("SELECT COUNT(*) FROM knowledge_edge")
        .expect("edges");
    assert_eq!(edges, 0);
    assert!(store.read_ko(&other_id).expect("symbol").is_some());
    assert_eq!(
        store
            .chunk_body(&other_chunk)
            .expect("symbol chunk")
            .as_deref(),
        Some("sym body")
    );
    assert_eq!(store.events_for(&other_id).expect("symbol events").len(), 1);
}

#[test]
fn updating_a_section_hash_keeps_the_object() {
    let (_dir, store) = open();
    let doc_id = KoId::new([1; 16]);
    let doc = document(doc_id, "doc.md", 2);
    store.insert_ko(&doc).expect("object");
    let revised = hash(9);

    store
        .sync_documents(|_| DocumentSync {
            hash_updates: vec![(doc_id, revised.clone())],
            ..DocumentSync::default()
        })
        .expect("sync");

    let stored = store.read_ko(&doc_id).expect("read").expect("present");
    let StoredKo::DocumentSection(section) = stored else {
        panic!("expected a document section");
    };
    assert_eq!(section.core.refs.content_hash.as_ref(), Some(&revised));
}
