; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/surface/).
; Source: nvim-treesitter@728e031f6b11 queries/surface (Apache-2.0)
; Surface expressions and components are Elixir code
([
  (expression_value)
  (component_name)
] @injection.content
  (#set! injection.language "elixir"))

; Surface comments are nvim-treesitter comments
((comment) @injection.content
  (#set! injection.language "comment"))
