//! Shared completion-record digest syntax.
//!
//! Evidence, candidate, outcome, and identity-receipt validators must agree
//! on what a SHA-256 looks like. Hex case is accepted; callers compare with
//! `eq_ignore_ascii_case` when matching two digests.

/// Return whether `value` is a 64-character hexadecimal SHA-256.
pub(crate) fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Return whether `value` is a 40-character lowercase hexadecimal git SHA.
pub(crate) fn is_git_sha(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
