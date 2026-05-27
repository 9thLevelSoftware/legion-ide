# Phase 4 Language And Terminal IDE Loop

Acceptance status: Accepted

## Delivered

- GUI Phase 4 dependency policy rebaseline for `devil-app -> devil-index` and `devil-app -> devil-terminal`.
- Protocol DTOs for `LanguageToolingProjection` and `TerminalPanelProjection`.
- UI shell snapshot fields and command intents for language hover/completion/definition/references/outline, edit-producing proposal actions, cancellation, and terminal launch/input/resize/kill/close/poll/search.
- App-owned language tooling workflow backed by `devil-index` lexical semantic projection and LSP edit-to-proposal conversion.
- App-owned terminal workflow backed by security-policy decisions, terminal protocol validators, metadata-only terminal audit persistence/events, and the default-off deterministic terminal fixture.
- Desktop view rows and bridge mappings for language and terminal panels.
- Cross-boundary regression tests for proposal-only language edits and terminal no-mutation guarantees.

## Acceptance Mapping

- Problems, hover, completion, definition, references, outline, and operation rows are represented in `LanguageToolingProjection`.
- Formatting, rename, organize-imports, and code-action actions create workspace proposal previews before mutation, and proposal ledger rows advance through Created -> Validated -> Previewed.
- Terminal launch, input, resize, kill, close, bounded output, search, scrollback, denial, and error states are represented in `TerminalPanelProjection`.
- Terminal and language workflows live in `devil-app`, not `devil-ui` or `devil-desktop`.
- Terminal launch and lifecycle operations are policy-gated, validator-checked, metadata-audited, denied by default, and denied for untrusted workspaces.

## Verification

Final gate commands:

- `cargo run -p xtask -- check-deps`
- `cargo fmt --all --check`
- `cargo check --workspace --all-targets`
- `cargo test --workspace --all-targets`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo deny check` passed with duplicate-version warnings under the existing warning-level policy

Targeted commands:

- `cargo test -p devil-protocol --test dto_contracts language_terminal_projection -- --nocapture`
- `cargo test -p devil-app --test language_terminal_integration -- --nocapture`
- `cargo test -p devil-app --test language_tooling_workflow -- --nocapture`
- `cargo test -p devil-app --test terminal_workflow -- --nocapture`
- `cargo test -p devil-desktop --test language_terminal_workflow -- --nocapture`
- `cargo test -p devil-desktop --test language_terminal_view -- --nocapture`
- `cargo test -p devil-terminal --all-targets`
- `cargo test -p devil-security --all-targets`

## Residual Risk

- Language results are currently lexical/semantic projections, not a production supervised LSP runtime.
- Formatting, organize-imports, and code-action previews intentionally use safe no-op workspace edits with diagnostics until live LSP edits are wired.
- Terminal lifecycle is proven through the deterministic fixture path; production native PTY behavior remains controlled by the existing terminal/security activation gates.
- The known ignored 100MB text performance workload remains outside Phase 4 acceptance.
