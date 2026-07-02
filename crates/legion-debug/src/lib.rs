//! Debug adapter client runtime.

#![warn(missing_docs)]

/// DAP client runtime and metadata projections.
pub mod dap;
/// DAP lifecycle state model.
pub mod state;

pub use dap::{DapClientConfig, DapClientError, DapClientOutcome, DapClientRuntime};
pub use state::DapLifecycleState;
