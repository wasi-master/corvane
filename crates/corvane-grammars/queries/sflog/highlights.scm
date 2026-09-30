; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/sflog/).
; Source: nvim-treesitter@728e031f6b11 queries/sflog (Apache-2.0)
; highlights.scm
[
  "|"
  "|["
  "]"
  "("
  ")"
  "|("
  ")|"
] @punctuation.bracket

[
  ","
  ";"
  ":"
] @punctuation.delimiter

"EXTERNAL" @keyword

"out of" @property

(number) @number

(identifier) @variable

(version) @string.special

(anonymous_block) @string

(limit) @property

(time) @function

(limit
  (identifier) @string)

(event_detail
  (event_detail_value) @string)

(log_level_setting
  (component) @type)

(log_level_setting
  (log_level) @constant)

(log_entry
  (event_identifier
    (identifier) @type))
