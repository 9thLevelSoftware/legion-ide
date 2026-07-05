# GUI Headless Input Evidence

Date: 2026-06-14T09:10:56Z
Source backlog: `plans/kanban/legion-ga-backlog.toml`
Source id: `P1.F1.T4`
Dependency satisfied: `P1.F1.T3`
Git HEAD: `0cc78b4b7094eccf3cc538f7ffc3c6bf22964982`

## Purpose

Record the exact docs-hygiene command output for the current SHA and list the representative headless input scenarios now covered by the harness.

## Verification

Command run:

```bash
cargo run -p xtask -- docs-hygiene
```

Exact output:

```text
Compiling proc-macro2 v1.0.106
   Compiling unicode-ident v1.0.24
   Compiling quote v1.0.45
   Compiling serde v1.0.228
   Compiling libc v0.2.186
   Compiling version_check v0.9.5
   Compiling zmij v1.0.21
   Compiling cfg-if v1.0.4
   Compiling serde_json v1.0.149
   Compiling getrandom v0.4.2
   Compiling typenum v1.20.0
   Compiling memchr v2.8.0
   Compiling generic-array v0.14.7
   Compiling itoa v1.0.18
   Compiling thiserror v2.0.18
   Compiling thiserror v1.0.69
   Compiling smallvec v1.15.1
   Compiling camino v1.2.2
   Compiling str_indices v0.4.4
   Compiling equivalent v1.0.2
   Compiling ropey v1.6.1
   Compiling indexmap v2.14.0
   Compiling hex v0.4.3
   Compiling heck v0.5.0
   Compiling syn v2.0.117
   Compiling cpufeatures v0.2.17
   Compiling uuid v1.23.1
   Compiling block-buffer v0.10.4
   Compiling crypto-common v0.1.7
   Compiling digest v0.10.7
   Compiling sha2 v0.10.9
   Compiling serde_derive v1.0.228
   Compiling thiserror-impl v2.0.18
   Compiling thiserror-impl v1.0.69
   Compiling clap_derive v4.5.61
   Compiling clap v4.5.61
   Compiling legion-protocol v0.1.0 (/Users/christopherwilloughby/legion-ide/crates/legion-protocol)
   Compiling serde_spanned v0.6.9
   Compiling toml_datetime v0.6.11
   Compiling cargo-platform v0.1.9
   Compiling cargo_metadata v0.18.1
   Compiling toml_edit v0.22.27
   Compiling toml v0.8.23
   Compiling legion-text v0.1.0 (/Users/christopherwilloughby/legion-ide/crates/legion-text)
   Compiling legion-observability v0.1.0 (/Users/christopherwilloughby/legion-ide/crates/legion-observability)
   Compiling legion-editor v0.1.0 (/Users/christopherwilloughby/legion-ide/crates/legion-editor)
   Compiling xtask v0.1.0 (/Users/christopherwilloughby/legion-ide/xtask)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 11.16s
     Running `target/debug/xtask docs-hygiene`
documentation hygiene checks passed
```

## Representative scenarios covered by the headless input harness

These are the representative workflows ported in `crates/legion-desktop/tests/headless_input.rs` under `P1.F1.T3`:

1. Open a workspace file through the real egui context.
2. Type into the open file and mark it dirty.
3. Save the dirty file through the real egui context with Cmd+S.
4. Run search through the real egui context with Cmd+F.
5. Switch product mode through the real egui context with Cmd+Alt+M.

## Acceptance note

This evidence is tied to the current Git SHA above. If the claimed ready SHA changes, regenerate this document from the new HEAD and rerun `cargo run -p xtask -- docs-hygiene`.
