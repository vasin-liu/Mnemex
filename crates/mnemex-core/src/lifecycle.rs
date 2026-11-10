//! Lifecycle fields carried by every knowledge object (v2 §五).
//!
//! Five fields: `model_confidence` (raw model score, absent without model
//! extraction), `confidence` (calibrated score, absent until calibration
//! lands — the "未校准" badge source, ADR-010), `usage_count`, `last_used`,
//! `freshness` (deterministic rule output: git / content hash / mtime,
//! never model-scored, ADR-007), and `contradiction_score` (computed only
//! for near-neighbor candidates).
//!
//! Operations never mutate these numbers directly: they are events
//! (see [`crate::event`]) and the state is derived from the event stream.

use std::error::Error;
use std::fmt;

/// Error returned when a score falls outside `[0.0, 1.0]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScoreError {
    /// The score was outside the valid range (NaN included).
    OutOfRange,
}

impl fmt::Display for ScoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfRange => write!(f, "score must be within [0.0, 1.0]"),
        }
    }
}

impl Error for ScoreError {}

fn check_range(value: f32) -> Result<(), ScoreError> {
    if (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(ScoreError::OutOfRange)
    }
}

/// Raw score produced by a model (ADR-010). Not interchangeable with
/// [`Confidence`]: assigning one to the other does not compile.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModelConfidence(f32);

impl ModelConfidence {
    /// Creates a raw model score in `[0.0, 1.0]`.
    pub fn new(value: f32) -> Result<Self, ScoreError> {
        check_range(value)?;
        Ok(Self(value))
    }

    /// Returns the raw score.
    pub fn get(self) -> f32 {
        self.0
    }
}

/// Calibrated confidence (ADR-010). Calibration is a post-roadmap item, so
/// this value stays `None` in every [`Lifecycle`] until that gate passes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Confidence(f32);

impl Confidence {
    /// Creates a calibrated score in `[0.0, 1.0]`.
    pub fn new(value: f32) -> Result<Self, ScoreError> {
        check_range(value)?;
        Ok(Self(value))
    }

    /// Returns the calibrated score.
    pub fn get(self) -> f32 {
        self.0
    }
}

/// Contradiction score, computed only for near-neighbor candidates
/// (same symbol / same file / vector neighbor; ADR-010).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContradictionScore(f32);

impl ContradictionScore {
    /// Creates a contradiction score in `[0.0, 1.0]`.
    pub fn new(value: f32) -> Result<Self, ScoreError> {
        check_range(value)?;
        Ok(Self(value))
    }

    /// Returns the score.
    pub fn get(self) -> f32 {
        self.0
    }
}

/// Deterministic staleness verdict, produced by rule sources only
/// (git / content hash / mtime — ADR-007).
///
/// There is deliberately no model-scored variant and no payload: the
/// presentation layer merges this rule verdict with the user's own
/// "marked stale" event (`crate::event::KnowledgeEvent::MarkedStale`)
/// instead of overwriting it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Freshness {
    /// Rule sources say the content matches current state.
    #[default]
    Fresh,
    /// Rule sources say the content is out of date.
    Stale,
}

/// UTC timestamp in milliseconds since the Unix epoch (design D7).
///
/// Timezone handling and human formatting belong to the adapter/UI layer,
/// not to the domain model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct UnixMillis(i64);

impl UnixMillis {
    /// Creates a timestamp from raw milliseconds.
    pub const fn new(millis: i64) -> Self {
        Self(millis)
    }

    /// Returns the raw milliseconds.
    pub const fn get(self) -> i64 {
        self.0
    }
}

/// Lifecycle state shared by every knowledge object (v2 §五, with the
/// ADR-010 confidence split).
///
/// Default state: nothing extracted, nothing used, rule freshness `Fresh`,
/// `confidence` absent because calibration has not landed.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Lifecycle {
    /// Raw model score, when a model extracted one.
    pub model_confidence: Option<ModelConfidence>,
    /// Calibrated score; `None` until the calibration gate passes.
    pub confidence: Option<Confidence>,
    /// How often this object was retrieved or referenced.
    pub usage_count: u64,
    /// Timestamp of the last use, if ever used.
    pub last_used: Option<UnixMillis>,
    /// Rule-verdict staleness (deterministic sources only).
    pub freshness: Freshness,
    /// Contradiction score for near-neighbor candidates, if computed.
    pub contradiction_score: Option<ContradictionScore>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn score_bounds_reject_out_of_range() {
        for bad in [-0.1_f32, 1.1_f32, f32::NAN, f32::INFINITY] {
            assert!(
                ModelConfidence::new(bad).is_err(),
                "model score {bad} must be rejected"
            );
            assert!(
                Confidence::new(bad).is_err(),
                "calibrated score {bad} must be rejected"
            );
            assert!(
                ContradictionScore::new(bad).is_err(),
                "contradiction {bad} must be rejected"
            );
        }
        for good in [0.0_f32, 0.25, 1.0] {
            let raw = ModelConfidence::new(good).expect("in-range raw score");
            assert_eq!(raw.get(), good);
            let calibrated = Confidence::new(good).expect("in-range calibrated score");
            assert_eq!(calibrated.get(), good);
            let contra = ContradictionScore::new(good).expect("in-range contradiction");
            assert_eq!(contra.get(), good);
        }
    }

    #[test]
    fn raw_and_calibrated_confidence_are_distinct_types() {
        // spec: 置信度双字段分离 — raw score must not slip into the
        // calibrated slot (type system rejects the mixing at compile time).
        assert_ne!(
            std::any::type_name::<ModelConfidence>(),
            std::any::type_name::<Confidence>()
        );
    }

    #[test]
    fn lifecycle_default_is_uncalibrated_and_unused() {
        let lc = Lifecycle::default();
        assert!(lc.model_confidence.is_none());
        assert!(
            lc.confidence.is_none(),
            "calibration not landed: confidence stays absent"
        );
        assert_eq!(lc.usage_count, 0);
        assert!(lc.last_used.is_none());
        assert_eq!(lc.freshness, Freshness::Fresh);
        assert!(lc.contradiction_score.is_none());
    }

    #[test]
    fn freshness_has_exactly_two_rule_variants() {
        // Exhaustive match without a wildcard: adding a variant breaks this
        // test at compile time, so the "rule-only, no model entry" shape is
        // pinned (spec: freshness 仅规则来源).
        let mut names = Vec::new();
        for f in [Freshness::Fresh, Freshness::Stale] {
            names.push(match f {
                Freshness::Fresh => "fresh",
                Freshness::Stale => "stale",
            });
        }
        assert_eq!(names.len(), 2);
    }

    #[test]
    fn unix_millis_roundtrip() {
        let t = UnixMillis::new(1_762_000_000_000);
        assert_eq!(t.get(), 1_762_000_000_000);
    }
}
