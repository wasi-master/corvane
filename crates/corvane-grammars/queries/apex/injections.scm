; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/apex/).
; Source: nvim-treesitter@728e031f6b11 queries/apex (Apache-2.0)
([
  (line_comment)
  (block_comment)
] @injection.content
  (#set! injection.language "comment"))

((block_comment) @injection.content
  (#match? @injection.content "/[*][*][\\s]")
  (#set! injection.language "javadoc"))

; markdown-style javadocs https://openjdk.org/jeps/467
((line_comment) @injection.content
  (#match? @injection.content "^///\\s")
  (#set! injection.language "javadoc"))
