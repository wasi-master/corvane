; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/po/).
; Source: nvim-treesitter@728e031f6b11 queries/po (Apache-2.0)
; Keywords
[
  "msgctxt"
  "msgid"
  "msgid_plural"
  "msgstr"
  "msgstr_plural"
] @keyword

; Punctuation
[
  "["
  "]"
] @punctuation.bracket

; Literals
(string) @string

(escape_sequence) @string.escape

(number) @number

; Comments
(comment) @comment @spell

(comment
  (reference
    (text) @string.special.path))

(comment
  (flag
    (text) @keyword.directive))
