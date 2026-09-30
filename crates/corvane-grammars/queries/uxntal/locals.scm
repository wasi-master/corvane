; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/uxntal/).
; Source: nvim-treesitter@728e031f6b11 queries/uxntal (Apache-2.0)
; Scopes
[
  (program)
  (macro)
  (memory_execution)
  (subroutine)
] @local.scope

; References
(identifier) @local.reference

; Definitions
(label
  "@"
  .
  (identifier) @local.definition.function)

(macro
  "%"
  .
  (identifier) @local.definition.macro)
