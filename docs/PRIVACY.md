# Legion Privacy Policy

This is the user-facing privacy policy for Legion IDE. It describes what
the current in-tree product does with data on the machine it runs on. It
does not describe a shipped consumer release.

Related documents: [`SECURITY.md`](SECURITY.md), [`MODES.md`](MODES.md),
[`TROUBLESHOOTING.md`](TROUBLESHOOTING.md), [`../LICENSE`](../LICENSE),
[`../THIRD_PARTY_NOTICES.md`](../THIRD_PARTY_NOTICES.md).

## Default: Manual, zero egress

**Manual** is the default product mode. In Manual mode Legion makes no
network calls: no phone-home, no usage analytics, no automatic crash
upload, and no provider traffic. Local editing, search, Git, terminal,
and language tooling stay on the machine except for Git remotes you
invoke yourself.

AI features — Assist, Delegate, and Legion Workflows — are opt-in. Each
requires an explicit mode change. No feature re-enables itself after you
turn it off.

## What stays on disk

Workspace-local state lives under `.legion/` in the opened workspace:

- session metadata (no dirty buffer bodies, no raw secrets);
- unsaved-buffer snapshots used for crash-safe restore;
- local-history blobs for files you saved through the proposal workflow;
- optional crash-report summaries when crash reports are enabled;
- a metadata-only support bundle when you export one.

These files are not uploaded. They are not included in Git unless you
add them.

## Diagnostics and support bundles

Help → **Export Support Bundle** (command palette: `Help: Export Support
Bundle`) writes `.legion/support-bundle.md`. The default export is
metadata only: version, mode, consent labels, tab counts, and crash
summary identifiers. It does not include editor text, dirty buffers,
search queries, terminal payloads, prompts, API keys, or raw panic
bodies.

Raw crash files are a separate, double-opt-in path. Product consent
keeps `raw_source_allowed` false. The Help/About export never takes that
path.

Crash reports themselves are opt-in (Settings → Privacy). Enabling them
still does not upload anything.

## AI and network

When you opt into Assist, Delegate, or Legion Workflows, network use is
limited to the provider or remote you configured. Workspace mutation
stays proposal-mediated: a model can suggest a change; it cannot write
until you approve. Manual mode remains available and remains zero-egress
for product features.

This policy does not cover Git remotes, package registries, or other
tools you run in the terminal. Those are your commands.

## Third-party code

Third-party notices ship with the native package as `THIRD_PARTY_NOTICES.md`
and are listed in [`legal/smallcode-attribution.md`](legal/smallcode-attribution.md).
