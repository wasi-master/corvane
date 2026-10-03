; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/roc/).
; Source: nvim-treesitter@728e031f6b11 queries/roc (Apache-2.0)
;injection from function calls
(function_call_pnc_expr
  (variable_expr
    (identifier) @injection.language)
  (const
    [
      (multiline_string)
      (string)
    ] @injection.content)
  (#any-of? @injection.language
    "json" "toml" "yaml" "xml" "sql" "lua" "js" "html" "css" "http" "jq" "latex" "md" "nix" "regex"))

;injection from piping function calls
(bin_op_expr
  part: (const
    [
      (multiline_string)
      (string)
    ] @injection.content)
  part: (operator)
  part: (variable_expr
    (identifier) @injection.language)
  (#any-of? @injection.language
    "json" "toml" "yaml" "xml" "sql" "lua" "js" "html" "css" "http" "jq" "latex" "md" "nix" "regex"))

([
  (line_comment)
  (doc_comment)
] @injection.content
  (#set! injection.language "comment"))
