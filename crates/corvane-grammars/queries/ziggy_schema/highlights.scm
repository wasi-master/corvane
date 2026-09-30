; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/ziggy_schema/).
; Source: nvim-treesitter@728e031f6b11 queries/ziggy_schema (Apache-2.0)
(struct_field
  key: (_) @keyword)

(tag_name) @function

[
  "unknown"
  "any"
  "struct"
  "root"
  "enum"
  "null"
] @keyword

(string) @string

(number) @number

[
  "true"
  "false"
] @boolean

(identifier) @type

"?" @type

[
  "bool"
  "bytes"
  "int"
  "float"
] @constant.builtin

(doc_comment) @comment.documentation

[
  ","
  ":"
  "|"
] @punctuation.delimiter

[
  "["
  "]"
  "{"
  "}"
] @punctuation.bracket
