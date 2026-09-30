; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/jsonnet/).
; Source: tree-sitter-jsonnet 0.0.1 (MIT): queries/highlights.scm
; Identifier fallback (specific rules below override)
(ident) @variable

; Variable references
(ident_ref (ident) @variable)

; Bindings
(bind (ident) @variable)

; Parameters
(param (ident) @variable.parameter)
(destruct_field (ident) @variable.parameter)
(destruct_rest (ident) @variable.parameter)

; Named arguments
(arg . (ident) @variable.parameter "=")

; Object fields
(field (field_name (ident) @property))
(suffix_index (ident) @property)

; Function definitions (override variable)
(bind (ident) @function (params))
(field (field_name (ident) @function) (params))

; std builtin
((ident_ref (ident) @variable.builtin)
 (#eq? @variable.builtin "std"))

; Literals
(number) @number
(string) @string
(verbatim_string) @string
(string_block) @string

"null" @constant.builtin

[
  "true"
  "false"
] @boolean

[
  "self"
  "super"
] @variable.builtin

(dollar) @variable.builtin

; Keywords
[
  "local"
  "assert"
  "function"
  "error"
  "tailstrict"
] @keyword

[
  "if"
  "then"
  "else"
] @keyword.conditional

[
  "for"
  "in"
] @keyword.repeat

[
  "import"
  "importstr"
  "importbin"
] @keyword.import

; Operators
[
  "+"
  "-"
  "*"
  "/"
  "%"
  "<<"
  ">>"
  "<"
  ">"
  "<="
  ">="
  "=="
  "!="
  "&"
  "^"
  "|"
  "&&"
  "||"
  "??"
  "!"
  "~"
  "="
  ":"
  "::"
  ":::"
  "..."
] @operator

(visibility) @operator

; Punctuation
[
  ","
  ";"
  "."
] @punctuation.delimiter

[
  "("
  ")"
  "["
  "]"
  "{"
  "}"
] @punctuation.bracket

"?" @punctuation.special

; Comments
(line_comment) @comment
(block_comment) @comment
