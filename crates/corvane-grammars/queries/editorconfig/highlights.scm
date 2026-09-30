; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/editorconfig/).
; Source: nvim-treesitter@728e031f6b11 queries/editorconfig (Apache-2.0)
(comment) @comment @spell

(property) @property

(string) @string

(header
  (glob) @string.special.path)

(character) @character

(character_escape) @string.escape

(wildcard) @character.special

(integer) @number

[
  "["
  "]"
  "{"
  "}"
] @punctuation.bracket

[
  ","
  ".."
  "/"
  "-"
] @punctuation.delimiter

[
  "="
  "!"
] @operator
