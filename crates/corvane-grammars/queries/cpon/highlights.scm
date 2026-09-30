; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/cpon/).
; Source: nvim-treesitter@728e031f6b11 queries/cpon (Apache-2.0)
; Literals
(string) @string

(escape_sequence) @string.escape

(hex_blob
  "x" @character.special
  (_) @string)

(esc_blob
  "b" @character.special
  (_) @string)

(datetime
  "d" @character.special
  (_) @string.special)

(_
  key: (_) @property)

(number) @number

(float) @number.float

(boolean) @boolean

(null) @constant.builtin

; Punctuation
[
  ","
  ":"
] @punctuation.delimiter

[
  "{"
  "}"
] @punctuation.bracket

[
  "["
  "]"
] @punctuation.bracket

[
  "<"
  ">"
] @punctuation.bracket

("\"" @string
  (#set! conceal ""))

; Comments
(comment) @comment @spell
