;; Hostile fixture: hand the host more bytes than it agreed to read.
;;
;; The fixture holds the host-log capability and makes exactly one call, so
;; neither the capability check nor the host-call counter will stop it.
;;
;; The 1024-byte payload is deliberately INSIDE the fixture's one page of
;; linear memory. A payload past the end of memory would be caught by the
;; pointer bounds check instead, and the output ceiling would never be the
;; thing under test. Here only the output quota stands between the guest and a
;; payload twice the size it was granted.
(module
  (import "env" "host_log" (func $host_log (param i32 i32)))
  (memory (export "memory") 1)
  (func (export "run") (result i32)
    i32.const 0
    i32.const 1024
    call $host_log
    i32.const 0))
