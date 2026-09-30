; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/lalrpop/).
; Source: nvim-treesitter@728e031f6b11 queries/lalrpop (Apache-2.0)
; dropped predicates tree-sitter-highlight cannot evaluate: #offset!
((comment) @injection.content
  (#set! injection.language "comment"))

([
  (normal_action)
  (failible_action)
] @injection.content
  (#set! injection.language "rust"))

((use) @injection.content
  (#set! injection.language "rust"))

((regex_literal) @injection.content
  (#set! injection.language "regex")
  )
