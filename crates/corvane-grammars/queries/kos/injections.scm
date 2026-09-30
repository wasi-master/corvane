; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/kos/).
; Source: nvim-treesitter@728e031f6b11 queries/kos (Apache-2.0)
(((comment) @_jsdoc_comment
  (#match? @_jsdoc_comment "^/[*][*][^*].*[*]/$")) @injection.content
  (#set! injection.language "jsdoc"))

((comment) @injection.content
  (#set! injection.language "comment"))
