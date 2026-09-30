; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/ssh_config/).
; Source: nvim-treesitter@728e031f6b11 queries/ssh_config (Apache-2.0)
(parameter
  keyword: "Tag"
  argument: (string) @local.reference)

(condition
  criteria: "tagged"
  argument: (pattern) @local.definition)
