; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/kdl/).
; Source: nvim-treesitter@728e031f6b11 queries/kdl (Apache-2.0)
(document) @local.scope

(node_children) @local.scope

(node) @local.scope

(identifier) @local.reference

(node_field) @local.definition.field

(node
  name: (identifier) @local.definition.type)

(type) @local.definition.type
