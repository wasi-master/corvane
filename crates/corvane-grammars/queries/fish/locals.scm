; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/fish/).
; Source: nvim-treesitter@728e031f6b11 queries/fish (Apache-2.0)
; Scopes
[
  (command)
  (function_definition)
  (if_statement)
  (for_statement)
  (begin_statement)
  (while_statement)
  (switch_statement)
] @local.scope

; Definitions
(function_definition
  name: (word) @local.definition.function)

; References
(variable_name) @local.reference

(word) @local.reference
