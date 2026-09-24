;; Hostile fixture: run long enough to matter, cheaply enough to dodge fuel.
;;
;; This loop is bounded at 200_000 iterations, so it terminates and it stays
;; well inside a generous fuel budget. Only the wall-clock deadline separates
;; it from a plugin that holds the UI thread. It exists so wall-time
;; enforcement can be tested without the fuel quota being what actually stops
;; the guest.
(module
  (func (export "run") (result i32)
    (local $i i32)
    (loop $again
      local.get $i
      i32.const 1
      i32.add
      local.tee $i
      i32.const 200000
      i32.lt_s
      br_if $again)
    local.get $i))
