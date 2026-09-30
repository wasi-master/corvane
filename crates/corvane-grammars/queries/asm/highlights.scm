; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/asm/).
; Source: nvim-treesitter@728e031f6b11 queries/asm (Apache-2.0)
; General
(label
  [
    (ident)
    (word)
  ] @label)

(reg) @variable.builtin

(meta
  kind: (_) @function.builtin)

(instruction
  kind: (_) @function.builtin)

(const
  name: (word) @constant)

; Comments
[
  (line_comment)
  (block_comment)
] @comment @spell

; Literals
(int) @number

(float) @number.float

(string) @string

; Keywords
[
  "byte"
  "word"
  "dword"
  "qword"
  "ptr"
  "rel"
  "label"
  "const"
] @keyword

; Operators & Punctuation
[
  "+"
  "-"
  "*"
  "/"
  "%"
  "|"
  "^"
  "&"
] @operator

[
  "("
  ")"
  "["
  "]"
] @punctuation.bracket

[
  ","
  ":"
] @punctuation.delimiter
