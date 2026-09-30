; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/elsa/).
; Source: nvim-treesitter@728e031f6b11 queries/elsa (Apache-2.0)
; Keywords
[
  "eval"
  "let"
] @keyword

; Function
(function) @function

; Method
(method) @function.method

; Parameter
(parameter) @variable.parameter

; Variables
(identifier) @variable

; Operators
[
  "\\"
  "->"
  "="
  (step)
] @operator

; Punctuation
[
  "("
  ")"
] @punctuation.bracket

":" @punctuation.delimiter

; Comments
(comment) @comment @spell
