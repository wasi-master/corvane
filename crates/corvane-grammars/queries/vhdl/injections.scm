; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/vhdl/).
; Source: nvim-treesitter@728e031f6b11 queries/vhdl (Apache-2.0)
(line_comment
  (comment_content) @injection.content
  (#set! injection.language "comment"))

(block_comment
  (comment_content) @injection.content
  (#set! injection.language "comment"))
