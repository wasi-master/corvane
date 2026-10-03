; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/godot_resource/).
; Source: nvim-treesitter@728e031f6b11 queries/godot_resource (Apache-2.0)
(identifier) @variable

(section
  (identifier) @tag)

(section
  [
    "["
    "]"
  ] @tag.delimiter)

(attribute
  (identifier) @tag.attribute)

(property
  (path) @property)

(constructor
  (identifier) @constructor)

(string) @string

(integer) @number

(float) @number.float

[
  (true)
  (false)
] @boolean

(null) @constant.builtin

(array
  [
    "["
    "]"
  ] @punctuation.bracket)

[
  "("
  ")"
  "{"
  "}"
] @punctuation.bracket

"=" @operator

(comment) @comment @spell
