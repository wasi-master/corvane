; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/xcompose/).
; Source: nvim-treesitter@728e031f6b11 queries/xcompose (Apache-2.0)
(keysym) @constant

((keysym) @constant.builtin
  (#eq? @constant.builtin "Multi_key"))

(text) @string

"include" @keyword.import

[
  (octal)
  (hex)
] @number

[
  (modifier)
  "None"
] @keyword.modifier

[
  "%L"
  "%H"
  "%S"
] @string.special

[
  "!"
  "~"
] @operator

[
  ":"
  "<"
  ">"
  "\""
] @punctuation.delimiter

(comment) @comment @spell
