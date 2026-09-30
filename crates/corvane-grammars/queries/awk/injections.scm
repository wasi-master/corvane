; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/awk/).
; Source: nvim-treesitter@728e031f6b11 queries/awk (Apache-2.0)
((comment) @injection.content
  (#set! injection.language "comment"))

((regex) @injection.content
  (#set! injection.language "regex"))

((print_statement
  (exp_list
    .
    (string) @injection.content))
  (#set! injection.language "printf"))

((printf_statement
  (exp_list
    .
    (string) @injection.content))
  (#set! injection.language "printf"))
