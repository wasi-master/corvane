; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/rust/).
; Source: tree-sitter-rust 0.24.2 (MIT): queries/injections.scm
((macro_invocation
  (token_tree) @injection.content)
 (#set! injection.language "rust")
 (#set! injection.include-children))

((macro_rule
  (token_tree) @injection.content)
 (#set! injection.language "rust")
 (#set! injection.include-children))
