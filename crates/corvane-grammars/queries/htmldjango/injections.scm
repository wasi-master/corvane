; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/htmldjango/).
; Source: nvim-treesitter@728e031f6b11 queries/htmldjango (Apache-2.0)
([
  (paired_comment)
  (unpaired_comment)
] @injection.content
  (#set! injection.language "comment"))

((content) @injection.content
  (#set! injection.language "html")
  (#set! injection.combined))
