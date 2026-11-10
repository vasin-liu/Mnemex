//! Mnemex CLI — Phase 0 index entry point.
//!
//! Argument handling is std-only. This binary composes the text scan with
//! SQLite; it does not open the vector store.

use std::path::PathBuf;
use std::process::ExitCode;

use mnemex_core::event::{EventSource, KnowledgeEvent};
use mnemex_core::ids::SourceRefs;
use mnemex_core::ko::{DocumentSection, KoCore};
use mnemex_core::lifecycle::Lifecycle;
use mnemex_ingest::{TextSection, scan_text};
use mnemex_store::schema::StoredKo;
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
    let sections = match scan_text(&dir) {
        Ok(sections) => sections,
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
    if let Err(err) = write_sections(&store, &sections) {
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

fn write_sections(store: &SqliteStore, sections: &[TextSection]) -> Result<(), StoreError> {
    for section in sections {
        if store.read_ko(&section.id)?.is_none() {
            let object = StoredKo::DocumentSection(DocumentSection {
                core: KoCore {
                    id: section.id,
                    refs: SourceRefs {
                        content_hash: Some(section.content_hash.clone()),
                        commit: None,
                        path: Some(PathBuf::from(&section.relative_path)),
                    },
                    lifecycle: Lifecycle::default(),
                    created_at: section.created_at,
                },
                title: section.title.clone(),
                heading_path: section.heading_path.clone(),
            });
            store.insert_ko(&object)?;
            store.append_event(&KnowledgeEvent::created(
                section.id,
                section.created_at,
                EventSource::Rule,
            ))?;
        }
        for chunk in &section.chunks {
            if store.chunk_pending(&chunk.id)?.is_none() {
                store.insert_chunk(&chunk.id, &section.id, &chunk.content_hash, &chunk.body)?;
            }
        }
    }
    Ok(())
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
