//! MCP adapter (dual-era).
//!
//! Dependency red line: `mnemex-adapter-mcp` and `mnemex-adapter-http`
//! never depend on each other; they share only the engine interface
//! (`mnemex-retrieval`) per v3 §5.3.
//!
//! Baseline (ADR-006): the 2026-07-28 stateless path is primary
//! (`server/discover`, `_meta` version negotiation, `resultType` on every
//! response, MRTR for missing client info), while `initialize` remains for
//! legacy clients and is not torn down in Phase 2. Client-supplied scope
//! (workspace path + optional branch/commit) stays a plain tool parameter;
//! server-minted handles are reserved for true server state (index
//! snapshots, pagination cursors, subscription and Tasks handles).

// Placeholder: protocol wiring lands with the Phase 2 adapter change.
