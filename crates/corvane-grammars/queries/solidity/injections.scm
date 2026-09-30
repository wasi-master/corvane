; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/solidity/).
; Source: nvim-treesitter@728e031f6b11 queries/solidity (Apache-2.0)
((comment) @injection.content
  (#set! injection.language "comment"))

((comment) @injection.content
  (#match? @injection.content "^///[^/]")
  (#set! injection.language "doxygen"))

((comment) @injection.content
  (#match? @injection.content "^///$")
  (#set! injection.language "doxygen"))

((comment) @injection.content
  (#match? @injection.content "^/[*][*][^*].*[*]/$")
  (#set! injection.language "doxygen"))
