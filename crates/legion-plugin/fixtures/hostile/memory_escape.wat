;; Hostile fixture: hand the host a pointer outside guest memory.
;;
;; The fixture holds the host-log capability and stays inside its host-call and
;; output quotas. Its attack is the pointer itself: it asks the host to read 8
;; bytes at offset 0x7FFF0000, far past the one page it owns. A host that
;; trusted guest-supplied pointers would read host memory on the plugin's
;; behalf.
(module
  (import "env" "host_log" (func $host_log (param i32 i32)))
  (memory (export "memory") 1)
  (func (export "run") (result i32)
    i32.const 2147418112
    i32.const 8
    call $host_log
    i32.const 0))
