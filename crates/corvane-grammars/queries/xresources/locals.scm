; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/xresources/).
; Source: nvim-treesitter@728e031f6b11 queries/xresources (Apache-2.0)
(define_directive
  name: (identifier) @local.definition.macro)

(define_function_directive
  name: (identifier) @local.definition.macro)

(parameters
  (identifier) @local.definition.parameter)

(identifier) @local.reference

(resources) @local.scope
