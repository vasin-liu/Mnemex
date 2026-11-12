//! SQLite schema: v1 tables, migrations, and row mapping (ADR-002).
//!
//! # Migration list is append-only
//!
//! The `MIGRATIONS` list is the ordered list of schema steps. `user_version` N means
//! the first N entries have been applied. Shipped entries are immutable: a
//! later change adds a new entry, it does not edit v1. Rewriting an applied
//! entry would make "already migrated" and "this binary's DDL" disagree, and
//! the ahead-of-program check would no longer mean anything.
//!
//! # `heading_path`
//!
//! [`mnemex_core::ko::DocumentSection::heading_path`] is stored as written.
//! The separator is `/` (for example `adr/002/source-of-truth`). The core
//! type stays [`String`]; this module does not parse or rewrite the path.
//!
//! # `edge_type`
//!
//! `knowledge_edge.edge_type` is a raw `INTEGER`. The value domain (call,
//! import, inherit, and the rest) belongs to the ingest change. v1 only
//! stores the integer and enforces the foreign keys.
//!
//! # Field ownership (design D5)
//!
//! Replay owns only the columns a `Used` event can derive. Everything else
//! is written by its producer and is not recomputed from the event log in v1.
//!
//! | Column | Owner in v1 |
//! | --- | --- |
//! | `usage_count` | Replay of `Used` events (`COUNT`) |
//! | `last_used` | Replay of `Used` events (`MAX(at)`) |
//! | `model_confidence` | Producer (model extraction); `NULL` when no model ran |
//! | `confidence` | Producer (calibration). Stays `NULL` until that gate lands |
//! | `freshness` | Producer (deterministic git / content-hash / mtime rules) |
//! | `contradiction_score` | Producer (near-neighbor calculation) |
//!
//! Created / verified / stale / contradictory / archived / feedback events
//! are stored. v1 does not fold them back into those producer-owned columns.
//!
//! # Integer codes
//!
//! Enum columns are plain `INTEGER`s converted by free functions in this
//! module (not `FromSql` / `ToSql` impls — those traits are foreign and the
//! domain types live in `mnemex-core`). Unknown codes are errors.
//!
//! - `Kind`: 1 code-symbol, 2 design-decision, 3 bug-record, 4 document-section, 5 commit-event
//! - `SymbolKind`: 1 function, 2 class, 3 module
//! - `Freshness`: 0 fresh, 1 stale
//! - `EventKind`: 1 created, 2 used, 3 verified, 4 marked-stale, 5 marked-contradictory, 6 archived, 7 feedback (`useful` is a separate 0/1 column, `NULL` otherwise)
//! - `EventSource`: 1 rule, 2 model, 3 user

use std::error::Error;
use std::fmt;
use std::path::PathBuf;

use mnemex_core::event::{EventKind, EventSource, KnowledgeEvent};
use mnemex_core::ids::{ChunkId, CommitSha, ContentHash, KoId};
use mnemex_core::ko::{
    BugRecord, CodeSymbol, CommitEvent, DesignDecision, DocumentSection, Kind, KnowledgeObject,
    KoCore, SymbolKind,
};
use mnemex_core::lifecycle::{
    Confidence, ContradictionScore, Freshness, Lifecycle, ModelConfidence, UnixMillis,
};
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::sqlite::{SqliteStore, StoreError};

/// `user_version` after every entry in the migration list has been applied.
pub const SCHEMA_VERSION: u32 = MIGRATIONS.len() as u32;

const MIGRATIONS: &[&str] = &[V1_DDL, V2_DDL];

const V1_DDL: &str = "
CREATE TABLE knowledge_objects (
    ko_id BLOB PRIMARY KEY NOT NULL,
    kind INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    model_confidence REAL,
    confidence REAL,
    usage_count INTEGER NOT NULL DEFAULT 0,
    last_used INTEGER,
    freshness INTEGER NOT NULL,
    contradiction_score REAL,
    content_hash BLOB,
    commit_sha BLOB,
    path TEXT,
    symbol_name TEXT,
    symbol_kind INTEGER,
    symbol_container TEXT,
    problem TEXT,
    diagnosis TEXT,
    resolution TEXT,
    symptom TEXT,
    root_cause TEXT,
    fix TEXT,
    title TEXT,
    heading_path TEXT,
    commit_event_sha BLOB,
    commit_message TEXT,
    authored_at INTEGER
);
CREATE TABLE knowledge_events (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    ko_id BLOB NOT NULL REFERENCES knowledge_objects (ko_id),
    kind INTEGER NOT NULL,
    at INTEGER NOT NULL,
    source INTEGER NOT NULL,
    useful INTEGER
);
CREATE INDEX idx_knowledge_events_ko_kind ON knowledge_events (ko_id, kind);
CREATE TABLE chunks (
    chunk_id BLOB PRIMARY KEY NOT NULL,
    ko_id BLOB NOT NULL REFERENCES knowledge_objects (ko_id),
    content_hash BLOB NOT NULL,
    pending INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX idx_chunks_ko ON chunks (ko_id);
CREATE TABLE knowledge_edge (
    src_ko BLOB NOT NULL REFERENCES knowledge_objects (ko_id),
    dst_ko BLOB NOT NULL REFERENCES knowledge_objects (ko_id),
    edge_type INTEGER NOT NULL
);
CREATE INDEX idx_knowledge_edge_src ON knowledge_edge (src_ko);
CREATE INDEX idx_knowledge_edge_dst ON knowledge_edge (dst_ko);
";

/// Adds the chunk body and an external-content FTS5 index over it.
///
/// `trigram` matches Chinese and English without a word segmenter.
/// Queries shorter than 3 characters are rejected by that tokenizer.
const V2_DDL: &str = "
ALTER TABLE chunks ADD COLUMN body TEXT NOT NULL DEFAULT '';
CREATE VIRTUAL TABLE chunks_fts USING fts5(
    body,
    content='chunks',
    content_rowid='rowid',
    tokenize='trigram'
);
CREATE TRIGGER chunks_ai AFTER INSERT ON chunks BEGIN
    INSERT INTO chunks_fts(rowid, body) VALUES (new.rowid, new.body);
END;
CREATE TRIGGER chunks_ad AFTER DELETE ON chunks BEGIN
    INSERT INTO chunks_fts(chunks_fts, rowid, body) VALUES ('delete', old.rowid, old.body);
END;
CREATE TRIGGER chunks_au AFTER UPDATE ON chunks BEGIN
    INSERT INTO chunks_fts(chunks_fts, rowid, body) VALUES ('delete', old.rowid, old.body);
    INSERT INTO chunks_fts(rowid, body) VALUES (new.rowid, new.body);
END;
INSERT INTO chunks_fts(chunks_fts) VALUES ('rebuild');
";

const INSERT_OBJECT: &str = "
INSERT INTO knowledge_objects (
    ko_id, kind, created_at, model_confidence, confidence, usage_count, last_used,
    freshness, contradiction_score, content_hash, commit_sha, path,
    symbol_name, symbol_kind, symbol_container,
    problem, diagnosis, resolution,
    symptom, root_cause, fix,
    title, heading_path,
    commit_event_sha, commit_message, authored_at
) VALUES (
    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
    ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26
)";

/// One knowledge object as stored in `knowledge_objects`.
///
/// The variant is the `kind` column. Sparse columns that belong to other
/// variants are `NULL` and are not part of this value.
#[derive(Debug, Clone, PartialEq)]
pub enum StoredKo {
    /// Function, class, or module.
    CodeSymbol(CodeSymbol),
    /// Problem, diagnosis, and resolution.
    DesignDecision(DesignDecision),
    /// Symptom, root cause, and fix.
    BugRecord(BugRecord),
    /// One section of a document.
    DocumentSection(DocumentSection),
    /// A git commit treated as a knowledge object.
    CommitEvent(CommitEvent),
}

/// One document section currently stored, with the path text left as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredDocumentSection {
    /// Object id.
    pub id: KoId,
    /// `path` column, with `/` separators, not rewritten through `PathBuf`.
    pub path: String,
    /// Section content hash.
    pub content_hash: ContentHash,
    /// Chunk ids belonging to this object.
    pub chunk_ids: Vec<ChunkId>,
}

/// A chunk body to insert during a document sync.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncChunk {
    /// Chunk id.
    pub id: ChunkId,
    /// Chunk content hash.
    pub content_hash: ContentHash,
    /// Chunk text.
    pub body: String,
}

/// A document section that is not stored yet.
#[derive(Debug, Clone, PartialEq)]
pub struct NewDocumentSection {
    /// The object row, including its creation time and content hash.
    pub object: StoredKo,
    /// Chunks inserted with the object.
    pub chunks: Vec<SyncChunk>,
}

/// Deletes, hash updates, and inserts applied in one transaction.
#[derive(Debug, Clone, Default)]
pub struct DocumentSync {
    /// Document sections to remove, with their edges, chunks, and events.
    pub delete_objects: Vec<KoId>,
    /// Chunks to remove from sections that stay.
    pub delete_chunks: Vec<ChunkId>,
    /// New content hashes for sections that stay.
    pub hash_updates: Vec<(KoId, ContentHash)>,
    /// Chunks to add under objects that already exist.
    pub insert_chunks: Vec<(KoId, SyncChunk)>,
    /// New document sections. Each gets one rule-sourced creation event.
    pub insert_sections: Vec<NewDocumentSection>,
}

/// Applies any migrations newer than the database, or refuses a database
/// that is newer than this binary.
pub(crate) fn migrate(conn: &mut Connection) -> Result<(), StoreError> {
    let current: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let latest = i64::from(SCHEMA_VERSION);
    if current > latest {
        let db_version = u32::try_from(current).unwrap_or(u32::MAX);
        return Err(StoreError::Migration { db_version });
    }
    if current < 0 {
        return Err(stored("user_version is negative"));
    }
    let start = usize::try_from(current).map_err(|_| stored("user_version does not fit"))?;
    for (index, ddl) in MIGRATIONS.iter().enumerate().skip(start) {
        let next = i64::try_from(index + 1).map_err(|_| stored("migration index overflow"))?;
        let tx = conn.transaction()?;
        tx.execute_batch(ddl)?;
        tx.pragma_update(None, "user_version", next)?;
        tx.commit()?;
    }
    Ok(())
}

impl SqliteStore {
    /// Inserts one knowledge object. The id must not already be present.
    pub fn insert_ko(&self, ko: &StoredKo) -> Result<(), StoreError> {
        insert_object(&self.conn, ko)
    }

    /// Reads one knowledge object by id. `Ok(None)` when the id is absent.
    ///
    /// An unknown `kind` integer, or a `NULL` in a column the kind requires,
    /// is an error. Missing optional lifecycle and reference columns stay
    /// `None`.
    pub fn read_ko(&self, id: &KoId) -> Result<Option<StoredKo>, StoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM knowledge_objects WHERE ko_id = ?1")?;
        let mut rows = stmt.query(params![id.as_bytes()])?;
        let Some(row) = rows.next()? else {
            return Ok(None);
        };
        Ok(Some(decode_object(row)?))
    }

    /// Appends one event and returns its `seq`.
    ///
    /// While the object stays, this is the only write path for
    /// `knowledge_events`: there is no update and no delete-by-seq. Removing
    /// a document section deletes that object's event rows in the same
    /// transaction.
    pub fn append_event(&self, event: &KnowledgeEvent) -> Result<i64, StoreError> {
        append_event_row(&self.conn, event)
    }

    /// Returns events for `ko` in append order (`seq` ascending).
    pub fn events_for(&self, ko: &KoId) -> Result<Vec<(i64, KnowledgeEvent)>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT seq, kind, at, source, useful FROM knowledge_events \
             WHERE ko_id = ?1 ORDER BY seq",
        )?;
        let mut rows = stmt.query(params![ko.as_bytes()])?;
        let mut out = Vec::new();
        while let Some(row) = rows.next()? {
            let seq: i64 = row.get(0)?;
            let kind = event_kind_from_cols(row.get(1)?, row.get(4)?)?;
            let at = UnixMillis::new(row.get(2)?);
            let source = event_source_from_int(row.get(3)?)?;
            out.push((seq, event_from_parts(kind, *ko, at, source)));
        }
        Ok(out)
    }

    /// Recomputes `usage_count` and `last_used` from `Used` events and
    /// writes them back.
    ///
    /// Returns the derived pair. Other lifecycle columns are left alone.
    pub fn sync_usage_from_events(
        &self,
        ko: &KoId,
    ) -> Result<(u64, Option<UnixMillis>), StoreError> {
        let used = event_kind_to_cols(EventKind::Used).0;
        let (count_i, max_at): (i64, Option<i64>) = self.conn.query_row(
            "SELECT COUNT(*), MAX(at) FROM knowledge_events WHERE ko_id = ?1 AND kind = ?2",
            params![ko.as_bytes(), used],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let count = u64::try_from(count_i).map_err(|_| stored("usage count is negative"))?;
        let last_used = match (count, max_at) {
            (0, _) => None,
            (_, Some(at)) => Some(at),
            (_, None) => return Err(stored("used events have no timestamp")),
        };
        let changed = self.conn.execute(
            "UPDATE knowledge_objects SET usage_count = ?1, last_used = ?2 WHERE ko_id = ?3",
            params![count_i, last_used, ko.as_bytes()],
        )?;
        if changed == 0 {
            return Err(stored("knowledge object not found"));
        }
        Ok((count, last_used.map(UnixMillis::new)))
    }

    /// Inserts a chunk alignment row and its body. `pending` defaults to
    /// waiting (1). The FTS index is maintained by the `chunks_ai` trigger.
    pub fn insert_chunk(
        &self,
        chunk_id: &ChunkId,
        ko: &KoId,
        content_hash: &ContentHash,
        body: &str,
    ) -> Result<(), StoreError> {
        insert_chunk_row(&self.conn, chunk_id, ko, content_hash, body)
    }

    /// Reads the chunk body. `Ok(None)` when the chunk id is absent.
    pub fn chunk_body(&self, chunk_id: &ChunkId) -> Result<Option<String>, StoreError> {
        let body = self
            .conn
            .query_row(
                "SELECT body FROM chunks WHERE chunk_id = ?1",
                params![chunk_id.as_bytes()],
                |row| row.get(0),
            )
            .optional()?;
        Ok(body)
    }

    /// Full-text search over chunk bodies. The trigram index requires a
    /// query of at least 3 characters.
    pub fn search_chunks(&self, query: &str) -> Result<Vec<ChunkId>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT chunks.chunk_id FROM chunks_fts \
             JOIN chunks ON chunks.rowid = chunks_fts.rowid \
             WHERE chunks_fts MATCH ?1",
        )?;
        let mut rows = stmt.query(params![query])?;
        let mut out = Vec::new();
        while let Some(row) = rows.next()? {
            let id_bytes: Vec<u8> = row.get(0)?;
            let chunk_id = ChunkId::from_slice(&id_bytes).map_err(|err| stored(err.to_string()))?;
            out.push(chunk_id);
        }
        Ok(out)
    }

    /// Lists every chunk row: id, content hash, and whether it is still pending.
    pub fn list_chunks(&self) -> Result<Vec<(ChunkId, ContentHash, bool)>, StoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT chunk_id, content_hash, pending FROM chunks")?;
        let mut rows = stmt.query([])?;
        let mut out = Vec::new();
        while let Some(row) = rows.next()? {
            let id_bytes: Vec<u8> = row.get(0)?;
            let hash_bytes: Vec<u8> = row.get(1)?;
            let pending: i64 = row.get(2)?;
            let chunk_id = ChunkId::from_slice(&id_bytes).map_err(|err| stored(err.to_string()))?;
            let content_hash =
                ContentHash::new(hash_bytes).map_err(|err| stored(err.to_string()))?;
            let pending = match pending {
                1 => true,
                0 => false,
                other => return Err(stored(format!("pending flag {other} is not 0 or 1"))),
            };
            out.push((chunk_id, content_hash, pending));
        }
        Ok(out)
    }

    /// Reads the pending flag. `Ok(None)` when the chunk id is absent.
    pub fn chunk_pending(&self, chunk_id: &ChunkId) -> Result<Option<bool>, StoreError> {
        let pending: Option<i64> = self
            .conn
            .query_row(
                "SELECT pending FROM chunks WHERE chunk_id = ?1",
                params![chunk_id.as_bytes()],
                |row| row.get(0),
            )
            .optional()?;
        match pending {
            None => Ok(None),
            Some(1) => Ok(Some(true)),
            Some(0) => Ok(Some(false)),
            Some(other) => Err(stored(format!("pending flag {other} is not 0 or 1"))),
        }
    }

    /// Sets the pending flag on an existing chunk.
    pub fn set_chunk_pending(&self, chunk_id: &ChunkId, pending: bool) -> Result<(), StoreError> {
        let changed = self.conn.execute(
            "UPDATE chunks SET pending = ?1 WHERE chunk_id = ?2",
            params![i64::from(pending), chunk_id.as_bytes()],
        )?;
        if changed == 0 {
            return Err(stored("chunk not found"));
        }
        Ok(())
    }

    /// Inserts one directed edge. `edge_type` is an ingest-defined integer.
    pub fn insert_edge(&self, src: &KoId, dst: &KoId, edge_type: i64) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO knowledge_edge (src_ko, dst_ko, edge_type) VALUES (?1, ?2, ?3)",
            params![src.as_bytes(), dst.as_bytes(), edge_type],
        )?;
        Ok(())
    }

    /// Reads the current document sections and applies `build`'s plan in one
    /// transaction. The path text is the stored column, not a `PathBuf`
    /// display form. Schema version is unchanged.
    pub fn sync_documents<F>(&self, build: F) -> Result<(), StoreError>
    where
        F: FnOnce(&[StoredDocumentSection]) -> DocumentSync,
    {
        let tx = self.conn.unchecked_transaction()?;
        let sections = list_document_sections(&tx)?;
        let plan = build(&sections);
        for chunk_id in &plan.delete_chunks {
            delete_chunk_row(&tx, chunk_id)?;
        }
        for ko in &plan.delete_objects {
            delete_document_section(&tx, ko)?;
        }
        for (ko, content_hash) in &plan.hash_updates {
            update_document_hash(&tx, ko, content_hash)?;
        }
        for (ko, chunk) in &plan.insert_chunks {
            insert_chunk_row(&tx, &chunk.id, ko, &chunk.content_hash, &chunk.body)?;
        }
        for section in &plan.insert_sections {
            if section.object.kind() != Kind::DocumentSection {
                return Err(stored("sync can insert only document sections"));
            }
            insert_object(&tx, &section.object)?;
            let core = section.object.core();
            append_event_row(
                &tx,
                &KnowledgeEvent::created(core.id, core.created_at, EventSource::Rule),
            )?;
            for chunk in &section.chunks {
                insert_chunk_row(&tx, &chunk.id, &core.id, &chunk.content_hash, &chunk.body)?;
            }
        }
        tx.commit()?;
        Ok(())
    }
}

fn insert_object(conn: &Connection, ko: &StoredKo) -> Result<(), StoreError> {
    let row = object_row(ko)?;
    let params: [&dyn rusqlite::types::ToSql; 26] = [
        &row.ko_id,
        &row.kind,
        &row.created_at,
        &row.model_confidence,
        &row.confidence,
        &row.usage_count,
        &row.last_used,
        &row.freshness,
        &row.contradiction_score,
        &row.content_hash,
        &row.commit_sha,
        &row.path,
        &row.symbol_name,
        &row.symbol_kind,
        &row.symbol_container,
        &row.problem,
        &row.diagnosis,
        &row.resolution,
        &row.symptom,
        &row.root_cause,
        &row.fix,
        &row.title,
        &row.heading_path,
        &row.commit_event_sha,
        &row.commit_message,
        &row.authored_at,
    ];
    conn.execute(INSERT_OBJECT, params.as_slice())?;
    Ok(())
}

fn append_event_row(conn: &Connection, event: &KnowledgeEvent) -> Result<i64, StoreError> {
    let (kind, useful) = event_kind_to_cols(event.kind());
    conn.execute(
        "INSERT INTO knowledge_events (ko_id, kind, at, source, useful) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            event.ko().as_bytes(),
            kind,
            event.at().get(),
            event_source_to_int(event.source()),
            useful,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

fn insert_chunk_row(
    conn: &Connection,
    chunk_id: &ChunkId,
    ko: &KoId,
    content_hash: &ContentHash,
    body: &str,
) -> Result<(), StoreError> {
    conn.execute(
        "INSERT INTO chunks (chunk_id, ko_id, content_hash, body) VALUES (?1, ?2, ?3, ?4)",
        params![
            chunk_id.as_bytes(),
            ko.as_bytes(),
            content_hash.as_bytes(),
            body
        ],
    )?;
    Ok(())
}

fn list_document_sections(conn: &Connection) -> Result<Vec<StoredDocumentSection>, StoreError> {
    let mut stmt = conn.prepare(
        "SELECT ko_id, path, content_hash FROM knowledge_objects WHERE kind = ?1 ORDER BY ko_id",
    )?;
    let mut rows = stmt.query(params![kind_to_int(Kind::DocumentSection)])?;
    let mut pending = Vec::new();
    while let Some(row) = rows.next()? {
        let id_bytes: Vec<u8> = row.get(0)?;
        let path: Option<String> = row.get(1)?;
        let hash_bytes: Option<Vec<u8>> = row.get(2)?;
        pending.push((id_bytes, path, hash_bytes));
    }
    drop(rows);
    drop(stmt);

    let mut out = Vec::with_capacity(pending.len());
    for (id_bytes, path, hash_bytes) in pending {
        let id = KoId::from_slice(&id_bytes).map_err(|err| stored(err.to_string()))?;
        let path = path.ok_or_else(|| stored("document section has no path"))?;
        let hash_bytes =
            hash_bytes.ok_or_else(|| stored("document section has no content hash"))?;
        let content_hash = ContentHash::new(hash_bytes).map_err(|err| stored(err.to_string()))?;
        let mut chunk_stmt =
            conn.prepare("SELECT chunk_id FROM chunks WHERE ko_id = ?1 ORDER BY chunk_id")?;
        let mut chunk_rows = chunk_stmt.query(params![id.as_bytes()])?;
        let mut chunk_ids = Vec::new();
        while let Some(chunk_row) = chunk_rows.next()? {
            let bytes: Vec<u8> = chunk_row.get(0)?;
            chunk_ids.push(ChunkId::from_slice(&bytes).map_err(|err| stored(err.to_string()))?);
        }
        out.push(StoredDocumentSection {
            id,
            path,
            content_hash,
            chunk_ids,
        });
    }
    Ok(out)
}

fn delete_chunk_row(conn: &Connection, chunk_id: &ChunkId) -> Result<(), StoreError> {
    let changed = conn.execute(
        "DELETE FROM chunks WHERE chunk_id = ?1",
        params![chunk_id.as_bytes()],
    )?;
    if changed == 0 {
        return Err(stored("chunk not found"));
    }
    Ok(())
}

fn delete_document_section(conn: &Connection, id: &KoId) -> Result<(), StoreError> {
    let kind: Option<i64> = conn
        .query_row(
            "SELECT kind FROM knowledge_objects WHERE ko_id = ?1",
            params![id.as_bytes()],
            |row| row.get(0),
        )
        .optional()?;
    if kind != Some(kind_to_int(Kind::DocumentSection)) {
        return Err(stored("document section not found"));
    }
    conn.execute(
        "DELETE FROM knowledge_edge WHERE src_ko = ?1 OR dst_ko = ?1",
        params![id.as_bytes()],
    )?;
    conn.execute(
        "DELETE FROM chunks WHERE ko_id = ?1",
        params![id.as_bytes()],
    )?;
    conn.execute(
        "DELETE FROM knowledge_events WHERE ko_id = ?1",
        params![id.as_bytes()],
    )?;
    let changed = conn.execute(
        "DELETE FROM knowledge_objects WHERE ko_id = ?1 AND kind = ?2",
        params![id.as_bytes(), kind_to_int(Kind::DocumentSection)],
    )?;
    if changed == 0 {
        return Err(stored("document section not found"));
    }
    Ok(())
}

fn update_document_hash(
    conn: &Connection,
    id: &KoId,
    content_hash: &ContentHash,
) -> Result<(), StoreError> {
    let changed = conn.execute(
        "UPDATE knowledge_objects SET content_hash = ?1 WHERE ko_id = ?2 AND kind = ?3",
        params![
            content_hash.as_bytes(),
            id.as_bytes(),
            kind_to_int(Kind::DocumentSection)
        ],
    )?;
    if changed == 0 {
        return Err(stored("document section not found"));
    }
    Ok(())
}

fn kind_to_int(kind: Kind) -> i64 {
    match kind {
        Kind::CodeSymbol => 1,
        Kind::DesignDecision => 2,
        Kind::BugRecord => 3,
        Kind::DocumentSection => 4,
        Kind::CommitEvent => 5,
    }
}

fn kind_from_int(code: i64) -> Result<Kind, StoreError> {
    match code {
        1 => Ok(Kind::CodeSymbol),
        2 => Ok(Kind::DesignDecision),
        3 => Ok(Kind::BugRecord),
        4 => Ok(Kind::DocumentSection),
        5 => Ok(Kind::CommitEvent),
        other => Err(unknown_int("kind", other)),
    }
}

fn symbol_kind_to_int(kind: SymbolKind) -> i64 {
    match kind {
        SymbolKind::Function => 1,
        SymbolKind::Class => 2,
        SymbolKind::Module => 3,
    }
}

fn symbol_kind_from_int(code: i64) -> Result<SymbolKind, StoreError> {
    match code {
        1 => Ok(SymbolKind::Function),
        2 => Ok(SymbolKind::Class),
        3 => Ok(SymbolKind::Module),
        other => Err(unknown_int("symbol kind", other)),
    }
}

fn freshness_to_int(freshness: Freshness) -> i64 {
    match freshness {
        Freshness::Fresh => 0,
        Freshness::Stale => 1,
    }
}

fn freshness_from_int(code: i64) -> Result<Freshness, StoreError> {
    match code {
        0 => Ok(Freshness::Fresh),
        1 => Ok(Freshness::Stale),
        other => Err(unknown_int("freshness", other)),
    }
}

fn event_kind_to_cols(kind: EventKind) -> (i64, Option<i64>) {
    match kind {
        EventKind::Created => (1, None),
        EventKind::Used => (2, None),
        EventKind::Verified => (3, None),
        EventKind::MarkedStale => (4, None),
        EventKind::MarkedContradictory => (5, None),
        EventKind::Archived => (6, None),
        EventKind::Feedback { useful } => (7, Some(i64::from(useful))),
    }
}

fn event_kind_from_cols(code: i64, useful: Option<i64>) -> Result<EventKind, StoreError> {
    match (code, useful) {
        (1, None) => Ok(EventKind::Created),
        (2, None) => Ok(EventKind::Used),
        (3, None) => Ok(EventKind::Verified),
        (4, None) => Ok(EventKind::MarkedStale),
        (5, None) => Ok(EventKind::MarkedContradictory),
        (6, None) => Ok(EventKind::Archived),
        (7, Some(1)) => Ok(EventKind::Feedback { useful: true }),
        (7, Some(0)) => Ok(EventKind::Feedback { useful: false }),
        (7, Some(other)) => Err(unknown_int("event useful", other)),
        (7, None) => Err(unknown_int("event useful", code)),
        (1..=6, Some(extra)) => Err(unknown_int("event useful", extra)),
        (other, _) => Err(unknown_int("event kind", other)),
    }
}

fn event_source_to_int(source: EventSource) -> i64 {
    match source {
        EventSource::Rule => 1,
        EventSource::Model => 2,
        EventSource::User => 3,
    }
}

fn event_source_from_int(code: i64) -> Result<EventSource, StoreError> {
    match code {
        1 => Ok(EventSource::Rule),
        2 => Ok(EventSource::Model),
        3 => Ok(EventSource::User),
        other => Err(unknown_int("event source", other)),
    }
}

fn event_from_parts(
    kind: EventKind,
    ko: KoId,
    at: UnixMillis,
    source: EventSource,
) -> KnowledgeEvent {
    match kind {
        EventKind::Created => KnowledgeEvent::created(ko, at, source),
        EventKind::Used => KnowledgeEvent::used(ko, at, source),
        EventKind::Verified => KnowledgeEvent::verified(ko, at, source),
        EventKind::MarkedStale => KnowledgeEvent::marked_stale(ko, at, source),
        EventKind::MarkedContradictory => KnowledgeEvent::marked_contradictory(ko, at, source),
        EventKind::Archived => KnowledgeEvent::archived(ko, at, source),
        EventKind::Feedback { useful } => KnowledgeEvent::feedback(ko, at, source, useful),
    }
}

struct ObjectRow {
    ko_id: Vec<u8>,
    kind: i64,
    created_at: i64,
    model_confidence: Option<f64>,
    confidence: Option<f64>,
    usage_count: i64,
    last_used: Option<i64>,
    freshness: i64,
    contradiction_score: Option<f64>,
    content_hash: Option<Vec<u8>>,
    commit_sha: Option<Vec<u8>>,
    path: Option<String>,
    symbol_name: Option<String>,
    symbol_kind: Option<i64>,
    symbol_container: Option<String>,
    problem: Option<String>,
    diagnosis: Option<String>,
    resolution: Option<String>,
    symptom: Option<String>,
    root_cause: Option<String>,
    fix: Option<String>,
    title: Option<String>,
    heading_path: Option<String>,
    commit_event_sha: Option<Vec<u8>>,
    commit_message: Option<String>,
    authored_at: Option<i64>,
}

fn object_row(ko: &StoredKo) -> Result<ObjectRow, StoreError> {
    let core = ko.core();
    let mut row = ObjectRow {
        ko_id: core.id.as_bytes().to_vec(),
        kind: kind_to_int(ko.kind()),
        created_at: core.created_at.get(),
        model_confidence: core
            .lifecycle
            .model_confidence
            .map(|score| f64::from(score.get())),
        confidence: core
            .lifecycle
            .confidence
            .map(|score| f64::from(score.get())),
        usage_count: i64::try_from(core.lifecycle.usage_count)
            .map_err(|_| stored("usage_count exceeds sqlite integer"))?,
        last_used: core.lifecycle.last_used.map(UnixMillis::get),
        freshness: freshness_to_int(core.lifecycle.freshness),
        contradiction_score: core
            .lifecycle
            .contradiction_score
            .map(|score| f64::from(score.get())),
        content_hash: core
            .refs
            .content_hash
            .as_ref()
            .map(|hash| hash.as_bytes().to_vec()),
        commit_sha: core.refs.commit.as_ref().map(|sha| sha.as_bytes().to_vec()),
        path: path_to_string(core.refs.path.as_ref())?,
        symbol_name: None,
        symbol_kind: None,
        symbol_container: None,
        problem: None,
        diagnosis: None,
        resolution: None,
        symptom: None,
        root_cause: None,
        fix: None,
        title: None,
        heading_path: None,
        commit_event_sha: None,
        commit_message: None,
        authored_at: None,
    };
    match ko {
        StoredKo::CodeSymbol(value) => {
            row.symbol_name = Some(value.name.clone());
            row.symbol_kind = Some(symbol_kind_to_int(value.symbol_kind));
            row.symbol_container = value.container.clone();
        }
        StoredKo::DesignDecision(value) => {
            row.problem = Some(value.problem.clone());
            row.diagnosis = Some(value.diagnosis.clone());
            row.resolution = Some(value.resolution.clone());
        }
        StoredKo::BugRecord(value) => {
            row.symptom = Some(value.symptom.clone());
            row.root_cause = Some(value.root_cause.clone());
            row.fix = Some(value.fix.clone());
        }
        StoredKo::DocumentSection(value) => {
            row.title = Some(value.title.clone());
            row.heading_path = Some(value.heading_path.clone());
        }
        StoredKo::CommitEvent(value) => {
            row.commit_event_sha = Some(value.sha.as_bytes().to_vec());
            row.commit_message = Some(value.message.clone());
            row.authored_at = Some(value.authored_at.get());
        }
    }
    Ok(row)
}

fn path_to_string(path: Option<&PathBuf>) -> Result<Option<String>, StoreError> {
    match path {
        None => Ok(None),
        Some(path) => path
            .to_str()
            .map(|text| Some(text.to_owned()))
            .ok_or_else(|| stored("path is not valid utf-8")),
    }
}

fn decode_object(row: &Row<'_>) -> Result<StoredKo, StoreError> {
    let core = decode_core(row)?;
    let kind = kind_from_int(row.get("kind")?)?;
    match kind {
        Kind::CodeSymbol => Ok(StoredKo::CodeSymbol(CodeSymbol {
            name: required_string(row, "symbol_name")?,
            symbol_kind: symbol_kind_from_int(required_i64(row, "symbol_kind")?)?,
            container: row.get("symbol_container")?,
            core,
        })),
        Kind::DesignDecision => Ok(StoredKo::DesignDecision(DesignDecision {
            problem: required_string(row, "problem")?,
            diagnosis: required_string(row, "diagnosis")?,
            resolution: required_string(row, "resolution")?,
            core,
        })),
        Kind::BugRecord => Ok(StoredKo::BugRecord(BugRecord {
            symptom: required_string(row, "symptom")?,
            root_cause: required_string(row, "root_cause")?,
            fix: required_string(row, "fix")?,
            core,
        })),
        Kind::DocumentSection => Ok(StoredKo::DocumentSection(DocumentSection {
            title: required_string(row, "title")?,
            heading_path: required_string(row, "heading_path")?,
            core,
        })),
        Kind::CommitEvent => {
            let sha_bytes = required_blob(row, "commit_event_sha")?;
            let sha = CommitSha::new(sha_bytes).map_err(|err| stored(err.to_string()))?;
            let authored_at = required_i64(row, "authored_at")?;
            Ok(StoredKo::CommitEvent(CommitEvent {
                sha,
                message: required_string(row, "commit_message")?,
                authored_at: UnixMillis::new(authored_at),
                core,
            }))
        }
    }
}

fn decode_core(row: &Row<'_>) -> Result<KoCore, StoreError> {
    let id_bytes: Vec<u8> = row.get("ko_id")?;
    let id = KoId::from_slice(&id_bytes).map_err(|err| stored(err.to_string()))?;
    let content_hash = optional_hash(row.get("content_hash")?)?;
    let commit = optional_sha(row.get("commit_sha")?)?;
    let path: Option<String> = row.get("path")?;
    let model_confidence = optional_model(row.get("model_confidence")?)?;
    let confidence = optional_confidence(row.get("confidence")?)?;
    let usage: i64 = row.get("usage_count")?;
    let usage_count = u64::try_from(usage).map_err(|_| stored("usage_count is negative"))?;
    let last_used: Option<i64> = row.get("last_used")?;
    let contradiction_score = optional_contradiction(row.get("contradiction_score")?)?;
    Ok(KoCore {
        id,
        refs: mnemex_core::ids::SourceRefs {
            content_hash,
            commit,
            path: path.map(PathBuf::from),
        },
        lifecycle: Lifecycle {
            model_confidence,
            confidence,
            usage_count,
            last_used: last_used.map(UnixMillis::new),
            freshness: freshness_from_int(row.get("freshness")?)?,
            contradiction_score,
        },
        created_at: UnixMillis::new(row.get("created_at")?),
    })
}

fn optional_hash(bytes: Option<Vec<u8>>) -> Result<Option<ContentHash>, StoreError> {
    match bytes {
        None => Ok(None),
        Some(bytes) => Ok(Some(
            ContentHash::new(bytes).map_err(|err| stored(err.to_string()))?,
        )),
    }
}

fn optional_sha(bytes: Option<Vec<u8>>) -> Result<Option<CommitSha>, StoreError> {
    match bytes {
        None => Ok(None),
        Some(bytes) => Ok(Some(
            CommitSha::new(bytes).map_err(|err| stored(err.to_string()))?,
        )),
    }
}

fn unit_f32(raw: f64) -> Result<f32, StoreError> {
    if !raw.is_finite() {
        return Err(stored("score is not finite"));
    }
    let score = raw as f32;
    if !score.is_finite() {
        return Err(stored("score is outside f32"));
    }
    Ok(score)
}

fn optional_model(raw: Option<f64>) -> Result<Option<ModelConfidence>, StoreError> {
    match raw {
        None => Ok(None),
        Some(raw) => Ok(Some(
            ModelConfidence::new(unit_f32(raw)?).map_err(|err| stored(err.to_string()))?,
        )),
    }
}

fn optional_confidence(raw: Option<f64>) -> Result<Option<Confidence>, StoreError> {
    match raw {
        None => Ok(None),
        Some(raw) => Ok(Some(
            Confidence::new(unit_f32(raw)?).map_err(|err| stored(err.to_string()))?,
        )),
    }
}

fn optional_contradiction(raw: Option<f64>) -> Result<Option<ContradictionScore>, StoreError> {
    match raw {
        None => Ok(None),
        Some(raw) => Ok(Some(
            ContradictionScore::new(unit_f32(raw)?).map_err(|err| stored(err.to_string()))?,
        )),
    }
}

fn required_string(row: &Row<'_>, column: &'static str) -> Result<String, StoreError> {
    let value: Option<String> = row.get(column)?;
    value.ok_or_else(|| stored(format!("column {column} is null")))
}

fn required_i64(row: &Row<'_>, column: &'static str) -> Result<i64, StoreError> {
    let value: Option<i64> = row.get(column)?;
    value.ok_or_else(|| stored(format!("column {column} is null")))
}

fn required_blob(row: &Row<'_>, column: &'static str) -> Result<Vec<u8>, StoreError> {
    let value: Option<Vec<u8>> = row.get(column)?;
    value.ok_or_else(|| stored(format!("column {column} is null")))
}

#[derive(Debug)]
struct StoredValueError(String);

impl fmt::Display for StoredValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for StoredValueError {}

fn stored(message: impl Into<String>) -> StoreError {
    stored_typed(rusqlite::types::Type::Null, message)
}

fn unknown_int(label: &'static str, code: i64) -> StoreError {
    stored_typed(
        rusqlite::types::Type::Integer,
        format!("unknown {label} code {code}"),
    )
}

fn stored_typed(ty: rusqlite::types::Type, message: impl Into<String>) -> StoreError {
    StoreError::Sqlite(rusqlite::Error::FromSqlConversionFailure(
        0,
        ty,
        Box::new(StoredValueError(message.into())),
    ))
}

impl StoredKo {
    fn core(&self) -> &KoCore {
        match self {
            Self::CodeSymbol(value) => value.core(),
            Self::DesignDecision(value) => value.core(),
            Self::BugRecord(value) => value.core(),
            Self::DocumentSection(value) => value.core(),
            Self::CommitEvent(value) => value.core(),
        }
    }

    fn kind(&self) -> Kind {
        match self {
            Self::CodeSymbol(value) => value.kind(),
            Self::DesignDecision(value) => value.kind(),
            Self::BugRecord(value) => value.kind(),
            Self::DocumentSection(value) => value.kind(),
            Self::CommitEvent(value) => value.kind(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enum_mappings_roundtrip_and_reject_unknown() {
        for kind in [
            Kind::CodeSymbol,
            Kind::DesignDecision,
            Kind::BugRecord,
            Kind::DocumentSection,
            Kind::CommitEvent,
        ] {
            assert_eq!(kind_from_int(kind_to_int(kind)).expect("known"), kind);
        }
        assert!(kind_from_int(99).is_err());

        for sk in [SymbolKind::Function, SymbolKind::Class, SymbolKind::Module] {
            assert_eq!(
                symbol_kind_from_int(symbol_kind_to_int(sk)).expect("known"),
                sk
            );
        }
        assert!(symbol_kind_from_int(0).is_err());

        for freshness in [Freshness::Fresh, Freshness::Stale] {
            assert_eq!(
                freshness_from_int(freshness_to_int(freshness)).expect("known"),
                freshness
            );
        }
        assert!(freshness_from_int(7).is_err());

        let events = [
            EventKind::Created,
            EventKind::Used,
            EventKind::Verified,
            EventKind::MarkedStale,
            EventKind::MarkedContradictory,
            EventKind::Archived,
            EventKind::Feedback { useful: true },
            EventKind::Feedback { useful: false },
        ];
        for event in events {
            let (code, useful) = event_kind_to_cols(event);
            assert_eq!(event_kind_from_cols(code, useful).expect("known"), event);
        }
        assert!(event_kind_from_cols(99, None).is_err());

        for src in [EventSource::Rule, EventSource::Model, EventSource::User] {
            assert_eq!(
                event_source_from_int(event_source_to_int(src)).expect("known"),
                src
            );
        }
        assert!(event_source_from_int(-1).is_err());
    }

    #[test]
    fn v1_database_gains_an_empty_body_and_keeps_later_text() {
        let dir = std::env::temp_dir().join(format!(
            "mnemex-v1-upgrade-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let db = dir.join("mnemex.db");
        let ko_id = KoId::new([0x41; 16]);
        let old_id = ChunkId::new([0x42; 16]);
        let new_id = ChunkId::new([0x43; 16]);
        let old_hash = ContentHash::new(vec![1, 2, 3]).expect("hash");
        let new_hash = ContentHash::new(vec![4, 5, 6]).expect("hash");
        {
            let conn = Connection::open(&db).expect("create v1 file");
            conn.execute_batch(V1_DDL).expect("v1 ddl");
            conn.pragma_update(None, "user_version", 1_i64)
                .expect("mark v1");
            conn.execute(
                "INSERT INTO knowledge_objects (ko_id, kind, created_at, freshness) \
                 VALUES (?1, 4, 1, 0)",
                params![ko_id.as_bytes()],
            )
            .expect("v1 ko");
            conn.execute(
                "INSERT INTO chunks (chunk_id, ko_id, content_hash) VALUES (?1, ?2, ?3)",
                params![old_id.as_bytes(), ko_id.as_bytes(), old_hash.as_bytes()],
            )
            .expect("v1 chunk");
        }

        let store = SqliteStore::open(&db).expect("upgrade");
        let version: i64 = store.query_scalar("PRAGMA user_version").expect("version");
        assert_eq!(version, 2);
        assert_eq!(
            store.chunk_body(&old_id).expect("old body"),
            Some(String::new())
        );
        assert_eq!(
            store.chunk_pending(&old_id).expect("old still pending"),
            Some(true)
        );
        store
            .insert_chunk(&new_id, &ko_id, &new_hash, "kept-body")
            .expect("write body after upgrade");
        drop(store);

        let store = SqliteStore::open(&db).expect("reopen");
        let version: i64 = store.query_scalar("PRAGMA user_version").expect("version");
        assert_eq!(version, 2);
        assert_eq!(
            store.chunk_body(&new_id).expect("kept body"),
            Some("kept-body".to_string())
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
