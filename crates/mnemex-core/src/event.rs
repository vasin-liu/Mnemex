//! Event-sourced knowledge events.
//!
//! All Knowledge Object mutations are recorded as append-only events (the v1
//! `knowledge_events` table shape, merged into Phase 1 by v3 §七). Events are
//! never rewritten; corrections and usage marks are new events, and UI
//! operations only append.

// Placeholder: event types land with `add-knowledge-object-model`.
