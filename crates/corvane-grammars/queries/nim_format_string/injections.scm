; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/nim_format_string/).
; Source: nvim-treesitter@728e031f6b11 queries/nim_format_string (Apache-2.0)
((matching_curlies
  (nim_expression
    !escaped_curly) @injection.content)
  (#set! injection.language "nim"))
