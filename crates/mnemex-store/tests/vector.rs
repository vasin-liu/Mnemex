//! Integration tests for the derived vector index (change add-lancedb-spike).
//!
//! Real LanceDB directory and real SQLite file. The file system is never mocked.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use mnemex_core::ids::{ChunkId, ContentHash, KoId, SourceRefs};
use mnemex_core::ko::{CodeSymbol, SymbolKind};
use mnemex_core::lifecycle::{Freshness, Lifecycle, UnixMillis};
use mnemex_store::schema::StoredKo;
use mnemex_store::sqlite::SqliteStore;
use mnemex_store::{AlignmentRow, LanceVectorStore, VectorStore, catch_up, rebuild_from_sqlite};

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
            "mnemex-vector-{tag}-{}-{nanos}-{seq}",
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

fn chunk(byte: u8) -> ChunkId {
    ChunkId::new([byte; 16])
}

fn digest(bytes: &[u8]) -> ContentHash {
    ContentHash::new(bytes.to_vec()).expect("non-empty digest")
}

fn open_index(dir: &Path) -> LanceVectorStore {
    LanceVectorStore::open(&dir.join("lancedb"), 4).expect("open vector index")
}

fn symbol(byte: u8) -> StoredKo {
    StoredKo::CodeSymbol(CodeSymbol {
        core: mnemex_core::ko::KoCore {
            id: KoId::new([byte; 16]),
            refs: SourceRefs::default(),
            lifecycle: Lifecycle {
                model_confidence: None,
                confidence: None,
                usage_count: 0,
                last_used: None,
                freshness: Freshness::Fresh,
                contradiction_score: None,
            },
            created_at: UnixMillis::new(1_700_000_000_000),
        },
        name: "open".to_string(),
        symbol_kind: SymbolKind::Function,
        container: None,
    })
}

#[test]
fn upsert_then_read_roundtrips_the_alignment_key() {
    let dir = TempDir::new("roundtrip");
    let index = open_index(dir.path());
    let id = chunk(1);
    let hash = digest(&[9, 9]);
    let vector = [1.0, 0.0, 0.0, 0.5];
    index.upsert(&id, &hash, &vector).expect("upsert");
    let read = index.read(&id, &hash).expect("read");
    assert_eq!(read.as_deref(), Some(vector.as_slice()));
}

#[test]
fn delete_one_key_leaves_the_other() {
    let dir = TempDir::new("delete");
    let index = open_index(dir.path());
    let a = chunk(1);
    let b = chunk(2);
    let hash_a = digest(&[1]);
    let hash_b = digest(&[2]);
    index
        .upsert(&a, &hash_a, &[1.0, 0.0, 0.0, 0.0])
        .expect("upsert a");
    index
        .upsert(&b, &hash_b, &[0.0, 1.0, 0.0, 0.0])
        .expect("upsert b");
    index.delete(&a, &hash_a).expect("delete a");
    assert!(index.read(&a, &hash_a).expect("read a").is_none());
    assert_eq!(
        index.read(&b, &hash_b).expect("read b").as_deref(),
        Some([0.0, 1.0, 0.0, 0.0].as_slice())
    );
}

#[test]
fn wrong_dimension_is_an_error_and_writes_nothing() {
    let dir = TempDir::new("dim");
    let index = open_index(dir.path());
    let id = chunk(3);
    let hash = digest(&[3]);
    let err = index
        .upsert(&id, &hash, &[1.0, 0.0, 0.0])
        .expect_err("length 3 must not match dimension 4");
    let msg = err.to_string();
    assert!(msg.contains("dimension"), "{msg}");
    assert!(index.read(&id, &hash).expect("read").is_none());
}

#[test]
fn catch_up_clears_pending_only_after_the_vector_is_readable() {
    let dir = TempDir::new("catch-ok");
    let sqlite = SqliteStore::open(&dir.path().join("mnemex.db")).expect("sqlite");
    let index = open_index(dir.path());
    let ko = symbol(4);
    let ko_id = KoId::new([4; 16]);
    sqlite.insert_ko(&ko).expect("ko");
    let id = chunk(4);
    let hash = digest(&[4, 4]);
    let vector = [0.25, 0.5, 0.75, 1.0];
    catch_up(&sqlite, &index, &ko_id, &id, &hash, &vector).expect("catch up");
    assert_eq!(sqlite.chunk_pending(&id).expect("pending"), Some(false));
    assert_eq!(
        index.read(&id, &hash).expect("read").as_deref(),
        Some(vector.as_slice())
    );
}

#[test]
fn catch_up_keeps_pending_when_the_vector_write_fails() {
    let dir = TempDir::new("catch-fail");
    let sqlite = SqliteStore::open(&dir.path().join("mnemex.db")).expect("sqlite");
    let index = open_index(dir.path());
    let ko = symbol(5);
    let ko_id = KoId::new([5; 16]);
    sqlite.insert_ko(&ko).expect("ko");
    let id = chunk(5);
    let hash = digest(&[5]);
    sqlite.insert_chunk(&id, &ko_id, &hash).expect("chunk");
    let err = catch_up(&sqlite, &index, &ko_id, &id, &hash, &[1.0, 0.0, 0.0])
        .expect_err("wrong dimension must fail the vector write");
    let msg = err.to_string();
    assert!(msg.contains("dimension"), "{msg}");
    assert_eq!(sqlite.chunk_pending(&id).expect("pending"), Some(true));
    assert!(index.read(&id, &hash).expect("read").is_none());
}

#[test]
fn rebuild_fills_an_empty_index_from_sqlite_rows() {
    let dir = TempDir::new("rebuild-fill");
    let sqlite = SqliteStore::open(&dir.path().join("mnemex.db")).expect("sqlite");
    let index = open_index(dir.path());
    let ko = symbol(6);
    let ko_id = KoId::new([6; 16]);
    sqlite.insert_ko(&ko).expect("ko");
    let id_a = chunk(6);
    let id_b = chunk(7);
    let hash_a = digest(&[6]);
    let hash_b = digest(&[7]);
    sqlite
        .insert_chunk(&id_a, &ko_id, &hash_a)
        .expect("chunk a");
    sqlite
        .insert_chunk(&id_b, &ko_id, &hash_b)
        .expect("chunk b");
    let rows = [
        AlignmentRow {
            chunk_id: id_a,
            content_hash: hash_a.clone(),
            vector: vec![1.0, 0.0, 0.0, 0.0],
        },
        AlignmentRow {
            chunk_id: id_b,
            content_hash: hash_b.clone(),
            vector: vec![0.0, 1.0, 0.0, 0.0],
        },
    ];
    rebuild_from_sqlite(&sqlite, &index, &rows).expect("rebuild");
    assert_eq!(
        index.read(&id_a, &hash_a).expect("read a").as_deref(),
        Some([1.0, 0.0, 0.0, 0.0].as_slice())
    );
    assert_eq!(
        index.read(&id_b, &hash_b).expect("read b").as_deref(),
        Some([0.0, 1.0, 0.0, 0.0].as_slice())
    );
    assert_eq!(sqlite.chunk_pending(&id_a).expect("pending a"), Some(false));
    assert_eq!(sqlite.chunk_pending(&id_b).expect("pending b"), Some(false));
}

#[test]
fn rebuild_drops_a_key_that_sqlite_does_not_have() {
    let dir = TempDir::new("rebuild-drop");
    let sqlite = SqliteStore::open(&dir.path().join("mnemex.db")).expect("sqlite");
    let index = open_index(dir.path());
    let ko = symbol(8);
    let ko_id = KoId::new([8; 16]);
    sqlite.insert_ko(&ko).expect("ko");
    let kept = chunk(8);
    let orphan = chunk(9);
    let hash_kept = digest(&[8]);
    let hash_orphan = digest(&[9]);
    sqlite
        .insert_chunk(&kept, &ko_id, &hash_kept)
        .expect("chunk");
    index
        .upsert(&orphan, &hash_orphan, &[0.0, 0.0, 1.0, 0.0])
        .expect("orphan");
    let rows = [AlignmentRow {
        chunk_id: kept,
        content_hash: hash_kept.clone(),
        vector: vec![0.0, 0.0, 0.0, 1.0],
    }];
    rebuild_from_sqlite(&sqlite, &index, &rows).expect("rebuild");
    assert!(
        index
            .read(&orphan, &hash_orphan)
            .expect("orphan read")
            .is_none()
    );
    assert_eq!(
        index.read(&kept, &hash_kept).expect("kept").as_deref(),
        Some([0.0, 0.0, 0.0, 1.0].as_slice())
    );
}

#[test]
fn rebuild_refuses_to_invent_a_missing_vector() {
    let dir = TempDir::new("rebuild-missing");
    let sqlite = SqliteStore::open(&dir.path().join("mnemex.db")).expect("sqlite");
    let index = open_index(dir.path());
    let ko = symbol(10);
    let ko_id = KoId::new([10; 16]);
    sqlite.insert_ko(&ko).expect("ko");
    let id = chunk(10);
    let hash = digest(&[10]);
    sqlite.insert_chunk(&id, &ko_id, &hash).expect("chunk");
    let err =
        rebuild_from_sqlite(&sqlite, &index, &[]).expect_err("empty index cannot invent a vector");
    let msg = err.to_string();
    assert!(msg.contains("vector"), "{msg}");
    assert_eq!(sqlite.chunk_pending(&id).expect("pending"), Some(true));
    assert!(index.read(&id, &hash).expect("read").is_none());
}
