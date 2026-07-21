//! Debug adapter client runtime.

#![warn(missing_docs)]

/// DAP client runtime and metadata projections (fixture path by default).
pub mod dap;
/// Evidence projection helpers for debug adapter and test run artifacts.
pub mod evidence;
/// DAP stdio JSON-RPC framing (Content-Length).
pub mod framing;
/// Live adapter process session (B1 scaffold; CI fake adapter).
pub mod live_session;
/// DAP lifecycle state model.
pub mod state;

pub use dap::{DapClientConfig, DapClientError, DapClientOutcome, DapClientRuntime};
pub use evidence::{
    EvidenceProjectionError, debug_adapter_audit_evidence, debug_adapter_audit_summary,
    test_run_summary_evidence, test_run_summary_text,
};
pub use framing::{DapFrameError, DapFramer, DapJsonRpc};
pub use live_session::{
    LiveDapHandshakeOutcome, LiveDapSession, LiveDapSessionError, fake_dap_adapter_path,
};
pub use state::DapLifecycleState;
