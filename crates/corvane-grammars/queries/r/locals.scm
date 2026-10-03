; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/r/).
; Source: nvim-treesitter@728e031f6b11 queries/r (Apache-2.0)
; locals.scm
(function_definition) @local.scope

(argument
  name: (identifier) @local.definition)

(parameter
  name: (identifier) @local.definition)

(binary_operator
  lhs: (identifier) @local.definition
  operator: "<-")

(binary_operator
  lhs: (identifier) @local.definition
  operator: "=")

(binary_operator
  operator: "->"
  rhs: (identifier) @local.definition)

(identifier) @local.reference
