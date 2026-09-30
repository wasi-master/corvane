; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/prql/).
; Source: nvim-treesitter@728e031f6b11 queries/prql (Apache-2.0)
; dropped predicates tree-sitter-highlight cannot evaluate: #offset!
((s_string) @injection.content
  (#set! injection.language "sql")
  )

(from_text
  (keyword_from_text)
  (keyword_json)
  (literal) @injection.content
  (#set! injection.language "json")
  )

((comment) @injection.content
  (#set! injection.language "comment"))
