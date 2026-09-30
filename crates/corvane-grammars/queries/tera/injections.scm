; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/tera/).
; Source: nvim-treesitter@728e031f6b11 queries/tera (Apache-2.0)
(frontmatter
  (content) @injection.content
  (#set! injection.language "yaml")
  (#set! injection.combined))

((comment_tag) @injection.content
  (#set! injection.language "comment"))
