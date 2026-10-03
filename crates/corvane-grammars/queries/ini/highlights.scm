; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/ini/).
; Source: nvim-treesitter@728e031f6b11 queries/ini (Apache-2.0)
(section_name
  (text) @markup.heading)

(comment) @comment @spell

[
  "["
  "]"
] @punctuation.bracket

"=" @operator

(setting
  (setting_name) @property)

(setting_value) @string
