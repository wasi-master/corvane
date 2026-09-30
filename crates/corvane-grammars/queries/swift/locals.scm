; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/swift/).
; Source: nvim-treesitter@728e031f6b11 queries/swift (Apache-2.0)
(import_declaration
  (identifier) @local.definition.import)

(function_declaration
  name: (simple_identifier) @local.definition.function)

; Scopes
[
  (statements)
  (for_statement)
  (while_statement)
  (repeat_while_statement)
  (do_statement)
  (if_statement)
  (guard_statement)
  (switch_statement)
  (property_declaration)
  (function_declaration)
  (class_declaration)
  (protocol_declaration)
] @local.scope
