; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/forth/).
; Source: nvim-treesitter@728e031f6b11 queries/forth (Apache-2.0)
(core) @function

(operator) @operator

(word) @variable

((word) @constant
  (#match? @constant "^[A-Z_]+$"))

(number) @number

(string) @string

[
  (start_definition)
  (end_definition)
] @punctuation.delimiter

(comment) @comment @spell
