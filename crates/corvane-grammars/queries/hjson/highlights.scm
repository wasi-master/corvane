; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/hjson/).
; Source: nvim-treesitter@728e031f6b11 queries/hjson (Apache-2.0)
(true) @boolean

(false) @boolean

(null) @constant.builtin

(number) @number

(pair
  key: (string) @label)

(pair
  value: (string) @string)

(array
  (string) @string)

;  (string_content (escape_sequence) @string.escape)
;  "," @punctuation.delimiter
"[" @punctuation.bracket

"]" @punctuation.bracket

"{" @punctuation.bracket

"}" @punctuation.bracket

(comment) @comment @spell
