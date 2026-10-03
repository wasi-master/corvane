; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/qmldir/).
; Source: nvim-treesitter@728e031f6b11 queries/qmldir (Apache-2.0)
; Preproc
(command
  (identifier) @keyword.directive)

; Keywords
(keyword) @keyword

; Literals
(number) @number

(float) @number.float

; Variables
[
  (identifier)
  (unit)
] @variable

; Comments
(comment) @comment @spell
