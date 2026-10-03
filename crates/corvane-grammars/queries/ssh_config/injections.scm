; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/ssh_config/).
; Source: nvim-treesitter@728e031f6b11 queries/ssh_config (Apache-2.0)
((comment) @injection.content
  (#set! injection.language "comment"))

((condition
  criteria: "exec"
  argument: (string) @injection.content)
  (#set! injection.language "bash"))

((parameter
  keyword: [
    "KnownHostsCommand"
    "LocalCommand"
    "RemoteCommand"
    "ProxyCommand"
  ]
  argument: (string) @injection.content)
  (#set! injection.language "bash"))
