; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/eds/).
; Source: nvim-treesitter@728e031f6b11 queries/eds (Apache-2.0)
"=" @punctuation.delimiter

[
  "["
  "]"
] @punctuation.bracket

((section_name) @variable.builtin
  (#match? @variable.builtin "(?i)^(FileInfo|DeviceInfo|DummyUsage|MandatoryObjects|OptionalObjects)$"))

((section_name) @variable.builtin
  (#match? @variable.builtin "^1"))

(section
  (section_name) @_name
  (#match? @_name "(?i)^Comments$")) @comment

(section
  (section_name) @_name
  (statement
    (key) @_key) @string
  (#match? @_key "(?i)^ParameterName$")
  (#not-match? @_name "(?i)^Comments$"))

(section
  (section_name) @_name
  (statement
    (key) @_key) @type
  (#match? @_key "(?i)^(ObjectType|DataType|AccessType)$")
  (#not-match? @_name "(?i)^Comments$"))

(section
  (section_name) @_name
  (statement
    (key) @_key) @attribute
  (#match? @_key "(?i)^PDOMapping$")
  (#not-match? @_name "(?i)^Comments$"))

(section
  (section_name) @_name
  (statement
    (key) @_key) @number
  (#match? @_key "(?i)^(DefaultValue|LowLimit|HighLimit|SubNumber)$")
  (#not-match? @_name "(?i)^Comments$"))
