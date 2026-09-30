; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/corn/).
; Source: nvim-treesitter@728e031f6b11 queries/corn (Apache-2.0)
[
  "let"
  "in"
] @keyword

[
  "{"
  "}"
  "["
  "]"
] @punctuation.bracket

"." @punctuation.delimiter

[
  ".."
  "="
] @operator

(input) @constant

(null) @constant.builtin

(comment) @comment @spell

(string) @string

(integer) @number

(float) @number.float

(float
  "." @number.float)

(boolean) @boolean

(path_seg) @property
