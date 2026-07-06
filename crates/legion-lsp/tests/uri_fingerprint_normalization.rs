//! URI-fingerprint normalization contract (PKT-S3-WEDGE-R3 root cause #1).
//!
//! rust-analyzer echoes document URIs in its own canonical form: on Windows
//! a document opened as `file:///C:/...` comes back in `publishDiagnostics`
//! as `file:///c:/...` (lowercase drive; percent-encoded `%3A` forms also
//! exist in the wild via lsp-types' Url). The fingerprint used to correlate
//! "the URI we opened" with "the URI the server echoed" hashed the RAW
//! string, so the uppercase and lowercase forms never matched: diagnostics
//! sat in the notification buffer while every pump filter reported silence —
//! the GP-1 s3 "wedge", and a product bug (ingest dropped diagnostics for
//! open buffers). The fingerprint must be stable across drive-designator
//! forms while preserving path-component case (real identity on
//! case-sensitive filesystems).

use legion_lsp::lsp_diagnostic_uri_fingerprint as fingerprint;

#[test]
fn windows_drive_designator_forms_hash_equal() {
    let upper = fingerprint("file:///C:/Users/dev/src/main.rs");
    assert_eq!(
        upper,
        fingerprint("file:///c:/Users/dev/src/main.rs"),
        "lowercase drive letter (rust-analyzer's echoed form) must match"
    );
    assert_eq!(
        upper,
        fingerprint("file:///C%3A/Users/dev/src/main.rs"),
        "percent-encoded drive colon must match"
    );
    assert_eq!(
        upper,
        fingerprint("file:///c%3A/Users/dev/src/main.rs"),
        "lowercase percent-encoded form (VS Code's canonical form) must match"
    );
}

#[test]
fn path_component_case_still_distinguishes_documents() {
    assert_ne!(
        fingerprint("file:///C:/Users/dev/src/Main.rs"),
        fingerprint("file:///C:/Users/dev/src/main.rs"),
        "only the drive designator is normalized; path case is identity on \
         case-sensitive filesystems"
    );
}

#[test]
fn unix_file_uris_are_untouched() {
    assert_ne!(
        fingerprint("file:///tmp/ws/src/main.rs"),
        fingerprint("file:///Tmp/ws/src/main.rs"),
        "a leading path segment that merely looks alphabetic must not be \
         treated as a drive letter"
    );
}

#[test]
fn non_file_uris_are_untouched() {
    assert_ne!(
        fingerprint("untitled:Untitled-1"),
        fingerprint("untitled:untitled-1")
    );
}
