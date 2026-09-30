; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/sourcepawn/).
; Source: nvim-treesitter@728e031f6b11 queries/sourcepawn (Apache-2.0)
; Parse JSDoc annotations in comments
((comment) @injection.content
  (#set! injection.language "jsdoc"))
