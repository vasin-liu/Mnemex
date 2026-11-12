//! Text scan and chunking. Real temp directories, no mocked filesystem.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use mnemex_ingest::scan_text;

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "mnemex-scan-{tag}-{}-{nanos}-{seq}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("temp dir");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn write(dir: &Path, rel: &str, bytes: &[u8]) {
    let path = dir.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("parents");
    }
    std::fs::write(path, bytes).expect("write");
}

#[test]
fn scans_text_extensions_and_skips_the_rest() {
    let dir = TempDir::new("ext");
    write(dir.path(), "notes.md", b"# Title\nhello\n");
    write(dir.path(), "plain.txt", b"plain text\n");
    write(dir.path(), "extra.markdown", b"markdown body\n");
    write(dir.path(), "UPPER.MD", b"upper\n");
    write(dir.path(), "code.rs", b"fn main() {}\n");

    let sections = scan_text(dir.path()).expect("scan").sections;
    let paths: Vec<&str> = sections.iter().map(|s| s.relative_path.as_str()).collect();
    assert!(paths.contains(&"notes.md"));
    assert!(paths.contains(&"plain.txt"));
    assert!(paths.contains(&"extra.markdown"));
    assert!(paths.contains(&"UPPER.MD"));
    assert!(!paths.iter().any(|p| p.ends_with(".rs")));
}

#[test]
fn skips_denied_directories() {
    let dir = TempDir::new("deny");
    write(dir.path(), "keep.txt", b"keep me\n");
    write(dir.path(), ".git/secret.md", b"secret\n");
    write(dir.path(), "target/out.txt", b"build\n");
    write(dir.path(), "node_modules/pkg.md", b"dep\n");

    let sections = scan_text(dir.path()).expect("scan").sections;
    let paths: Vec<&str> = sections.iter().map(|s| s.relative_path.as_str()).collect();
    assert_eq!(paths, vec!["keep.txt"]);
}

#[test]
fn invalid_utf8_does_not_fail_the_scan() {
    let dir = TempDir::new("utf8");
    write(dir.path(), "bad.txt", &[0xff, 0xfe, 0x00]);
    write(dir.path(), "good.txt", b"good text\n");

    let sections = scan_text(dir.path()).expect("scan").sections;
    assert_eq!(sections.len(), 1);
    assert_eq!(sections[0].relative_path, "good.txt");
    assert_eq!(sections[0].chunks[0].body, "good text\n");
}

#[test]
fn atx_headings_build_a_slash_path_and_fences_hide_hashes() {
    let dir = TempDir::new("head");
    let md = "\
# Parent\n\
keep\n\
## Child\n\
child body\n\
```\n\
# not a heading\n\
```\n\
after\n";
    write(dir.path(), "doc.md", md.as_bytes());

    let sections = scan_text(dir.path()).expect("scan").sections;
    let parent = sections
        .iter()
        .find(|s| s.heading_path == "Parent")
        .expect("parent");
    assert_eq!(parent.title, "Parent");
    assert_eq!(parent.chunks.len(), 1);
    assert_eq!(parent.chunks[0].body, "keep\n");

    let child = sections
        .iter()
        .find(|s| s.heading_path == "Parent/Child")
        .expect("child");
    assert_eq!(child.title, "Child");
    let body: String = child.chunks.iter().map(|c| c.body.as_str()).collect();
    assert_eq!(body, "child body\n```\n# not a heading\n```\nafter\n");
    assert_eq!(sections.len(), 2);
}

#[test]
fn long_body_splits_on_the_scalar_cap_and_on_newlines() {
    let dir = TempDir::new("split");
    let hard: String = "a".repeat(600);
    let mut wrapped = "b".repeat(400);
    wrapped.push('\n');
    wrapped.push_str(&"c".repeat(200));
    write(dir.path(), "hard.txt", hard.as_bytes());
    write(dir.path(), "wrapped.txt", wrapped.as_bytes());

    let sections = scan_text(dir.path()).expect("scan").sections;
    let hard_section = sections
        .iter()
        .find(|s| s.relative_path == "hard.txt")
        .expect("hard");
    assert!(hard_section.chunks.len() > 1);
    assert!(
        hard_section
            .chunks
            .iter()
            .all(|c| c.body.chars().count() <= 512)
    );
    let joined: String = hard_section
        .chunks
        .iter()
        .map(|c| c.body.as_str())
        .collect();
    assert_eq!(joined, hard);
    assert_eq!(hard_section.chunks[0].body.chars().count(), 512);

    let wrapped_section = sections
        .iter()
        .find(|s| s.relative_path == "wrapped.txt")
        .expect("wrapped");
    let joined: String = wrapped_section
        .chunks
        .iter()
        .map(|c| c.body.as_str())
        .collect();
    assert_eq!(joined, wrapped);
    assert_eq!(
        wrapped_section.chunks[0].body,
        format!("{}\n", "b".repeat(400))
    );
}

#[test]
fn empty_file_produces_no_chunk() {
    let dir = TempDir::new("empty");
    write(dir.path(), "empty.txt", b"");
    write(dir.path(), "full.txt", b"full\n");

    let sections = scan_text(dir.path()).expect("scan").sections;
    assert!(sections.iter().all(|s| s.relative_path != "empty.txt"));
    assert!(
        sections
            .iter()
            .all(|s| s.chunks.iter().all(|c| !c.body.is_empty()))
    );
    assert_eq!(sections.len(), 1);
}

#[test]
fn report_lists_empty_paths_and_undecodable_files() {
    let dir = TempDir::new("report");
    write(dir.path(), "empty.txt", b"");
    write(dir.path(), "sub/blank.txt", b"");
    write(dir.path(), "bad.txt", &[0xff, 0xfe, 0xfd]);
    write(dir.path(), "notes.md", b"# Title\nhello\n");

    let report = scan_text(dir.path()).expect("scan");
    assert!(report.present_paths.iter().any(|p| p == "empty.txt"));
    assert!(report.present_paths.iter().any(|p| p == "sub/blank.txt"));
    assert!(report.present_paths.iter().any(|p| p == "bad.txt"));
    assert!(report.present_paths.iter().any(|p| p == "notes.md"));
    assert_eq!(report.undecodable_paths, vec!["bad.txt".to_string()]);
    assert!(
        report
            .sections
            .iter()
            .all(|section| section.relative_path != "empty.txt")
    );
    assert!(
        report
            .sections
            .iter()
            .any(|section| section.relative_path == "notes.md")
    );
}

#[test]
fn chunk_hashes_are_sha256_and_differ_when_bodies_differ() {
    let dir = TempDir::new("hash");
    write(dir.path(), "abc.txt", b"abc");
    write(dir.path(), "abd.txt", b"abd");

    let sections = scan_text(dir.path()).expect("scan").sections;
    let abc = sections
        .iter()
        .find(|s| s.relative_path == "abc.txt")
        .expect("abc");
    let abd = sections
        .iter()
        .find(|s| s.relative_path == "abd.txt")
        .expect("abd");
    let abc_hash = abc.chunks[0].content_hash.as_bytes();
    let abd_hash = abd.chunks[0].content_hash.as_bytes();
    assert_eq!(abc_hash.len(), 32);
    assert_eq!(abd_hash.len(), 32);
    assert_ne!(abc_hash, abd_hash);
    assert_eq!(
        abc_hash,
        [
            0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae,
            0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61,
            0xf2, 0x00, 0x15, 0xad,
        ]
    );
}
