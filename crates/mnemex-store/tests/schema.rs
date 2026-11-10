//! Integration tests for the SQLite schema (change add-store-schema).
//!
//! Real SQLite on a real temp directory — the file system is never mocked.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use mnemex_core::event::{EventSource, KnowledgeEvent};
use mnemex_core::ids::{ChunkId, CommitSha, ContentHash, KoId, SourceRefs};
use mnemex_core::ko::{
    BugRecord, CodeSymbol, CommitEvent, DesignDecision, DocumentSection, KoCore, SymbolKind,
};
use mnemex_core::lifecycle::{
    Confidence, ContradictionScore, Freshness, Lifecycle, ModelConfidence, UnixMillis,
};
use mnemex_store::schema::StoredKo;
use mnemex_store::sqlite::{SqliteStore, StoreError};

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
            "mnemex-schema-{tag}-{}-{nanos}-{seq}",
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
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn open() -> (TempDir, SqliteStore) {
    let dir = TempDir::new("db");
    let store = SqliteStore::open(&dir.path().join("mnemex.db")).expect("open");
    (dir, store)
}

fn hash(bytes: &[u8]) -> ContentHash {
    ContentHash::new(bytes.to_vec()).expect("non-empty digest")
}

fn sha(bytes: &[u8]) -> CommitSha {
    CommitSha::new(bytes.to_vec()).expect("non-empty sha")
}

fn model(v: f32) -> ModelConfidence {
    ModelConfidence::new(v).expect("in-range model score")
}

fn calibrated(v: f32) -> Confidence {
    Confidence::new(v).expect("in-range calibrated score")
}

fn contradiction(v: f32) -> ContradictionScore {
    ContradictionScore::new(v).expect("in-range contradiction")
}

fn core(id_byte: u8, lifecycle: Lifecycle, refs: SourceRefs) -> KoCore {
    KoCore {
        id: KoId::new([id_byte; 16]),
        refs,
        lifecycle,
        created_at: UnixMillis::new(1_700_000_000_000 + i64::from(id_byte)),
    }
}

#[test]
fn fresh_database_applies_v1_and_reopen_keeps_rows() {
    let dir = TempDir::new("migrate");
    let db = dir.path().join("mnemex.db");
    let version = {
        let store = SqliteStore::open(&db).expect("fresh open");
        let version: i64 = store
            .query_scalar("PRAGMA user_version")
            .expect("user_version");
        assert_eq!(version, i64::from(mnemex_store::schema::SCHEMA_VERSION));
        assert!(version > 0, "v1 migration must have run");
        for name in [
            "knowledge_objects",
            "knowledge_events",
            "chunks",
            "knowledge_edge",
        ] {
            let present: i64 = store
                .query_scalar(&format!(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = '{name}'"
                ))
                .expect("sqlite_master");
            assert_eq!(present, 1, "{name} must exist");
        }
        store
            .execute(
                "INSERT INTO knowledge_objects (ko_id, kind, created_at, usage_count, freshness) \
                 VALUES (X'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', 1, 42, 0, 0)",
            )
            .expect("seed row");
        version
    };
    let store = SqliteStore::open(&db).expect("reopen");
    let again: i64 = store
        .query_scalar("PRAGMA user_version")
        .expect("user_version");
    assert_eq!(again, version, "reopen must not bump the schema version");
    let created: i64 = store
        .query_scalar("SELECT created_at FROM knowledge_objects")
        .expect("seed row survives reopen");
    assert_eq!(created, 42);
}

#[test]
fn ahead_user_version_refuses_open() {
    let dir = TempDir::new("ahead");
    let db = dir.path().join("mnemex.db");
    {
        let store = SqliteStore::open(&db).expect("open");
        store
            .execute("PRAGMA user_version = 99")
            .expect("stamp an unknown future version");
    }
    let err = SqliteStore::open(&db).expect_err("ahead version must refuse open");
    match err {
        StoreError::Migration { db_version } => assert_eq!(db_version, 99),
        other => panic!("expected Migration, got {other}"),
    }
}

#[test]
fn five_kinds_roundtrip_including_absent_fields() {
    let (_dir, store) = open();

    let symbol = StoredKo::CodeSymbol(CodeSymbol {
        core: core(
            1,
            Lifecycle {
                model_confidence: Some(model(0.25)),
                confidence: None,
                usage_count: 0,
                last_used: None,
                freshness: Freshness::Fresh,
                contradiction_score: None,
            },
            SourceRefs {
                content_hash: Some(hash(&[1, 2, 3])),
                commit: Some(sha(&[9; 20])),
                path: Some(PathBuf::from("src/lib.rs")),
            },
        ),
        name: "open".to_string(),
        symbol_kind: SymbolKind::Function,
        container: Some("mnemex_store::sqlite".to_string()),
    });
    let decision = StoredKo::DesignDecision(DesignDecision {
        core: core(
            2,
            Lifecycle {
                model_confidence: None,
                confidence: None,
                usage_count: 2,
                last_used: Some(UnixMillis::new(1_700_000_111_000)),
                freshness: Freshness::Stale,
                contradiction_score: Some(contradiction(1.0)),
            },
            SourceRefs {
                content_hash: Some(hash(&[4, 5])),
                commit: None,
                path: None,
            },
        ),
        problem: "where does the schema live".to_string(),
        diagnosis: "one sparse table".to_string(),
        resolution: "single knowledge_objects table".to_string(),
    });
    let bug = StoredKo::BugRecord(BugRecord {
        core: core(
            3,
            Lifecycle::default(),
            SourceRefs {
                content_hash: None,
                commit: Some(sha(&[7, 7, 7])),
                path: None,
            },
        ),
        symptom: "read returned a zero score".to_string(),
        root_cause: "NULL was coerced".to_string(),
        fix: "keep SQL NULL as None".to_string(),
    });
    let section = StoredKo::DocumentSection(DocumentSection {
        core: core(
            4,
            Lifecycle {
                model_confidence: Some(model(1.0)),
                confidence: Some(calibrated(0.0)),
                usage_count: 0,
                last_used: None,
                freshness: Freshness::Fresh,
                contradiction_score: None,
            },
            SourceRefs {
                content_hash: None,
                commit: None,
                path: Some(PathBuf::from("docs/adr/ADR-002.md")),
            },
        ),
        title: "Source of truth".to_string(),
        heading_path: "adr/002/source-of-truth".to_string(),
    });
    let commit = StoredKo::CommitEvent(CommitEvent {
        core: core(5, Lifecycle::default(), SourceRefs::default()),
        sha: sha(&[0xab; 32]),
        message: "feat: land the schema".to_string(),
        authored_at: UnixMillis::new(1_700_000_222_000),
    });

    for ko in [&symbol, &decision, &bug, &section, &commit] {
        store.insert_ko(ko).expect("insert");
        let id = match ko {
            StoredKo::CodeSymbol(v) => v.core.id,
            StoredKo::DesignDecision(v) => v.core.id,
            StoredKo::BugRecord(v) => v.core.id,
            StoredKo::DocumentSection(v) => v.core.id,
            StoredKo::CommitEvent(v) => v.core.id,
        };
        let loaded = store.read_ko(&id).expect("read").expect("row present");
        assert_eq!(&loaded, ko);
    }

    let loaded_bug = store
        .read_ko(&KoId::new([3; 16]))
        .expect("read bug")
        .expect("bug present");
    let StoredKo::BugRecord(bug_row) = loaded_bug else {
        panic!("kind must stay BugRecord");
    };
    assert!(bug_row.core.lifecycle.model_confidence.is_none());
    assert!(bug_row.core.lifecycle.confidence.is_none());
    assert!(bug_row.core.lifecycle.contradiction_score.is_none());
    assert!(bug_row.core.lifecycle.last_used.is_none());
    assert!(bug_row.core.refs.content_hash.is_none());
    assert!(bug_row.core.refs.path.is_none());
    assert_eq!(bug_row.core.lifecycle.usage_count, 0);
}

#[test]
fn unknown_kind_integer_is_an_error() {
    let (_dir, store) = open();
    store
        .execute(
            "INSERT INTO knowledge_objects (ko_id, kind, created_at, usage_count, freshness) \
             VALUES (X'0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b', 99, 1, 0, 0)",
        )
        .expect("raw row");
    let err = store
        .read_ko(&KoId::new([0x0b; 16]))
        .expect_err("unknown kind must not decode");
    assert!(matches!(err, StoreError::Sqlite(_)));
}

#[test]
fn events_append_only_and_usage_replays() {
    let (_dir, store) = open();
    let id = KoId::new([6; 16]);
    store
        .insert_ko(&StoredKo::CodeSymbol(CodeSymbol {
            core: core(6, Lifecycle::default(), SourceRefs::default()),
            name: "used".to_string(),
            symbol_kind: SymbolKind::Module,
            container: None,
        }))
        .expect("ko");

    let created = KnowledgeEvent::created(id, UnixMillis::new(1_000), EventSource::Rule);
    let used_a = KnowledgeEvent::used(id, UnixMillis::new(2_000), EventSource::Model);
    let used_b = KnowledgeEvent::used(id, UnixMillis::new(3_000), EventSource::User);
    let used_c = KnowledgeEvent::used(id, UnixMillis::new(4_000), EventSource::Rule);
    let vote = KnowledgeEvent::feedback(id, UnixMillis::new(5_000), EventSource::User, false);

    let mut seqs = Vec::new();
    for event in [&created, &used_a, &used_b, &used_c, &vote] {
        seqs.push(store.append_event(event).expect("append"));
    }
    assert!(seqs.windows(2).all(|pair| pair[0] < pair[1]), "seq grows");

    let listed = store.events_for(&id).expect("list");
    assert_eq!(listed.len(), 5);
    assert_eq!(listed.iter().map(|(seq, _)| *seq).collect::<Vec<_>>(), seqs);
    assert_eq!(listed[0].1, created);
    assert_eq!(listed[1].1, used_a);
    assert_eq!(listed[4].1, vote);
    assert_eq!(store.events_for(&id).expect("second read"), listed);

    let missing = KoId::new([0xee; 16]);
    let orphan = store.append_event(&KnowledgeEvent::used(
        missing,
        UnixMillis::new(9),
        EventSource::User,
    ));
    assert!(orphan.is_err(), "fk must reject an event for a missing ko");

    let (count, last) = store.sync_usage_from_events(&id).expect("replay");
    assert_eq!(count, 3, "only Used events count");
    assert_eq!(last, Some(UnixMillis::new(4_000)));
    let loaded = store.read_ko(&id).expect("read").expect("present");
    let StoredKo::CodeSymbol(symbol) = loaded else {
        panic!("kind preserved");
    };
    assert_eq!(symbol.core.lifecycle.usage_count, 3);
    assert_eq!(
        symbol.core.lifecycle.last_used,
        Some(UnixMillis::new(4_000))
    );
}

#[test]
fn chunks_default_pending_and_edges_require_endpoints() {
    let (_dir, store) = open();
    let src = KoId::new([7; 16]);
    let dst = KoId::new([8; 16]);
    for (byte, name) in [(7u8, "src"), (8u8, "dst")] {
        store
            .insert_ko(&StoredKo::CodeSymbol(CodeSymbol {
                core: core(byte, Lifecycle::default(), SourceRefs::default()),
                name: name.to_string(),
                symbol_kind: SymbolKind::Class,
                container: None,
            }))
            .expect("endpoint");
    }

    let chunk_id = ChunkId::new([0x0c; 16]);
    let digest = hash(&[9, 9, 9]);
    store
        .insert_chunk(&chunk_id, &src, &digest, "")
        .expect("insert chunk");
    assert_eq!(
        store.chunk_pending(&chunk_id).expect("read pending"),
        Some(true)
    );
    store
        .set_chunk_pending(&chunk_id, false)
        .expect("flip pending");
    assert_eq!(
        store.chunk_pending(&chunk_id).expect("read flipped"),
        Some(false)
    );

    let missing = KoId::new([0xdd; 16]);
    assert!(
        store
            .insert_chunk(&ChunkId::new([0x11; 16]), &missing, &digest, "")
            .is_err()
    );
    assert!(store.insert_edge(&missing, &dst, 1).is_err());
    assert!(store.insert_edge(&src, &missing, 2).is_err());

    store
        .insert_edge(&src, &dst, 7)
        .expect("edge between real kos");
    let edges: i64 = store
        .query_scalar("SELECT COUNT(*) FROM knowledge_edge")
        .expect("edge count");
    assert_eq!(edges, 1);
}

fn insert_symbol(store: &SqliteStore, byte: u8) -> KoId {
    let id = KoId::new([byte; 16]);
    store
        .insert_ko(&StoredKo::CodeSymbol(CodeSymbol {
            core: core(byte, Lifecycle::default(), SourceRefs::default()),
            name: "section".to_string(),
            symbol_kind: SymbolKind::Class,
            container: None,
        }))
        .expect("ko");
    id
}

#[test]
fn chunk_body_roundtrips_and_empty_body_stays_pending() {
    let (_dir, store) = open();
    let ko = insert_symbol(&store, 0x31);
    let filled = ChunkId::new([0x32; 16]);
    store
        .insert_chunk(&filled, &ko, &hash(b"filled"), "alpha beta gamma")
        .expect("insert body");
    assert_eq!(
        store.chunk_body(&filled).expect("read body"),
        Some("alpha beta gamma".to_string())
    );

    let empty = ChunkId::new([0x33; 16]);
    store
        .insert_chunk(&empty, &ko, &hash(b"empty"), "")
        .expect("insert empty");
    assert_eq!(
        store.chunk_body(&empty).expect("read empty"),
        Some(String::new())
    );
    assert_eq!(store.chunk_pending(&empty).expect("pending"), Some(true));
}

#[test]
fn fts_hits_a_word_in_the_body_and_misses_an_absent_word() {
    let (_dir, store) = open();
    let ko = insert_symbol(&store, 0x34);
    let id = ChunkId::new([0x35; 16]);
    store
        .insert_chunk(&id, &ko, &hash(b"fts"), "alpha beta gamma")
        .expect("insert");
    let hits = store.search_chunks("gamma").expect("hit");
    assert_eq!(hits, vec![id]);
    let misses = store.search_chunks("zzzabsent").expect("miss");
    assert!(misses.is_empty());
}
