;; Hostile fixture: reach the workspace WITHOUT naming WASI.
;;
;; A plugin author who knows `wasi_snapshot_preview1` is blocked will simply
;; ask for the same authority under a different import name. This fixture
;; imports `env.read_file`, which is not WASI and is not on any blocklist.
;;
;; It must still be refused, because the host's import rule is an allowlist of
;; exactly one function, not a blocklist of known-bad ones. If the host ever
;; regressed to blocklisting WASI, this fixture would load.
(module
  (import "env" "read_file" (func $read_file (param i32 i32) (result i32)))
  (func (export "run") (result i32)
    i32.const 0
    i32.const 0
    call $read_file))
