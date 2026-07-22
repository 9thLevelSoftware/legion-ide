//! Debug adapter client runtime.

#![warn(missing_docs)]

/// Live adapter binary resolution (env + PATH + optional fake).
pub mod adapter_resolve;
/// DAP client runtime and metadata projections (fixture path by default).
pub mod dap;
/// Evidence projection helpers for debug adapter and test run artifacts.
pub mod evidence;
/// DAP stdio Microsoft DAP framing (Content-Length).
pub mod framing;
/// Live adapter process session (B1/B2; CI fake adapter).
pub mod live_session;
/// DAP lifecycle state model.
pub mod state;

pub use adapter_resolve::{
    DapMode, ResolvedAdapter, dogfood_requires_system_adapter, resolve_live_adapter,
    resolve_system_adapter,
};
pub use dap::{DapClientConfig, DapClientError, DapClientOutcome, DapClientRuntime};
pub use evidence::{
    EvidenceProjectionError, debug_adapter_audit_evidence, debug_adapter_audit_summary,
    test_run_summary_evidence, test_run_summary_text,
};
pub use framing::{DapFrameError, DapFramer, DapMessage};
pub use live_session::{
    LiveBreakpoint, LiveDapHandshakeOutcome, LiveDapSession, LiveDapSessionError,
    LiveDapStopOutcome, LiveStackFrame, LiveVariable, fake_dap_adapter_path,
};
pub use state::DapLifecycleState;
