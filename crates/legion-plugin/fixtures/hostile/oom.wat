(module
  (memory 1)
  (func (export "run") (result i32)
    i32.const 2147483647
    memory.grow
    i32.const -1
    i32.eq
    if
      unreachable
    end
    i32.const 0))
