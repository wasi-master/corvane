; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/foam/).
; Source: nvim-treesitter@728e031f6b11 queries/foam (Apache-2.0)
(dict) @local.scope

(dict
  key: (_) @local.definition.type)

(key_value
  keyword: (_) @local.definition.parameter)

(key_value
  value: (macro
    (identifier)*)* @local.reference)
