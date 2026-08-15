# Third-Party Notices

Legion IDE is proprietary software. All workspace crates are
`license = "Proprietary"` and are not published. This file records third-party
projects whose licensed material Legion uses or derives from, together with
the required license texts.

## SmallCode

- Project: SmallCode — AI coding agent optimized for small LLMs
- Repository: https://github.com/Doorman11991/smallcode
- License: MIT
- Copyright: Copyright (c) 2026 Doorman11991

### Scope of use

Legion's use of SmallCode is a **behavioral reimplementation**, governed by
ADR-0049 (`plans/adrs/ADR-0049-smallcode-behavioral-cannibalization.md`):

- Decision logic (tool-call recovery, routing, loop governors, plan anchoring,
  patch semantics) is re-implemented in Rust from SmallCode's documented
  behavior.
- Test vectors derived from SmallCode's test suite and source are stored as
  fixture data under `crates/legion-ai/tests/fixtures/smallcode_vectors/`,
  each carrying a provenance header.
- **No SmallCode source code is compiled into Legion binaries.** No
  JavaScript, no SmallCode executor, transports, or plugin code is included.
- Per-file provenance is tracked in `docs/legal/smallcode-attribution.md`.

The MIT notice below is preserved for all material (test vectors, algorithmic
structure) taken substantially verbatim.

### MIT License (full text)

```
MIT License

Copyright (c) 2026 Doorman11991

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```
