; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/hoon/).
; Source: nvim-treesitter@728e031f6b11 queries/hoon (Apache-2.0)
(number) @number

(string) @string

[
  "("
  ")"
  "["
  "]"
] @punctuation.bracket

[
  (coreTerminator)
  (seriesTerminator)
] @punctuation.delimiter

(rune) @operator

(term) @constant

(aura) @constant.builtin

(lineComment) @comment

(boolean) @constant.builtin

(date) @string.special

(mold) @string.special.symbol

(specialIndex) @number

(lark) @operator

(fullContext) @string.special.symbol
