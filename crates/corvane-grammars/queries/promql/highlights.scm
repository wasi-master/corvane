; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/promql/).
; Source: nvim-treesitter@728e031f6b11 queries/promql (Apache-2.0)
; highlights.scm
[
  "*"
  "/"
  "%"
  "+"
  "-"
  ">"
  ">="
  "<"
  "<="
  "="
  "=~"
  "!="
  "!~"
] @operator

[
  "{"
  "}"
  "["
  "]"
  "("
  ")"
] @punctuation.bracket

(float_literal) @number.float

(string_literal) @string

(metric_name) @type

(range_selection) @type

(subquery_range_selection) @type

(label_name) @variable.member

((label_name)
  [
    "=~"
    "!~"
  ]
  (label_value) @string.regexp)

((label_name)
  [
    "="
    "!="
  ]
  (label_value) @string)

(function_name) @function.call

(comment) @comment @spell
