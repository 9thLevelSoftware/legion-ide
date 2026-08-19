;; A real WebAssembly component targeting `legion:plugin/plugin-host`.
;;
;; This fixture is the anti-drift anchor for the WIT ABI: it is written against
;; the canonical ABI shape of `grammars.wit`, `themes.wit`, and `lsp.wit` by
;; hand, so any change to a record's field list, field order, or field type
;; changes the flattened core signature and makes this component fail to link
;; against the generated host bindings. Nothing here is regenerated from the
;; `.wit` files, so the two are genuinely independent statements of the ABI.
;;
;; Each `string` flattens to a (pointer, length) pair of i32 in the canonical
;; ABI, so a record of N strings lowers to 2N core parameters:
;;   register-grammar      5 strings -> 10 i32
;;   register-theme        2 strings ->  4 i32
;;   register-lsp-adapter  3 strings ->  6 i32
(component
  (import "legion:plugin/grammars" (instance $grammars
    (type $gc (record
      (field "language-id" string)
      (field "grammar-name" string)
      (field "artifact-uri" string)
      (field "artifact-hash" string)
      (field "required-capability" string)
    ))
    (export "grammar-contribution" (type $grammar-contribution (eq $gc)))
    (export "register-grammar" (func (param "contribution" $grammar-contribution)))
  ))

  (import "legion:plugin/themes" (instance $themes
    (type $tc (record
      (field "label" string)
      (field "required-capability" string)
    ))
    (export "theme-contribution" (type $theme-contribution (eq $tc)))
    (export "register-theme" (func (param "contribution" $theme-contribution)))
  ))

  (import "legion:plugin/lsp" (instance $lsp
    (type $lc (record
      (field "language-id" string)
      (field "server-label" string)
      (field "required-capability" string)
    ))
    (export "lsp-adapter-contribution" (type $lsp-adapter-contribution (eq $lc)))
    (export "register-lsp-adapter" (func (param "contribution" $lsp-adapter-contribution)))
  ))

  ;; The guest's linear memory lives in its own core module so it can be aliased
  ;; into the `canon lower` definitions before the main module is instantiated.
  (core module $Memory
    (memory (export "memory") 1)
  )
  (core instance $memory (instantiate $Memory))
  (alias core export $memory "memory" (core memory $mem))

  (alias export $grammars "register-grammar" (func $register-grammar))
  (alias export $themes "register-theme" (func $register-theme))
  (alias export $lsp "register-lsp-adapter" (func $register-lsp-adapter))

  (core func $register-grammar-lowered
    (canon lower (func $register-grammar) (memory $mem) string-encoding=utf8))
  (core func $register-theme-lowered
    (canon lower (func $register-theme) (memory $mem) string-encoding=utf8))
  (core func $register-lsp-adapter-lowered
    (canon lower (func $register-lsp-adapter) (memory $mem) string-encoding=utf8))

  (core module $Main
    (import "grammars" "register-grammar"
      (func $register-grammar (param i32 i32 i32 i32 i32 i32 i32 i32 i32 i32)))
    (import "themes" "register-theme"
      (func $register-theme (param i32 i32 i32 i32)))
    (import "lsp" "register-lsp-adapter"
      (func $register-lsp-adapter (param i32 i32 i32 i32 i32 i32)))
    (import "legion" "memory" (memory 1))

    ;;                          offset  length
    (data (i32.const 0)   "rust-plugin")                          ;;   0  11
    (data (i32.const 16)  "rust-plugin-grammar")                  ;;  16  19
    (data (i32.const 48)  "file:///tmp/rust-plugin-grammar.wasm") ;;  48  36
    (data (i32.const 96)  "sha256:rust-plugin-grammar")           ;;  96  26
    (data (i32.const 128) "plugin.grammar.tree_sitter")           ;; 128  26
    (data (i32.const 160) "Legion Dark")                          ;; 160  11
    (data (i32.const 176) "plugin.theme")                         ;; 176  12
    (data (i32.const 192) "rust-analyzer")                        ;; 192  13
    (data (i32.const 208) "plugin.lsp.registration")              ;; 208  23

    (func (export "activate")
      (call $register-grammar
        (i32.const 0)   (i32.const 11)   ;; language-id
        (i32.const 16)  (i32.const 19)   ;; grammar-name
        (i32.const 48)  (i32.const 36)   ;; artifact-uri
        (i32.const 96)  (i32.const 26)   ;; artifact-hash
        (i32.const 128) (i32.const 26))  ;; required-capability
      (call $register-theme
        (i32.const 160) (i32.const 11)   ;; label
        (i32.const 176) (i32.const 12))  ;; required-capability
      (call $register-lsp-adapter
        (i32.const 0)   (i32.const 11)   ;; language-id
        (i32.const 192) (i32.const 13)   ;; server-label
        (i32.const 208) (i32.const 23))) ;; required-capability
  )

  (core instance $main (instantiate $Main
    (with "grammars" (instance (export "register-grammar" (func $register-grammar-lowered))))
    (with "themes" (instance (export "register-theme" (func $register-theme-lowered))))
    (with "lsp" (instance (export "register-lsp-adapter" (func $register-lsp-adapter-lowered))))
    (with "legion" (instance $memory))
  ))

  (func (export "activate") (canon lift (core func $main "activate")))
)
