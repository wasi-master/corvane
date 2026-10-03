; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/twig/).
; Source: nvim-treesitter@728e031f6b11 queries/twig (Apache-2.0)
(comment) @comment @spell

(filter_identifier) @function.call

(function_identifier) @function.call

(test) @function.builtin

(variable) @variable

(string) @string

(interpolated_string) @string

(operator) @operator

(number) @number

(boolean) @boolean

(null) @constant.builtin

(keyword) @keyword

(attribute) @attribute

(tag) @tag

(conditional) @keyword.conditional

(repeat) @keyword.repeat

(method) @function.method

(parameter) @variable.parameter

[
  "{{"
  "}}"
  "{{-"
  "-}}"
  "{{~"
  "~}}"
  "{%"
  "%}"
  "{%-"
  "-%}"
  "{%~"
  "~%}"
] @tag.delimiter

[
  ","
  "."
] @punctuation.delimiter

[
  "?"
  ":"
  "="
  "|"
] @operator

(interpolated_string
  [
    "#{"
    "}"
  ] @punctuation.special)

[
  "("
  ")"
  "["
  "]"
] @punctuation.bracket

(hash
  [
    "{"
    "}"
  ] @punctuation.bracket)
