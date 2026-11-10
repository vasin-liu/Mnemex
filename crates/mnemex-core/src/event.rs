//! Event-sourced knowledge events (v1 `knowledge_events` shape, merged into
//! Phase 1 by v3 §七).
//!
//! Every operation on a knowledge object — creation, retrieval use, user
//! verification, user stale marks, contradiction marks, archiving,
//! usefulness feedback — is an immutable, append-only event (spec: 事件
//! 溯源不变式). Numeric lifecycle fields are never mutated by operations;
//! state is derived from the event stream.
//!
//! Each event records where it came from ([`EventSource`]: rule / model /
//! user) so the presentation layer can tell "rule verdict" apart from
//! "you marked" (prototype timeline semantics).
//!
//! Shape note: `KnowledgeEvent` is a struct with private fields wrapping
//! an [`EventKind`] enum — Rust enum variant fields cannot be private, so
//! this is how the immutability contract is made compile-time (design D6).

use crate::ids::KoId;
use crate::lifecycle::UnixMillis;

/// Where an event came from (spec: 事件来源可辨).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventSource {
    /// Deterministic rule / pipeline trigger.
    Rule,
    /// Model extraction or scoring.
    Model,
    /// Explicit user action in the UI.
    User,
}

/// Discriminant of an event; `Feedback` carries the usefulness vote.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventKind {
    /// The object entered the knowledge base.
    Created,
    /// The object was retrieved or referenced.
    Used,
    /// The user verified the object.
    Verified,
    /// The user marked the object stale (coexists with rule freshness).
    MarkedStale,
    /// The object was marked contradictory with a near neighbor.
    MarkedContradictory,
    /// The object was archived.
    Archived,
    /// Usefulness feedback ("useful / not useful" vote).
    Feedback {
        /// Whether the user judged the object useful.
        useful: bool,
    },
}

/// Immutable, append-only event about one knowledge object.
///
/// Constructed only through the associated functions (one per operation);
/// fields are private and there are no mutators, so a constructed event
/// can never be rewritten (spec: 事件不可变).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KnowledgeEvent {
    kind: EventKind,
    ko: KoId,
    at: UnixMillis,
    source: EventSource,
}

impl KnowledgeEvent {
    /// Event: the object was created.
    pub fn created(ko: KoId, at: UnixMillis, source: EventSource) -> Self {
        Self::new(EventKind::Created, ko, at, source)
    }

    /// Event: the object was retrieved or referenced.
    pub fn used(ko: KoId, at: UnixMillis, source: EventSource) -> Self {
        Self::new(EventKind::Used, ko, at, source)
    }

    /// Event: the user verified the object.
    pub fn verified(ko: KoId, at: UnixMillis, source: EventSource) -> Self {
        Self::new(EventKind::Verified, ko, at, source)
    }

    /// Event: the user marked the object stale.
    pub fn marked_stale(ko: KoId, at: UnixMillis, source: EventSource) -> Self {
        Self::new(EventKind::MarkedStale, ko, at, source)
    }

    /// Event: the object was marked contradictory with a near neighbor.
    pub fn marked_contradictory(ko: KoId, at: UnixMillis, source: EventSource) -> Self {
        Self::new(EventKind::MarkedContradictory, ko, at, source)
    }

    /// Event: the object was archived.
    pub fn archived(ko: KoId, at: UnixMillis, source: EventSource) -> Self {
        Self::new(EventKind::Archived, ko, at, source)
    }

    /// Event: usefulness feedback.
    pub fn feedback(ko: KoId, at: UnixMillis, source: EventSource, useful: bool) -> Self {
        Self::new(EventKind::Feedback { useful }, ko, at, source)
    }

    fn new(kind: EventKind, ko: KoId, at: UnixMillis, source: EventSource) -> Self {
        Self {
            kind,
            ko,
            at,
            source,
        }
    }

    /// Returns the operation discriminant.
    pub fn kind(&self) -> EventKind {
        self.kind
    }

    /// Returns the id of the object this event is about.
    pub fn ko(&self) -> KoId {
        self.ko
    }

    /// Returns when the event happened.
    pub fn at(&self) -> UnixMillis {
        self.at
    }

    /// Returns where the event came from.
    pub fn source(&self) -> EventSource {
        self.source
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::KoId;
    use crate::lifecycle::UnixMillis;

    fn ko() -> KoId {
        KoId::new([3; 16])
    }

    fn at() -> UnixMillis {
        UnixMillis::new(1_762_000_000_000)
    }

    #[test]
    fn every_operation_is_an_event() {
        // spec: 操作即事件 — each operation below exists only as an event
        // constructor; none of them mutates numeric lifecycle fields.
        let cases: Vec<(KnowledgeEvent, EventKind)> = vec![
            (
                KnowledgeEvent::created(ko(), at(), EventSource::Rule),
                EventKind::Created,
            ),
            (
                KnowledgeEvent::used(ko(), at(), EventSource::Rule),
                EventKind::Used,
            ),
            (
                KnowledgeEvent::verified(ko(), at(), EventSource::User),
                EventKind::Verified,
            ),
            (
                KnowledgeEvent::marked_stale(ko(), at(), EventSource::User),
                EventKind::MarkedStale,
            ),
            (
                KnowledgeEvent::marked_contradictory(ko(), at(), EventSource::Model),
                EventKind::MarkedContradictory,
            ),
            (
                KnowledgeEvent::archived(ko(), at(), EventSource::User),
                EventKind::Archived,
            ),
            (
                KnowledgeEvent::feedback(ko(), at(), EventSource::User, true),
                EventKind::Feedback { useful: true },
            ),
            (
                KnowledgeEvent::feedback(ko(), at(), EventSource::User, false),
                EventKind::Feedback { useful: false },
            ),
        ];
        for (ev, expected_kind) in &cases {
            assert_eq!(ev.kind(), *expected_kind);
            assert_eq!(ev.ko(), ko());
            assert_eq!(ev.at().get(), 1_762_000_000_000);
        }
    }

    #[test]
    fn event_sources_are_distinguishable() {
        // spec: 事件来源可辨 — rule / model / user are distinguishable so
        // the presentation layer can tell "rule verdict" from "you marked".
        let by_rule = KnowledgeEvent::verified(ko(), at(), EventSource::Rule);
        let by_model = KnowledgeEvent::verified(ko(), at(), EventSource::Model);
        let by_user = KnowledgeEvent::verified(ko(), at(), EventSource::User);
        assert_eq!(by_rule.source(), EventSource::Rule);
        assert_eq!(by_model.source(), EventSource::Model);
        assert_eq!(by_user.source(), EventSource::User);
        assert_ne!(by_rule, by_model);
        assert_ne!(by_model, by_user);
    }

    #[test]
    fn events_are_immutable_after_construction() {
        // spec: 事件不可变 — private fields, no mutators: a constructed event
        // stays equal to its own clone and there is no API that rewrites it.
        let ev = KnowledgeEvent::archived(ko(), at(), EventSource::User);
        assert_eq!(ev, ev.clone());
        let snapshot = ev.clone();
        drop(ev);
        assert_eq!(snapshot.kind(), EventKind::Archived);
    }

    #[test]
    fn feedback_carries_usefulness() {
        let up = KnowledgeEvent::feedback(ko(), at(), EventSource::User, true);
        let down = KnowledgeEvent::feedback(ko(), at(), EventSource::User, false);
        assert!(matches!(up.kind(), EventKind::Feedback { useful: true }));
        assert!(matches!(down.kind(), EventKind::Feedback { useful: false }));
        assert_ne!(up, down);
    }
}
