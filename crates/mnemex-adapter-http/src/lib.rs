//! Independent local HTTP/JSON adapter.
//!
//! Dependency red line: `mnemex-adapter-http` and `mnemex-adapter-mcp`
//! never depend on each other; they share only the engine interface
//! (`mnemex-retrieval`) per v3 §5.3. The JSON API must not inherit MCP
//! protocol semantics (`resultType` / `_meta`); merging the two would tie
//! the product API to a protocol.
//!
//! Privacy: the server binds loopback by default. Any non-loopback egress
//! requires the triple gate (ADR-004).

// Placeholder: protocol wiring lands with the Phase 2 adapter change.
