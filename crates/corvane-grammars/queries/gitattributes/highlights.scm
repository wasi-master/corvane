; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/gitattributes/).
; Source: nvim-treesitter@728e031f6b11 queries/gitattributes (Apache-2.0)
(dir_sep) @punctuation.delimiter

(quoted_pattern
  "\"" @punctuation.special)

(range_notation) @string.special

(range_notation
  [
    "["
    "]"
  ] @punctuation.bracket)

(wildcard) @character.special

(range_negation) @operator

(character_class) @constant

(class_range
  "-" @operator)

[
  (ansi_c_escape)
  (escaped_char)
] @string.escape

(attribute
  (attr_name) @variable.parameter)

(attribute
  (builtin_attr) @variable.builtin)

[
  (attr_reset)
  (attr_unset)
  (attr_set)
] @operator

(boolean_value) @boolean

(string_value) @string

(macro_tag) @keyword.directive

(macro_def
  macro_name: (_) @property)

; we do not lint syntax errors
; [
;   (pattern_negation)
;   (redundant_escape)
;   (trailing_slash)
; ] @error
(comment) @comment @spell
