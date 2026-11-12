//! Ingest pipeline: scan, parse, chunk.
//!
//! Dependency red line: `mnemex-ingest` depends on `mnemex-core` and `sha2`.
//! It does not depend on `mnemex-store`; the CLI writes scan results.
//!
//! Text and Markdown scanning is implemented. Code parsing (Java / Rust /
//! TypeScript / Python) is still a placeholder. Scan pages without a text
//! layer are marked and skipped, never routed to a vision model by default
//! (ADR-012); PDF extraction is not in this crate yet.

mod error;
mod reconcile;

pub mod chunk;
pub mod parse;
pub mod scan;

pub use error::IngestError;
pub use reconcile::{ChunkInsert, SectionHashUpdate, StoredSection, TextSyncPlan, plan_text_sync};
pub use scan::{TextChunk, TextScan, TextSection, scan_text};
