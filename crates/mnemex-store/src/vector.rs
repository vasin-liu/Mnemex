//! Derived vector index (ADR-001, ADR-002).
//!
//! SQLite remains the source of truth. Rows align on `chunk_id` plus
//! `content_hash`. This module never invents a vector the caller did not
//! supply, and LanceDB types stay behind [`VectorStore`].

use std::collections::HashMap;
use std::future::Future;
use std::path::Path;
use std::sync::Arc;

use arrow_array::{
    Array, FixedSizeListArray, Float32Array, LargeStringArray, RecordBatch, StringArray,
    StringViewArray,
};
use arrow_schema::{ArrowError, DataType, Field, Schema, SchemaRef};
use futures::TryStreamExt;
use lancedb::query::{ExecutableQuery, QueryBase};
use mnemex_core::ids::{ChunkId, ContentHash, KoId};

use crate::sqlite::{SqliteStore, StoreError};

const TABLE_NAME: &str = "chunks";

/// One alignment key plus the caller-supplied vector to write for it.
#[derive(Debug, Clone, PartialEq)]
pub struct AlignmentRow {
    /// Chunk identifier half of the alignment key.
    pub chunk_id: ChunkId,
    /// Content digest half of the alignment key.
    pub content_hash: ContentHash,
    /// Caller-supplied vector. Length must match the dimension fixed at open.
    pub vector: Vec<f32>,
}

/// Abstraction over vector store backends (LanceDB preferred, sqlite-vec
/// fallback — ADR-001).
///
/// Contract (ADR-002):
/// - SQLite is the source of truth; the vector store is derived and
///   fully rebuildable;
/// - rows align with SQLite chunks by `chunk_id` + `content_hash`;
/// - exact deletes use that alignment key;
/// - a dimension change equals a full re-embed (ADR-003) and is not done here.
pub trait VectorStore {
    /// Inserts or replaces the vector stored under the alignment key.
    fn upsert(
        &self,
        chunk_id: &ChunkId,
        content_hash: &ContentHash,
        vector: &[f32],
    ) -> Result<(), StoreError>;

    /// Reads the vector for an alignment key. `Ok(None)` when the key is absent.
    fn read(
        &self,
        chunk_id: &ChunkId,
        content_hash: &ContentHash,
    ) -> Result<Option<Vec<f32>>, StoreError>;

    /// Deletes one alignment key. Other keys stay.
    fn delete(&self, chunk_id: &ChunkId, content_hash: &ContentHash) -> Result<(), StoreError>;

    /// Makes the index contain exactly `rows`. Keys absent from `rows` are
    /// dropped. Each row is written from the caller-supplied vector.
    fn rebuild(&self, rows: &[AlignmentRow]) -> Result<(), StoreError>;
}

/// Local LanceDB dataset opened at a fixed vector dimension.
///
/// The dataset directory is the caller's choice. Tests place it next to the
/// SQLite file so both live under one parent directory (ADR-001 backup).
pub struct LanceVectorStore {
    runtime: tokio::runtime::Runtime,
    table: lancedb::Table,
    dimension: usize,
    schema: SchemaRef,
}

impl LanceVectorStore {
    /// Opens or creates a local dataset at `dir` with a fixed dimension.
    ///
    /// A dataset that already has a different vector width is an error.
    /// The width is not altered in place.
    pub fn open(dir: &Path, dimension: usize) -> Result<Self, StoreError> {
        let dimension_i32 = i32::try_from(dimension)
            .map_err(|_| vector_detail("dimension does not fit in the arrow field"))?;
        if dimension_i32 <= 0 {
            return Err(vector_detail("dimension must be positive"));
        }
        let uri = dir
            .to_str()
            .ok_or_else(|| vector_detail("dataset path is not utf-8"))?
            .to_string();
        std::fs::create_dir_all(dir)
            .map_err(|err| vector_cause("failed to create the dataset directory", err))?;
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|err| vector_cause("failed to start the vector runtime", err))?;
        let schema = chunk_schema(dimension_i32);
        let table = runtime.block_on(open_table(&uri, schema.clone(), dimension))?;
        Ok(Self {
            runtime,
            table,
            dimension,
            schema,
        })
    }

    fn block<T>(&self, fut: impl Future<Output = Result<T, StoreError>>) -> Result<T, StoreError> {
        self.runtime.block_on(fut)
    }
}

impl VectorStore for LanceVectorStore {
    fn upsert(
        &self,
        chunk_id: &ChunkId,
        content_hash: &ContentHash,
        vector: &[f32],
    ) -> Result<(), StoreError> {
        check_dimension(self.dimension, vector)?;
        let batch = alignment_batch(&self.schema, chunk_id, content_hash, vector)?;
        let table = self.table.clone();
        let predicate = predicate(chunk_id, content_hash);
        self.block(async move {
            table.delete(predicate.as_str()).await.map_err(from_lance)?;
            table.add(batch).execute().await.map_err(from_lance)?;
            Ok(())
        })
    }

    fn read(
        &self,
        chunk_id: &ChunkId,
        content_hash: &ContentHash,
    ) -> Result<Option<Vec<f32>>, StoreError> {
        let table = self.table.clone();
        let predicate = predicate(chunk_id, content_hash);
        let hits = self.block(async move { collect_hits(&table, Some(predicate)).await })?;
        let mut found = None;
        for hit in hits {
            if hit.chunk_id != *chunk_id || hit.content_hash != *content_hash {
                continue;
            }
            if found.is_some() {
                return Err(vector_detail("alignment key matched more than one row"));
            }
            found = Some(hit.vector);
        }
        Ok(found)
    }

    fn delete(&self, chunk_id: &ChunkId, content_hash: &ContentHash) -> Result<(), StoreError> {
        let table = self.table.clone();
        let predicate = predicate(chunk_id, content_hash);
        self.block(async move {
            table.delete(predicate.as_str()).await.map_err(from_lance)?;
            Ok(())
        })
    }

    fn rebuild(&self, rows: &[AlignmentRow]) -> Result<(), StoreError> {
        let mut wanted = HashMap::<(ChunkId, ContentHash), &[f32]>::new();
        for row in rows {
            check_dimension(self.dimension, &row.vector)?;
            let key = (row.chunk_id, row.content_hash.clone());
            if wanted.insert(key, row.vector.as_slice()).is_some() {
                return Err(vector_detail("duplicate alignment key in rebuild"));
            }
        }
        let existing = self.alignment_keys()?;
        for (chunk_id, content_hash) in &existing {
            let key = (*chunk_id, content_hash.clone());
            if !wanted.contains_key(&key) {
                self.delete(chunk_id, content_hash)?;
            }
        }
        for row in rows {
            self.upsert(&row.chunk_id, &row.content_hash, &row.vector)?;
        }
        Ok(())
    }
}

impl LanceVectorStore {
    fn alignment_keys(&self) -> Result<Vec<(ChunkId, ContentHash)>, StoreError> {
        let table = self.table.clone();
        let hits = self.block(async move { collect_hits(&table, None).await })?;
        Ok(hits
            .into_iter()
            .map(|hit| (hit.chunk_id, hit.content_hash))
            .collect())
    }
}

/// Commits the SQLite chunk row first, writes the vector, and clears
/// `pending` only after the alignment key reads back equal.
pub fn catch_up(
    sqlite: &SqliteStore,
    index: &impl VectorStore,
    ko: &KoId,
    chunk_id: &ChunkId,
    content_hash: &ContentHash,
    vector: &[f32],
) -> Result<(), StoreError> {
    ensure_chunk(sqlite, ko, chunk_id, content_hash)?;
    index.upsert(chunk_id, content_hash, vector)?;
    match index.read(chunk_id, content_hash)? {
        Some(got) if got == vector => {}
        _ => {
            index.delete(chunk_id, content_hash)?;
            return Err(vector_detail("read-back did not match the written vector"));
        }
    }
    sqlite.set_chunk_pending(chunk_id, false)
}

/// Rebuilds the index so its alignment keys match the current SQLite chunk
/// rows, using caller vectors when the index does not already hold one.
///
/// Pending rows are cleared only after the rebuilt key reads back.
pub fn rebuild_from_sqlite(
    sqlite: &SqliteStore,
    index: &impl VectorStore,
    supplied: &[AlignmentRow],
) -> Result<(), StoreError> {
    let chunks = sqlite.list_chunks()?;
    let mut supplied_at = HashMap::<(ChunkId, ContentHash), usize>::new();
    for (index_of, row) in supplied.iter().enumerate() {
        let key = (row.chunk_id, row.content_hash.clone());
        if supplied_at.insert(key, index_of).is_some() {
            return Err(vector_detail("duplicate supplied alignment key"));
        }
    }

    let mut desired = Vec::with_capacity(chunks.len());
    for (chunk_id, content_hash, _) in &chunks {
        let key = (*chunk_id, content_hash.clone());
        if let Some(at) = supplied_at.get(&key) {
            desired.push(supplied[*at].clone());
            continue;
        }
        match index.read(chunk_id, content_hash)? {
            Some(vector) => desired.push(AlignmentRow {
                chunk_id: *chunk_id,
                content_hash: content_hash.clone(),
                vector,
            }),
            None => {
                return Err(vector_detail(
                    "missing caller vector for a sqlite chunk row",
                ));
            }
        }
    }

    index.rebuild(&desired)?;
    for (chunk_id, content_hash, pending) in &chunks {
        let expected = desired
            .iter()
            .find(|row| row.chunk_id == *chunk_id && row.content_hash == *content_hash);
        let Some(expected) = expected else {
            return Err(vector_detail("rebuild dropped a sqlite chunk row"));
        };
        match index.read(chunk_id, content_hash)? {
            Some(got) if got == expected.vector => {}
            _ => return Err(vector_detail("rebuild read-back did not match")),
        }
        if *pending {
            sqlite.set_chunk_pending(chunk_id, false)?;
        }
    }
    Ok(())
}

fn ensure_chunk(
    sqlite: &SqliteStore,
    ko: &KoId,
    chunk_id: &ChunkId,
    content_hash: &ContentHash,
) -> Result<(), StoreError> {
    let rows = sqlite.list_chunks()?;
    match rows.iter().find(|(id, _, _)| id == chunk_id) {
        None => sqlite.insert_chunk(chunk_id, ko, content_hash),
        Some((_, stored, _)) if stored == content_hash => Ok(()),
        Some(_) => Err(vector_detail(
            "committed chunk content hash does not match the vector key",
        )),
    }
}

fn check_dimension(dimension: usize, vector: &[f32]) -> Result<(), StoreError> {
    if vector.len() == dimension {
        Ok(())
    } else {
        Err(vector_detail(format!(
            "vector length {} does not match dimension {dimension}",
            vector.len()
        )))
    }
}

fn chunk_schema(dimension: i32) -> SchemaRef {
    let item = Arc::new(Field::new("item", DataType::Float32, true));
    Arc::new(Schema::new(vec![
        Field::new("chunk_id", DataType::Utf8, false),
        Field::new("content_hash", DataType::Utf8, false),
        Field::new("vector", DataType::FixedSizeList(item, dimension), false),
    ]))
}

async fn open_table(
    uri: &str,
    schema: SchemaRef,
    dimension: usize,
) -> Result<lancedb::Table, StoreError> {
    let conn = lancedb::connect(uri).execute().await.map_err(from_lance)?;
    match conn.open_table(TABLE_NAME).execute().await {
        Ok(table) => {
            let existing = table.schema().await.map_err(from_lance)?;
            let found = schema_dimension(&existing)?;
            if found != dimension {
                return Err(vector_detail(format!(
                    "opened dimension {found} does not match dimension {dimension}"
                )));
            }
            Ok(table)
        }
        Err(lancedb::Error::TableNotFound { .. }) => conn
            .create_empty_table(TABLE_NAME, schema)
            .execute()
            .await
            .map_err(from_lance),
        Err(err) => Err(from_lance(err)),
    }
}

fn schema_dimension(schema: &Schema) -> Result<usize, StoreError> {
    let field = schema.field_with_name("vector").map_err(from_arrow)?;
    match field.data_type() {
        DataType::FixedSizeList(_, size) => {
            let size = usize::try_from(*size)
                .map_err(|_| vector_detail("vector dimension is negative"))?;
            if size == 0 {
                return Err(vector_detail("dimension must be positive"));
            }
            Ok(size)
        }
        other => Err(vector_detail(format!("vector column has type {other}"))),
    }
}

struct Hit {
    chunk_id: ChunkId,
    content_hash: ContentHash,
    vector: Vec<f32>,
}

async fn collect_hits(
    table: &lancedb::Table,
    filter: Option<String>,
) -> Result<Vec<Hit>, StoreError> {
    let mut query = table.query();
    if let Some(filter) = filter {
        query = query.only_if(filter);
    }
    let stream = query.execute().await.map_err(from_lance)?;
    let batches: Vec<RecordBatch> = stream.try_collect().await.map_err(from_lance)?;
    let mut hits = Vec::new();
    for batch in &batches {
        hits.extend(hits_in_batch(batch)?);
    }
    Ok(hits)
}

fn hits_in_batch(batch: &RecordBatch) -> Result<Vec<Hit>, StoreError> {
    let schema = batch.schema();
    let id_idx = schema.index_of("chunk_id").map_err(from_arrow)?;
    let hash_idx = schema.index_of("content_hash").map_err(from_arrow)?;
    let vec_idx = schema.index_of("vector").map_err(from_arrow)?;
    let ids = batch.column(id_idx);
    let hashes = batch.column(hash_idx);
    let vectors = batch.column(vec_idx);
    let mut hits = Vec::with_capacity(batch.num_rows());
    for row in 0..batch.num_rows() {
        let (chunk_id, content_hash) = parse_key(&string_at(ids, row)?, &string_at(hashes, row)?)?;
        hits.push(Hit {
            chunk_id,
            content_hash,
            vector: vector_at(vectors, row)?,
        });
    }
    Ok(hits)
}

fn alignment_batch(
    schema: &SchemaRef,
    chunk_id: &ChunkId,
    content_hash: &ContentHash,
    vector: &[f32],
) -> Result<RecordBatch, StoreError> {
    let field = schema.field_with_name("vector").map_err(from_arrow)?;
    let DataType::FixedSizeList(item, size) = field.data_type() else {
        return Err(vector_detail("vector column is not a fixed-size list"));
    };
    let values = Float32Array::from(vector.to_vec());
    let list = FixedSizeListArray::try_new(item.clone(), *size, Arc::new(values), None)
        .map_err(from_arrow)?;
    RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(StringArray::from(vec![hex_encode(chunk_id.as_bytes())])),
            Arc::new(StringArray::from(vec![hex_encode(content_hash.as_bytes())])),
            Arc::new(list),
        ],
    )
    .map_err(from_arrow)
}

fn predicate(chunk_id: &ChunkId, content_hash: &ContentHash) -> String {
    format!(
        "chunk_id = '{}' AND content_hash = '{}'",
        hex_encode(chunk_id.as_bytes()),
        hex_encode(content_hash.as_bytes())
    )
}

fn string_at(column: &dyn Array, row: usize) -> Result<String, StoreError> {
    if column.is_null(row) {
        return Err(vector_detail("null alignment key"));
    }
    if let Some(values) = column.as_any().downcast_ref::<StringArray>() {
        return Ok(values.value(row).to_string());
    }
    if let Some(values) = column.as_any().downcast_ref::<LargeStringArray>() {
        return Ok(values.value(row).to_string());
    }
    if let Some(values) = column.as_any().downcast_ref::<StringViewArray>() {
        return Ok(values.value(row).to_string());
    }
    Err(vector_detail(format!(
        "alignment key column has type {}",
        column.data_type()
    )))
}

fn vector_at(column: &dyn Array, row: usize) -> Result<Vec<f32>, StoreError> {
    let Some(lists) = column.as_any().downcast_ref::<FixedSizeListArray>() else {
        return Err(vector_detail(format!(
            "vector column has type {}",
            column.data_type()
        )));
    };
    if lists.is_null(row) {
        return Err(vector_detail("null vector"));
    }
    let values = lists.value(row);
    let Some(floats) = values.as_any().downcast_ref::<Float32Array>() else {
        return Err(vector_detail("vector values are not float32"));
    };
    let mut out = Vec::with_capacity(floats.len());
    for value in floats.iter() {
        let Some(value) = value else {
            return Err(vector_detail("null vector component"));
        };
        out.push(value);
    }
    Ok(out)
}

fn parse_key(id_hex: &str, hash_hex: &str) -> Result<(ChunkId, ContentHash), StoreError> {
    let id = hex_decode(id_hex)?;
    let hash = hex_decode(hash_hex)?;
    let chunk_id = ChunkId::from_slice(&id).map_err(|err| vector_cause("invalid chunk id", err))?;
    let content_hash =
        ContentHash::new(hash).map_err(|err| vector_cause("invalid content hash", err))?;
    Ok((chunk_id, content_hash))
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from(HEX[usize::from(byte >> 4)]));
        out.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    out
}

fn hex_decode(text: &str) -> Result<Vec<u8>, StoreError> {
    let bytes = text.as_bytes();
    if !bytes.len().is_multiple_of(2) {
        return Err(vector_detail("alignment key hex has an odd length"));
    }
    let mut out = Vec::with_capacity(bytes.len() / 2);
    for [hi, lo] in bytes.as_chunks::<2>().0 {
        let hi = hex_val(*hi)?;
        let lo = hex_val(*lo)?;
        out.push((hi << 4) | lo);
    }
    Ok(out)
}

fn hex_val(byte: u8) -> Result<u8, StoreError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(vector_detail("alignment key is not lowercase hex")),
    }
}

fn from_lance(err: lancedb::Error) -> StoreError {
    vector_cause(err.to_string(), err)
}

fn from_arrow(err: ArrowError) -> StoreError {
    vector_cause(err.to_string(), err)
}

fn vector_detail(detail: impl Into<String>) -> StoreError {
    StoreError::Vector {
        detail: detail.into(),
        cause: None,
    }
}

fn vector_cause(
    detail: impl Into<String>,
    cause: impl std::error::Error + Send + Sync + 'static,
) -> StoreError {
    StoreError::Vector {
        detail: detail.into(),
        cause: Some(Box::new(cause)),
    }
}
