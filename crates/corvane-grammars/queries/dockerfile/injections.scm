; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/dockerfile/).
; Source: nvim-treesitter@728e031f6b11 queries/dockerfile (Apache-2.0)
((comment) @injection.content
  (#set! injection.language "comment"))

((shell_command
  (shell_fragment) @injection.content)
  (#set! injection.language "bash")
  (#set! injection.combined))

((run_instruction
  (heredoc_block) @injection.content)
  (#set! injection.language "bash")
  (#set! injection.include-children))
