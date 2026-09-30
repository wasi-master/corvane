; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/gosum/).
; Source: nvim-treesitter@728e031f6b11 queries/gosum (Apache-2.0)
[
  "alpha"
  "beta"
  "dev"
  "pre"
  "rc"
  "+incompatible"
] @keyword

(module_path) @string.special.url

(module_version) @string.special

(hash_version) @attribute

(hash) @string.special.symbol

[
  (number)
  (number_with_decimal)
  (hex_number)
] @number

(checksum
  "go.mod" @string)

[
  ":"
  "."
  "-"
  "/"
] @punctuation.delimiter
