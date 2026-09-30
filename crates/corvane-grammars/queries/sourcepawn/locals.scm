; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/sourcepawn/).
; Source: nvim-treesitter@728e031f6b11 queries/sourcepawn (Apache-2.0)
[
  (function_definition)
  (alias_declaration)
  (enum_struct_method)
  (methodmap_method)
  (methodmap_method_constructor)
  (methodmap_method_destructor)
  (methodmap_property_method)
] @local.scope

; Definitions
(variable_declaration
  name: (identifier) @local.definition)

(old_variable_declaration
  name: (identifier) @local.definition)

; References
(identifier) @local.reference
