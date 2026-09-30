; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/http/).
; Source: nvim-treesitter@728e031f6b11 queries/http (Apache-2.0)
; dropped predicates tree-sitter-highlight cannot evaluate: #offset!
; Comments
((comment) @injection.content
  (#set! injection.language "comment"))

; Body
((json_body) @injection.content
  (#set! injection.language "json"))

((xml_body) @injection.content
  (#set! injection.language "xml"))

((graphql_data) @injection.content
  (#set! injection.language "graphql"))

; Script (default to javascript)
((comment
  name: (_) @_name
  (#eq? @_name "lang")
  value: (_) @injection.language)?
  .
  (_
    (script) @injection.content
    )
  (#set! injection.language "javascript"))
