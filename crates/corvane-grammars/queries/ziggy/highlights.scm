; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/ziggy/).
; Source: nvim-treesitter@728e031f6b11 queries/ziggy (Apache-2.0)
[
  (true)
  (false)
] @constant.builtin

(null) @constant.builtin

[
  (integer)
  (float)
] @number

(struct_field
  key: (_) @keyword)

(struct
  name: (_) @type)

(tag) @function

[
  (string)
  (line_string)*
] @string

(comment) @comment

(escape_sequence) @string.escape

"," @punctuation.delimiter

[
  "["
  "]"
  "{"
  "}"
  "("
  ")"
] @punctuation.bracket

(top_comment) @comment
