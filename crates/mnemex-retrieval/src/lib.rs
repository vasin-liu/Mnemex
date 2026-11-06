//! Retrieval paths and ranking.
//!
//! Dependency red line: `mnemex-retrieval` depends on `mnemex-core` and
//! `mnemex-store` only.
//!
//! Fast Path (ADR-007): SQLite FTS5 and vector search each return top-k,
//! fused with RRF. `freshness` downweights deterministically (git / hash /
//! mtime). The `confidence` downweight consumes the calibrated value only
//! and stays OFF until calibration lands.
//!
//! Deep Path: graph expansion plus reranker, triggered on demand; its gain
//! is quantified on the Phase 1 regression eval set (ADR-008).

pub mod deep;
pub mod fast;
pub mod fusion;
pub mod rank;
