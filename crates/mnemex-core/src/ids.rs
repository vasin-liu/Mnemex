//! Identifier semantics.
//!
//! - KO id: the stable primary reference. Context Bundle references use
//!   KO id + `content_hash` + commit as the primary key, with paths as a
//!   secondary locator only (ADR-011);
//! - `chunk_id` + `content_hash`: the alignment key between SQLite (source
//!   of truth) and the vector store; the vector store is a derived,
//!   rebuildable index (ADR-002).

// Placeholder: id newtypes land with `add-knowledge-object-model`.
