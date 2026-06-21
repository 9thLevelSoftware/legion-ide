//! Language-tooling orchestration extracted from `lib.rs` (design §10).
//!
//! This module provides the capability-gated decision layer for LSP tooling
//! lifecycle operations such as rust-analyzer binary acquisition.

mod download;
pub use download::{
    DownloadDecision, RustAnalyzerDownloadRequest, evaluate_rust_analyzer_download,
    verify_downloaded_artifact,
};
