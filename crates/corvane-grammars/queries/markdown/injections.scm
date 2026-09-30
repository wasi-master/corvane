; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/markdown/).
; Source: nvim-treesitter@728e031f6b11 queries/markdown (Apache-2.0)
; dropped predicates tree-sitter-highlight cannot evaluate: #offset!
(fenced_code_block
  (info_string
    (language) @injection.language)
  (code_fence_content) @injection.content)

((html_block) @injection.content
  (#set! injection.language "html")
  (#set! injection.combined)
  (#set! injection.include-children))

((minus_metadata) @injection.content
  (#set! injection.language "yaml")
  
  (#set! injection.include-children))

((plus_metadata) @injection.content
  (#set! injection.language "toml")
  
  (#set! injection.include-children))

([
  (inline)
  (pipe_table_cell)
] @injection.content
  (#set! injection.language "markdown_inline"))
