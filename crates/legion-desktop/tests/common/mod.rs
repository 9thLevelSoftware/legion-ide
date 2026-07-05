//! Shared helpers for desktop integration tests.
//!
//! These utilities back the architectural-boundary tests that assert a module's
//! source does (or does not) reference a given symbol. Raw `str::contains`
//! checks are brittle: they match inside doc comments, string literals, and as
//! substrings of unrelated identifiers (e.g. `legion_app` inside
//! `legion_application`, or `EditorEngine` inside `EditorEngineProxy`). The
//! helpers here strip comments and string/raw-string literals first, then match
//! on whole identifier tokens only, so the boundary checks fail only on a real
//! source reference.

// Each test binary that includes this module only uses a subset of the helpers,
// so suppress the per-binary dead-code warnings the shared-module pattern emits.
#![allow(dead_code)]

/// Returns `source` with line comments, block comments (nested), and
/// double-quoted/raw string literals replaced by spaces. Char literals and
/// lifetimes are left intact (they cannot contain the multi-character
/// identifiers these tests scan for, so they never produce false positives).
pub fn strip_comments_and_strings(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut out = String::with_capacity(source.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        let next = bytes.get(i + 1).copied();

        // Line comment: // ... \n
        if b == b'/' && next == Some(b'/') {
            while i < bytes.len() && bytes[i] != b'\n' {
                out.push(' ');
                i += 1;
            }
            continue;
        }

        // Block comment (supports nesting): /* ... */
        if b == b'/' && next == Some(b'*') {
            let mut depth = 1;
            out.push(' ');
            out.push(' ');
            i += 2;
            while i < bytes.len() && depth > 0 {
                if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'*') {
                    depth += 1;
                    out.push(' ');
                    out.push(' ');
                    i += 2;
                } else if bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/') {
                    depth -= 1;
                    out.push(' ');
                    out.push(' ');
                    i += 2;
                } else {
                    if bytes[i] == b'\n' {
                        out.push('\n');
                    } else {
                        out.push(' ');
                    }
                    i += 1;
                }
            }
            continue;
        }

        // Raw string literal: r"...", r#"..."#, r##"..."##, ...
        if b == b'r' && matches!(next, Some(b'"') | Some(b'#')) {
            let mut j = i + 1;
            let mut hashes = 0;
            while j < bytes.len() && bytes[j] == b'#' {
                hashes += 1;
                j += 1;
            }
            if j < bytes.len() && bytes[j] == b'"' {
                // Confirmed raw string opener.
                out.push(' '); // r
                for _ in 0..hashes {
                    out.push(' ');
                }
                out.push(' '); // opening quote
                j += 1;
                loop {
                    if j >= bytes.len() {
                        break;
                    }
                    if bytes[j] == b'"' {
                        let mut k = j + 1;
                        let mut closing = 0;
                        while k < bytes.len() && bytes[k] == b'#' && closing < hashes {
                            closing += 1;
                            k += 1;
                        }
                        if closing == hashes {
                            out.push(' '); // closing quote
                            for _ in 0..hashes {
                                out.push(' ');
                            }
                            j = k;
                            break;
                        }
                    }
                    if bytes[j] == b'\n' {
                        out.push('\n');
                    } else {
                        out.push(' ');
                    }
                    j += 1;
                }
                i = j;
                continue;
            }
            // Not a raw string (e.g. an identifier starting with `r`): fall through.
        }

        // Regular string literal: "..." with \" escapes.
        if b == b'"' {
            out.push(' ');
            i += 1;
            while i < bytes.len() {
                if bytes[i] == b'\\' {
                    out.push(' ');
                    if i + 1 < bytes.len() {
                        out.push(' ');
                    }
                    i += 2;
                    continue;
                }
                if bytes[i] == b'"' {
                    out.push(' ');
                    i += 1;
                    break;
                }
                if bytes[i] == b'\n' {
                    out.push('\n');
                } else {
                    out.push(' ');
                }
                i += 1;
            }
            continue;
        }

        // Default: copy the byte. Source is UTF-8; multibyte chars are copied
        // byte-for-byte which is safe because we only compare ASCII identifiers.
        out.push(b as char);
        i += 1;
    }
    out
}

fn is_ident_byte(b: u8) -> bool {
    b == b'_' || b.is_ascii_alphanumeric()
}

/// True if `ident` appears in `source` as a whole identifier token, ignoring
/// occurrences inside comments and string literals.
pub fn source_uses_identifier(source: &str, ident: &str) -> bool {
    assert!(!ident.is_empty(), "identifier must not be empty");
    let stripped = strip_comments_and_strings(source);
    let hay = stripped.as_bytes();
    let needle = ident.as_bytes();
    let mut start = 0;
    while let Some(pos) = find_from(hay, needle, start) {
        let before_ok = pos == 0 || !is_ident_byte(hay[pos - 1]);
        let after_idx = pos + needle.len();
        let after_ok = after_idx >= hay.len() || !is_ident_byte(hay[after_idx]);
        if before_ok && after_ok {
            return true;
        }
        start = pos + 1;
    }
    false
}

fn find_from(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.len() > hay.len() {
        return None;
    }
    let mut i = from;
    while i + needle.len() <= hay.len() {
        if &hay[i..i + needle.len()] == needle {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Asserts the given source references none of the forbidden identifiers (as
/// whole tokens), reporting the offending symbol on failure.
pub fn assert_source_excludes(source: &str, label: &str, forbidden: &[&str]) {
    for symbol in forbidden {
        assert!(
            !source_uses_identifier(source, symbol),
            "{label} must not reference `{symbol}` (architectural boundary)"
        );
    }
}

/// Asserts the given source references the identifier (as a whole token).
pub fn assert_source_includes(source: &str, label: &str, symbol: &str) {
    assert!(
        source_uses_identifier(source, symbol),
        "{label} should reference `{symbol}`"
    );
}
