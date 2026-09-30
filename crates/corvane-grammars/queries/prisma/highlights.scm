; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/prisma/).
; Source: nvim-treesitter@728e031f6b11 queries/prisma (Apache-2.0)
(variable) @variable

[
  "datasource"
  "generator"
  "model"
  "view"
] @keyword

[
  "type"
  "enum"
] @keyword.type

(comment) @comment @spell

(developer_comment) @comment.documentation @spell

[
  (attribute)
  (call_expression)
] @function

(arguments) @property

(column_type) @type

(enumeral) @constant

(column_declaration
  (identifier) @variable)

(string) @string

[
  "("
  ")"
  "["
  "]"
  "{"
  "}"
] @punctuation.bracket

[
  "="
  "@"
] @operator
