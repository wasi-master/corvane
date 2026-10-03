; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/gotmpl/).
; Source: nvim-treesitter@728e031f6b11 queries/gotmpl (Apache-2.0)
[
  (if_action)
  (range_action)
  (block_action)
  (with_action)
  (define_action)
] @local.scope

(variable_definition
  variable: (variable) @local.definition.var)

(variable) @local.reference
