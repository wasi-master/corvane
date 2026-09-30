; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/passwd/).
; Source: nvim-treesitter@728e031f6b11 queries/passwd (Apache-2.0)
(user) @module

(auth) @string.special.symbol

(gecos) @string

(home) @string.special.path

(shell) @string.special.path

[
  (gid)
  (uid)
] @number

(separator) @punctuation.delimiter
