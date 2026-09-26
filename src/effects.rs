//! Extended notes: `docs/internals/effects.md`

#![cfg_attr(
    not(test),
    forbid(
        clippy::disallowed_methods,
        clippy::disallowed_types,
        clippy::disallowed_macros
    )
)]

use std::collections::{BTreeMap, BTreeSet};

pub const CLIPPY_TOML: &str = "clippy.toml";

pub const ALLOWLIST_TOML: &str = "effects/allowlist.toml";

pub const WRAPPERS_TOML: &str = "effects/wrappers.toml";

pub const EFFECT_SITES_JSON: &str = "effect_sites.json";

pub const RESIDUE_CLASSES_JSON: &str = "effects/residue-classes.json";

pub const RESIDUE_HISTOGRAM_JSON: &str = "effects/residue-histogram.json";
pub const RESIDUE_SYNTHETIC_JSON: &str = "effects/residue-synthetic.json";
pub const SEQUENTIAL_RESIDUE_HISTOGRAM_JSON: &str = "residue-histogram-sequential.json";

pub const FUNNEL_MODULES_JSON: &str = "effects/funnel-modules.json";

pub const REGENERATE: &str = "UPSTROKE_REGENERATE_EFFECT_ARTIFACTS";

pub const GOVERNED_LINTS: &[&str] = &[
    "disallowed_methods",
    "disallowed_types",
    "disallowed_macros",
    "style",
    "all",
    "warnings",
];

pub const USED_GOVERNED_LINTS: &[&str] = &[
    "clippy::disallowed_methods",
    "clippy::disallowed_types",
    "clippy::disallowed_macros",
];

#[must_use]
pub fn normalize_lint(entry: &str) -> Option<&'static str> {
    let segments: Vec<&str> = entry
        .split("::")
        .map(|segment| {
            let segment = segment.trim();
            segment.strip_prefix("r#").unwrap_or(segment)
        })
        .collect();
    let named = match segments.as_slice() {
        ["clippy", name] => RENAMED_TO_A_GOVERNED_LINT
            .iter()
            .find(|(old, _)| old == name)
            .map_or(*name, |(_, new)| *new),
        [name] if PREFIXLESS_GROUP_ALIASES.contains(name) => {
            name.strip_prefix("clippy_").unwrap_or(*name)
        }
        [.., name] => *name,
        [] => return None,
    };
    GOVERNED_LINTS
        .iter()
        .copied()
        .find(|governed| *governed == named)
}

const PREFIXLESS_GROUP_ALIASES: [&str; 2] = ["clippy_all", "clippy_style"];

const RENAMED_TO_A_GOVERNED_LINT: [(&str, &str); 2] = [
    ("disallowed_method", "disallowed_methods"),
    ("disallowed_type", "disallowed_types"),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernedAllow {
    pub line: usize,
    pub inner: bool,
    pub module_level: bool,
    pub lints: Vec<String>,
    pub written: Vec<String>,
    pub keywords: Vec<&'static str>,
    pub reasoned: bool,
}

pub const RUSTC_WHITESPACE: [char; 11] = [
    '\u{0009}', '\u{000A}', '\u{000B}', '\u{000C}', '\u{000D}', '\u{0020}', '\u{0085}', '\u{200E}',
    '\u{200F}', '\u{2028}', '\u{2029}',
];

#[must_use]
pub fn is_rustc_whitespace(character: char) -> bool {
    RUSTC_WHITESPACE.contains(&character)
}

#[must_use]
pub fn blank_comments(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                let mut depth = 1usize;
                i += 2;
                while i < bytes.len() && depth > 0 {
                    if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'*') {
                        depth += 1;
                        i += 2;
                    } else if bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/') {
                        depth -= 1;
                        i += 2;
                    } else {
                        if bytes[i] == b'\n' {
                            out.push(b'\n');
                        }
                        i += 1;
                    }
                }
            }
            b'r' | b'b' if i == 0 || !is_ident_byte(bytes[i - 1]) => match literal_end(bytes, i) {
                Some(end) => {
                    out.extend_from_slice(&bytes[i..end]);
                    i = end;
                }
                None => {
                    out.push(bytes[i]);
                    i += 1;
                }
            },
            b'"' => {
                let end = literal_end(bytes, i).unwrap_or(bytes.len());
                out.extend_from_slice(&bytes[i..end]);
                i = end;
            }
            b'\'' => match char_literal_end(bytes, i) {
                Some(end) => {
                    out.extend_from_slice(&bytes[i..end]);
                    i = end;
                }
                None => {
                    out.push(bytes[i]);
                    i += 1;
                }
            },
            byte => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn char_literal_end(bytes: &[u8], from: usize) -> Option<usize> {
    if bytes.get(from) != Some(&b'\'') {
        return None;
    }
    let mut at = from + 1;
    if bytes.get(at) == Some(&b'\\') {
        at += 2;
        loop {
            match *bytes.get(at)? {
                b'\'' => return Some(at + 1),
                b'\n' => return None,
                b'\\' => at += 2,
                _ => at += 1,
            }
        }
    }
    let width = match *bytes.get(at)? {
        0x00..=0x7F => 1,
        0xC2..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xF4 => 4,
        _ => return None,
    };
    at += width;
    (bytes.get(at) == Some(&b'\'')).then_some(at + 1)
}

fn literal_end(bytes: &[u8], from: usize) -> Option<usize> {
    let mut j = from;
    if bytes.get(j) == Some(&b'b') {
        j += 1;
    }
    let raw = bytes.get(j) == Some(&b'r');
    if raw {
        j += 1;
    }
    let hash_start = j;
    while bytes.get(j) == Some(&b'#') {
        j += 1;
    }
    let hashes = j - hash_start;
    if bytes.get(j) != Some(&b'"') || (!raw && hashes > 0) {
        return None;
    }
    j += 1;
    if raw {
        let close: Vec<u8> = std::iter::once(b'"')
            .chain(std::iter::repeat_n(b'#', hashes))
            .collect();
        while j < bytes.len() && !bytes[j..].starts_with(&close) {
            j += 1;
        }
        return Some((j + close.len()).min(bytes.len()));
    }
    while j < bytes.len() && bytes[j] != b'"' {
        j += if bytes[j] == b'\\' { 2 } else { 1 };
    }
    Some((j + 1).min(bytes.len()))
}

#[must_use]
pub fn blank_comments_and_strings(source: &str) -> String {
    let code = code_bytes_only(source);
    let unread =
        |character: char| is_rustc_whitespace(character) && !character.is_ascii_whitespace();
    if !code.contains(unread) {
        return code;
    }
    let mut spaced = String::with_capacity(code.len());
    for character in code.chars() {
        if unread(character) {
            spaced.extend(std::iter::repeat_n(' ', character.len_utf8()));
        } else {
            spaced.push(character);
        }
    }
    spaced
}

#[allow(clippy::too_many_lines)]
fn code_bytes_only(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut out = vec![b' '; bytes.len()];
    for (index, byte) in bytes.iter().enumerate() {
        if *byte == b'\n' {
            out[index] = b'\n';
        }
    }
    let keep = |out: &mut Vec<u8>, from: usize, to: usize| {
        out[from..to].copy_from_slice(&bytes[from..to]);
    };

    let mut i = 0;
    let mut code_start = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'/' => {
                keep(&mut out, code_start, i);
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
                code_start = i;
            }
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'*' => {
                keep(&mut out, code_start, i);
                let mut depth = 1;
                i += 2;
                while i < bytes.len() && depth > 0 {
                    if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
                        depth += 1;
                        i += 2;
                    } else if bytes[i] == b'*' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
                        depth -= 1;
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                code_start = i;
            }
            b'r' | b'b' => {
                let mut j = i;
                if bytes[j] == b'b' {
                    j += 1;
                }
                let raw = j < bytes.len() && bytes[j] == b'r';
                if raw {
                    j += 1;
                }
                let hash_start = j;
                while j < bytes.len() && bytes[j] == b'#' {
                    j += 1;
                }
                let hashes = j - hash_start;
                if j < bytes.len() && bytes[j] == b'"' && (raw || hashes == 0) {
                    keep(&mut out, code_start, i);
                    j += 1;
                    if raw {
                        let close: Vec<u8> = std::iter::once(b'"')
                            .chain(std::iter::repeat_n(b'#', hashes))
                            .collect();
                        while j < bytes.len() && !bytes[j..].starts_with(&close) {
                            j += 1;
                        }
                        j = (j + close.len()).min(bytes.len());
                    } else {
                        while j < bytes.len() && bytes[j] != b'"' {
                            j += if bytes[j] == b'\\' { 2 } else { 1 };
                        }
                        j = (j + 1).min(bytes.len());
                    }
                    i = j;
                    code_start = i;
                } else {
                    i += 1;
                }
            }
            b'"' => {
                keep(&mut out, code_start, i);
                i += 1;
                while i < bytes.len() && bytes[i] != b'"' {
                    i += if bytes[i] == b'\\' { 2 } else { 1 };
                }
                i = (i + 1).min(bytes.len());
                code_start = i;
            }
            b'\'' => match char_literal_end(bytes, i) {
                Some(end) => {
                    keep(&mut out, code_start, i);
                    i = end;
                    code_start = i;
                }
                None => i += 1,
            },
            _ => i += 1,
        }
    }
    keep(&mut out, code_start, bytes.len());
    String::from_utf8_lossy(&out).into_owned()
}

#[must_use]
pub fn production_region(source: &str) -> String {
    let blanked = blank_comments_and_strings(source);
    match blanked.find("#[cfg(test)]") {
        Some(cut) => source[..cut].to_owned(),
        None => source.to_owned(),
    }
}

#[must_use]
pub fn production_code(source: &str) -> String {
    const ATTR: &[u8] = b"#[cfg(test)]";
    let blanked = blank_comments_and_strings(source);
    let bytes = blanked.as_bytes();
    let mut out = bytes.to_vec();
    let mut from = 0;
    while let Some(at) = bytes
        .get(from..)
        .and_then(|rest| rest.windows(ATTR.len()).position(|at| at == ATTR))
        .map(|found| from + found)
    {
        let mut start = at + ATTR.len();
        loop {
            while bytes.get(start).is_some_and(u8::is_ascii_whitespace) {
                start += 1;
            }
            if bytes.get(start) == Some(&b'#') {
                let open = if bytes.get(start + 1) == Some(&b'!') {
                    start + 2
                } else {
                    start + 1
                };
                if bytes.get(open) == Some(&b'[') {
                    if let Some(close) = matching(bytes, open, b'[', b']') {
                        start = close + 1;
                        continue;
                    }
                }
            }
            break;
        }
        let end = configured_item_end(bytes, start);
        for byte in &mut out[at..end] {
            if *byte != b'\n' {
                *byte = b' ';
            }
        }
        from = end.max(at + ATTR.len());
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn configured_item_end(bytes: &[u8], start: usize) -> usize {
    let return_start = configured_function_return_start(bytes, start);
    let mut depth = 0usize;
    let mut index = start;
    while let Some(&byte) = bytes.get(index) {
        if depth == 0
            && return_start.is_some_and(|at| index >= at)
            && starts_named_function_item(bytes, index)
        {
            return start;
        }
        match byte {
            b'{' if depth == 0 => {
                let Some(close) = matching(bytes, index, b'{', b'}') else {
                    return start;
                };
                let mut after = close + 1;
                while bytes.get(after).is_some_and(u8::is_ascii_whitespace) {
                    after += 1;
                }
                return if bytes.get(after) == Some(&b';') {
                    after + 1
                } else {
                    close + 1
                };
            }
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' if depth == 0 => return index,
            b')' | b']' | b'}' => depth -= 1,
            b';' if depth == 0 => return index + 1,
            b',' if depth == 0 && return_start.is_none_or(|at| index < at) => return index + 1,
            _ => {}
        }
        index += 1;
    }
    start
}

fn starts_named_function_item(bytes: &[u8], at: usize) -> bool {
    if at
        .checked_sub(1)
        .and_then(|before| bytes.get(before))
        .is_some_and(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'#'))
    {
        return false;
    }
    let Some(rest) = bytes.get(at..).and_then(|rest| rest.strip_prefix(b"fn")) else {
        return false;
    };
    rest.first().is_some_and(u8::is_ascii_whitespace)
        && rest
            .iter()
            .find(|byte| !byte.is_ascii_whitespace())
            .is_some_and(|byte| byte.is_ascii_alphabetic() || *byte == b'_')
}

fn configured_function_return_start(bytes: &[u8], start: usize) -> Option<usize> {
    let mut cursor = start;
    loop {
        while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
            cursor += 1;
        }
        let rest = bytes.get(cursor..)?;
        let length = rest
            .iter()
            .take_while(|byte| byte.is_ascii_alphanumeric() || **byte == b'_')
            .count();
        let word = rest.get(..length)?;
        cursor += length;
        match word {
            b"fn" => break,
            b"pub" => {
                while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
                    cursor += 1;
                }
                if bytes.get(cursor) == Some(&b'(') {
                    let close = matching(bytes, cursor, b'(', b')')?;
                    cursor = close + 1;
                }
            }
            b"async" | b"const" | b"unsafe" | b"extern" => {}
            _ => return None,
        }
    }
    while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
        cursor += 1;
    }
    let name_start = cursor;
    while bytes
        .get(cursor)
        .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
    {
        cursor += 1;
    }
    if cursor == name_start {
        return None;
    }
    while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
        cursor += 1;
    }
    if bytes.get(cursor) != Some(&b'(') {
        return None;
    }
    let close = matching(bytes, cursor, b'(', b')')?;
    cursor = close + 1;
    while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
        cursor += 1;
    }
    bytes
        .get(cursor..)
        .is_some_and(|rest| rest.starts_with(b"->"))
        .then_some(cursor + 2)
}

#[must_use]
pub fn governed_allows(source: &str) -> Vec<GovernedAllow> {
    let blanked = blank_comments_and_strings(source);
    let bytes = blanked.as_bytes();
    let mut found = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let Some((inner, open)) = attribute_open(source, bytes, i) else {
            i += 1;
            continue;
        };
        let Some(close) = matching(bytes, open, b'[', b']') else {
            i += 1;
            continue;
        };
        let within = bytes.get(..close).unwrap_or_default();
        let mut lints = Vec::new();
        let mut written = Vec::new();
        let mut keywords: Vec<&'static str> = Vec::new();
        let mut reasoned = false;
        for keyword in ["allow", "expect"] {
            let attribute = blanked.get(open + 1..close).unwrap_or_default();
            for (at, _) in attribute.match_indices(keyword) {
                let start = open + 1 + at;
                let is_word_start = start
                    .checked_sub(1)
                    .and_then(|before| bytes.get(before))
                    .is_none_or(|byte| !is_ident_byte(*byte));
                let paren = past_comments_and_whitespace(source, start + keyword.len(), false);
                if !is_word_start || within.get(paren) != Some(&b'(') {
                    continue;
                }
                let Some(end) = matching(within, paren, b'(', b')') else {
                    continue;
                };
                let before = lints.len();
                for entry in blanked.get(paren + 1..end).unwrap_or_default().split(',') {
                    let entry = entry.trim();
                    if entry.is_empty() {
                        continue;
                    }
                    if entry.starts_with("reason") {
                        reasoned = true;
                        continue;
                    }
                    written.push(entry.split_ascii_whitespace().collect());
                    if let Some(name) = normalize_lint(entry) {
                        lints.push(name.to_owned());
                    }
                }
                if lints.len() > before && !keywords.contains(&keyword) {
                    keywords.push(keyword);
                }
            }
        }
        if !lints.is_empty() {
            found.push(GovernedAllow {
                line: blanked
                    .get(..i)
                    .map_or(0, |before| before.matches('\n').count())
                    + 1,
                inner,
                module_level: is_module_level(source, bytes, i, close, inner),
                lints,
                written,
                keywords,
                reasoned,
            });
        }
        i = close + 1;
    }
    found
}

fn attribute_open(source: &str, blanked: &[u8], hash: usize) -> Option<(bool, usize)> {
    if blanked.get(hash) != Some(&b'#') {
        return None;
    }
    let bytes = source.as_bytes();
    let after_hash = past_comments_and_whitespace(source, hash + 1, false);
    let inner = bytes.get(after_hash) == Some(&b'!');
    let open = if inner {
        past_comments_and_whitespace(source, after_hash + 1, false)
    } else {
        after_hash
    };
    (bytes.get(open) == Some(&b'[')).then_some((inner, open))
}

fn past_whitespace(bytes: &[u8], from: usize) -> usize {
    let mut at = from;
    while bytes.get(at).is_some_and(u8::is_ascii_whitespace) {
        at += 1;
    }
    at
}

fn past_comments_and_whitespace(source: &str, from: usize, doc_comments_too: bool) -> usize {
    let bytes = source.as_bytes();
    let mut at = from;
    while let Some(rest) = source.get(at..) {
        let skipped = doc_comments_too || !is_doc_comment(bytes, at);
        if let Some(character) = rest
            .chars()
            .next()
            .filter(|character| is_rustc_whitespace(*character))
        {
            at += character.len_utf8();
        } else if rest.starts_with("/*") && skipped {
            at = block_comment_end(bytes, at);
        } else if rest.starts_with("//") && skipped {
            at += rest.find('\n').unwrap_or(rest.len());
        } else {
            break;
        }
    }
    at
}

pub(crate) fn is_doc_comment(bytes: &[u8], at: usize) -> bool {
    if bytes.get(at) != Some(&b'/') {
        return false;
    }
    match (bytes.get(at + 1), bytes.get(at + 2), bytes.get(at + 3)) {
        (Some(b'/' | b'*'), Some(b'!'), _) => true,
        (Some(b'/'), Some(b'/'), next) => next != Some(&b'/'),
        (Some(b'*'), Some(b'*'), next) => !matches!(next, Some(b'*' | b'/')),
        _ => false,
    }
}

pub(crate) fn block_comment_end(bytes: &[u8], from: usize) -> usize {
    let mut depth = 1_usize;
    let mut at = from + 2;
    while depth > 0 {
        match (bytes.get(at), bytes.get(at + 1)) {
            (None, _) => break,
            (Some(b'/'), Some(b'*')) => {
                depth += 1;
                at += 2;
            }
            (Some(b'*'), Some(b'/')) => {
                depth -= 1;
                at += 2;
            }
            _ => at += 1,
        }
    }
    at.min(bytes.len())
}

fn matching(bytes: &[u8], open: usize, opener: u8, closer: u8) -> Option<usize> {
    let mut depth = 0usize;
    for (index, byte) in bytes.iter().enumerate().skip(open) {
        if *byte == opener {
            depth += 1;
        } else if *byte == closer {
            depth -= 1;
            if depth == 0 {
                return Some(index);
            }
        }
    }
    None
}

fn is_module_level(source: &str, bytes: &[u8], hash: usize, close: usize, inner: bool) -> bool {
    let attribute_end = |at: usize| {
        attribute_open(source, bytes, at).and_then(|(_, open)| matching(bytes, open, b'[', b']'))
    };
    if inner {
        let mut at = past_whitespace(bytes, 0);
        while at < hash {
            let Some(end) = attribute_end(at) else {
                return false;
            };
            at = past_whitespace(bytes, end + 1);
        }
        return at == hash;
    }
    let mut at = past_whitespace(bytes, close + 1);
    while let Some(end) = attribute_end(at) {
        at = past_whitespace(bytes, end + 1);
    }
    if word_at(bytes, at, b"pub") {
        at = past_whitespace(bytes, at + b"pub".len());
        if bytes.get(at) == Some(&b'(') {
            let Some(end) = matching(bytes, at, b'(', b')') else {
                return false;
            };
            at = past_whitespace(bytes, end + 1);
        }
    }
    word_at(bytes, at, b"mod")
}

fn word_at(bytes: &[u8], at: usize, word: &[u8]) -> bool {
    bytes.get(at..).is_some_and(|rest| rest.starts_with(word))
        && !bytes
            .get(at + word.len())
            .is_some_and(|byte| is_ident_byte(*byte))
}

pub const FROZEN_LEGACY_ALLOWLIST: &[&str] = &[
    "src/engine/coordinator.rs",
    "src/engine/resume.rs",
    "src/engine/attempt.rs",
    "src/engine/preflight.rs",
    "src/workspace.rs",
    "src/gates.rs",
    "src/review.rs",
    "src/agent/claude.rs",
    "src/agent/codex.rs",
    "src/agent/copilot.rs",
    "src/agent/bin.rs",
    "src/capacity.rs",
    "src/export.rs",
    "src/main.rs",
    "src/answer.rs",
    "src/config.rs",
    "src/connect.rs",
    "src/route.rs",
    "src/status.rs",
    "src/validate.rs",
    "src/events/mod.rs",
    "src/events/log/premove.rs",
    "src/engine/tests.rs",
    "examples/probe.rs",
];

pub const TOPOLOGY_MODULES: &[&str] = &[
    "src/topology/",
    "src/runner/",
    "src/workspace_manager.rs",
    "src/workspace_manager/",
    "src/engine/topology.rs",
    "src/engine/topology/",
];

#[must_use]
pub fn legacy_growth<'a>(frozen: &[&str], current: &[&'a str]) -> Vec<&'a str> {
    let frozen: BTreeSet<&str> = frozen.iter().copied().collect();
    current
        .iter()
        .copied()
        .filter(|path| !frozen.contains(path))
        .collect()
}

#[must_use]
pub fn topology_modules_among<'a>(paths: &[&'a str]) -> Vec<&'a str> {
    paths
        .iter()
        .copied()
        .filter(|path| {
            TOPOLOGY_MODULES
                .iter()
                .any(|banned| path.starts_with(banned) || *path == *banned)
        })
        .collect()
}

pub const CLASSIFIED_MODULES: &[&str] = &[
    "src/workspace_manager.rs",
    "src/workspace_manager/containment.rs",
    "src/workspace_manager/hooks.rs",
    "src/workspace_manager/naming.rs",
    "src/workspace_manager/object.rs",
    "src/workspace_manager/parsers.rs",
    "src/workspace_manager/residue.rs",
    "src/workspace_manager/snapshot_ref.rs",
    "src/workspace_manager/worktree.rs",
    "src/rundir.rs",
    "src/rundir/classify.rs",
    "src/rundir/discovery.rs",
    "src/rundir/names.rs",
    "src/rundir/ownership.rs",
    "src/rundir/retention.rs",
    "src/interaction.rs",
    "src/util.rs",
    "src/events/log.rs",
    "src/runner/host.rs",
    "src/runner/host/environment.rs",
    "src/runner/host/naming.rs",
    "src/runner/host/probe.rs",
    "src/runner/invocation.rs",
    "src/runner/container.rs",
    "src/runner/container/view.rs",
    "src/engine/coordinator.rs",
    "src/engine/resume.rs",
    "src/engine/attempt.rs",
    "src/engine/preflight.rs",
    "src/workspace.rs",
    "src/gates.rs",
    "src/review.rs",
    "src/agent/proc.rs",
    "src/agent/proc/ambient.rs",
    "src/agent/proc/drain.rs",
    "src/agent/proc/input.rs",
    "src/agent/proc/pipe_io.rs",
    "src/agent/proc/worker.rs",
    "src/agent/proc/hooks.rs",
    "src/agent/bin.rs",
    "src/agent/claude.rs",
    "src/agent/codex.rs",
    "src/agent/copilot.rs",
    "src/capacity.rs",
    "src/export.rs",
    "src/main.rs",
    "src/answer.rs",
    "src/config.rs",
    "src/connect.rs",
    "src/route.rs",
    "src/status.rs",
    "src/validate.rs",
    "src/events/mod.rs",
    "src/events/log/premove.rs",
];

#[must_use]
pub fn externally_reachable_fns(source: &str) -> Vec<String> {
    let region = production_code(source);
    let bytes = region.as_bytes();
    let mut names = BTreeSet::new();
    let mut trait_impl_spans = Vec::new();
    let mut public_trait_spans = Vec::new();

    for (start, after) in keyword_sites(&region, "trait") {
        if !declares_visibility(region.get(..start).unwrap_or_default()) {
            continue;
        }
        let Some(brace) = find_header_brace(&region, after) else {
            continue;
        };
        if let Some(end) = matching(bytes, brace, b'{', b'}') {
            public_trait_spans.push((brace, end));
        }
    }

    for (_, after) in keyword_sites(&region, "impl") {
        let Some(brace) = find_header_brace(&region, after) else {
            continue;
        };
        let header = region.get(after..brace).unwrap_or_default();
        if keyword_sites(header, "for").next().is_none() {
            continue;
        }
        if let Some(end) = matching(bytes, brace, b'{', b'}') {
            trait_impl_spans.push((brace, end));
        }
    }

    for (index, name) in declared_fns(&region) {
        let visible = declares_visibility(region.get(..index).unwrap_or_default());
        let in_trait_impl = trait_impl_spans
            .iter()
            .any(|(open, close)| index > *open && index < *close);
        let is_default_body = public_trait_spans
            .iter()
            .any(|(open, close)| index > *open && index < *close)
            && find_header_brace(&region, index).is_some();
        if visible || in_trait_impl || is_default_body {
            names.insert(name.to_owned());
        }
    }
    names.into_iter().collect()
}

#[must_use]
pub fn reachable_fn_multiplicity(source: &str) -> BTreeMap<String, usize> {
    let region = production_code(source);
    let reachable: BTreeSet<String> = externally_reachable_fns(source).into_iter().collect();
    let mut bearers = BTreeMap::new();
    for (_, name) in declared_fns(&region) {
        if reachable.contains(name) {
            *bearers.entry(name.to_owned()).or_insert(0) += 1;
        }
    }
    bearers
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnerHeader {
    Module(String),
    Trait(String),
    TraitImpl(String),
    InherentImpl,
    Unread,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnerScope {
    pub opened_at: usize,
    pub header: OwnerHeader,
}

#[must_use]
pub fn reachable_fn_owners(source: &str) -> BTreeMap<String, Vec<Vec<OwnerScope>>> {
    let region = production_code(source);
    let reachable: BTreeSet<String> = externally_reachable_fns(source).into_iter().collect();
    let mut bearers = declared_fns(&region).into_iter().peekable();
    let mut open: Vec<(u8, OwnerScope)> = Vec::new();
    let mut header_from = 0;
    let mut owners: BTreeMap<String, Vec<Vec<OwnerScope>>> = BTreeMap::new();
    for (at, byte) in region.bytes().enumerate() {
        while let Some((_, name)) = bearers.next_if(|(index, _)| *index <= at) {
            if reachable.contains(name) {
                owners
                    .entry(name.to_owned())
                    .or_default()
                    .push(open.iter().map(|(_, scope)| scope.clone()).collect());
            }
        }
        let in_braces = open.last().is_none_or(|(opener, _)| *opener == b'{');
        match byte {
            b'{' => {
                let header = owner_header(region.get(header_from..at).unwrap_or_default());
                open.push((
                    byte,
                    OwnerScope {
                        opened_at: at,
                        header,
                    },
                ));
                header_from = at + 1;
            }
            b'(' | b'[' => open.push((
                byte,
                OwnerScope {
                    opened_at: at,
                    header: OwnerHeader::Unread,
                },
            )),
            b'}' | b')' | b']' => {
                let opener = match byte {
                    b'}' => b'{',
                    b')' => b'(',
                    _ => b'[',
                };
                if open.last().is_some_and(|(opened, _)| *opened == opener) {
                    open.pop();
                }
                if byte == b'}' {
                    header_from = at + 1;
                }
            }
            b';' if in_braces => header_from = at + 1,
            _ => {}
        }
    }
    owners
}

fn owner_header(header: &str) -> OwnerHeader {
    let mut item = header.trim_start();
    while let Some(attribute) = item.strip_prefix('#') {
        let attribute = attribute.strip_prefix('!').unwrap_or(attribute);
        let close = attribute
            .starts_with('[')
            .then(|| matching(attribute.as_bytes(), 0, b'[', b']'))
            .flatten();
        let Some(close) = close else {
            return OwnerHeader::Unread;
        };
        item = attribute.get(close + 1..).unwrap_or_default().trim_start();
    }

    let mut depth = 0usize;
    let mut plain = String::with_capacity(item.len());
    let mut previous = ' ';
    for character in without_visibility(item).chars() {
        match character {
            '<' => {
                if depth == 0 {
                    plain.push(' ');
                }
                depth += 1;
            }
            '>' if depth > 0 && previous != '-' => depth -= 1,
            _ if depth == 0 => plain.push(character),
            _ => {}
        }
        previous = character;
    }

    let mut words = plain.split_whitespace().peekable();
    words.next_if_eq(&"unsafe");
    match words.next() {
        Some("mod") => match words.next() {
            Some(name) if is_identifier(name) => OwnerHeader::Module(name.to_owned()),
            _ => OwnerHeader::Unread,
        },
        Some("trait") => match words.next().map(|name| name.trim_end_matches(':')) {
            Some(name) if is_identifier(name) => OwnerHeader::Trait(name.to_owned()),
            _ => OwnerHeader::Unread,
        },
        Some("impl") => {
            let written: Vec<&str> = words.take_while(|word| *word != "where").collect();
            match written.iter().position(|word| *word == "for") {
                None => OwnerHeader::InherentImpl,
                Some(1) => written
                    .first()
                    .filter(|name| is_identifier(name))
                    .map_or(OwnerHeader::Unread, |name| {
                        OwnerHeader::TraitImpl((*name).to_owned())
                    }),
                _ => OwnerHeader::Unread,
            }
        }
        _ => OwnerHeader::Unread,
    }
}

fn without_visibility(item: &str) -> &str {
    let Some(after) = item.strip_prefix("pub") else {
        return item;
    };
    if let Some(restriction) = after.trim_start().strip_prefix('(') {
        return restriction.split_once(')').map_or(item, |(_, rest)| rest);
    }
    if after.starts_with(char::is_whitespace) {
        after
    } else {
        item
    }
}

fn is_identifier(word: &str) -> bool {
    word.bytes().all(is_ident_byte)
        && word
            .bytes()
            .next()
            .is_some_and(|first| !first.is_ascii_digit())
}

fn keyword_sites<'a>(text: &'a str, keyword: &'a str) -> impl Iterator<Item = (usize, usize)> + 'a {
    let bytes = text.as_bytes();
    text.match_indices(keyword)
        .map(|(start, word)| (start, start + word.len()))
        .filter(move |(start, end)| {
            let glued_before = start
                .checked_sub(1)
                .and_then(|before| bytes.get(before))
                .is_some_and(|byte| is_ident_byte(*byte));
            let glued_after = text.get(*end..).is_some_and(|rest| {
                rest.starts_with(|next: char| next.is_alphanumeric() || next == '_')
            });
            !glued_before && !glued_after
        })
}

fn declared_fns(region: &str) -> Vec<(usize, &str)> {
    keyword_sites(region, "fn")
        .filter_map(|(index, after)| {
            let written = region
                .get(after..)?
                .strip_prefix(is_rustc_whitespace)?
                .trim_start_matches(is_rustc_whitespace)
                .split(|c: char| is_rustc_whitespace(c) || matches!(c, '(' | '<'))
                .next()?;
            let name = written.strip_prefix("r#").unwrap_or(written);
            (!name.is_empty() && !name.starts_with('$')).then_some((index, name))
        })
        .collect()
}

fn is_ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn declares_visibility(prefix: &str) -> bool {
    let mut rest = prefix.trim_end();
    for modifier in ["extern", "unsafe", "const", "async"] {
        for _ in 0..3 {
            rest = rest.strip_suffix(modifier).unwrap_or(rest).trim_end();
        }
    }
    if rest.ends_with("pub") {
        return true;
    }
    rest.strip_suffix(')')
        .and_then(|restriction| restriction.rsplit_once('('))
        .is_some_and(|(before, _)| before.trim_end().ends_with("pub"))
}

fn find_header_brace(region: &str, from: usize) -> Option<usize> {
    let bytes = region.as_bytes();
    let mut angle = 0i32;
    let mut paren = 0i32;
    let mut bracket = 0i32;
    for (index, byte) in bytes.iter().enumerate().skip(from) {
        match byte {
            b'<' => angle += 1,
            b'>' => angle -= 1,
            b'(' => paren += 1,
            b')' => paren -= 1,
            b'[' => bracket += 1,
            b']' => bracket -= 1,
            b';' if angle <= 0 && paren <= 0 && bracket <= 0 => return None,
            b'{' if angle <= 0 && paren <= 0 && bracket <= 0 => return Some(index),
            _ => {}
        }
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DenialFixture {
    pub shape: &'static str,
    pub source: &'static str,
    pub lint: &'static str,
    pub resolves_to: &'static str,
}

pub const DENIAL_FIXTURES: &[DenialFixture] = &[
    DenialFixture {
        shape: "renamed-import",
        source: "use std::fs::write as scribble;\n\
                 pub fn go(p: &str) { let _ = scribble(p, \"x\"); }\n",
        lint: "clippy::disallowed_methods",
        resolves_to: "std::fs::write",
    },
    DenialFixture {
        shape: "re-export",
        source: "pub mod hatch { pub use std::fs::write; }\n\
                 pub fn go(p: &str) { let _ = hatch::write(p, \"x\"); }\n",
        lint: "clippy::disallowed_methods",
        resolves_to: "std::fs::write",
    },
    DenialFixture {
        shape: "function-value",
        source: "pub fn go(p: &str) {\n\
                 \x20   let f = std::fs::write::<&str, &str>;\n\
                 \x20   let _ = f(p, \"x\");\n\
                 }\n",
        lint: "clippy::disallowed_methods",
        resolves_to: "std::fs::write",
    },
    DenialFixture {
        shape: "legacy-wrapper call",
        source: "pub fn go(p: &std::path::Path) {\n\
                 \x20   let _ = upstroke::util::write_text(p, \"x\");\n\
                 }\n",
        lint: "clippy::disallowed_methods",
        resolves_to: "upstroke::util::write_text",
    },
    DenialFixture {
        shape: "method call",
        source: "pub fn go(p: &std::path::Path) -> std::io::Result<()> {\n\
                 \x20   let f = std::fs::File::open(p)?;\n\
                 \x20   f.sync_all()\n\
                 }\n",
        lint: "clippy::disallowed_methods",
        resolves_to: "std::fs::File::sync_all",
    },
    DenialFixture {
        shape: "macro-expanded code",
        source: "pub fn go() { println!(\"escaped\"); }\n",
        lint: "clippy::disallowed_macros",
        resolves_to: "std::println",
    },
    DenialFixture {
        shape: "type",
        source: "pub fn go() { let _ = std::process::Command::new(\"git\"); }\n",
        lint: "clippy::disallowed_types",
        resolves_to: "std::process::Command",
    },
];

pub const DENIAL_CONTROL: &str = "pub fn go(p: &std::path::Path) -> bool {\n\
                                  \x20   let _ = upstroke::util::tail(\"x\", 1);\n\
                                  \x20   p.exists()\n\
                                  }\n";

#[cfg(test)]
pub(crate) mod census_domain {
    use std::path::PathBuf;

    use super::lint_levels::{applied_attributes, attribute_arguments, attribute_name};
    use super::{block_comment_end, is_doc_comment};

    pub(crate) fn production_calls(code: &str, name: &str, form: Call) -> usize {
        let needle = format!("{name}(");
        code.match_indices(&needle)
            .filter(|(at, _)| {
                code[..*at]
                    .chars()
                    .next_back()
                    .is_none_or(|before| !before.is_alphanumeric() && before != '_')
            })
            .filter(|(at, _)| !code[..*at].trim_end().ends_with("fn"))
            .filter(|(at, _)| {
                let dotted = code[..*at].trim_end().ends_with('.');
                match form {
                    Call::Free => !dotted,
                    Call::Method => dotted,
                }
            })
            .count()
    }

    #[derive(Clone, Copy)]
    pub(crate) enum Call {
        Free,
        Method,
    }

    pub(crate) fn whole_file_test_modules(
        source_root: &std::path::Path,
        files: &[PathBuf],
        floor: usize,
    ) -> std::collections::BTreeSet<PathBuf> {
        let declarations = declared_whole_file_test_modules(source_root, files);
        assert!(
            declarations.len() >= floor,
            "only {} test-only `mod …;` declarations were derived and the floor is {floor}; \
             the derivation is finding nothing",
            declarations.len()
        );
        let mut modules = std::collections::BTreeSet::new();
        let mut edges: Vec<(PathBuf, PathBuf)> = Vec::new();
        for declaration in &declarations {
            let resolved = sole_present(&declaration.candidates, &|path| path.is_file())
                .unwrap_or_else(|present| {
                    panic!(
                        "`{}` declares `mod {};` under {} and {present} of {:?} exist. A skip \
                         path naming no file is a skip that has stopped meaning anything",
                        declaration.declared_in.display(),
                        declaration.name,
                        declaration.render_guard(),
                        declaration.candidates
                    )
                })
                .clone();
            assert!(
                modules.insert(resolved.clone()),
                "two declarations resolve to `{}`; one of them is deriving a skip for a file it \
                 does not declare",
                resolved.display()
            );
            edges.push((declaration.declared_in.clone(), resolved));
        }
        assert!(
            declaration_cycle(&edges).is_none(),
            "the module declarations are cyclic, so no file's guard can be trusted: {:?}",
            declaration_cycle(&edges)
        );
        assert!(
            modules
                .iter()
                .any(|path| path.file_stem().is_none_or(|stem| stem != "tests")),
            "every module derived here is called `tests.rs`, which is exactly what the file-name \
             rule this replaces also finds. The derivation has degraded to the rule it exists to \
             be better than: {modules:?}"
        );
        modules
    }

    pub(crate) fn declaration_cycle(edges: &[(PathBuf, PathBuf)]) -> Option<Vec<PathBuf>> {
        use std::collections::{BTreeMap, BTreeSet};

        #[derive(Clone, Copy, PartialEq, Eq)]
        enum Colour {
            White,
            Grey,
            Black,
        }

        let mut adjacency: BTreeMap<&PathBuf, Vec<&PathBuf>> = BTreeMap::new();
        let mut nodes: BTreeSet<&PathBuf> = BTreeSet::new();
        for (from, to) in edges {
            adjacency.entry(from).or_default().push(to);
            nodes.insert(from);
            nodes.insert(to);
        }

        let mut colour: BTreeMap<&PathBuf, Colour> =
            nodes.iter().map(|node| (*node, Colour::White)).collect();
        for start in &nodes {
            if colour.get(start) != Some(&Colour::White) {
                continue;
            }
            colour.insert(start, Colour::Grey);
            let mut stack: Vec<(&PathBuf, usize)> = vec![(start, 0)];
            while let Some((node, taken)) = stack.pop() {
                let outgoing: &[&PathBuf] = adjacency
                    .get(node)
                    .map_or(&[][..], |edges| edges.as_slice());
                let Some(next) = outgoing.get(taken).copied() else {
                    colour.insert(node, Colour::Black);
                    continue;
                };
                stack.push((node, taken + 1));
                match colour.get(next) {
                    Some(Colour::Grey) => {
                        let path: Vec<&PathBuf> = stack.iter().map(|(at, _)| *at).collect();
                        let from = path.iter().position(|at| *at == next).unwrap_or(0);
                        let mut cycle: Vec<PathBuf> =
                            path[from..].iter().map(|at| (*at).clone()).collect();
                        cycle.push(next.clone());
                        return Some(cycle);
                    }
                    Some(Colour::Black) => {}
                    _ => {
                        colour.insert(next, Colour::Grey);
                        stack.push((next, 0));
                    }
                }
            }
        }
        None
    }

    pub(crate) fn sole_present<'a>(
        candidates: &'a [PathBuf; 2],
        exists: &dyn Fn(&std::path::Path) -> bool,
    ) -> Result<&'a PathBuf, usize> {
        let present: Vec<&PathBuf> = candidates
            .iter()
            .filter(|candidate| exists(candidate))
            .collect();
        match present.as_slice() {
            [only] => Ok(only),
            other => Err(other.len()),
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) enum CandidateRefusal {
        OutsideThePackage {
            declared_in: PathBuf,
            package_dir: PathBuf,
        },
    }

    impl std::fmt::Display for CandidateRefusal {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::OutsideThePackage {
                    declared_in,
                    package_dir,
                } => write!(
                    f,
                    "`{}` is not inside `{}`, so the target inventory read for that package \
                     does not say whether it is a crate root",
                    declared_in.display(),
                    package_dir.display()
                ),
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) enum InventoryRefusal {
        NotRun {
            manifest: PathBuf,
            why: String,
        },
        Failed {
            manifest: PathBuf,
            status: String,
            stderr: String,
        },
        Unreadable {
            manifest: PathBuf,
            why: String,
        },
        NoPackage {
            manifest: PathBuf,
        },
        NoTargets {
            manifest: PathBuf,
        },
    }

    impl std::fmt::Display for InventoryRefusal {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::NotRun { manifest, why } => write!(
                    f,
                    "`cargo metadata` for `{}` could not be started ({why}), so the crate roots \
                     are unknown and this census will not guess them from file names",
                    manifest.display()
                ),
                Self::Failed {
                    manifest,
                    status,
                    stderr,
                } => write!(
                    f,
                    "`cargo metadata` for `{}` exited {status}: {stderr}",
                    manifest.display()
                ),
                Self::Unreadable { manifest, why } => write!(
                    f,
                    "`cargo metadata` for `{}` did not answer with the document this reads \
                     ({why})",
                    manifest.display()
                ),
                Self::NoPackage { manifest } => write!(
                    f,
                    "`cargo metadata` named no package whose manifest is `{}`",
                    manifest.display()
                ),
                Self::NoTargets { manifest } => write!(
                    f,
                    "the package at `{}` declares no target, so it has no crate root",
                    manifest.display()
                ),
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct CrateRoots {
        package_dir: PathBuf,
        roots: std::collections::BTreeSet<PathBuf>,
    }

    impl CrateRoots {
        pub(crate) fn from_metadata_json(
            json: &str,
            manifest: &std::path::Path,
        ) -> Result<Self, InventoryRefusal> {
            let refuse = |why: &str| InventoryRefusal::Unreadable {
                manifest: manifest.to_path_buf(),
                why: why.to_owned(),
            };
            let document: serde_json::Value =
                serde_json::from_str(json).map_err(|error| refuse(&error.to_string()))?;
            let packages = document
                .get("packages")
                .and_then(serde_json::Value::as_array)
                .ok_or_else(|| refuse("no `packages` array"))?;
            let package = packages
                .iter()
                .find(|package| {
                    package
                        .get("manifest_path")
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(|path| std::path::Path::new(path) == manifest)
                })
                .ok_or_else(|| InventoryRefusal::NoPackage {
                    manifest: manifest.to_path_buf(),
                })?;
            let targets = package
                .get("targets")
                .and_then(serde_json::Value::as_array)
                .ok_or_else(|| refuse("the package has no `targets` array"))?;
            let mut roots = std::collections::BTreeSet::new();
            for target in targets {
                let source = target
                    .get("src_path")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| refuse("a target carries no `src_path`"))?;
                roots.insert(PathBuf::from(source));
            }
            if roots.is_empty() {
                return Err(InventoryRefusal::NoTargets {
                    manifest: manifest.to_path_buf(),
                });
            }
            Ok(Self {
                package_dir: manifest
                    .parent()
                    .ok_or_else(|| refuse("the manifest path has no directory"))?
                    .to_path_buf(),
                roots,
            })
        }

        pub(crate) fn package_dir(&self) -> &std::path::Path {
            &self.package_dir
        }

        pub(crate) fn roots(&self) -> impl Iterator<Item = &std::path::Path> {
            self.roots.iter().map(PathBuf::as_path)
        }

        pub(crate) fn is_root(&self, path: &std::path::Path) -> bool {
            self.roots.contains(path)
        }

        pub(crate) fn is_root_relative(&self, relative: &str) -> bool {
            let mut candidate = self.package_dir.clone();
            for part in relative.split('/') {
                candidate.push(part);
            }
            self.is_root(&candidate)
        }
    }

    pub(crate) fn module_directory(
        roots: &CrateRoots,
        declared_in: &std::path::Path,
    ) -> Result<PathBuf, CandidateRefusal> {
        let parent = declared_in
            .parent()
            .expect("a source file has a directory")
            .to_path_buf();
        let stem = declared_in.file_stem().expect("a source file has a name");
        if roots.is_root(declared_in) {
            return Ok(parent);
        }
        if !declared_in.starts_with(roots.package_dir()) {
            return Err(CandidateRefusal::OutsideThePackage {
                declared_in: declared_in.to_path_buf(),
                package_dir: roots.package_dir().to_path_buf(),
            });
        }
        if stem == "mod" {
            return Ok(parent);
        }
        Ok(parent.join(stem))
    }

    pub(crate) fn candidates_for(
        roots: &CrateRoots,
        declared_in: &std::path::Path,
        inline_path: &[String],
        name: &str,
    ) -> Result<[PathBuf; 2], CandidateRefusal> {
        let mut dir = module_directory(roots, declared_in)?;
        for enclosing in inline_path {
            dir.push(enclosing);
        }
        Ok([
            dir.join(format!("{name}.rs")),
            dir.join(name).join("mod.rs"),
        ])
    }

    pub(crate) fn contained_in(base: &std::path::Path, candidate: &std::path::Path) -> bool {
        let Ok(rest) = candidate.strip_prefix(base) else {
            return false;
        };
        rest.components().count() > 0
            && rest
                .components()
                .all(|part| matches!(part, std::path::Component::Normal(_)))
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct TestModuleDeclaration {
        pub(crate) declared_in: PathBuf,
        pub(crate) name: String,
        pub(crate) inline_path: Vec<String>,
        pub(crate) guard: String,
        pub(crate) candidates: [PathBuf; 2],
    }

    impl TestModuleDeclaration {
        fn render_guard(&self) -> String {
            if self.inline_path.is_empty() {
                format!("`cfg({})`", self.guard)
            } else {
                format!(
                    "`cfg({})` through `{}`",
                    self.guard,
                    self.inline_path.join("::")
                )
            }
        }
    }

    pub(crate) fn declared_whole_file_test_modules(
        source_root: &std::path::Path,
        files: &[PathBuf],
    ) -> Vec<TestModuleDeclaration> {
        let roots = crate::effects::tests::crate_roots();
        assert!(
            roots.roots().any(|root| root.starts_with(source_root)),
            "no target of the package at `{}` lives under `{}`, so its inventory does not \
             describe the tree this census was handed: {:?}",
            roots.package_dir().display(),
            source_root.display(),
            roots.roots().collect::<Vec<_>>()
        );
        let mut found = Vec::new();
        for path in files {
            let source = std::fs::read_to_string(path).expect("read source");
            let declarations = scan_module_declarations(&source)
                .unwrap_or_else(|refusal| panic!("{}: {refusal}", path.display()));
            let parent = path.parent().expect("a source file has a directory");
            for declaration in declarations {
                if !declaration.test_only {
                    continue;
                }
                let candidates =
                    candidates_for(roots, path, &declaration.inline_path, &declaration.name)
                        .unwrap_or_else(|refusal| panic!("{refusal}"));
                for candidate in &candidates {
                    assert!(
                        contained_in(parent, candidate),
                        "`{}` declares `mod {};` and the candidate `{}` leaves `{}`",
                        path.display(),
                        declaration.name,
                        candidate.display(),
                        parent.display()
                    );
                }
                found.push(TestModuleDeclaration {
                    declared_in: path.clone(),
                    name: declaration.name,
                    inline_path: declaration.inline_path,
                    guard: declaration.guard,
                    candidates,
                });
            }
        }
        found
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct ScannedDeclaration {
        pub(crate) name: String,
        pub(crate) inline_path: Vec<String>,
        pub(crate) guard: String,
        pub(crate) test_only: bool,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct ScannedInlineModule {
        pub(crate) name: String,
        pub(crate) inline_path: Vec<String>,
        pub(crate) guard: String,
        pub(crate) test_only: bool,
        pub(crate) line: usize,
        pub(crate) outer_attributes: String,
        pub(crate) body: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Default)]
    pub(crate) struct ScannedModules {
        pub(crate) declared: Vec<ScannedDeclaration>,
        pub(crate) inline: Vec<ScannedInlineModule>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) enum ScanRefusal {
        UnclosedAttribute {
            line: usize,
        },
        UnbalancedBraces {
            line: usize,
        },
        MalformedDeclaration {
            line: usize,
        },
        UnreadablePredicate {
            line: usize,
            written: String,
            why: String,
        },
        UnsupportedPathAttribute {
            line: usize,
            name: String,
        },
        UnsupportedInnerCfg {
            line: usize,
        },
        DuplicateDeclaration {
            line: usize,
            name: String,
        },
        ModuleShapedMacroBody {
            line: usize,
            macro_name: String,
        },
    }

    impl std::fmt::Display for ScanRefusal {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::UnclosedAttribute { line } => {
                    write!(f, "line {line}: an attribute is never closed")
                }
                Self::UnbalancedBraces { line } => {
                    write!(
                        f,
                        "line {line}: a `}}` closes a block that was never opened"
                    )
                }
                Self::MalformedDeclaration { line } => write!(
                    f,
                    "line {line}: a `mod` declaration has no name, or no `;` or `{{` after it"
                ),
                Self::UnreadablePredicate { line, written, why } => write!(
                    f,
                    "line {line}: `cfg({written})` cannot be decided against `test`: {why}"
                ),
                Self::UnsupportedPathAttribute { line, name } => write!(
                    f,
                    "line {line}: `mod {name}` carries a `path` attribute, which this \
                     derivation refuses rather than resolves"
                ),
                Self::UnsupportedInnerCfg { line } => write!(
                    f,
                    "line {line}: an inner `#![cfg(…)]` gates the module it is written in, \
                     which this derivation does not model"
                ),
                Self::DuplicateDeclaration { line, name } => {
                    write!(
                        f,
                        "line {line}: `mod {name};` is declared twice in one module"
                    )
                }
                Self::ModuleShapedMacroBody { line, macro_name } => write!(
                    f,
                    "line {line}: the body of `{macro_name}!` holds a module-shaped token \
                     sequence. A macro body is token trees, not items, and whether the \
                     expansion declares a module is not readable from here"
                ),
            }
        }
    }

    pub(crate) fn scan_module_declarations(
        source: &str,
    ) -> Result<Vec<ScannedDeclaration>, ScanRefusal> {
        scan_modules(source).map(|scanned| scanned.declared)
    }

    pub(crate) fn scan_modules(source: &str) -> Result<ScannedModules, ScanRefusal> {
        struct Scope {
            open_depth: usize,
            name: String,
            preds: Vec<Predicate>,
            declared: std::collections::BTreeSet<String>,
        }

        let blanked = super::blank_comments_and_strings(source);
        debug_assert_eq!(blanked.len(), source.len());
        let bytes = blanked.as_bytes();
        let line_of = |at: usize| {
            blanked
                .get(..at)
                .map_or(0, |before| before.matches('\n').count())
                + 1
        };

        let mut found = Vec::new();
        let mut inline = Vec::new();
        let mut scopes: Vec<Scope> = Vec::new();
        let mut top_level: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        let mut pending: Vec<Predicate> = Vec::new();
        let mut pending_path = false;
        let mut attributes_from: Option<usize> = None;
        let mut depth = 0_usize;
        let mut i = 0;

        while let Some(&byte) = bytes.get(i) {
            if byte.is_ascii_whitespace() {
                i += 1;
                continue;
            }

            if byte == b'#' {
                let Some((inner, open)) = super::attribute_open(source, bytes, i) else {
                    i += 1;
                    continue;
                };
                let Some(close) = super::matching(bytes, open, b'[', b']') else {
                    return Err(ScanRefusal::UnclosedAttribute { line: line_of(i) });
                };
                let raw = source.get(open + 1..close).unwrap_or_default();
                let shape = blanked.get(open + 1..close).unwrap_or_default();
                let name = attribute_name(shape.trim_start());
                match name {
                    "cfg" | "cfg_attr" => {
                        if name == "cfg_attr" && raw.contains("path") {
                            pending_path = true;
                        }
                        let text = with_literal_identity(raw, shape).ok_or_else(|| {
                            ScanRefusal::UnreadablePredicate {
                                line: line_of(i),
                                written: raw.trim().to_owned(),
                                why: "its comments and literals do not read as the blanked text \
                                      does"
                                    .to_owned(),
                            }
                        })?;
                        for applied in applied_attributes(text.trim()) {
                            if attribute_name(applied.text) != "cfg" {
                                continue;
                            }
                            let written =
                                attribute_arguments(applied.text).unwrap_or_default().trim();
                            let gate = parse_predicate(written).map_err(|why| {
                                ScanRefusal::UnreadablePredicate {
                                    line: line_of(i),
                                    written: written.to_owned(),
                                    why,
                                }
                            })?;
                            let under = applied
                                .under
                                .into_iter()
                                .collect::<Result<Vec<Predicate>, String>>()
                                .map_err(|why| ScanRefusal::UnreadablePredicate {
                                    line: line_of(i),
                                    written: written.to_owned(),
                                    why,
                                })?;
                            if inner {
                                return Err(ScanRefusal::UnsupportedInnerCfg { line: line_of(i) });
                            }
                            pending.push(if under.is_empty() {
                                gate
                            } else {
                                Predicate::Any(vec![
                                    Predicate::Not(Box::new(Predicate::all(under))),
                                    gate,
                                ])
                            });
                        }
                    }
                    "path" => pending_path = true,
                    _ => {}
                }
                if !inner {
                    attributes_from.get_or_insert(i);
                }
                i = close + 1;
                continue;
            }

            if let Some(invocation) = macro_at(bytes, i) {
                let MacroInvocation { name, open, close } = invocation;
                if let Some(shaped) = module_shaped_between(bytes, open + 1, close) {
                    return Err(ScanRefusal::ModuleShapedMacroBody {
                        line: line_of(shaped),
                        macro_name: name,
                    });
                }
                pending.clear();
                pending_path = false;
                attributes_from = None;
                i = close + 1;
                continue;
            }

            if let Some(shape) = module_at(bytes, i) {
                let ModuleShape {
                    name_at,
                    name,
                    body,
                } = shape;
                if name.is_empty() {
                    return Err(ScanRefusal::MalformedDeclaration { line: line_of(i) });
                }
                if pending_path {
                    return Err(ScanRefusal::UnsupportedPathAttribute {
                        line: line_of(i),
                        name,
                    });
                }
                let mut preds: Vec<Predicate> = scopes
                    .iter()
                    .flat_map(|scope| scope.preds.iter().cloned())
                    .collect();
                preds.extend(pending.iter().cloned());
                match body {
                    Some(brace) => {
                        let effective = Predicate::all(preds);
                        let close =
                            super::matching(bytes, brace, b'{', b'}').unwrap_or(bytes.len());
                        inline.push(ScannedInlineModule {
                            name: name.clone(),
                            inline_path: scopes.iter().map(|scope| scope.name.clone()).collect(),
                            guard: effective.render(),
                            test_only: entails_test(&effective),
                            line: line_of(name_at),
                            outer_attributes: attributes_from
                                .and_then(|from| source.get(from..i))
                                .unwrap_or_default()
                                .to_owned(),
                            body: source.get(brace + 1..close).unwrap_or_default().to_owned(),
                        });
                        scopes.push(Scope {
                            open_depth: depth,
                            name,
                            preds: std::mem::take(&mut pending),
                            declared: std::collections::BTreeSet::new(),
                        });
                        depth += 1;
                        i = brace + 1;
                    }
                    None => {
                        let declared = match scopes.last_mut() {
                            Some(scope) => &mut scope.declared,
                            None => &mut top_level,
                        };
                        if !declared.insert(name.clone()) {
                            return Err(ScanRefusal::DuplicateDeclaration {
                                line: line_of(name_at),
                                name,
                            });
                        }
                        let effective = Predicate::all(preds);
                        found.push(ScannedDeclaration {
                            name,
                            inline_path: scopes.iter().map(|scope| scope.name.clone()).collect(),
                            guard: effective.render(),
                            test_only: entails_test(&effective),
                        });
                        pending.clear();
                        i = bytes
                            .get(name_at..)
                            .and_then(|rest| rest.iter().position(|byte| *byte == b';'))
                            .map_or(bytes.len(), |at| name_at + at + 1);
                    }
                }
                pending_path = false;
                attributes_from = None;
                continue;
            }

            pending.clear();
            pending_path = false;
            attributes_from = None;
            if byte == b'{' {
                depth += 1;
                i += 1;
                continue;
            }
            if byte == b'}' {
                if depth == 0 {
                    return Err(ScanRefusal::UnbalancedBraces { line: line_of(i) });
                }
                depth -= 1;
                scopes.retain(|scope| scope.open_depth < depth);
                i += 1;
                continue;
            }
            if super::is_ident_byte(byte) {
                i = word(bytes, i).end;
                continue;
            }
            i += 1;
        }
        Ok(ScannedModules {
            declared: found,
            inline,
        })
    }

    struct MacroInvocation {
        name: String,
        open: usize,
        close: usize,
    }

    fn macro_at(bytes: &[u8], at: usize) -> Option<MacroInvocation> {
        let name = word(bytes, at);
        let after_name = name.end;
        if name.text.is_empty() {
            return None;
        }
        if !name.raw && is_keyword(name.text) {
            return None;
        }
        let bang = whitespace(bytes, after_name);
        if bytes.get(bang) != Some(&b'!') {
            return None;
        }
        let mut cursor = whitespace(bytes, bang + 1);
        if !name.raw && name.text == b"macro_rules" {
            let defined = word(bytes, cursor);
            if !defined.text.is_empty() {
                cursor = whitespace(bytes, defined.end);
            }
        }
        let (opener, closer) = match bytes.get(cursor) {
            Some(b'(') => (b'(', b')'),
            Some(b'[') => (b'[', b']'),
            Some(b'{') => (b'{', b'}'),
            _ => return None,
        };
        let close = super::matching(bytes, cursor, opener, closer)?;
        Some(MacroInvocation {
            name: String::from_utf8_lossy(name.text).into_owned(),
            open: cursor,
            close,
        })
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct OutsideInvocation {
        pub(crate) line: usize,
        pub(crate) name: String,
    }

    #[must_use]
    pub(crate) fn macro_invocations_outside_function_bodies(
        source: &str,
    ) -> Vec<OutsideInvocation> {
        let code = super::production_code(source);
        let bytes = code.as_bytes();
        let bodies = function_bodies(bytes);
        macro_bangs(bytes)
            .into_iter()
            .filter(|(bang, _)| {
                !bodies
                    .iter()
                    .any(|(open, close)| open < bang && bang < close)
            })
            .map(|(bang, name)| OutsideInvocation {
                line: code
                    .get(..bang)
                    .map_or(0, |before| before.matches('\n').count())
                    + 1,
                name,
            })
            .collect()
    }

    fn is_identifier_byte(byte: u8) -> bool {
        byte.is_ascii_alphanumeric() || byte == b'_' || !byte.is_ascii()
    }

    fn identifier_end(bytes: &[u8], from: usize) -> usize {
        let mut end = from;
        while bytes.get(end).is_some_and(|byte| is_identifier_byte(*byte)) {
            end += 1;
        }
        end
    }

    fn raw_prefix_before(bytes: &[u8], start: usize) -> bool {
        start >= 2
            && bytes.get(start - 2..start) == Some(b"r#".as_slice())
            && !start
                .checked_sub(3)
                .and_then(|before| bytes.get(before))
                .is_some_and(|byte| is_identifier_byte(*byte))
    }

    fn token_end(bytes: &[u8], from: usize) -> usize {
        if bytes.get(from..from + 2) == Some(b"r#".as_slice())
            && bytes
                .get(from + 2)
                .is_some_and(|byte| is_identifier_byte(*byte))
        {
            return identifier_end(bytes, from + 2);
        }
        identifier_end(bytes, from)
    }

    fn function_bodies(bytes: &[u8]) -> Vec<(usize, usize)> {
        let mut bodies = Vec::new();
        let mut at = 0;
        while let Some(&byte) = bytes.get(at) {
            if !is_identifier_byte(byte) {
                at += 1;
                continue;
            }
            let end = identifier_end(bytes, at);
            if bytes.get(at..end) == Some(b"fn".as_slice()) && !raw_prefix_before(bytes, at) {
                let name = whitespace(bytes, end);
                if name > end
                    && bytes
                        .get(name)
                        .is_some_and(|byte| is_identifier_byte(*byte))
                {
                    if let Some(open) = body_brace(bytes, token_end(bytes, name)) {
                        if let Some(close) = super::matching(bytes, open, b'{', b'}') {
                            bodies.push((open, close));
                        }
                    }
                }
            }
            at = end;
        }
        bodies
    }

    fn body_brace(bytes: &[u8], from: usize) -> Option<usize> {
        let mut angle = 0_usize;
        let mut at = from;
        while let Some(&byte) = bytes.get(at) {
            match byte {
                b'(' => at = super::matching(bytes, at, b'(', b')')?,
                b'[' => at = super::matching(bytes, at, b'[', b']')?,
                b'{' if angle == 0 => return Some(at),
                b'{' => at = super::matching(bytes, at, b'{', b'}')?,
                b'<' => angle += 1,
                b'>' if at.checked_sub(1).and_then(|before| bytes.get(before)) == Some(&b'-') => {}
                b'>' => angle = angle.checked_sub(1)?,
                b';' | b')' | b']' | b'}' if angle == 0 => return None,
                _ => {}
            }
            at += 1;
        }
        None
    }

    fn macro_bangs(bytes: &[u8]) -> Vec<(usize, String)> {
        let mut found = Vec::new();
        for (bang, byte) in bytes.iter().enumerate() {
            if *byte != b'!' || bytes.get(bang + 1) == Some(&b'=') {
                continue;
            }
            let mut name_end = bang;
            while name_end > 0 && bytes.get(name_end - 1).is_some_and(u8::is_ascii_whitespace) {
                name_end -= 1;
            }
            let mut name_start = name_end;
            while name_start > 0
                && bytes
                    .get(name_start - 1)
                    .is_some_and(|before| is_identifier_byte(*before))
            {
                name_start -= 1;
            }
            let Some(name) = bytes.get(name_start..name_end) else {
                continue;
            };
            if name.is_empty()
                || name.first().is_some_and(u8::is_ascii_digit)
                || (is_keyword(name) && !raw_prefix_before(bytes, name_start))
            {
                continue;
            }
            let after = whitespace(bytes, bang + 1);
            let opens = |at: usize| matches!(bytes.get(at), Some(b'(' | b'[' | b'{'));
            let invoked = opens(after)
                || (bytes
                    .get(after)
                    .is_some_and(|next| is_identifier_byte(*next))
                    && opens(whitespace(bytes, token_end(bytes, after))));
            if invoked {
                found.push((bang, String::from_utf8_lossy(name).into_owned()));
            }
        }
        found
    }

    fn module_shaped_between(bytes: &[u8], from: usize, to: usize) -> Option<usize> {
        let mut at = from;
        while at < to {
            if !super::is_ident_byte(bytes[at]) {
                at += 1;
                continue;
            }
            let keyword = word(bytes, at);
            if !keyword.raw && keyword.text == b"mod" {
                let name_at = whitespace(bytes, keyword.end);
                if name_at > keyword.end {
                    let declared = word(bytes, name_at);
                    if !declared.text.is_empty()
                        && matches!(
                            bytes.get(whitespace(bytes, declared.end)),
                            Some(b';' | b'{')
                        )
                    {
                        return Some(at);
                    }
                }
            }
            at = keyword.end;
        }
        None
    }

    fn identifier(bytes: &[u8], from: usize) -> (usize, &[u8]) {
        let mut end = from;
        while end < bytes.len() && super::is_ident_byte(bytes[end]) {
            end += 1;
        }
        (end, &bytes[from..end])
    }

    struct Word<'a> {
        end: usize,
        raw: bool,
        text: &'a [u8],
    }

    fn word(bytes: &[u8], from: usize) -> Word<'_> {
        if bytes.get(from) == Some(&b'r')
            && bytes.get(from + 1) == Some(&b'#')
            && bytes
                .get(from + 2)
                .is_some_and(|byte| super::is_ident_byte(*byte))
        {
            let (end, text) = identifier(bytes, from + 2);
            return Word {
                end,
                raw: true,
                text,
            };
        }
        let (end, text) = identifier(bytes, from);
        Word {
            end,
            raw: false,
            text,
        }
    }

    const KEYWORDS: &[&[u8]] = &[
        b"as",
        b"break",
        b"const",
        b"continue",
        b"crate",
        b"dyn",
        b"else",
        b"enum",
        b"extern",
        b"false",
        b"fn",
        b"for",
        b"if",
        b"impl",
        b"in",
        b"let",
        b"loop",
        b"match",
        b"mod",
        b"move",
        b"mut",
        b"pub",
        b"ref",
        b"return",
        b"self",
        b"Self",
        b"static",
        b"struct",
        b"super",
        b"trait",
        b"true",
        b"type",
        b"unsafe",
        b"use",
        b"where",
        b"while",
        b"async",
        b"await",
        b"dyn",
        b"abstract",
        b"become",
        b"box",
        b"do",
        b"final",
        b"macro",
        b"override",
        b"priv",
        b"typeof",
        b"unsized",
        b"virtual",
        b"yield",
        b"try",
        b"gen",
    ];

    fn is_keyword(text: &[u8]) -> bool {
        KEYWORDS.contains(&text)
    }

    fn whitespace(bytes: &[u8], from: usize) -> usize {
        let mut at = from;
        while at < bytes.len() && bytes[at].is_ascii_whitespace() {
            at += 1;
        }
        at
    }

    struct ModuleShape {
        name_at: usize,
        name: String,
        body: Option<usize>,
    }

    fn module_at(bytes: &[u8], at: usize) -> Option<ModuleShape> {
        let mut token = word(bytes, at);
        if !token.raw && token.text == b"pub" {
            let after = whitespace(bytes, token.end);
            let cursor = if bytes.get(after) == Some(&b'(') {
                super::matching(bytes, after, b'(', b')')? + 1
            } else {
                after
            };
            token = word(bytes, whitespace(bytes, cursor));
        }
        if token.raw || token.text != b"mod" {
            return None;
        }
        let after_keyword = whitespace(bytes, token.end);
        if after_keyword == token.end {
            return None;
        }
        let declared = word(bytes, after_keyword);
        let name_end = declared.end;
        let name = String::from_utf8_lossy(declared.text).into_owned();
        let terminator = whitespace(bytes, name_end);
        match bytes.get(terminator) {
            Some(b'{') => Some(ModuleShape {
                name_at: after_keyword,
                name,
                body: Some(terminator),
            }),
            Some(b';') => Some(ModuleShape {
                name_at: after_keyword,
                name,
                body: None,
            }),
            _ => Some(ModuleShape {
                name_at: after_keyword,
                name: String::new(),
                body: None,
            }),
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) enum Predicate {
        Test,
        Other(String),
        All(Vec<Predicate>),
        Any(Vec<Predicate>),
        Not(Box<Predicate>),
    }

    impl Predicate {
        fn all(parts: Vec<Predicate>) -> Self {
            if parts.len() == 1 {
                parts.into_iter().next().unwrap_or(Self::All(Vec::new()))
            } else {
                Self::All(parts)
            }
        }

        pub(crate) fn render(&self) -> String {
            fn join(parts: &[Predicate]) -> String {
                parts
                    .iter()
                    .map(Predicate::render)
                    .collect::<Vec<_>>()
                    .join(", ")
            }
            match self {
                Self::Test => "test".to_owned(),
                Self::Other(written) => written.clone(),
                Self::All(parts) if parts.is_empty() => "true".to_owned(),
                Self::All(parts) => format!("all({})", join(parts)),
                Self::Any(parts) => format!("any({})", join(parts)),
                Self::Not(inner) => format!("not({})", inner.render()),
            }
        }
    }

    pub(crate) fn entails_test(predicate: &Predicate) -> bool {
        matches!(decide_without_test(predicate), Some(false))
    }

    pub(crate) fn decide_without_test(predicate: &Predicate) -> Option<bool> {
        match predicate {
            Predicate::Test => Some(false),
            Predicate::Other(_) => None,
            Predicate::Not(inner) => decide_without_test(inner).map(|value| !value),
            Predicate::All(parts) => {
                let mut every_part_is_true = true;
                for part in parts {
                    match decide_without_test(part) {
                        Some(false) => return Some(false),
                        Some(true) => {}
                        None => every_part_is_true = false,
                    }
                }
                every_part_is_true.then_some(true)
            }
            Predicate::Any(parts) => {
                let mut every_part_is_false = true;
                for part in parts {
                    match decide_without_test(part) {
                        Some(true) => return Some(true),
                        Some(false) => {}
                        None => every_part_is_false = false,
                    }
                }
                every_part_is_false.then_some(false)
            }
        }
    }

    pub(crate) fn parse_predicate(written: &str) -> Result<Predicate, String> {
        let text = written.trim();
        if text.is_empty() {
            return Err("the predicate is empty".to_owned());
        }
        let name_end = text
            .find(|ch: char| !(ch.is_alphanumeric() || ch == '_'))
            .unwrap_or(text.len());
        let (name, rest) = text.split_at(name_end);
        let rest = rest.trim_start();
        if !rest.starts_with('(') {
            if name.is_empty() {
                return Err(format!("`{text}` does not begin with a name"));
            }
            if rest.is_empty() {
                return Ok(if name == "test" {
                    Predicate::Test
                } else {
                    Predicate::Other(name.to_owned())
                });
            }
            let Some(value) = rest.strip_prefix('=') else {
                return Err(format!("`{text}` is neither an atom nor a combinator"));
            };
            if value.trim().is_empty() {
                return Err(format!("`{name}` is compared with nothing"));
            }
            return Ok(Predicate::Other(text.to_owned()));
        }
        let inner = split_arguments(rest)?;
        let parts = inner
            .into_iter()
            .map(parse_predicate)
            .collect::<Result<Vec<_>, _>>()?;
        match name {
            "all" => Ok(Predicate::All(parts)),
            "any" => Ok(Predicate::Any(parts)),
            "not" => match <[Predicate; 1]>::try_from(parts) {
                Ok([only]) => Ok(Predicate::Not(Box::new(only))),
                Err(parts) => Err(format!("`not` takes one predicate, not {}", parts.len())),
            },
            other => Err(format!("`{other}(…)` is not a predicate combinator")),
        }
    }

    fn split_arguments(text: &str) -> Result<Vec<&str>, String> {
        let bytes = text.as_bytes();
        let mut depth = 0_usize;
        let mut close = None;
        let mut quoted = false;
        for (at, byte) in bytes.iter().enumerate() {
            match byte {
                b'"' => quoted = !quoted,
                b'(' if !quoted => depth += 1,
                b')' if !quoted => {
                    depth -= 1;
                    if depth == 0 {
                        close = Some(at);
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(close) = close else {
            return Err(format!("`{text}` has an unbalanced parenthesis"));
        };
        if !text[close + 1..].trim().is_empty() {
            return Err(format!("`{text}` has text after its closing parenthesis"));
        }
        let body = &text[1..close];
        if body.trim().is_empty() {
            return Ok(Vec::new());
        }
        let mut parts = Vec::new();
        let mut depth = 0_usize;
        let mut quoted = false;
        let mut from = 0;
        for (at, byte) in body.bytes().enumerate() {
            match byte {
                b'"' => quoted = !quoted,
                b'(' if !quoted => depth += 1,
                b')' if !quoted => depth -= 1,
                b',' if !quoted && depth == 0 => {
                    parts.push(&body[from..at]);
                    from = at + 1;
                }
                _ => {}
            }
        }
        let last = &body[from..];
        if !last.trim().is_empty() {
            parts.push(last);
        }
        Ok(parts)
    }

    #[must_use]
    pub(crate) fn with_literal_identity(raw: &str, blanked: &str) -> Option<String> {
        let bytes = raw.as_bytes();
        let shape = blanked.as_bytes();
        if bytes.len() != shape.len() {
            return None;
        }
        let erased = |from: usize, to: usize| {
            shape
                .get(from..to)
                .is_some_and(|run| run.iter().all(|byte| matches!(byte, b' ' | b'\n')))
        };
        let mut out = String::with_capacity(raw.len());
        let mut at = 0;
        while let Some(&byte) = bytes.get(at) {
            let comment_end = match (byte, bytes.get(at + 1)) {
                (b'/', Some(b'/')) => Some(
                    bytes
                        .get(at..)
                        .and_then(|rest| rest.iter().position(|next| *next == b'\n'))
                        .map_or(bytes.len(), |length| at + length),
                ),
                (b'/', Some(b'*')) => Some(block_comment_end(bytes, at)),
                _ => None,
            };
            if let Some(end) = comment_end {
                if !erased(at, end) || is_doc_comment(bytes, at) {
                    return None;
                }
                out.push(' ');
                at = end;
                continue;
            }
            let literal_end = match byte {
                b'r' | b'b' | b'"' => super::literal_end(bytes, at),
                b'\'' => super::char_literal_end(bytes, at),
                _ => None,
            };
            if let Some(end) = literal_end {
                if !erased(at, end) {
                    return None;
                }
                out.push_str(&literal_token(raw.get(at..end)?));
                at = end;
                continue;
            }
            let character = raw.get(at..)?.chars().next()?;
            let width = character.len_utf8();
            let written = shape.get(at..at + width)?;
            if Some(written) == bytes.get(at..at + width) {
                out.push(character);
            } else if super::is_rustc_whitespace(character) && written.iter().all(|b| *b == b' ') {
                out.push(' ');
            } else {
                return None;
            }
            at += width;
        }
        Some(out)
    }

    fn literal_token(literal: &str) -> String {
        let hex =
            |bytes: &[u8]| -> String { bytes.iter().map(|byte| format!("{byte:02x}")).collect() };
        match string_literal_value(literal) {
            Some(value) if value.bytes().all(is_kept_value_byte) => format!("\"{value}\""),
            Some(value) => format!("\"%{}\"", hex(value.as_bytes())),
            None => format!("\"?{}\"", hex(literal.as_bytes())),
        }
    }

    fn is_kept_value_byte(byte: u8) -> bool {
        byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.')
    }

    #[must_use]
    pub(crate) fn literal_token_value(token: &str) -> Option<String> {
        let inner = token.strip_prefix('"')?.strip_suffix('"')?;
        let Some(hex) = inner.strip_prefix('%') else {
            return inner
                .bytes()
                .all(is_kept_value_byte)
                .then(|| inner.to_owned());
        };
        let bytes = (0..hex.len())
            .step_by(2)
            .map(|at| u8::from_str_radix(hex.get(at..at + 2)?, 16).ok())
            .collect::<Option<Vec<u8>>>()?;
        String::from_utf8(bytes).ok()
    }

    const MOST_RAW_STRING_HASHES: usize = 255;

    fn string_literal_value(literal: &str) -> Option<String> {
        let text = literal.replace("\r\n", "\n");
        if text.contains('\r') {
            return None;
        }
        if let Some(delimited) = text.strip_prefix('r') {
            let content = delimited.trim_start_matches('#');
            let hashes = delimited.len() - content.len();
            let closing = format!("\"{}", "#".repeat(hashes));
            return (hashes <= MOST_RAW_STRING_HASHES)
                .then_some(content)?
                .strip_prefix('"')?
                .strip_suffix(closing.as_str())
                .map(str::to_owned);
        }
        let content = text.strip_prefix('"')?.strip_suffix('"')?;
        let mut value = String::with_capacity(content.len());
        let mut characters = content.chars();
        while let Some(character) = characters.next() {
            match character {
                '"' => return None,
                '\\' => unescape(&mut characters, &mut value)?,
                other => value.push(other),
            }
        }
        Some(value)
    }

    fn unescape(characters: &mut std::str::Chars<'_>, value: &mut String) -> Option<()> {
        let escaped = match characters.next()? {
            'n' => '\n',
            'r' => '\r',
            't' => '\t',
            '\\' => '\\',
            '0' => '\0',
            '\'' => '\'',
            '"' => '"',
            'x' => {
                let high = characters.next()?.to_digit(16)?;
                let low = characters.next()?.to_digit(16)?;
                char::from_u32(high * 16 + low).filter(char::is_ascii)?
            }
            'u' => {
                if characters.next()? != '{' {
                    return None;
                }
                let mut code = 0_u32;
                let mut digits = 0_usize;
                loop {
                    match characters.next()? {
                        '}' if digits > 0 => break,
                        '_' if digits > 0 => {}
                        digit => {
                            code = code * 16 + digit.to_digit(16)?;
                            digits += 1;
                            if digits > 6 {
                                return None;
                            }
                        }
                    }
                }
                char::from_u32(code)?
            }
            '\n' => {
                let rest = characters.as_str();
                let skipped = rest.len() - rest.trim_start_matches([' ', '\t', '\n', '\r']).len();
                *characters = rest.get(skipped..)?.chars();
                return Some(());
            }
            _ => return None,
        };
        value.push(escaped);
        Some(())
    }
}

#[cfg(test)]
pub(crate) mod lint_levels {
    use std::collections::BTreeSet;

    use super::census_domain::{
        Predicate, decide_without_test, parse_predicate, with_literal_identity,
    };
    use super::{
        PREFIXLESS_GROUP_ALIASES, RENAMED_TO_A_GOVERNED_LINT, block_comment_end, is_doc_comment,
        past_comments_and_whitespace,
    };

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct Resolution {
        pub(crate) level: Option<&'static str>,
        pub(crate) refused_downgrade: bool,
        pub(crate) undecided: bool,
    }

    pub(crate) type World = (Option<&'static str>, bool);

    #[must_use]
    pub(crate) fn file_level_lint_resolution(source: &str, lint: &str) -> Resolution {
        let worlds = file_level_lint_worlds(source, lint);
        let mut agreed = worlds.iter();
        match (agreed.next(), agreed.next()) {
            (Some(&(level, refused_downgrade)), None) => Resolution {
                level,
                refused_downgrade,
                undecided: false,
            },
            _ => Resolution {
                level: None,
                refused_downgrade: false,
                undecided: true,
            },
        }
    }

    struct Statement {
        effect: Effect,
        conditions: Vec<String>,
    }

    #[derive(Clone, Copy)]
    enum Effect {
        Level {
            level: &'static str,
            by_a_group: bool,
            recorded: bool,
        },
        Warnings,
    }

    #[derive(Debug)]
    struct Unreadable;

    enum Named {
        TheLint,
        AGroupOfIt,
        Warnings,
        Other,
        Unknown,
    }

    const GROUPS_NAMING_THE_GOVERNED_LINTS: [&str; 2] = ["all", "style"];

    const LINT_TOOLS_NAMING_NO_GOVERNED_LINT: [&str; 2] = ["rustdoc", "rustc"];

    const MOST_UNDECIDED_PREDICATES: usize = 12;

    #[must_use]
    pub(crate) fn file_level_lint_worlds(source: &str, lint: &str) -> BTreeSet<World> {
        let mut worlds = BTreeSet::new();
        let Some(statements) = lint_statements_in_the_prologue(source, lint) else {
            return worlds;
        };
        let variables: Vec<&str> = statements
            .iter()
            .flat_map(|statement| statement.conditions.iter().map(String::as_str))
            .collect::<BTreeSet<&str>>()
            .into_iter()
            .collect();
        if variables.len() > MOST_UNDECIDED_PREDICATES {
            return worlds;
        }
        for assignment in 0_u64..(1_u64 << variables.len()) {
            let holds = |condition: &String| {
                variables
                    .iter()
                    .position(|variable| *variable == condition.as_str())
                    .is_some_and(|index| assignment & (1_u64 << index) != 0)
            };
            let applied = statements
                .iter()
                .filter(|statement| statement.conditions.iter().all(holds));
            let Some(world) = replay(applied) else {
                return BTreeSet::new();
            };
            worlds.insert(world);
        }
        worlds
    }

    fn replay<'a>(statements: impl Iterator<Item = &'a Statement>) -> Option<World> {
        let mut level = None;
        let mut forbidden_by_a_group = false;
        let mut unrecorded = false;
        let mut refused_downgrade = false;
        let mut warnings_stated = false;
        for statement in statements {
            let Effect::Level {
                level: stated,
                by_a_group,
                recorded,
            } = statement.effect
            else {
                warnings_stated = true;
                continue;
            };
            if level == Some("forbid") {
                match stated {
                    "deny" => {}
                    "forbid" => forbidden_by_a_group = by_a_group,
                    _ if forbidden_by_a_group => {
                        level = Some(stated);
                        forbidden_by_a_group = false;
                        unrecorded = stated != "warn" && !recorded;
                    }
                    _ => refused_downgrade = true,
                }
            } else {
                level = Some(stated);
                forbidden_by_a_group = stated == "forbid" && by_a_group;
                unrecorded = matches!(stated, "allow" | "expect") && !recorded;
            }
        }
        if unrecorded || (warnings_stated && matches!(level, None | Some("warn"))) {
            return None;
        }
        let level = if forbidden_by_a_group && level == Some("forbid") {
            Some("deny")
        } else {
            level
        };
        Some((level, refused_downgrade))
    }

    fn lint_statements_in_the_prologue(source: &str, lint: &str) -> Option<Vec<Statement>> {
        let blanked = super::blank_comments_and_strings(source);
        let bytes = source.as_bytes();
        let mut statements = Vec::new();
        let mut at = prologue_start(source);
        loop {
            at = past_inner_doc_comments(source, at);
            if bytes.get(at) != Some(&b'#') {
                break;
            }
            let bang = past_comments_and_whitespace(source, at + 1, false);
            if bytes.get(bang) != Some(&b'!') {
                break;
            }
            let open = past_comments_and_whitespace(source, bang + 1, false);
            if bytes.get(open) != Some(&b'[') {
                return None;
            }
            let close = super::matching(blanked.as_bytes(), open, b'[', b']')?;
            let attribute =
                with_literal_identity(source.get(open + 1..close)?, blanked.get(open + 1..close)?)?;
            let recorded = recorded_by_the_placement_census(source.get(at..=close)?, lint);
            for applied in applied_attributes(attribute.trim()) {
                let stated = stated_effects(applied.text, lint, recorded);
                if stated.as_ref().is_ok_and(Vec::is_empty) {
                    continue;
                }
                let mut conditions = Vec::new();
                let mut in_the_production_build = true;
                for predicate in &applied.under {
                    let Ok(predicate) = predicate else {
                        return None;
                    };
                    match decide_without_test(predicate) {
                        Some(true) => {}
                        Some(false) => in_the_production_build = false,
                        None => conditions.push(predicate.render()),
                    }
                }
                if !in_the_production_build {
                    continue;
                }
                for effect in stated.ok()? {
                    statements.push(Statement {
                        effect,
                        conditions: conditions.clone(),
                    });
                }
            }
            at = close + 1;
        }
        (!an_inner_attribute_follows(source, at)).then_some(statements)
    }

    fn prologue_start(source: &str) -> usize {
        let start = if source.starts_with('\u{feff}') {
            '\u{feff}'.len_utf8()
        } else {
            0
        };
        if !source
            .get(start..)
            .is_some_and(|rest| rest.starts_with("#!"))
        {
            return start;
        }
        let next = past_comments_and_whitespace(source, start + 2, false);
        if source.as_bytes().get(next) == Some(&b'[') {
            return start;
        }
        source
            .get(start..)
            .and_then(|rest| rest.find('\n'))
            .map_or(source.len(), |line| start + line)
    }

    fn past_inner_doc_comments(source: &str, from: usize) -> usize {
        let bytes = source.as_bytes();
        let mut at = past_comments_and_whitespace(source, from, false);
        while is_doc_comment(bytes, at) && bytes.get(at + 2) == Some(&b'!') {
            at = if bytes.get(at + 1) == Some(&b'*') {
                block_comment_end(bytes, at)
            } else {
                source
                    .get(at..)
                    .and_then(|rest| rest.find('\n'))
                    .map_or(source.len(), |line| at + line)
            };
            at = past_comments_and_whitespace(source, at, false);
        }
        at
    }

    fn an_inner_attribute_follows(source: &str, from: usize) -> bool {
        let bytes = source.as_bytes();
        let hash = past_comments_and_whitespace(source, from, true);
        let bang = past_comments_and_whitespace(source, hash + 1, true);
        let open = past_comments_and_whitespace(source, bang + 1, true);
        bytes.get(hash) == Some(&b'#')
            && bytes.get(bang) == Some(&b'!')
            && bytes.get(open) == Some(&b'[')
    }

    fn recorded_by_the_placement_census(attribute: &str, lint: &str) -> bool {
        let governed = lint.rsplit("::").next().unwrap_or(lint);
        super::governed_allows(attribute)
            .iter()
            .flat_map(|allow| allow.lints.iter())
            .any(|named| {
                named == governed || GROUPS_NAMING_THE_GOVERNED_LINTS.contains(&named.as_str())
            })
    }

    fn stated_effects(
        attribute: &str,
        lint: &str,
        recorded: bool,
    ) -> Result<Vec<Effect>, Unreadable> {
        const LEVELS: [&str; 5] = ["allow", "expect", "warn", "deny", "forbid"];
        let (name_end, name) = attribute_name_token(attribute);
        if attribute
            .get(name_end..)
            .is_some_and(|rest| rest.trim_start().starts_with("::"))
        {
            return Err(Unreadable);
        }
        let Some(level) = LEVELS.into_iter().find(|level| *level == name) else {
            return Ok(Vec::new());
        };
        let list = attribute_arguments(attribute).ok_or(Unreadable)?;
        let mut effects = Vec::new();
        let mut reasoned = false;
        for entry in top_level_arguments(list) {
            let entry = entry.trim();
            if entry.is_empty() {
                continue;
            }
            if reasoned {
                return Err(Unreadable);
            }
            if is_a_reason(entry) {
                reasoned = true;
                continue;
            }
            let path = lint_path(entry).ok_or(Unreadable)?;
            match what_a_lint_path_names(&path, lint) {
                Named::TheLint => effects.push(Effect::Level {
                    level,
                    by_a_group: false,
                    recorded,
                }),
                Named::AGroupOfIt => effects.push(Effect::Level {
                    level,
                    by_a_group: true,
                    recorded,
                }),
                Named::Warnings => effects.push(Effect::Warnings),
                Named::Other => {}
                Named::Unknown => return Err(Unreadable),
            }
        }
        Ok(effects)
    }

    fn is_a_reason(entry: &str) -> bool {
        entry
            .strip_prefix("r#")
            .unwrap_or(entry)
            .strip_prefix("reason")
            .map(str::trim_start)
            .and_then(|rest| rest.strip_prefix('='))
            .is_some_and(|value| super::census_domain::literal_token_value(value.trim()).is_some())
    }

    fn lint_path(entry: &str) -> Option<Vec<&str>> {
        let mut segments = Vec::new();
        let mut rest = entry;
        loop {
            let (length, name) = attribute_name_token(rest);
            if name.is_empty() {
                return None;
            }
            segments.push(name);
            rest = rest.get(length..)?.trim_start();
            if rest.is_empty() {
                return Some(segments);
            }
            rest = rest.strip_prefix("::")?.trim_start();
        }
    }

    fn what_a_lint_path_names(path: &[&str], lint: &str) -> Named {
        let governed = lint.rsplit("::").next().unwrap_or(lint);
        let renamed = |old: &str| RENAMED_TO_A_GOVERNED_LINT.contains(&(old, governed));
        match path {
            ["clippy", name] if *name == governed || renamed(name) => Named::TheLint,
            ["clippy", name] if GROUPS_NAMING_THE_GOVERNED_LINTS.contains(name) => {
                Named::AGroupOfIt
            }
            ["clippy", _] => Named::Other,
            [tool, _] if LINT_TOOLS_NAMING_NO_GOVERNED_LINT.contains(tool) => Named::Other,
            ["warnings"] => Named::Warnings,
            [name] if *name == governed => Named::TheLint,
            [name]
                if GROUPS_NAMING_THE_GOVERNED_LINTS.contains(name)
                    || PREFIXLESS_GROUP_ALIASES.contains(name) =>
            {
                Named::AGroupOfIt
            }
            [_] => Named::Other,
            _ => Named::Unknown,
        }
    }

    pub(crate) struct Applied<'a> {
        pub(crate) text: &'a str,
        pub(crate) under: Vec<Result<Predicate, String>>,
    }

    #[must_use]
    pub(crate) fn applied_attributes(attribute: &str) -> Vec<Applied<'_>> {
        let mut into = Vec::new();
        applied_under(attribute, &mut Vec::new(), &mut into);
        into
    }

    fn applied_under<'a>(
        attribute: &'a str,
        under: &mut Vec<Result<Predicate, String>>,
        into: &mut Vec<Applied<'a>>,
    ) {
        if attribute_name(attribute) != "cfg_attr" {
            into.push(Applied {
                text: attribute,
                under: under.clone(),
            });
            return;
        }
        let Some(body) = attribute_arguments(attribute) else {
            return;
        };
        let mut arguments = top_level_arguments(body).into_iter();
        under.push(parse_predicate(arguments.next().unwrap_or_default().trim()));
        for argument in arguments {
            applied_under(argument.trim(), under, into);
        }
        under.pop();
    }

    #[must_use]
    pub(crate) fn attribute_name(attribute: &str) -> &str {
        attribute_name_token(attribute).1
    }

    fn attribute_name_token(attribute: &str) -> (usize, &str) {
        let is_identifier = |character: char| character.is_alphanumeric() || character == '_';
        let from = if attribute
            .strip_prefix("r#")
            .is_some_and(|rest| rest.starts_with(is_identifier))
        {
            2
        } else {
            0
        };
        let rest = attribute.get(from..).unwrap_or_default();
        let length = rest
            .find(|character: char| !is_identifier(character))
            .unwrap_or(rest.len());
        (from + length, rest.get(..length).unwrap_or_default())
    }

    #[must_use]
    pub(crate) fn attribute_arguments(attribute: &str) -> Option<&str> {
        attribute
            .get(attribute_name_token(attribute).0..)
            .map(str::trim_start)
            .and_then(|rest| rest.strip_prefix('('))
            .and_then(|body| body.strip_suffix(')'))
    }

    #[must_use]
    pub(crate) fn top_level_arguments(body: &str) -> Vec<&str> {
        let mut parts = Vec::new();
        let mut depth = 0_usize;
        let mut quoted = false;
        let mut from = 0;
        for (at, byte) in body.bytes().enumerate() {
            match byte {
                b'"' => quoted = !quoted,
                b'(' | b'[' | b'{' if !quoted => depth += 1,
                b')' | b']' | b'}' if !quoted => depth = depth.saturating_sub(1),
                b',' if !quoted && depth == 0 => {
                    parts.push(body.get(from..at).unwrap_or_default());
                    from = at + 1;
                }
                _ => {}
            }
        }
        parts.push(body.get(from..).unwrap_or_default());
        parts
    }

    #[must_use]
    pub(crate) fn leading_inner_attributes(source: &str) -> &str {
        let blanked = super::blank_comments_and_strings(source);
        let bytes = blanked.as_bytes();
        let mut end = 0;
        while let Some(close) =
            super::attribute_open(source, bytes, super::past_whitespace(bytes, end))
                .filter(|(inner, _)| *inner)
                .and_then(|(_, open)| super::matching(bytes, open, b'[', b']'))
        {
            end = close + 1;
        }
        source.get(..end).unwrap_or_default()
    }

    #[must_use]
    pub(crate) fn file_level_lint_state(source: &str, lint: &str) -> Option<&'static str> {
        file_level_lint_resolution(source, lint).level
    }
}

#[cfg(test)]
pub(crate) mod tests;
