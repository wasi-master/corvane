; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/earthfile/).
; Source: nvim-treesitter@728e031f6b11 queries/earthfile (Apache-2.0)
((comment) @injection.content
  (#set! injection.language "comment"))

((line_continuation_comment) @injection.content
  (#set! injection.language "comment"))

((shell_fragment) @injection.content
  (#set! injection.language "bash")
  (#set! injection.include-children))
