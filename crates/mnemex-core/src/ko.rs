//! Knowledge Object (KO) types (v2 §二).
//!
//! Documents and code are one object space, not a code-only graph. Five
//! planned types cover it: [`CodeSymbol`], [`DesignDecision`],
//! [`BugRecord`], [`DocumentSection`], and [`CommitEvent`] — all reached
//! through the unified [`KnowledgeObject`] contract (composition, not
//! inheritance; 优先级结论批次二).
//!
//! Structural relations (call / inherit / import edges) are NOT embedded
//! here (design D2): they live in the store's edge graph (SQLite edge
//! table, ADR-005) and are queried by KO id. Likewise, a [`CommitEvent`]
//! associates the objects it touched through the event stream and store
//! references, not through an embedded list.

use crate::ids::{CommitSha, KoId, SourceRefs};
use crate::lifecycle::{Lifecycle, UnixMillis};

/// Discriminant of the five knowledge object types (exactly five; spec:
/// 类型判别完备).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// Function / class / module code unit.
    CodeSymbol,
    /// Problem–diagnosis–resolution triple.
    DesignDecision,
    /// Debug record with root cause and fix.
    BugRecord,
    /// Semantic unit of a split document.
    DocumentSection,
    /// Git commit as a first-class event.
    CommitEvent,
}

impl Kind {
    /// Returns a stable kebab-case label (identifier-style English).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CodeSymbol => "code-symbol",
            Self::DesignDecision => "design-decision",
            Self::BugRecord => "bug-record",
            Self::DocumentSection => "document-section",
            Self::CommitEvent => "commit-event",
        }
    }
}

/// Fields shared by every knowledge object (design D1).
#[derive(Debug, Clone, PartialEq)]
pub struct KoCore {
    /// Stable primary reference.
    pub id: KoId,
    /// Primary (hash + commit) and secondary (path) references.
    pub refs: SourceRefs,
    /// Lifecycle state; derived from the event stream.
    pub lifecycle: Lifecycle,
    /// Creation timestamp.
    pub created_at: UnixMillis,
}

/// Unified contract over all knowledge object types.
///
/// Implementors expose their shared [`KoCore`] plus a [`Kind`]
/// discriminant; `id`, `lifecycle`, `refs`, and `created_at` come for free
/// as default methods.
pub trait KnowledgeObject {
    /// Returns the shared core of this object.
    fn core(&self) -> &KoCore;

    /// Returns the type discriminant.
    fn kind(&self) -> Kind;

    /// Returns the object id.
    fn id(&self) -> &KoId {
        &self.core().id
    }

    /// Returns the lifecycle state.
    fn lifecycle(&self) -> &Lifecycle {
        &self.core().lifecycle
    }

    /// Returns the primary/secondary references.
    fn refs(&self) -> &SourceRefs {
        &self.core().refs
    }

    /// Returns the creation timestamp.
    fn created_at(&self) -> UnixMillis {
        self.core().created_at
    }
}

/// The kind of code unit a [`CodeSymbol`] represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymbolKind {
    /// A function or method.
    Function,
    /// A class, struct, or trait.
    Class,
    /// A module or namespace.
    Module,
}

/// Function/class/module code unit (v2 §二).
///
/// Call / inherit / import relations are not embedded (design D2); they
/// are edges in the store's graph, queried by this symbol's id.
#[derive(Debug, Clone, PartialEq)]
pub struct CodeSymbol {
    /// Shared core.
    pub core: KoCore,
    /// Simple name of the symbol.
    pub name: String,
    /// What kind of code unit this is.
    pub symbol_kind: SymbolKind,
    /// Qualified container (module path), when known.
    pub container: Option<String>,
}

impl KnowledgeObject for CodeSymbol {
    fn core(&self) -> &KoCore {
        &self.core
    }

    fn kind(&self) -> Kind {
        Kind::CodeSymbol
    }
}

/// Problem–diagnosis–resolution triple extracted from commits or
/// documents (v2 §二; prototype detail page uses the same three cards).
#[derive(Debug, Clone, PartialEq)]
pub struct DesignDecision {
    /// Shared core.
    pub core: KoCore,
    /// The problem being decided.
    pub problem: String,
    /// The diagnosis / reasoning.
    pub diagnosis: String,
    /// The chosen resolution.
    pub resolution: String,
}

impl KnowledgeObject for DesignDecision {
    fn core(&self) -> &KoCore {
        &self.core
    }

    fn kind(&self) -> Kind {
        Kind::DesignDecision
    }
}

/// Debug record: symptom, root cause, and fix (v2 §二).
#[derive(Debug, Clone, PartialEq)]
pub struct BugRecord {
    /// Shared core.
    pub core: KoCore,
    /// The observed symptom.
    pub symptom: String,
    /// The diagnosed root cause.
    pub root_cause: String,
    /// The applied fix.
    pub fix: String,
}

impl KnowledgeObject for BugRecord {
    fn core(&self) -> &KoCore {
        &self.core
    }

    fn kind(&self) -> Kind {
        Kind::BugRecord
    }
}

/// Semantic unit of a document, split by section (v2 §二).
#[derive(Debug, Clone, PartialEq)]
pub struct DocumentSection {
    /// Shared core.
    pub core: KoCore,
    /// Section title.
    pub title: String,
    /// Heading path locating the section inside the document. Separator
    /// convention is finalized with the store schema (design D8).
    pub heading_path: String,
}

impl KnowledgeObject for DocumentSection {
    fn core(&self) -> &KoCore {
        &self.core
    }

    fn kind(&self) -> Kind {
        Kind::DocumentSection
    }
}

/// Git commit as a first-class event (v2 §二).
///
/// The affected-object association is carried by the event stream and
/// store references (design D2), not by an embedded list here.
#[derive(Debug, Clone, PartialEq)]
pub struct CommitEvent {
    /// Shared core.
    pub core: KoCore,
    /// The commit's own identifier.
    pub sha: CommitSha,
    /// Commit message.
    pub message: String,
    /// Authoring timestamp.
    pub authored_at: UnixMillis,
}

impl KnowledgeObject for CommitEvent {
    fn core(&self) -> &KoCore {
        &self.core
    }

    fn kind(&self) -> Kind {
        Kind::CommitEvent
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::KoId;
    use crate::lifecycle::UnixMillis;

    fn core(id_byte: u8) -> KoCore {
        KoCore {
            id: KoId::new([id_byte; 16]),
            refs: SourceRefs::default(),
            lifecycle: Lifecycle::default(),
            created_at: UnixMillis::new(1_762_000_000_000),
        }
    }

    #[test]
    fn five_types_share_one_contract() {
        let symbol = CodeSymbol {
            core: core(1),
            name: "VectorStore".to_string(),
            symbol_kind: SymbolKind::Class,
            container: Some("mnemex-store".to_string()),
        };
        let decision = DesignDecision {
            core: core(2),
            problem: "two stores, no cross-txn".to_string(),
            diagnosis: "vector index is derived".to_string(),
            resolution: "SQLite is source of truth".to_string(),
        };
        let bug = BugRecord {
            core: core(3),
            symptom: "NaN on load".to_string(),
            root_cause: "float16 load".to_string(),
            fix: "forbid float16".to_string(),
        };
        let section = DocumentSection {
            core: core(4),
            title: "双库一致性协议".to_string(),
            heading_path: "docs/adr#adr-002".to_string(),
        };
        let commit = CommitEvent {
            core: core(5),
            sha: crate::ids::CommitSha::new(vec![0xab; 20]).expect("non-empty sha"),
            message: "feat: scaffold".to_string(),
            authored_at: UnixMillis::new(1_762_000_000_000),
        };

        let objects: Vec<Box<dyn KnowledgeObject>> = vec![
            Box::new(symbol),
            Box::new(decision),
            Box::new(bug),
            Box::new(section),
            Box::new(commit),
        ];
        let expected = [
            Kind::CodeSymbol,
            Kind::DesignDecision,
            Kind::BugRecord,
            Kind::DocumentSection,
            Kind::CommitEvent,
        ];
        for (i, obj) in objects.iter().enumerate() {
            // All five types walk the same access path (spec: 统一契约访问).
            assert_eq!(obj.kind(), expected[i]);
            assert_eq!(obj.id(), &KoId::new([(i as u8 + 1); 16]));
            assert_eq!(obj.created_at().get(), 1_762_000_000_000);
            assert!(obj.lifecycle().confidence.is_none());
            assert!(obj.refs().path.is_none());
        }
    }

    #[test]
    fn kind_covers_exactly_five_types() {
        // Exhaustive match without a wildcard: a sixth variant breaks this
        // test at compile time (spec: 类型判别完备).
        let mut labels = Vec::new();
        for k in [
            Kind::CodeSymbol,
            Kind::DesignDecision,
            Kind::BugRecord,
            Kind::DocumentSection,
            Kind::CommitEvent,
        ] {
            labels.push(match k {
                Kind::CodeSymbol => Kind::CodeSymbol.as_str(),
                Kind::DesignDecision => Kind::DesignDecision.as_str(),
                Kind::BugRecord => Kind::BugRecord.as_str(),
                Kind::DocumentSection => Kind::DocumentSection.as_str(),
                Kind::CommitEvent => Kind::CommitEvent.as_str(),
            });
        }
        assert_eq!(labels.len(), 5);
        assert_eq!(labels, [
            "code-symbol",
            "design-decision",
            "bug-record",
            "document-section",
            "commit-event"
        ]);
    }

    #[test]
    fn structural_edges_are_not_embedded() {
        // D2: KO types carry identity and semantic fields only; call/import
        // edges live in the store's edge graph, so no field here is a
        // collection of relations. This pins the field set.
        let symbol = CodeSymbol {
            core: core(9),
            name: "f".to_string(),
            symbol_kind: SymbolKind::Function,
            container: None,
        };
        assert_eq!(symbol.container, None);
        let text = format!("{symbol:?}");
        assert!(!text.contains("edges"), "no relation collections in the type");
    }
}
