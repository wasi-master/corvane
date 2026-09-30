; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/ungrammar/).
; Source: nvim-treesitter@728e031f6b11 queries/ungrammar (Apache-2.0)
(comment) @comment @spell

(definition) @keyword

(identifier) @variable

(label_name) @label

(token) @string

[
  "="
  "|"
] @operator

[
  "*"
  "?"
] @keyword.repeat

":" @punctuation.delimiter

[
  "("
  ")"
] @punctuation.bracket
