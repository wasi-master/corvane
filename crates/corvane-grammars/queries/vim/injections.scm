; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/vim/).
; Source: helix@ba40e547426b queries/vim (MPL-2.0)
((comment) @injection.content
  (#set! injection.language "comment"))
((set_item
  option: (option_name) @_option
  value: (set_value) @injection.content)
  (#any-of? @_option
    "includeexpr" "inex" "printexpr" "pexpr" "formatexpr" "fex" "indentexpr" "inde" "foldtext" "fdt"
    "foldexpr" "fde" "diffexpr" "dex" "patchexpr" "pex" "charconvert" "ccv")
  (#set! injection.language "vim"))

; If we support perl at some point...
; (perl_statement (script (body) @perl))
; (perl_statement (chunk) @perl)
(autocmd_statement
  (pattern) @injection.content
  (#set! injection.language "regex"))

(python_statement
  (chunk) @injection.content
  (#set! injection.language "python"))

(python_statement
  (script
    (body) @injection.content
    (#set! injection.language "python")))

(ruby_statement
  (chunk) @injection.content
  (#set! injection.language "ruby"))

(ruby_statement
  (script
    (body) @injection.content
    (#set! injection.language "ruby")))

(lua_statement
  (chunk) @injection.content
  (#set! injection.language "lua"))

(lua_statement
  (script
    (body) @injection.content
    (#set! injection.language "lua")))

