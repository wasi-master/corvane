; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/wxml/).
; Source: nvim-treesitter@728e031f6b11 queries/wxml (Apache-2.0)
((comment) @injection.content
  (#set! injection.language "comment"))

((raw_text) @injection.content
  (#set! injection.language "javascript")
  (#set! injection.include-children))

((expression) @injection.content
  (#set! injection.language "javascript")
  (#set! injection.include-children))
