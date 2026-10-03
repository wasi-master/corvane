; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/elm/).
; Source: nvim-treesitter@728e031f6b11 queries/elm (Apache-2.0)
([
  (line_comment)
  (block_comment)
] @injection.content
  (#set! injection.language "comment"))

((glsl_content) @injection.content
  (#set! injection.language "glsl"))
