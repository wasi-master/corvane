; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/systemverilog/).
; Source: nvim-treesitter@728e031f6b11 queries/systemverilog (Apache-2.0)
([
  (one_line_comment)
  (block_comment)
] @injection.content
  (#set! injection.language "comment"))

((macro_text) @injection.content
  (#set! injection.language "verilog"))
