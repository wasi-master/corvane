; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/enforce/).
; Source: nvim-treesitter@728e031f6b11 queries/enforce (Apache-2.0)
([
  (comment_block)
  (comment_line)
] @injection.content
  (#set! injection.language "comment"))

([
  (doc_block)
  (doc_line)
] @injection.content
  (#set! injection.language "doxygen"))

; TODO: string and print (numbered) format injection
