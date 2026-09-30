; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/javascript/).
; Source: tree-sitter-javascript 0.25.0 (MIT): queries/locals.scm
; Scopes
;-------

[
  (statement_block)
  (function_expression)
  (arrow_function)
  (function_declaration)
  (method_definition)
] @local.scope

; Definitions
;------------

(pattern/identifier) @local.definition

(variable_declarator
  name: (identifier) @local.definition)

; References
;------------

(identifier) @local.reference
