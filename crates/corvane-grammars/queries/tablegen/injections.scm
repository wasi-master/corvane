; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/tablegen/).
; Source: nvim-treesitter@728e031f6b11 queries/tablegen (Apache-2.0)
; dropped predicates tree-sitter-highlight cannot evaluate: #offset!
((code) @injection.content
  (#set! injection.language "cpp")
  )

((tablegen_file
  (comment) @injection.content)
  (#match? @injection.content "^.*RUN")
  (#set! injection.language "bash")
  )
