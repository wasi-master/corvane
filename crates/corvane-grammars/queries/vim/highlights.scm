; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/vim/).
; Source: helix@ba40e547426b queries/vim (MPL-2.0)
((set_item
  option: (option_name) @_option
  value: (set_value) @function)
  (#any-of? @_option "tagfunc" "tfu" "completefunc" "cfu" "omnifunc" "ofu" "operatorfunc" "opfunc"))
(set_item
  "?" @operator)

(inv_option
  "!" @operator)

; Options
((set_value) @constant.numeric
  (#match? @constant.numeric "^[0-9]+([.][0-9]+)?$"))

(ternary_expression
  [
    "?"
    ":"
  ] @keyword.operator)

[
  ","
  ":"
] @punctuation.delimiter

(field_expression field: (identifier) @variable.other.member)

(field_expression
  "." @punctuation.delimiter)
; Punctuation
[
  "("
  ")"
  "{"
  "}"
  "["
  "]"
  "#{"
] @punctuation.bracket

(binary_operation
  "." @operator)

; Some characters have different meanings based on the context
(unary_operation
  "!" @operator)

; Operators
[
  "||"
  "&&"
  "&"
  "+"
  "-"
  "*"
  "/"
  "%"
  ".."
  "is"
  "isnot"
  "=="
  "!="
  ">"
  ">="
  "<"
  "<="
  "=~"
  "!~"
  "="
  "+="
  "-="
  "*="
  "/="
  "%="
  ".="
  "..="
  "<<"
  "=<<"
  (match_case)
] @operator

((scoped_identifier
  (scope) @_scope
  .
  (identifier) @constant.builtin.boolean)
  (#eq? @_scope "v:")
  (#any-of? @constant.builtin.boolean "true" "false"))

(literal_dictionary
  (literal_key) @variable.parameter)

[
  (marker_definition)
  (endmarker)
] @label

(heredoc
  (parameter) @keyword)

(heredoc
  (body) @string)

(filename) @string.special.path

(pattern_multi) @string.regexp

(pattern) @string.special

(line_continuation_comment) @comment

(comment) @comment

(float_literal) @constant.numeric.float

(integer_literal) @constant.numeric.integer

; Literals
(string_literal) @string

; Scriptencoding command
(scriptencoding_statement
  (encoding) @string.special)

; Colorscheme command
(colorscheme_statement
  (name) @string)

; Runtime command
(runtime_statement
  (where) @keyword.operator)

(plus_cmd
  "+" @variable.parameter) @variable.parameter

; Edit command
(plus_plus_opt
  val: _? @constant) @variable.parameter

(command_attribute
  val: (behavior
         _ @constant))

(command_attribute
  name: _ @variable.parameter)

; Command command
(command) @string

(highlight_statement
  [
    "default"
    "link"
    "clear"
  ] @keyword)

(hl_group) @type

; Highlight command
(hl_attribute
  key: _ @variable.parameter
  val: _ @constant)

(normal_statement
  (commands) @constant)

(au_event) @constant

(augroup_name) @namespace

[
  "<buffer>"
  "<nowait>"
  "<silent>"
  "<script>"
  "<expr>"
  "<unique>"
] @constant.builtin

(syntax_argument
  name: _ @keyword)

(syntax_statement
  [
    "enable"
    "on"
    "off"
    "reset"
    "case"
    "spell"
    "foldlevel"
    "iskeyword"
    "keyword"
    "match"
    "cluster"
    "region"
    "clear"
    "include"
  ] @keyword)

; Syntax command
(syntax_statement
  (keyword) @string)

; Filetype command
(filetype_statement
  [
    "detect"
    "plugin"
    "indent"
    "on"
    "off"
  ] @keyword)

(command_name) @function.macro

(keycode) @constant.character.escape

(map_statement
  cmd: _ @keyword)

; Commands and user defined commands
[
  "let"
  "unlet"
  "const"
  "call"
  "execute"
  "normal"
  "set"
  "setfiletype"
  "setlocal"
  "silent"
  "echo"
  "echon"
  "echohl"
  "echomsg"
  "echoerr"
  "autocmd"
  "augroup"
  "return"
  "syntax"
  "filetype"
  "source"
  "lua"
  "ruby"
  "perl"
  "python"
  "highlight"
  "command"
  "delcommand"
  "comclear"
  "colorscheme"
  "scriptencoding"
  "startinsert"
  "stopinsert"
  "global"
  "runtime"
  "wincmd"
  "cnext"
  "cprevious"
  "cNext"
  "vertical"
  "leftabove"
  "aboveleft"
  "rightbelow"
  "belowright"
  "topleft"
  "botright"
  (unknown_command_name)
  "edit"
  "enew"
  "find"
  "ex"
  "visual"
  "view"
  "eval"
  "sign"
] @keyword

[
  (scope)
  "a:"
  "$"
] @namespace

[
  (no_option)
  (inv_option)
  (default_option)
  (option_name)
] @variable.builtin

[
  (bang)
  (spread)
] @punctuation.special

(default_parameter
  (identifier) @variable.parameter)

(parameters
  (identifier) @variable.parameter)

(call_expression
  function:
    (scoped_identifier
      (identifier) @function))

(call_expression
  function: (identifier) @function)

; Function related
(function_declaration
  name: (_) @function)

[
  "function"
  "endfunction"
] @keyword.function

[
  "for"
  "endfor"
  "in"
  "while"
  "endwhile"
  "break"
  "continue"
] @keyword.control.repeat

[
  "try"
  "catch"
  "finally"
  "endtry"
  "throw"
] @keyword.control.exception

; Keywords
[
  "if"
  "else"
  "elseif"
  "endif"
] @keyword.control.conditional

((identifier) @constant
  (#match? @constant "^[A-Z][A-Z_0-9]*$"))

(identifier) @variable

