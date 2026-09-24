//! Host bindings generated from the plugin WIT ABI in `crates/legion-plugin/wit/`.
//!
//! `wasmtime::component::bindgen!` parses the WIT package at compile time and
//! emits the host side of the `legion:plugin/plugin-host` world: a `Host` trait
//! per imported interface (`grammars`, `themes`, `lsp`), the record types those
//! interfaces exchange, and `PluginHost::add_to_linker` / `instantiate`. A
//! malformed or drifting `.wit` file is therefore a compile error in this crate,
//! not a runtime surprise.
//!
//! The generated items carry no rustdoc, so `missing_docs` is silenced for this
//! module only; the WIT files themselves are the documentation of record.
#![allow(missing_docs)]

wasmtime::component::bindgen!({
    path: "wit",
    world: "plugin-host",
});
