; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/puppet/).
; Source: nvim-treesitter@728e031f6b11 queries/puppet (Apache-2.0)
; dropped predicates tree-sitter-highlight cannot evaluate: #offset!
((regex) @injection.content
  (#set! injection.language "regex")
  )

((comment) @injection.content
  (#set! injection.language "comment"))
