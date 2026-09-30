; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/gaptst/).
; Source: nvim-treesitter@728e031f6b11 queries/gaptst (Apache-2.0)
((comment) @injection.content
  (#set! injection.language "comment"))

((gap_expression) @injection.content
  (#set! injection.language "gap"))

((input_line) @injection.content
  (#set! injection.language "gap")
  (#set! injection.combined))
