//! Storage abstractions for Mnemex.
//!
//! Dependency red line: `mnemex-store` depends only on `mnemex-core`.
//!
//! SQLite is the source of truth (FTS5, knowledge objects, events, graph
//! edges). The vector store is a derived index that can be dropped and fully
//! rebuilt from SQLite at any time.

/// Abstraction over vector store backends (LanceDB preferred, sqlite-vec
/// fallback — ADR-001).
///
/// Contract highlights (ADR-002):
/// - SQLite is the source of truth; the vector store is derived and
///   fully rebuildable;
/// - rows align with SQLite chunks by `chunk_id` + `content_hash`;
/// - implementations must support exact deletes by that alignment key and
///   full rebuilds; a dimension change equals a full re-embed (ADR-003).
///
/// Methods land with the `add-lancedb-spike` change.
pub trait VectorStore {
    // Intentionally empty placeholder; see the trait-level docs.
}
