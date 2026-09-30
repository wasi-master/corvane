; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/re2c/).
; Source: nvim-treesitter@728e031f6b11 queries/re2c (Apache-2.0)
; dropped predicates tree-sitter-highlight cannot evaluate: #offset!
((host_lang) @injection.content
  (#set! injection.parent))

((define
  option: (option_name) @_yy
  value: (dstring) @injection.content)
  (#any-of? @_yy
    "YYPEEK" "YYSKIP" "YYBACKUP" "YYBACKUPCTX" "YYRESTORE" "YYRESTORECTX" "YYFILL" "YYSHIFT")
  
  (#set! injection.parent))

((comment) @injection.content
  (#set! injection.language "comment"))
