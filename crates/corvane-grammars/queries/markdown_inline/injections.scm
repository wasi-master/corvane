; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/markdown_inline/).
; Source: nvim-treesitter@728e031f6b11 queries/markdown_inline (Apache-2.0)
((html_tag) @injection.content
  (#set! injection.language "html")
  (#set! injection.combined))

((latex_block) @injection.content
  (#set! injection.language "latex")
  (#set! injection.include-children))
