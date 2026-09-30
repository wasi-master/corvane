; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/jq/).
; Source: nvim-treesitter@728e031f6b11 queries/jq (Apache-2.0)
((comment) @injection.content
  (#set! injection.language "comment"))

; test(val)
(query
  ((funcname) @_function
    (#any-of? @_function "test" "match" "capture" "scan" "split" "splits" "sub" "gsub"))
  (args
    .
    (query
      (string) @injection.content
      (#set! injection.language "regex"))))

; test(regex; flags)
(query
  ((funcname) @_function
    (#any-of? @_function "test" "match" "capture" "scan" "split" "splits" "sub" "gsub"))
  (args
    .
    (args
      (query
        (string) @injection.content
        (#set! injection.language "regex")))))
