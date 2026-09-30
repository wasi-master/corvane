; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/make/).
; Source: nvim-treesitter@728e031f6b11 queries/make (Apache-2.0)
((comment) @injection.content
  (#set! injection.language "comment"))

((shell_text) @injection.content
  (#set! injection.language "bash"))

((shell_command) @injection.content
  (#set! injection.language "bash"))
