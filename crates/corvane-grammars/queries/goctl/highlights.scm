; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/goctl/).
; Source: nvim-treesitter@728e031f6b11 queries/goctl (Apache-2.0)
; Key Symbol
"=" @operator

[
  "."
  ","
  ":"
  ";"
] @punctuation.delimiter

[
  "("
  ")"
  "{"
  "}"
  "["
  "]"
] @punctuation.bracket

; Keywords
[
  "syntax"
  "info"
  "service"
] @keyword

"import" @keyword.import

"returns" @keyword.return

[
  "type"
  "struct"
] @keyword.type

[
  "@doc"
  "@handler"
  "@server"
] @attribute

; Service
(serviceName) @type

; Httpmethod
(HTTPMETHOD) @keyword.operator

; Field
(fieldType) @type.builtin

(fieldName) @variable.member

(anonymousField) @variable.member

; Functions
(handlerValue) @function.method

; Strings
(VALUE) @string

(tag) @string.documentation

(PATH) @string.special.path

; Comments
(comment) @comment @spell

(key) @variable.member

(identValue) @string

(DURATION) @number

(NUMBER) @number

; Struct
(structNameId) @type

(body) @type
