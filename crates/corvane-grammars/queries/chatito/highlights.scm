; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/chatito/).
; Source: nvim-treesitter@728e031f6b11 queries/chatito (Apache-2.0)
; Punctuation
[
  "%["
  "@["
  "~["
  "*["
  "]"
  "("
  ")"
] @punctuation.bracket

"," @punctuation.delimiter

eq: _ @operator

([
  "\""
  "'"
] @punctuation.special
  (#set! conceal ""))

[
  "%"
  "?"
  "#"
] @character.special

; Entities
(intent) @module

(slot) @type

(variation) @attribute

(alias) @keyword.directive

(number) @number

(argument
  key: (string) @property
  value: (string) @string)

(escape) @string.escape

; Import
"import" @keyword.import

(file) @string.special.path

; Text
(word) @spell

; Comment
(comment) @comment @spell
