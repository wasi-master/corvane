; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/starlark/).
; Source: nvim-treesitter@728e031f6b11 queries/starlark (Apache-2.0)
((binary_operator
  left: (string
    (string_content) @injection.content)
  operator: "%")
  (#set! injection.language "printf"))

((comment) @injection.content
  (#set! injection.language "comment"))
