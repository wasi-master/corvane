; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/kusto/).
; Source: nvim-treesitter@728e031f6b11 queries/kusto (Apache-2.0)
(identifier) @variable

(let_keyword) @keyword

(function_call
  (identifier) @function.call)

[
  (type_cast_function)
  (to_scalar_function)
  (between_function)
  (datatable_function)
] @function.call

(typed_parameter
  (identifier) @variable.parameter)

(function_arguments
  (identifier) @variable.parameter)

[
  (binary_operator)
  (compound_keywords)
  (operator)
  (range_operator)
  (join_operator)
  (sub_operator)
  (to_operator)
  (mv_apply_operator)
  (sort_keyword)
] @keyword.operator

(string) @string

(number) @number

(bool) @boolean

(null) @constant.builtin

(comment) @comment @spell

(type) @type

(join_types) @keyword.modifier

[
  "("
  ")"
  "["
  "]"
  "{"
  "}"
] @punctuation.bracket

[
  ","
  ":"
  (pipe)
] @punctuation.delimiter
