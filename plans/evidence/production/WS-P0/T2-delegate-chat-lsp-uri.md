# T2/T3 follow-on — Delegate chat body + LSP location URI paths + router honesty

**Date:** 2026-07-21

## Changes

| Item | Detail |
| --- | --- |
| **B5 Delegate chat** | `resolve_delegate_chat_reply` fills assistant `content_label` from Anthropic when BYOK/env credentials exist; offline fixture still contains `Delegate provider answer ready` for tests |
| **B6 ProviderRouter honesty** | Documented that `route_completion` is **metadata-only by protocol** (fingerprints/bytes in `output_labels`, never raw text). Product prose is mapped at the composition edge (assist, chat, inline). |
| **B21 LSP go-to-def path** | `path_from_file_uri` + wire into `location_projection_for_item` / workspace-symbol projection so desktop auto-nav (`OpenPathAtPosition`) receives a path |

## Verification

```text
cargo test -p legion-lsp --test read_side_contract
cargo test -p legion-app --test delegated_task_integration
cargo check -p legion-app --lib
```
