# Phase 2 B16 — Cargo debug configs default stop_on_entry

**Date:** 2026-07-23  
**Status:** Ready to land

## Delivered

Discovered cargo launch configurations set `stop_on_entry: true` so live
system-adapter launches (B12/B13) pause at entry without requiring a prior
breakpoint. Fixture/fake paths remain fine either way.

## Residual

- Human windowed GUI journal
