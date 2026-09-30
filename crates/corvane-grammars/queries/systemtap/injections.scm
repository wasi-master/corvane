; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/systemtap/).
; Source: nvim-treesitter@728e031f6b11 queries/systemtap (Apache-2.0)
((comment) @injection.content
  (#set! injection.language "comment"))

((embedded_code) @injection.content
  (#set! injection.language "c"))
