; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/markdown_inline/).
; Source: tree-sitter-md 0.5.3 (MIT): tree-sitter-markdown-inline/queries/injections.scm
((html_tag) @injection.content
  (#set! injection.language "html"))

((latex_block) @injection.content
  (#set! injection.language "latex"))
