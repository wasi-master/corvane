; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/doxygen/).
; Source: nvim-treesitter@728e031f6b11 queries/doxygen (Apache-2.0)
((type) @injection.content
  (#set! injection.parent))

([
  (function_link)
  (code)
] @injection.content
  (#set! injection.parent))

((link) @injection.content
  (#set! injection.language "html"))

(code_block
  (code_block_language) @injection.language
  (code_block_content) @injection.content)
