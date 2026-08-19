//! Proof that the plugin WIT ABI compiles, generates host bindings, and that
//! those bindings actually carry a real component's contributions to the host.
//!
//! `legion_plugin::wit_bindings` runs `wasmtime::component::bindgen!` over
//! `crates/legion-plugin/wit/`, so this test file failing to *compile* is itself
//! a signal: it means the WIT no longer produces the host surface it claims to.

use std::path::PathBuf;

use legion_plugin::wit_bindings::{
    PluginHost,
    legion::plugin::{
        grammars::GrammarContribution, lsp::LspAdapterContribution, themes::ThemeContribution,
    },
};
use wasmtime::{
    Engine, Store,
    component::{Component, HasSelf, Linker},
};

/// Everything a guest registered, in call order, recorded verbatim.
#[derive(Default)]
struct RecordingHost {
    grammars: Vec<GrammarContribution>,
    themes: Vec<ThemeContribution>,
    lsp_adapters: Vec<LspAdapterContribution>,
}

impl legion_plugin::wit_bindings::legion::plugin::grammars::Host for RecordingHost {
    fn register_grammar(&mut self, contribution: GrammarContribution) {
        self.grammars.push(contribution);
    }
}

impl legion_plugin::wit_bindings::legion::plugin::themes::Host for RecordingHost {
    fn register_theme(&mut self, contribution: ThemeContribution) {
        self.themes.push(contribution);
    }
}

impl legion_plugin::wit_bindings::legion::plugin::lsp::Host for RecordingHost {
    fn register_lsp_adapter(&mut self, contribution: LspAdapterContribution) {
        self.lsp_adapters.push(contribution);
    }
}

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("abi")
        .join("contributions.wat")
}

/// Instantiate the hand-written fixture component against the generated host
/// bindings and run its `activate` export.
fn activate_fixture() -> RecordingHost {
    let binary = wat::parse_file(fixture_path()).expect("fixture parses as a component");

    let engine = Engine::default();
    let component = Component::new(&engine, &binary).expect("fixture is a valid component");

    let mut linker: Linker<RecordingHost> = Linker::new(&engine);
    PluginHost::add_to_linker::<RecordingHost, HasSelf<RecordingHost>>(&mut linker, |state| state)
        .expect("generated host bindings register on the linker");

    let mut store = Store::new(&engine, RecordingHost::default());
    let bindings = PluginHost::instantiate(&mut store, &component, &linker)
        .expect("fixture component satisfies the plugin-host world");

    bindings
        .call_activate(&mut store)
        .expect("guest activate runs to completion");

    store.into_data()
}

#[test]
fn grammar_abi_carries_every_declared_field_across_the_boundary() {
    let host = activate_fixture();

    assert_eq!(
        host.grammars.len(),
        1,
        "fixture registers exactly one grammar"
    );
    let grammar = &host.grammars[0];
    assert_eq!(grammar.language_id, "rust-plugin");
    assert_eq!(grammar.grammar_name, "rust-plugin-grammar");
    assert_eq!(grammar.artifact_uri, "file:///tmp/rust-plugin-grammar.wasm");
    assert_eq!(grammar.artifact_hash, "sha256:rust-plugin-grammar");
    assert_eq!(grammar.required_capability, "plugin.grammar.tree_sitter");
}

#[test]
fn theme_abi_carries_every_declared_field_across_the_boundary() {
    let host = activate_fixture();

    assert_eq!(host.themes.len(), 1, "fixture registers exactly one theme");
    let theme = &host.themes[0];
    assert_eq!(theme.label, "Legion Dark");
    assert_eq!(theme.required_capability, "plugin.theme");
}

#[test]
fn lsp_adapter_abi_carries_every_declared_field_across_the_boundary() {
    let host = activate_fixture();

    assert_eq!(
        host.lsp_adapters.len(),
        1,
        "fixture registers exactly one lsp adapter"
    );
    let adapter = &host.lsp_adapters[0];
    assert_eq!(adapter.language_id, "rust-plugin");
    assert_eq!(adapter.server_label, "rust-analyzer");
    assert_eq!(adapter.required_capability, "plugin.lsp.registration");
}

#[test]
fn plugin_host_world_grants_no_ambient_authority() {
    // ADR-0050 admits wasmtime on the condition that the plugin world imports
    // nothing but Legion's own registration interfaces. (ADR-0047 is Extension
    // Distribution Strategy — signed bundles and Open VSX — and says nothing
    // about ambient authority.) A component that asks
    // for WASI (or anything else) must fail to link against this linker, which
    // is populated *only* by the generated bindings.
    let wasi_component = r#"
        (component
          (import "wasi:cli/environment@0.2.0" (instance
            (export "get-environment" (func (result (list (tuple string string)))))
          ))
          (core module $M (func (export "activate")))
          (core instance $m (instantiate $M))
          (func (export "activate") (canon lift (core func $m "activate")))
        )
    "#;
    let binary = wat::parse_str(wasi_component).expect("probe component parses");

    let engine = Engine::default();
    let component = Component::new(&engine, &binary).expect("probe is a valid component");

    let mut linker: Linker<RecordingHost> = Linker::new(&engine);
    PluginHost::add_to_linker::<RecordingHost, HasSelf<RecordingHost>>(&mut linker, |state| state)
        .expect("generated host bindings register on the linker");

    let mut store = Store::new(&engine, RecordingHost::default());
    let error = linker
        .instantiate(&mut store, &component)
        .expect_err("a component importing WASI must not link");
    let rendered = format!("{error:?}");
    assert!(
        rendered.contains("wasi:cli/environment"),
        "unresolved import should name the denied interface, got: {rendered}"
    );
}

#[test]
fn wit_world_declares_all_three_contribution_interfaces() {
    // The generated `PluginHost` only exists if the world resolved, and the
    // three `Host` traits above only compile if each interface is imported by
    // it. This test pins the remaining fact the type system cannot: that the
    // world exposes exactly one host-callable entrypoint.
    let lsp = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("wit")
            .join("lsp.wit"),
    )
    .expect("read lsp.wit");

    assert!(lsp.contains("world plugin-host"));

    // Counted inside the world block, not across the file. The previous version
    // counted `export ` substrings in the whole of `lsp.wit`, so a new export in
    // an unrelated interface would have failed it while a second world
    // entrypoint added to `grammars.wit` would not. It measured the wrong text.
    //
    // That `export activate` exists at all is enforced by the compiler, not
    // here: removing it fails the build with `no method named 'call_activate'`.
    // What this asserts is the part the type system cannot — that the world
    // stays a single entrypoint, because a second one is a new host-callable
    // surface and needs its own decision.
    let world = lsp
        .split_once("world plugin-host")
        .expect("world header")
        .1
        .split_once('}')
        .expect("world block is brace-delimited")
        .0;
    assert_eq!(
        world.matches("export ").count(),
        1,
        "plugin-host must expose exactly one export; a second entrypoint needs an ADR.          World block was: {world}"
    );
    assert!(world.contains("export activate: func();"));
}
