; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/properties/).
; Source: nvim-treesitter@728e031f6b11 queries/properties (Apache-2.0)
(comment) @comment @spell

(key) @property

(value) @string

(value
  (escape) @string.escape)

((value) @boolean
  (#any-of? @boolean "true" "false"))

((value) @number
  (#match? @number "^[0-9]+$"))

((index) @number
  (#match? @number "^[0-9]+$"))

((substitution
  (key) @constant)
  (#match? @constant "^[A-Z_][A-Z0-9_]*$"))

(substitution
  (key) @function
  "::" @punctuation.special
  (secret) @constant.macro)

(property
  [
    "="
    ":"
  ] @operator)

[
  "${"
  "}"
] @punctuation.special

(substitution
  ":" @punctuation.special)

[
  "["
  "]"
] @punctuation.bracket

[
  "."
  "\\"
] @punctuation.delimiter
