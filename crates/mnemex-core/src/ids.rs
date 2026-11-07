//! Identifier types for the Mnemex knowledge model.
//!
//! All identifiers are opaque newtypes with no implicit conversion between
//! them (spec: knowledge-object-model / 引用语义).
//!
//! Alignment and reference semantics:
//! - `ChunkId` + `ContentHash` is the alignment key between SQLite (source
//!   of truth) and the vector store (ADR-002);
//! - references to a knowledge object use KO id + `ContentHash` + commit as
//!   the primary key, with paths as a secondary locator only (ADR-011);
//! - `ContentHash` does not commit to a hash algorithm; the ingest stage
//!   picks one when it introduces hashing (design D3).

use std::error::Error;
use std::fmt;
use std::path::PathBuf;

/// Error returned when an identifier cannot be constructed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdError {
    /// The input byte string was empty.
    Empty,
    /// The input byte string had a length other than the required 16 bytes.
    InvalidLength,
}

impl fmt::Display for IdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "identifier byte string must not be empty"),
            Self::InvalidLength => write!(f, "identifier must be exactly 16 bytes"),
        }
    }
}

impl Error for IdError {}

/// Opaque knowledge-object id (16 bytes).
///
/// The generation strategy (random / content-addressed / ULID-like) is
/// decided by the ingest stage; this type only fixes the shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KoId([u8; 16]);

impl KoId {
    /// Creates an id from a fixed 16-byte value.
    pub const fn new(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// Creates an id from a slice; fails unless it holds exactly 16 bytes.
    pub fn from_slice(bytes: &[u8]) -> Result<Self, IdError> {
        let arr: [u8; 16] = bytes
            .try_into()
            .map_err(|_| IdError::InvalidLength)?;
        Ok(Self(arr))
    }

    /// Returns the id bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

/// Opaque chunk id (16 bytes) — the SQLite↔vector alignment key
/// together with [`ContentHash`] (ADR-002). Never interchangeable with
/// [`KoId`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkId([u8; 16]);

impl ChunkId {
    /// Creates a chunk id from a fixed 16-byte value.
    pub const fn new(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// Creates a chunk id from a slice; fails unless it holds exactly
    /// 16 bytes.
    pub fn from_slice(bytes: &[u8]) -> Result<Self, IdError> {
        let arr: [u8; 16] = bytes
            .try_into()
            .map_err(|_| IdError::InvalidLength)?;
        Ok(Self(arr))
    }

    /// Returns the id bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

/// Opaque content digest of a chunk or object.
///
/// Variable length on purpose: the hash algorithm is an ingest-stage
/// decision (design D3). An empty digest is always rejected.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ContentHash(Box<[u8]>);

impl ContentHash {
    /// Creates a digest from non-empty bytes.
    pub fn new(bytes: impl Into<Box<[u8]>>) -> Result<Self, IdError> {
        let bytes = bytes.into();
        if bytes.is_empty() {
            return Err(IdError::Empty);
        }
        Ok(Self(bytes))
    }

    /// Returns the digest bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

/// Opaque git commit identifier (sha-1 or sha-256 sized; design D3).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CommitSha(Box<[u8]>);

impl CommitSha {
    /// Creates a commit id from non-empty bytes.
    pub fn new(bytes: impl Into<Box<[u8]>>) -> Result<Self, IdError> {
        let bytes = bytes.into();
        if bytes.is_empty() {
            return Err(IdError::Empty);
        }
        Ok(Self(bytes))
    }

    /// Returns the commit id bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

/// Primary-vs-secondary references to a knowledge object (ADR-011).
///
/// `content_hash` + `commit` are the primary, compaction-stable key;
/// `path` is only a locator aid — a missing or changed path does not
/// invalidate the primary reference.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct SourceRefs {
    /// Digest of the referenced content, when known.
    pub content_hash: Option<ContentHash>,
    /// Commit the content was last seen at, when known.
    pub commit: Option<CommitSha>,
    /// Secondary locator aid; may be absent or stale.
    pub path: Option<PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ko_id_roundtrip() {
        let bytes = [7u8; 16];
        let id = KoId::new(bytes);
        assert_eq!(id.as_bytes(), &bytes);
    }

    #[test]
    fn chunk_id_from_slice_rejects_wrong_length() {
        let short = [1u8; 15];
        assert!(matches!(ChunkId::from_slice(&short), Err(IdError::InvalidLength)));
        let ok = [1u8; 16];
        let id = ChunkId::from_slice(&ok).expect("16 bytes must be accepted");
        assert_eq!(id.as_bytes(), ok);
    }

    #[test]
    fn four_id_types_are_distinct_and_derived() {
        // Distinct types: swapping them cannot compile (spec: 标识防混用).
        assert_ne!(
            std::any::type_name::<KoId>(),
            std::any::type_name::<ChunkId>()
        );
        assert_ne!(
            std::any::type_name::<ContentHash>(),
            std::any::type_name::<CommitSha>()
        );
        // Debug / Clone / Eq usable on all four.
        let ko = KoId::new([0; 16]);
        assert_eq!(ko.clone(), ko);
        let chunk = ChunkId::new([1; 16]);
        assert_eq!(chunk.clone(), chunk);
        let hash = ContentHash::new(vec![1, 2, 3]).expect("non-empty digest");
        assert_eq!(hash.clone(), hash);
        let sha = CommitSha::new(vec![9; 20]).expect("non-empty sha");
        assert_eq!(sha.clone(), sha);
        let text = format!("{hash:?}");
        assert!(text.contains("ContentHash"));
    }

    #[test]
    fn content_hash_rejects_empty() {
        assert!(matches!(ContentHash::new(Vec::new()), Err(IdError::Empty)));
        assert!(matches!(CommitSha::new(Vec::new()), Err(IdError::Empty)));
    }

    #[test]
    fn source_refs_defaults_to_all_absent() {
        let refs = SourceRefs::default();
        assert!(refs.content_hash.is_none());
        assert!(refs.commit.is_none());
        assert!(refs.path.is_none());
    }

    #[test]
    fn source_refs_path_only_is_valid() {
        // Primary keys absent, path alone is a valid secondary-only reference.
        let refs = SourceRefs {
            content_hash: None,
            commit: None,
            path: Some(std::path::PathBuf::from("docs/adr/ADR-002.md")),
        };
        assert!(refs.path.is_some());
    }
}
