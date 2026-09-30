; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/sosl/).
; Source: nvim-treesitter@728e031f6b11 queries/sosl (Apache-2.0)
(find_clause
  (term) @string)

(sobject_return
  (identifier) @type)

(with_type
  (_
    "=" @operator))

[
  "ALL"
  "DIVISION"
  "EMAIL"
  "FIND"
  "ListView"
  "HIGHLIGHT"
  "IN"
  "METADATA"
  "NAME"
  "NETWORK"
  "PHONE"
  "PricebookId"
  "RETURNING"
  "SIDEBAR"
  "SNIPPET"
  "SPELL_CORRECTION"
  "target_length"
  "USING"
] @keyword
