---
{"body":"Source: FINISH-PLAN.md\n\nEnd-to-end testing and release artifacts.\n\n- Integration tests: open project → edit → syntax colors → LSP diagnostics → terminal commit → `legion-app`, `legion-desktop`\n- Dog-food on own codebase, file bugs, fix blockers → all crates\n- CI pipeline: build, test, package for Windows/macOS/Linux → xtask, CI\n- Release binaries with auto-updater → `legion-app`\n- **Done when:** install from release artifact on clean machine, open project, edit, run, commit\n\n## Deliberately Deferred\n\nThese have production-grade client code already built. They need server-side infrastructure that's out of scope for the IDE finish:\n\n- Remote development server\n- Plugin marketplace\n- VS Code extension runtime (Node.js host)\n- Collaboration transport\n- Hosted telemetry backend","changeId":"chg_phase-6-integration-testing-release","dependencies":[{"mediaType":"text/markdown","path":".legion/project/specs/req_phase-6-integration-testing-release.md","sha256":"sha256:d4aff2e130b445a038989ec15e07b4afb9e2d3a49cc4a4caa51988be4ec52129"}],"kind":"change-design","schemaVersion":"0.1.0","title":"Phase 6 implementation plan"}
---

# Phase 6 implementation plan

Source: FINISH-PLAN.md

End-to-end testing and release artifacts.

- Integration tests: open project → edit → syntax colors → LSP diagnostics → terminal commit → `legion-app`, `legion-desktop`
- Dog-food on own codebase, file bugs, fix blockers → all crates
- CI pipeline: build, test, package for Windows/macOS/Linux → xtask, CI
- Release binaries with auto-updater → `legion-app`
- **Done when:** install from release artifact on clean machine, open project, edit, run, commit

## Deliberately Deferred

These have production-grade client code already built. They need server-side infrastructure that's out of scope for the IDE finish:

- Remote development server
- Plugin marketplace
- VS Code extension runtime (Node.js host)
- Collaboration transport
- Hosted telemetry backend
