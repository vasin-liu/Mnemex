//! Pure diff between a text scan and the document sections already stored.
//!
//! No disk and no database. The store applies the plan in one transaction.

use std::collections::HashSet;

use mnemex_core::ids::{ChunkId, ContentHash, KoId};

use crate::scan::{TextChunk, TextScan, TextSection};

/// A document section the caller already has in the database.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredSection {
    /// Object id.
    pub id: KoId,
    /// Relative path as stored, with `/` separators.
    pub relative_path: String,
    /// SHA-256 of the section body before it was split.
    pub content_hash: ContentHash,
    /// Chunk ids currently stored for this object.
    pub chunk_ids: Vec<ChunkId>,
}

/// New content hash for a section that still exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionHashUpdate {
    /// Object id that stays.
    pub id: KoId,
    /// Hash of the new section body.
    pub content_hash: ContentHash,
}

/// A chunk to insert under an object that already exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkInsert {
    /// Existing object id.
    pub ko_id: KoId,
    /// Chunk that was not stored yet.
    pub chunk: TextChunk,
}

/// What to delete, update, and insert so storage matches one scan.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TextSyncPlan {
    /// Document sections to remove, including their chunks and events.
    pub delete_objects: Vec<KoId>,
    /// Chunks to remove from sections that stay.
    pub delete_chunks: Vec<ChunkId>,
    /// Content hashes to write onto sections that stay.
    pub hash_updates: Vec<SectionHashUpdate>,
    /// Chunks to add onto sections that stay.
    pub insert_chunks: Vec<ChunkInsert>,
    /// Sections that are not stored yet, including their chunks.
    pub insert_sections: Vec<TextSection>,
}

/// Compares `scan` with `stored` document sections.
///
/// Non-UTF-8 paths keep their stored sections. A path that is no longer a
/// text file, or a section id the scan did not produce, is deleted. A
/// surviving section with a new body updates its hash and exchanges chunks.
pub fn plan_text_sync(scan: &TextScan, stored: &[StoredSection]) -> TextSyncPlan {
    let mut plan = TextSyncPlan::default();
    let mut kept = HashSet::new();

    for existing in stored {
        if scan
            .undecodable_paths
            .iter()
            .any(|path| path == &existing.relative_path)
        {
            kept.insert(existing.id);
            continue;
        }
        if !scan
            .present_paths
            .iter()
            .any(|path| path == &existing.relative_path)
        {
            plan.delete_objects.push(existing.id);
            continue;
        }
        let Some(section) = scan
            .sections
            .iter()
            .find(|section| section.id == existing.id)
        else {
            plan.delete_objects.push(existing.id);
            continue;
        };
        kept.insert(existing.id);
        if section.content_hash != existing.content_hash {
            plan.hash_updates.push(SectionHashUpdate {
                id: existing.id,
                content_hash: section.content_hash.clone(),
            });
        }
        for chunk_id in &existing.chunk_ids {
            if section.chunks.iter().all(|chunk| chunk.id != *chunk_id) {
                plan.delete_chunks.push(*chunk_id);
            }
        }
        for chunk in &section.chunks {
            if !existing.chunk_ids.contains(&chunk.id) {
                plan.insert_chunks.push(ChunkInsert {
                    ko_id: existing.id,
                    chunk: chunk.clone(),
                });
            }
        }
    }

    for section in &scan.sections {
        if kept.contains(&section.id) || stored.iter().any(|existing| existing.id == section.id) {
            continue;
        }
        plan.insert_sections.push(section.clone());
    }
    plan
}

#[cfg(test)]
mod tests {
    use super::{StoredSection, plan_text_sync};
    use crate::scan::{TextChunk, TextScan, TextSection};
    use mnemex_core::ids::{ChunkId, ContentHash, KoId};
    use mnemex_core::lifecycle::UnixMillis;

    fn hash(byte: u8) -> ContentHash {
        ContentHash::new(vec![byte; 32]).expect("hash")
    }

    fn section(id_byte: u8, path: &str, hash_byte: u8, chunk_byte: u8) -> TextSection {
        TextSection {
            id: KoId::new([id_byte; 16]),
            relative_path: path.to_string(),
            title: "Title".to_string(),
            heading_path: "Title".to_string(),
            content_hash: hash(hash_byte),
            created_at: UnixMillis::new(1),
            chunks: vec![TextChunk {
                id: ChunkId::new([chunk_byte; 16]),
                ordinal: 0,
                content_hash: hash(chunk_byte),
                body: "body".to_string(),
            }],
        }
    }

    fn stored_from(section: &TextSection) -> StoredSection {
        StoredSection {
            id: section.id,
            relative_path: section.relative_path.clone(),
            content_hash: section.content_hash.clone(),
            chunk_ids: section.chunks.iter().map(|chunk| chunk.id).collect(),
        }
    }

    fn scan_of(sections: Vec<TextSection>, present: &[&str], undecodable: &[&str]) -> TextScan {
        TextScan {
            sections,
            present_paths: present.iter().map(|path| (*path).to_string()).collect(),
            undecodable_paths: undecodable.iter().map(|path| (*path).to_string()).collect(),
        }
    }

    #[test]
    fn missing_path_deletes_the_object() {
        let old = section(1, "gone.md", 2, 3);
        let plan = plan_text_sync(&scan_of(vec![], &[], &[]), &[stored_from(&old)]);
        assert_eq!(plan.delete_objects, vec![old.id]);
        assert!(plan.delete_chunks.is_empty());
        assert!(plan.insert_sections.is_empty());
    }

    #[test]
    fn heading_change_deletes_the_old_id_and_inserts_the_new_section() {
        let old = section(1, "doc.md", 2, 3);
        let new = section(4, "doc.md", 5, 6);
        let plan = plan_text_sync(
            &scan_of(vec![new.clone()], &["doc.md"], &[]),
            &[stored_from(&old)],
        );
        assert_eq!(plan.delete_objects, vec![old.id]);
        assert_eq!(plan.insert_sections, vec![new]);
    }

    #[test]
    fn undecodable_path_keeps_the_stored_section() {
        let old = section(1, "bad.txt", 2, 3);
        let plan = plan_text_sync(
            &scan_of(vec![], &["bad.txt"], &["bad.txt"]),
            &[stored_from(&old)],
        );
        assert!(plan.delete_objects.is_empty());
        assert!(plan.delete_chunks.is_empty());
        assert!(plan.hash_updates.is_empty());
        assert!(plan.insert_chunks.is_empty());
        assert!(plan.insert_sections.is_empty());
    }

    #[test]
    fn present_path_without_a_section_deletes_the_object() {
        let old = section(1, "empty.txt", 2, 3);
        let plan = plan_text_sync(&scan_of(vec![], &["empty.txt"], &[]), &[stored_from(&old)]);
        assert_eq!(plan.delete_objects, vec![old.id]);
    }

    #[test]
    fn body_change_replaces_chunks_and_updates_the_hash() {
        let old = section(1, "doc.md", 2, 3);
        let new = section(1, "doc.md", 9, 8);
        let plan = plan_text_sync(
            &scan_of(vec![new.clone()], &["doc.md"], &[]),
            &[stored_from(&old)],
        );
        assert!(plan.delete_objects.is_empty());
        assert_eq!(plan.delete_chunks, vec![old.chunks[0].id]);
        assert_eq!(plan.hash_updates.len(), 1);
        assert_eq!(plan.hash_updates[0].id, old.id);
        assert_eq!(plan.hash_updates[0].content_hash, new.content_hash);
        assert_eq!(plan.insert_chunks.len(), 1);
        assert_eq!(plan.insert_chunks[0].ko_id, old.id);
        assert_eq!(plan.insert_chunks[0].chunk, new.chunks[0]);
        assert!(plan.insert_sections.is_empty());
    }

    #[test]
    fn unchanged_content_yields_an_empty_plan() {
        let current = section(1, "doc.md", 2, 3);
        let plan = plan_text_sync(
            &scan_of(vec![current.clone()], &["doc.md"], &[]),
            &[stored_from(&current)],
        );
        assert!(plan.delete_objects.is_empty());
        assert!(plan.delete_chunks.is_empty());
        assert!(plan.hash_updates.is_empty());
        assert!(plan.insert_chunks.is_empty());
        assert!(plan.insert_sections.is_empty());
    }
}
