; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/verilog/).
; Source: helix@ba40e547426b queries/verilog (MPL-2.0)
; dropped 1 pattern(s) using config (not in the pinned grammar)
[
  "["
  "]"
  "("
  ")"
] @punctuation.bracket
(generate_block_identifier) @comment

(type_declaration
 (simple_identifier) @type)

(struct_union_member
 (data_type_or_void
  (data_type
   (simple_identifier) @type)))

(member_identifier
 (simple_identifier) @variable.other.member)

(struct_union_member
 (list_of_variable_decl_assignments
  (variable_decl_assignment
   (simple_identifier) @variable.other.member)))

[
  (integer_atom_type)
  (non_integer_type)
  "genvar"
] @type.builtin

(type_declaration
 (simple_identifier) @type)

(enum_name_declaration
 (enum_identifier
  (simple_identifier) @constant))

[
  "enum"
] @type

(struct_union) @type

;;TODO: fixme
;(assignment_pattern_expression
 ;(assignment_pattern
  ;(parameter_identifier) @variable.other.member))

(type_declaration
  (data_type ["packed"] @label))

(task_identifier
 (task_identifier
  (simple_identifier) @function.method))

(function_subroutine_call 
 (subroutine_call
  (system_tf_call
   (system_tf_identifier) @function.builtin)))

(function_subroutine_call 
 (subroutine_call
  (tf_call
   (simple_identifier) @function)))

(function_identifier 
 (function_identifier 
  (simple_identifier) @function))

(lifetime) @label

(net_declaration
 (simple_identifier) @type)

(interface_port_declaration
 (interface_identifier
  (simple_identifier) @type))

(name_of_instance
 (instance_identifier
  (simple_identifier) @variable))

(module_instantiation
 (simple_identifier) @constructor)

(checker_instantiation
 (checker_identifier
  (simple_identifier) @constructor))

(time_unit) @constant

[
  (integral_number)
  (unsigned_number)
  (unbased_unsized_literal)
] @constant.numeric

(parameter_identifier
 (simple_identifier) @variable.parameter)

(class_constructor_declaration
 "new" @constructor)

(module_declaration
 (module_header
  (simple_identifier) @constructor))

(text_macro_identifier
 (simple_identifier) @function.macro)

(default_nettype_compiler_directive
 (default_nettype_value) @string)

[
 ";"
 "::"
 ","
 "."
] @punctuation.delimiter


; begin/end label
(seq_block
 (simple_identifier) @comment)

[
  (include_compiler_directive)
  (default_nettype_compiler_directive)
  (timescale_compiler_directive)
] @keyword.directive

[
  (double_quoted_string)
  (string_literal)
] @string

(net_port_type1
 (simple_identifier) @type)

(modport_identifier
 (modport_identifier
  (simple_identifier) @variable.other.member))

(interface_identifier
 (simple_identifier) @type)

(method_call_body
  (method_identifier) @variable.other.member)

(data_type
 (simple_identifier) @type)

[
  "signed"
  "unsigned"
] @label

[
  (net_type)
  (integer_vector_type)
  (integer_atom_type)
] @type.builtin

(tf_port_item1 ("var") @type.builtin)
(data_declaration ("var") @type.builtin)
(variable_port_header ("var") @type.builtin)
(port_identifier
 (simple_identifier) @variable)

(port_direction) @label
(edge_identifier) @constant

(cast
 ["'" "(" ")"] @keyword.operator)

[
  "or"
  "and"
] @keyword.operator

[
  "="
  "-"
  "+"
  "/"
  "*"
  "**"
  "^"
  "&"
  "|"
  "&&"
  "||"
  ":"
  (unary_operator)
  "{"
  "}"
  "'{"
  "<="
  "@"
  "=="
  "!="
  "==="
  "!=="
  "-:"
  "<"
  ">"
  ">="
  "%"
  ">>"
  "<<"
  ">>>"
  "<<<"
  "|="
  (inc_or_dec_operator)
  "?"
] @operator
(parameter_port_list
 "#" @constructor)

(package_declaration
 (package_identifier
  (simple_identifier) @constant))

(package_scope
 (package_identifier
  (simple_identifier) @constant))

(text_macro_identifier
 (simple_identifier) @keyword.directive)

(package_import_declaration
 (package_import_item
  (package_identifier
   (simple_identifier) @constant)))

(package_import_declaration
 "import" @keyword.control.import)

(include_compiler_directive) @keyword.directive
(comment) @comment

[
  "if"
  "else"
  (case_keyword)
  "endcase"
] @keyword.control.conditional

[
  (always_keyword)
  "generate"
  "for"
  "foreach"
  "repeat"
  "forever"
  "initial"
  "while"
] @keyword.control

[
  "begin"
  "end"
] @label

"return" @keyword.control.return

[
  "function"
  "endfunction"

  "task"
  "endtask"
] @keyword.function

