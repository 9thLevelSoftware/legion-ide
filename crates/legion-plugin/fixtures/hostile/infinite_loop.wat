;; Hostile fixture: burn CPU forever.
;;
;; This loop has no exit. Nothing in the guest will stop it and nothing in the
;; guest can be trusted to. Containment must come entirely from the host's fuel
;; quota; without fuel metering this call never returns and the host hangs.
(module
  (func (export "run") (result i32)
    (loop $spin
      br $spin)
    i32.const 0))
