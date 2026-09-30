; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/glimmer/).
; Source: nvim-treesitter@728e031f6b11 queries/glimmer (Apache-2.0)
[
  (element_node)
  (block_statement)
] @local.scope

(identifier) @local.reference

(block_params
  (identifier) @local.definition.var)
