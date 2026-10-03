; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/promql/).
; Source: nvim-treesitter@728e031f6b11 queries/promql (Apache-2.0)
; dropped predicates tree-sitter-highlight cannot evaluate: #offset!
((comment) @injection.content
  (#set! injection.language "comment"))

((label_name)
  [
    "=~"
    "!~"
  ]
  (label_value) @injection.content
  (#set! injection.language "regex")
  )
