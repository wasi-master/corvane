; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/gn/).
; Source: nvim-treesitter@728e031f6b11 queries/gn (Apache-2.0)
; Includes
"import" @keyword.import

; Conditionals
[
  "if"
  "else"
] @keyword.conditional

; Repeats
"foreach" @keyword.repeat

; Operators
[
  "="
  "+="
  "-="
  "!"
  "+"
  "-"
  "<"
  "<="
  ">"
  ">="
  "=="
  "!="
  "&&"
  "||"
] @operator

; Variables
(identifier) @variable

; Functions
(call_expression
  function: (identifier) @function.call)

; Fields
(scope_access
  field: (identifier) @variable.member)

; Literals
(string) @string

(escape_sequence) @string.escape

(expansion) @none

(integer) @number

(hex) @string.special

(boolean) @boolean

; Punctuation
[
  "{"
  "}"
  "["
  "]"
  "("
  ")"
] @punctuation.bracket

[
  "."
  ","
] @punctuation.delimiter

(expansion
  [
    "$"
    "${"
    "}"
  ] @punctuation.special)

; Comments
(comment) @comment
