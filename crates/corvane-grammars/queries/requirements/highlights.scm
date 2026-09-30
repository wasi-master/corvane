; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/requirements/).
; Source: nvim-treesitter@728e031f6b11 queries/requirements (Apache-2.0)
; packages
(package) @variable

(extras
  (package) @variable.parameter)

(path) @string.special.path

(url) @string.special.url

; versions
(version_cmp) @operator

(version) @number

; markers
(marker_var) @attribute

(marker_op) @keyword.operator

; options
(option) @function

"=" @operator

; punctuation
[
  "["
  "]"
  "("
  ")"
] @punctuation.bracket

[
  ","
  ";"
  "@"
] @punctuation.delimiter

[
  "${"
  "}"
] @punctuation.special

; misc
(env_var) @constant

(quoted_string) @string

(linebreak) @character.special

(comment) @comment @spell
