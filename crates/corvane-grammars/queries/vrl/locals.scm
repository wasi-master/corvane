; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/vrl/).
; Source: nvim-treesitter@728e031f6b11 queries/vrl (Apache-2.0)
(closure_variables
  (ident) @local.definition.parameter)

[
  (ident)
  (metadata)
] @local.reference

(query
  (event) @local.reference)

[
  (block)
  (closure)
  (if_statement)
] @local.scope
