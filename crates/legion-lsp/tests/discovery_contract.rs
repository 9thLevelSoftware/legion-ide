use std::path::PathBuf;

use legion_lsp::{DiscoveredBinary, RustAnalyzerDiscovery};
use legion_protocol::LspServerBinaryProvenance;

#[test]
fn configured_path_wins_over_everything() {
    let d = RustAnalyzerDiscovery {
        configured_path: Some(PathBuf::from("/cfg/rust-analyzer")),
        project_local_path: Some(PathBuf::from("/proj/rust-analyzer")),
        bundled_path: Some(PathBuf::from("/bundle/rust-analyzer")),
        path_env: Some("/usr/bin".into()),
    };
    match d.resolve() {
        DiscoveredBinary::Found { path, provenance } => {
            assert_eq!(path, PathBuf::from("/cfg/rust-analyzer"));
            assert_eq!(provenance, LspServerBinaryProvenance::Configured);
        }
        DiscoveredBinary::NotFound => panic!("expected configured path"),
    }
}

#[test]
fn empty_discovery_is_not_found() {
    let d = RustAnalyzerDiscovery {
        configured_path: None,
        project_local_path: None,
        bundled_path: None,
        path_env: Some(String::new()),
    };
    assert!(matches!(d.resolve(), DiscoveredBinary::NotFound));
}
