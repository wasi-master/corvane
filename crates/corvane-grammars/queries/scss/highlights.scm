; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/scss/).
; Source: nvim-treesitter@728e031f6b11 queries/scss (Apache-2.0)
; dropped 9 pattern(s) using single_line_comment, name, variable_name, key, value, variable_value (not in the pinned grammar)
; inherited from css
[
  "@media"
  "@charset"
  "@namespace"
  "@supports"
  "@keyframes"
  (at_keyword)
] @keyword.directive

"@import" @keyword.import

[
  (to)
  (from)
] @keyword

(comment) @comment @spell

(tag_name) @tag

(class_name) @type

(id_name) @constant

[
  (property_name)
  (feature_name)
] @property

(function_name) @function

[
  "~"
  ">"
  "+"
  "-"
  "*"
  "/"
  "="
  "^="
  "|="
  "~="
  "$="
  "*="
] @operator

[
  "and"
  "or"
  "not"
  "only"
] @keyword.operator

(important) @keyword.modifier

[
  (nesting_selector)
  (universal_selector)
] @character.special

(attribute_selector
  (plain_value) @string)

(pseudo_element_selector
  "::"
  (tag_name) @attribute)

(pseudo_class_selector
  (class_name) @attribute)

(attribute_name) @tag.attribute

(namespace_name) @module

(keyframes_name) @variable

((property_name) @variable
  (#match? @variable "^[-][-]"))

((plain_value) @variable
  (#match? @variable "^[-][-]"))

[
  (string_value)
  (color_value)
  (unit)
] @string

(integer_value) @number

(float_value) @number.float

[
  "#"
  ","
  "."
  ":"
  "::"
  ";"
] @punctuation.delimiter

[
  "{"
  ")"
  "("
  "}"
  "["
  "]"
] @punctuation.bracket



[
  "@at-root"
  "@debug"
  "@error"
  "@extend"
  "@forward"
  "@mixin"
  "@use"
  "@warn"
] @keyword

"@function" @keyword.function

"@return" @keyword.return

"@include" @keyword.import

[
  "@while"
  "@each"
  "@for"
  "from"
  "through"
  "in"
] @keyword.repeat

(function_name) @function

[
  ">="
  "<="
] @operator

(mixin_statement
  (parameters
    (parameter) @variable.parameter))

(function_statement
  (parameters
    (parameter) @variable.parameter))

(plain_value) @string

(keyword_query) @function

(identifier) @variable

(for_statement
  (variable) @variable.parameter)

(argument) @variable.parameter

[
  "["
  "]"
] @punctuation.bracket

(include_statement
  (identifier) @function)
