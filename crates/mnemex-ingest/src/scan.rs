//! Directory walk for text and Markdown files.
//!
//! Identifiers are derived so a second scan of unchanged bytes yields the
//! same object id and chunk ids (design D3). The relative path uses `/`
//! even on Windows.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use mnemex_core::ids::{ChunkId, ContentHash, KoId};
use mnemex_core::lifecycle::UnixMillis;
use sha2::{Digest, Sha256};

use crate::chunk::{sections_from_text, split_body};
use crate::error::IngestError;

/// One scalar-capped piece of a section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextChunk {
    /// Chunk id derived from the object id, ordinal, and content hash.
    pub id: ChunkId,
    /// Zero-based position inside the section.
    pub ordinal: u32,
    /// SHA-256 of this chunk's UTF-8 body.
    pub content_hash: ContentHash,
    /// Chunk text. Chunks of one section concatenate to the section body.
    pub body: String,
}

/// A document section produced by a text scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextSection {
    /// Object id derived from the relative path and heading path.
    pub id: KoId,
    /// Path relative to the scan root, with `/` separators.
    pub relative_path: String,
    /// Section title.
    pub title: String,
    /// Heading titles joined with `/`.
    pub heading_path: String,
    /// SHA-256 of the section body before it was split.
    pub content_hash: ContentHash,
    /// File modification time, milliseconds since the Unix epoch.
    pub created_at: UnixMillis,
    /// Non-empty chunks of this section.
    pub chunks: Vec<TextChunk>,
}

/// Result of one text scan.
///
/// `present_paths` lists every text file that was seen, including empty and
/// non-UTF-8 files. `undecodable_paths` is the subset that was not valid
/// UTF-8. Both use `/` separators. Empty and undecodable files produce no
/// sections.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextScan {
    /// Sections from files that decoded as non-empty UTF-8.
    pub sections: Vec<TextSection>,
    /// Relative paths of text files that exist under the root.
    pub present_paths: Vec<String>,
    /// Relative paths that exist but are not valid UTF-8.
    pub undecodable_paths: Vec<String>,
}

/// Recursively scans `root` for `.md`, `.markdown`, and `.txt` files.
///
/// `.git`, `target`, and `node_modules` path segments are skipped. A file
/// that is not valid UTF-8 is recorded and skipped. An empty file produces
/// no section.
pub fn scan_text(root: &Path) -> Result<TextScan, IngestError> {
    let meta = std::fs::metadata(root).map_err(|source| IngestError::RootUnavailable {
        path: root.to_path_buf(),
        source,
    })?;
    if !meta.is_dir() {
        return Err(IngestError::RootUnavailable {
            path: root.to_path_buf(),
            source: std::io::Error::other("scan root is not a directory"),
        });
    }

    let mut files = Vec::new();
    collect(root, root, &mut files)?;
    files.sort();

    let mut sections = Vec::new();
    let mut present_paths = Vec::new();
    let mut undecodable_paths = Vec::new();
    for relative in files {
        let Some(relative_path) = portable(&relative) else {
            continue;
        };
        present_paths.push(relative_path.clone());
        let full = root.join(&relative);
        let bytes = std::fs::read(&full).map_err(|source| IngestError::Unreadable {
            path: full.clone(),
            source,
        })?;
        let Ok(text) = String::from_utf8(bytes) else {
            undecodable_paths.push(relative_path);
            continue;
        };
        if text.is_empty() {
            continue;
        }
        let created_at = modified_millis(&full)?;
        let stem = file_stem(&full);
        for draft in sections_from_text(&stem, &text) {
            let chunks = split_body(&draft.body);
            if chunks.is_empty() {
                continue;
            }
            let section_hash = content_hash(&full, draft.body.as_bytes())?;
            let id = object_id(&relative_path, &draft.heading_path);
            let mut text_chunks = Vec::with_capacity(chunks.len());
            for (ordinal, body) in chunks.into_iter().enumerate() {
                let ordinal = u32::try_from(ordinal).map_err(|_| IngestError::Unreadable {
                    path: full.clone(),
                    source: std::io::Error::new(ErrorKind::InvalidData, "chunk ordinal overflow"),
                })?;
                let hash = content_hash(&full, body.as_bytes())?;
                let chunk_id = chunk_id(&id, ordinal, &hash);
                text_chunks.push(TextChunk {
                    id: chunk_id,
                    ordinal,
                    content_hash: hash,
                    body,
                });
            }
            sections.push(TextSection {
                id,
                relative_path: relative_path.clone(),
                title: draft.title,
                heading_path: draft.heading_path,
                content_hash: section_hash,
                created_at,
                chunks: text_chunks,
            });
        }
    }
    Ok(TextScan {
        sections,
        present_paths,
        undecodable_paths,
    })
}

fn collect(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), IngestError> {
    let read = std::fs::read_dir(dir).map_err(|source| {
        if dir == root {
            IngestError::RootUnavailable {
                path: dir.to_path_buf(),
                source,
            }
        } else {
            IngestError::Unreadable {
                path: dir.to_path_buf(),
                source,
            }
        }
    })?;
    let mut entries = Vec::new();
    for entry in read {
        entries.push(entry.map_err(|source| IngestError::Unreadable {
            path: dir.to_path_buf(),
            source,
        })?);
    }
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let relative = path
            .strip_prefix(root)
            .map_err(|_| IngestError::Unreadable {
                path: path.clone(),
                source: std::io::Error::other("path escaped the scan root"),
            })?;
        if denied(relative) {
            continue;
        }
        let file_type = entry
            .file_type()
            .map_err(|source| IngestError::Unreadable {
                path: path.clone(),
                source,
            })?;
        if file_type.is_dir() {
            collect(root, &path, out)?;
        } else if file_type.is_file() && is_text_file(&path) {
            out.push(relative.to_path_buf());
        }
    }
    Ok(())
}

fn denied(relative: &Path) -> bool {
    relative.components().any(|component| {
        let name = component.as_os_str();
        name == ".git" || name == "target" || name == "node_modules"
    })
}

fn is_text_file(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|ext| ext.to_str()) else {
        return false;
    };
    let ext = ext.to_ascii_lowercase();
    ext == "md" || ext == "markdown" || ext == "txt"
}

fn portable(relative: &Path) -> Option<String> {
    let mut parts = Vec::new();
    for component in relative.components() {
        parts.push(component.as_os_str().to_str()?.to_string());
    }
    Some(parts.join("/"))
}

fn file_stem(path: &Path) -> String {
    match path.file_stem().and_then(|stem| stem.to_str()) {
        Some(stem) if !stem.is_empty() => stem.to_string(),
        _ => "document".to_string(),
    }
}

fn modified_millis(path: &Path) -> Result<UnixMillis, IngestError> {
    let modified = std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .map_err(|source| IngestError::Unreadable {
            path: path.to_path_buf(),
            source,
        })?;
    let millis = match modified.duration_since(UNIX_EPOCH) {
        Ok(duration) => i64::try_from(duration.as_millis()).unwrap_or(i64::MAX),
        Err(_) => 0,
    };
    Ok(UnixMillis::new(millis))
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    let digest = Sha256::digest(bytes);
    let mut out = [0u8; 32];
    out.copy_from_slice(digest.as_slice());
    out
}

fn content_hash(path: &Path, bytes: &[u8]) -> Result<ContentHash, IngestError> {
    ContentHash::new(sha256(bytes).to_vec()).map_err(|_| IngestError::Unreadable {
        path: path.to_path_buf(),
        source: std::io::Error::new(ErrorKind::InvalidData, "content hash was empty"),
    })
}

fn object_id(relative_path: &str, heading_path: &str) -> KoId {
    let mut material = Vec::with_capacity(relative_path.len() + 1 + heading_path.len());
    material.extend_from_slice(relative_path.as_bytes());
    material.push(b'/');
    material.extend_from_slice(heading_path.as_bytes());
    KoId::new(first16(&sha256(&material)))
}

fn chunk_id(ko: &KoId, ordinal: u32, hash: &ContentHash) -> ChunkId {
    let mut material = Vec::with_capacity(16 + 4 + hash.as_bytes().len());
    material.extend_from_slice(ko.as_bytes());
    material.extend_from_slice(&ordinal.to_be_bytes());
    material.extend_from_slice(hash.as_bytes());
    ChunkId::new(first16(&sha256(&material)))
}

fn first16(digest: &[u8; 32]) -> [u8; 16] {
    let mut id = [0u8; 16];
    id.copy_from_slice(&digest[..16]);
    id
}
