//! Lifecycle fields shared by Knowledge Objects.
//!
//! Five fields:
//! - `confidence`: calibrated score. Split from the raw `model_confidence`;
//!   calibration is a post-roadmap item, so until it lands the calibrated
//!   value does not participate in ranking (ADR-007, ADR-010);
//! - `usage_count`: how often the object was retrieved or referenced;
//! - `last_used`: timestamp of the last use;
//! - `freshness`: deterministic staleness from git / content hash / mtime —
//!   never model-scored (ADR-007);
//! - `contradiction_score`: disagreement with near-neighbor candidates.

// Placeholder: field types land with `add-knowledge-object-model`.
