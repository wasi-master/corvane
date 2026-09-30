; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/thrift/).
; Source: nvim-treesitter@728e031f6b11 queries/thrift (Apache-2.0)
; Scopes
[
  (document)
  (definition)
] @local.scope

; References
(identifier) @local.reference

; Definitions
(annotation_identifier) @local.definition

(const_definition
  (identifier) @local.definition.constant)

(enum_definition
  "enum"
  .
  (identifier) @local.definition.enum
  "{"
  (identifier) @local.definition.constant
  "}")

(senum_definition
  "senum"
  .
  (identifier) @local.definition.enum)

(field
  (identifier) @local.definition.field)

(function_definition
  (identifier) @local.definition.function)

(namespace_declaration
  "namespace"
  (namespace_scope)
  .
  (_) @local.definition.namespace
  (namespace_uri)?)

(parameter
  (identifier) @local.definition.parameter)

(struct_definition
  "struct"
  .
  (identifier) @local.definition.type)

(union_definition
  "union"
  .
  (identifier) @local.definition.type)

(exception_definition
  "exception"
  .
  (identifier) @local.definition.type)

(service_definition
  "service"
  .
  (identifier) @local.definition.type)

(interaction_definition
  "interaction"
  .
  (identifier) @local.definition.type)

(typedef_identifier) @local.definition.type
