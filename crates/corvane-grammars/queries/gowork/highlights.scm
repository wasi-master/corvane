; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/gowork/).
; Source: nvim-treesitter@728e031f6b11 queries/gowork (Apache-2.0)
[
  "replace"
  "go"
  "use"
] @keyword

"=>" @operator

(comment) @comment @spell

[
  (version)
  (go_version)
] @string
