; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/swift/).
; Source: nvim-treesitter@728e031f6b11 queries/swift (Apache-2.0)
((regex_literal) @injection.content
  (#set! injection.language "regex"))

([
  (comment)
  (multiline_comment)
] @injection.content
  (#set! injection.language "comment"))
