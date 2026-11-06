//! Ingest pipeline: scan, parse, chunk.
//!
//! Dependency red line: `mnemex-ingest` depends only on `mnemex-core`;
//! storage wiring lands with the Phase 0 pipeline change.
//!
//! Planned scope (Phase 0): file scanning, git analysis, four-language
//! parsing (Java / Rust / TypeScript / Python) with explainable call-edge
//! precision, and chunking with a 512-token cap (ADR-003). Scan pages
//! without a text layer are marked and skipped, never routed to a vision
//! model by default (ADR-012).

pub mod chunk;
pub mod parse;
pub mod scan;
