;; Hostile fixture: allocate far past the granted memory ceiling.
;;
;; The fixture asks to grow linear memory to 4096 pages (256 MiB), which is
;; well inside what the WebAssembly specification permits for wasm32. So the
;; spec will NOT stop this: only the host's ResourceLimiter can.
;;
;; If growth is refused, `memory.grow` yields -1 and the fixture returns its
;; unchanged size, so the test can assert the guest never got the memory. If
;; growth were ever allowed, the fixture proves the escape is real by writing
;; at the 100 MiB offset and returning the grown page count.
(module
  (memory 1)
  (func (export "run") (result i32)
    i32.const 4095
    memory.grow
    i32.const -1
    i32.ne
    if ;; the ceiling did not hold - touch memory the host never intended to grant
      i32.const 104857600
      i32.const 1
      i32.store
    end
    memory.size))
