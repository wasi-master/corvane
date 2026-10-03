; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/jjdescription/).
; Source: nvim-treesitter@728e031f6b11 queries/jjdescription (Apache-2.0)
((comment_content) @injection.content
  (#set! injection.language "comment"))

((rest) @injection.content
  (#match? @injection.content "^diff")
  (#set! injection.language "diff"))
