; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/gomod/).
; Source: nvim-treesitter@728e031f6b11 queries/gomod (Apache-2.0)
[
  "require"
  "replace"
  "go"
  "toolchain"
  "exclude"
  "retract"
  "module"
] @keyword

"=>" @operator

(comment) @comment @spell

(module_path) @string.special.url

(tool_directive) @keyword.directive

(tool) @string.special.url

[
  (version)
  (go_version)
  (toolchain_name)
] @string.special

[
  "("
  ")"
  "["
  "]"
] @punctuation.bracket

"," @punctuation.delimiter
