//! Renderer-backed desktop adapter for Devil IDE.
//!
//! This crate owns native windowing, renderer resources, and adapter-local
//! presentation state. Product state remains owned by `devil-app` and
//! projection/intent contracts remain owned by `devil-ui` and `devil-protocol`.

#![warn(missing_docs)]

pub mod bridge;
pub mod metrics;
pub mod search;
pub mod session;
pub mod smoke;
pub mod view;
pub mod workflow;
