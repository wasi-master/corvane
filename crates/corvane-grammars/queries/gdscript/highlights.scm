; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/gdscript/).
; Source: helix@ba40e547426b queries/gdscript (MPL-2.0)
; dropped 1 pattern(s) using underscore (not in the pinned grammar)
[
  "setget"
  "onready"
  "extends"
  "set"
  "get"
  "await"
] @keyword

[
  (remote_keyword)
  (static_keyword)
  "const"
  "signal"
  "@"
] @keyword.storage.modifier

[
  "var"
  "class"
  "class_name"
  "enum"
] @keyword.storage.type


[
  "in"
  "is"
  "as"
  "and"
  "or"
  "not"
] @keyword.operator

[
  "export"
] @keyword.control.import

[
  "func"
] @keyword.function

[
  "return"
  "pass"
  "break"
  "continue"
] @keyword.control.return

[
  "while"
  "for"
] @keyword.control.repeat

[
  "if"
  "else"
  "elif"
  "match"
  "when"
] @keyword.control.conditional

(annotation (identifier) @keyword.storage.modifier)

[
  "+"
  "-"
  "*"
  "/"
  "%"
  "=="
  "!="
  ">"
  "<"
  ">="
  "<="
  "="
  "+="
  "-="
  "*="
  "/="
  "%="
  "&"
  "|"
  "^"
  "~"
  "<<"
  ">>"
  ":="
  ":"
] @operator

(null) @constant.builtin

[
  (true)
  (false)
] @constant.builtin.boolean
(escape_sequence) @constant.character.escape
(float) @constant.numeric.float
(integer) @constant.numeric.integer
(const_statement (name) @constant)
(signal_statement (name) @label)

[
  (string_name)
  (node_path)
  (get_node)
] @label
(attribute 
  (identifier) @type.builtin
  (#match? @type.builtin "^(AABB|Array|Basis|bool|Callable|Color|Dictionary|float|int|NodePath|Object|Packed(Byte|Color|String)Array|PackedFloat(32|64)Array|PackedInt(32|64)Array|PackedVector(2|3)Array|Plane|Projection|Quaternion|Rect2([i]{0,1})|RID|Signal|String|StringName|Transform(2|3)D|Variant|Vector(2|3|4)([i]{0,1}))$"))

(attribute 
  (identifier) 
  (identifier) @variable.other.member)
(variable_statement (identifier) @variable)
(enumerator (identifier) @type.enum.variant)
(enum_definition (name) @type.enum)
(binary_operator (identifier) @type)
(expression_statement (array (identifier) @type))
(type) @type
(string) @string

;; Literals
(comment) @comment
(lambda (parameters) @variable.parameter)


(constructor_definition "_init" @function)
; Function definitions

(function_definition 
  name: (name) @function
  parameters: (parameters) @variable.parameter )
(lambda (name) @function)

(call (identifier) @function)
(base_call (identifier) @function)
; Function calls

(attribute_call (identifier) @function)
(class_definition (name) @type)


; class
(class_name_statement (name) @type)
; Identifier naming conventions

(
  (identifier) @constant 
  (#match? @constant "^[A-Z][A-Z\\d_]+$"))

