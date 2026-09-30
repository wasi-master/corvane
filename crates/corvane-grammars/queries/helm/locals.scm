; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/helm/).
; Source: nvim-treesitter@728e031f6b11 queries/helm (Apache-2.0)
; inherited from gotmpl
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


