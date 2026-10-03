; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/zsh/).
; Source: nvim-treesitter@728e031f6b11 queries/zsh (Apache-2.0)
; Scopes
(function_definition) @local.scope

; Definitions
(variable_assignment
  name: (variable_name) @local.definition.var)

(function_definition
  name: (word) @local.definition.function)

; References
(variable_name) @local.reference

(word) @local.reference
