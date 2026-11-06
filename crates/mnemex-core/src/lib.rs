//! Mnemex core domain model.
//!
//! Dependency red line: `mnemex-core` is the innermost crate and MUST NOT
//! depend on any other `mnemex-*` crate. Every other workspace member may
//! depend on it, never the other way around.
//!
//! This scaffold pins the module vocabulary only; concrete type fields land
//! with the `add-knowledge-object-model` change.

pub mod event;
pub mod ids;
pub mod ko;
pub mod lifecycle;
