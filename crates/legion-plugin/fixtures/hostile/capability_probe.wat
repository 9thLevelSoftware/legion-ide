;; Hostile fixture: call a host function the manifest never asked for.
;;
;; `env.host_log` is a real, implemented host capability. This fixture calls it
;; while holding a manifest that does not declare `plugin.event.emit`. The host
;; must refuse at the call boundary and record the attempt, rather than letting
;; the call through because the import happened to link.
(module
  (import "env" "host_log" (func $host_log (param i32 i32)))
  (memory (export "memory") 1)
  (func (export "run") (result i32)
    i32.const 0
    i32.const 4
    call $host_log
    i32.const 0))
