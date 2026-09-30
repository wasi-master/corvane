; Synced by tools/ts-queries/sync.py; do not edit (additions: tools/ts-queries/patches/typescript/).
; Source: nvim-treesitter@728e031f6b11 queries/typescript (Apache-2.0)
; dropped predicates tree-sitter-highlight cannot evaluate: #offset!
; inherited from ecma
(((comment) @_jsdoc_comment
  (#match? @_jsdoc_comment "^/[*][*][^*].*[*]/$")) @injection.content
  (#set! injection.language "jsdoc"))

((comment) @injection.content
  (#set! injection.language "comment"))

; html(`...`), html`...`, sql(`...`), etc.
(call_expression
  function: (identifier) @injection.language
  arguments: [
    (arguments
      (template_string) @injection.content)
    (template_string) @injection.content
  ]
  (#match? @injection.language "^[a-zA-Z][a-zA-Z0-9]*$")
  
  (#set! injection.include-children)
  ; Languages excluded from auto-injection due to special rules
  ; - svg uses the html parser
  ; - css uses the styled parser
  (#not-any-of? @injection.language "svg" "css"))

; svg`...` or svg(`...`)
(call_expression
  function: (identifier) @_name
  (#eq? @_name "svg")
  arguments: [
    (arguments
      (template_string) @injection.content)
    (template_string) @injection.content
  ]
  
  (#set! injection.include-children)
  (#set! injection.language "html"))

; Vercel PostgreSQL
; foo.sql`...` or foo.sql(`...`)
(call_expression
  function: (member_expression
    property: (property_identifier) @injection.language)
  arguments: [
    (arguments
      (template_string) @injection.content)
    (template_string) @injection.content
  ]
  (#eq? @injection.language "sql")
  
  (#set! injection.include-children))

; Sanity CMS GROQ query
; defineQuery(`...`)
(call_expression
  function: (identifier) @_name
  (#eq? @_name "defineQuery")
  arguments: (arguments
    (template_string) @injection.content)
  
  (#set! injection.include-children)
  (#set! injection.language "groq"))

; gql`...` or gql(`...`)
(call_expression
  function: (identifier) @_name
  (#eq? @_name "gql")
  arguments: [
    (arguments
      (template_string) @injection.content)
    (template_string) @injection.content
  ]
  
  (#set! injection.include-children)
  (#set! injection.language "graphql"))

(call_expression
  function: (identifier) @_name
  (#eq? @_name "hbs")
  arguments: (template_string) @injection.content
  
  (#set! injection.include-children)
  (#set! injection.language "glimmer"))

; css`<css>`, keyframes`<css>`
(call_expression
  function: (identifier) @_name
  (#any-of? @_name "css" "keyframes")
  arguments: (template_string) @injection.content
  
  (#set! injection.include-children)
  (#set! injection.language "styled"))

; styled.div`<css>`
(call_expression
  function: (member_expression
    object: (identifier) @_name
    (#eq? @_name "styled"))
  arguments: ((template_string) @injection.content
    
    (#set! injection.include-children)
    (#set! injection.language "styled")))

; styled(Component)`<css>`
(call_expression
  function: (call_expression
    function: (identifier) @_name
    (#eq? @_name "styled"))
  arguments: ((template_string) @injection.content
    
    (#set! injection.include-children)
    (#set! injection.language "styled")))

; styled.div.attrs({ prop: "foo" })`<css>`
(call_expression
  function: (call_expression
    function: (member_expression
      object: (member_expression
        object: (identifier) @_name
        (#eq? @_name "styled"))))
  arguments: ((template_string) @injection.content
    
    (#set! injection.include-children)
    (#set! injection.language "styled")))

; styled(Component).attrs({ prop: "foo" })`<css>`
(call_expression
  function: (call_expression
    function: (member_expression
      object: (call_expression
        function: (identifier) @_name
        (#eq? @_name "styled"))))
  arguments: ((template_string) @injection.content
    
    (#set! injection.include-children)
    (#set! injection.language "styled")))

((regex_pattern) @injection.content
  (#set! injection.language "regex"))

; ((comment) @_gql_comment
;   (#eq? @_gql_comment "/* GraphQL */")
;   (template_string) @injection.content
;   (#set! injection.language "graphql"))
((template_string) @injection.content
  (#match? @injection.content "^`#graphql")
  
  (#set! injection.include-children)
  (#set! injection.language "graphql"))

; el.innerHTML = `<html>`
(assignment_expression
  left: (member_expression
    property: (property_identifier) @_prop
    (#any-of? @_prop "outerHTML" "innerHTML"))
  right: (template_string) @injection.content
  
  (#set! injection.include-children)
  (#set! injection.language "html"))

; el.innerHTML = '<html>'
(assignment_expression
  left: (member_expression
    property: (property_identifier) @_prop
    (#any-of? @_prop "outerHTML" "innerHTML"))
  right: (string
    (string_fragment) @injection.content)
  (#set! injection.language "html"))

;---- Angular injections -----
; @Component({
;   template: `<html>`
; })
(decorator
  (call_expression
    function: ((identifier) @_name
      (#eq? @_name "Component"))
    arguments: (arguments
      (object
        (pair
          key: ((property_identifier) @_prop
            (#eq? @_prop "template"))
          value: ((template_string) @injection.content
            
            (#set! injection.include-children)
            (#set! injection.language "angular")))))))

; @Component({
;   styles: [`<css>`]
; })
(decorator
  (call_expression
    function: ((identifier) @_name
      (#eq? @_name "Component"))
    arguments: (arguments
      (object
        (pair
          key: ((property_identifier) @_prop
            (#eq? @_prop "styles"))
          value: (array
            ((template_string) @injection.content
              
              (#set! injection.include-children)
              (#set! injection.language "css"))))))))

; @Component({
;   styles: `<css>`
; })
(decorator
  (call_expression
    function: ((identifier) @_name
      (#eq? @_name "Component"))
    arguments: (arguments
      (object
        (pair
          key: ((property_identifier) @_prop
            (#eq? @_prop "styles"))
          value: ((template_string) @injection.content
            (#set! injection.include-children)
            
            (#set! injection.language "css")))))))



; styled.div<{}>`<css>`
(call_expression
  function: (non_null_expression
    (instantiation_expression
      (member_expression
        object: (identifier) @_name
        (#eq? @_name "styled")
        property: (property_identifier))
      type_arguments: (type_arguments)))
  arguments: ((template_string) @injection.content
    
    (#set! injection.include-children)
    (#set! injection.language "styled")))

; styled.div<T>`<css>`
(binary_expression
  left: (binary_expression
    left: (member_expression
      object: (identifier) @_name
      (#eq? @_name "styled")
      property: (property_identifier))
    right: (identifier))
  right: (template_string) @injection.content
  
  (#set! injection.include-children)
  (#set! injection.language "styled"))
