; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/vento/).
; Source: nvim-treesitter@728e031f6b11 queries/vento (Apache-2.0)
(comment) @comment @spell

[
  "if"
  "/if"
  "else"
  "for"
  "/for"
  "layout"
  "/layout"
  "set"
  "/set"
  "import"
  "export"
  "/export"
  "include"
  "function"
  "/function"
  "fragment"
  "/fragment"
  "of"
  "async"
] @keyword

(tag
  [
    "{{"
    "{{-"
    "}}"
    "-}}"
  ] @punctuation.special)

"|>" @operator
