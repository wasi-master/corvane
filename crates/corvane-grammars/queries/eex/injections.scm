; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/eex/).
; Source: nvim-treesitter@728e031f6b11 queries/eex (Apache-2.0)
; EEx expressions are Elixir
((expression) @injection.content
  (#set! injection.language "elixir"))

; EEx expressions can span multiple interpolated lines
((partial_expression) @injection.content
  (#set! injection.language "elixir")
  (#set! injection.combined))
