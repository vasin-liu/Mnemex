//! Mnemex CLI — Phase 0 index entry point.
//!
//! Argument handling is std-only. This binary composes the text scan with
//! SQLite; it does not open the vector store.

use std::path::PathBuf;
use std::process::ExitCode;

use mnemex_core::ids::SourceRefs;
use mnemex_core::ko::{DocumentSection, KoCore};
use mnemex_core::lifecycle::Lifecycle;
use mnemex_ingest::{StoredSection, TextScan, TextSection, plan_text_sync, scan_text};
use mnemex_store::schema::{DocumentSync, NewDocumentSection, StoredKo, SyncChunk};
use mnemex_store::sqlite::{SqliteStore, StoreError};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None | Some("-h") | Some("--help") => {
            print_help();
            ExitCode::SUCCESS
        }
        Some("-V") | Some("--version") => {
            println!("mnemex {VERSION}");
            ExitCode::SUCCESS
        }
        Some("index") => index(&args[1..]),
        Some(cmd) => {
            eprintln!("mnemex: unknown command `{cmd}`");
            print_help();
            ExitCode::FAILURE
        }
    }
}

fn index(args: &[String]) -> ExitCode {
    let (dir, db) = match parse_index(args) {
        Ok(paths) => paths,
        Err(message) => {
            eprintln!("mnemex: {message}");
            return ExitCode::FAILURE;
        }
    };
    let report = match scan_text(&dir) {
        Ok(report) => report,
        Err(err) => {
            eprintln!("mnemex: {err}");
            return ExitCode::FAILURE;
        }
    };
    let store = match SqliteStore::open(&db) {
        Ok(store) => store,
        Err(err) => {
            eprintln!("mnemex: {err}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(err) = sync_sections(&store, &report) {
        eprintln!("mnemex: {err}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn parse_index(args: &[String]) -> Result<(PathBuf, PathBuf), String> {
    let mut dir = None;
    let mut db = None;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--dir" => {
                let path = rest
                    .next()
                    .ok_or_else(|| "--dir needs a path".to_string())?;
                dir = Some(PathBuf::from(path));
            }
            "--db" => {
                let path = rest.next().ok_or_else(|| "--db needs a path".to_string())?;
                db = Some(PathBuf::from(path));
            }
            other => return Err(format!("unknown argument `{other}`")),
        }
    }
    match (dir, db) {
        (Some(dir), Some(db)) => Ok((dir, db)),
        (None, _) => Err("index requires --dir".to_string()),
        (Some(_), None) => Err("index requires --db".to_string()),
    }
}

fn sync_sections(store: &SqliteStore, report: &TextScan) -> Result<(), StoreError> {
    store.sync_documents(|stored| {
        let stored_sections: Vec<StoredSection> = stored
            .iter()
            .map(|section| StoredSection {
                id: section.id,
                relative_path: section.path.clone(),
                content_hash: section.content_hash.clone(),
                chunk_ids: section.chunk_ids.clone(),
            })
            .collect();
        let plan = plan_text_sync(report, &stored_sections);
        DocumentSync {
            delete_objects: plan.delete_objects,
            delete_chunks: plan.delete_chunks,
            hash_updates: plan
                .hash_updates
                .into_iter()
                .map(|update| (update.id, update.content_hash))
                .collect(),
            insert_chunks: plan
                .insert_chunks
                .into_iter()
                .map(|insert| {
                    (
                        insert.ko_id,
                        SyncChunk {
                            id: insert.chunk.id,
                            content_hash: insert.chunk.content_hash,
                            body: insert.chunk.body,
                        },
                    )
                })
                .collect(),
            insert_sections: plan.insert_sections.into_iter().map(new_section).collect(),
        }
    })
}

fn new_section(section: TextSection) -> NewDocumentSection {
    let chunks = section
        .chunks
        .into_iter()
        .map(|chunk| SyncChunk {
            id: chunk.id,
            content_hash: chunk.content_hash,
            body: chunk.body,
        })
        .collect();
    NewDocumentSection {
        object: StoredKo::DocumentSection(DocumentSection {
            core: KoCore {
                id: section.id,
                refs: SourceRefs {
                    content_hash: Some(section.content_hash),
                    commit: None,
                    path: Some(PathBuf::from(section.relative_path)),
                },
                lifecycle: Lifecycle::default(),
                created_at: section.created_at,
            },
            title: section.title,
            heading_path: section.heading_path,
        }),
        chunks,
    }
}

fn print_help() {
    println!(
        "mnemex {VERSION} — local engineering memory engine\n\
         \n\
         Usage:\n\
         \x20 mnemex [--version | --help]\n\
         \x20 mnemex index --dir <directory> --db <database>\n\
         \n\
         Scope: the workspace path (plus optional branch/commit) that callers\n\
         pass explicitly on every request. Mnemex never mints scope handles;\n\
         server-minted handles are reserved for real server state only\n\
         (index snapshots, pagination cursors). See docs/adr/ADR-006."
    );
}
