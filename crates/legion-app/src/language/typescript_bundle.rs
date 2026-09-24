//! Offline TypeScript language-server/compiler bundle binding.

use std::path::PathBuf;

use legion_lsp::{LspArtifactRuntime, LspDownloadedArtifactMetadata, LspNodeVersion};

use super::ArtifactDescriptor;

/// Pinned TypeScript language-server package version.
pub const TYPESCRIPT_LANGUAGE_SERVER_VERSION: &str = "6.0.0";
/// Pinned TypeScript compiler package version.
pub const TYPESCRIPT_COMPILER_VERSION: &str = "6.0.3";
/// SHA-256 of the pinned language-server archive.
pub const TYPESCRIPT_LANGUAGE_SERVER_SHA256: &str =
    "6e23b48efc76af4e70928cdfe62ea6e6cfef67ab4c1e7579c4e82dd284fbdfd2";
/// SHA-256 of the pinned compiler archive.
pub const TYPESCRIPT_COMPILER_SHA256: &str =
    "33cd0ee1beaa8c9e9d15a9da836c62ddea4c34a42d7c2d349dbc80d94165d22a";

/// Pinned offline TypeScript language-server and compiler metadata.
#[derive(Debug, Clone)]
pub struct TypeScriptBundleDescriptor {
    /// Verified language-server archive descriptor.
    pub server: ArtifactDescriptor,
    /// Verified TypeScript compiler archive descriptor.
    pub compiler: ArtifactDescriptor,
    /// Relative compiler entrypoint inside the materialized compiler package.
    pub tsserver_entrypoint: PathBuf,
}

impl TypeScriptBundleDescriptor {
    /// Returns the development-pinned server/compiler descriptors.
    pub fn pinned() -> Self {
        let node = LspArtifactRuntime::Node {
            minimum_version: LspNodeVersion {
                major: 22,
                minor: 22,
                patch: 2,
            },
        };
        Self {
            server: ArtifactDescriptor::from_metadata(
                "typescript-language-server",
                TYPESCRIPT_LANGUAGE_SERVER_SHA256,
                &LspDownloadedArtifactMetadata {
                    package_name: "typescript-language-server".into(),
                    version: TYPESCRIPT_LANGUAGE_SERVER_VERSION.into(),
                    archive_format: "tar.gz".into(),
                    package_root: "package".into(),
                    entrypoint: "lib/cli.mjs".into(),
                    runtime: node.clone(),
                },
            ),
            compiler: ArtifactDescriptor::from_metadata(
                "typescript",
                TYPESCRIPT_COMPILER_SHA256,
                &LspDownloadedArtifactMetadata {
                    package_name: "typescript".into(),
                    version: TYPESCRIPT_COMPILER_VERSION.into(),
                    archive_format: "tar.gz".into(),
                    package_root: "package".into(),
                    entrypoint: "lib/tsserver.js".into(),
                    runtime: node,
                },
            ),
            tsserver_entrypoint: "lib/tsserver.js".into(),
        }
    }

    /// Builds the bounded TypeScript server initialization options.
    pub fn initialization_options(&self, compiler_path: &str) -> serde_json::Value {
        serde_json::json!({
            "disableAutomaticTypingAcquisition": true,
            "tsserver": {
                "path": compiler_path,
                "logVerbosity": "off",
                "trace": "off"
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinned_bundle_has_distinct_server_compiler_receipts() {
        let bundle = TypeScriptBundleDescriptor::pinned();
        assert_ne!(
            bundle.server.expected_sha256,
            bundle.compiler.expected_sha256
        );
        assert_eq!(bundle.tsserver_entrypoint, PathBuf::from("lib/tsserver.js"));
        let options = bundle.initialization_options("/cache/package/lib/tsserver.js");
        assert_eq!(options["disableAutomaticTypingAcquisition"], true);
        assert_eq!(options["tsserver"]["logVerbosity"], "off");
        assert_eq!(options["tsserver"]["trace"], "off");
    }
}
