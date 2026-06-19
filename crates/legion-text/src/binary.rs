//! Binary file detection heuristic for the Legion IDE text layer.
//!
//! This module implements the same NUL-byte scan strategy used by `git` and `grep` to decide
//! whether a file should be treated as binary or text. The first [`BINARY_DETECTION_WINDOW_BYTES`]
//! bytes of content are scanned for a NUL (`\0`) byte; if one is found the file is classified as
//! binary, otherwise it is treated as text.
//!
//! The heuristic is intentionally simple and fast: real binary formats (ELF, PE, PNG, PDF, ZIP,
//! etc.) almost always contain NUL bytes in their headers, while valid UTF-8 text files never
//! contain NUL bytes (the NUL code point is encoded as two bytes `0xC0 0x80` in modified UTF-8
//! and is exceedingly rare even in raw UTF-8 text files). The window avoids spending O(n) time on
//! very large files while still catching every common binary format.

use memchr::memchr;

/// Number of bytes inspected by the default binary detection window.
///
/// Matches the 8 KiB heuristic used by `git diff --stat` and GNU `grep`.
pub const BINARY_DETECTION_WINDOW_BYTES: usize = 8192;

/// Result of a binary-detection scan on a byte slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryDetectionResult {
    /// No NUL byte was found within the inspection window — the content is treated as text.
    Text,
    /// A NUL byte was found within the inspection window at the given offset.
    Binary {
        /// Byte offset of the first NUL byte within the scanned window.
        first_nul_offset: usize,
    },
}

impl BinaryDetectionResult {
    /// Returns `true` if the content was classified as binary.
    pub fn is_binary(self) -> bool {
        matches!(self, BinaryDetectionResult::Binary { .. })
    }

    /// Returns `true` if the content was classified as text.
    pub fn is_text(self) -> bool {
        matches!(self, BinaryDetectionResult::Text)
    }
}

/// Detect whether `bytes` should be treated as a binary file.
///
/// Scans the first [`BINARY_DETECTION_WINDOW_BYTES`] bytes for a NUL (`\0`) character.
/// Returns [`BinaryDetectionResult::Binary`] with the offset of the first NUL byte if one is
/// found, or [`BinaryDetectionResult::Text`] otherwise.
///
/// # Examples
///
/// ```
/// use legion_text::binary::{detect_binary, BinaryDetectionResult};
///
/// assert_eq!(detect_binary(b"hello, world"), BinaryDetectionResult::Text);
/// assert_eq!(
///     detect_binary(b"\x00binary"),
///     BinaryDetectionResult::Binary { first_nul_offset: 0 }
/// );
/// ```
pub fn detect_binary(bytes: &[u8]) -> BinaryDetectionResult {
    detect_binary_with_window(bytes, BINARY_DETECTION_WINDOW_BYTES)
}

/// Detect whether `bytes` should be treated as a binary file using an explicit window size.
///
/// Scans up to `window_bytes` bytes for a NUL (`\0`) character.
/// Returns [`BinaryDetectionResult::Binary`] with the offset of the first NUL byte if one is
/// found within the window, or [`BinaryDetectionResult::Text`] otherwise.
///
/// Use [`detect_binary`] for the standard 8 KiB window.
pub fn detect_binary_with_window(bytes: &[u8], window_bytes: usize) -> BinaryDetectionResult {
    let window = if bytes.len() > window_bytes {
        &bytes[..window_bytes]
    } else {
        bytes
    };

    match memchr(0, window) {
        Some(offset) => BinaryDetectionResult::Binary {
            first_nul_offset: offset,
        },
        None => BinaryDetectionResult::Text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_content_is_text() {
        assert_eq!(detect_binary(b""), BinaryDetectionResult::Text);
    }

    #[test]
    fn ascii_text_is_text() {
        assert_eq!(
            detect_binary(b"Hello, world! This is plain ASCII text.\n"),
            BinaryDetectionResult::Text
        );
    }

    #[test]
    fn utf8_text_is_text() {
        let utf8 = "Hello, 世界! Привет мир! 🦀".as_bytes();
        assert_eq!(detect_binary(utf8), BinaryDetectionResult::Text);
    }

    #[test]
    fn nul_byte_at_start_is_binary() {
        assert_eq!(
            detect_binary(b"\x00some data after"),
            BinaryDetectionResult::Binary {
                first_nul_offset: 0
            }
        );
    }

    #[test]
    fn nul_byte_in_middle_is_binary() {
        assert_eq!(
            detect_binary(b"before\x00after"),
            BinaryDetectionResult::Binary {
                first_nul_offset: 6
            }
        );
    }

    #[test]
    fn nul_byte_beyond_window_is_text() {
        // 8192 bytes of 'a' then a NUL — the NUL is outside the detection window.
        let mut data = vec![b'a'; BINARY_DETECTION_WINDOW_BYTES];
        data.push(b'\0');
        assert_eq!(detect_binary(&data), BinaryDetectionResult::Text);
    }

    #[test]
    fn nul_byte_at_window_boundary_is_text() {
        // NUL placed at index 8191 — the last byte scanned by the window (indices 0..8192).
        // The window slice is bytes[..8192], so index 8191 IS included.
        let mut data = vec![b'a'; BINARY_DETECTION_WINDOW_BYTES];
        data[BINARY_DETECTION_WINDOW_BYTES - 1] = b'\0';
        assert_eq!(
            detect_binary(&data),
            BinaryDetectionResult::Binary {
                first_nul_offset: BINARY_DETECTION_WINDOW_BYTES - 1
            }
        );
    }

    #[test]
    fn custom_window_detects_within_range() {
        // NUL at offset 5, window of 10 — should be detected.
        let data = b"hello\x00world!!";
        assert_eq!(
            detect_binary_with_window(data, 10),
            BinaryDetectionResult::Binary {
                first_nul_offset: 5
            }
        );

        // Same data but window of 4 — NUL is outside, should be text.
        assert_eq!(
            detect_binary_with_window(data, 4),
            BinaryDetectionResult::Text
        );
    }

    #[test]
    fn simulated_png_header_is_binary() {
        // Standard PNG magic bytes: 0x89 'P' 'N' 'G' \r \n 0x1A \n followed by NUL in IHDR chunk.
        let png_start: &[u8] = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00];
        assert!(detect_binary(png_start).is_binary());
    }

    #[test]
    fn simulated_elf_header_is_binary() {
        // ELF magic: 0x7F 'E' 'L' 'F' followed by class, data, version, OS/ABI (0x00).
        let elf_start: &[u8] = &[0x7F, 0x45, 0x4C, 0x46, 0x02, 0x01, 0x01, 0x00];
        assert!(detect_binary(elf_start).is_binary());
    }
}
