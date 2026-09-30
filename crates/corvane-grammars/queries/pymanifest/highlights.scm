; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/pymanifest/).
; Source: nvim-treesitter@728e031f6b11 queries/pymanifest (Apache-2.0)
(keyword) @keyword

(dir_sep) @punctuation.delimiter

(glob) @punctuation.special

(linebreak) @character.special

(char_sequence) @string.special

(char_sequence
  [
    "["
    "]"
  ] @punctuation.bracket)

(char_sequence
  "!" @operator)

(char_range
  "-" @operator)

(escaped_char) @string.escape

(comment) @comment @spell
