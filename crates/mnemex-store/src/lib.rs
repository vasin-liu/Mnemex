//! Storage abstractions for Mnemex.
//!
//! Dependency red line: `mnemex-store` depends on `mnemex-core`, `rusqlite`
//! (MIT), and `lancedb` 0.41.0 (Apache-2.0). Arrow, Tokio, and futures are
//! the public API and runtime of that LanceDB release.
//!
//! SQLite is the source of truth (FTS5, knowledge objects, events, graph
//! edges). The vector store is a derived index that can be dropped and fully
//! rebuilt from SQLite at any time.

pub mod schema;
pub mod sqlite;
pub mod vector;

pub use vector::{AlignmentRow, LanceVectorStore, VectorStore, catch_up, rebuild_from_sqlite};
