; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/git_config/).
; Source: nvim-treesitter@728e031f6b11 queries/git_config (Apache-2.0)
; dropped predicates tree-sitter-highlight cannot evaluate: #offset!
((comment) @injection.content
  (#set! injection.language "comment"))

((variable
  (name) @_name
  value: (string) @injection.content)
  (#any-of? @_name "cmd" "command" "textconv" "sendmailCmd" "sendmailcmd")
  (#set! injection.language "bash"))

(section
  (variable
    (name) @_name
    value: (string) @injection.content)
  (#eq? @_name "tool")
  (#set! injection.language "bash"))

(section
  (section_header
    (section_name) @_pager)
  (variable
    value: (string) @injection.content)
  (#eq? @_pager "pager")
  (#set! injection.language "bash"))

(section
  (section_header
    (section_name) @_interactive)
  (variable
    (name) @_name
    value: (string) @injection.content)
  (#eq? @_interactive "interactive")
  (#any-of? @_name "diffFilter" "difffilter")
  (#set! injection.language "bash"))

; https://github.com/git-lfs/git-lfs
; git lfs install
(section
  (section_header
    (section_name) @_filter
    (subsection_name) @_lfs)
  (variable
    (name) @_name
    value: (string) @injection.content)
  (#eq? @_filter "filter")
  (#eq? @_lfs "lfs")
  (#any-of? @_name "smudge" "process" "clean")
  (#set! injection.language "bash"))

(section
  (section_header
    (section_name) @_alias)
  (variable
    value: (string) @injection.content)
  (#eq? @_alias "alias")
  (#match? @injection.content "^!")
  
  (#set! injection.language "bash"))

(section
  (section_header
    (section_name) @_alias)
  (variable
    value: (string
      "\""
      "\"") @injection.content)
  (#eq? @_alias "alias")
  (#match? @injection.content "^\"!")
  
  (#set! injection.language "bash"))
