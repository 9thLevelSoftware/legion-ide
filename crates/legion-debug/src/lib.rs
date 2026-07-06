//! Debug adapter client runtime.

#![warn(missing_docs)]

/// DAP client runtime and metadata projections.
pub mod dap;
/// Evidence projection helpers for debug adapter and test run artifacts.
pub mod evidence;
/// DAP lifecycle state model.
pub mod state;

pub use dap::{DapClientConfig, DapClientError, DapClientOutcome, DapClientRuntime};
pub use evidence::{
    EvidenceProjectionError, debug_adapter_audit_evidence, debug_adapter_audit_summary,
    test_run_summary_evidence, test_run_summary_text,
};
pub use state::DapLifecycleState;
