; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/bp/).
; Source: nvim-treesitter@728e031f6b11 queries/bp (Apache-2.0)
(module
  (property
    field: (identifier) @local.definition.parameter))

(map_expression
  (property
    field: (identifier) @local.definition.field))

(assignment
  left: (identifier) @local.definition.var)

(pattern_binding
  binding: (identifier) @local.definition.var)

(identifier) @local.reference
