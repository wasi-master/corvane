; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/strace/).
; Source: nvim-treesitter@728e031f6b11 queries/strace (Apache-2.0)
[
  "killed"
  "by"
  "exited"
  "with"
  "<unfinished ...>"
  "<..."
  "resumed>"
] @keyword

[
  (errorName)
  (errorDescription)
] @keyword.exception

(syscall) @function.builtin

; Literals
[
  (integer)
  (pointer)
] @number

(value) @label

(string) @string

[
  "="
  "|"
  "*"
  "&&"
  "=="
] @operator

; Punctuation
[
  "+++"
  "---"
  "..."
  "~"
] @punctuation.special

[
  "("
  ")"
  "["
  "]"
] @punctuation.bracket

[
  ","
  "=>"
] @punctuation.delimiter

(comment) @comment @spell
