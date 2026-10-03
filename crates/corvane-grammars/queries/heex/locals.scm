; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/heex/).
; Source: nvim-treesitter@728e031f6b11 queries/heex (Apache-2.0)
; HEEx tags, components, and slots are references
[
  (component_name)
  (slot_name)
  (tag_name)
] @local.reference

; Create a new scope within each HEEx tag, component, and slot
[
  (component)
  (slot)
  (tag)
] @local.scope
