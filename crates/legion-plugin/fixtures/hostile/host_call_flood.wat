;; Hostile fixture: flood the host with calls it is allowed to make.
;;
;; This fixture DOES hold the host-log capability, so no capability check will
;; stop it. It calls `env.host_log` 4096 times. Only the host-call quota can
;; contain it, and the host must stop counting at the granted ceiling rather
;; than trusting the guest to stop asking.
(module
  (import "env" "host_log" (func $host_log (param i32 i32)))
  (memory (export "memory") 1)
  (func (export "run") (result i32)
    (local $i i32)
    (loop $again
      i32.const 0
      i32.const 4
      call $host_log
      local.get $i
      i32.const 1
      i32.add
      local.tee $i
      i32.const 4096
      i32.lt_s
      br_if $again)
    local.get $i))
