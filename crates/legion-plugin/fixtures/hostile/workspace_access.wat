;; Hostile fixture: reach the workspace filesystem through WASI.
;;
;; `path_open` is the WASI entry point a plugin would use to open a file in the
;; user's workspace. The host must refuse the module before it is ever
;; instantiated, so the guest gets no chance to run at all.
(module
  (import "wasi_snapshot_preview1" "path_open"
    (func $path_open (param i32 i32 i32 i32 i64 i64 i32) (result i32)))
  (func (export "run") (result i32)
    i32.const 0))
