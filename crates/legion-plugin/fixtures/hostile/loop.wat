(module
  (func (export "run") (result i32)
    (local i32)
    i32.const 0
    local.set 0
    loop $spin
      local.get 0
      i32.const 1
      i32.add
      local.tee 0
      i32.const 4
      i32.lt_s
      br_if $spin
      unreachable
    end
    i32.const 0))
