; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/poe_filter/).
; Source: nvim-treesitter@728e031f6b11 queries/poe_filter (Apache-2.0)
; dropped predicates tree-sitter-highlight cannot evaluate: #not-has-parent?
[
  "Show"
  "Hide"
  "Minimal"
] @module

[
  "Import"
  "Optional"
] @keyword.import

(condition
  (name) @keyword.conditional)

(action
  (name) @keyword)

(continue) @label

(operator) @operator

(string) @string

(file) @string.special.path

[
  (quality)
  (rarity)
  (influence)
  (colour)
  (shape)
] @constant.builtin

(sockets) @variable.builtin

(number) @number

(boolean) @boolean

[
  (disable)
  "Temp"
] @constant

(comment) @comment @spell

"\"" @punctuation.delimiter

; conceal unnecessary quotes
("\"" @conceal
  
  (#set! conceal ""))
