//! Split document text into sections and scalar-capped chunks.
//!
//! ATX headings (`#` through `######` plus a space) start a section.
//! A fenced code block toggled by a line that starts with ``` hides
//! hashes inside it. The 512-scalar cap is the ADR-003 stand-in for a
//! tokenizer: a newline inside the window ends the chunk after that
//! newline, otherwise the chunk is cut at the 512th scalar.

/// Maximum Unicode scalar values in one chunk.
pub(crate) const CHUNK_SCALAR_CAP: usize = 512;

/// One heading's text before it is split into chunks.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct DraftSection {
    /// Section title. The file stem when the file has no heading.
    pub title: String,
    /// Heading titles joined with `/`.
    pub heading_path: String,
    /// Body under the heading, excluding the heading line.
    pub body: String,
}

/// Splits `text` into sections. An empty string yields no sections.
pub(crate) fn sections_from_text(file_stem: &str, text: &str) -> Vec<DraftSection> {
    if text.is_empty() {
        return Vec::new();
    }

    let mut out = Vec::new();
    let mut stack: Vec<(u8, String)> = Vec::new();
    let mut title = file_stem.to_string();
    let mut heading_path = file_stem.to_string();
    let mut body = String::new();
    let mut in_fence = false;

    for line in text.split_inclusive('\n') {
        let content = line.trim_end_matches(['\r', '\n']);
        if is_fence(content) {
            in_fence = !in_fence;
            body.push_str(line);
            continue;
        }
        if !in_fence
            && let Some((level, heading)) = parse_atx(content)
        {
            push_section(&mut out, &title, &heading_path, &body);
            body.clear();
            while stack.last().is_some_and(|(lvl, _)| *lvl >= level) {
                stack.pop();
            }
            stack.push((level, heading.clone()));
            title = heading;
            heading_path = stack
                .iter()
                .map(|(_, name)| name.as_str())
                .collect::<Vec<_>>()
                .join("/");
            continue;
        }
        body.push_str(line);
    }
    push_section(&mut out, &title, &heading_path, &body);
    out
}

/// Splits `body` into chunks of at most [`CHUNK_SCALAR_CAP`] scalars.
/// Concatenating the chunks reproduces `body`.
pub(crate) fn split_body(body: &str) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut rest = body;
    while !rest.is_empty() {
        if rest.chars().count() <= CHUNK_SCALAR_CAP {
            chunks.push(rest.to_string());
            break;
        }
        let mut end = 0;
        let mut last_newline = None;
        for (seen, (idx, ch)) in rest.char_indices().enumerate() {
            if seen == CHUNK_SCALAR_CAP {
                break;
            }
            end = idx + ch.len_utf8();
            if ch == '\n' {
                last_newline = Some(end);
            }
        }
        let cut = last_newline.unwrap_or(end);
        if cut == 0 {
            break;
        }
        chunks.push(rest[..cut].to_string());
        rest = &rest[cut..];
    }
    chunks
}

fn push_section(out: &mut Vec<DraftSection>, title: &str, heading_path: &str, body: &str) {
    if body.is_empty() {
        return;
    }
    out.push(DraftSection {
        title: title.to_string(),
        heading_path: heading_path.to_string(),
        body: body.to_string(),
    });
}

fn is_fence(content: &str) -> bool {
    content.trim_start().starts_with("```")
}

fn parse_atx(content: &str) -> Option<(u8, String)> {
    let rest = content.trim_end();
    let bytes = rest.as_bytes();
    let mut level = 0u8;
    while (level as usize) < bytes.len() && level < 6 && bytes[level as usize] == b'#' {
        level += 1;
    }
    if level == 0 {
        return None;
    }
    let title = rest[level as usize..].strip_prefix(' ')?.trim();
    if title.is_empty() {
        return None;
    }
    Some((level, title.to_string()))
}
