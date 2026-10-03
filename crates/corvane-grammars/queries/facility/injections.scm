; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/facility/).
; Source: nvim-treesitter@728e031f6b11 queries/facility (Apache-2.0)
((remarks) @injection.content
  (#set! injection.language "markdown"))

([
  (comment)
  (doc_comment)
] @injection.content
  (#set! injection.language "comment"))
