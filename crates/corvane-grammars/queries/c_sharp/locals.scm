; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/c_sharp/).
; Source: nvim-treesitter@728e031f6b11 queries/c_sharp (Apache-2.0)
; Definitions
(variable_declarator
  .
  (identifier) @local.definition.var)

(variable_declarator
  (tuple_pattern
    (identifier) @local.definition.var))

(declaration_expression
  name: (identifier) @local.definition.var)

(foreach_statement
  left: (identifier) @local.definition.var)

(foreach_statement
  left: (tuple_pattern
    (identifier) @local.definition.var))

(parameter
  (identifier) @local.definition.parameter)

(method_declaration
  name: (identifier) @local.definition.method)

(local_function_statement
  name: (identifier) @local.definition.method)

(property_declaration
  name: (identifier) @local.definition)

(type_parameter
  (identifier) @local.definition.type)

(class_declaration
  name: (identifier) @local.definition)

; References
(identifier) @local.reference

; Scope
(block) @local.scope
