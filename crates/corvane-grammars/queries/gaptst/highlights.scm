; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/gaptst/).
; Source: nvim-treesitter@728e031f6b11 queries/gaptst (Apache-2.0)
(output_line) @markup.raw.block

[
  "#@local"
  "#@exec"
] @keyword

[
  "gap> "
  "> "
] @keyword.debug

[
  "#@if"
  "#@else"
  "#@fi"
] @keyword.conditional

(comment) @comment @spell
