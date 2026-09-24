//! Pinned npm archive descriptors approved for downloaded language servers.
//!
//! Extracted verbatim from `lib.rs` so a descriptor, its policy gate and the
//! supply-chain predicates that guard it live together rather than inside the
//! runtime module. Every public item here is re-exported from the crate root,
//! so no caller path changes.

use std::path::PathBuf;

use crate::{LspArtifactRuntime, LspDownloadedArtifactMetadata, LspNodeVersion};

/// Pinned identity of one npm archive approved for a language server.
///
/// Every field is exact: a released version rather than a range or dist-tag,
/// and a SHA-256 recomputed from the retained archive rather than transcribed
/// from a registry manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LspPinnedArchive {
    /// Package name recorded by the package manifest.
    pub package_name: &'static str,
    /// Exact pinned release.
    pub version: &'static str,
    /// Archive URL this exact release is fetched from.
    pub archive_url: &'static str,
    /// SHA-256 of the archive.
    pub checksum_sha256: &'static str,
    /// Archive format (for example, `tar.gz`).
    pub archive_format: &'static str,
    /// Relative package root inside the materialized artifact directory.
    pub package_root: &'static str,
    /// Relative executable entrypoint below `package_root`.
    pub entrypoint: &'static str,
    /// Minimum Node runtime declared by the package.
    pub minimum_node: LspNodeVersion,
}

impl LspPinnedArchive {
    /// Returns the owned packaging metadata described by this pinned archive.
    pub fn metadata(&self) -> LspDownloadedArtifactMetadata {
        LspDownloadedArtifactMetadata {
            package_name: self.package_name.to_string(),
            version: self.version.to_string(),
            archive_format: self.archive_format.to_string(),
            package_root: PathBuf::from(self.package_root),
            entrypoint: PathBuf::from(self.entrypoint),
            runtime: LspArtifactRuntime::Node {
                minimum_version: self.minimum_node,
            },
        }
    }
}

/// The approved `typescript-language-server` release for the tier-two
/// registry. The digest is the SHA-256 of the retained
/// `typescript-language-server-6.0.0.tgz` (515,598 bytes) and the Node minimum
/// is the `engines.node` constraint declared by that same release.
pub const TYPESCRIPT_LANGUAGE_SERVER_ARCHIVE: LspPinnedArchive = LspPinnedArchive {
    package_name: "typescript-language-server",
    version: "6.0.0",
    archive_url: "https://registry.npmjs.org/typescript-language-server/-/typescript-language-server-6.0.0.tgz",
    checksum_sha256: "6e23b48efc76af4e70928cdfe62ea6e6cfef67ab4c1e7579c4e82dd284fbdfd2",
    archive_format: "tar.gz",
    package_root: "package",
    entrypoint: "lib/cli.mjs",
    minimum_node: LspNodeVersion {
        major: 22,
        minor: 22,
        patch: 2,
    },
};

/// The peer `typescript` compiler release whose `lib/tsserver.js` the pinned
/// language server drives.
///
/// [`LspDownloadedArtifactMetadata`] describes exactly one archive and one
/// entrypoint, so the registry has no first-class representation for a peer
/// archive. This constant records the peer's pinned identity as its own
/// descriptor so a caller can verify it; it is deliberately not folded into
/// the language server's descriptor, which would misreport two archives as
/// one.
pub const TYPESCRIPT_COMPILER_ARCHIVE: LspPinnedArchive = LspPinnedArchive {
    package_name: "typescript",
    version: "6.0.3",
    archive_url: "https://registry.npmjs.org/typescript/-/typescript-6.0.3.tgz",
    checksum_sha256: "33cd0ee1beaa8c9e9d15a9da836c62ddea4c34a42d7c2d349dbc80d94165d22a",
    archive_format: "tar.gz",
    package_root: "package",
    entrypoint: "lib/tsserver.js",
    minimum_node: LspNodeVersion {
        major: 14,
        minor: 17,
        patch: 0,
    },
};

/// Policy gate authorizing the pinned TypeScript language-server download.
pub const TYPESCRIPT_LANGUAGE_SERVER_POLICY_GATE: &str =
    "policy://lsp-download/typescript-language-server";

// Visibility widened from private to `pub(crate)` by the extraction. This is
// the one character-level deviation from the pure move and it is forced:
// `resolve_downloaded_process` still lives in `lib.rs` and must call this
// predicate. The item stays out of the crate's public API.
pub(crate) fn is_sha256_digest(value: &str) -> bool {
    value.len() == 64
        && value.bytes().all(|byte| {
            byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte) || (b'A'..=b'F').contains(&byte)
        })
}

/// Returns whether `value` names one exact release rather than a range or a
/// dist-tag.
///
/// `6.0.0` and `6.0.0-rc.1` are exact pins. `^6.0.0`, `~6.0`, `>=6.0.0`,
/// `1.x`, `6.0`, `*`, `latest` and `next` are not: each can resolve to a
/// different archive over time, so no digest recorded beside them is
/// verifiable.
pub fn is_exact_pinned_version(value: &str) -> bool {
    let (core, suffix) = match value.find(['-', '+']) {
        Some(index) => (&value[..index], Some(&value[index + 1..])),
        None => (value, None),
    };
    let mut components = core.split('.');
    for _ in 0..3 {
        let Some(component) = components.next() else {
            return false;
        };
        if component.is_empty() || !component.bytes().all(|byte| byte.is_ascii_digit()) {
            return false;
        }
    }
    if components.next().is_some() {
        return false;
    }
    match suffix {
        None => true,
        Some(suffix) => {
            !suffix.is_empty()
                && suffix.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'-' || byte == b'+'
                })
        }
    }
}
