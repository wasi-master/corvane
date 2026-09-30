; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/less/).
; Source: tree-sitter-less 1.0.0 (MIT): queries/highlights.scm
[
  "@import"
  "@namespace"
  "@charset"
] @keyword

(js_comment) @comment @spell

(function_name) @function

[
  ">="
  "<="
] @operator

(plain_value) @string

(keyword_query) @function

(identifier) @variable

(variable) @variable

(arguments
  (variable) @variable.parameter)

[
  "["
  "]"
] @punctuation.bracket

(import_statement
  (identifier) @function)
