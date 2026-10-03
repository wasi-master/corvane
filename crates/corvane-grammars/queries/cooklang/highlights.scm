; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/cooklang/).
; Source: nvim-treesitter@728e031f6b11 queries/cooklang (Apache-2.0)
(metadata) @comment

(comment) @comment @spell

[
  "{"
  "}"
] @punctuation.bracket

"%" @punctuation.special

(ingredient
  "@" @punctuation.delimiter
  (name)? @string.special.symbol
  (amount
    (quantity)? @number
    (units)? @constant)?)

(timer
  "~" @punctuation.delimiter
  (name)? @string.special.symbol
  (amount
    (quantity)? @number
    (units)? @constant)?)

(cookware
  "#" @punctuation.delimiter
  (name)? @string.special.symbol
  (amount
    (quantity)? @number
    (units)? @constant)?)
