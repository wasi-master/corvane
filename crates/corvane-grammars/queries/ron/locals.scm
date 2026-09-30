; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/ron/).
; Source: nvim-treesitter@728e031f6b11 queries/ron (Apache-2.0)
(source_file) @local.scope

(source_file
  (array) @local.scope)

(source_file
  (map) @local.scope)

(source_file
  (struct) @local.scope)

(source_file
  (tuple) @local.scope)

(identifier) @local.reference

(struct_entry
  (identifier) @local.definition.field)

(struct_entry
  (identifier) @local.definition.enum
  (enum_variant))

(struct
  (struct_name) @local.definition.type)
