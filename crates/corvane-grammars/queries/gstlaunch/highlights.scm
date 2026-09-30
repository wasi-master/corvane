; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/gstlaunch/).
; Source: nvim-treesitter@728e031f6b11 queries/gstlaunch (Apache-2.0)
[
  "!"
  "="
] @operator

[
  ","
  "."
  ";"
  "/"
] @punctuation.delimiter

[
  "("
  ")"
] @punctuation.bracket

(property
  key: (identifier) @variable.member)

(value) @string

(string_literal) @string

(cap
  .
  (identifier) @string
  .
  (identifier) @string)

(simple_element
  type: (_) @type)

(bin
  type: (_) @type)
