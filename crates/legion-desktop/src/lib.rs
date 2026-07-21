//! Renderer-backed desktop adapter for Legion IDE.
//!
//! This crate owns native windowing, renderer resources, and adapter-local
//! presentation state. Product state remains owned by `legion-app` and
//! projection/intent contracts remain owned by `legion-ui` and `legion-protocol`.

#![warn(missing_docs)]

pub mod beta;
pub mod bridge;
/// Honest cut-line / fixture status copy for deferred or simulated surfaces (Tier 0).
pub mod cut_lines;
pub mod diagnostics;
pub mod harness;
pub mod health;
pub mod manual_perf;
pub mod metrics;
pub mod package;
pub mod platform;
pub mod search;
pub mod session;
pub mod smoke;
mod theme;
pub mod view;
pub mod workflow;
