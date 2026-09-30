; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/rasi/).
; Source: nvim-treesitter@728e031f6b11 queries/rasi (Apache-2.0)
(rule_set
  (selectors
    (id_selector))) @local.scope

(block
  (declaration
    (property_name) @local.definition.var))

(reference_value
  name: (identifier) @local.reference)
