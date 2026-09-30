; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/wing/).
; Source: nvim-treesitter@728e031f6b11 queries/wing (Apache-2.0)
(block) @local.scope

(variable_definition_statement
  name: (identifier) @local.definition)

; TODO: Missing "@local.reference" usage tuned for each relevant identifier location
