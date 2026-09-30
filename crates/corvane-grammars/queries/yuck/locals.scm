; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/yuck/).
; Source: nvim-treesitter@728e031f6b11 queries/yuck (Apache-2.0)
[
  (ast_block)
  (json_array)
  (json_object)
  (parenthesized_expression)
] @local.scope

(symbol) @local.reference

(keyword) @local.definition.field

(json_object
  (simplexpr
    (ident) @local.definition.field))

(symbol) @local.definition.type
