(module
  (import "env" "host_log" (func $host_log (param i32 i32)))
  (func (export "run") (result i32)
    i32.const 0
    i32.const 0
    call $host_log
    i32.const 0))
