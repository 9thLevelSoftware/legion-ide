# SPIKE-001A: Native UI Editor Latency Proof

## Status

Draft — required before UI hiring scale and pre-implementation freeze clearance.

## Objective

Prove the native shell/editor path can sustain editing and interaction requirements under load and that ADR-0002 assumptions remain valid before implementation scale.

## Setup

- Platform targets: Windows primary, macOS and Linux parity checks.
- Representative Rust workloads: at least one large single-file project and one large multi-module workspace.
- Measurement harness capturing input-to-paint latency, frame time distribution, memory growth, and responsiveness metrics.

## Required validation criteria

- Editor text rendering and cursor throughput remain responsive while typing and scrolling in large files.
- Completion overlay and inline diff rendering behave without input-latency spikes.
- Clipboard, IME, and command palette interactions remain stable.
- Windowing and keyboard focus behavior remain deterministic with focus changes.
- Accessibility and navigation feasibility are validated in the target shell path.

## Evidence artifacts

- Benchmarks for:
  - Input-to-paint latency distribution,
  - Frame time variance,
  - CPU/GPU utilization under sustained editing,
  - Memory growth during long sessions.
- Decision log capturing pass/fail for GPUI-style implementation.
- Explicit sign-off before expanding UI scope beyond Spike 1A.
