; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/powershell/).
; Source: nvim-treesitter@728e031f6b11 queries/powershell (Apache-2.0)
; dropped predicates tree-sitter-highlight cannot evaluate: #offset!
; comments and block-comments
((comment) @injection.content
  (#set! injection.language "comment"))

; dynamic invocation like & "$env:ChocolateyInstall\bin\choco.exe"
(command
  (command_invokation_operator) @_operator
  (command_name_expr
    (string_literal) @injection.content)
  (#eq? @_operator "&")
  (#set! injection.language "powershell")
  
  (#set! injection.include-children))

; switch -regex
(switch_statement
  (switch_parameters
    (switch_parameter) @_parameter)
  (switch_body
    (switch_clauses
      (switch_clause
        (switch_clause_condition
          (string_literal) @injection.content))))
  (#eq? @_parameter "-regex")
  (#set! injection.language "regex")
  
  (#set! injection.include-children))
