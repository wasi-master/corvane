; Kotlin (tree-sitter-kotlin-ng) highlights, written for Corvane with Zed's
; capture names. Later patterns win.

(identifier) @variable

((identifier) @constant
  (#match? @constant "^[A-Z][A-Z0-9_]+$"))

((identifier) @boolean
  (#any-of? @boolean "true" "false"))

((identifier) @constant.builtin
  (#eq? @constant.builtin "null"))

(this_expression) @variable.special
(super_expression) @variable.special

; types
(user_type (identifier) @type)
(type_alias type: (identifier) @type)
(class_declaration name: (identifier) @type)
(object_declaration name: (identifier) @type)
(enum_entry (identifier) @constant)
(type_parameter (identifier) @type)

; functions
(function_declaration name: (identifier) @function)
(call_expression (identifier) @function.call)
(call_expression
  (navigation_expression (identifier) @function.method.call .))

; members, parameters
(navigation_expression (identifier) @property .)
(parameter (identifier) @variable.parameter)
(class_parameter (identifier) @variable.parameter)
(lambda_parameters (variable_declaration (identifier) @variable.parameter))
(label) @label

; packages
(package_header (qualified_identifier (identifier) @module))
(import (qualified_identifier (identifier) @module))

; annotations
(annotation) @attribute
(annotation (user_type (identifier) @attribute))

; literals
(string_literal) @string
(multiline_string_literal) @string
(character_literal) @string
(escape_sequence) @string.escape
(interpolation) @embedded
(number_literal) @number
(float_literal) @number

; comments
(line_comment) @comment
(block_comment) @comment
(shebang) @keyword.directive

; keywords
[
  "package" "import" "class" "interface" "object" "fun" "val" "var" "typealias"
  "constructor" "init" "companion" "by" "where" "get" "set"
] @keyword

[
  "if" "else" "when" "for" "while" "do" "return" "throw" "try" "catch" "finally"
] @keyword.control

["as" "is" "in"] @keyword.operator

(visibility_modifier) @keyword.modifier
(inheritance_modifier) @keyword.modifier
(member_modifier) @keyword.modifier
(class_modifier) @keyword.modifier
(function_modifier) @keyword.modifier
(property_modifier) @keyword.modifier
(parameter_modifier) @keyword.modifier
(platform_modifier) @keyword.modifier
(variance_modifier) @keyword.modifier
(reification_modifier) @keyword.modifier

; punctuation
["(" ")" "[" "]" "{" "}"] @punctuation.bracket
["," "." ";" ":" "::"] @punctuation.delimiter
["?." "!!" "->" "=" "==" "!=" "+" "-" "*" "/" "%" "<" ">" "<=" ">=" "&&" "||" "!" ".." "+=" "-="] @operator

; string templates: the expression inside keeps its own colours
(interpolation (identifier) @variable)
